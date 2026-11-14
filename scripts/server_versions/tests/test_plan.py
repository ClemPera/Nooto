from __future__ import annotations

from server_versions.models import GitState
from server_versions.restore.plan import (
    compatibility_warnings,
    source_mismatch_warnings,
)


def make_git(*, dirty: bool = False) -> GitState:
    return GitState(
        branch="main",
        commit="abcdef",
        short_commit="abcdef",
        subject="Work",
        dirty=dirty,
        repo_dirty=False,
    )


def test_compatibility_warnings_match() -> None:
    assert compatibility_warnings([1, 2], [1, 2]) == ["DB and server image migrations match."]


def test_compatibility_warnings_db_ahead() -> None:
    warnings = compatibility_warnings([1, 2, 3], [1, 2])
    assert warnings == [
        "DB has migrations not in the server image: [3]. The older server may fail."
    ]


def test_compatibility_warnings_image_ahead() -> None:
    warnings = compatibility_warnings([1], [1, 4])
    assert warnings == [
        "Server image has migrations not in the DB: [4]. They will be applied on start."
    ]


def test_compatibility_warnings_unknown() -> None:
    assert compatibility_warnings(None, [1]) == [
        "Current DB schema is unknown; compatibility was not checked."
    ]
    assert compatibility_warnings([1], None) == [
        "Server image migration list is unknown; compatibility was not checked."
    ]


def test_source_mismatch_unknown() -> None:
    assert source_mismatch_warnings(None, make_git(), "selected server image") == [
        "selected server image source is unknown; it may not match the current checkout."
    ]


def test_source_mismatch_commit() -> None:
    source = {
        "known": True,
        "branch": "old",
        "short_commit": "12345678",
        "commit": "12345678",
        "dirty": False,
    }
    warnings = source_mismatch_warnings(source, make_git(), "selected server image")
    assert warnings == [
        "selected server image was built from old @ 12345678, current checkout is main @ abcdef."
    ]


def test_source_mismatch_dirty_source() -> None:
    source = {
        "known": True,
        "branch": "main",
        "short_commit": "abcdef",
        "commit": "abcdef",
        "dirty": True,
    }
    assert source_mismatch_warnings(source, make_git(), "image") == [
        "image was built from a dirty tree; it may not match the current checkout."
    ]


def test_source_mismatch_current_dirty() -> None:
    source = {
        "known": True,
        "branch": "main",
        "short_commit": "abcdef",
        "commit": "abcdef",
        "dirty": False,
    }
    assert source_mismatch_warnings(source, make_git(dirty=True), "image") == [
        "current checkout has uncommitted server/shared changes; image does not include them."
    ]
