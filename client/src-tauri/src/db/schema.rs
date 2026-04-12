use aes_gcm::{Aes256Gcm, Key};
use anyhow::{Context, Result};
use rusqlite::Connection;

use rusqlite::Error::QueryReturnedNoRows;

/// Local SQLite note row (content and metadata are ciphertext blobs).
#[derive(Debug)]
pub struct Note {
    pub uuid: String,
    pub id_workspace: Option<u32>,
    pub content: Vec<u8>,
    pub nonce: Vec<u8>,
    pub metadata: Vec<u8>,
    pub metadata_nonce: Vec<u8>,
    pub updated_at: i64,
    pub synched: bool,
    pub deleted: bool,
}

impl From<shared::Note> for Note {
    fn from(note: shared::Note) -> Self {
        Note {
            uuid: note.uuid,
            id_workspace: None,
            content: note.content,
            nonce: note.nonce,
            metadata: note.metadata,
            metadata_nonce: note.metadata_nonce,
            updated_at: note.updated_at,
            synched: true,
            deleted: note.deleted,
        }
    }
}

impl Into<shared::Note> for Note {
    fn into(self) -> shared::Note {
        shared::Note {
            uuid: self.uuid,
            content: self.content,
            nonce: self.nonce,
            metadata: self.metadata,
            metadata_nonce: self.metadata_nonce,
            updated_at: self.updated_at,
            deleted: self.deleted,
        }
    }
}

impl Note {
    /// Creates the `note` table if it does not already exist.
    pub fn create(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS note (
                uuid BLOB PRIMARY KEY,
                id_workspace INTEGER NOT NULL REFERENCES workspace(id),
                content BLOB,
                nonce BLOB,
                metadata BLOB,
                metadata_nonce BLOB,
                updated_at INTEGER,
                synched INTEGER NOT NULL,
                deleted INTEGER NOT NULL
            )",
            (),
        )
        .context("Failed to create note table")?;

        Ok(())
    }

    /// Fetches a note by UUID. Returns `None` if not found.
    pub fn select(conn: &Connection, uuid: String) -> Result<Option<Self>> {
        let note = match conn.query_one(
            "SELECT * FROM note WHERE uuid = ?",
            (uuid,),
            |row| {
                Ok(Note {
                    uuid: row.get(0)?,
                    id_workspace: row.get(1)?,
                    content: row.get(2)?,
                    nonce: row.get(3)?,
                    metadata: row.get(4)?,
                    metadata_nonce: row.get(5)?,
                    updated_at: row.get(6)?,
                    synched: row.get(7)?,
                    deleted: row.get(8)?,
                })
            },
        ) {
            Ok(note) => Some(note),
            Err(_) => None,
        };

        Ok(note)
    }

    /// Inserts this note into the database.
    pub fn insert(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO note (uuid, content, nonce, metadata, metadata_nonce, id_workspace, updated_at, synched, deleted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (&self.uuid, &self.content, &self.nonce, &self.metadata, &self.metadata_nonce, &self.id_workspace, &self.updated_at, &self.synched, &self.deleted),
        )
        .context("Failed to insert note")?;

        Ok(())
    }

    /// Updates the note's encrypted content, metadata, timestamps, and sync flag.
    pub fn update(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "UPDATE note SET content = ?, nonce = ?, metadata = ?, metadata_nonce = ?, updated_at = ?, synched = ?, deleted = ? WHERE uuid = ?",
            (&self.content, &self.nonce, &self.metadata, &self.metadata_nonce, &self.updated_at, &self.synched, &self.deleted, &self.uuid),
        )
        .context("Failed to update note")?;

        Ok(())
    }

    /// Returns all notes belonging to `id_workspace`.
    pub fn select_all(conn: &Connection, id_workspace: u32) -> Result<Vec<Self>> {
        let mut stmt = conn
            .prepare("SELECT * FROM note WHERE id_workspace = ?")
            .context("Failed to prepare note query")?;

        let notes = stmt
            .query_map([id_workspace], |row| {
                Ok(Note {
                    uuid: row.get(0)?,
                    id_workspace: row.get(1)?,
                    content: row.get(2)?,
                    nonce: row.get(3)?,
                    metadata: row.get(4)?,
                    metadata_nonce: row.get(5)?,
                    updated_at: row.get(6)?,
                    synched: row.get(7)?,
                    deleted: row.get(8)?,
                })
            })
            .context("Failed to query notes")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to read note rows")?;

        Ok(notes)
    }

    /// Deletes all notes belonging to `id_workspace` (used on logout).
    pub fn delete_all_from_workspace(conn: &Connection, id_workspace: u32) -> Result<()> {
        conn.execute(
            "DELETE FROM note WHERE id_workspace = ?",
            (id_workspace,),
        )
        .context("Failed to delete notes from workspace")?;

        Ok(())
    }
}

/// Local workspace row — holds the MEK in plaintext and optional server credentials.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: u32,
    pub workspace_name: String,
    pub username: Option<String>,

    //TODO: Do not store that in plain text but give the user the possibility to use biometric to decrypt?
    pub master_encryption_key: Key<Aes256Gcm>,

    pub salt_recovery_data: String,
    pub mek_recovery_nonce: Vec<u8>,
    pub encrypted_mek_recovery: Vec<u8>,
    pub token: Option<Vec<u8>>,
    pub instance: Option<String>,
    pub last_sync_at: i64,
    pub latest_note_id: Option<String>,
}

impl Workspace {
    /// Creates the `workspace` table if it does not already exist.
    pub fn create(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS workspace (
                id INTEGER PRIMARY KEY,
                workspace_name TEXT,
                username TEXT,
                master_encryption_key BLOB,
                salt_recovery_data TEXT,
                mek_recovery_nonce BLOB,
                encrypted_mek_recovery BLOB,
                token TEXT,
                instance TEXT,
                last_sync_at INTEGER NOT NULL DEFAULT -9223372036854775808,
                latest_note_id TEXT
            )",
            (),
        )
        .context("Failed to create workspace table")?;

        Ok(())
    }

    /// Inserts this workspace into the database.
    pub fn insert(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO workspace (workspace_name, username, master_encryption_key, salt_recovery_data, mek_recovery_nonce, encrypted_mek_recovery, token, instance, last_sync_at, latest_note_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            (&self.workspace_name, &self.username, &self.master_encryption_key.to_vec(), &self.salt_recovery_data, &self.mek_recovery_nonce, &self.encrypted_mek_recovery, &self.token, &self.instance, &self.last_sync_at, &self.latest_note_id),
        )
        .context("Failed to insert workspace")?;

        Ok(())
    }

    /// Fetches a workspace by name. Returns `None` if not found.
    pub fn select(conn: &Connection, workspace_name: String) -> Result<Option<Self>> {
        let workspace = match conn.query_one(
            "SELECT * FROM workspace WHERE workspace_name = ?",
            (workspace_name,),
            |row| {
                let mek_bytes: Vec<u8> = row.get(3)?;
                let mek: [u8; 32] = mek_bytes.try_into().map_err(|_| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "MEK must be exactly 32 bytes",
                        )),
                    )
                })?;
                let mek: Key<Aes256Gcm> = mek.into();

                Ok(Workspace {
                    id: row.get(0)?,
                    workspace_name: row.get(1)?,
                    username: row.get(2)?,
                    master_encryption_key: mek,
                    salt_recovery_data: row.get(4)?,
                    mek_recovery_nonce: row.get(5)?,
                    encrypted_mek_recovery: row.get(6)?,
                    token: row.get(7)?,
                    instance: row.get(8)?,
                    last_sync_at: row.get(9)?,
                    latest_note_id: row.get(10)?,
                })
            },
        ) {
            Ok(v) => Some(v),
            Err(e) if e == QueryReturnedNoRows => None,
            Err(e) => return Err(e).context("Failed to select workspace"),
        };

        Ok(workspace)
    }

    /// Returns all workspaces stored locally.
    pub fn select_all(conn: &Connection) -> Result<Vec<Self>> {
        let mut stmt = conn
            .prepare("SELECT * FROM workspace")
            .context("Failed to prepare workspace query")?;

        let workspaces = stmt
            .query_map([], |row| {
                let mek_bytes: Vec<u8> = row.get(3)?;
                let mek: [u8; 32] = mek_bytes.try_into().map_err(|_| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Blob,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "MEK must be exactly 32 bytes",
                        )),
                    )
                })?;
                let mek: Key<Aes256Gcm> = mek.into();

                Ok(Workspace {
                    id: row.get(0)?,
                    workspace_name: row.get(1)?,
                    username: row.get(2)?,
                    master_encryption_key: mek,
                    salt_recovery_data: row.get(4)?,
                    mek_recovery_nonce: row.get(5)?,
                    encrypted_mek_recovery: row.get(6)?,
                    token: row.get(7)?,
                    instance: row.get(8)?,
                    last_sync_at: row.get(9)?,
                    latest_note_id: row.get(10)?,
                })
            })
            .context("Failed to query workspaces")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to read workspace rows")?;

        Ok(workspaces)
    }

    /// Persists all fields of this workspace back to the database.
    pub fn update(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "UPDATE workspace SET workspace_name = ?, username = ?, master_encryption_key = ?, salt_recovery_data = ?, mek_recovery_nonce = ?, encrypted_mek_recovery = ?, token = ?, instance = ?, last_sync_at = ?, latest_note_id = ? WHERE id = ?",
            (&self.workspace_name, &self.username, &self.master_encryption_key.to_vec(), &self.salt_recovery_data, &self.mek_recovery_nonce, &self.encrypted_mek_recovery, &self.token, &self.instance, &self.last_sync_at, &self.latest_note_id, &self.id),
        )
        .context("Failed to update workspace")?;

        Ok(())
    }

    /// Updates only the `latest_note_id` column for the given workspace.
    pub fn update_latest_note(conn: &Connection, workspace_id: u32, uuid: Option<&str>) -> Result<()> {
        conn.execute(
            "UPDATE workspace SET latest_note_id = ? WHERE id = ?",
            (uuid, workspace_id),
        )
        .context("Failed to update latest note on workspace")?;

        Ok(())
    }

    /// Deletes this workspace row from the database.
    pub fn delete(&self, conn: &Connection) -> Result<()> {
        conn.execute("DELETE FROM workspace WHERE id = ?", (&self.id,))
            .context("Failed to delete workspace")?;

        Ok(())
    }
}

/// Generic key-value store table for app-level settings (e.g. the logged workspace name).
#[derive(Debug, Clone)]
pub struct Common {
    pub key: String,
    pub value: String,
}

impl Common {
    /// Creates the `common` table if it does not already exist.
    pub fn create(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS common (
                key TEXT PRIMARY KEY,
                value TEXT
            )",
            (),
        )
        .context("Failed to create common table")?;

        Ok(())
    }

    /// Inserts a new key-value entry.
    pub fn insert(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO common (key, value) VALUES (?1, ?2)",
            (&self.key, &self.value),
        )
        .context("Failed to insert common entry")?;

        Ok(())
    }

    /// Updates the value for an existing key.
    pub fn update(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "UPDATE common SET value = ? WHERE key = ?",
            (&self.value, &self.key),
        )
        .context("Failed to update common entry")?;

        Ok(())
    }

    /// Fetches an entry by key. Returns `None` if the key doesn't exist.
    pub fn select(conn: &Connection, key: String) -> Result<Option<Self>> {
        let value = match conn.query_one(
            "SELECT value FROM common WHERE key = ?",
            (key.clone(),),
            |row| {
                Ok(Common {
                    key: key.clone(),
                    value: row.get(0)?,
                })
            },
        ) {
            Ok(v) => Some(v),
            Err(e) if e == QueryReturnedNoRows => None,
            Err(e) => return Err(e).context("Failed to select common entry"),
        };

        Ok(value)
    }

    /// Deletes an entry by key (no-op if the key doesn't exist).
    pub fn delete(conn: &Connection, key: String) -> Result<()> {
        conn.execute("DELETE FROM common WHERE key = ?", (key,))
            .context("Failed to delete common entry")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{Aes256Gcm, KeyInit};
    use argon2::password_hash::rand_core::OsRng;

    fn open_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        Note::create(&conn).unwrap();
        Workspace::create(&conn).unwrap();
        Common::create(&conn).unwrap();
        conn
    }

    fn random_key() -> Key<Aes256Gcm> {
        Aes256Gcm::generate_key(OsRng)
    }

    fn sample_workspace(name: &str) -> Workspace {
        Workspace {
            id: 0,
            workspace_name: name.to_string(),
            username: None,
            master_encryption_key: random_key(),
            salt_recovery_data: "salt".to_string(),
            mek_recovery_nonce: vec![1, 2, 3],
            encrypted_mek_recovery: vec![4, 5, 6],
            token: None,
            instance: None,
            last_sync_at: 0,
            latest_note_id: None,
        }
    }

    fn sample_note(workspace_id: u32) -> Note {
        Note {
            uuid: "test-uuid-001".to_string(),
            id_workspace: Some(workspace_id),
            content: vec![1, 2, 3],
            nonce: vec![4, 5, 6],
            metadata: vec![7, 8, 9],
            metadata_nonce: vec![10, 11, 12],
            updated_at: 1700000000,
            synched: false,
            deleted: false,
        }
    }

    // --- Note conversions ---

    #[test]
    fn note_from_shared_sets_synched_true() {
        let shared = shared::Note {
            uuid: "uuid".to_string(),
            content: vec![],
            nonce: vec![],
            metadata: vec![],
            metadata_nonce: vec![],
            updated_at: 0,
            deleted: false,
        };
        let note = Note::from(shared);
        assert!(note.synched);
        assert!(note.id_workspace.is_none());
    }

    #[test]
    fn note_into_shared_drops_local_fields() {
        let note = sample_note(1);
        let shared: shared::Note = note.into();
        assert_eq!(shared.uuid, "test-uuid-001");
        assert_eq!(shared.content, vec![1, 2, 3]);
    }

    #[test]
    fn note_roundtrip_from_shared_and_back() {
        let original = shared::Note {
            uuid: "roundtrip-uuid".to_string(),
            content: vec![9, 8, 7],
            nonce: vec![6, 5, 4],
            metadata: vec![3, 2, 1],
            metadata_nonce: vec![0],
            updated_at: 42,
            deleted: true,
        };
        let local = Note::from(original.clone());
        let back: shared::Note = local.into();

        assert_eq!(back.uuid, original.uuid);
        assert_eq!(back.content, original.content);
        assert_eq!(back.nonce, original.nonce);
        assert_eq!(back.metadata, original.metadata);
        assert_eq!(back.metadata_nonce, original.metadata_nonce);
        assert_eq!(back.updated_at, original.updated_at);
        assert_eq!(back.deleted, original.deleted);
    }

    // --- Note DB operations ---

    #[test]
    fn note_insert_and_select() {
        let conn = open_db();
        let ws = sample_workspace("ws1");
        ws.insert(&conn).unwrap();
        let ws_id = conn.last_insert_rowid() as u32;

        let note = sample_note(ws_id);
        note.insert(&conn).unwrap();

        let fetched = Note::select(&conn, note.uuid.clone()).unwrap().unwrap();
        assert_eq!(fetched.uuid, note.uuid);
        assert_eq!(fetched.content, note.content);
        assert_eq!(fetched.nonce, note.nonce);
        assert_eq!(fetched.updated_at, note.updated_at);
        assert_eq!(fetched.deleted, note.deleted);
        assert!(!fetched.synched);
    }

    #[test]
    fn note_select_missing_returns_none() {
        let conn = open_db();
        let result = Note::select(&conn, "nonexistent".to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn note_update() {
        let conn = open_db();
        let ws = sample_workspace("ws1");
        ws.insert(&conn).unwrap();
        let ws_id = conn.last_insert_rowid() as u32;

        let mut note = sample_note(ws_id);
        note.insert(&conn).unwrap();

        note.content = vec![99, 88, 77];
        note.synched = true;
        note.update(&conn).unwrap();

        let fetched = Note::select(&conn, note.uuid.clone()).unwrap().unwrap();
        assert_eq!(fetched.content, vec![99, 88, 77]);
        assert!(fetched.synched);
    }

    #[test]
    fn note_select_all_filters_by_workspace() {
        let conn = open_db();
        let ws1 = sample_workspace("ws1");
        ws1.insert(&conn).unwrap();
        let ws1_id = conn.last_insert_rowid() as u32;

        let ws2 = sample_workspace("ws2");
        ws2.insert(&conn).unwrap();
        let ws2_id = conn.last_insert_rowid() as u32;

        let mut note1 = sample_note(ws1_id);
        note1.uuid = "uuid-1".to_string();
        note1.insert(&conn).unwrap();

        let mut note2 = sample_note(ws2_id);
        note2.uuid = "uuid-2".to_string();
        note2.insert(&conn).unwrap();

        let ws1_notes = Note::select_all(&conn, ws1_id).unwrap();
        assert_eq!(ws1_notes.len(), 1);
        assert_eq!(ws1_notes[0].uuid, "uuid-1");
    }

    #[test]
    fn note_delete_all_from_workspace() {
        let conn = open_db();
        let ws = sample_workspace("ws1");
        ws.insert(&conn).unwrap();
        let ws_id = conn.last_insert_rowid() as u32;

        let mut note1 = sample_note(ws_id);
        note1.uuid = "uuid-a".to_string();
        note1.insert(&conn).unwrap();

        let mut note2 = sample_note(ws_id);
        note2.uuid = "uuid-b".to_string();
        note2.insert(&conn).unwrap();

        Note::delete_all_from_workspace(&conn, ws_id).unwrap();

        assert!(Note::select_all(&conn, ws_id).unwrap().is_empty());
    }

    // --- Workspace DB operations ---

    #[test]
    fn workspace_insert_and_select() {
        let conn = open_db();
        let ws = sample_workspace("my_workspace");
        ws.insert(&conn).unwrap();

        let fetched = Workspace::select(&conn, "my_workspace".to_string())
            .unwrap()
            .unwrap();

        assert_eq!(fetched.workspace_name, "my_workspace");
        assert!(fetched.username.is_none());
        assert_eq!(fetched.last_sync_at, 0);
    }

    #[test]
    fn workspace_select_missing_returns_none() {
        let conn = open_db();
        let result = Workspace::select(&conn, "ghost".to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn workspace_update() {
        let conn = open_db();
        let ws = sample_workspace("ws");
        ws.insert(&conn).unwrap();
        let id = conn.last_insert_rowid() as u32;

        let mut ws = Workspace::select(&conn, "ws".to_string()).unwrap().unwrap();
        ws.username = Some("alice".to_string());
        ws.last_sync_at = 1234567890;
        ws.update(&conn).unwrap();

        let fetched = Workspace::select(&conn, "ws".to_string()).unwrap().unwrap();
        assert_eq!(fetched.username, Some("alice".to_string()));
        assert_eq!(fetched.last_sync_at, 1234567890);

        let _ = id;
    }

    #[test]
    fn workspace_update_latest_note() {
        let conn = open_db();
        let ws = sample_workspace("ws");
        ws.insert(&conn).unwrap();
        let id = conn.last_insert_rowid() as u32;

        Workspace::update_latest_note(&conn, id, Some("some-note-uuid")).unwrap();

        let fetched = Workspace::select(&conn, "ws".to_string()).unwrap().unwrap();
        assert_eq!(fetched.latest_note_id, Some("some-note-uuid".to_string()));
    }

    #[test]
    fn workspace_delete() {
        let conn = open_db();
        let ws = sample_workspace("ws_to_delete");
        ws.insert(&conn).unwrap();
        let id = conn.last_insert_rowid() as u32;

        let ws = Workspace { id, ..sample_workspace("ws_to_delete") };
        ws.delete(&conn).unwrap();

        let result = Workspace::select(&conn, "ws_to_delete".to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn workspace_select_all() {
        let conn = open_db();
        sample_workspace("ws1").insert(&conn).unwrap();
        sample_workspace("ws2").insert(&conn).unwrap();

        let all = Workspace::select_all(&conn).unwrap();
        assert_eq!(all.len(), 2);
    }

    // --- Common DB operations ---

    #[test]
    fn common_insert_and_select() {
        let conn = open_db();
        let entry = Common { key: "theme".to_string(), value: "dark".to_string() };
        entry.insert(&conn).unwrap();

        let fetched = Common::select(&conn, "theme".to_string()).unwrap().unwrap();
        assert_eq!(fetched.value, "dark");
    }

    #[test]
    fn common_select_missing_returns_none() {
        let conn = open_db();
        let result = Common::select(&conn, "nope".to_string()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn common_update() {
        let conn = open_db();
        let entry = Common { key: "lang".to_string(), value: "en".to_string() };
        entry.insert(&conn).unwrap();

        let mut updated = entry;
        updated.value = "fr".to_string();
        updated.update(&conn).unwrap();

        let fetched = Common::select(&conn, "lang".to_string()).unwrap().unwrap();
        assert_eq!(fetched.value, "fr");
    }

    #[test]
    fn common_delete() {
        let conn = open_db();
        let entry = Common { key: "to_delete".to_string(), value: "val".to_string() };
        entry.insert(&conn).unwrap();

        Common::delete(&conn, "to_delete".to_string()).unwrap();

        let result = Common::select(&conn, "to_delete".to_string()).unwrap();
        assert!(result.is_none());
    }
}
