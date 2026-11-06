from __future__ import annotations

import sys
import time
from typing import TYPE_CHECKING

from ..errors import CliError
from ..formatting import fmt_migrations, fmt_version, short_id, timestamp_name
from ..git import git_state
from ..metadata import (
    artifact_exists,
    ensure_dirs,
    ensure_name_available,
    find_artifact,
    server_artifact_available,
    unique_name,
    write_metadata,
)
from ..models import GitState, Metadata, RunningServerInfo
from ..naming import default_name, name_from_source, validate_name
from ..probe import running_server_info
from ..prompts import prompt_yes_no
from .builders import server_metadata_checkout, server_metadata_running
from .cleanup import remove_snapshot_artifact
from .warnings import print_dirty_warning

if TYPE_CHECKING:
    from ..docker import Docker

__all__ = [
    "capture_running_image",
    "save_running_server",
    "save_server",
]


def capture_running_image(
    docker: Docker,
    name: str,
    running: RunningServerInfo,
    git: GitState,
) -> Metadata:
    image_id = running.get("image_id")
    if not image_id:
        raise CliError("Could not determine the running image ID.")
    tag = docker.snapshot_tag(name)
    if docker.image_exists(tag) and not artifact_exists("server", name):
        raise CliError(f"Image tag {tag} already exists without metadata. Choose another name.")
    try:
        docker.tag(image_id, tag)
        matched_name = running.get("matched_artifact")
        matched = find_artifact("server", matched_name) if matched_name else None
        meta = server_metadata_running(name, git, docker, tag, image_id, running, matched)
        write_metadata("server", meta)
    except BaseException:
        remove_snapshot_artifact(docker, name)
        raise
    print(f"  captured running image as {tag}")
    return meta


def save_server(
    docker: Docker,
    *,
    name: str | None = None,
    running: bool = False,
    allow_dirty: bool = False,
    assume_yes: bool = False,
) -> Metadata:
    ensure_dirs()
    git = git_state()
    if running:
        return save_running_server(docker, git, name=name, assume_yes=assume_yes)

    if git["dirty"] and not allow_dirty:
        print_dirty_warning(git)
        if assume_yes or not sys.stdin.isatty():
            raise CliError(
                "Dirty server/shared tree. Commit the changes or re-run with --allow-dirty."
            )
        if not prompt_yes_no(
            "Build includes uncommitted changes and will be marked -dirty. Continue?",
            default=False,
        ):
            raise CliError("Aborted.")

    if name:
        name = validate_name(name)
        ensure_name_available(name, ["server"])
    else:
        name = unique_name(default_name(git), ["server"], docker=docker)
    tag = docker.snapshot_tag(name)
    if docker.image_exists(tag) and not artifact_exists("server", name):
        raise CliError(f"Image tag {tag} already exists without metadata. Choose another name.")

    dirty_note = " (dirty)" if git["dirty"] else ""
    print(
        f"Building {docker.compose_image} from "
        f"{git['branch']} @ {git['short_commit']}{dirty_note} ..."
    )
    started = time.monotonic()
    docker.build_server()
    elapsed = time.monotonic() - started
    image_id = docker.image_id(docker.compose_image)
    if not image_id:
        raise CliError("Build finished but the compose image could not be found.")
    try:
        docker.tag(docker.compose_image, tag)
        meta = server_metadata_checkout(name, git, docker, tag, image_id)
        write_metadata("server", meta)
    except BaseException:
        remove_snapshot_artifact(docker, name)
        raise

    print(f"Saved server image in {elapsed:.1f}s: {tag}")
    print(f"  image:    {image_id}")
    print(f"  source:   {git['branch']} @ {git['short_commit']}{dirty_note}")
    print(
        f"  version:  {meta['server']['version']} "
        f"protocol {meta['server']['protocol_version']} "
        f"migrations {fmt_migrations(meta['server']['migrations'])}"
    )
    print("The running server was not changed. Use restore/switch to run this image.")
    return meta


def save_running_server(
    docker: Docker,
    git: GitState,
    *,
    name: str | None = None,
    assume_yes: bool = False,
) -> Metadata:
    ensure_dirs()
    if not docker.server_running():
        raise CliError("The server is not running, so there is no running image to save.")
    running = running_server_info(docker)
    image_id = running.get("image_id")
    if not image_id:
        raise CliError("Could not determine the running image ID.")
    matched_name = running.get("matched_artifact")
    matched = find_artifact("server", matched_name) if matched_name else None

    matched_available = bool(matched and server_artifact_available(docker, matched))
    if name:
        name = validate_name(name)
        ensure_name_available(name, ["server"])
    elif running["source_known"] and matched and matched_available:
        print(f"Running image is already saved as {running['matched_artifact']}.")
        if assume_yes or not sys.stdin.isatty():
            return matched
        if not prompt_yes_no("Create another copy?", default=False):
            return matched
        matched_source = matched.get("source")
        if not isinstance(matched_source, dict):
            matched_source = {}
        name = unique_name(
            name_from_source(matched_source, extra="copy"),
            ["server"],
            docker=docker,
        )
    else:
        if matched and not matched_available:
            print("Saved metadata exists but its image is missing; saving a new copy.")
        name = unique_name(
            f"running-{short_id(image_id)}-{timestamp_name()}",
            ["server"],
            docker=docker,
        )

    tag = docker.snapshot_tag(name)
    if docker.image_exists(tag) and not artifact_exists("server", name):
        raise CliError(f"Image tag {tag} already exists without metadata. Choose another name.")
    try:
        docker.tag(image_id, tag)
        meta = server_metadata_running(name, git, docker, tag, image_id, running, matched)
        write_metadata("server", meta)
    except BaseException:
        remove_snapshot_artifact(docker, name)
        raise

    print(f"Saved running server image: {tag}")
    print(f"  image:   {image_id}")
    print(f"  /version: {fmt_version(running)}")
    if meta["source"]["known"]:
        print(f"  source:  {meta['source']['branch']} @ {meta['source']['short_commit']}")
    else:
        print("  source:  unknown (the image does not match a saved server version)")
    return meta
