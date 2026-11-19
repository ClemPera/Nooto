from __future__ import annotations

from typing import cast

import pytest

from server_versions import reset
from server_versions.docker import Docker
from server_versions.docker.compose import DockerCompose
from server_versions.errors import CliError
from server_versions.models import Metadata, ProbeInfo


class FakeDocker:
    def __init__(self, database: str = "nooto", migrations: list[int] | None = None) -> None:
        self.database = database
        self.migrations = [1] if migrations is None else migrations
        self.calls: list[str] = []

    def ensure_db_up(self) -> None:
        pass

    def db_name(self) -> str:
        return self.database

    def db_migrations(self) -> list[int]:
        return list(self.migrations)

    def stop_server(self) -> None:
        self.calls.append("stop_server")

    def drop_database(self) -> None:
        self.calls.append("drop_database")
        self.migrations = []

    def start_stopped_server(self) -> None:
        self.calls.append("start_stopped_server")
        self.migrations = [1]


class FakeProbe:
    def __init__(self, docker: Docker) -> None:
        self.docker = docker

    def wait(self, timeout: int = 60) -> ProbeInfo:
        return ProbeInfo(version_source="http", server_version="0.4.0", protocol_version=3)


@pytest.fixture
def backup_calls(monkeypatch: pytest.MonkeyPatch) -> list[str]:
    calls: list[str] = []

    def fake_backup(_docker: Docker, reason: str) -> Metadata:
        calls.append(reason)
        return {"name": "pre-drop-test"}

    monkeypatch.setattr(reset, "auto_backup_db", fake_backup)
    monkeypatch.setattr(reset, "VersionProbe", FakeProbe)
    return calls


def test_drop_db_default_flow(backup_calls: list[str], capsys: pytest.CaptureFixture[str]) -> None:
    docker = FakeDocker()

    reset.drop_db(cast(Docker, docker), assume_yes=True)

    assert backup_calls == ["pre-drop"]
    assert docker.calls == ["stop_server", "drop_database", "start_stopped_server"]
    output = capsys.readouterr().out
    assert "pre-drop-test" in output
    assert "rebuilt schema V1" in output


def test_drop_db_skip_backup(backup_calls: list[str], capsys: pytest.CaptureFixture[str]) -> None:
    docker = FakeDocker()

    reset.drop_db(cast(Docker, docker), assume_yes=True, skip_backup=True)

    assert backup_calls == []
    assert docker.calls == ["stop_server", "drop_database", "start_stopped_server"]
    assert "pre-drop-test" not in capsys.readouterr().out


def test_drop_db_no_start(backup_calls: list[str], capsys: pytest.CaptureFixture[str]) -> None:
    docker = FakeDocker()

    reset.drop_db(cast(Docker, docker), assume_yes=True, no_start=True)

    assert backup_calls == ["pre-drop"]
    assert docker.calls == ["stop_server", "drop_database"]
    output = capsys.readouterr().out
    assert "left stopped" in output
    assert "rebuilt schema" not in output


def test_drop_db_refuses_system_database(backup_calls: list[str]) -> None:
    docker = FakeDocker(database="mysql")

    with pytest.raises(CliError, match="Refusing to drop system database mysql."):
        reset.drop_db(cast(Docker, docker), assume_yes=True)

    assert docker.calls == []
    assert backup_calls == []


def test_drop_db_declined_prompt(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(reset, "prompt_yes_no", lambda *_args, **_kwargs: False)
    docker = FakeDocker()

    with pytest.raises(CliError, match="Aborted."):
        reset.drop_db(cast(Docker, docker))

    assert docker.calls == []


def test_drop_db_prompt_mentions_database(monkeypatch: pytest.MonkeyPatch) -> None:
    prompts: list[str] = []

    def fake_prompt(question: str, **_kwargs: object) -> bool:
        prompts.append(question)
        return False

    monkeypatch.setattr(reset, "prompt_yes_no", fake_prompt)
    docker = FakeDocker(database="nooto_custom")

    with pytest.raises(CliError, match="Aborted."):
        reset.drop_db(cast(Docker, docker))

    assert len(prompts) == 1
    assert "nooto_custom" in prompts[0]


def make_compose(
    monkeypatch: pytest.MonkeyPatch,
    *,
    database: str,
    charset: str | None = "utf8mb4",
    collation: str | None = "utf8mb4_general_ci",
) -> tuple[DockerCompose, list[str]]:
    compose = object.__new__(DockerCompose)
    executed: list[str] = []
    monkeypatch.setattr(compose, "db_name", lambda: database)
    monkeypatch.setattr(compose, "db_charset", lambda: (charset, collation))
    monkeypatch.setattr(compose, "db_exec_root", executed.append)
    return compose, executed


def test_drop_database_sql(monkeypatch: pytest.MonkeyPatch) -> None:
    compose, executed = make_compose(monkeypatch, database="nooto")

    compose.drop_database()

    assert executed == [
        "DROP DATABASE IF EXISTS `nooto`; CREATE DATABASE `nooto` "
        "CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci"
    ]


def test_drop_database_skips_unsafe_charset(monkeypatch: pytest.MonkeyPatch) -> None:
    compose, executed = make_compose(
        monkeypatch,
        database="nooto",
        charset="utf8mb4; DROP",
        collation="bad value",
    )

    compose.drop_database()

    assert executed == ["DROP DATABASE IF EXISTS `nooto`; CREATE DATABASE `nooto`"]


def test_drop_database_refuses_system_database(monkeypatch: pytest.MonkeyPatch) -> None:
    compose, executed = make_compose(monkeypatch, database="MySQL")

    with pytest.raises(CliError, match="Refusing to drop system database MySQL."):
        compose.drop_database()

    assert executed == []
