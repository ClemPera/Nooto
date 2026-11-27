from __future__ import annotations

import gzip
import os
import shutil
import tempfile
from pathlib import Path
from typing import TYPE_CHECKING

from ..config import SYSTEM_DATABASES
from ..errors import CliError
from ..metadata import db_file_path, sha256_file
from ..models import Metadata

if TYPE_CHECKING:
    from ..docker import Docker

__all__ = ["restore_db_from_meta", "verify_db_dump"]


def verify_db_dump(meta: Metadata) -> Path:
    path = db_file_path(meta)
    if not path.exists():
        raise CliError(f"DB dump is missing: {path}")
    actual = sha256_file(path)
    db = meta.get("db")
    expected = db.get("sha256") if isinstance(db, dict) else None
    if not expected:
        raise CliError(f"DB artifact {meta.get('name', '?')} has no checksum.")
    if actual != expected:
        raise CliError(f"DB dump checksum mismatch for {path}: expected {expected}, got {actual}")
    return path


def restore_db_from_meta(docker: Docker, meta: Metadata) -> None:
    path = verify_db_dump(meta)
    db = meta.get("db")
    if not isinstance(db, dict):
        raise CliError(f"DB artifact {meta.get('name', '?')} has no DB metadata.")
    expected_db = db.get("database")
    if not isinstance(expected_db, str) or not expected_db:
        raise CliError(f"DB artifact {meta.get('name', '?')} has no database name.")
    if expected_db.lower() in SYSTEM_DATABASES:
        raise CliError(f"Refusing to restore into system database {expected_db}.")

    fd, tmp_name = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.restore-", suffix=".sql")
    tmp = Path(tmp_name)
    try:
        with os.fdopen(fd, "wb") as raw, gzip.open(path, "rb") as gz:
            shutil.copyfileobj(gz, raw, length=1024 * 1024)
        if tmp.stat().st_size == 0:
            raise CliError("DB dump is empty after decompression.")

        docker.refresh_sudo()
        current_db = docker.db_name()
        if current_db.lower() in SYSTEM_DATABASES:
            raise CliError(f"Refusing to restore into system database {current_db}.")
        if current_db != expected_db:
            raise CliError(f"DB dump is for {expected_db}, but the container uses {current_db}.")

        docker.clear_db()
        with tmp.open("rb") as raw:
            docker.import_db(raw)
    except (OSError, EOFError) as exc:
        raise CliError(f"Could not read DB dump {path}: {exc}") from exc
    finally:
        tmp.unlink(missing_ok=True)
