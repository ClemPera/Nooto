from __future__ import annotations

import gzip
import sys
from pathlib import Path
from typing import TYPE_CHECKING

from ..config import AUTO_DIR, DB_DIR
from ..errors import CliError
from ..formatting import fmt_migrations, fmt_version, human_size, timestamp_name
from ..git import git_state
from ..metadata import (
    artifact_exists,
    ensure_dirs,
    ensure_name_available,
    find_artifact,
    sha256_file,
    unique_name,
    write_metadata,
)
from ..models import DbInfo, DumpInfo, Metadata, ProbeInfo
from ..naming import default_name, validate_name
from ..probe import (
    VersionProbe,
    print_running_server,
    running_server_info,
)
from ..prompts import prompt_yes_no
from .builders import db_metadata
from .cleanup import prune_auto_backups, remove_db_dump, remove_snapshot_artifact
from .server import capture_running_image
from .warnings import print_unknown_running_warning

if TYPE_CHECKING:
    from ..docker import Docker

__all__ = [
    "auto_backup_db",
    "auto_name",
    "collect_db_info",
    "dump_db_file",
    "save_db",
]


def collect_db_info(docker: Docker) -> DbInfo:
    charset, collation = docker.db_charset()
    return DbInfo(
        database=docker.db_name(),
        applied_migrations=docker.db_migrations(),
        charset=charset,
        collation=collation,
    )


def wait_for_server(docker: Docker, timeout: int = 60) -> ProbeInfo:
    return VersionProbe(docker).wait(timeout=timeout)


def dump_db_file(
    docker: Docker,
    name: str,
    target_dir: Path,
    *,
    stop_server: bool = True,
    db_info: DbInfo | None = None,
) -> DumpInfo:
    target_dir.mkdir(parents=True, exist_ok=True)
    info = db_info or collect_db_info(docker)
    was_active = docker.server_active()
    tmp = target_dir / f".{name}.sql.gz.tmp"
    final = target_dir / f"{name}.sql.gz"
    if final.exists():
        raise CliError(f"Refusing to overwrite existing DB dump: {final}")
    stopped = False
    sha = ""
    size = 0
    try:
        if was_active and stop_server:
            docker.stop_server()
            stopped = True
        with (
            tmp.open("wb") as raw,
            gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as gz,
        ):
            docker.dump_db_to(gz)
        try:
            uncompressed = 0
            with gzip.open(tmp, "rb") as gz:
                while chunk := gz.read(1024 * 1024):
                    uncompressed += len(chunk)
        except (OSError, EOFError) as exc:
            raise CliError(f"DB dump is not valid gzip: {exc}") from exc
        if uncompressed == 0:
            raise CliError("DB dump is empty.")
        sha = sha256_file(tmp)
        size = tmp.stat().st_size
        tmp.chmod(0o600)
        result = DumpInfo(
            database=info["database"],
            applied_migrations=info["applied_migrations"],
            charset=info["charset"],
            collation=info["collation"],
            file=final.name,
            sha256=sha,
            size_bytes=size,
        )
        tmp.replace(final)
    except BaseException:
        tmp.unlink(missing_ok=True)
        raise
    finally:
        if was_active and stop_server and not stopped:
            try:
                if not docker.server_active():
                    stopped = True
            except Exception:  # noqa: BLE001 - best-effort state check
                pass
        if stopped:
            try:
                docker.start_stopped_server()
                wait_for_server(docker, timeout=30)
            except Exception as exc:  # noqa: BLE001 - best-effort restart
                print(f"Warning: server did not come back up: {exc}", file=sys.stderr)
    return result


def auto_name(reason: str) -> str:
    base = f"{reason}-{timestamp_name()}"
    name = base
    counter = 2
    while (AUTO_DIR / f"{name}.json").exists() or (AUTO_DIR / f"{name}.sql.gz").exists():
        name = f"{base[:90]}-{counter}"
        counter += 1
    return name


def auto_backup_db(docker: Docker, reason: str) -> Metadata:
    ensure_dirs()
    name = auto_name(reason)
    git = git_state()
    running = running_server_info(docker)
    info = collect_db_info(docker)
    dump_created = False
    try:
        dump_created = True
        dump = dump_db_file(docker, name, AUTO_DIR, stop_server=True, db_info=info)
        meta = db_metadata(name, git, running, dump, auto=True, reason=reason)
        write_metadata("db", meta)
    except BaseException:
        if dump_created:
            remove_db_dump(name, auto=True)
        raise
    prune_auto_backups()
    return meta


def save_db(
    docker: Docker,
    *,
    name: str | None = None,
    with_running_image: bool | None = None,
    assume_yes: bool = False,
) -> Metadata:
    ensure_dirs()
    git = git_state()
    docker.ensure_db_up()
    running = running_server_info(docker)
    running_active = running["version_source"] != "not-running"
    matched_name = running.get("matched_artifact")
    suffix: str | None = None
    if running_active:
        if not running["source_known"]:
            suffix = "running-unknown"
        elif matched_name:
            matched = find_artifact("server", matched_name)
            matched_source = matched.get("source") if matched else None
            if isinstance(matched_source, dict):
                matched_short = matched_source.get("short_commit")
                if isinstance(matched_short, str) and matched_short != git["short_commit"]:
                    suffix = f"running-{matched_short}"
                    if matched_source.get("dirty"):
                        suffix += "-dirty"
                elif matched_source.get("dirty"):
                    suffix = "running-dirty"

    if running_active and not running["source_known"]:
        print_unknown_running_warning(running, git)
        if with_running_image is None:
            if assume_yes or not sys.stdin.isatty():
                # Match the interactive prompt default for non-interactive runs.
                with_running_image = True
            else:
                with_running_image = prompt_yes_no(
                    "Save this exact running image alongside the DB dump?",
                    default=True,
                )
    elif with_running_image and not running_active:
        raise CliError("--with-running-image requires a running server.")

    if running_active and running["source_known"]:
        print_running_server(running)
    elif not running_active:
        print("Server is not running; the DB dump has no running image.")

    if with_running_image and running_active and not running.get("image_id"):
        raise CliError("Could not determine the running image ID.")
    if name:
        name = validate_name(name)
        kinds = ["db"] + (["server"] if with_running_image and running_active else [])
        ensure_name_available(name, kinds)
    else:
        kinds = ["db", "server"] if (with_running_image and running_active) else ["db"]
        name = unique_name(
            default_name(git, extra=suffix),
            kinds,
            docker=docker if "server" in kinds else None,
        )

    captured = bool(with_running_image and running_active)
    if captured:
        tag = docker.snapshot_tag(name)
        if docker.image_exists(tag) and not artifact_exists("server", name):
            raise CliError(f"Image tag {tag} already exists without metadata. Choose another name.")
        capture_running_image(docker, name, running, git)
        running["captured_artifact"] = name

    dump_created = False
    try:
        print(f"Saving DB dump {name} ...")
        info = collect_db_info(docker)
        dump_created = True
        dump = dump_db_file(docker, name, DB_DIR, stop_server=True, db_info=info)
        meta = db_metadata(name, git, running, dump)
        write_metadata("db", meta)
    except BaseException:
        if dump_created:
            remove_db_dump(name)
        if captured:
            remove_snapshot_artifact(docker, name)
        raise

    print(f"Saved DB dump: {DB_DIR / dump['file']}")
    print(f"  size:     {human_size(dump['size_bytes'])}")
    print(f"  schema:   {fmt_migrations(dump['applied_migrations'])}")
    print(f"  database: {dump['database']}")
    print(f"  running:  {fmt_version(running)}")
    captured_artifact = running.get("captured_artifact")
    if captured_artifact:
        print(f"  image:    captured as {captured_artifact}")
    return meta
