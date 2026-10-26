from __future__ import annotations

import re
import tomllib

from .config import REPO_ROOT


def read_server_version() -> str | None:
    path = REPO_ROOT / "server" / "Cargo.toml"
    try:
        data = tomllib.loads(path.read_text())
    except (OSError, tomllib.TOMLDecodeError):
        return None
    package = data.get("package")
    if not isinstance(package, dict):
        return None
    version = package.get("version")
    return str(version) if version else None


def read_protocol_version() -> int | None:
    path = REPO_ROOT / "shared" / "src" / "lib.rs"
    try:
        text = path.read_text()
    except OSError:
        return None
    match = re.search(
        r"pub\s+const\s+PROTOCOL_VERSION\s*:\s*u32\s*=\s*(\d+)\s*;",
        text,
    )
    return int(match.group(1)) if match else None


def read_source_migrations() -> list[int]:
    versions: list[int] = []
    for path in (
        REPO_ROOT / "server" / "src" / "migrations.rs",
        REPO_ROOT / "server" / "migrations.rs",
    ):
        try:
            text = path.read_text()
        except OSError:
            continue
        versions = [int(value) for value in re.findall(r"\(\s*(\d+)\s*,\s*include_str!", text)]
        if versions:
            break
    if not versions:
        for migration in (REPO_ROOT / "server" / "migrations").glob("V*__*.sql"):
            match = re.match(r"V(\d+)__", migration.name)
            if match:
                versions.append(int(match.group(1)))
    return sorted(set(versions))
