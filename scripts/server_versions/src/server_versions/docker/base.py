from __future__ import annotations

import json
import shutil
import subprocess
import sys
from typing import Any

from ..config import DB_SERVICE, REPO_ROOT, SERVER_SERVICE
from ..errors import CliError
from ..process import StreamArg, run_capture, run_quiet, run_text


class DockerBase:
    def __init__(self) -> None:
        self._prefix: list[str] | None = None
        config = self._read_config()
        project = config.get("name")
        self.project = project if isinstance(project, str) and project else REPO_ROOT.name.lower()
        services = config.get("services")
        if not isinstance(services, dict):
            raise CliError("docker compose config has no services")
        db_service = services.get(DB_SERVICE)
        if not isinstance(db_service, dict):
            raise CliError(f"docker-compose.yml has no '{DB_SERVICE}' service")
        server_service = services.get(SERVER_SERVICE)
        if not isinstance(server_service, dict):
            raise CliError(f"docker-compose.yml has no '{SERVER_SERVICE}' service")
        image = server_service.get("image")
        if not isinstance(image, str) or not image:
            image = f"{self.project}-{SERVER_SERVICE}"
        if ":" not in image.rsplit("/", 1)[-1]:
            image += ":latest"
        self.compose_image = image
        self.snapshot_repo = f"{self.project}-server-snapshot"

    @staticmethod
    def _read_config() -> dict[str, Any]:
        proc = run_capture(["docker", "compose", "config", "--format", "json"], check=False)
        if proc.returncode != 0:
            detail = (
                proc.stderr.strip().splitlines()[-1] if proc.stderr.strip() else "unknown error"
            )
            raise CliError(f"docker compose config failed: {detail}")
        try:
            data = json.loads(proc.stdout)
        except json.JSONDecodeError as exc:
            raise CliError(f"Could not parse docker compose config: {exc}") from exc
        if not isinstance(data, dict):
            raise CliError("docker compose config is not a JSON object")
        return data

    def prefix(self) -> list[str]:
        if self._prefix is not None:
            return self._prefix
        if run_quiet(["docker", "info"]) == 0:
            self._prefix = ["docker"]
            return self._prefix
        if shutil.which("sudo") is None:
            raise CliError("Docker is not accessible and sudo is not installed.")
        if run_quiet(["sudo", "-n", "docker", "info"]) == 0:
            self._prefix = ["sudo", "-n", "docker"]
            return self._prefix
        if not sys.stdin.isatty():
            raise CliError(
                "Docker requires sudo. Run the script in a terminal or add your "
                "user to the docker group."
            )
        print("Docker requires sudo for this user.")
        if subprocess.run(["sudo", "-v"], check=False, cwd=REPO_ROOT).returncode != 0:
            raise CliError("sudo authentication failed.")
        if run_quiet(["sudo", "-n", "docker", "info"]) != 0:
            raise CliError("Docker daemon is not running or not accessible through sudo.")
        self._prefix = ["sudo", "-n", "docker"]
        return self._prefix

    def refresh_sudo(self) -> None:
        if self._prefix != ["sudo", "-n", "docker"]:
            return
        if run_quiet(["sudo", "-n", "docker", "info"]) == 0:
            return
        if not sys.stdin.isatty():
            raise CliError("sudo session expired; run the script in a terminal.")
        print("Refreshing sudo ...")
        if subprocess.run(["sudo", "-v"], check=False, cwd=REPO_ROOT).returncode != 0:
            raise CliError("sudo authentication failed.")
        if run_quiet(["sudo", "-n", "docker", "info"]) != 0:
            raise CliError("sudo session is still not valid.")

    def _compose_cmd(self, *args: str) -> list[str]:
        return [
            *self.prefix(),
            "compose",
            "--project-name",
            self.project,
            "--project-directory",
            str(REPO_ROOT),
            *args,
        ]

    def compose_capture(self, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
        return run_capture(self._compose_cmd(*args), check=check)

    def compose_run(
        self,
        *args: str,
        check: bool = True,
        stdin: StreamArg = None,
        stdout: StreamArg = None,
        stderr: StreamArg = None,
        env: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        return run_text(
            self._compose_cmd(*args),
            check=check,
            stdin=stdin,
            stdout=stdout,
            stderr=stderr,
            env=env,
        )

    def compose_popen(self, *args: str, **kwargs: Any) -> subprocess.Popen[bytes]:
        cmd = self._compose_cmd(*args)
        try:
            return subprocess.Popen[bytes](cmd, cwd=REPO_ROOT, **kwargs)
        except FileNotFoundError as exc:
            raise CliError(f"Command not found: {cmd[0]}") from exc
