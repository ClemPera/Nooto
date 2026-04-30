<div align="center">

<img src="client/public/nooto.svg" alt="Nooto" width="80" />

# Nooto

**Private notes. Yours alone.**

A cross-platform, end-to-end encrypted note-taking app with optional self-hosted sync.  
Your notes are encrypted on your device before they ever leave it.

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](./LICENSE)
[![Beta](https://img.shields.io/badge/status-beta-orange)]()

</div>

---

> **⚠️ Beta software** — Nooto is functional but still in active development. Expect rough edges.  
> **🔐 Security notice** — The encryption design has not been audited by an independent security expert. Use at your own discretion.

---

## Overview

Nooto is a local-first note-taking application. Notes are stored on your device and encrypted with AES-256-GCM using a key that never leaves your machine. Sync is optional and self-hostable — when enabled, only encrypted blobs reach the server.

**Key properties:**
- End-to-end encrypted — the server only ever sees ciphertext
- Local-first — fully usable offline, no account required
- Self-hosted sync — run your own server, keep full control of your data
- Cross-platform — desktop (Linux, macOS, Windows) and Android

## Screenshots

> *Screenshots coming soon.*
>
> <!-- Replace with actual screenshots once available -->
> <!-- Suggested captures:
>      1. Main editor with a note open (markdown rendered)
>      2. Sidebar showing workspaces + notes list
>      3. Login / unlock screen -->

## How it works

At registration, a random **Master Encryption Key (MEK)** is generated on your device. It is encrypted with your password via Argon2id and AES-256-GCM before being sent to the server — the plaintext MEK never leaves your device. All note content and metadata are encrypted locally with this MEK before sync.

See [`technical_infos.md`](./technical_infos.md) for the full cryptographic design.

---

## Installation

### Desktop (pre-built binaries)

Download the latest installer for your platform from the [Releases](https://github.com/ClemPera/Nooto/releases) page.

| Platform | Format |
|---|---|
| Linux x86_64 | `.deb`, `.rpm`, `.AppImage` |
| Linux aarch64 | `.deb`, `.rpm`, `.AppImage` |
| macOS (universal) | `.dmg` |
| Windows x86_64 | `.msi`, `.exe` |
| Android | `.apk` |

### Build from source

#### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) ≥ 20
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS

#### Client (desktop)

```sh
cd client
npm install
npm run tauri build
```

The built installer and binary will be in `target/release/bundle/`.

#### Client (Android)

```sh
cd client
npm install
npm run tauri android build -- --apk
```

Requires Android SDK and NDK. See the [Tauri Android guide](https://v2.tauri.app/start/prerequisites/#android) for setup.

---

## Running the sync server

The sync server is optional. Without it, Nooto works fully offline as a local note-taking app.

### With Docker (recommended)

**1. Configure environment**

```sh
cp .env.example .env
```

Edit `.env` and set secure passwords:

```env
MARIADB_ROOT_PASSWORD=a_strong_root_password
MARIADB_DATABASE=nooto
MARIADB_USER=nooto
MARIADB_PASSWORD=a_strong_password

# Port exposed on the host (default: 3000)
SERVER_PORT=3000
```

**2. Start the stack**

```sh
docker compose up -d
```

This starts a MariaDB instance and the `nooto-server`. Database migrations run automatically on startup.

**3. Connect the client**

In Nooto, go to **Settings → Sync** and enter your server URL (e.g. `http://your-server:3000`).

### Build the server manually

```sh
cargo build --release -p nooto-server
```

The server reads `DATABASE_URL` from the environment:

```sh
export DATABASE_URL=mysql://nooto:password@localhost:3306/nooto
./target/release/nooto-server
```

### Docker Hub

A pre-built server image is available:

```sh
docker pull clempera8/nooto-server
```

---

## Development

### Run the client in dev mode

```sh
cd client
npm install
npm run tauri dev
```

### Run the server in dev mode

```sh
export DATABASE_URL=mysql://nooto:password@localhost:3306/nooto
cargo run -p nooto-server
```

### Run frontend tests

```sh
cd client
npm test
```

### Run server tests

```sh
cargo test -p nooto-server
```

---

## Project structure

```
Nooto/
├── client/             # Tauri desktop & Android app
│   ├── src/            # React/TypeScript frontend
│   └── src-tauri/      # Rust Tauri backend (local DB, crypto, sync)
├── server/             # Axum HTTP sync server
├── shared/             # Shared Rust types (serialization)
└── docker-compose.yml
```

---

## Contributing

Contributions are welcome! Open an issue before starting significant work so we can align on direction.

If you find a security issue, please **do not open a public issue** — contact me directly.

---

## License

[AGPL-3.0](./LICENSE) — self-hosting is and will always remain free.
