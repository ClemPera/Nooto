from __future__ import annotations

from pathlib import Path

import pytest

from server_versions import metadata
from server_versions.snapshots import both, cleanup, db


@pytest.fixture
def snapshot_dirs(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    root = tmp_path / ".server-snapshots"
    db_dir = root / "db"
    server_dir = root / "server"
    auto_dir = db_dir / "_auto"
    monkeypatch.setattr(metadata, "SNAPSHOT_ROOT", root)
    monkeypatch.setattr(metadata, "DB_DIR", db_dir)
    monkeypatch.setattr(metadata, "SERVER_DIR", server_dir)
    monkeypatch.setattr(metadata, "AUTO_DIR", auto_dir)
    monkeypatch.setattr(metadata, "LOCK_FILE", root / ".lock")
    monkeypatch.setattr(db, "DB_DIR", db_dir)
    monkeypatch.setattr(db, "AUTO_DIR", auto_dir)
    monkeypatch.setattr(db, "SNAPSHOT_ROOT", root, raising=False)
    monkeypatch.setattr(both, "DB_DIR", db_dir)
    monkeypatch.setattr(cleanup, "DB_DIR", db_dir)
    monkeypatch.setattr(cleanup, "AUTO_DIR", auto_dir)
    return root
