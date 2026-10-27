from __future__ import annotations

import sys
from typing import TYPE_CHECKING

from .config import SYSTEM_DATABASES
from .errors import CliError
from .formatting import fmt_migrations, fmt_version
from .probe import VersionProbe
from .prompts import prompt_yes_no
from .snapshots.db import auto_backup_db

if TYPE_CHECKING:
    from .docker import Docker

__all__ = ["drop_db"]


def drop_db(
    docker: Docker,
    *,
    assume_yes: bool = False,
    skip_backup: bool = False,
    no_start: bool = False,
) -> None:
    docker.ensure_db_up()
    database = docker.db_name()
    if database.lower() in SYSTEM_DATABASES:
        raise CliError(f"Refusing to drop system database {database}.")
    migrations = docker.db_migrations()

    actions = ["drop and recreate the database"]
    if not no_start:
        actions.append("start server")
    backup_text = "skipped (--skip-backup)" if skip_backup else "will be taken"
    print()
    print("Drop DB plan")
    print("------------")
    print(f"  Database:      {database}")
    print(f"  Schema:        {fmt_migrations(migrations)}")
    print(f"  Actions:       {', '.join(actions)}")
    print(f"  Safety backup: {backup_text}")
    print()

    if not assume_yes and not prompt_yes_no(
        f"Drop database {database}? This cannot be undone.", default=False
    ):
        raise CliError("Aborted.")

    if not skip_backup:
        print("Backing up the current DB ...")
        auto = auto_backup_db(docker, "pre-drop")
        print(f"  safety backup: {auto['name']}")
    elif not no_start:
        print("Skipping the safety backup (--skip-backup).", file=sys.stderr)

    docker.stop_server()
    print(f"Dropping database {database} ...")
    docker.drop_database()

    if no_start:
        print("  database dropped")
        print("Server left stopped (--no-start).")
        return

    print("Starting server ...")
    docker.start_stopped_server()
    info = VersionProbe(docker).wait()
    print(f"  server up: {fmt_version(info)}")
    print(f"  rebuilt schema {fmt_migrations(docker.db_migrations())}")
    print("Drop complete.")
