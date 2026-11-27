from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from server_versions import metadata
from server_versions.errors import CliError

SHA = "a" * 64


def make_git() -> dict[str, Any]:
    return {
        "branch": "main",
        "commit": "1234567890abcdef",
        "short_commit": "12345678",
        "subject": "Work",
        "dirty": False,
        "repo_dirty": False,
    }


def make_db_meta(name: str = "dump") -> dict[str, Any]:
    return {
        "schema_version": 1,
        "kind": "db",
        "name": name,
        "created_at": "2026-01-02T03:04:05+00:00",
        "checkout": make_git(),
        "db": {
            "database": "nooto",
            "file": f"{name}.sql.gz",
            "sha256": SHA,
            "size_bytes": 1024,
            "applied_migrations": [1, 2],
            "charset": "utf8mb4",
            "collation": "utf8mb4_general_ci",
        },
        "running_server": {
            "image_id": None,
            "image_created": None,
            "server_version": None,
            "protocol_version": None,
            "version_source": "not-running",
            "source_known": False,
            "matched_artifact": None,
        },
    }


def make_server_meta(name: str = "image") -> dict[str, Any]:
    return {
        "schema_version": 1,
        "kind": "server",
        "name": name,
        "created_at": "2026-01-02T03:04:05+00:00",
        "source": {
            "kind": "checkout",
            "known": True,
            "branch": "main",
            "commit": "1234567890abcdef",
            "short_commit": "12345678",
            "subject": "Work",
            "dirty": False,
            "repo_dirty": False,
        },
        "image": {
            "tag": f"nooto-server-snapshot:{name}",
            "id": SHA,
            "created": "2026-01-01T00:00:00Z",
            "compose_image": "nooto-server:latest",
        },
        "server": {
            "version": "0.4.0",
            "protocol_version": 3,
            "migrations": [1, 2],
        },
    }


def test_validate_db_metadata() -> None:
    meta = make_db_meta()
    assert metadata.validate_metadata("db", meta) is meta


def test_validate_server_metadata() -> None:
    meta = make_server_meta()
    assert metadata.validate_metadata("server", meta) is meta


def test_rejects_bool_schema_version() -> None:
    meta = make_db_meta()
    meta["schema_version"] = True
    with pytest.raises(CliError, match="schema_version"):
        metadata.validate_metadata("db", meta)


def test_rejects_wrong_kind() -> None:
    meta = make_db_meta()
    meta["kind"] = "server"
    with pytest.raises(CliError, match="kind"):
        metadata.validate_metadata("db", meta)


def test_rejects_bad_name() -> None:
    meta = make_db_meta()
    meta["name"] = "../escape"
    with pytest.raises(CliError, match="Name must be"):
        metadata.validate_metadata("db", meta)


def test_rejects_unsafe_dump_filename() -> None:
    meta = make_db_meta()
    meta["db"]["file"] = "../dump.sql.gz"
    with pytest.raises(CliError, match="Unsafe DB dump filename"):
        metadata.validate_metadata("db", meta)


def test_rejects_dump_filename_not_matching_name() -> None:
    meta = make_db_meta()
    meta["db"]["file"] = "other.sql.gz"
    with pytest.raises(CliError, match="db.file is not dump.sql.gz"):
        metadata.validate_metadata("db", meta)


def test_rejects_bad_checksum() -> None:
    meta = make_db_meta()
    meta["db"]["sha256"] = "abc"
    with pytest.raises(CliError, match="SHA-256"):
        metadata.validate_metadata("db", meta)


def test_rejects_boolean_size() -> None:
    meta = make_db_meta()
    meta["db"]["size_bytes"] = True
    with pytest.raises(CliError, match="integer"):
        metadata.validate_metadata("db", meta)


def test_rejects_negative_size() -> None:
    meta = make_db_meta()
    meta["db"]["size_bytes"] = -1
    with pytest.raises(CliError, match="negative"):
        metadata.validate_metadata("db", meta)


def test_rejects_boolean_migration() -> None:
    meta = make_db_meta()
    meta["db"]["applied_migrations"] = [1, True]
    with pytest.raises(CliError, match="list of integers"):
        metadata.validate_metadata("db", meta)


def test_rejects_empty_image_id() -> None:
    meta = make_server_meta()
    meta["image"]["id"] = ""
    with pytest.raises(CliError, match="image.id"):
        metadata.validate_metadata("server", meta)


def test_rejects_non_object() -> None:
    with pytest.raises(CliError, match="not an object"):
        metadata.validate_metadata("db", [])


@pytest.mark.usefixtures("snapshot_dirs")
def test_roundtrip() -> None:
    db_meta = make_db_meta()
    server_meta = make_server_meta()
    metadata.write_metadata("db", db_meta)
    metadata.write_metadata("server", server_meta)
    (metadata.DB_DIR / "dump.sql.gz").write_bytes(b"data")

    assert metadata.read_metadata("db", "dump") == db_meta
    assert metadata.read_metadata("server", "image") == server_meta
    assert metadata.find_artifact("db", "dump") == db_meta
    assert metadata.find_server_by_image_id(SHA) == server_meta
    assert metadata.artifact_exists("db", "dump")
    assert metadata.artifact_exists("server", "image")
    assert metadata.db_file_path(db_meta) == metadata.DB_DIR / "dump.sql.gz"
    assert metadata.sha256_file(metadata.DB_DIR / "dump.sql.gz") == (
        "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7"
    )

    assert [item["name"] for item in metadata.load_artifacts("db")] == ["dump"]
    assert [item["name"] for item in metadata.load_artifacts("server")] == ["image"]

    auto_meta = make_db_meta("auto-dump")
    auto_meta["auto"] = True
    auto_meta["reason"] = "pre-restore"
    metadata.write_metadata("db", auto_meta)
    assert metadata.read_metadata("db", "auto-dump", auto=True) == auto_meta
    names = {item["name"] for item in metadata.load_artifacts("db", include_auto=True)}
    assert names == {"dump", "auto-dump"}


@pytest.mark.usefixtures("snapshot_dirs")
def test_ensure_name_available_reports_each_kind_once() -> None:
    metadata.ensure_dirs()
    (metadata.DB_DIR / "dump.json").write_text("{}")
    (metadata.DB_DIR / "dump.sql.gz").write_bytes(b"x")
    with pytest.raises(CliError) as error:
        metadata.ensure_name_available("dump", ["db"])
    assert "(db)." in str(error.value)
    assert "db file" not in str(error.value)


@pytest.mark.usefixtures("snapshot_dirs")
def test_read_metadata_rejects_name_mismatch() -> None:
    metadata.ensure_dirs()
    (metadata.DB_DIR / "dump.json").write_text(json.dumps(make_db_meta("other")))
    with pytest.raises(CliError, match="not 'dump'"):
        metadata.read_metadata("db", "dump")


@pytest.mark.usefixtures("snapshot_dirs")
def test_load_artifacts_skips_name_mismatch() -> None:
    metadata.ensure_dirs()
    (metadata.DB_DIR / "dump.json").write_text(json.dumps(make_db_meta("other")))
    assert metadata.load_artifacts("db") == []


@pytest.mark.usefixtures("snapshot_dirs")
def test_write_metadata_removes_tmp_when_replace_fails(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail_replace(_self: Path, _target: Any) -> None:
        raise OSError("replace failed")

    monkeypatch.setattr(Path, "replace", fail_replace)
    with pytest.raises(OSError, match="replace failed"):
        metadata.write_metadata("db", make_db_meta())
    assert not (metadata.DB_DIR / "dump.json.tmp").exists()
    assert not (metadata.DB_DIR / "dump.json").exists()


@pytest.mark.usefixtures("snapshot_dirs")
def test_write_metadata_removes_tmp_when_write_fails(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    original = Path.write_text

    def fail_write(self: Path, data: str, *args: Any, **kwargs: Any) -> int:
        original(self, data[:10], *args, **kwargs)
        raise OSError("write failed")

    monkeypatch.setattr(Path, "write_text", fail_write)
    with pytest.raises(OSError, match="write failed"):
        metadata.write_metadata("db", make_db_meta())
    assert not (metadata.DB_DIR / "dump.json.tmp").exists()
