from __future__ import annotations

import subprocess
from typing import IO, Any

from .config import REPO_ROOT
from .errors import CliError

StreamArg = int | IO[Any] | None


def _failure(cmd: list[str], proc: subprocess.CompletedProcess[Any]) -> CliError:
    detail = proc.stderr.strip() if isinstance(proc.stderr, str) else ""
    return CliError(
        f"Command failed ({proc.returncode}): {' '.join(cmd)}" + (f"\n{detail}" if detail else "")
    )


def run_capture(cmd: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    try:
        proc = subprocess.run(
            cmd,
            check=False,
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            errors="replace",
        )
    except FileNotFoundError as exc:
        raise CliError(f"Command not found: {cmd[0]}") from exc
    if check and proc.returncode != 0:
        raise _failure(cmd, proc)
    return proc


def run_text(
    cmd: list[str],
    *,
    check: bool = True,
    stdin: StreamArg = None,
    stdout: StreamArg = None,
    stderr: StreamArg = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    try:
        proc = subprocess.run(
            cmd,
            check=False,
            cwd=REPO_ROOT,
            stdin=stdin,
            stdout=stdout,
            stderr=stderr,
            env=env,
            text=True,
            errors="replace",
        )
    except FileNotFoundError as exc:
        raise CliError(f"Command not found: {cmd[0]}") from exc
    if check and proc.returncode != 0:
        raise _failure(cmd, proc)
    return proc


def run_binary(
    cmd: list[str],
    *,
    check: bool = True,
    stdin: StreamArg = None,
    stdout: StreamArg = None,
    stderr: StreamArg = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[bytes]:
    try:
        proc = subprocess.run(
            cmd,
            check=False,
            cwd=REPO_ROOT,
            stdin=stdin,
            stdout=stdout,
            stderr=stderr,
            env=env,
        )
    except FileNotFoundError as exc:
        raise CliError(f"Command not found: {cmd[0]}") from exc
    if check and proc.returncode != 0:
        detail = proc.stderr.decode(errors="replace").strip() if proc.stderr else ""
        raise CliError(
            f"Command failed ({proc.returncode}): {' '.join(cmd)}"
            + (f"\n{detail}" if detail else "")
        )
    return proc


def run_quiet(cmd: list[str]) -> int:
    try:
        return subprocess.run(
            cmd,
            check=False,
            cwd=REPO_ROOT,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode
    except FileNotFoundError:
        return 127
