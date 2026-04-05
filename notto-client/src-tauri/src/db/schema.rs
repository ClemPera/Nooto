use aes_gcm::{Aes256Gcm, Key};
use chrono::NaiveDateTime;
use rusqlite::Connection;
use tauri_plugin_log::log::debug;

use crate::crypt::NoteData;

use rusqlite::Error::QueryReturnedNoRows;

#[derive(Debug)]
pub struct Note {
    pub uuid: String,
    pub id_workspace: Option<u32>,
    pub content: Vec<u8>, //Serialized encrypted content.
    pub nonce: Vec<u8>, //Nonce used to decrypt data.
    pub metadata: Vec<u8>,
    pub metadata_nonce: Vec<u8>,
    pub updated_at: i64,
    pub synched: bool, //true: note has already been sent with server
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
            deleted: note.deleted
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
            deleted: self.deleted
        }
    }
}

impl Note {
    pub fn create(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
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
            (), // empty list of parameters.
        ).unwrap();

        Ok(())
    }

    pub fn select(conn: &Connection, uuid: String) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let note = match conn.query_one(
            "SELECT * FROM note WHERE uuid = ?", 
            (uuid,),
            |row| {
                Ok(Note{
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
            }
        ) {
            Ok(note) => Some(note),
            Err(_) => None
        };

        Ok(note)
    }

    pub fn insert(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute(
            "INSERT INTO note (uuid, content, nonce, metadata, metadata_nonce, id_workspace, updated_at, synched, deleted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", 
            (&self.uuid, &self.content, &self.nonce, &self.metadata, &self.metadata_nonce, &self.id_workspace, &self.updated_at, &self.synched, &self.deleted)
        ).unwrap();

        Ok(())
    }

    pub fn update(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute("UPDATE note SET content = ?, nonce = ?, metadata = ?, metadata_nonce = ?, updated_at = ?, synched = ?, deleted = ? WHERE uuid = ?",
            (&self.content, &self.nonce, &self.metadata, &self.metadata_nonce, &self.updated_at, &self.synched, &self.deleted, &self.uuid))?;

        Ok(())
    }

    pub fn select_all(conn: &Connection, id_workspace: u32) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let mut stmt = conn.prepare("SELECT * FROM note WHERE id_workspace = ?").unwrap();

        let rows = stmt.query_map(
            [id_workspace,],
            |row| {
                Ok(Note{
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
            }
        ).unwrap();

        let mut notes = Vec::new();

        for note in rows {
            notes.push(note.unwrap());
        }

        Ok(notes)
    }

    pub fn delete_all_from_workspace(conn: &Connection, id_workspace: u32) {
        conn.execute("DELETE FROM note WHERE id_workspace = ?", (id_workspace, )).unwrap();
    }
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: Option<u32>,
    pub workspace_name: String,
    pub username: Option<String>,

    //TODO: Do not store that in plain text but use give the user the possibility to use biometric to decrypt?
    pub master_encryption_key: Key<Aes256Gcm>, 

    pub salt_recovery_data: String,
    pub mek_recovery_nonce: Vec<u8>,
    pub encrypted_mek_recovery: Vec<u8>,
    pub token: Option<Vec<u8>>,
    pub instance: Option<String>,
    pub last_sync_at: i64,
}

impl Workspace {
    pub fn create(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
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
        ).unwrap();

        // Migration: add last_sync_at to existing databases that don't have it yet
        let _ = conn.execute(
            "ALTER TABLE workspace ADD COLUMN last_sync_at INTEGER NOT NULL DEFAULT -9223372036854775808",
            (),
        );

        Ok(())
    }

    pub fn insert(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute(
            "INSERT INTO workspace (id, workspace_name, username, master_encryption_key, salt_recovery_data, mek_recovery_nonce, encrypted_mek_recovery, token, instance, last_sync_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", 
            (&self.id, &self.workspace_name, &self.username, &self.master_encryption_key.to_vec(), &self.salt_recovery_data, &self.mek_recovery_nonce, &self.encrypted_mek_recovery, &self.token, &self.instance, &self.last_sync_at)
        ).unwrap();

        Ok(())
    }

    pub fn select(conn: &Connection, workspace_name: String) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let workspace = match conn.query_one(
            "SELECT * FROM workspace WHERE workspace_name = ?", 
            (workspace_name,),
            |row| {
                let mek: Vec<u8> = row.get(3)?;
                let mek: [u8; 32] = mek.try_into().unwrap();
                let mek: Key<Aes256Gcm> = mek.into();

                Ok(Workspace{
                    id: row.get(0)?,
                    workspace_name: row.get(1)?,
                    username: row.get(2)?,
                    master_encryption_key: mek,
                    salt_recovery_data: row.get(4)?,
                    mek_recovery_nonce: row.get(5)?,
                    encrypted_mek_recovery: row.get(6)?,
                    token: row.get(7)?,
                    instance: row.get(8)?,
                    last_sync_at: row.get(9)?
                })
            }
        ) {
            Ok(v) => Some(v),
            Err(e) if e == QueryReturnedNoRows => None,
            Err(e) => return Err(e.into())
        };

        Ok(workspace)
    }

    pub fn select_all(conn: &Connection) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let mut stmt = conn.prepare("SELECT * FROM workspace").unwrap();

        let rows = stmt.query_map(
            [],
            |row| {
                let mek: Vec<u8> = row.get(3)?;
                let mek: [u8; 32] = mek.try_into().unwrap();
                let mek: Key<Aes256Gcm> = mek.into();

                Ok(Workspace{
                    id: row.get(0)?,
                    workspace_name: row.get(1)?,
                    username: row.get(2)?,
                    master_encryption_key: mek,
                    salt_recovery_data: row.get(4)?,
                    mek_recovery_nonce: row.get(5)?,
                    encrypted_mek_recovery: row.get(6)?,
                    token: row.get(7)?,
                    instance: row.get(8)?,
                    last_sync_at: row.get(9)?
                })
            }
        ).unwrap();

        let mut workspaces = Vec::new();

        for workspace in rows {
            workspaces.push(workspace.unwrap());
        }

        Ok(workspaces)
    }
    
    pub fn update(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        conn.execute("UPDATE workspace SET workspace_name = ?, username = ?, master_encryption_key = ?, salt_recovery_data = ?, mek_recovery_nonce = ?, encrypted_mek_recovery = ?, token = ?, instance = ?, last_sync_at = ? WHERE id = ?",
        (&self.workspace_name, &self.username, &self.master_encryption_key.to_vec(), &self.salt_recovery_data, &self.mek_recovery_nonce, &self.encrypted_mek_recovery, &self.token, &self.instance, &self.last_sync_at, &self.id))?;
        
        Ok(())
    }

    pub fn delete(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute("DELETE FROM workspace WHERE id = ?", (&self.id, ))?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Common {
    pub key: String,
    pub value: String
}

impl Common {
    pub fn create(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute(
        "CREATE TABLE IF NOT EXISTS common (
                key TEXT PRIMARY KEY,
                value TEXT
            )", 
            (), // empty list of parameters.
        ).unwrap();

        Ok(())
    }

    pub fn insert(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute(
            "INSERT INTO common (key, value) VALUES (?1, ?2)", 
            (&self.key, &self.value)
        ).unwrap();

        Ok(())
    }

    pub fn update(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute("UPDATE common SET value = ? WHERE key = ?",
            (&self.value, &self.key))?;

        Ok(())
    }

    pub fn select(conn: &Connection, key: String) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let value = match conn.query_one("SELECT value FROM common WHERE key = ?", 
            (key.clone(), ), 
            |row| {
                Ok(Common{
                    key,
                    value: row.get(0)?
                })
            }
        ) {
            Ok(v) => Some(v),
            Err(e) if e == QueryReturnedNoRows => None,
            Err(e) => return Err(e.into())
        };

        Ok(value)
    }

    pub fn delete(conn: &Connection, key: String) {
        conn.execute("DELETE FROM common WHERE key = ?", (key, )).unwrap();
    }
}