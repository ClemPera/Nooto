from __future__ import annotations

import re
from collections.abc import Mapping
from typing import Any

from .errors import CliError
from .formatting import timestamp_name
from .models import GitState


def slugify(text: str, limit: int = 40) -> str:
    slug = re.sub(r"[^a-zA-Z0-9._-]+", "-", text).strip("-").lower()
    slug = slug[:limit].strip("-")
    return slug or "detached"


def default_name(git: GitState, *, extra: str | None = None, dirty: bool | None = None) -> str:
    if dirty is None:
        dirty = git["dirty"]
    parts = [slugify(git["branch"]), git["short_commit"]]
    if dirty:
        parts.append("dirty")
    if extra:
        parts.append(slugify(extra, 60))
    parts.append(timestamp_name())
    return "-".join(parts)[:100].strip("-")


def name_from_source(source: Mapping[str, Any], *, extra: str | None = None) -> str:
    parts = [
        slugify(str(source.get("branch") or "unknown")),
        str(source.get("short_commit") or "unknown"),
    ]
    if source.get("dirty"):
        parts.append("dirty")
    if extra:
        parts.append(slugify(extra, 60))
    parts.append(timestamp_name())
    return "-".join(parts)[:100].strip("-")


def validate_name(name: str) -> str:
    if not isinstance(name, str):
        raise CliError("Name must be a string.")
    name = name.strip().lower()
    if not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,99}", name):
        raise CliError(
            "Name must be 1-100 chars: lowercase letters, digits, dot, dash, "
            "underscore, and start with a letter or digit."
        )
    return name


def validate_db_filename(value: str) -> str:
    if (
        not value
        or value in {".", ".."}
        or not value.endswith(".sql.gz")
        or "/" in value
        or "\\" in value
        or value != value.rsplit("/", 1)[-1]
    ):
        raise CliError(f"Unsafe DB dump filename: {value!r}")
    return value
