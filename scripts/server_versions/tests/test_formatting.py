from __future__ import annotations

from server_versions.formatting import (
    fmt_migrations,
    fmt_time,
    fmt_version,
    human_size,
    normalize_image_id,
    short_id,
    truncate,
)


def test_human_size() -> None:
    assert human_size(None) == "?"
    assert human_size(0) == "0 B"
    assert human_size(1023) == "1023 B"
    assert human_size(1024) == "1.0 KiB"
    assert human_size(1536) == "1.5 KiB"
    assert human_size(1024 * 1024) == "1.0 MiB"
    assert human_size(1024 * 1024 * 1024) == "1.0 GiB"


def test_fmt_migrations() -> None:
    assert fmt_migrations(None) == "unknown"
    assert fmt_migrations([]) == "none"
    assert fmt_migrations([1, 3]) == "V1,V3"


def test_normalize_image_id() -> None:
    raw = "a" * 64
    assert normalize_image_id(raw) == f"sha256:{raw}"
    assert normalize_image_id(f"sha256:{raw}") == f"sha256:{raw}"
    assert normalize_image_id("  abc  ") == "abc"
    assert normalize_image_id(None) is None
    assert normalize_image_id("") is None


def test_short_id() -> None:
    assert short_id("a" * 64) == "a" * 12
    assert short_id("sha256:" + "b" * 64) == "b" * 12
    assert short_id(None) == "?"


def test_truncate() -> None:
    assert truncate("short", 10) == "short"
    assert truncate("abcdefgh", 5) == "abcd…"
    assert truncate("abcde", 5) == "abcde"


def test_fmt_time() -> None:
    assert fmt_time(None) == "?"
    assert fmt_time("2026-01-02T03:04:05+00:00") == "2026-01-02 03:04"
    assert fmt_time("not a date") == "not a date"


def test_fmt_version() -> None:
    assert fmt_version({"version_source": "not-running"}) == "not running"
    assert (
        fmt_version(
            {
                "version_source": "http",
                "server_version": "0.4.0",
                "protocol_version": 3,
            }
        )
        == "version 0.4.0 protocol 3"
    )
    assert (
        fmt_version(
            {
                "version_source": "artifact",
                "server_version": "0.4.0",
                "protocol_version": 3,
            }
        )
        == "version 0.4.0 protocol 3 (from saved metadata)"
    )
    assert fmt_version({"version_source": "unavailable"}) == (
        "up (/version unavailable, older server)"
    )
    assert fmt_version({}) == "up (version unknown)"
