from __future__ import annotations

import argparse
import os
import sys
from collections.abc import Sequence
from typing import IO

from .commands import (
    cmd_delete,
    cmd_list,
    cmd_show,
    interactive_menu,
)
from .docker import Docker
from .errors import CliError
from .metadata import acquire_lock
from .reset import drop_db
from .restore.main import restore
from .snapshots.both import save_both
from .snapshots.db import save_db
from .snapshots.server import save_server

EPILOG = """\
Examples:
  uv run --project scripts/server_versions server-versions
  uv run --project scripts/server_versions server-versions save-db
  uv run --project scripts/server_versions server-versions save-server
  uv run --project scripts/server_versions server-versions save-both
  uv run --project scripts/server_versions server-versions list
  uv run --project scripts/server_versions server-versions restore --db <name> --server <name>
  uv run --project scripts/server_versions server-versions switch --db <name> --server current
  uv run --project scripts/server_versions server-versions drop-db

From scripts/server_versions, drop --project:
  uv run server-versions list

Dirty means uncommitted changes under server/ or shared/. Those directories are
copied into the server image, so a dirty build includes them and is marked
-dirty. A clean checkout can still run an image built from another branch; that
is recorded as the running image source and is not dirty.
"""


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Snapshot and restore Nooto server DB dumps and Docker images.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=EPILOG,
    )
    sub = parser.add_subparsers(dest="command")

    save_db_parser = sub.add_parser("save-db", help="Dump the current DB")
    save_db_parser.add_argument("--name")
    group = save_db_parser.add_mutually_exclusive_group()
    group.add_argument(
        "--with-running-image",
        dest="with_running_image",
        action="store_true",
        help="Also tag the exact running image",
    )
    group.add_argument(
        "--no-running-image",
        dest="with_running_image",
        action="store_false",
        help="Do not capture the running image",
    )
    save_db_parser.set_defaults(with_running_image=None)
    save_db_parser.add_argument("--yes", action="store_true")

    save_server_parser = sub.add_parser(
        "save-server", help="Build and tag the current branch image"
    )
    save_server_parser.add_argument("--name")
    save_server_parser.add_argument(
        "--running",
        action="store_true",
        help="Tag the running image without rebuilding",
    )
    save_server_parser.add_argument("--allow-dirty", action="store_true")
    save_server_parser.add_argument("--yes", action="store_true")

    save_both_parser = sub.add_parser("save-both", help="Rebuild, restart, then dump the DB")
    save_both_parser.add_argument("--name")
    save_both_parser.add_argument("--allow-dirty", action="store_true")
    save_both_parser.add_argument("--yes", action="store_true")

    restore_parser = sub.add_parser(
        "restore", aliases=["switch"], help="Restore a DB dump and/or image"
    )
    restore_parser.add_argument("--db", help="DB artifact name, or 'current'")
    restore_parser.add_argument("--server", help="Server artifact name, or 'current'")
    restore_parser.add_argument("--no-start", action="store_true")
    restore_parser.add_argument("--yes", action="store_true")
    restore_parser.add_argument("--skip-backup", action="store_true")
    restore_parser.add_argument("--no-rollback", action="store_true")

    list_parser = sub.add_parser("list", help="List artifacts")
    list_parser.add_argument("--all", action="store_true", help="Include automatic safety backups")
    list_parser.add_argument("--json", action="store_true")

    show_parser = sub.add_parser("show", help="Show artifact metadata")
    show_parser.add_argument("name")
    show_parser.add_argument("--json", action="store_true")

    delete_parser = sub.add_parser("delete", help="Delete an artifact")
    delete_parser.add_argument("name")
    delete_parser.add_argument("--kind", choices=("both", "db", "server"), default="both")
    delete_parser.add_argument("--yes", action="store_true")

    drop_db_parser = sub.add_parser("drop-db", help="Drop and recreate the current DB")
    drop_db_parser.add_argument("--yes", action="store_true")
    drop_db_parser.add_argument("--skip-backup", action="store_true")
    drop_db_parser.add_argument("--no-start", action="store_true")

    return parser


def _run_interactive() -> int:
    lock: IO[str] | None = None
    try:
        lock = acquire_lock()
    except (CliError, OSError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    try:
        interactive_menu()
        return 0
    except KeyboardInterrupt:
        print()
        return 130
    except BrokenPipeError:
        return 0
    except Exception as exc:  # noqa: BLE001 - top-level error boundary
        detail = str(exc) or exc.__class__.__name__
        print(f"Error: {detail}", file=sys.stderr)
        return 1
    finally:
        if lock is not None:
            lock.close()


def main(argv: Sequence[str] | None = None) -> int:
    if os.geteuid() == 0:
        print(
            "Run as your normal user; the script uses sudo for Docker only.",
            file=sys.stderr,
        )
        return 2
    os.umask(0o077)
    args_list = list(sys.argv[1:] if argv is None else argv)
    if not args_list:
        return _run_interactive()

    parser = build_parser()
    args = parser.parse_args(args_list)
    lock: IO[str] | None = None
    try:
        if args.command in (
            "save-db",
            "save-server",
            "save-both",
            "restore",
            "switch",
            "delete",
            "drop-db",
        ):
            lock = acquire_lock()
    except (CliError, OSError) as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    try:
        if args.command == "save-db":
            save_db(
                Docker(),
                name=args.name,
                with_running_image=args.with_running_image,
                assume_yes=args.yes,
            )
        elif args.command == "save-server":
            save_server(
                Docker(),
                name=args.name,
                running=args.running,
                allow_dirty=args.allow_dirty,
                assume_yes=args.yes,
            )
        elif args.command == "save-both":
            save_both(
                Docker(),
                name=args.name,
                allow_dirty=args.allow_dirty,
                assume_yes=args.yes,
            )
        elif args.command in ("restore", "switch"):
            restore(
                Docker(),
                db_name=args.db,
                server_name=args.server,
                no_start=args.no_start,
                assume_yes=args.yes,
                skip_backup=args.skip_backup,
                no_rollback=args.no_rollback,
            )
        elif args.command == "list":
            cmd_list(include_all=args.all, as_json=args.json)
        elif args.command == "show":
            cmd_show(args.name, as_json=args.json)
        elif args.command == "delete":
            cmd_delete(args.name, kind=args.kind, assume_yes=args.yes)
        elif args.command == "drop-db":
            drop_db(
                Docker(),
                assume_yes=args.yes,
                skip_backup=args.skip_backup,
                no_start=args.no_start,
            )
        else:
            parser.print_help()
            return 2
        return 0
    except BrokenPipeError:
        if args.command in ("list", "show"):
            return 0
        print(
            f"Error: output pipe closed; {args.command} may not have completed.",
            file=sys.stderr,
        )
        return 1
    except CliError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("\nInterrupted.", file=sys.stderr)
        return 130
    except Exception as exc:  # noqa: BLE001 - top-level error boundary
        detail = str(exc) or exc.__class__.__name__
        print(f"Error: {detail}", file=sys.stderr)
        return 1
    finally:
        if lock is not None:
            lock.close()
