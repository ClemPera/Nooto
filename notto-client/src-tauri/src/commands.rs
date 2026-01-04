use tokio::sync::Mutex;

use serde::Serialize;
use tauri::State;
use tauri_plugin_log::log::{debug, trace};

use crate::{AppState, crypt, sync};
use crate::crypt::NoteData;
use crate::db;
use crate::db::schema::{Note, Workspace};

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
    fn from(workspace: Workspace) -> Self{
        FilteredWorkspace {
            id: workspace.id.unwrap(),
            workspace_name: workspace.workspace_name
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NoteMetadata {
    pub id: u32,
    pub title: String,
    pub updated_at: i64,
}

impl From<Note> for NoteMetadata {
    fn from(note: Note) -> Self {
        NoteMetadata {
            id: note.id.unwrap(),
            title: note.title,
            updated_at: note.updated_at*1000 //Convert to TS timestamps
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn init(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError>  {
    let mut state = state.lock().await;

    let workspace = {
        let conn = state.database.lock().await;
        db::operations::get_logged_workspace(&conn)
    };

    state.workspace = workspace;

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn create_note(state: State<'_, Mutex<AppState>>, title: String) -> Result<(), CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    let workspace = state.workspace.clone().unwrap();

    db::operations::create_note(&conn, workspace.id.unwrap(), title, workspace.master_encryption_key).unwrap();

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_note(state: State<'_, Mutex<AppState>>, id: u32) -> Result<NoteData, CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;
    
    let note = db::operations::get_note(&conn, id, state.workspace.clone().unwrap().master_encryption_key).unwrap();

    Ok(note)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn edit_note(state: State<'_, Mutex<AppState>>, note: NoteData) -> Result<(), CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    db::operations::update_note(&conn, note, state.workspace.clone().unwrap().master_encryption_key).unwrap();

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_all_notes_metadata(state: State<'_, Mutex<AppState>>, id_workspace: u32) -> Result<Vec<NoteMetadata>, CommandError> {    
    let state = state.lock().await;

    let conn = state.database.lock().await;

    let notes = db::operations::get_notes(&conn, id_workspace).unwrap();

    let notes_metadata = notes.into_iter().map(NoteMetadata::from).collect();
    
    Ok(notes_metadata)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn create_workspace(state: State<'_, Mutex<AppState>>, username: String) -> Result<(), CommandError> {
    let mut state = state.lock().await;

    let workspace = {
        let conn = state.database.lock().await;
        db::operations::create_workspace(&conn, username).unwrap()
    };

    state.workspace = Some(workspace);

    debug!("user created");
    
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_workspaces(state: State<'_, Mutex<AppState>>) -> Result<Vec<FilteredWorkspace>, CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;
    
    let workspaces = db::operations::get_workspaces(&conn).unwrap();

    let filtered_worspaces = workspaces.into_iter().map(FilteredWorkspace::from).collect();

    Ok(filtered_worspaces)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn test(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError> {
    let state = state.lock().await;
    
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn set_logged_workspace(state: State<'_, Mutex<AppState>>, workspace_name: String) -> Result<FilteredWorkspace, CommandError> {
    let mut state = state.lock().await;
    
    let workspace = match workspace_name.is_empty() {
        false => {
            let workspace = {
                let conn = state.database.lock().await;
                match db::operations::get_workspace(&conn, workspace_name).unwrap() {
                    Some(u) => u,
                    None => return Err(CommandError { message: "Workspace doesn't exist".to_string() })
                }
            };
        
            Some(workspace)
        },
        true => None
    };

    state.workspace = workspace.clone();

    let conn = state.database.lock().await;
    db::operations::set_logged_workspace(&conn, workspace.clone());

    let workspace = workspace.unwrap();
    
    Ok(FilteredWorkspace { id: workspace.id.unwrap(), workspace_name: workspace.workspace_name })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_logged_workspace(state: State<'_, Mutex<AppState>>) -> Result<Option<FilteredWorkspace>, CommandError> {
    let state = state.lock().await;

    match &state.workspace {
        Some(w) => Ok(Some(FilteredWorkspace {
            id: w.id.unwrap(),
            workspace_name: w.workspace_name.clone()
        })),
        None => Ok(None)
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_create_account(state: State<'_, Mutex<AppState>>, worspace_name: String, password: String, instance: Option<String>) -> Result<(), CommandError> {
    trace!("create account command received");
    
    let mut state = state.lock().await;
    
    let conn = state.database.lock().await;
    let workspace = db::operations::get_workspace(&conn, worspace_name).unwrap().unwrap();
    let account = crypt::create_account(password, state.workspace.clone().unwrap().master_encryption_key);
    
    trace!("create account: start creating");
    sync::create_account(workspace, account, instance).await;
    
    debug!("account has been created");

    //TODO: send back recovery key to frontend
    
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_login(state: State<'_, Mutex<AppState>>, username: String, workspace_name: String, password: String, instance: Option<String>) -> Result<(), CommandError> {
    trace!("login command received");

    let mut state = state.lock().await;

    let instance = match instance {
        Some(i) => i,
        None => "http://localhost:3000".to_string()
    };

    let login_data = sync::login(username.clone(), password.clone(), instance.clone()).await;

    debug!("account has been logged in");

    let mut workspace = {
        let conn = state.database.lock().await;
        db::operations::create_workspace(&conn, workspace_name.clone()).unwrap();
        db::operations::get_workspace(&conn, workspace_name).unwrap().unwrap()
    };

    trace!("create workspace = ok");

    let mek = crypt::decrypt_mek(password, login_data.encrypted_mek_password, login_data.salt_data, login_data.mek_password_nonce);

    trace!("mek decrypted");
    
    workspace.master_encryption_key = mek;
    workspace.token = Some(login_data.token.clone());
    workspace.instance = Some(instance.clone());

    state.workspace = Some(workspace.clone());

    trace!("state modified: {state:?}");

    {
        let conn = state.database.lock().await;
        db::operations::update_workspace(&conn, workspace);
    }

    trace!("workspace modified");

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
