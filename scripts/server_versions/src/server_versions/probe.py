from __future__ import annotations

import http.client
import json
import socket
import time
import urllib.error
import urllib.request
from typing import TYPE_CHECKING, Any

from .errors import CliError
from .formatting import fmt_version
from .metadata import find_artifact, find_server_by_image_id
from .models import ProbeInfo, RunningServerInfo
from .spinner import Spinner

if TYPE_CHECKING:
    from .docker import Docker


def http_get_json(url: str, timeout: float = 2.0) -> dict[str, Any]:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            body = response.read()
            return {"status": response.status, "data": json.loads(body)}
    except urllib.error.HTTPError as exc:
        return {"status": exc.code, "error": f"HTTP {exc.code}"}
    except (
        urllib.error.URLError,
        TimeoutError,
        OSError,
        http.client.HTTPException,
    ) as exc:
        return {"status": None, "error": str(exc)}
    except (json.JSONDecodeError, UnicodeDecodeError):
        return {"status": 200, "error": "invalid JSON"}


class VersionProbe:
    def __init__(self, docker: Docker) -> None:
        self.docker = docker

    def current(self, *, port: int | None = None, assume_running: bool = False) -> ProbeInfo:
        if not assume_running and not self.docker.server_running():
            return ProbeInfo(
                version_source="not-running",
                server_version=None,
                protocol_version=None,
            )
        if port is None:
            port = self.docker.port()
        result = http_get_json(f"http://127.0.0.1:{port}/version")
        data = result.get("data")
        if result.get("status") == 200 and isinstance(data, dict):
            return ProbeInfo(
                version_source="http",
                server_version=data.get("server_version"),
                protocol_version=data.get("protocol_version"),
            )
        if result.get("status") == 404:
            return ProbeInfo(
                version_source="unavailable",
                server_version=None,
                protocol_version=None,
            )
        return ProbeInfo(
            version_source="unreachable",
            server_version=None,
            protocol_version=None,
        )

    def wait(self, timeout: int = 60) -> ProbeInfo:
        port = self.docker.port()
        info: ProbeInfo | None = None
        with Spinner(f"Waiting for server on 127.0.0.1:{port}"):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if not self.docker.server_running():
                    time.sleep(1)
                    continue
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=1):
                        pass
                except OSError:
                    time.sleep(0.5)
                    continue
                candidate = self.current(port=port, assume_running=True)
                if candidate.get("version_source") in ("http", "unavailable"):
                    info = candidate
                    break
                time.sleep(0.5)
            else:
                raise CliError(f"Server did not start within {timeout}s.")
        if info is None:
            raise CliError("Server did not report a usable version.")
        if not self.docker.server_running():
            raise CliError("Server exited during startup.")
        return info


def running_server_info(docker: Docker) -> RunningServerInfo:
    if not docker.server_running():
        return RunningServerInfo(
            image_id=None,
            image_created=None,
            server_version=None,
            protocol_version=None,
            version_source="not-running",
            source_known=False,
            matched_artifact=None,
        )
    image_id = docker.server_image_id()
    image_created = docker.image_created(image_id) if image_id else None
    version = VersionProbe(docker).current()
    matched = find_server_by_image_id(image_id)
    matched_source = matched.get("source") if matched else None
    known = bool(isinstance(matched_source, dict) and matched_source.get("known"))
    server_version = version.get("server_version")
    protocol_version = version.get("protocol_version")
    version_source = version.get("version_source", "unknown")
    if matched and server_version is None:
        matched_server = matched.get("server")
        if isinstance(matched_server, dict):
            candidate_version = matched_server.get("version")
            candidate_protocol = matched_server.get("protocol_version")
            if candidate_version is not None:
                server_version = candidate_version
                protocol_version = candidate_protocol
                version_source = "artifact"
    return RunningServerInfo(
        image_id=image_id,
        image_created=image_created,
        server_version=server_version,
        protocol_version=protocol_version,
        version_source=version_source,
        source_known=known,
        matched_artifact=matched.get("name") if matched else None,
    )


def print_running_server(running: RunningServerInfo) -> None:
    if running.get("version_source") == "not-running":
        print("Server is not running; the DB dump has no running image.")
        return
    print(f"Running server image: {running.get('image_id') or '?'}")
    matched_name = running.get("matched_artifact")
    if matched_name:
        matched = find_artifact("server", matched_name)
        source = matched.get("source") if matched else None
        if isinstance(source, dict) and source.get("known"):
            dirty = " (dirty)" if source.get("dirty") else ""
            print(
                f"  matches saved server: {matched_name} "
                f"({source.get('branch')} @ {source.get('short_commit')}){dirty}"
            )
        else:
            print(f"  matches saved server: {matched_name} (source unknown)")
    else:
        print("  source: unknown (not one of the saved server versions)")
    print(f"  {fmt_version(running)}")
