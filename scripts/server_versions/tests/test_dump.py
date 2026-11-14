from __future__ import annotations

import gzip
import hashlib
import stat
import subprocess
from pathlib import Path
from typing import IO, cast

import pytest

from server_versions import metadata
from server_versions.docker import Docker
from server_versions.errors import CliError
from server_versions.models import (
    DbInfo,
    GitState,
    ProbeInfo,
    RunningServerInfo,
)
from server_versions.snapshots import db as db_module
from server_versions.snapshots.cleanup import delete_db_artifact, delete_server_artifact
from server_versions.snapshots.db import auto_backup_db, dump_db_file, save_db

SQL = b"CREATE TABLE test (id INT);\n"
INFO = DbInfo(
    database="nooto",
    applied_migrations=[1, 2],
    charset="utf8mb4",
    collation="utf8mb4_general_ci",
)
SHA = "a" * 64
GIT = GitState(
    branch="main",
    commit="1234567890abcdef",
    short_commit="12345678",
    subject="Work",
    dirty=False,
    repo_dirty=False,
)
NOT_RUNNING = RunningServerInfo(
    image_id=None,
    image_created=None,
    server_version=None,
    protocol_version=None,
    version_source="not-running",
    source_known=False,
    matched_artifact=None,
)


class FakeDocker:
    def __init__(
        self,
        *,
        active: bool,
        fail_stop: bool = False,
        deactivate_on_stop_failure: bool = False,
    ) -> None:
        self.active = active
        self.fail_stop = fail_stop
        self.deactivate_on_stop_failure = deactivate_on_stop_failure
        self.stopped = False
        self.started = False

    def server_active(self) -> bool:
        return self.active

    def stop_server(self) -> None:
        if self.fail_stop:
            if self.deactivate_on_stop_failure:
                self.active = False
            raise CliError("stop failed")
        self.stopped = True

    def start_stopped_server(self) -> None:
        self.started = True

    def ensure_db_up(self) -> None:
        pass

    def dump_db_to(self, out_file: IO[bytes]) -> None:
        out_file.write(SQL)


class FakeProbe:
    def __init__(self, docker: object) -> None:
        self.docker = docker

    def wait(self, timeout: int = 60) -> ProbeInfo:
        return ProbeInfo(version_source="http", server_version="0.4.0", protocol_version=3)


@pytest.fixture
def fake_probe(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(db_module, "VersionProbe", FakeProbe)


@pytest.mark.usefixtures("fake_probe")
def test_dump_stops_and_restarts(tmp_path: Path) -> None:
    fake = FakeDocker(active=True)
    target = tmp_path / "db"
    result = dump_db_file(cast(Docker, fake), "dump", target, db_info=INFO)

    assert fake.stopped is True
    assert fake.started is True
    final = target / "dump.sql.gz"
    assert final.exists()
    assert stat.S_IMODE(final.stat().st_mode) == 0o600
    with gzip.open(final, "rb") as handle:
        assert handle.read() == SQL
    assert result["database"] == "nooto"
    assert result["file"] == "dump.sql.gz"
    assert result["sha256"] == hashlib.sha256(final.read_bytes()).hexdigest()
    assert result["size_bytes"] == final.stat().st_size


@pytest.mark.usefixtures("fake_probe")
def test_dump_stop_failure_does_not_restart(tmp_path: Path) -> None:
    fake = FakeDocker(active=True, fail_stop=True)
    target = tmp_path / "db"
    with pytest.raises(CliError, match="stop failed"):
        dump_db_file(cast(Docker, fake), "dump", target, db_info=INFO)

    assert fake.started is False
    assert not (target / "dump.sql.gz").exists()
    assert not (target / ".dump.sql.gz.tmp").exists()


@pytest.mark.usefixtures("fake_probe")
def test_dump_stop_failure_restarts_when_server_became_inactive(tmp_path: Path) -> None:
    fake = FakeDocker(active=True, fail_stop=True, deactivate_on_stop_failure=True)
    target = tmp_path / "db"
    with pytest.raises(CliError, match="stop failed"):
        dump_db_file(cast(Docker, fake), "dump", target, db_info=INFO)

    assert fake.started is True
    assert not (target / "dump.sql.gz").exists()


@pytest.mark.usefixtures("fake_probe")
def test_dump_inactive_server_is_not_started(tmp_path: Path) -> None:
    fake = FakeDocker(active=False)
    target = tmp_path / "db"
    dump_db_file(cast(Docker, fake), "dump", target, db_info=INFO)
    assert fake.stopped is False
    assert fake.started is False
    assert (target / "dump.sql.gz").exists()


def make_server_meta() -> dict[str, object]:
    return {
        "schema_version": 1,
        "kind": "server",
        "name": "image",
        "created_at": "2026-01-02T03:04:05+00:00",
        "source": {"known": True, "kind": "checkout"},
        "image": {
            "tag": "nooto-server-snapshot:image",
            "id": SHA,
            "compose_image": "nooto-server:latest",
        },
        "server": {"version": "0.4.0", "protocol_version": 3, "migrations": [1]},
    }


class FakeImageDocker:
    def __init__(
        self,
        *,
        stored_tag: str,
        image_id: str,
        tag_target: str | None,
        tags: list[str] | None = None,
        present: bool = True,
    ) -> None:
        self.stored_tag = stored_tag
        self.image_id_value = image_id
        self.tag_target = tag_target
        self.tags = tags
        self.present = present
        self.removed: list[str] = []

    def image_exists(self, ref: str) -> bool:
        if ref == self.stored_tag:
            return self.tag_target is not None
        if ref == self.image_id_value:
            return self.present
        return False

    def image_id(self, ref: str) -> str | None:
        return self.tag_target if ref == self.stored_tag else None

    def image_tags(self, ref: str) -> list[str] | None:
        return None if self.tags is None else list(self.tags)

    def rmi(self, ref: str) -> subprocess.CompletedProcess[bytes]:
        self.removed.append(ref)
        return subprocess.CompletedProcess(args=[], returncode=0, stdout=b"", stderr=b"")


@pytest.mark.usefixtures("snapshot_dirs")
def test_delete_server_artifact_removes_matching_tag() -> None:
    metadata.write_metadata("server", make_server_meta())
    image_id = f"sha256:{SHA}"
    fake = FakeImageDocker(
        stored_tag="nooto-server-snapshot:image",
        image_id=image_id,
        tag_target=image_id,
    )
    delete_server_artifact(cast(Docker, fake), "image")
    assert fake.removed == ["nooto-server-snapshot:image"]
    assert metadata.read_metadata("server", "image") is None


@pytest.mark.usefixtures("snapshot_dirs")
def test_delete_server_artifact_returns_true_when_tag_removed() -> None:
    metadata.write_metadata("server", make_server_meta())
    image_id = f"sha256:{SHA}"
    fake = FakeImageDocker(
        stored_tag="nooto-server-snapshot:image",
        image_id=image_id,
        tag_target=image_id,
    )
    assert delete_server_artifact(cast(Docker, fake), "image") is True


@pytest.mark.usefixtures("snapshot_dirs")
def test_delete_server_artifact_refuses_other_tags() -> None:
    metadata.write_metadata("server", make_server_meta())
    fake = FakeImageDocker(
        stored_tag="nooto-server-snapshot:image",
        image_id=f"sha256:{SHA}",
        tag_target=f"sha256:{'b' * 64}",
        tags=["nooto-server:latest"],
    )
    with pytest.raises(CliError, match="still tagged"):
        delete_server_artifact(cast(Docker, fake), "image")
    assert fake.removed == []
    assert metadata.read_metadata("server", "image") is not None


@pytest.mark.usefixtures("snapshot_dirs")
def test_delete_server_artifact_refuses_when_tags_unknown() -> None:
    metadata.write_metadata("server", make_server_meta())
    fake = FakeImageDocker(
        stored_tag="nooto-server-snapshot:image",
        image_id=f"sha256:{SHA}",
        tag_target=f"sha256:{'b' * 64}",
        tags=None,
    )
    with pytest.raises(CliError, match="could not inspect"):
        delete_server_artifact(cast(Docker, fake), "image")
    assert fake.removed == []
    assert metadata.read_metadata("server", "image") is not None


@pytest.mark.usefixtures("snapshot_dirs")
def test_delete_server_artifact_image_gone() -> None:
    metadata.write_metadata("server", make_server_meta())
    fake = FakeImageDocker(
        stored_tag="nooto-server-snapshot:image",
        image_id=f"sha256:{SHA}",
        tag_target=None,
        present=False,
    )
    delete_server_artifact(cast(Docker, fake), "image")
    assert fake.removed == []
    assert metadata.read_metadata("server", "image") is None


@pytest.mark.usefixtures("snapshot_dirs")
def test_delete_server_artifact_returns_false_when_image_gone() -> None:
    metadata.write_metadata("server", make_server_meta())
    fake = FakeImageDocker(
        stored_tag="nooto-server-snapshot:image",
        image_id=f"sha256:{SHA}",
        tag_target=None,
        present=False,
    )
    assert delete_server_artifact(cast(Docker, fake), "image") is False


@pytest.mark.usefixtures("snapshot_dirs")
def test_delete_db_artifact_removes_dump_and_metadata() -> None:
    meta = {
        "schema_version": 1,
        "kind": "db",
        "name": "dump",
        "created_at": "2026-01-02T03:04:05+00:00",
        "checkout": {},
        "db": {
            "database": "nooto",
            "file": "dump.sql.gz",
            "sha256": SHA,
            "size_bytes": 4,
            "applied_migrations": [],
            "charset": None,
            "collation": None,
        },
        "running_server": {
            "version_source": "not-running",
            "source_known": False,
        },
    }
    metadata.write_metadata("db", meta)
    dump = metadata.DB_DIR / "dump.sql.gz"
    dump.write_bytes(b"data")
    delete_db_artifact("dump")
    assert not dump.exists()
    assert metadata.read_metadata("db", "dump") is None


@pytest.mark.usefixtures("snapshot_dirs", "fake_probe")
def test_save_db_metadata_failure_removes_dump(monkeypatch: pytest.MonkeyPatch) -> None:
    fake = FakeDocker(active=False)
    monkeypatch.setattr(db_module, "git_state", lambda: GIT)
    monkeypatch.setattr(db_module, "running_server_info", lambda _docker: NOT_RUNNING)
    monkeypatch.setattr(db_module, "collect_db_info", lambda _docker: INFO)

    def fail_write(_kind: str, _meta: object) -> None:
        raise CliError("metadata write failed")

    monkeypatch.setattr(db_module, "write_metadata", fail_write)

    with pytest.raises(CliError, match="metadata write failed"):
        save_db(cast(Docker, fake), name="dump")

    assert not (metadata.DB_DIR / "dump.sql.gz").exists()
    assert not (metadata.DB_DIR / "dump.json").exists()


@pytest.mark.usefixtures("snapshot_dirs", "fake_probe")
def test_auto_backup_metadata_failure_removes_dump(monkeypatch: pytest.MonkeyPatch) -> None:
    fake = FakeDocker(active=False)
    monkeypatch.setattr(db_module, "git_state", lambda: GIT)
    monkeypatch.setattr(db_module, "running_server_info", lambda _docker: NOT_RUNNING)
    monkeypatch.setattr(db_module, "collect_db_info", lambda _docker: INFO)

    def fail_write(_kind: str, _meta: object) -> None:
        raise CliError("metadata write failed")

    monkeypatch.setattr(db_module, "write_metadata", fail_write)

    with pytest.raises(CliError, match="metadata write failed"):
        auto_backup_db(cast(Docker, fake), "pre-restore")

    assert list(metadata.AUTO_DIR.glob("*.sql.gz")) == []
    assert list(metadata.AUTO_DIR.glob("*.json")) == []
