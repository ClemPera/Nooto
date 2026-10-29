from __future__ import annotations

import sys
from typing import TYPE_CHECKING

from ..errors import CliError
from ..formatting import normalize_image_id, short_id
from ..models import Metadata

if TYPE_CHECKING:
    from ..docker import Docker

__all__ = ["verify_server_image"]


def verify_server_image(docker: Docker, meta: Metadata) -> str:
    image = meta.get("image")
    if not isinstance(image, dict):
        raise CliError(f"Server artifact {meta.get('name', '?')} has no image metadata.")
    tag = image.get("tag")
    image_id = normalize_image_id(image.get("id"))
    if not image_id:
        raise CliError(f"Server artifact {meta.get('name', '?')} has no image ID.")
    if isinstance(tag, str) and tag and docker.image_exists(tag):
        actual = docker.image_id(tag)
        if actual == image_id:
            return tag
        if actual:
            print(
                f"Warning: tag {tag} now points to {short_id(actual)}; "
                f"using image ID {short_id(image_id)} instead.",
                file=sys.stderr,
            )
    if docker.image_exists(image_id):
        return image_id
    raise CliError(
        f"Server image for {meta['name']} is missing. "
        "The tag and image ID are both unavailable; save the image again."
    )
