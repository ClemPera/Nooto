from __future__ import annotations

import sys
import time
from typing import TYPE_CHECKING

from ..config import DB_DIR
from ..errors import CliError
from ..formatting import fmt_migrations, fmt_version, human_size, normalize_image_id, short_id
from ..git import git_state
from ..metadata import (
    artifact_exists,
    ensure_dirs,
    ensure_name_available,
    unique_name,
    write_metadata,
)
from ..models import DumpInfo, Metadata, RunningServerInfo
from ..naming import default_name, validate_name
from ..probe import VersionProbe, running_server_info
from ..prompts import prompt_yes_no
from ..restore.rollback import rollback_restore
from .builders import db_metadata, server_metadata_checkout
from .cleanup import remove_db_dump, remove_snapshot_artifact
from .db import auto_backup_db, collect_db_info, dump_db_file
from .warnings import print_dirty_warning

if TYPE_CHECKING:
    from ..docker import Docker

__all__ = ["save_both"]


def _cleanup_partial(
    docker: Docker,
    name: str,
    *,
    server_created: bool,
    dump_created: bool,
) -> None:
    if server_created:
        remove_snapshot_artifact(docker, name)
    if dump_created:
        remove_db_dump(name)


def save_both(
    docker: Docker,
    *,
    name: str | None = None,
    allow_dirty: bool = False,
    assume_yes: bool = False,
) -> tuple[Metadata, Metadata]:
    ensure_dirs()
    git = git_state()
    if git["dirty"] and not allow_dirty:
        print_dirty_warning(git)
        if assume_yes or not sys.stdin.isatty():
            raise CliError(
                "Dirty server/shared tree. Commit the changes or re-run with --allow-dirty."
            )
        if not prompt_yes_no(
            "Build includes uncommitted changes and both artifacts will be marked "
            "-dirty. Continue?",
            default=False,
        ):
            raise CliError("Aborted.")

    if name:
        name = validate_name(name)
        ensure_name_available(name, ["db", "server"])
    else:
        name = unique_name(default_name(git), ["db", "server"], docker=docker)
    tag = docker.snapshot_tag(name)
    if docker.image_exists(tag) and not artifact_exists("server", name):
        raise CliError(f"Image tag {tag} already exists without metadata. Choose another name.")

    docker.ensure_db_up()
    previous_image_id = normalize_image_id(
        docker.server_image_id() or docker.image_id(docker.compose_image)
    )
    server_was_running = docker.server_running()
    image_switched = False
    db_touched = False
    server_created = False
    dump_created = False
    safety: Metadata | None = None
    running: RunningServerInfo | None = None
    server_meta: Metadata | None = None
    dump: DumpInfo | None = None
    db_meta: Metadata | None = None
    dirty_note = " (dirty)" if git["dirty"] else ""
    try:
        print(
            f"Building {docker.compose_image} from "
            f"{git['branch']} @ {git['short_commit']}{dirty_note} ..."
        )
        started = time.monotonic()
        docker.build_server()
        image_switched = True
        elapsed = time.monotonic() - started
        image_id = docker.image_id(docker.compose_image)
        if not image_id:
            raise CliError("Build finished but the compose image could not be found.")
        server_created = True
        docker.tag(docker.compose_image, tag)
        server_meta = server_metadata_checkout(name, git, docker, tag, image_id)
        write_metadata("server", server_meta)
        print(f"  built in {elapsed:.1f}s -> {image_id}")

        print("Backing up the current DB before restart ...")
        safety = auto_backup_db(docker, "pre-save-both")
        print(f"  safety backup: {safety['name']}")

        print("Recreating server with the new image ...")
        db_touched = True
        docker.start_server()
        info = VersionProbe(docker).wait()
        print(f"  server up: {fmt_version(info)}")

        running = running_server_info(docker)
        if normalize_image_id(running.get("image_id")) != normalize_image_id(image_id):
            raise CliError(
                f"Server is running {short_id(running.get('image_id'))} instead of "
                f"the new image {short_id(image_id)}."
            )
        print("Dumping DB ...")
        db_info = collect_db_info(docker)
        dump_created = True
        dump = dump_db_file(docker, name, DB_DIR, stop_server=True, db_info=db_info)
        db_meta = db_metadata(name, git, running, dump)
        write_metadata("db", db_meta)
    except BaseException as exc:
        print(f"Save-both failed: {exc}", file=sys.stderr)
        if image_switched or db_touched:
            try:
                rollback_restore(
                    docker,
                    safety,
                    db_touched,
                    image_switched,
                    previous_image_id,
                    not server_was_running,
                )
            except BaseException as rollback_exc:  # noqa: BLE001 - preserve original
                print(f"Rollback itself failed: {rollback_exc}", file=sys.stderr)
        else:
            print(
                "No rollback needed; the running server was not changed.",
                file=sys.stderr,
            )
        _cleanup_partial(
            docker,
            name,
            server_created=server_created,
            dump_created=dump_created,
        )
        raise

    assert running is not None
    assert server_meta is not None
    assert dump is not None
    assert db_meta is not None
    print(f"Saved pair {name}")
    print(f"  server: {tag}")
    print(f"  db:     {DB_DIR / dump['file']} ({human_size(dump['size_bytes'])})")
    print(f"  schema: {fmt_migrations(dump['applied_migrations'])}")
    print(f"  running: {fmt_version(running)}")
    if running.get("matched_artifact"):
        print(f"  source:  {running['matched_artifact']}")
    return db_meta, server_meta
