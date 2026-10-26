from __future__ import annotations

import json
import subprocess
from typing import cast

from ..formatting import normalize_image_id
from ..process import run_binary, run_capture, run_quiet, run_text
from .base import DockerBase


class DockerImages(DockerBase):
    def image_exists(self, ref: str) -> bool:
        return run_quiet([*self.prefix(), "image", "inspect", ref]) == 0

    def image_id(self, ref: str) -> str | None:
        proc = run_capture(
            [*self.prefix(), "image", "inspect", "--format", "{{.Id}}", ref],
            check=False,
        )
        return normalize_image_id(proc.stdout.strip())

    def image_created(self, ref: str) -> str | None:
        proc = run_capture(
            [*self.prefix(), "image", "inspect", "--format", "{{.Created}}", ref],
            check=False,
        )
        return proc.stdout.strip() or None

    def image_tags(self, ref: str) -> list[str] | None:
        proc = run_capture(
            [
                *self.prefix(),
                "image",
                "inspect",
                "--format",
                "{{json .RepoTags}}",
                ref,
            ],
            check=False,
        )
        if proc.returncode != 0:
            return None
        try:
            data = json.loads(proc.stdout)
        except json.JSONDecodeError:
            return None
        if not isinstance(data, list) or not all(isinstance(item, str) for item in data):
            return None
        return cast("list[str]", data)

    def tag(self, source: str, target: str) -> None:
        self.refresh_sudo()
        run_text([*self.prefix(), "tag", source, target])

    def rmi(self, ref: str) -> subprocess.CompletedProcess[bytes]:
        return run_binary(
            [*self.prefix(), "rmi", ref],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def snapshot_tag(self, name: str) -> str:
        return f"{self.snapshot_repo}:{name}"
