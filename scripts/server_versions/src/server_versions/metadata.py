from __future__ import annotations

import contextlib
import datetime as dt
import fcntl
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import IO, TYPE_CHECKING, Any, cast

from .config import AUTO_DIR, DB_DIR, LOCK_FILE, SCHEMA_VERSION, SERVER_DIR, SNAPSHOT_ROOT
from .errors import CliError
from .formatting import normalize_image_id
from .models import Metadata
from .naming import validate_db_filename, validate_name

if TYPE_CHECKING:
    from .docker import Docker


def ensure_dirs() -> None:
    for path in (SNAPSHOT_ROOT, DB_DIR, SERVER_DIR, AUTO_DIR):
        path.mkdir(parents=True, exist_ok=True)
        with contextlib.suppress(OSError):
            path.chmod(0o700)


def acquire_lock() -> IO[str]:
    ensure_dirs()
    handle = LOCK_FILE.open("w")
    with contextlib.suppress(OSError):
        LOCK_FILE.chmod(0o600)
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as exc:
        handle.close()
        raise CliError("Another server-versions.py process is already running.") from exc
    return handle


def metadata_path(kind: str, name: str, *, auto: bool = False) -> Path:
    name = validate_name(name)
    base = AUTO_DIR if auto else (DB_DIR if kind == "db" else SERVER_DIR)
    return base / f"{name}.json"


def _metadata_error(path: Path | None, message: str) -> CliError:
    if path is None:
        return CliError(f"Invalid metadata: {message}")
    return CliError(f"Invalid metadata {path}: {message}")


def _metadata_dict(value: Any, label: str, path: Path | None) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise _metadata_error(path, f"{label} is not an object")
    return cast("dict[str, Any]", value)


def _metadata_str(
    value: Any, label: str, path: Path | None, *, optional: bool = False
) -> str | None:
    if value is None and optional:
        return None
    if not isinstance(value, str):
        raise _metadata_error(path, f"{label} is not a string")
    return value


def _require_str(value: Any, label: str, path: Path | None) -> str:
    result = _metadata_str(value, label, path)
    if result is None:
        raise _metadata_error(path, f"{label} is not a string")
    return result


def _metadata_bool(value: Any, label: str, path: Path | None) -> bool:
    if not isinstance(value, bool):
        raise _metadata_error(path, f"{label} is not a boolean")
    return value


def _metadata_int(
    value: Any, label: str, path: Path | None, *, optional: bool = False
) -> int | None:
    if value is None and optional:
        return None
    if not isinstance(value, int) or isinstance(value, bool):
        raise _metadata_error(path, f"{label} is not an integer")
    return value


def _require_int(value: Any, label: str, path: Path | None) -> int:
    result = _metadata_int(value, label, path)
    if result is None:
        raise _metadata_error(path, f"{label} is not an integer")
    return result


def _metadata_migrations(
    value: Any, label: str, path: Path | None, *, optional: bool = False
) -> list[int] | None:
    if value is None and optional:
        return None
    if not isinstance(value, list) or not all(
        isinstance(item, int) and not isinstance(item, bool) for item in value
    ):
        raise _metadata_error(path, f"{label} is not a list of integers")
    return cast("list[int]", value)


def validate_metadata(kind: str, meta: Any, *, path: Path | None = None) -> Metadata:
    data = _metadata_dict(meta, "root", path)
    schema_version = data.get("schema_version")
    if type(schema_version) is not int or schema_version != SCHEMA_VERSION:
        raise _metadata_error(path, "unsupported schema_version")
    if data.get("kind") != kind:
        raise _metadata_error(path, f"kind is not {kind!r}")
    name = _require_str(data.get("name"), "name", path)
    try:
        validate_name(name)
    except CliError as exc:
        raise _metadata_error(path, str(exc)) from exc
    _require_str(data.get("created_at"), "created_at", path)
    if "auto" in data:
        _metadata_bool(data["auto"], "auto", path)
    if "reason" in data:
        _metadata_str(data["reason"], "reason", path, optional=True)

    if kind == "db":
        db = _metadata_dict(data.get("db"), "db", path)
        _require_str(db.get("database"), "db.database", path)
        filename = _require_str(db.get("file"), "db.file", path)
        try:
            validate_db_filename(filename)
        except CliError as exc:
            raise _metadata_error(path, str(exc)) from exc
        expected_file = f"{name}.sql.gz"
        if filename != expected_file:
            raise _metadata_error(path, f"db.file is not {expected_file}")
        sha = _require_str(db.get("sha256"), "db.sha256", path)
        if not re.fullmatch(r"[0-9a-f]{64}", sha):
            raise _metadata_error(path, "db.sha256 is not a SHA-256 hex digest")
        size = _require_int(db.get("size_bytes"), "db.size_bytes", path)
        if size < 0:
            raise _metadata_error(path, "db.size_bytes is negative")
        _metadata_migrations(db.get("applied_migrations"), "db.applied_migrations", path)
        _metadata_str(db.get("charset"), "db.charset", path, optional=True)
        _metadata_str(db.get("collation"), "db.collation", path, optional=True)

        checkout = _metadata_dict(data.get("checkout"), "checkout", path)
        for field in ("branch", "commit", "short_commit", "subject"):
            _metadata_str(checkout.get(field), f"checkout.{field}", path, optional=True)
        for field in ("dirty", "repo_dirty", "detached"):
            if field in checkout:
                _metadata_bool(checkout[field], f"checkout.{field}", path)

        running = _metadata_dict(data.get("running_server"), "running_server", path)
        _metadata_str(running.get("image_id"), "running_server.image_id", path, optional=True)
        _metadata_str(
            running.get("image_created"),
            "running_server.image_created",
            path,
            optional=True,
        )
        _metadata_str(
            running.get("server_version"),
            "running_server.server_version",
            path,
            optional=True,
        )
        _metadata_int(
            running.get("protocol_version"),
            "running_server.protocol_version",
            path,
            optional=True,
        )
        _require_str(running.get("version_source"), "running_server.version_source", path)
        _metadata_bool(running.get("source_known"), "running_server.source_known", path)
        _metadata_str(
            running.get("matched_artifact"),
            "running_server.matched_artifact",
            path,
            optional=True,
        )
        _metadata_str(
            running.get("captured_artifact"),
            "running_server.captured_artifact",
            path,
            optional=True,
        )
    else:
        image = _metadata_dict(data.get("image"), "image", path)
        _require_str(image.get("tag"), "image.tag", path)
        image_id = _require_str(image.get("id"), "image.id", path)
        if not normalize_image_id(image_id):
            raise _metadata_error(path, "image.id is empty")
        _metadata_str(image.get("created"), "image.created", path, optional=True)
        _metadata_str(image.get("compose_image"), "image.compose_image", path, optional=True)

        source = _metadata_dict(data.get("source"), "source", path)
        _metadata_bool(source.get("known"), "source.known", path)
        _metadata_str(source.get("kind"), "source.kind", path, optional=True)
        _metadata_str(
            source.get("matched_artifact"),
            "source.matched_artifact",
            path,
            optional=True,
        )
        for field in ("branch", "commit", "short_commit", "subject"):
            _metadata_str(source.get(field), f"source.{field}", path, optional=True)
        for field in ("dirty", "repo_dirty"):
            if field in source:
                _metadata_bool(source[field], f"source.{field}", path)
        if source.get("checkout") is not None:
            _metadata_dict(source["checkout"], "source.checkout", path)

        server = _metadata_dict(data.get("server"), "server", path)
        _metadata_str(server.get("version"), "server.version", path, optional=True)
        _metadata_int(
            server.get("protocol_version"),
            "server.protocol_version",
            path,
            optional=True,
        )
        _metadata_migrations(server.get("migrations"), "server.migrations", path, optional=True)
    return data


def write_metadata(kind: str, meta: Metadata) -> None:
    ensure_dirs()
    validate_metadata(kind, meta)
    name = _require_str(meta.get("name"), "name", None)
    path = metadata_path(kind, name, auto=bool(meta.get("auto")))
    tmp = path.with_suffix(".json.tmp")
    try:
        tmp.write_text(json.dumps(meta, indent=2) + "\n")
        tmp.replace(path)
    except BaseException:
        with contextlib.suppress(OSError):
            tmp.unlink(missing_ok=True)
        raise
    with contextlib.suppress(OSError):
        path.chmod(0o600)


def read_metadata(kind: str, name: str, *, auto: bool = False) -> Metadata | None:
    path = metadata_path(kind, name, auto=auto)
    if not path.exists():
        return None
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise CliError(f"Could not read metadata {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise CliError(f"Metadata {path} is not a JSON object.")
    expected_name = validate_name(name)
    if data.get("name") != expected_name:
        raise CliError(f"Metadata {path} is for {data.get('name')!r}, not {expected_name!r}.")
    return validate_metadata(kind, data, path=path)


def sort_key(meta: Metadata) -> tuple[dt.datetime, str]:
    created = meta.get("created_at")
    if not isinstance(created, str):
        created = ""
    try:
        parsed = dt.datetime.fromisoformat(created)
    except (TypeError, ValueError):
        parsed = dt.datetime.min.replace(tzinfo=dt.UTC)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.UTC)
    name = meta.get("name")
    if not isinstance(name, str):
        name = ""
    return parsed, name


def load_artifacts(kind: str, *, include_auto: bool = False) -> list[Metadata]:
    if kind == "db":
        paths = list(DB_DIR.glob("*.json"))
        if include_auto:
            paths += list(AUTO_DIR.glob("*.json"))
    else:
        paths = list(SERVER_DIR.glob("*.json"))
    items: list[Metadata] = []
    for path in paths:
        try:
            data = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError) as exc:
            print(f"Warning: skipping unreadable metadata {path}: {exc}", file=sys.stderr)
            continue
        if not isinstance(data, dict):
            print(
                f"Warning: skipping metadata {path}: not a JSON object",
                file=sys.stderr,
            )
            continue
        if data.get("name") != path.stem:
            print(
                f"Warning: skipping metadata {path}: name does not match filename",
                file=sys.stderr,
            )
            continue
        try:
            validate_metadata(kind, data, path=path)
        except CliError as exc:
            print(f"Warning: skipping metadata {path}: {exc}", file=sys.stderr)
            continue
        items.append(cast("Metadata", data))
    items.sort(key=sort_key, reverse=True)
    return items


def find_artifact(kind: str, name: str, *, include_auto: bool = False) -> Metadata | None:
    for meta in load_artifacts(kind, include_auto=include_auto):
        if meta.get("name") == name:
            return meta
    return None


def find_server_by_image_id(image_id: str | None) -> Metadata | None:
    wanted = normalize_image_id(image_id)
    if not wanted:
        return None
    for meta in load_artifacts("server"):
        image = meta.get("image")
        if isinstance(image, dict) and normalize_image_id(image.get("id")) == wanted:
            return meta
    return None


def artifact_exists(kind: str, name: str) -> bool:
    base = DB_DIR if kind == "db" else SERVER_DIR
    if (base / f"{name}.json").exists():
        return True
    return kind == "db" and (base / f"{name}.sql.gz").exists()


def unique_name(base: str, kinds: list[str], *, docker: Docker | None = None) -> str:
    name = base
    counter = 2
    while any(artifact_exists(kind, name) for kind in kinds) or (
        docker is not None and docker.image_exists(docker.snapshot_tag(name))
    ):
        suffix = f"-{counter}"
        name = f"{base[: 100 - len(suffix)]}{suffix}"
        counter += 1
    return name


def ensure_name_available(name: str, kinds: list[str]) -> None:
    conflicts = [kind for kind in kinds if artifact_exists(kind, name)]
    if conflicts:
        raise CliError(
            f"Artifact {name} already exists ({', '.join(conflicts)}). Choose another name."
        )


def server_artifact_available(docker: Docker, meta: Metadata) -> bool:
    image = meta.get("image")
    if not isinstance(image, dict):
        return False
    tag = image.get("tag")
    image_id = normalize_image_id(image.get("id"))
    if isinstance(tag, str) and tag and docker.image_exists(tag):
        actual = docker.image_id(tag)
        if actual and (not image_id or actual == image_id):
            return True
    return bool(image_id and docker.image_exists(image_id))


def db_file_path(meta: Metadata) -> Path:
    base = AUTO_DIR if meta.get("auto") else DB_DIR
    db = meta.get("db")
    if not isinstance(db, dict):
        raise CliError(f"DB artifact {meta.get('name', '?')} has no dump file in its metadata.")
    filename = db.get("file")
    if not isinstance(filename, str) or not filename:
        raise CliError(f"DB artifact {meta.get('name', '?')} has no dump file in its metadata.")
    return base / validate_db_filename(filename)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()
