from __future__ import annotations

import sys
from typing import TYPE_CHECKING

from ..config import AUTO_DIR, AUTO_KEEP, DB_DIR
from ..errors import CliError
from ..formatting import normalize_image_id, short_id
from ..metadata import (
    db_file_path,
    load_artifacts,
    metadata_path,
    read_metadata,
)
from ..models import Metadata

if TYPE_CHECKING:
    from ..docker import Docker


def remove_db_dump(name: str, *, auto: bool = False) -> None:
    base = AUTO_DIR if auto else DB_DIR
    try:
        (base / f"{name}.sql.gz").unlink(missing_ok=True)
    except OSError as exc:
        print(f"Warning: could not remove DB dump for {name}: {exc}", file=sys.stderr)
    try:
        metadata_path("db", name, auto=auto).unlink(missing_ok=True)
    except OSError as exc:
        print(f"Warning: could not remove DB metadata for {name}: {exc}", file=sys.stderr)


def remove_snapshot_artifact(docker: Docker, name: str) -> None:
    try:
        metadata_path("server", name).unlink(missing_ok=True)
    except OSError as exc:
        print(f"Warning: could not remove server metadata for {name}: {exc}", file=sys.stderr)
    tag = docker.snapshot_tag(name)
    try:
        proc = docker.rmi(tag)
    except Exception as exc:  # noqa: BLE001 - best-effort cleanup
        print(f"Warning: could not remove tag {tag}: {exc}", file=sys.stderr)
        return
    if proc.returncode == 0:
        return
    detail = proc.stderr.decode(errors="replace").strip()
    if "No such image" not in detail:
        print(
            f"Warning: could not remove tag {tag}: {detail or 'docker rmi failed'}",
            file=sys.stderr,
        )


def delete_db_artifact(name: str, *, auto: bool = False, meta: Metadata | None = None) -> None:
    path = metadata_path("db", name, auto=auto)
    if meta is None and path.exists():
        meta = read_metadata("db", name, auto=auto)
    if meta:
        db_file_path(meta).unlink(missing_ok=True)
    path.unlink(missing_ok=True)


def _remove_image(docker: Docker, ref: str) -> None:
    proc = docker.rmi(ref)
    if proc.returncode == 0:
        return
    detail = proc.stderr.decode(errors="replace").strip()
    if "No such image" in detail:
        return
    raise CliError(f"Could not remove image {ref}: {detail or 'docker rmi failed'}")


def delete_server_artifact(docker: Docker, name: str, *, meta: Metadata | None = None) -> bool:
    if meta is None:
        meta = read_metadata("server", name)
    if not meta:
        raise CliError(f"No server artifact named {name}.")
    image = meta.get("image")
    if not isinstance(image, dict):
        raise CliError(f"Server artifact {name} has no image metadata; refusing to delete.")
    stored_tag = image.get("tag")
    tag = stored_tag if isinstance(stored_tag, str) and stored_tag else None
    image_id = normalize_image_id(image.get("id"))
    if not image_id:
        raise CliError(f"Server artifact {name} has no image ID; refusing to delete.")

    if tag and docker.image_exists(tag) and docker.image_id(tag) == image_id:
        _remove_image(docker, tag)
        metadata_path("server", name).unlink(missing_ok=True)
        return True

    if not docker.image_exists(image_id):
        print(
            f"Warning: image for server artifact {name} is already gone; removing metadata only.",
            file=sys.stderr,
        )
        metadata_path("server", name).unlink(missing_ok=True)
        return False

    tags = docker.image_tags(image_id)
    if tags is None:
        raise CliError(
            f"Refusing to delete server artifact {name}: could not inspect the tags "
            f"of image {short_id(image_id)}."
        )
    if tags:
        raise CliError(
            f"Refusing to delete server artifact {name}: image {short_id(image_id)} "
            f"is still tagged ({', '.join(tags)}). Remove those tags first."
        )
    _remove_image(docker, image_id)
    metadata_path("server", name).unlink(missing_ok=True)
    return True


def _artifact_name(meta: Metadata) -> str | None:
    name = meta.get("name")
    return name if isinstance(name, str) else None


def prune_auto_backups() -> None:
    autos = [meta for meta in load_artifacts("db", include_auto=True) if meta.get("auto")]
    for meta in autos[AUTO_KEEP:]:
        name = _artifact_name(meta)
        if name is None:
            continue
        try:
            delete_db_artifact(name, auto=True, meta=meta)
        except (OSError, CliError) as exc:
            print(
                f"Warning: could not prune auto backup {name}: {exc}",
                file=sys.stderr,
            )
