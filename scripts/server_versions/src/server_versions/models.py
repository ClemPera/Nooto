from __future__ import annotations

from typing import Any, NotRequired, TypedDict


class GitState(TypedDict):
    branch: str
    commit: str
    short_commit: str
    subject: str
    dirty: bool
    repo_dirty: bool


class VersionInfo(TypedDict):
    version_source: str
    server_version: str | None
    protocol_version: int | None


class ProbeInfo(TypedDict, total=False):
    version_source: str
    server_version: str | None
    protocol_version: int | None


class RunningServerInfo(TypedDict):
    image_id: str | None
    image_created: str | None
    server_version: str | None
    protocol_version: int | None
    version_source: str
    source_known: bool
    matched_artifact: str | None
    captured_artifact: NotRequired[str]


class DbInfo(TypedDict):
    database: str
    applied_migrations: list[int]
    charset: str | None
    collation: str | None


class DumpInfo(DbInfo):
    file: str
    sha256: str
    size_bytes: int


Metadata = dict[str, Any]
