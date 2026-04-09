use aes_gcm::{Aes256Gcm, Key};
use anyhow::{Context, Result};
use chrono::Local;
use rusqlite::Connection;
use tauri_plugin_log::log::{debug, trace};
use uuid::{NoContext, Uuid};

use crate::{
    crypt::{self, NoteData},
    db::schema::{Common, Note, Workspace},
};

//TODO: refactor this, data encryption and stuff should not be inside db?
pub fn create_note(
    conn: &Connection,
    id_workspace: u32,
    title: String,
    parent_id: Option<String>,
    is_folder: bool,
    mek: Key<Aes256Gcm>,
) -> Result<String> {
    let (content, nonce) = crypt::encrypt_data("".as_bytes(), &mek)
        .context("Failed to encrypt initial note content")?;

    let metadata_ser = serde_json::to_vec(&crypt::NoteMetadata {
        title,
        parent_id,
        is_folder,
        folder_open: true,
    })
    .context("Failed to serialize note metadata")?;

    let (metadata, metadata_nonce) = crypt::encrypt_data(&metadata_ser, &mek)
        .context("Failed to encrypt note metadata")?;

    let note = Note {
        uuid: Uuid::new_v7(uuid::Timestamp::now(NoContext)).to_string(),
        id_workspace: Some(id_workspace),
        content,
        nonce,
        metadata,
        metadata_nonce,
        updated_at: Local::now().to_utc().timestamp(),
        synched: false,
        deleted: false,
    };

    note.insert(conn).context("Failed to save new note")?;

    Ok(note.uuid)
}

pub fn get_note(conn: &Connection, uuid: String, mek: Key<Aes256Gcm>) -> Result<NoteData> {
    trace!("getting note {uuid}");

    let note = Note::select(conn, uuid.clone())
        .context("Failed to read note from database")?
        .ok_or_else(|| anyhow::anyhow!("Note '{}' not found", uuid))?;

    let content_plaintext = crypt::decrypt_data(&note.content, &note.nonce, &mek)
        .context("Failed to decrypt note content")?;
    let metadata_plaintext = crypt::decrypt_data(&note.metadata, &note.metadata_nonce, &mek)
        .context("Failed to decrypt note metadata")?;

    let metadata: crypt::NoteMetadata = serde_json::from_slice(&metadata_plaintext)
        .context("Failed to parse note metadata")?;

    let decrypted_note = NoteData {
        id: note.uuid,
        title: metadata.title,
        parent_id: metadata.parent_id,
        is_folder: metadata.is_folder,
        folder_open: metadata.folder_open,
        content: String::from_utf8(content_plaintext).context("Note content is not valid UTF-8")?,
        updated_at: note.updated_at,
        deleted: note.deleted,
    };

    Ok(decrypted_note)
}

pub fn get_notes(conn: &Connection, id_workspace: u32) -> Result<Vec<Note>> {
    Note::select_all(conn, id_workspace).context("Failed to read notes from database")
}

pub fn update_note(conn: &Connection, note_data: NoteData, mek: Key<Aes256Gcm>) -> Result<()> {
    let (content, nonce) = crypt::encrypt_data(note_data.content.as_bytes(), &mek)
        .context("Failed to encrypt note content")?;

    let metadata_ser = serde_json::to_vec(&crypt::NoteMetadata {
        title: note_data.title,
        parent_id: note_data.parent_id,
        is_folder: note_data.is_folder,
        folder_open: note_data.folder_open,
    })
    .context("Failed to serialize note metadata")?;

    let (metadata, metadata_nonce) =
        crypt::encrypt_data(&metadata_ser, &mek).context("Failed to encrypt note metadata")?;

    let mut note = Note::select(conn, note_data.id.clone())
        .context("Failed to read note from database")?
        .ok_or_else(|| anyhow::anyhow!("Note '{}' not found", note_data.id))?;

    note.content = content;
    note.nonce = nonce;
    note.metadata = metadata;
    note.metadata_nonce = metadata_nonce;
    note.updated_at = Local::now().to_utc().timestamp();
    note.synched = false;
    note.deleted = note_data.deleted;

    note.update(conn).context("Failed to save updated note")?;

    debug!("note updated: dt:{}", note.updated_at);
    Ok(())
}

pub fn create_workspace(conn: &Connection, workspace_name: String) -> Result<Workspace> {
    let workspace_encryption_data =
        crypt::create_workspace().context("Failed to generate workspace encryption data")?;

    let workspace = Workspace {
        id: 0, // placeholder, overwritten after insert
        workspace_name,
        username: None,
        master_encryption_key: workspace_encryption_data.master_encryption_key,
        salt_recovery_data: workspace_encryption_data.salt_recovery_data.to_string(),
        mek_recovery_nonce: workspace_encryption_data.mek_recovery_nonce,
        encrypted_mek_recovery: workspace_encryption_data.encrypted_mek_recovery,
        token: None,
        instance: None,
        last_sync_at: chrono::DateTime::<chrono::Utc>::MIN_UTC.timestamp(),
    };

    workspace.insert(conn).context("Failed to save new workspace")?;

    let workspace = Workspace {
        id: conn.last_insert_rowid() as u32,
        ..workspace
    };

    //TODO: send recovery keys to frontend

    Ok(workspace)
}

pub fn update_workspace(conn: &Connection, new_workspace: Workspace) -> Result<()> {
    new_workspace.update(conn).context("Failed to update workspace")
}

pub fn get_workspace(conn: &Connection, workspace_name: String) -> Result<Option<Workspace>> {
    Workspace::select(conn, workspace_name).context("Failed to read workspace from database")
}

pub fn get_workspaces(conn: &Connection) -> Result<Vec<Workspace>> {
    Workspace::select_all(conn).context("Failed to read workspaces from database")
}

fn common_insert_or_update(conn: &Connection, key: String, value: String) -> Result<()> {
    match Common::select(conn, key.clone()).context("Failed to read common entry")? {
        Some(mut common) => {
            common.value = value;
            common.update(conn).context("Failed to update common entry")?;
        }
        None => {
            let common = Common { key, value };
            common.insert(conn).context("Failed to insert common entry")?;
        }
    }

    Ok(())
}

pub fn set_logged_workspace(conn: &Connection, workspace: Option<Workspace>) -> Result<()> {
    match workspace {
        Some(workspace) => {
            common_insert_or_update(conn, "logged".to_string(), workspace.workspace_name)
        }
        None => Common::delete(conn, "logged".to_string()),
    }
}

pub fn get_logged_workspace(conn: &Connection) -> Result<Option<Workspace>> {
    match Common::select(conn, "logged".to_string()).context("Failed to read logged workspace key")? {
        Some(lu) => {
            let workspace = Workspace::select(conn, lu.value)
                .context("Failed to read logged workspace")?
                .ok_or_else(|| anyhow::anyhow!("Logged workspace no longer exists in database"))?;
            Ok(Some(workspace))
        }
        None => Ok(None),
    }
}

pub fn set_latest_note(conn: &Connection, uuid: Option<String>) -> Result<()> {
    match uuid {
        Some(uuid) => common_insert_or_update(conn, "latest_note".to_string(), uuid),
        None => Common::delete(conn, "latest_note".to_string()),
    }
}

pub fn get_latest_note(conn: &Connection) -> Result<Option<String>> {
    match Common::select(conn, "latest_note".to_string()).context("Failed to read latest note")? {
        Some(lu) => Ok(Some(lu.value)),
        None => Ok(None),
    }
}

pub fn logout_workspace(conn: &Connection, workspace_name: String) -> Result<()> {
    let workspace = Workspace::select(conn, workspace_name)
        .context("Failed to read workspace")?
        .ok_or_else(|| anyhow::anyhow!("Workspace not found"))?;

    //TODO: This doesn't feel right without stopping sync
    Note::delete_all_from_workspace(conn, workspace.id).context("Failed to delete notes from workspace")?;
    workspace.delete(conn).context("Failed to delete workspace")?;
    Common::delete(conn, "logged".to_string()).context("Failed to clear logged workspace")?;

    Ok(())
}

pub fn sync_logout_workspace(conn: &Connection, workspace_name: String) -> Result<()> {
    let mut workspace = Workspace::select(conn, workspace_name)
        .context("Failed to read workspace")?
        .ok_or_else(|| anyhow::anyhow!("Workspace not found"))?;

    workspace.username = None;
    workspace.token = None;
    workspace.instance = None;

    workspace.update(conn).context("Failed to update workspace after sync logout")?;

    Ok(())
}
