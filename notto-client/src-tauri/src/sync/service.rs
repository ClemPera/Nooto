use std::{thread, time::Duration};

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::error;
use shared::{SelectNoteParams, SentNotes};
use tokio::sync::{Mutex, MutexGuard};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::log::{debug, error, trace, warn};

use crate::{AppState, db::{self, schema::Note}, sync};

#[derive(Clone, Serialize)]
pub enum SyncStatus {
    Synched,
    Syncing,
    Error(String),
    Offline,
    NotConnected
}

pub async fn run(handle: AppHandle) {
    let state = handle.state::<Mutex<AppState>>();
    let mut last_sync = DateTime::<Utc>::MIN_UTC.timestamp();
    loop{
        {
            let state = state.lock().await;

            if let Some(workspace) = state.workspace.clone() {
                if workspace.id.is_some() && workspace.token.is_some() && workspace.instance.is_some() {
                    //Update sync infos
                    let sync = Local::now().to_utc().timestamp();
    
                    //Sync
                    match receive_latest_notes(&state, last_sync).await {
                        Ok(_) => {},
                        Err(e) => {
                            if let Some(e) = e.downcast_ref::<reqwest::Error>() {
                                if e.is_connect() {
                                    handle.emit("sync-status", SyncStatus::Offline).unwrap();
                                    warn!("Couldn't connect to server");
                                }
                                else{
                                    handle.emit("sync-status", SyncStatus::Error(e.to_string())).unwrap();
                                    error!("{e}")
                                }
                            }else{
                                handle.emit("sync-status", SyncStatus::Error(e.to_string())).unwrap();
                                error!("{e}")
                            }
                        }
                    };

                    match send_latest_notes(&state, &handle).await {
                        Ok(_) => {},
                        Err(e) => {
                            if let Some(e) = e.downcast_ref::<reqwest::Error>() {
                                if e.is_connect() {
                                    warn!("Couldn't connect to server");
                                    handle.emit("sync-status", SyncStatus::Offline).unwrap();
                                }
                                else{
                                    handle.emit("sync-status", SyncStatus::Error(e.to_string())).unwrap();
                                    error!("{e}")
                                }
                            }else{
                                handle.emit("sync-status", SyncStatus::Error(e.to_string())).unwrap();
                                error!("{e}")
                            }
                        }
                    };
    
                    handle.emit("sync-status", SyncStatus::Synched).unwrap();
                    
                    last_sync = sync;
                }else {
                    // trace!("Conditions are not respected to sync {state:?}");
                    handle.emit("sync-status", SyncStatus::NotConnected).unwrap();
                    last_sync = DateTime::<Utc>::MIN_UTC.timestamp();
                }
            }
        }

        thread::sleep(Duration::from_secs(1));
    }
}


pub async fn receive_latest_notes(state: &MutexGuard<'_, AppState>, last_sync: i64) -> Result<(), Box<dyn std::error::Error>> {
    let conn = state.database.lock().await;
    
    let workspace = state.workspace.clone().unwrap();

    let params = SelectNoteParams {
        username: workspace.username.unwrap(),
        token: hex::encode(workspace.token.unwrap()), 
        updated_at: last_sync
    };
    
    //Ask server for modified notes
    let notes = sync::operations::select_notes(params, workspace.instance.unwrap()).await?;

    trace!("notes received : {notes:?}");
    
    // Put new notes to database
    notes.into_iter().for_each(|note| {
        let mut note = db::schema::Note::from(note);
        note.id_workspace = workspace.id;
        
        //Check if exist
        let selected_note = db::schema::Note::select(&conn, note.uuid.clone()).unwrap();

        match selected_note {
            Some(sn) => {
                if note.updated_at > sn.updated_at {
                    //Note is more recent on server
                    match sn.synched {
                        true => note.update(&conn).unwrap(),
                        false => error!("Note {:?} is in conflict and it's not handled :(", sn.uuid) //TODO
                    };
                }
            },
            None => note.insert(&conn).unwrap()
        }

        //TODO: if deleted
    });

    Ok(())
}

pub async fn send_latest_notes(state: &MutexGuard<'_, AppState>, handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let conn = state.database.lock().await;

    let workspace = state.workspace.clone().unwrap();
    
    //Fetch db find all notes with synched = false;
    let notes = Note::select_all(&conn, workspace.id.unwrap()).unwrap();

    //TODO: Optimise that with a database query
    let notes: Vec<Note> = notes.into_iter().filter(|note| !note.synched).collect();

    if !notes.is_empty() {
        handle.emit("sync-status", SyncStatus::Syncing).unwrap();

        let sent_notes = SentNotes {
            username: workspace.username.unwrap(),
            notes: notes.into_iter().map(|n| n.into()).collect(),
            token: workspace.token.unwrap()
        };

        //Send server these notes
        let results = sync::operations::send_notes(sent_notes, workspace.instance.unwrap()).await?;

        //Handle Results
        results.into_iter().for_each(|result| {
            match result.status {
                shared::NoteStatus::Ok => {
                    let mut note = Note::select(&conn, result.uuid).unwrap().unwrap();

                    note.synched = true;

                    note.update(&conn).unwrap();
                },
                shared::NoteStatus::Conflict => {
                    //TODO
                    error!("Note {:?} is in conflict and it's not handled :(", result.uuid) 
                }
            }
        });
    }
    
    Ok(())
}