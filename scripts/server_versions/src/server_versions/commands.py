from __future__ import annotations

import json
import sys
from collections.abc import Mapping
from typing import Any

from .config import REPO_ROOT
from .docker import Docker
from .errors import CliError
from .formatting import (
    fmt_migrations,
    fmt_time,
    human_size,
    print_table,
    short_id,
    truncate,
)
from .git import git_state
from .metadata import (
    db_file_path,
    ensure_dirs,
    load_artifacts,
    read_metadata,
)
from .models import Metadata
from .naming import validate_name
from .prompts import prompt_optional, prompt_yes_no
from .reset import drop_db
from .restore.main import restore
from .snapshots.both import save_both
from .snapshots.cleanup import delete_db_artifact, delete_server_artifact
from .snapshots.db import save_db
from .snapshots.server import save_server

__all__ = [
    "artifact_summary",
    "choose_artifact",
    "choose_restore_pair",
    "cmd_delete",
    "cmd_list",
    "cmd_show",
    "interactive_menu",
]


def _section(meta: Metadata, key: str) -> Mapping[str, Any]:
    value = meta.get(key)
    return value if isinstance(value, dict) else {}


def _artifact_name(meta: Metadata) -> str:
    name = meta.get("name")
    return name if isinstance(name, str) else "?"


def cmd_list(*, include_all: bool = False, as_json: bool = False) -> None:
    db_items = load_artifacts("db", include_auto=include_all)
    server_items = load_artifacts("server")
    if as_json:
        print(json.dumps({"db": db_items, "server": server_items}, indent=2))
        return

    print("DB DUMPS")
    if not db_items:
        print("  (none)")
    else:
        rows: list[list[str]] = []
        for meta in db_items:
            db = _section(meta, "db")
            checkout = _section(meta, "checkout")
            running = _section(meta, "running_server")
            if running.get("captured_artifact"):
                running_source = f"captured:{running['captured_artifact']}"
            elif running.get("matched_artifact"):
                running_source = f"matched:{running['matched_artifact']}"
            elif running.get("image_id"):
                running_source = "unknown"
            else:
                running_source = "-"
            flags = []
            if meta.get("auto"):
                flags.append("AUTO")
            try:
                missing = not db_file_path(meta).exists()
            except CliError:
                missing = True
            if missing:
                flags.append("MISSING")
            rows.append(
                [
                    _artifact_name(meta),
                    truncate(str(checkout.get("branch") or "?"), 28),
                    str(checkout.get("short_commit") or "?"),
                    fmt_time(meta.get("created_at")),
                    human_size(db.get("size_bytes")),
                    fmt_migrations(db.get("applied_migrations")),
                    truncate(running_source, 36),
                    ",".join(flags),
                ]
            )
        print_table(
            [
                "NAME",
                "BRANCH",
                "COMMIT",
                "CREATED",
                "SIZE",
                "SCHEMA",
                "RUNNING",
                "FLAGS",
            ],
            rows,
        )

    print()
    print("SERVER IMAGES")
    if not server_items:
        print("  (none)")
    else:
        server_rows: list[list[str]] = []
        for meta in server_items:
            source_meta = _section(meta, "source")
            server = _section(meta, "server")
            flags = []
            if source_meta.get("dirty"):
                flags.append("DIRTY")
            if not source_meta.get("known"):
                flags.append("UNKNOWN")
            image = _section(meta, "image")
            server_rows.append(
                [
                    _artifact_name(meta),
                    truncate(str(source_meta.get("branch") or "unknown"), 28),
                    str(source_meta.get("short_commit") or "?"),
                    fmt_time(meta.get("created_at")),
                    short_id(image.get("id")),
                    str(server.get("version") or "?"),
                    str(server.get("protocol_version") or "?"),
                    fmt_migrations(server.get("migrations")),
                    ",".join(flags),
                ]
            )
        print_table(
            [
                "NAME",
                "BRANCH",
                "COMMIT",
                "CREATED",
                "IMAGE",
                "VERSION",
                "PROTOCOL",
                "MIGRATIONS",
                "FLAGS",
            ],
            server_rows,
        )


def cmd_show(name: str, *, as_json: bool = False) -> None:
    name = validate_name(name)
    items = [
        meta
        for meta in (read_metadata("db", name), read_metadata("server", name))
        if meta is not None
    ]
    if not items:
        raise CliError(f"No artifact named {name}.")
    if as_json:
        print(json.dumps(items[0] if len(items) == 1 else items, indent=2))
        return
    for meta in items:
        print(f"# {meta['kind']} {meta['name']}")
        print(json.dumps(meta, indent=2))
        print()


def cmd_delete(name: str, *, kind: str = "both", assume_yes: bool = False) -> None:
    name = validate_name(name)
    db_meta = read_metadata("db", name) if kind in ("both", "db") else None
    server_meta = read_metadata("server", name) if kind in ("both", "server") else None
    found: list[str] = []
    if db_meta is not None:
        found.append("db")
    if server_meta is not None:
        found.append("server")
    if not found:
        raise CliError(f"No artifact named {name}.")
    print(f"Delete {', '.join(found)} artifact(s) named {name}?")
    if not assume_yes and not prompt_yes_no("Confirm delete?", default=False):
        return
    if "server" in found:
        docker = Docker()
        removed_image = delete_server_artifact(docker, name, meta=server_meta)
        if removed_image:
            print("Deleted server image.")
        else:
            print("Deleted server metadata (image was already gone).")
    if "db" in found:
        delete_db_artifact(name, meta=db_meta)
        print("Deleted DB dump.")


def artifact_summary(meta: Metadata) -> str:
    if meta.get("kind") == "db":
        db = _section(meta, "db")
        return (
            f"{fmt_migrations(db.get('applied_migrations'))}, "
            f"{human_size(db.get('size_bytes'))}, "
            f"saved {fmt_time(meta.get('created_at'))}"
        )
    server = _section(meta, "server")
    source = _section(meta, "source")
    source_text = (
        f"{source.get('branch')} @ {source.get('short_commit')}"
        if source.get("known")
        else "unknown source"
    )
    return (
        f"version {server.get('version')}, protocol {server.get('protocol_version')}, "
        f"migrations {fmt_migrations(server.get('migrations'))}, {source_text}"
    )


def choose_artifact(
    _kind: str, items: list[Metadata], label: str, default: str | None
) -> str | None:
    if not items:
        print(f"No {label.lower()}s saved. Keeping current.")
        return None
    print(f"\n{label}s:")
    for index, meta in enumerate(items, 1):
        print(f"  {index}) {_artifact_name(meta)}")
        print(f"     {artifact_summary(meta)}")
    prompt = f"{label} [number/name/k=keep current]"
    if default:
        prompt += f" (Enter = {default})"
    while True:
        try:
            raw = input(prompt + ": ").strip()
        except EOFError as exc:
            raise CliError("Input closed.") from exc
        if not raw:
            if default:
                return default
            print("Enter a number, a name, or k.")
            continue
        if raw.lower() in ("k", "keep", "current"):
            return None
        if raw.isdigit():
            index = int(raw)
            if 1 <= index <= len(items):
                return _artifact_name(items[index - 1])
            print("Number out of range.")
            continue
        lowered = raw.lower()
        if any(_artifact_name(meta) == lowered for meta in items):
            return lowered
        print("Unknown name.")


def choose_restore_pair() -> tuple[str | None, str | None]:
    db_items = load_artifacts("db")
    server_items = load_artifacts("server")
    if not db_items and not server_items:
        raise CliError("No artifacts saved yet.")
    server_names = {_artifact_name(meta) for meta in server_items}
    default = next(
        (_artifact_name(meta) for meta in db_items if _artifact_name(meta) in server_names),
        None,
    )
    db_name = choose_artifact("db", db_items, "DB dump", default)
    server_name = choose_artifact("server", server_items, "Server image", default)
    if not db_name and not server_name:
        raise CliError("Nothing selected.")
    return db_name, server_name


def interactive_menu() -> None:
    ensure_dirs()
    while True:
        git = git_state()
        print()
        print("Nooto server version manager")
        print(f"  repo:     {REPO_ROOT}")
        print(
            f"  checkout: {git['branch']} @ {git['short_commit']} "
            f"({'dirty server/shared' if git['dirty'] else 'clean server/shared'})"
        )
        print("  dirty means uncommitted changes under server/ or shared/")
        print()
        print("  1) Save DB dump")
        print("  2) Save server image (rebuild current branch)")
        print("  3) Save running server image (no rebuild)")
        print("  4) Save both (rebuild, restart, dump)")
        print("  5) Restore / switch")
        print("  6) List artifacts")
        print("  7) Show artifact")
        print("  8) Delete artifact")
        print("  9) Drop current database (recreate)")
        print(" 10) Quit")
        try:
            choice = input("> ").strip()
        except EOFError:
            return
        try:
            if choice == "1":
                name = prompt_optional("Name [default]: ")
                save_db(Docker(), name=name or None)
            elif choice == "2":
                name = prompt_optional("Name [default]: ")
                save_server(Docker(), name=name or None)
            elif choice == "3":
                save_server(Docker(), running=True)
            elif choice == "4":
                name = prompt_optional("Name [default]: ")
                save_both(Docker(), name=name or None)
            elif choice == "5":
                db_name, server_name = choose_restore_pair()
                restore(Docker(), db_name=db_name, server_name=server_name)
            elif choice == "6":
                cmd_list()
            elif choice == "7":
                name = input("Artifact name: ").strip()
                if name:
                    cmd_show(name)
            elif choice == "8":
                name = input("Artifact name: ").strip()
                if name:
                    cmd_delete(name)
            elif choice == "9":
                drop_db(Docker())
            elif choice == "10":
                return
            else:
                print("Unknown choice.")
        except EOFError:
            return
        except CliError as exc:
            print(f"Error: {exc}", file=sys.stderr)
        except KeyboardInterrupt:
            print("\nInterrupted.")
            return
        if choice in ("1", "2", "3", "4", "5", "6", "7", "8", "9"):
            try:
                input("\nPress Enter to continue...")
            except EOFError:
                return
