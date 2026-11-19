from __future__ import annotations

import sys
from typing import TYPE_CHECKING, Any

from ..errors import CliError
from ..formatting import fmt_migrations, fmt_version, normalize_image_id, short_id
from ..git import git_state
from ..metadata import (
    ensure_dirs,
    find_server_by_image_id,
    read_metadata,
)
from ..models import Metadata
from ..naming import validate_name
from ..probe import VersionProbe
from ..prompts import prompt_yes_no
from ..snapshots.db import auto_backup_db
from .db import restore_db_from_meta, verify_db_dump
from .image import verify_server_image
from .plan import (
    compatibility_warnings,
    print_restore_summary,
    source_mismatch_warnings,
)
from .rollback import rollback_restore
from .start import start_server_after_restore

if TYPE_CHECKING:
    from ..docker import Docker

__all__ = ["restore"]


def _require_artifact(kind: str, name: str) -> Metadata:
    validated = validate_name(name)
    meta = read_metadata(kind, validated)
    if not meta:
        raise CliError(f"No {kind} artifact named {validated}.")
    return meta


def _section(meta: Metadata | None, key: str) -> dict[str, Any]:
    if meta is None:
        return {}
    value = meta.get(key)
    return value if isinstance(value, dict) else {}


def restore(
    docker: Docker,
    *,
    db_name: str | None = None,
    server_name: str | None = None,
    no_start: bool = False,
    assume_yes: bool = False,
    skip_backup: bool = False,
    no_rollback: bool = False,
) -> None:
    ensure_dirs()
    git = git_state()
    if db_name in ("current", "keep", "-"):
        db_name = None
    if server_name in ("current", "keep", "-"):
        server_name = None

    db_meta = _require_artifact("db", db_name) if db_name else None
    server_meta = _require_artifact("server", server_name) if server_name else None
    if not db_meta and not server_meta:
        raise CliError("Nothing to restore. Choose a DB dump, a server image, or both.")

    if db_meta:
        verify_db_dump(db_meta)
    image_ref = verify_server_image(docker, server_meta) if server_meta else None

    if db_meta or not no_start:
        docker.ensure_db_up()
        current_migrations = docker.db_migrations()
    else:
        current_migrations = None
    current_image_id = docker.server_image_id() or docker.image_id(docker.compose_image)
    current_img_meta = find_server_by_image_id(current_image_id)

    db_section = _section(db_meta, "db")
    server_section = _section(server_meta, "server")
    current_server_section = _section(current_img_meta, "server")
    target_db_migs = db_section.get("applied_migrations") if db_meta else current_migrations
    if server_meta:
        target_img_migs = server_section.get("migrations")
    elif current_img_meta:
        target_img_migs = current_server_section.get("migrations")
    else:
        target_img_migs = None

    print_restore_summary(
        db_meta,
        server_meta,
        current_migrations,
        current_image_id,
        current_img_meta,
        no_start,
        git,
    )
    for warning in compatibility_warnings(target_db_migs, target_img_migs):
        print(f"  {warning}")
    raw_source = (
        server_meta.get("source")
        if server_meta
        else (current_img_meta.get("source") if current_img_meta else None)
    )
    source = raw_source if isinstance(raw_source, dict) else None
    label = "selected server image" if server_meta else "current server image"
    for warning in source_mismatch_warnings(source, git, label):
        print(f"  Warning: {warning}")
    print()

    if not assume_yes and not prompt_yes_no("Proceed with restore?", default=False):
        raise CliError("Aborted.")

    needs_backup = db_meta is not None or not no_start
    auto: Metadata | None = None
    if needs_backup and not skip_backup:
        print("Backing up the current DB ...")
        auto = auto_backup_db(docker, "pre-restore")
        print(f"  safety backup: {auto['name']}")
    elif needs_backup and skip_backup:
        print("Skipping the safety backup (--skip-backup).", file=sys.stderr)

    db_touched = False
    image_switched = False
    try:
        docker.stop_server()

        if db_meta:
            print("Restoring DB ...")
            db_touched = True
            restore_db_from_meta(docker, db_meta)
            restored = docker.db_migrations()
            expected = db_section.get("applied_migrations", [])
            if restored != expected:
                raise CliError(f"DB migrations after restore are {restored}, expected {expected}.")
            print(f"  restored schema {fmt_migrations(restored)}")

        if server_meta and image_ref:
            print("Switching server image ...")
            image_switched = True
            docker.tag(image_ref, docker.compose_image)
            print(f"  {image_ref} -> {docker.compose_image}")

        if not no_start:
            print("Starting server ...")
            db_touched = True
            start_server_after_restore(docker, image_switched=image_switched)
            info = VersionProbe(docker).wait()
            print(f"  server up: {fmt_version(info)}")
            expected_image_id = normalize_image_id(
                _section(server_meta, "image").get("id") if server_meta else current_image_id
            )
            actual_image_id = normalize_image_id(docker.server_image_id())
            if not actual_image_id:
                raise CliError("Could not determine the running server image after start.")
            if expected_image_id and actual_image_id != expected_image_id:
                raise CliError(
                    f"Server is running image {short_id(actual_image_id)}, "
                    f"expected {short_id(expected_image_id)}."
                )
            if server_meta and info.get("version_source") == "http":
                expected_version = server_section.get("version")
                expected_protocol = server_section.get("protocol_version")
                if expected_version and info.get("server_version") != expected_version:
                    print(
                        f"  Warning: /version reports {info.get('server_version')}, "
                        f"saved metadata says {expected_version}."
                    )
                if (
                    expected_protocol is not None
                    and info.get("protocol_version") != expected_protocol
                ):
                    print(
                        f"  Warning: /version reports protocol "
                        f"{info.get('protocol_version')}, saved metadata says "
                        f"{expected_protocol}."
                    )
        else:
            print("Server left stopped (--no-start).")
    except BaseException as exc:
        if isinstance(exc, KeyboardInterrupt):
            print("Restore interrupted.", file=sys.stderr)
        else:
            print(f"Restore failed: {exc}", file=sys.stderr)
        if no_rollback:
            print("Rollback skipped (--no-rollback).", file=sys.stderr)
        else:
            try:
                rollback_restore(
                    docker,
                    auto,
                    db_touched,
                    image_switched,
                    current_image_id,
                    no_start,
                )
            except BaseException as rollback_exc:  # noqa: BLE001 - preserve original
                print(f"Rollback itself failed: {rollback_exc}", file=sys.stderr)
        raise

    print("Restore complete.")
