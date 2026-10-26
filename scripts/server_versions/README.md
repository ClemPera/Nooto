# Nooto server versions

Dev-only CLI to snapshot and restore the Nooto server database and Docker images.
It saves DB dumps and server images under `.server-snapshots/`, records where each
artifact came from, and restores a matching pair for testing older or newer code.

The tool never changes Rust or client code. It only drives Docker Compose and writes
the snapshot directory.

## Requirements

- Linux
- Docker and Docker Compose v2 (`docker compose`)
- [uv](https://docs.astral.sh/uv/)
- Access to Docker, either through your user being in the `docker` group or through
  `sudo`. The tool prompts for sudo only when the daemon is not directly accessible.
- `git` in `PATH`

## Setup

From the repository root:

```sh
uv sync --project scripts/server_versions
```

Or from the subproject:

```sh
cd scripts/server_versions
uv sync
```

`uv sync` creates a virtual environment and installs dev tools. The tool itself has
no runtime dependencies outside the Python standard library.

## Running

From the repository root:

```sh
uv run --project scripts/server_versions server-versions --help
uv run --project scripts/server_versions server-versions list
```

From `scripts/server_versions`:

```sh
uv run server-versions list
```

The compatibility launcher still works and only adds the package to `sys.path`:

```sh
python3 scripts/server-versions.py --help
```

It requires Python 3.11 or newer. If the package is installed non-editable, set
`NOOTO_ROOT` to the repository so the tool can find the compose project.

Running without a subcommand opens the interactive menu. The menu offers save,
restore, list, show, delete, and quit.

## Commands

### `save-db`

Dump the current database.

```sh
uv run server-versions save-db
uv run server-versions save-db --name before-upgrade
uv run server-versions save-db --no-running-image
uv run server-versions save-db --with-running-image --yes
```

Flags:

- `--name NAME` use an explicit name instead of the generated one
- `--with-running-image` also tag the exact running server image
- `--no-running-image` do not capture the running image
- `--yes` accept prompts and run non-interactively

When the running image source is unknown, the tool asks whether to save it with the
dump. The default is yes. In non-interactive mode (`--yes` or no TTY) it saves the
image, matching the prompt default.

### `save-server`

Build an image from the current checkout and tag it as a snapshot.

```sh
uv run server-versions save-server
uv run server-versions save-server --name feature-x
uv run server-versions save-server --allow-dirty
uv run server-versions save-server --running
```

Flags:

- `--name NAME` use an explicit name
- `--running` tag the running image without rebuilding
- `--allow-dirty` allow uncommitted changes under `server/` or `shared/`
- `--yes` accept prompts

A dirty build is allowed only with `--allow-dirty` or after confirming the warning.
The generated artifact name gets a `-dirty` suffix; an explicit `--name` is used
unchanged.

### `save-both`

Build, restart the server with the new image, and dump the database in one step.
This is the usual way to create a matching pair.

```sh
uv run server-versions save-both
uv run server-versions save-both --name release-check --allow-dirty
```

Flags: `--name`, `--allow-dirty`, `--yes`.

The command creates a safety backup of the current DB just before restarting the
server. If anything fails after the build or the restart, it rolls back the DB and
image, then removes the new artifacts it created. The safety backup is kept.

### `restore` and `switch`

Restore a DB dump, switch the server image, or both. `switch` is an alias.

```sh
uv run server-versions restore --db before-upgrade
uv run server-versions restore --db before-upgrade --server current
uv run server-versions switch --server feature-x --no-start
uv run server-versions restore --db before-upgrade --yes --skip-backup
```

Flags:

- `--db NAME` DB artifact name, or `current`/`keep`/`-` to keep the current DB
- `--server NAME` server artifact name, or `current`/`keep`/`-` to keep the image
- `--no-start` leave the server stopped after restoring
- `--yes` accept prompts
- `--skip-backup` do not take a safety DB backup
- `--no-rollback` do not roll back on failure

The restore flow:

1. Resolve and verify the selected artifacts. DB dumps are checked against their
   SHA-256 checksum and server images are checked by tag and image ID.
2. Print a restore plan with schema compatibility and checkout mismatch warnings.
3. Prompt for confirmation unless `--yes`.
4. Take a safety DB backup unless `--skip-backup`.
5. Stop the server, restore the DB, switch the image, and start the server.
6. Verify the restored schema and the running image. Warn when `/version` differs
   from the saved metadata.
7. On failure, roll back to the safety backup and previous image, then re-raise.

### `list`

List saved DB dumps and server images.

```sh
uv run server-versions list
uv run server-versions list --all
uv run server-versions list --json
```

`--all` includes automatic safety backups. `--json` prints the raw metadata. Rows
are ordered newest first.

### `show`

Show the metadata for a DB dump, server image, or both with the same name.

```sh
uv run server-versions show before-upgrade
uv run server-versions show before-upgrade --json
```

### `delete`

Delete a DB dump, server image, or both.

```sh
uv run server-versions delete before-upgrade
uv run server-versions delete before-upgrade --kind db --yes
uv run server-versions delete before-upgrade --kind server
```

Server image deletion is conservative:

- If the snapshot tag exists and points at the artifact image, only that tag is
  removed.
- If the image is untagged, the image ID is removed.
- If the image still has other tags, the command refuses and lists them. Remove
  those tags first.
- If the image is already gone, the metadata is removed with a warning.

## Artifact layout

```
.server-snapshots/
├── .lock
├── db/
│   ├── <name>.json
│   ├── <name>.sql.gz
│   └── _auto/
│       ├── <reason>-<timestamp>.json
│       └── <reason>-<timestamp>.sql.gz
└── server/
    └── <name>.json
```

DB dumps are gzip-compressed SQL. Metadata is JSON with `schema_version: 1`.
Server images are Docker images tagged
`<compose-project>-server-snapshot:<name>`.

Each metadata file records:

- `schema_version`, `kind`, `name`, `created_at`
- the git checkout: branch, commit, short commit, subject, dirty flags
- for DB artifacts: database name, dump filename, SHA-256, size, applied
  migrations, charset, collation, and the running server information
- for server artifacts: image tag, image ID, image creation time, compose image,
  source information, server version, protocol version, and source migrations

The `dirty` flag only covers `server/` and `shared/`, the directories copied into
the server image. `repo_dirty` records the wider working tree state. A clean
checkout can still run an image built from another branch; that is recorded as the
running image source.

## Automatic backups and retention

`save-both` and `restore` create automatic DB backups under `db/_auto/` before
changing state. Names look like `pre-restore-20260101-120000`.

The tool keeps the newest 5 automatic backups and prunes older ones. Automatic
backups are hidden from `list` unless `--all` is passed. They are internal rollback
snapshots: `restore`, `show`, and `delete` do not accept them. They are never deleted
by `delete` or by partial-artifact cleanup.

## Safety behavior

- A lock file prevents two mutating runs at the same time.
- DB dumps are verified as valid gzip before being published, then checksummed.
- Dump files are written atomically and forced to mode `600`.
- Metadata is written atomically through a temporary file.
- Restores verify checksums before touching the database.
- Restores verify the database name and refuse system databases.
- A safety backup is taken before restores and before restarting the server in `save-both`.
- Failed restores roll back the DB and the previous image when possible.
- `save-db` removes the new dump and any captured image tag when metadata writing fails.
- `save-server` and `save-both` remove the new snapshot tag and metadata when saving
  the image metadata fails. `save-both` also removes the new DB artifact it created.
- Dirty checkouts are rejected for builds unless explicitly allowed.

## Common workflows

Save a DB before trying a migration:

```sh
uv run server-versions save-db --name before-migration-42
```

Create a matching server and DB pair for a branch:

```sh
uv run server-versions save-both --name feature-check
```

Go back to an older pair:

```sh
uv run server-versions restore --db feature-check --server feature-check
```

Switch only the image, keep the DB:

```sh
uv run server-versions switch --server old-build --db current
```

Save the currently running image without rebuilding:

```sh
uv run server-versions save-server --running
```

## Troubleshooting

### Docker permission denied

Add your user to the `docker` group and start a new login session:

```sh
sudo usermod -aG docker "$USER"
```

Alternatively run from a terminal so the tool can use sudo. Non-interactive runs
cannot answer a sudo prompt.

### sudo session expired

The tool refreshes sudo before Docker operations. If the session expired in a
non-interactive run, rerun from a terminal or authenticate sudo first.

### Port already in use

The tool reads the published port from `docker compose port server 3000`. If the
port is occupied, stop the other process or change the compose port mapping.

### Missing server image

Restore checks the snapshot tag first, then the image ID. If both are gone, the
metadata cannot be used. Save the image again or delete the stale metadata.

### Checksum mismatch

The dump file changed after it was saved. Restore refuses to use it. Save a new
dump. Do not edit files under `.server-snapshots/db/`.

### Dirty tree warning

Commit the changes under `server/` or `shared/`, or rerun with `--allow-dirty`.
Dirty artifacts are marked and produce a warning when restored.

### Save-both failed

The tool prints the original failure, rolls back the running state, and removes
the new artifacts. Check the log for a rollback failure. The safety backup name is
printed just before the server restart.

## Development checks

From `scripts/server_versions`:

```sh
uv sync
uv run ruff format --check .
uv run ruff check .
uv run mypy
uv run basedpyright
uv run pytest
```

From the repository root, use `uv run --directory scripts/server_versions ...` for
the same checks.

Pytest uses temporary snapshot directories. Tests never touch the real
`.server-snapshots/` directory. Mutating Docker commands are not run by the tests.

For Pylance, open `scripts/server_versions` as the workspace folder, or keep the
root `.vscode` settings and set `python.analysis.useNearestConfiguration`. Select
`scripts/server_versions/.venv/bin/python` as the interpreter.

## Layout

```
scripts/server_versions/
├── pyproject.toml
├── uv.lock
├── .python-version
├── README.md
├── pyrightconfig.json
├── src/server_versions/
│   ├── cli.py, commands.py
│   ├── config.py, metadata.py, models.py
│   ├── docker/
│   ├── snapshots/
│   └── restore/
└── tests/
```

`scripts/server-versions.py` is the thin compatibility launcher. New work should
use the uv entry point.
