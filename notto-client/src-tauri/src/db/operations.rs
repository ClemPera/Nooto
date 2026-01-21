use core::panic;

use aes_gcm::{Aes256Gcm, Key};
use chrono::{DateTime, Local, NaiveDateTime};
use rusqlite::Connection;
use serde::Serialize;
use tauri_plugin_log::log::{debug, trace};
use uuid::{NoContext, Uuid};

use crate::{crypt::{self, NoteData}, db::schema::{Common, Note, Workspace}};

//TODO: refactor this, data encryption and stuff should not be inside db?
pub fn create_note(conn: &Connection, id_workspace: u32, title: String, mek: Key<Aes256Gcm>) -> Result<(), Box<dyn std::error::Error>> {
    let (content, nonce) = crypt::encrypt_note("".to_string(), mek).unwrap(); //Content empty because it's first note

    let note = Note {
        uuid: Uuid::new_v7(uuid::Timestamp::now(NoContext)).as_bytes().to_vec(),
        id_workspace: Some(id_workspace),
        content,
        nonce,
        title,
        updated_at: Local::now().to_utc().timestamp(),
        synched: false
    };

    note.insert(conn,).unwrap();

    Ok(())
}

pub fn get_note(conn: &Connection, uuid: Vec<u8>, mek: Key<Aes256Gcm>) -> Result<NoteData, Box<dyn std::error::Error>> {
    let note = Note::select(conn, uuid).unwrap().unwrap();

    //TODO: decrypt elsewhere?
    let decrypted_note = crypt::decrypt_note(note, mek).unwrap();

    Ok(decrypted_note)
}

pub fn get_notes(conn: &Connection, id_workspace: u32) -> Result<Vec<Note>, Box<dyn std::error::Error>> {
    let notes = Note::select_all(conn, id_workspace).unwrap();

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

pub fn create_workspace(conn: &Connection, workspace_name: String) -> Result<Workspace, Box<dyn std::error::Error>> {
    let workspace_encryption_data = crypt::create_workspace();

    let mut workspace = Workspace {
        id: None,
        workspace_name,
        username: None,
        master_encryption_key: workspace_encryption_data.master_encryption_key,
        salt_recovery_data: workspace_encryption_data.salt_recovery_data.to_string(),
        mek_recovery_nonce: workspace_encryption_data.mek_recovery_nonce,
        encrypted_mek_recovery: workspace_encryption_data.encrypted_mek_recovery,
        token: None,
        instance: None
    };

    workspace.insert(&conn).unwrap();

    workspace.id = Some(conn.last_insert_rowid() as u32);

    //TODO: send recovery keys to frontend

    Ok(workspace)
}

pub fn update_workspace(conn: &Connection, new_workspace: Workspace) {
    new_workspace.update(conn).unwrap();
}

pub fn get_workspace(conn: &Connection, workspace_name: String) -> Result<Option<Workspace>, Box<dyn std::error::Error>> {
    let workspace = Workspace::select(conn, workspace_name).unwrap();

    Ok(workspace)
}

pub fn get_workspaces(conn: &Connection) -> Result<Vec<Workspace>, Box<dyn std::error::Error>> {
    let workspaces = Workspace::select_all(conn).unwrap();

    Ok(workspaces)
}

pub fn set_logged_workspace(conn: &Connection, workspace: Option<Workspace>) {
    match workspace {
        Some(workspace) => {
            match Common::select(conn, "logged".to_string()).unwrap() {
                Some(mut common) => {
                        common.value = workspace.workspace_name;
        
                        common.update(conn).unwrap();
                    },
        
                None => {
                    let common = Common {
                        key: "logged".to_string(),
                        value: workspace.workspace_name,
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

pub fn get_logged_workspace(conn: &Connection) -> Option<Workspace> {
    match Common::select(conn, "logged".to_string()).unwrap() {
        Some(lu) => {
            Some(Workspace::select(conn, lu.value).unwrap().unwrap())
        },
        None => None,
    }
}

pub fn logout_workspace(conn: &Connection, workspace_name: String) {
    let workspace = Workspace::select(conn, workspace_name).unwrap().unwrap();

    //TODO: This doesn't feel right without stopping sync
    Note::delete_from_workspace(conn, workspace.id.unwrap());
    workspace.delete(conn).unwrap();

    Common::delete(conn, "logged".to_string());
}

pub fn sync_logout_workspace(conn: &Connection, workspace_name: String) {
    let mut workspace = Workspace::select(conn, workspace_name).unwrap().unwrap();

    workspace.username = None;
    workspace.token = None;
    workspace.instance = None;

    workspace.update(conn).unwrap();
}