from __future__ import annotations

import sys
from typing import TYPE_CHECKING

from ..formatting import fmt_version
from ..models import Metadata
from ..probe import VersionProbe
from .db import restore_db_from_meta
from .start import start_server_after_restore

if TYPE_CHECKING:
    from ..docker import Docker

__all__ = ["rollback_restore"]


def rollback_restore(
    docker: Docker,
    auto: Metadata | None,
    db_touched: bool,
    image_switched: bool,
    previous_image_id: str | None,
    no_start: bool,
) -> None:
    print("Rolling back ...", file=sys.stderr)
    can_start = True
    try:
        docker.refresh_sudo()
    except Exception as exc:  # noqa: BLE001 - best-effort rollback
        print(f"  Could not refresh sudo: {exc}", file=sys.stderr)
    if db_touched:
        stopped = True
        try:
            docker.stop_server()
        except Exception as exc:  # noqa: BLE001 - best-effort rollback
            try:
                running = docker.server_may_be_active()
            except Exception:  # noqa: BLE001 - assume active when unknown
                running = True
            if running:
                stopped = False
                can_start = False
                print(
                    f"  Could not stop server for DB rollback: {exc}",
                    file=sys.stderr,
                )
            else:
                print(f"  Warning: docker compose stop failed: {exc}", file=sys.stderr)
        if stopped:
            if auto:
                try:
                    restore_db_from_meta(docker, auto)
                    print("  DB restored from the safety backup.", file=sys.stderr)
                except Exception as exc:  # noqa: BLE001 - best-effort rollback
                    can_start = False
                    print(f"  DB rollback failed: {exc}", file=sys.stderr)
            else:
                can_start = False
                print(
                    "  No safety backup is available; leaving the server stopped.",
                    file=sys.stderr,
                )
    if image_switched and previous_image_id:
        try:
            docker.tag(previous_image_id, docker.compose_image)
            print("  Previous server image restored.", file=sys.stderr)
        except Exception as exc:  # noqa: BLE001 - best-effort rollback
            can_start = False
            print(f"  Image rollback failed: {exc}", file=sys.stderr)
    elif image_switched:
        can_start = False
        print(
            "  Previous image ID is unknown; image rollback skipped.",
            file=sys.stderr,
        )
    if not no_start and can_start:
        try:
            start_server_after_restore(docker, image_switched=image_switched)
            info = VersionProbe(docker).wait()
            print(f"  Server up: {fmt_version(info)}", file=sys.stderr)
        except Exception as exc:  # noqa: BLE001 - best-effort rollback
            print(f"  Server restart failed: {exc}", file=sys.stderr)
    elif not no_start:
        print(
            "  Server left stopped because rollback did not complete.",
            file=sys.stderr,
        )
