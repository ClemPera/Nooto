use tokio::sync::Mutex;

use serde::Serialize;
use tauri::State;
use tauri_plugin_log::log::{debug, trace};
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
}

impl From<Note> for NoteMetadata {
    fn from(note: Note) -> Self {
        NoteMetadata {
            id: Uuid::from_slice(note.uuid.as_slice()).unwrap().to_string(),
            title: note.title,
            updated_at: note.updated_at * 1000, //Convert to TS timestamps
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
) -> Result<(), CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    let workspace = state.workspace.clone().unwrap();

    db::operations::create_note(
        &conn,
        workspace.id.unwrap(),
        title,
        workspace.master_encryption_key,
    )
    .unwrap();

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_note(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<NoteData, CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    let note = db::operations::get_note(
        &conn,
        Uuid::parse_str(&id).unwrap().as_bytes().to_vec(),
        state.workspace.clone().unwrap().master_encryption_key,
    )
    .unwrap();

    Ok(note)
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
    let notes = {
        let conn = state.database.lock().await;
        db::operations::get_notes(&conn, workspace.id.unwrap()).unwrap()
    };

    if !notes.is_empty() {
        let notes: Vec<NoteData> = notes
            .into_iter()
            .map(|n| crypt::decrypt_note(n, mek).unwrap())
            .collect();

        {
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
