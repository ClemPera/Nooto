from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from ..formatting import fmt_migrations, fmt_time, human_size, short_id
from ..models import GitState, Metadata

__all__ = [
    "compatibility_warnings",
    "print_restore_summary",
    "source_mismatch_warnings",
]


def compatibility_warnings(
    db_migrations: list[int] | None,
    image_migrations: list[int] | None,
) -> list[str]:
    if db_migrations is None:
        return ["Current DB schema is unknown; compatibility was not checked."]
    if image_migrations is None:
        return ["Server image migration list is unknown; compatibility was not checked."]
    db_set = set(db_migrations)
    image_set = set(image_migrations)
    warnings: list[str] = []
    if db_set - image_set:
        warnings.append(
            "DB has migrations not in the server image: "
            f"{sorted(db_set - image_set)}. The older server may fail."
        )
    if image_set - db_set:
        warnings.append(
            "Server image has migrations not in the DB: "
            f"{sorted(image_set - db_set)}. They will be applied on start."
        )
    if not warnings:
        warnings.append("DB and server image migrations match.")
    return warnings


def source_mismatch_warnings(
    source: Mapping[str, Any] | None, git: GitState, label: str
) -> list[str]:
    if not source or not source.get("known"):
        return [f"{label} source is unknown; it may not match the current checkout."]
    warnings: list[str] = []
    if source.get("commit") != git["commit"]:
        warnings.append(
            f"{label} was built from {source.get('branch')} @ "
            f"{source.get('short_commit')}, current checkout is "
            f"{git['branch']} @ {git['short_commit']}."
        )
    if source.get("dirty"):
        warnings.append(
            f"{label} was built from a dirty tree; it may not match the current checkout."
        )
    elif git.get("dirty"):
        warnings.append(
            "current checkout has uncommitted server/shared changes; "
            f"{label} does not include them."
        )
    return warnings


def _nested(meta: Metadata | None, key: str) -> Mapping[str, Any]:
    if meta is None:
        return {}
    value = meta.get(key)
    return value if isinstance(value, dict) else {}


def print_restore_summary(
    db_meta: Metadata | None,
    server_meta: Metadata | None,
    current_migrations: list[int] | None,
    current_image_id: str | None,
    current_img_meta: Metadata | None,
    no_start: bool,
    git: GitState,
) -> None:
    print()
    print("Restore plan")
    print("------------")
    print(
        f"  Checkout:     {git['branch']} @ {git['short_commit']} "
        f"({'dirty server/shared' if git['dirty'] else 'clean server/shared'})"
    )
    if db_meta:
        db = _nested(db_meta, "db")
        print(
            f"  DB dump:      {db_meta['name']} "
            f"({fmt_migrations(db.get('applied_migrations'))}, "
            f"{human_size(db.get('size_bytes'))}, "
            f"saved {fmt_time(db_meta.get('created_at'))})"
        )
    else:
        print(f"  DB dump:      keep current ({fmt_migrations(current_migrations)})")
    if server_meta:
        server = _nested(server_meta, "server")
        print(
            f"  Server image: {server_meta['name']} "
            f"(version {server.get('version')}, protocol {server.get('protocol_version')}, "
            f"migrations {fmt_migrations(server.get('migrations'))})"
        )
    elif current_img_meta:
        server = _nested(current_img_meta, "server")
        print(
            f"  Server image: keep current {short_id(current_image_id)} "
            f"({current_img_meta['name']}, migrations "
            f"{fmt_migrations(server.get('migrations'))})"
        )
    else:
        print(f"  Server image: keep current {short_id(current_image_id)} (source unknown)")
    actions = []
    if db_meta:
        actions.append("restore DB")
    if server_meta:
        actions.append("switch image")
    if not no_start:
        actions.append("start server")
    print(f"  Actions:      {', '.join(actions)}")
    print()
