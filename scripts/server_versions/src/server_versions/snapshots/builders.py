from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ..config import SCHEMA_VERSION
from ..formatting import now_iso
from ..models import DumpInfo, GitState, Metadata, RunningServerInfo
from ..project import (
    read_protocol_version,
    read_server_version,
    read_source_migrations,
)

if TYPE_CHECKING:
    from ..docker import Docker


def db_metadata(
    name: str,
    git: GitState,
    running: RunningServerInfo,
    dump: DumpInfo,
    *,
    auto: bool = False,
    reason: str | None = None,
) -> Metadata:
    meta: Metadata = {
        "schema_version": SCHEMA_VERSION,
        "kind": "db",
        "name": name,
        "created_at": now_iso(),
        "checkout": git,
        "db": {
            "database": dump["database"],
            "file": dump["file"],
            "sha256": dump["sha256"],
            "size_bytes": dump["size_bytes"],
            "applied_migrations": dump["applied_migrations"],
            "charset": dump["charset"],
            "collation": dump["collation"],
        },
        "running_server": running,
    }
    if auto:
        meta["auto"] = True
        meta["reason"] = reason
    return meta


def server_metadata_checkout(
    name: str,
    git: GitState,
    docker: Docker,
    tag: str,
    image_id: str,
) -> Metadata:
    return {
        "schema_version": SCHEMA_VERSION,
        "kind": "server",
        "name": name,
        "created_at": now_iso(),
        "source": {
            "kind": "checkout",
            "known": True,
            "branch": git["branch"],
            "commit": git["commit"],
            "short_commit": git["short_commit"],
            "subject": git["subject"],
            "dirty": git["dirty"],
            "repo_dirty": git["repo_dirty"],
        },
        "image": {
            "tag": tag,
            "id": image_id,
            "created": docker.image_created(image_id),
            "compose_image": docker.compose_image,
        },
        "server": {
            "version": read_server_version(),
            "protocol_version": read_protocol_version(),
            "migrations": read_source_migrations(),
        },
    }


def server_metadata_running(
    name: str,
    git: GitState,
    docker: Docker,
    tag: str,
    image_id: str,
    running: RunningServerInfo,
    matched: Metadata | None,
) -> Metadata:
    matched_source = matched.get("source") if matched else None
    known = bool(isinstance(matched_source, dict) and matched_source.get("known"))
    source: dict[str, Any] = {
        "kind": "running-image",
        "known": known,
        "matched_artifact": matched.get("name") if matched else None,
        "checkout": git,
    }
    if known and isinstance(matched_source, dict):
        for key in (
            "branch",
            "commit",
            "short_commit",
            "subject",
            "dirty",
            "repo_dirty",
        ):
            if key in matched_source:
                source[key] = matched_source[key]
    server: dict[str, Any] = {
        "version": running.get("server_version"),
        "protocol_version": running.get("protocol_version"),
        "migrations": None,
    }
    matched_server = matched.get("server") if matched else None
    if isinstance(matched_server, dict):
        server["version"] = server["version"] or matched_server.get("version")
        server["protocol_version"] = server["protocol_version"] or matched_server.get(
            "protocol_version"
        )
        server["migrations"] = matched_server.get("migrations")
    return {
        "schema_version": SCHEMA_VERSION,
        "kind": "server",
        "name": name,
        "created_at": now_iso(),
        "source": source,
        "image": {
            "tag": tag,
            "id": image_id,
            "created": running.get("image_created") or docker.image_created(image_id),
            "compose_image": docker.compose_image,
        },
        "server": server,
    }
