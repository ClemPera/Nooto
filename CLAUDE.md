# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Working Branch

**IMPORTANT:** Always work on the `claude-dev` branch when making modifications. Create it if it doesn't exist:
```bash
git checkout claude-dev  # Switch to claude-dev branch
# OR if it doesn't exist:
git checkout -b claude-dev  # Create and switch to claude-dev branch
```

## Project Overview

Notto is an end-to-end encrypted note-taking application built with:
- **Client**: Tauri v2 desktop app (Rust backend + React frontend with TypeScript)
- **Server**: Axum HTTP server for sync
- **Database**: MariaDB (server), SQLite (client)
- **Encryption**: Zero-knowledge architecture with AES-256-GCM and Argon2id

## Build and Development Commands

### Frontend (Tauri Client)
```bash
cd notto-client
npm install                    # Install dependencies
npm run tauri dev              # Run development mode
npm run build                  # Build TypeScript
npm run tauri build            # Build production app
```

### Backend (Sync Server)
```bash
cd notto-server
cargo run                      # Run development server (listens on 0.0.0.0:3000)
cargo build --release          # Build production binary
```

### Database
```bash
cd mariadb
docker-compose up              # Start MariaDB and Adminer (port 8080)
docker-compose down            # Stop services
```

Database connection string must be set in `notto-server/.env` as `DATABASE_URL`.

### Workspace Commands
From project root:
```bash
cargo build                    # Build all workspace members
cargo test                     # Run tests for all crates
```

## Architecture

### Workspace Structure
This is a Cargo workspace with three members:
- `shared/`: Common types and structs shared between client and server
- `notto-client/src-tauri/`: Tauri backend (Rust)
- `notto-server/`: HTTP sync server (Axum)

### Client Architecture (Tauri)

**Main Components:**
- `src-tauri/src/lib.rs`: Application entry point, manages AppState and background sync service
- `src-tauri/src/commands.rs`: Tauri commands exposed to frontend
- `src-tauri/src/db/`: SQLite database operations for local storage
- `src-tauri/src/crypt/`: Encryption/decryption operations (notes, MEK, authentication)
- `src-tauri/src/sync/`: Sync logic and background service

**AppState:**
The application maintains a global state with:
- SQLite connection (wrapped in `Mutex`)
- Current user (with decrypted MEK in memory)

**Background Sync Service:**
A tokio task runs every 1 second (`sync/service.rs`):
1. Receives latest notes from server (updated since last sync)
2. Sends unsynced local notes to server
3. Handles conflicts (currently logs error, needs implementation)
4. Only runs when user is logged in with valid token

**Frontend (React):**
- `src/App.tsx`: Main app component, handles user initialization
- `src/components/Home.tsx`: Note viewing and editing
- `src/components/Sync.tsx`: Sync UI
- `src/components/Login/`: Login and account creation flows
- `src/store/general.tsx`: Zustand store for state management

### Server Architecture (Axum)

**Routes:**
- `POST /create_account`: Create new user account
- `GET /login`: Request login salts
- `POST /login`: Authenticate and get token + encrypted MEK
- `POST /note`: Send notes to server
- `GET /note`: Fetch notes updated since timestamp

**Authentication:**
- Token-based authentication after login
- Tokens stored in `user_token` table
- `user_verify()` function checks token validity

### Encryption Architecture

**Zero-Knowledge Design:**
The server never has access to plaintext data. All encryption/decryption happens client-side.

**Key Hierarchy:**
1. **Master Encryption Key (MEK)**: 256-bit key that encrypts all note content
2. **Password Hash (Auth)**: Derived from password + salt_auth, used for authentication
3. **Password Hash (Data)**: Derived from password + salt_data, encrypts the MEK
4. **Stored Password Hash**: Double-hashed on server with salt_server_auth
5. **Recovery Keys**: 24-word BIP39 mnemonics for account and data recovery

**Encryption Flow:**
- Notes encrypted with MEK using AES-256-GCM
- MEK encrypted with password_hash_data and stored both locally and on server
- Server stores only encrypted MEK, never the plaintext MEK or password
- Each encryption uses a unique nonce

**See `technical_infos.md` for complete security documentation.**

### Shared Types (`shared/src/lib.rs`)

Key structs used across client and server:
- `User`: User account with encrypted MEK and all salts
- `Note`: Note with encrypted content, nonce, timestamps
- `SentNotes`: Batch of notes to sync with token
- `LoginRequest`, `LoginParams`, `Login`: Authentication flow types

## Key Implementation Details

### Tauri Commands
All frontend-backend communication uses Tauri commands (see `commands.rs`):
- Note operations: `create_note`, `get_note`, `edit_note`, `get_all_notes_metadata`
- User operations: `create_user`, `get_users`, `set_user`
- Sync operations: `sync_create_account`, `sync_login`

Commands use `Result<T, CommandError>` for error handling visible to frontend.

### Database Schemas

**Client (SQLite):**
- `notes`: id, id_user, id_server, title, content (encrypted), nonce, updated_at, synched
- `users`: id, username, master_encryption_key, token, instance, encrypted/nonce fields

**Server (MariaDB):**
- `users`: id, username, stored_password_hash, encrypted_mek_password, various salts
- `notes`: id, id_user, title (encrypted), content (encrypted), nonce, updated_at
- `user_token`: id, id_user, token

### Sync Strategy
- **Local-first**: All operations work offline, sync happens in background
- **Timestamp-based**: Compares `updated_at` to determine latest version
- **Conflict detection**: Server returns `NoteStatus::Conflict` when server version is newer
- **Unsynced flag**: `synched` field tracks which notes need uploading

## Development Notes

### Current Limitations (TODOs in code)
- Conflict resolution not implemented (logged as error)
- Deleted notes not handled in sync
- Recovery key not sent to frontend after account creation
- Unused imports warning suppressed in client Cargo.toml
- Test user hardcoded in App.tsx

### Testing
The application uses hardcoded test credentials in development:
- Username: "test_account"
- Default server instance: "http://localhost:3000"

### Security Considerations
- MEK is kept in memory during user session (cleared on logout)
- Server instance URL can be configured (defaults to localhost)
- All network communication should use HTTPS in production
- Argon2id used for all password/key derivation
