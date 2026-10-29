from __future__ import annotations

from ..formatting import fmt_version
from ..models import GitState, RunningServerInfo


def print_dirty_warning(git: GitState) -> None:
    print()
    print("Dirty server/shared tree")
    print("------------------------")
    print("Uncommitted changes under server/ or shared/ are included in the image.")
    print("The artifact will be marked -dirty and will not match a clean commit.")
    print(f"Checkout: {git['branch']} @ {git['short_commit']}")
    print()


def print_unknown_running_warning(running: RunningServerInfo, git: GitState) -> None:
    print()
    print("Unknown running image")
    print("--------------------")
    if running.get("matched_artifact"):
        print(
            f"The running image matches saved artifact {running['matched_artifact']}, "
            "but that artifact's branch and commit are unknown."
        )
    else:
        print("The server is running an image that is not one of the saved server versions.")
    print(f"  image:    {running.get('image_id') or '?'}")
    print(f"  created:  {running.get('image_created') or '?'}")
    print(f"  /version: {fmt_version(running)}")
    print(
        f"  checkout: {git['branch']} @ {git['short_commit']} "
        f"({'dirty server/shared' if git['dirty'] else 'clean server/shared'})"
    )
    print("The image's branch and commit cannot be read from the image alone.")
    print("If you save the DB without the image, the dump's source code is unknown.")
    print()
