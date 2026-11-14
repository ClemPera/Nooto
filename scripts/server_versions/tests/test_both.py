from __future__ import annotations

from typing import Any, cast

import pytest

from server_versions.docker import Docker
from server_versions.errors import CliError
from server_versions.models import DumpInfo, GitState, Metadata, ProbeInfo, RunningServerInfo
from server_versions.snapshots import both as both_module
from server_versions.snapshots.both import save_both

GIT = GitState(
    branch="main",
    commit="1234567890abcdef",
    short_commit="12345678",
    subject="Work",
    dirty=False,
    repo_dirty=False,
)
OLD_ID = f"sha256:{'a' * 64}"
NEW_ID = f"sha256:{'b' * 64}"
SERVER_TAG = "nooto-server-snapshot:pair"
RUNNING = RunningServerInfo(
    image_id=NEW_ID,
    image_created=None,
    server_version="0.4.0",
    protocol_version=3,
    version_source="http",
    source_known=True,
    matched_artifact="pair",
)
DUMP = DumpInfo(
    database="nooto",
    applied_migrations=[1],
    charset="utf8mb4",
    collation="utf8mb4_general_ci",
    file="pair.sql.gz",
    sha256="c" * 64,
    size_bytes=128,
)
SERVER_META: Metadata = {"name": "pair", "kind": "server"}
DB_META: Metadata = {"name": "pair", "kind": "db"}


class FakeBothDocker:
    compose_image = "nooto-server:latest"

    def __init__(self, *, was_running: bool) -> None:
        self.was_running = was_running
        self.events: list[str] = []

    def snapshot_tag(self, name: str) -> str:
        return f"nooto-server-snapshot:{name}"

    def image_exists(self, ref: str) -> bool:
        return False

    def ensure_db_up(self) -> None:
        self.events.append("ensure_db_up")

    def server_image_id(self) -> str:
        return OLD_ID

    def server_running(self) -> bool:
        return self.was_running

    def build_server(self) -> None:
        self.events.append("build")

    def image_id(self, ref: str) -> str:
        return NEW_ID

    def image_created(self, ref: str) -> str | None:
        return None

    def tag(self, source: str, target: str) -> None:
        self.events.append("tag")

    def start_server(self) -> None:
        self.events.append("start")


class FakeProbe:
    def __init__(self, docker: object) -> None:
        self.docker = docker

    def wait(self, timeout: int = 60) -> ProbeInfo:
        return ProbeInfo(version_source="http", server_version="0.4.0", protocol_version=3)


def patch_save_both(
    monkeypatch: pytest.MonkeyPatch,
    docker: FakeBothDocker,
    *,
    backup_error: str | None = None,
) -> list[dict[str, Any]]:
    rollbacks: list[dict[str, Any]] = []

    def fake_write(kind: str, _meta: Metadata) -> None:
        docker.events.append(f"write:{kind}")

    def fake_backup(_fake_docker: object, _reason: str) -> Metadata:
        docker.events.append("backup")
        if backup_error is not None:
            raise CliError(backup_error)
        return {"name": "pre-save-both-test", "auto": True}

    def fake_dump(*_args: object, **_kwargs: object) -> DumpInfo:
        docker.events.append("dump")
        return DUMP

    def fake_rollback(
        _fake_docker: object,
        auto: Metadata | None,
        db_touched: bool,
        image_switched: bool,
        previous_image_id: str | None,
        no_start: bool,
    ) -> None:
        rollbacks.append(
            {
                "auto": auto,
                "db_touched": db_touched,
                "image_switched": image_switched,
                "previous_image_id": previous_image_id,
                "no_start": no_start,
            }
        )

    monkeypatch.setattr(both_module, "ensure_dirs", lambda: None)
    monkeypatch.setattr(both_module, "git_state", lambda: GIT)
    monkeypatch.setattr(both_module, "write_metadata", fake_write)
    monkeypatch.setattr(both_module, "auto_backup_db", fake_backup)
    monkeypatch.setattr(both_module, "dump_db_file", fake_dump)
    monkeypatch.setattr(both_module, "collect_db_info", lambda _fake_docker: DUMP)
    monkeypatch.setattr(both_module, "VersionProbe", FakeProbe)
    monkeypatch.setattr(both_module, "running_server_info", lambda _fake_docker: RUNNING)
    monkeypatch.setattr(both_module, "server_metadata_checkout", lambda *_args: SERVER_META)
    monkeypatch.setattr(both_module, "db_metadata", lambda *_args, **_kwargs: DB_META)
    monkeypatch.setattr(both_module, "rollback_restore", fake_rollback)
    monkeypatch.setattr(
        both_module,
        "remove_snapshot_artifact",
        lambda _fake_docker, _name: docker.events.append("cleanup-server"),
    )
    monkeypatch.setattr(
        both_module,
        "remove_db_dump",
        lambda _name: docker.events.append("cleanup-db"),
    )
    return rollbacks


@pytest.mark.usefixtures("snapshot_dirs")
def test_save_both_backs_up_after_build_before_start(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    docker = FakeBothDocker(was_running=True)
    rollbacks = patch_save_both(monkeypatch, docker)

    db_meta, server_meta = save_both(cast(Docker, docker), name="pair", assume_yes=True)

    assert db_meta is DB_META
    assert server_meta is SERVER_META
    assert docker.events == [
        "ensure_db_up",
        "build",
        "tag",
        "write:server",
        "backup",
        "start",
        "dump",
        "write:db",
    ]
    assert rollbacks == []


@pytest.mark.parametrize("server_was_running", [True, False])
@pytest.mark.usefixtures("snapshot_dirs")
def test_save_both_failure_before_start_passes_no_start(
    monkeypatch: pytest.MonkeyPatch, server_was_running: bool
) -> None:
    docker = FakeBothDocker(was_running=server_was_running)
    rollbacks = patch_save_both(monkeypatch, docker, backup_error="backup failed")

    with pytest.raises(CliError, match="backup failed"):
        save_both(cast(Docker, docker), name="pair", assume_yes=True)

    assert "start" not in docker.events
    assert len(rollbacks) == 1
    rollback = rollbacks[0]
    assert rollback["auto"] is None
    assert rollback["db_touched"] is False
    assert rollback["image_switched"] is True
    assert rollback["previous_image_id"] == OLD_ID
    assert rollback["no_start"] is (not server_was_running)
    assert "cleanup-server" in docker.events
    assert "cleanup-db" not in docker.events
