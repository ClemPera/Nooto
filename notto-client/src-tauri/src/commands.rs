use tokio::sync::Mutex;

use serde::Serialize;
use tauri::State;
use tauri_plugin_log::log::{debug, trace};

use crate::{AppState, crypt, sync};
use crate::crypt::NoteData;
use crate::db;
use crate::db::schema::{Note, User};

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
pub struct FilteredUser {
    pub id: u32,
    pub username: String,
}

impl From<User> for FilteredUser {
    fn from(user: User) -> Self{
        FilteredUser {
            id: user.id.unwrap(),
            username: user.username
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
            updated_at: note.updated_at
        }
    }
}

#[tauri::command]
pub async fn init(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError>  {
    let mut state = state.lock().await;

    let user = {
        let conn = state.database.lock().await;
        db::operations::get_logged_user(&conn)
    };

    state.user = user;

    Ok(())
}

#[tauri::command]
pub async fn create_note(state: State<'_, Mutex<AppState>>, title: String) -> Result<(), CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    let user = state.user.clone().unwrap();

    db::operations::create_note(&conn, user.id.unwrap(), title, user.master_encryption_key).unwrap();

    Ok(())
}

#[tauri::command]
pub async fn get_note(state: State<'_, Mutex<AppState>>, id: u32) -> Result<NoteData, CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;
    
    let note = db::operations::get_note(&conn, id, state.user.clone().unwrap().master_encryption_key).unwrap();

    Ok(note)
}

#[tauri::command]
pub async fn edit_note(state: State<'_, Mutex<AppState>>, note: NoteData) -> Result<(), CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;

    db::operations::update_note(&conn, note, state.user.clone().unwrap().master_encryption_key).unwrap();

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_all_notes_metadata(state: State<'_, Mutex<AppState>>, id_user: u32) -> Result<Vec<NoteMetadata>, CommandError> {    
    let state = state.lock().await;

    let conn = state.database.lock().await;

    let notes = db::operations::get_notes(&conn, id_user).unwrap();

    let notes_metadata = notes.into_iter().map(NoteMetadata::from).collect();
    
    Ok(notes_metadata)
}

#[tauri::command]
pub async fn create_user(state: State<'_, Mutex<AppState>>, username: String) -> Result<(), CommandError> {
    let mut state = state.lock().await;

    let user = {
        let conn = state.database.lock().await;
        db::operations::create_user(&conn, username).unwrap()
    };

    state.user = Some(user);

    debug!("user created");
    
    Ok(())
}

#[tauri::command]
pub async fn get_users(state: State<'_, Mutex<AppState>>) -> Result<Vec<FilteredUser>, CommandError> {
    let state = state.lock().await;

    let conn = state.database.lock().await;
    
    let users = db::operations::get_users(&conn).unwrap();

    let filtered_users= users.into_iter().map(FilteredUser::from).collect();

    Ok(filtered_users)
}

#[tauri::command]
pub async fn test(state: State<'_, Mutex<AppState>>) -> Result<(), CommandError> {
    let state = state.lock().await;
    
    Ok(())
}

#[tauri::command]
pub async fn set_logged_user(state: State<'_, Mutex<AppState>>, username: String) -> Result<(), CommandError> {
    let mut state = state.lock().await;
    
    let user = match username.is_empty() {
        false => {
            let user = {
                let conn = state.database.lock().await;
                match db::operations::get_user(&conn, username).unwrap() {
                    Some(u) => u,
                    None => return Err(CommandError { message: "User doesn't exist".to_string() })
                }
            };
        
            Some(user)
        },
        true => None
    };

    state.user = user.clone();

    let conn = state.database.lock().await;
    db::operations::set_logged_user(&conn, user);

    Ok(())
}

#[tauri::command]
pub async fn get_logged_user(state: State<'_, Mutex<AppState>>) -> Result<Option<FilteredUser>, CommandError> {
    let state = state.lock().await;

    match &state.user {
        Some(u) => Ok(Some(FilteredUser {
            id: u.id.unwrap(),
            username: u.username.clone()
        })),
        None => Ok(None)
    }
}


#[tauri::command(rename_all = "snake_case")]
pub async fn sync_create_account(state: State<'_, Mutex<AppState>>, username: String, password: String, instance: Option<String>) -> Result<(), CommandError> {
    trace!("create account command received");
    
    let mut state = state.lock().await;
    
    let conn = state.database.lock().await;
    let user = db::operations::get_user(&conn, username).unwrap().unwrap();
    let account = crypt::create_account(password, state.user.clone().unwrap().master_encryption_key);
    
    trace!("create account: start creating");
    sync::create_account(user, account, instance).await;
    
    debug!("account has been created");

    //TODO: send back recovery key to frontend
    
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_login(state: State<'_, Mutex<AppState>>, username: String, password: String, instance: Option<String>) -> Result<bool, CommandError> {
    trace!("login command received");

    let mut state = state.lock().await;

    let instance = match instance {
        Some(i) => i,
        None => "http://localhost:3000".to_string()
    };

    let login_data = sync::login(username.clone(), password.clone(), instance.clone()).await;

    debug!("account has been logged in");

    let mut user = {
        let conn = state.database.lock().await;
        match db::operations::get_user(&conn, username).unwrap() {
            Some(u) => u,
            None => return Err(CommandError { message: "User doesn't exist".to_string() })
        }
    };

    trace!("get user = ok");

    //TODO: if !user.has_mek() then do not decrypt mek?
    //TODO: handle if user account not created locally?

    let mek = crypt::decrypt_mek(password, login_data.encrypted_mek_password, login_data.salt_data, login_data.mek_password_nonce);

    trace!("mek encrypted");
    
    user.master_encryption_key = mek;
    user.token = Some(login_data.token.clone());
    user.instance = Some(instance.clone());

    state.user = Some(user.clone());

    trace!("state modified");

    {
        let conn = state.database.lock().await;
        db::operations::update_user(&conn, user);
    }

    trace!("user modified");

    Ok(true)
}