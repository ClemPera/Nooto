from __future__ import annotations

from .compose import DockerCompose
from .images import DockerImages


class Docker(DockerCompose, DockerImages):
    pass


__all__ = ["Docker"]
