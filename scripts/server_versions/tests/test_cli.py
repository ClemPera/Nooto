from __future__ import annotations

import pytest

from server_versions import cli
from server_versions.cli import build_parser


def test_no_command() -> None:
    args = build_parser().parse_args([])
    assert args.command is None


def test_save_db_defaults() -> None:
    args = build_parser().parse_args(["save-db"])
    assert args.command == "save-db"
    assert args.name is None
    assert args.with_running_image is None
    assert args.yes is False


def test_save_db_running_image_flags() -> None:
    parser = build_parser()
    with_image = parser.parse_args(["save-db", "--with-running-image", "--name", "x"])
    assert with_image.with_running_image is True
    assert with_image.name == "x"
    without_image = parser.parse_args(["save-db", "--no-running-image", "--yes"])
    assert without_image.with_running_image is False
    assert without_image.yes is True


def test_save_server_flags() -> None:
    args = build_parser().parse_args(
        ["save-server", "--name", "image", "--running", "--allow-dirty", "--yes"]
    )
    assert args.command == "save-server"
    assert args.name == "image"
    assert args.running is True
    assert args.allow_dirty is True
    assert args.yes is True


def test_save_both_flags() -> None:
    args = build_parser().parse_args(["save-both", "--name", "pair", "--allow-dirty"])
    assert args.command == "save-both"
    assert args.name == "pair"
    assert args.allow_dirty is True
    assert args.yes is False


def test_restore_flags() -> None:
    args = build_parser().parse_args(
        [
            "restore",
            "--db",
            "x",
            "--server",
            "y",
            "--no-start",
            "--yes",
            "--skip-backup",
            "--no-rollback",
        ]
    )
    assert args.command == "restore"
    assert args.db == "x"
    assert args.server == "y"
    assert args.no_start is True
    assert args.yes is True
    assert args.skip_backup is True
    assert args.no_rollback is True


def test_switch_alias() -> None:
    args = build_parser().parse_args(["switch", "--db", "x"])
    assert args.command == "switch"
    assert args.db == "x"
    assert args.server is None


def test_list_flags() -> None:
    args = build_parser().parse_args(["list", "--all", "--json"])
    assert args.command == "list"
    assert args.all is True
    assert args.json is True


def test_drop_db_defaults() -> None:
    args = build_parser().parse_args(["drop-db"])
    assert args.command == "drop-db"
    assert args.yes is False
    assert args.skip_backup is False
    assert args.no_start is False


def test_drop_db_flags() -> None:
    args = build_parser().parse_args(["drop-db", "--yes", "--skip-backup", "--no-start"])
    assert args.command == "drop-db"
    assert args.yes is True
    assert args.skip_backup is True
    assert args.no_start is True


def test_show_and_delete() -> None:
    parser = build_parser()
    show = parser.parse_args(["show", "dump", "--json"])
    assert show.command == "show"
    assert show.name == "dump"
    assert show.json is True
    delete = parser.parse_args(["delete", "dump", "--kind", "server", "--yes"])
    assert delete.command == "delete"
    assert delete.name == "dump"
    assert delete.kind == "server"
    assert delete.yes is True


def test_restore_from_spec_example() -> None:
    args = build_parser().parse_args(["restore", "--db", "x"])
    assert args.db == "x"
    assert args.command == "restore"


class FakeLock:
    def close(self) -> None:
        pass


def test_broken_pipe_on_list_returns_zero(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def raise_broken(**_kwargs: object) -> None:
        raise BrokenPipeError

    monkeypatch.setattr(cli, "cmd_list", raise_broken)
    monkeypatch.setattr(cli.os, "geteuid", lambda: 1000)

    assert cli.main(["list"]) == 0
    assert capsys.readouterr().err == ""


def test_broken_pipe_on_save_db_returns_one(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def raise_broken(*_args: object, **_kwargs: object) -> None:
        raise BrokenPipeError

    monkeypatch.setattr(cli.os, "geteuid", lambda: 1000)
    monkeypatch.setattr(cli, "acquire_lock", lambda: FakeLock())
    monkeypatch.setattr(cli, "Docker", lambda: object())
    monkeypatch.setattr(cli, "save_db", raise_broken)

    assert cli.main(["save-db"]) == 1
    assert "may not have completed" in capsys.readouterr().err
