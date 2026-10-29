from __future__ import annotations

import pytest

from server_versions import commands
from server_versions.models import Metadata


def test_list_keeps_full_artifact_names(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    db_name = "db-" + "a" * 97
    server_name = "server-" + "b" * 93
    db_meta: Metadata = {
        "schema_version": 1,
        "kind": "db",
        "name": db_name,
        "created_at": "2026-01-02T03:04:05+00:00",
        "checkout": {"branch": "main", "short_commit": "12345678"},
        "db": {
            "database": "nooto",
            "file": f"{db_name}.sql.gz",
            "sha256": "a" * 64,
            "size_bytes": 10,
            "applied_migrations": [1],
            "charset": None,
            "collation": None,
        },
        "running_server": {
            "version_source": "not-running",
            "source_known": False,
        },
    }
    server_meta: Metadata = {
        "schema_version": 1,
        "kind": "server",
        "name": server_name,
        "created_at": "2026-01-02T03:04:05+00:00",
        "source": {"known": True, "branch": "main", "short_commit": "12345678"},
        "image": {"tag": "x", "id": "sha256:" + "b" * 64},
        "server": {"version": "0.4.0", "protocol_version": 3, "migrations": [1]},
    }

    def fake_load_artifacts(kind: str, **_kwargs: object) -> list[Metadata]:
        return [db_meta] if kind == "db" else [server_meta]

    monkeypatch.setattr(commands, "load_artifacts", fake_load_artifacts)

    commands.cmd_list()
    output = capsys.readouterr().out

    assert db_name in output
    assert server_name in output
    assert "…" not in output
