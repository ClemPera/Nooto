from __future__ import annotations

import os
import shutil
import subprocess
import sys
import threading
from typing import IO, Protocol

from ..config import DB_SERVICE, SERVER_SERVICE, SYSTEM_DATABASES
from ..errors import CliError
from ..formatting import normalize_image_id
from ..process import run_capture
from .base import DockerBase


class _Writer(Protocol):
    def write(self, data: bytes, /) -> int: ...


def _quote_ident(name: str) -> str:
    return "`" + name.replace("`", "``") + "`"


class DockerCompose(DockerBase):
    def server_state(self) -> str | None:
        proc = self.compose_capture("ps", "-a", "-q", SERVER_SERVICE)
        container_ids = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
        if not container_ids:
            return None
        proc = run_capture(
            [
                *self.prefix(),
                "inspect",
                "--format",
                "{{.State.Status}}",
                container_ids[0],
            ],
            check=False,
        )
        if proc.returncode != 0:
            detail = proc.stderr.strip() or "docker inspect failed"
            if "No such object" in detail or "No such container" in detail:
                return None
            raise CliError(f"Could not inspect server container: {detail}")
        return proc.stdout.strip() or None

    def server_running(self) -> bool:
        return self.server_state() == "running"

    def server_active(self) -> bool:
        return self.server_state() in {"running", "restarting"}

    def server_may_be_active(self) -> bool:
        return self.server_state() not in {
            None,
            "created",
            "exited",
            "dead",
            "removing",
        }

    def server_image_id(self) -> str | None:
        proc = self.compose_capture("ps", "-a", "-q", SERVER_SERVICE)
        container_ids = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
        if container_ids:
            proc = run_capture(
                [*self.prefix(), "inspect", "--format", "{{.Image}}", container_ids[0]],
                check=False,
            )
            if proc.returncode != 0:
                detail = proc.stderr.strip() or "docker inspect failed"
                if "No such object" not in detail and "No such container" not in detail:
                    raise CliError(f"Could not inspect server image: {detail}")
            else:
                image_id = normalize_image_id(proc.stdout.strip())
                if image_id:
                    return image_id
        proc = self.compose_capture("images", "-q", SERVER_SERVICE)
        for line in proc.stdout.splitlines():
            image_id = normalize_image_id(line.strip())
            if image_id:
                return image_id
        return None

    def build_server(self) -> None:
        self.refresh_sudo()
        env = os.environ.copy()
        if not sys.stdout.isatty():
            env["BUILDKIT_PROGRESS"] = "plain"
        self.compose_run("build", SERVER_SERVICE, env=env)

    def start_server(self) -> None:
        self.refresh_sudo()
        self.compose_run(
            "up",
            "-d",
            "--no-build",
            "--no-deps",
            "--force-recreate",
            SERVER_SERVICE,
        )

    def stop_server(self) -> None:
        self.refresh_sudo()
        proc = self.compose_run(
            "stop",
            SERVER_SERVICE,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
        )
        if proc.returncode != 0:
            detail = (proc.stderr or "").strip()
            raise CliError(f"Could not stop server: {detail or 'docker compose stop failed'}")

    def start_stopped_server(self) -> None:
        proc = self.compose_run(
            "start",
            SERVER_SERVICE,
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if proc.returncode != 0:
            self.start_server()

    def port(self) -> int:
        proc = self.compose_capture("port", SERVER_SERVICE, "3000", check=False)
        for line in proc.stdout.splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                return int(line.rsplit(":", 1)[1])
            except (ValueError, IndexError):
                continue
        return 3000

    def logs(self) -> None:
        self.compose_run("logs", "--tail=80", SERVER_SERVICE, check=False)

    def ensure_db_up(self) -> None:
        self.refresh_sudo()
        self.compose_run("up", "-d", "--wait", DB_SERVICE)

    def db_query(self, sql: str, *, check: bool = True) -> str:
        script = (
            'MYSQL_PWD="$MARIADB_ROOT_PASSWORD" exec mariadb -uroot -N -B '
            '-e "$1" "$MARIADB_DATABASE"'
        )
        proc = self.compose_run(
            "exec",
            "-T",
            DB_SERVICE,
            "sh",
            "-c",
            script,
            "sh",
            sql,
            check=check,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return (proc.stdout or "").strip()

    def db_exec(self, sql: str) -> None:
        self.db_query(sql)

    def db_name(self) -> str:
        name = self.db_query("SELECT DATABASE()")
        if not name or name == "NULL":
            raise CliError("MARIADB_DATABASE is not set in the db container.")
        return name

    def db_migrations(self) -> list[int]:
        exists = self.db_query(
            "SELECT COUNT(*) FROM information_schema.tables "
            "WHERE table_schema = DATABASE() AND table_name = 'schema_migrations'"
        )
        if exists.strip() in ("", "0"):
            return []
        output = self.db_query("SELECT version FROM schema_migrations ORDER BY version")
        return [int(line) for line in output.splitlines() if line.strip().isdigit()]

    def db_charset(self) -> tuple[str | None, str | None]:
        output = self.db_query(
            "SELECT DEFAULT_CHARACTER_SET_NAME, DEFAULT_COLLATION_NAME "
            "FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = DATABASE()"
        )
        if "\t" in output:
            charset, collation = output.split("\t", 1)
            return charset, collation
        return None, None

    def db_tables(self) -> list[str]:
        output = self.db_query(
            "SELECT table_name FROM information_schema.tables "
            "WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' "
            "ORDER BY table_name"
        )
        return [line for line in output.splitlines() if line]

    def clear_db(self) -> None:
        database = self.db_name()
        if database.lower() in SYSTEM_DATABASES:
            raise CliError(f"Refusing to clear system database {database}.")
        tables = self.db_tables()
        if not tables:
            return
        drops = ", ".join(_quote_ident(table) for table in tables)
        self.db_exec(
            f"SET FOREIGN_KEY_CHECKS=0; DROP TABLE IF EXISTS {drops}; SET FOREIGN_KEY_CHECKS=1;"
        )

    def dump_db_to(self, out_file: _Writer) -> None:
        self.refresh_sudo()
        script = (
            'MYSQL_PWD="$MARIADB_ROOT_PASSWORD" exec mariadb-dump '
            "--single-transaction --quick --routines --triggers --events "
            '--default-character-set=utf8mb4 -uroot "$MARIADB_DATABASE"'
        )
        proc = self.compose_popen(
            "exec",
            "-T",
            DB_SERVICE,
            "sh",
            "-c",
            script,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        stdout = proc.stdout
        stderr = proc.stderr
        if stdout is None or stderr is None:
            proc.kill()
            proc.wait()
            raise CliError("Could not open DB dump pipes.")
        stderr_chunks: list[bytes] = []

        def drain_stderr() -> None:
            stderr_chunks.append(stderr.read())

        drain = threading.Thread(target=drain_stderr, daemon=True)
        drain.start()
        try:
            shutil.copyfileobj(stdout, out_file, length=1024 * 1024)
            code = proc.wait()
        except BaseException:
            proc.kill()
            proc.wait()
            raise
        finally:
            drain.join(timeout=5)
            stdout.close()
            stderr.close()
        if code != 0:
            detail = b"".join(stderr_chunks).decode(errors="replace").strip()
            raise CliError(f"DB dump failed ({code})" + (f"\n{detail}" if detail else ""))

    def import_db(self, in_file: IO[bytes]) -> None:
        script = 'MYSQL_PWD="$MARIADB_ROOT_PASSWORD" exec mariadb -uroot "$MARIADB_DATABASE"'
        self.compose_run(
            "exec",
            "-T",
            DB_SERVICE,
            "sh",
            "-c",
            script,
            stdin=in_file,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
