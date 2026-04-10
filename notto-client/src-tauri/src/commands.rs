use anyhow::Context;
use chrono::Local;
use shared::{SelectNoteParams, SentNotes};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use serde::Serialize;
use tauri_plugin_log::log::{debug, error, trace};
use uuid::Uuid;

use crate::crypt::NoteData;
use crate::db;
use crate::db::schema::{Note, Workspace};
use crate::{crypt, sync, AppState};

/// Categorises errors so the frontend can react appropriately.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// Unexpected failure (crypto, DB, encoding, etc.).
    Internal,
    /// The requested resource does not exist.
    NotFound,
    /// No workspace is loaded or the user is not logged in.
    Unauthorized,
    /// Could not reach the server.
    Network,
    /// The caller supplied an invalid value (bad UUID, empty name, etc.).
    InvalidInput,
}

/// Serialisable error returned to the frontend by every command.
#[derive(Debug, Serialize)]
pub struct CommandError {
    pub kind: ErrorKind,
    pub message: String,
}

impl CommandError {
    pub fn unauthorized(msg: impl Into<String> + Clone) -> Self {
        error!("Unauthorized: {}", msg.clone().into());
        CommandError { kind: ErrorKind::Unauthorized, message: msg.into() }
    }

    pub fn not_found(msg: impl Into<String> + Clone) -> Self {
        error!("NotFound: {}", msg.clone().into());
        CommandError { kind: ErrorKind::NotFound, message: msg.into() }
    }

    pub fn invalid_input(msg: impl Into<String> + Clone) -> Self {
        error!("InvalidInput: {}", msg.clone().into());
        CommandError { kind: ErrorKind::InvalidInput, message: msg.into() }
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(err: anyhow::Error) -> Self {
        let kind = if err.downcast_ref::<reqwest::Error>().is_some() {
            ErrorKind::Network
        } else {
            ErrorKind::Internal
        };

        error!("{:?}: {}",kind, format!("{:#}", err) );
        CommandError { kind, message: format!("{:#}", err) }
    }
}

#[derive(Debug, Serialize)]
pub struct FilteredWorkspace {
    pub id: u32,
    pub workspace_name: String,
}

impl From<Workspace> for FilteredWorkspace {
    fn from(workspace: Workspace) -> Self {
        FilteredWorkspace {
            id: workspace.id,
            workspace_name: workspace.workspace_name,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NoteMetadata {
    pub id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub is_folder: bool,
    pub folder_open: bool,
    pub updated_at: i64,
    pub deleted: bool,
}

impl NoteMetadata {
    pub fn from_note(note: Note, key: &aes_gcm::Key<aes_gcm::Aes256Gcm>) -> anyhow::Result<Self> {
        let metadata_plaintext = crypt::decrypt_data(&note.metadata, &note.metadata_nonce, key)
            .context("Failed to decrypt note metadata")?;
        let metadata: crypt::NoteMetadata = serde_json::from_slice(&metadata_plaintext)
            .context("Failed to parse note metadata")?;

        Ok(NoteMetadata {
            id: note.uuid,
            title: metadata.title,
            parent_id: metadata.parent_id,
            is_folder: metadata.is_folder,
            folder_open: metadata.folder_open,
            updated_at: note.updated_at * 1000,
            deleted: note.deleted,
        })
    }
}

/// Response type for get_note command.
/// Converts updated_at from Unix seconds (DB) to milliseconds (JS/TS) at the boundary.
#[derive(Debug, Serialize, Clone)]
pub struct NoteResponse {
    pub id: String,
    pub title: String,
    pub parent_id: Option<String>,
    pub is_folder: bool,
    pub folder_open: bool,
    pub content: String,
    pub updated_at: i64,
    pub deleted: bool,
}

impl From<NoteData> for NoteResponse {
    fn from(note: NoteData) -> Self {
        NoteResponse {
            id: note.id,
            title: note.title,
            parent_id: note.parent_id,
            is_folder: note.is_folder,
            folder_open: note.folder_open,
            content: note.content,
            updated_at: note.updated_at * 1000, // Unix seconds → ms for JS/TS
            deleted: note.deleted,
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn init(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError> {
    let mut state = state.lock().await;

    let workspace = {
        let conn = state.database.lock().await;
        db::operations::get_logged_workspace(&conn)?
    };

    state.workspace = workspace;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn create_note(
    state: State<'_, Mutex<AppState>>,
    title: String,
    parent_id: Option<String>,
) -> Result<String, CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    let note_uuid = db::operations::create_note(
        &conn,
        workspace.id,
        title,
        parent_id,
        false, // is_folder
        workspace.master_encryption_key,
    )?;

    Ok(note_uuid)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn create_folder(
    state: State<'_, Mutex<AppState>>,
    title: String,
    parent_id: Option<String>,
) -> Result<String, CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    let folder_uuid = db::operations::create_note(
        &conn,
        workspace.id,
        title,
        parent_id,
        true, // is_folder
        workspace.master_encryption_key,
    )?;

    Ok(folder_uuid)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_note(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<NoteResponse, CommandError> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| CommandError::invalid_input(format!("'{}' is not a valid note ID", id)))?;

    let state = state.lock().await;
    let conn = state.database.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    let note = db::operations::get_note(&conn, uuid.to_string(), workspace.master_encryption_key)?;

    Workspace::update_latest_note(&conn, workspace.id, Some(&note.id))?;

    Ok(NoteResponse::from(note))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn edit_note(
    state: State<'_, Mutex<AppState>>,
    note: NoteData,
) -> Result<(), CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    db::operations::update_note(&conn, note, workspace.master_encryption_key)?;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_all_notes_metadata(
    state: State<'_, Mutex<AppState>>,
    id_workspace: u32,
) -> Result<Vec<NoteMetadata>, CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    let notes = db::operations::get_notes(&conn, id_workspace)?;

    let notes_metadata = notes
        .into_iter()
        .map(|n| NoteMetadata::from_note(n, &workspace.master_encryption_key))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(notes_metadata)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn create_workspace(
    state: State<'_, Mutex<AppState>>,
    workspace_name: String,
) -> Result<(), CommandError> {
    let mut state = state.lock().await;

    let workspace = {
        let conn = state.database.lock().await;
        db::operations::create_workspace(&conn, workspace_name)?
    };

    state.workspace = Some(workspace);

    debug!("workspace created");

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_workspaces(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<FilteredWorkspace>, CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let workspaces = db::operations::get_workspaces(&conn)?;

    let filtered = workspaces.into_iter().map(FilteredWorkspace::from).collect();

    Ok(filtered)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_logged_workspace(
    state: State<'_, Mutex<AppState>>,
    workspace_name: String,
) -> Result<FilteredWorkspace, CommandError> {
    let mut state = state.lock().await;

    let workspace = if !workspace_name.is_empty() {
        let workspace = {
            let conn = state.database.lock().await;
            db::operations::get_workspace(&conn, workspace_name)?
                .ok_or_else(|| CommandError::not_found("Workspace doesn't exist"))?
        };

        Some(workspace)
    } else {
        None
    };

    state.workspace = workspace.clone();

    let conn = state.database.lock().await;
    db::operations::set_logged_workspace(&conn, workspace.clone())?;

    let workspace = workspace.ok_or_else(|| CommandError::not_found("Workspace not found"))?;

    Ok(FilteredWorkspace::from(workspace))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_logged_workspace(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<FilteredWorkspace>, CommandError> {
    let state = state.lock().await;

    match &state.workspace {
        Some(w) => Ok(Some(FilteredWorkspace { id: w.id, workspace_name: w.workspace_name.clone() })),
        None => Ok(None),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_create_account(
    state: State<'_, Mutex<AppState>>,
    username: String,
    password: String,
    instance: Option<String>,
) -> Result<(), CommandError> {
    //For now, login needs to be run after create_account

    trace!("create account command received");

    let state = state.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("A workspace must be loaded before creating an account"))?;

    let account = crypt::create_account(password, workspace.master_encryption_key)?;

    trace!("create account: start creating");

    sync::create_account(workspace, username, account, instance)
        .await?;

    debug!("account has been created");

    //TODO: send back recovery key to frontend

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_login(
    state: State<'_, Mutex<AppState>>,
    username: String,
    password: String,
    instance: Option<String>,
) -> Result<(), CommandError> {
    trace!("login command received");

    let mut state = state.lock().await;

    let mut workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("A workspace must be loaded before logging in"))?;

    let instance: String = instance.context("Instance url is empty")?;

    let login_data = sync::login(username.clone(), password.clone(), instance.clone())
        .await?;

    debug!("account has been logged in");

    let mek = crypt::decrypt_mek(
        password,
        login_data.encrypted_mek_password,
        login_data.salt_data,
        login_data.mek_password_nonce,
    )?;

    trace!("mek decrypted");

    let notes: Vec<NoteData> = {
        let conn = state.database.lock().await;
        let notes: Vec<Note> = db::operations::get_notes(&conn, workspace.id)?;

        notes
            .into_iter()
            .map(|n| db::operations::get_note(&conn, n.uuid, mek))
            .collect::<anyhow::Result<Vec<_>>>()?
    };

    if !notes.is_empty() {
        let conn = state.database.lock().await;
        for note in notes {
            db::operations::update_note(&conn, note, mek)?;
        }
    }

    trace!("notes converted using server key");

    workspace.master_encryption_key = mek;
    workspace.token = Some(login_data.token);
    workspace.instance = Some(instance);
    workspace.username = Some(username);

    {
        let conn = state.database.lock().await;
        db::operations::update_workspace(&conn, workspace.clone())?;
    }

    trace!("db workspace modified");

    state.workspace = Some(workspace);

    trace!("state modified: {state:?}");

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_logout(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError> {
    let mut state = state.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    {
        let conn = state.database.lock().await;
        db::operations::sync_logout_workspace(&conn, workspace.workspace_name.clone())?;
    }

    state.workspace = {
        let conn = state.database.lock().await;
        db::operations::get_workspace(&conn, workspace.workspace_name)?
    };

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn logout(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError> {
    let mut state = state.lock().await;

    {
        let conn = state.database.lock().await;
        let workspace = state
            .workspace
            .clone()
            .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

        db::operations::logout_workspace(&conn, workspace.workspace_name)?;
    }

    state.workspace = None;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_version(_state: State<'_, Mutex<AppState>>) -> Result<&'static str, CommandError> {
    Ok(env!("CARGO_PKG_VERSION"))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_note(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    let mut note = db::operations::get_note(&conn, id, workspace.master_encryption_key)?;

    note.deleted = true;

    db::operations::update_note(&conn, note, workspace.master_encryption_key)?;

    Workspace::update_latest_note(&conn, workspace.id, None)?;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn restore_note(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    let mut note = db::operations::get_note(&conn, id, workspace.master_encryption_key)?;

    note.deleted = false;

    db::operations::update_note(&conn, note, workspace.master_encryption_key)?;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_latest_note_id(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<String>, CommandError> {
    let state = state.lock().await;

    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    Ok(workspace.latest_note_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn handle_conflict(
    state: State<'_, Mutex<AppState>>,
    handle: AppHandle,
    id: String,
    local: bool,
) -> Result<(), CommandError> {
    //Handle the conflict with note.
    //  Either keep the note local (local=1) or replace with the server one (local=0)
    //TODO: should this be inside `sync`?

    let state = state.lock().await;
    let workspace = state
        .workspace
        .clone()
        .ok_or_else(|| CommandError::unauthorized("No workspace is loaded"))?;

    match local {
        true => {
            let mut note = {
                let conn = state.database.lock().await;
                let mut note = Note::select(&conn, id)
                    .context("Failed to find note")?
                    .ok_or_else(|| CommandError::not_found("Note not found"))?;

                note.synched = true;
                note.update(&conn).context("Failed to mark note as synched")?;

                note
            };

            note.updated_at = Local::now().to_utc().timestamp();

            let username = workspace
                .username
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Workspace has no username"))?;
            let token = workspace
                .token
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Workspace has no token"))?;
            let instance = workspace
                .instance
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Workspace has no instance"))?;

            let sent_notes = SentNotes {
                username,
                notes: vec![note.into()],
                token,
                force: true,
            };

            let results = sync::operations::send_notes(sent_notes, instance).await?;

            for result in results {
                match result.status {
                    shared::NoteStatus::Ok => {}
                    shared::NoteStatus::Conflict(conflicted_note) => {
                        error!(
                            "Conflict in conflict handling, this shouldn't happen: {:?}",
                            conflicted_note
                        );
                    }
                }
            }

            debug!("conflicted note has been sent to server");
        }
        false => {
            let username = workspace
                .username
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Workspace has no username"))?;
            let token = workspace
                .token
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Workspace has no token"))?;
            let instance = workspace
                .instance
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Workspace has no instance"))?;

            let params = SelectNoteParams {
                username,
                token: hex::encode(token),
                note_id: id,
            };

            let note = sync::operations::select_note(params, instance).await?;

            {
                let conn = state.database.lock().await;

                let note = db::schema::Note::from(note);
                note.update(&conn).context("Failed to save server note locally")?;

                let all_notes = db::operations::get_notes(&conn, workspace.id)?;

                let notes_metadata = all_notes
                    .into_iter()
                    .map(|n| NoteMetadata::from_note(n, &workspace.master_encryption_key))
                    .collect::<anyhow::Result<Vec<_>>>()?;

                handle
                    .emit("new_note_metadata", &notes_metadata)
                    .context("Failed to emit updated notes")?;
            }

            debug!("conflicted note has been saved locally");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{Aes256Gcm, KeyInit};
    use argon2::password_hash::rand_core::OsRng;

    fn random_key() -> aes_gcm::Key<Aes256Gcm> {
        Aes256Gcm::generate_key(OsRng)
    }

    fn dummy_workspace(name: &str) -> Workspace {
        Workspace {
            id: 7,
            workspace_name: name.to_string(),
            username: Some("alice".to_string()),
            master_encryption_key: random_key(),
            salt_recovery_data: "salt".to_string(),
            mek_recovery_nonce: vec![],
            encrypted_mek_recovery: vec![],
            token: None,
            instance: None,
            last_sync_at: 0,
            latest_note_id: None,
        }
    }

    // --- CommandError constructors ---

    #[test]
    fn command_error_unauthorized_has_correct_kind() {
        let e = CommandError::unauthorized("not logged in");
        assert!(matches!(e.kind, ErrorKind::Unauthorized));
        assert_eq!(e.message, "not logged in");
    }

    #[test]
    fn command_error_not_found_has_correct_kind() {
        let e = CommandError::not_found("note missing");
        assert!(matches!(e.kind, ErrorKind::NotFound));
        assert_eq!(e.message, "note missing");
    }

    #[test]
    fn command_error_invalid_input_has_correct_kind() {
        let e = CommandError::invalid_input("empty name");
        assert!(matches!(e.kind, ErrorKind::InvalidInput));
        assert_eq!(e.message, "empty name");
    }

    // --- From<anyhow::Error> ---

    #[test]
    fn command_error_from_plain_anyhow_is_internal() {
        let e: CommandError = anyhow::anyhow!("db error").into();
        assert!(matches!(e.kind, ErrorKind::Internal));
    }

    #[test]
    fn command_error_from_reqwest_error_is_network() {
        // Build a reqwest::Error by triggering a URL parse failure wrapped in anyhow.
        let reqwest_err = reqwest::blocking::get("not-a-valid-url://bad").unwrap_err();
        let anyhow_err = anyhow::Error::new(reqwest_err);
        let e: CommandError = anyhow_err.into();
        assert!(matches!(e.kind, ErrorKind::Network));
    }

    // --- FilteredWorkspace ---

    #[test]
    fn filtered_workspace_from_workspace() {
        let ws = dummy_workspace("my_ws");
        let filtered = FilteredWorkspace::from(ws);
        assert_eq!(filtered.id, 7);
        assert_eq!(filtered.workspace_name, "my_ws");
    }
}
