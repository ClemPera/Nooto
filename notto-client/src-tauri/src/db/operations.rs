use core::panic;

use aes_gcm::{Aes256Gcm, Key};
use chrono::{DateTime, Local, NaiveDateTime};
use rusqlite::Connection;
use serde::Serialize;
use tauri_plugin_log::log::{debug, trace};

use crate::{crypt::{self, NoteData}, db::schema::{Common, Note, User}};

//TODO: refactor this, data encryption and stuff should not be inside db?
pub fn create_note(conn: &Connection, id_user: u32, title: String, mek: Key<Aes256Gcm>) -> Result<(), Box<dyn std::error::Error>> {
    let (content, nonce) = crypt::encrypt_note("".to_string(), mek).unwrap(); //Content empty because it's first note

    let note = Note {
        id: None,
        id_server: None,
        id_user: Some(id_user),
        content,
        nonce,
        title,
        updated_at: Local::now().to_utc().timestamp(),
        synched: false
    };

    note.insert(conn,).unwrap();

    Ok(())
}

pub fn get_note(conn: &Connection, id: u32, mek: Key<Aes256Gcm>) -> Result<NoteData, Box<dyn std::error::Error>> {
    let note = Note::select(conn, id).unwrap().unwrap();

    let decrypted_note = crypt::decrypt_note(note, mek).unwrap();

    debug!("note decrypted");

    Ok(decrypted_note)
}

pub fn get_notes(conn: &Connection, id_user: u32) -> Result<Vec<Note>, Box<dyn std::error::Error>> {
    let notes = Note::select_all(conn, id_user).unwrap();

    Ok(notes)
}

pub fn update_note(conn: &Connection, note_data: NoteData, mek: Key<Aes256Gcm>) -> Result<(), Box<dyn std::error::Error>> {
    let (content, nonce) = crypt::encrypt_note(note_data.content, mek).unwrap();
    
    let mut note = Note::select(conn, note_data.id).unwrap().unwrap();

    note.title = note_data.title;
    note.content = content;
    note.nonce = nonce;
    note.updated_at = Local::now().to_utc().timestamp();
    note.synched = false;
    
    note.update(conn).unwrap();
    
    trace!("note updated");
    Ok(())
}

pub fn create_user(conn: &Connection, username: String) -> Result<User, Box<dyn std::error::Error>> {
    let user_encryption_data = crypt::create_user();

    let user = User {
        id: None,
        username,
        master_encryption_key: user_encryption_data.master_encryption_key,
        salt_recovery_data: user_encryption_data.salt_recovery_data.to_string(),
        mek_recovery_nonce: user_encryption_data.mek_recovery_nonce,
        encrypted_mek_recovery: user_encryption_data.encrypted_mek_recovery,
        token: None,
        instance: None
    };

    user.insert(&conn).unwrap();

    //TODO: send recovery keys to frontend

    Ok(user)
}

pub fn update_user(conn: &Connection, new_user: User) {
    new_user.update(conn).unwrap();
}

pub fn get_user(conn: &Connection, username: String) -> Result<Option<User>, Box<dyn std::error::Error>> {
    let user = User::select(conn, username).unwrap();

    Ok(user)
}

pub fn get_users(conn: &Connection) -> Result<Vec<User>, Box<dyn std::error::Error>> {
    let users = User::select_all(conn).unwrap();

    Ok(users)
}

pub fn set_logged_user(conn: &Connection, user: Option<User>) {
    match user {
        Some(user) => {
            match Common::select(conn, "logged".to_string()).unwrap() {
                Some(mut common) => {
                        common.value = user.username;
        
                        common.update(conn).unwrap();
                    },
        
                None => {
                    let common = Common {
                        key: "logged".to_string(),
                        value: user.username,
                    };
                    
                    common.insert(conn).unwrap();
                },
            }
        },
        None => {
            Common::delete(conn, "logged".to_string());
        }
    }
}

pub fn get_logged_user(conn: &Connection) -> Option<User> {
    match Common::select(conn, "logged".to_string()).unwrap() {
        Some(lu) => {
            Some(User::select(conn, lu.value).unwrap().unwrap())
        },
        None => None,
    }
}