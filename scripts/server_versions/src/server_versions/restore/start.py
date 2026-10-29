from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..docker import Docker

__all__ = ["start_server_after_restore"]


def start_server_after_restore(docker: Docker, *, image_switched: bool) -> None:
    if image_switched:
        docker.start_server()
    else:
        docker.start_stopped_server()
