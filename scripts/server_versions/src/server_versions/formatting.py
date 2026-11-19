from __future__ import annotations

import datetime as dt
import re
from collections.abc import Mapping, Sequence
from typing import Any


def now_iso() -> str:
    return dt.datetime.now().astimezone().isoformat(timespec="seconds")


def timestamp_name() -> str:
    return dt.datetime.now().astimezone().strftime("%Y%m%d-%H%M%S")


def fmt_time(value: str | None) -> str:
    if not value:
        return "?"
    try:
        parsed = dt.datetime.fromisoformat(value)
    except ValueError:
        return value
    return parsed.strftime("%Y-%m-%d %H:%M")


def human_size(size: int | None) -> str:
    if size is None:
        return "?"
    value = float(size)
    for unit in ("B", "KiB", "MiB", "GiB"):
        if value < 1024 or unit == "GiB":
            return f"{value:.0f} {unit}" if unit == "B" else f"{value:.1f} {unit}"
        value /= 1024
    return f"{value:.1f} GiB"


def fmt_migrations(versions: list[int] | None) -> str:
    if versions is None:
        return "unknown"
    if not versions:
        return "none"
    return ",".join(f"V{v}" for v in versions)


def normalize_image_id(image_id: str | None) -> str | None:
    if not image_id:
        return None
    value = image_id.strip()
    if re.fullmatch(r"[0-9a-f]{64}", value):
        return f"sha256:{value}"
    return value


def short_id(image_id: str | None) -> str:
    normalized = normalize_image_id(image_id)
    if not normalized:
        return "?"
    return normalized.removeprefix("sha256:")[:12]


def truncate(text: str, width: int) -> str:
    return text if len(text) <= width else text[: width - 1] + "…"


def fmt_version(info: Mapping[str, Any]) -> str:
    source = info.get("version_source")
    if source == "http":
        return f"version {info.get('server_version')} protocol {info.get('protocol_version')}"
    if source == "artifact":
        return (
            f"version {info.get('server_version')} protocol "
            f"{info.get('protocol_version')} (from saved metadata)"
        )
    if source == "unavailable":
        return "up (/version unavailable, older server)"
    if source == "not-running":
        return "not running"
    return "up (version unknown)"


def print_table(headers: Sequence[str], rows: Sequence[Sequence[str]]) -> None:
    widths = [len(header) for header in headers]
    for row in rows:
        if len(row) != len(headers):
            raise RuntimeError(f"Table row has {len(row)} cells, expected {len(headers)}")
        for index, cell in enumerate(row):
            widths[index] = max(widths[index], len(cell))
    print("  " + "  ".join(header.ljust(widths[index]) for index, header in enumerate(headers)))
    for row in rows:
        print("  " + "  ".join(cell.ljust(widths[index]) for index, cell in enumerate(row)))
