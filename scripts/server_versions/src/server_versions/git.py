from __future__ import annotations

from .models import GitState
from .process import run_capture


def git_text(*args: str) -> str:
    proc = run_capture(["git", *args])
    return proc.stdout.strip()


def git_state() -> GitState:
    branch = git_text("branch", "--show-current") or "detached"
    commit = git_text("rev-parse", "HEAD")
    short_commit = git_text("rev-parse", "--short=8", "HEAD")
    subject = git_text("log", "-1", "--format=%s")
    # Dirty means uncommitted changes under the directories copied into the image.
    dirty = bool(git_text("status", "--porcelain", "--", "server", "shared"))
    repo_dirty = bool(git_text("status", "--porcelain"))
    return GitState(
        branch=branch,
        commit=commit,
        short_commit=short_commit,
        subject=subject,
        dirty=dirty,
        repo_dirty=repo_dirty,
    )
