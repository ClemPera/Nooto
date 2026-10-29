from __future__ import annotations

from typing import cast

from server_versions.docker import Docker
from server_versions.restore.start import start_server_after_restore


class FakeStartDocker:
    def __init__(self) -> None:
        self.calls: list[str] = []

    def start_server(self) -> None:
        self.calls.append("start_server")

    def start_stopped_server(self) -> None:
        self.calls.append("start_stopped_server")


def test_start_after_image_switch_force_recreates() -> None:
    fake = FakeStartDocker()
    start_server_after_restore(cast(Docker, fake), image_switched=True)
    assert fake.calls == ["start_server"]


def test_start_without_image_switch_keeps_container_image() -> None:
    fake = FakeStartDocker()
    start_server_after_restore(cast(Docker, fake), image_switched=False)
    assert fake.calls == ["start_stopped_server"]
