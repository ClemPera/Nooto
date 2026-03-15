use chrono::Local;
use shared::{SelectNoteParams, SelectNotesParams, SentNotes};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use serde::Serialize;
use tauri_plugin_log::log::{debug, error, trace};
use uuid::Uuid;

use crate::crypt::NoteData;
use crate::db;
use crate::db::schema::{Note, Workspace};
use crate::{crypt, sync, AppState};

///Convert any error to string for frontend
#[derive(Debug, Serialize)]
pub struct CommandError {
    message: String,
}

impl From<Box<dyn std::error::Error>> for CommandError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        CommandError {
            message: err.to_string(),
        }
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
            id: workspace.id.unwrap(),
            workspace_name: workspace.workspace_name,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NoteMetadata {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
    pub deleted: bool
}

impl From<Note> for NoteMetadata {
    fn from(note: Note) -> Self {
        NoteMetadata {
            id: note.uuid,
            title: note.title,
            updated_at: note.updated_at * 1000, // Unix seconds → ms for JS/TS
            deleted: note.deleted
        }
    }
}

/// Response type for get_note command.
/// Converts updated_at from Unix seconds (DB) to milliseconds (JS/TS) at the boundary.
#[derive(Debug, Serialize, Clone)]
pub struct NoteResponse {
    pub id: String,
    pub title: String,
    pub content: String,
    pub updated_at: i64,
    pub deleted: bool,
}

impl From<NoteData> for NoteResponse {
    fn from(note: NoteData) -> Self {
        NoteResponse {
            id: note.id,
            title: note.title,
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
        db::operations::get_logged_workspace(&conn)
    };

    state.workspace = workspace;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn create_note(
    state: State<'_, Mutex<AppState>>,
    title: String,
) -> Result<String, CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    let workspace = state.workspace.clone().unwrap();

    let note_uuid = db::operations::create_note(
        &conn,
        workspace.id.unwrap(),
        title,
        workspace.master_encryption_key,
    )
    .unwrap();

    Ok(note_uuid)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_note(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<NoteResponse, CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let note = db::operations::get_note(
        &conn,
        Uuid::parse_str(&id).unwrap().to_string(),
        state.workspace.clone().unwrap().master_encryption_key,
    )
    .unwrap();

    // Save current note uuid to db
    db::operations::set_latest_note(&conn, Some(note.clone().id));

    Ok(NoteResponse::from(note))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn edit_note(
    state: State<'_, Mutex<AppState>>,
    note: NoteData,
) -> Result<(), CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    db::operations::update_note(
        &conn,
        note,
        state.workspace.clone().unwrap().master_encryption_key,
    )
    .unwrap();

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_all_notes_metadata(
    state: State<'_, Mutex<AppState>>,
    id_workspace: u32,
) -> Result<Vec<NoteMetadata>, CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    let notes = db::operations::get_notes(&conn, id_workspace).unwrap();

    let notes_metadata = notes.into_iter().map(NoteMetadata::from).collect();

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
        db::operations::create_workspace(&conn, workspace_name).unwrap()
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

    let workspaces = db::operations::get_workspaces(&conn).unwrap();

    let filtered_worspaces = workspaces
        .into_iter()
        .map(FilteredWorkspace::from)
        .collect();

    Ok(filtered_worspaces)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_logged_workspace(
    state: State<'_, Mutex<AppState>>,
    workspace_name: String,
) -> Result<FilteredWorkspace, CommandError> {
    let mut state = state.lock().await;

    let workspace = match workspace_name.is_empty() {
        false => {
            let workspace = {
                let conn = state.database.lock().await;
                match db::operations::get_workspace(&conn, workspace_name).unwrap() {
                    Some(u) => u,
                    None => {
                        return Err(CommandError {
                            message: "Workspace doesn't exist".to_string(),
                        })
                    }
                }
            };

            Some(workspace)
        }
        true => None,
    };

    state.workspace = workspace.clone();

    let conn = state.database.lock().await;
    db::operations::set_logged_workspace(&conn, workspace.clone());

    let workspace = workspace.unwrap();

    Ok(FilteredWorkspace {
        id: workspace.id.unwrap(),
        workspace_name: workspace.workspace_name,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_logged_workspace(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<FilteredWorkspace>, CommandError> {
    let state = state.lock().await;

    match &state.workspace {
        Some(w) => Ok(Some(FilteredWorkspace {
            id: w.id.unwrap(),
            workspace_name: w.workspace_name.clone(),
        })),
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

    let workspace = state.workspace.clone().ok_or_else(|| CommandError {
        message: "A workspace should have been loaded before creating an account".to_string(),
    })?;

    let account = crypt::create_account(password, workspace.master_encryption_key);

    trace!("create account: start creating");
    sync::create_account(workspace, username, account, instance).await;

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

    let mut workspace = state.workspace.clone().ok_or_else(|| CommandError {
        message: "A workspace should have been loaded before creating an account".to_string(),
    })?;

    let instance = match instance {
        Some(i) => i,
        None => "http://localhost:3000".to_string(), //TODO
    };

    let login_data = sync::login(username.clone(), password.clone(), instance.clone()).await;

    debug!("account has been logged in");

    let mek = crypt::decrypt_mek(
        password,
        login_data.encrypted_mek_password,
        login_data.salt_data,
        login_data.mek_password_nonce,
    );

    trace!("mek decrypted");

    // Convert notes using server key
    let notes: Vec<Note> = {
        let conn = state.database.lock().await;
        db::operations::get_notes(&conn, workspace.id.unwrap()).unwrap()
    };

    if !notes.is_empty() {
        //Decrypt note using old mek
        let notes: Vec<NoteData> = notes
            .into_iter()
            .map(|n| crypt::decrypt_note(n, workspace.master_encryption_key).unwrap())
            .collect();

        {
            //Update notes inside db using new mek
            let conn = state.database.lock().await;
            notes
                .into_iter()
                .for_each(|n| db::operations::update_note(&conn, n, mek).unwrap());
        }
    }

    trace!("notes converted using server key");

    workspace.master_encryption_key = mek;
    workspace.token = Some(login_data.token);
    workspace.instance = Some(instance);
    workspace.username = Some(username);

    {
        let conn = state.database.lock().await;
        db::operations::update_workspace(&conn, workspace.clone());
    }

    trace!("db workspace modified");

    state.workspace = Some(workspace);

    trace!("state modified: {state:?}");

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_logout(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError> {
    let mut state = state.lock().await;
    let workspace = state.workspace.clone().unwrap();

    {
        let conn = state.database.lock().await;
        db::operations::sync_logout_workspace(&conn, workspace.workspace_name.clone());
    }

    state.workspace = {
        let conn = state.database.lock().await;
        db::operations::get_workspace(&conn, workspace.workspace_name).unwrap()
    };

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn logout(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError> {
    let mut state = state.lock().await;

    {
        let conn = state.database.lock().await;
        let workspace = state.workspace.clone().unwrap();

        db::operations::logout_workspace(&conn, workspace.workspace_name);
    }

    state.workspace = None;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_version(state: State<'_, Mutex<AppState>>) -> Result<&str, CommandError> {
    return Ok(env!("CARGO_PKG_VERSION"));
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_note(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let mut note = db::operations::get_note(
        &conn,
        id,
        state.workspace.clone().unwrap().master_encryption_key,
    )
    .unwrap();
    note.deleted = true;

    db::operations::update_note(
        &conn,
        note,
        state.workspace.clone().unwrap().master_encryption_key,
    )
    .unwrap();

    //Delete latest selected note from db
    db::operations::set_latest_note(&conn, None);

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn restore_note(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<(), CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    let mut note = db::operations::get_note(
        &conn,
        id,
        state.workspace.clone().unwrap().master_encryption_key,
    )
    .unwrap();
    note.deleted = false;

    db::operations::update_note(
        &conn,
        note,
        state.workspace.clone().unwrap().master_encryption_key,
    )
    .unwrap();

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_latest_note_id(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<String>, CommandError> {
    let state = state.lock().await;
    let conn = state.database.lock().await;

    Ok(db::operations::get_latest_note(&conn))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn handle_conflict(
    state: State<'_, Mutex<AppState>>,
    handle: AppHandle,
    id: String,
    local: bool
) -> Result<(), CommandError> {
    //Handle the conflict with note. 
    //  Either keep the note local (local=1) or replace with the server one (local=0)
    //TODO: should this be inside `sync`?

    let state = state.lock().await;
    let workspace = state.workspace.clone().unwrap();

    match local {
        true => {
            //Send to server with force
            let mut note = {
                let conn = state.database.lock().await;
                let mut note = Note::select(&conn, id).unwrap().unwrap();

                note.synched = true;
                note.update(&conn).unwrap();

                note
            };

            note.updated_at = Local::now().to_utc().timestamp();

            let sent_notes = SentNotes {
                username: workspace.username.unwrap(),
                notes: vec![note.into()],
                token: workspace.token.unwrap(),
                force: true
            };

            let results = sync::operations::send_notes(sent_notes, workspace.instance.unwrap()).await.unwrap();
            results.into_iter().for_each(|result| {
                match result.status {
                    shared::NoteStatus::Ok => {},
                    shared::NoteStatus::Conflict(conflicted_note) => {
                        error!("Conflict in conflict handling, this shouldn't happen lol: {:?}", conflicted_note)
                    }
                }
            });
            
            debug!("conflicted note has been sent to server")
        },
        false => {
            //Get server note and replace local one.
            let params = SelectNoteParams {
                username: workspace.username.clone().unwrap(),
                token: hex::encode(workspace.token.clone().unwrap()),
                note_id: id,
            };

            let note = sync::operations::select_note(params, workspace.instance.unwrap()).await.unwrap();

            {
                let conn = state.database.lock().await;

                let note = db::schema::Note::from(note);
                note.update(&conn).unwrap();
                
                let all_notes = db::operations::get_notes(&conn, workspace.id.unwrap()).unwrap();
                let notes_metadata: Vec<NoteMetadata> = all_notes.into_iter().map(NoteMetadata::from).collect();
                
                handle.emit("new_note_metadata", &notes_metadata).unwrap();
            }
                debug!("conflicted note has been saved locally")
        },
    }

    Ok(())
}