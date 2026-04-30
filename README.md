<div align="center">

<img src="client/public/nooto.svg" alt="Nooto" width="80" />

# Nooto

**Private notes. Yours alone.**

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](./LICENSE)
[![Beta](https://img.shields.io/badge/status-beta-orange)]()

</div>

---

> **Beta software** -- Nooto is functional but still in active development. Expect rough edges.
> **Security notice** -- The encryption design has not been audited by an independent security expert. Use at your own discretion.

---

## Overview

Nooto is a note-taking app that keeps your notes private. Everything is encrypted on your device before it ever leaves it, using strong AES-256-GCM encryption. Nobody but you can read your notes, not even us.

Sync is built in and works out of the box. It is optional and cross-device. If you want to go further, you can run your own server and keep full control over where your data is stored.

**At a glance:**
- Notes are encrypted on your device before sync, no one can read them
- Works fully offline, no account needed
- Sync across your devices using the built-in public server or your own
- Open source and auditable
- Available on Linux, macOS, Windows and Android

### What does the server actually store?

Every note, including its title and content, is encrypted before leaving your device. Here is what our server holds for a given note:

```
uuid:     01938f2a-4b7c-7e1d-a2f3-9c8b1d2e3f4a
content:  8f3a2c1bfe92d4a7c3b1e8f209d4a3c7...  (ciphertext)
metadata: 2d1a8b3c4e5f7a9b2c1d8e3f4a5b6c7d...  (ciphertext)
```

No readable title, no readable content, no plaintext of any kind.

### Encryption

Notes are encrypted with **AES-256-GCM**, which is considered post-quantum resistant. Encryption keys are derived locally from your credentials via **Argon2id** and never leave your device.

---

## Screenshots

> *Screenshots coming soon.*
>
> <!-- Replace with actual screenshots once available -->
> <!-- Suggested captures:
>      1. Full layout: sidebar (workspaces + notes list) + editor with a real note open
>      2. Main editor closeup with markdown rendered (heading, body, bullet list)
>      3. Login / unlock screen -->

---

## Installation

### Pre-built releases

Download the latest installer for your platform from the [Releases](https://github.com/ClemPera/Nooto/releases) page.

| Platform | Format |
|---|---|
| Linux x86_64 | `.deb`, `.rpm`, `.AppImage` |
| Linux aarch64 | `.deb`, `.rpm`, `.AppImage` |
| macOS (universal) | `.dmg` |
| Windows x86_64 | `.msi`, `.exe` |
| Android | `.apk` |

### Build from source

**Prerequisites:**
- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) >= 20
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS

**Desktop:**

```sh
cd client
npm install
npm run tauri build
```

**Android:**

```sh
cd client
npm install
npm run tauri android build -- --apk
```

---

## Sync

Nooto includes a public server already configured in the app. You can start syncing across devices without any setup by creating an account on the welcome screen.

If you prefer to host your own server, see the section below.

### Self-hosting

**1. Configure environment**

```sh
cp .env.example .env
```

Edit `.env` with your own passwords:

```env
MARIADB_ROOT_PASSWORD=a_strong_root_password
MARIADB_DATABASE=nooto
MARIADB_USER=nooto
MARIADB_PASSWORD=a_strong_password

# Port exposed on the host (default: 3000)
SERVER_PORT=3000
```

**2. Start the stack**

Using the pre-built image from Docker Hub:

```sh
docker compose up -d
```

This pulls `clempera8/nooto-server` and starts it alongside a MariaDB instance. Migrations run automatically on startup.

To build the image locally instead:

```sh
docker compose up -d --build
```

Without Docker:

```sh
cargo build --release -p nooto-server
export DATABASE_URL=mysql://nooto:password@localhost:3306/nooto
./target/release/nooto-server
```

**3. Connect the client**

When creating an account or logging in, open **Advanced settings** and enter your server URL.

---

## Project structure

```
Nooto/
├── client/             # Tauri desktop and Android app
│   ├── src/            # React/TypeScript frontend
│   └── src-tauri/      # Rust Tauri backend (local DB, crypto, sync)
├── server/             # Axum HTTP sync server
├── shared/             # Shared Rust types (serialization)
└── docker-compose.yml
```

---

## Contributing

Contributions are welcome! Open an issue before starting significant work so we can align on direction.

If you find a security issue, please **do not open a public issue** -- contact me directly.

---

## License

[AGPL-3.0](./LICENSE)
