from __future__ import annotations

from typing import cast

import pytest

from server_versions import metadata
from server_versions.docker import Docker
from server_versions.errors import CliError
from server_versions.models import GitState
from server_versions.naming import (
    default_name,
    name_from_source,
    slugify,
    validate_db_filename,
    validate_name,
)


class FakeDocker:
    def __init__(self, tags: set[str]) -> None:
        self.tags = tags

    def snapshot_tag(self, name: str) -> str:
        return f"nooto-server-snapshot:{name}"

    def image_exists(self, ref: str) -> bool:
        return ref in self.tags


def make_git(*, dirty: bool = False) -> GitState:
    return GitState(
        branch="Feature/X",
        commit="1234567890abcdef",
        short_commit="12345678",
        subject="Work",
        dirty=dirty,
        repo_dirty=False,
    )


def test_slugify() -> None:
    assert slugify("--Hello, World!--") == "hello-world"
    assert slugify("") == "detached"
    assert slugify("a" * 80, 10) == "a" * 10


def test_default_name(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("server_versions.naming.timestamp_name", lambda: "20260101-000000")
    assert default_name(make_git()) == "feature-x-12345678-20260101-000000"
    assert default_name(make_git(dirty=True)) == ("feature-x-12345678-dirty-20260101-000000")
    assert default_name(make_git(), extra="A B", dirty=False) == (
        "feature-x-12345678-a-b-20260101-000000"
    )


def test_name_from_source(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("server_versions.naming.timestamp_name", lambda: "20260101-000000")
    assert (
        name_from_source(
            {"branch": "Feature/X", "short_commit": "abc", "dirty": True},
            extra="copy",
        )
        == "feature-x-abc-dirty-copy-20260101-000000"
    )
    assert name_from_source({}) == "unknown-unknown-20260101-000000"


def test_validate_name() -> None:
    assert validate_name("  My.Dump_1  ") == "my.dump_1"
    with pytest.raises(CliError):
        validate_name("")
    with pytest.raises(CliError):
        validate_name("-bad")
    with pytest.raises(CliError):
        validate_name("has space")


def test_validate_db_filename() -> None:
    assert validate_db_filename("dump.sql.gz") == "dump.sql.gz"
    for value in ("", ".", "..", "../dump.sql.gz", "dump.sql", "dir/dump.sql.gz"):
        with pytest.raises(CliError):
            validate_db_filename(value)


@pytest.mark.usefixtures("snapshot_dirs")
def test_unique_name_checks_artifacts_and_tags() -> None:
    metadata.ensure_dirs()
    fake = cast(Docker, FakeDocker({"nooto-server-snapshot:base"}))
    assert metadata.unique_name("base", ["db"], docker=fake) == "base-2"
    (metadata.DB_DIR / "base-2.json").write_text("{}")
    assert metadata.unique_name("base", ["db"], docker=fake) == "base-3"
