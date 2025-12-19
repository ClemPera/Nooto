use aes_gcm::{Aes256Gcm, Key};
use chrono::NaiveDateTime;
use rusqlite::Connection;
use tauri_plugin_log::log::debug;

use crate::crypt::NoteData;

use rusqlite::Error::QueryReturnedNoRows;

#[derive(Debug)]
pub struct Note {
    pub id: Option<u32>,
    pub id_server: Option<u64>,
    pub id_user: Option<u32>,
    pub title: String,
    pub content: Vec<u8>, //Serialized encrypted content.
    pub nonce: Vec<u8>, //Nonce used to decrypt data.
    pub updated_at: i64,
    pub synched: bool //true: note has already been sent with server
}

impl From<shared::Note> for Note {
    fn from(note: shared::Note) -> Self {
        Note {
            id: Some(note.id),
            id_server: note.id_server,
            id_user: None,
            title: note.title,
            content: note.content,
            nonce: note.nonce,
            updated_at: note.updated_at,
            synched: true
        }
    }
}

impl Into<shared::Note> for Note {
    fn into(self) -> shared::Note {
        shared::Note {
            id: self.id.unwrap(),
            id_server: self.id_server,
            title: self.title,
            content: self.content,
            nonce: self.nonce,
            updated_at: self.updated_at,
        }
    }
}

impl Note {
    pub fn create(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute(
        "CREATE TABLE IF NOT EXISTS note (
                id INTEGER PRIMARY KEY,
                id_server INTEGER,
                id_user INTEGER NOT NULL REFERENCES user(id),
                title TEXT,
                content BLOB,
                nonce BLOB,
                updated_at INTEGER,
                synched INTEGER NOT NULL
            )", 
            (), // empty list of parameters.
        ).unwrap();

        Ok(())
    }

    pub fn select(conn: &Connection, id: u32) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let note = match conn.query_one(
            "SELECT * FROM note WHERE id = ?", 
            (id,),
            |row| {
                Ok(Note{
                    id: row.get(0)?,
                    id_server: row.get(1)?,
                    id_user: row.get(2)?,
                    title: row.get(3)?,
                    content: row.get(4)?,
                    nonce: row.get(5)?,
                    updated_at: row.get(6)?,
                    synched: row.get(7)?
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
            "INSERT INTO note (id_server, title, content, nonce, id_user, updated_at, synched) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", 
            (&self.id_server, &self.title, &self.content, &self.nonce, &self.id_user, &self.updated_at, &self.synched)
        ).unwrap();

        Ok(())
    }

    pub fn update(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute("UPDATE note SET id_server = ?, title = ?, content = ?, nonce = ?, updated_at = ?, synched = ? WHERE id = ?",
            (&self.id_server, &self.title, &self.content, &self.nonce, &self.updated_at, &self.synched, &self.id))?;

        Ok(())
    }

    pub fn select_all(conn: &Connection, id_user: u32) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let mut stmt = conn.prepare("SELECT * FROM note WHERE id_user = ?").unwrap();

        let rows = stmt.query_map(
            [id_user,],
            |row| {
                Ok(Note{
                    id: row.get(0)?,
                    id_server: row.get(1)?,
                    id_user: row.get(2)?,
                    title: row.get(3)?,
                    content: row.get(4)?,
                    nonce: row.get(5)?,
                    updated_at: row.get(6)?,
                    synched: row.get(7)?,
                })
            }
        ).unwrap();

        let mut notes = Vec::new();

        for note in rows {
            notes.push(note.unwrap());
        }

        Ok(notes)
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: Option<u32>,
    pub username: String,

    //TODO: Do not store that in plain text but use give the user the possibility to use biometric to decrypt?
    pub master_encryption_key: Key<Aes256Gcm>, 

    pub salt_recovery_data: String,
    pub mek_recovery_nonce: Vec<u8>,
    pub encrypted_mek_recovery: Vec<u8>,
    pub token: Option<Vec<u8>>,
    pub instance: Option<String>
}

impl User {
    pub fn create(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute(
        "CREATE TABLE IF NOT EXISTS user (
                id INTEGER PRIMARY KEY,
                username TEXT,
                master_encryption_key BLOB,
                salt_recovery_data TEXT,
                mek_recovery_nonce BLOB,
                encrypted_mek_recovery BLOB,
                token TEXT,
                instance TEXT
            )", 
            (), // empty list of parameters.
        ).unwrap();

        Ok(())
    }

    pub fn insert(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute(
            "INSERT INTO user (id, username, master_encryption_key, salt_recovery_data, mek_recovery_nonce, encrypted_mek_recovery, token, instance) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", 
            (&self.id, &self.username, &self.master_encryption_key.to_vec(), &self.salt_recovery_data, &self.mek_recovery_nonce, &self.encrypted_mek_recovery, &self.token, &self.instance)
        ).unwrap();

        Ok(())
    }

    pub fn select(conn: &Connection, username: String) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let user = match conn.query_one(
            "SELECT * FROM user WHERE username = ?", 
            (username,),
            |row| {
                let mek: Vec<u8> = row.get(2)?;
                let mek: [u8; 32] = mek.try_into().unwrap();
                let mek: Key<Aes256Gcm> = mek.into();

                Ok(User{
                    id: row.get(0)?,
                    username: row.get(1)?,
                    master_encryption_key: mek,
                    salt_recovery_data: row.get(3)?,
                    mek_recovery_nonce: row.get(4)?,
                    encrypted_mek_recovery: row.get(5)?,
                    token: row.get(6)?,
                    instance: row.get(7)?
                })
            }
        ) {
            Ok(v) => Some(v),
            Err(e) if e == QueryReturnedNoRows => None,
            Err(e) => return Err(e.into())
        };

        Ok(user)
    }

    pub fn select_all(conn: &Connection) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let mut stmt = conn.prepare("SELECT * FROM user").unwrap();

        let rows = stmt.query_map(
            [],
            |row| {
                let mek: Vec<u8> = row.get(2)?;
                let mek: [u8; 32] = mek.try_into().unwrap();
                let mek: Key<Aes256Gcm> = mek.into();

                Ok(User{
                    id: row.get(0)?,
                    username: row.get(1)?,
                    master_encryption_key: mek,
                    salt_recovery_data: row.get(3)?,
                    mek_recovery_nonce: row.get(4)?,
                    encrypted_mek_recovery: row.get(5)?,
                    token: row.get(6)?,
                    instance: row.get(7)?
                })
            }
        ).unwrap();

        let mut users = Vec::new();

        for user in rows {
            users.push(user.unwrap());
        }

        Ok(users)
    }
    
    pub fn update(&self, conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        conn.execute("UPDATE user SET username = ?, master_encryption_key = ?, salt_recovery_data = ?, mek_recovery_nonce = ?, encrypted_mek_recovery = ?, token = ?, instance = ? WHERE id = ?",
        (&self.username, &self.master_encryption_key.to_vec(), &self.salt_recovery_data, &self.mek_recovery_nonce, &self.encrypted_mek_recovery, &self.token, &self.id, &self.instance))?;
        
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
        conn.execute("UPDATE note SET value = ? WHERE key = ?",
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