from __future__ import annotations

from pathlib import Path

import pytest

from server_versions import project


def write_repo(tmp_path: Path) -> Path:
    (tmp_path / "server" / "src").mkdir(parents=True)
    (tmp_path / "server" / "Cargo.toml").write_text(
        '[package]\nname = "server"\nversion = "1.2.3"\n'
    )
    (tmp_path / "shared" / "src").mkdir(parents=True)
    (tmp_path / "shared" / "src" / "lib.rs").write_text("pub const PROTOCOL_VERSION: u32 = 7;\n")
    (tmp_path / "server" / "src" / "migrations.rs").write_text(
        "pub const MIGRATIONS: &[(u32, &str)] = &[\n"
        '    (1, include_str!("migrations/V1__init.sql")),\n'
        '    (2, include_str!("migrations/V2__more.sql")),\n'
        "];\n"
    )
    return tmp_path


def test_reads_versions(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    repo = write_repo(tmp_path)
    monkeypatch.setattr(project, "REPO_ROOT", repo)
    assert project.read_server_version() == "1.2.3"
    assert project.read_protocol_version() == 7
    assert project.read_source_migrations() == [1, 2]


def test_missing_files(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(project, "REPO_ROOT", tmp_path)
    assert project.read_server_version() is None
    assert project.read_protocol_version() is None
    assert project.read_source_migrations() == []


def test_migration_filename_fallback(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    migrations = tmp_path / "server" / "migrations"
    migrations.mkdir(parents=True)
    (migrations / "V3__third.sql").write_text("")
    (migrations / "V10__tenth.sql").write_text("")
    (migrations / "notes.txt").write_text("")
    monkeypatch.setattr(project, "REPO_ROOT", tmp_path)
    assert project.read_source_migrations() == [3, 10]


def test_invalid_cargo_toml(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    server = tmp_path / "server"
    server.mkdir()
    (server / "Cargo.toml").write_text("not toml [")
    monkeypatch.setattr(project, "REPO_ROOT", tmp_path)
    assert project.read_server_version() is None
