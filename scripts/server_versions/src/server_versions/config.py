from __future__ import annotations

import os
from pathlib import Path

from .errors import CliError


def _is_repo_root(path: Path) -> bool:
    return (path / "docker-compose.yml").is_file() and (path / "server").is_dir()


def _search_up(start: Path) -> Path | None:
    for candidate in (start, *start.parents):
        if _is_repo_root(candidate):
            return candidate
    return None


def _resolve_repo_root() -> Path:
    env = os.environ.get("NOOTO_ROOT")
    if env:
        root = Path(env).expanduser().resolve()
        if not _is_repo_root(root):
            raise CliError(f"NOOTO_ROOT does not contain docker-compose.yml and server/: {root}")
        return root
    for start in (Path(__file__).resolve().parent, Path.cwd().resolve()):
        found = _search_up(start)
        if found is not None:
            return found
    raise CliError(
        "Could not find the Nooto repository root. Run from the repository or set NOOTO_ROOT."
    )


REPO_ROOT: Path = _resolve_repo_root()
SNAPSHOT_ROOT = REPO_ROOT / ".server-snapshots"
DB_DIR = SNAPSHOT_ROOT / "db"
SERVER_DIR = SNAPSHOT_ROOT / "server"
AUTO_DIR = DB_DIR / "_auto"
LOCK_FILE = SNAPSHOT_ROOT / ".lock"

DB_SERVICE = "db"
SERVER_SERVICE = "server"
AUTO_KEEP = 5
SCHEMA_VERSION = 1
SYSTEM_DATABASES = frozenset({"mysql", "information_schema", "performance_schema", "sys"})
