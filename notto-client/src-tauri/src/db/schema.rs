use aes_gcm::{Aes256Gcm, Key};
use anyhow::{Context, Result};
use rusqlite::Connection;

use rusqlite::Error::QueryReturnedNoRows;

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

    pub fn insert(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO note (uuid, content, nonce, metadata, metadata_nonce, id_workspace, updated_at, synched, deleted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (&self.uuid, &self.content, &self.nonce, &self.metadata, &self.metadata_nonce, &self.id_workspace, &self.updated_at, &self.synched, &self.deleted),
        )
        .context("Failed to insert note")?;

        Ok(())
    }

    pub fn update(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "UPDATE note SET content = ?, nonce = ?, metadata = ?, metadata_nonce = ?, updated_at = ?, synched = ?, deleted = ? WHERE uuid = ?",
            (&self.content, &self.nonce, &self.metadata, &self.metadata_nonce, &self.updated_at, &self.synched, &self.deleted, &self.uuid),
        )
        .context("Failed to update note")?;

        Ok(())
    }

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

    pub fn delete_all_from_workspace(conn: &Connection, id_workspace: u32) -> Result<()> {
        conn.execute(
            "DELETE FROM note WHERE id_workspace = ?",
            (id_workspace,),
        )
        .context("Failed to delete notes from workspace")?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: Option<u32>,
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
}

impl Workspace {
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
                last_sync_at INTEGER NOT NULL DEFAULT -9223372036854775808
            )",
            (),
        )
        .context("Failed to create workspace table")?;

        Ok(())
    }

    pub fn insert(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO workspace (id, workspace_name, username, master_encryption_key, salt_recovery_data, mek_recovery_nonce, encrypted_mek_recovery, token, instance, last_sync_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            (&self.id, &self.workspace_name, &self.username, &self.master_encryption_key.to_vec(), &self.salt_recovery_data, &self.mek_recovery_nonce, &self.encrypted_mek_recovery, &self.token, &self.instance, &self.last_sync_at),
        )
        .context("Failed to insert workspace")?;

        Ok(())
    }

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
                })
            },
        ) {
            Ok(v) => Some(v),
            Err(e) if e == QueryReturnedNoRows => None,
            Err(e) => return Err(e).context("Failed to select workspace"),
        };

        Ok(workspace)
    }

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
                })
            })
            .context("Failed to query workspaces")?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to read workspace rows")?;

        Ok(workspaces)
    }

    pub fn update(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "UPDATE workspace SET workspace_name = ?, username = ?, master_encryption_key = ?, salt_recovery_data = ?, mek_recovery_nonce = ?, encrypted_mek_recovery = ?, token = ?, instance = ?, last_sync_at = ? WHERE id = ?",
            (&self.workspace_name, &self.username, &self.master_encryption_key.to_vec(), &self.salt_recovery_data, &self.mek_recovery_nonce, &self.encrypted_mek_recovery, &self.token, &self.instance, &self.last_sync_at, &self.id),
        )
        .context("Failed to update workspace")?;

        Ok(())
    }

    pub fn delete(&self, conn: &Connection) -> Result<()> {
        conn.execute("DELETE FROM workspace WHERE id = ?", (&self.id,))
            .context("Failed to delete workspace")?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Common {
    pub key: String,
    pub value: String,
}

impl Common {
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

    pub fn insert(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO common (key, value) VALUES (?1, ?2)",
            (&self.key, &self.value),
        )
        .context("Failed to insert common entry")?;

        Ok(())
    }

    pub fn update(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "UPDATE common SET value = ? WHERE key = ?",
            (&self.value, &self.key),
        )
        .context("Failed to update common entry")?;

        Ok(())
    }

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

    pub fn delete(conn: &Connection, key: String) -> Result<()> {
        conn.execute("DELETE FROM common WHERE key = ?", (key,))
            .context("Failed to delete common entry")?;

        Ok(())
    }
}
