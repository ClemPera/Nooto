use serde::Serialize;
use shared::{SelectNotesParams, SentNotes};
use tokio::{sync::Mutex, time::Duration};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::log::{debug, error, info, trace, warn};

use crate::{AppState, commands, crypt::{self, NoteData}, db::{self, schema::{Note, Workspace}}, sync};

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

    loop {
        'sync: {
            let workspace = {
                let state = state.lock().await;
                state.workspace.clone()
            };

            if let Some(workspace) = workspace {
                if workspace.id.is_some() && workspace.token.is_some() && workspace.instance.is_some() {
                    // On first iteration (or after logout), init from the persisted value.
                    let current_last_seen = workspace.last_sync_at;

                    match receive_latest_notes(&state, workspace.clone(), current_last_seen, &handle).await {
                        Ok(max_ts) => {
                            if let Some(ts) = max_ts {
                                update_last_sync(&state, workspace.clone(), ts).await.unwrap();
                            }
                        },
                        Err(e) => {
                            if let Some(e) = e.downcast_ref::<reqwest::Error>() {
                                if e.is_connect() {
                                    handle.emit("sync-status", SyncStatus::Offline).unwrap();
                                    warn!("Couldn't connect to server");
                                    break 'sync;
                                } else {
                                    handle.emit("sync-status", SyncStatus::Error(e.to_string())).unwrap();
                                    error!("{e}");
                                    break 'sync;
                                }
                            } else {
                                handle.emit("sync-status", SyncStatus::Error(e.to_string())).unwrap();
                                error!("{e}");
                                break 'sync;
                            }
                        }
                    };

                    match send_latest_notes(&state, workspace.clone(), &handle).await {
                        Ok(max_ts) => {
                            if let Some(ts) = max_ts {
                                update_last_sync(&state, workspace.clone(), ts).await.unwrap();
                            }
                        },
                        Err(e) => {
                            if let Some(e) = e.downcast_ref::<reqwest::Error>() {
                                if e.is_connect() {
                                    handle.emit("sync-status", SyncStatus::Offline).unwrap();
                                    warn!("Couldn't connect to server");
                                    break 'sync;
                                } else {
                                    handle.emit("sync-status", SyncStatus::Error(e.to_string())).unwrap();
                                    error!("{e}");
                                    break 'sync;
                                }
                            } else {
                                handle.emit("sync-status", SyncStatus::Error(e.to_string())).unwrap();
                                error!("{e}");
                                break 'sync;
                            }
                        }
                    };

                    handle.emit("sync-status", SyncStatus::Synched).unwrap();
                } else {
                    handle.emit("sync-status", SyncStatus::NotConnected).unwrap();
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub async fn receive_latest_notes(
    state: &Mutex<AppState>,
    workspace: Workspace,
    last_seen: i64,
    handle: &AppHandle,
) -> Result<Option<i64>, Box<dyn std::error::Error + Send + Sync>> {
    let params = SelectNotesParams {
        username: workspace.username.clone().unwrap(),
        token: hex::encode(workspace.token.clone().unwrap()),
        updated_at: last_seen,
    };

    let notes = sync::operations::select_notes(params, workspace.instance.clone().unwrap()).await?;

    if notes.is_empty() {
        return Ok(None);
    }

    let max_updated_at = notes.iter().map(|n| n.updated_at).max();

    let state = state.lock().await;
    let conn = state.database.lock().await;

    notes.into_iter().for_each(|note| {
        debug!("note received: {}, {}", note.uuid, note.updated_at);

        let mut note = db::schema::Note::from(note);
        note.id_workspace = workspace.id;

        match db::schema::Note::select(&conn, note.uuid.clone()).unwrap() {
            Some(sn) => {
                if note.updated_at > sn.updated_at {
                    match sn.synched {
                        true => note.update(&conn).unwrap(),
                        false => {
                            info!("Note {:?} is in conflict (client side)", note.uuid);
                            
                            let content_plaintext = crypt::decrypt_data(&note.content, &note.nonce, &workspace.master_encryption_key).unwrap();
                            let metadata_plaintext = crypt::decrypt_data(&note.metadata, &note.metadata_nonce, &workspace.master_encryption_key).unwrap();
                            let metadata: crypt::NoteMetadata = serde_json::from_slice(&metadata_plaintext).unwrap();

                            let note_data = NoteData {
                                id: note.uuid.clone(),
                                title: metadata.title,
                                parent_id: metadata.parent_id,
                                is_folder: metadata.is_folder,
                                folder_open: metadata.folder_open,
                                content: String::from_utf8(content_plaintext).unwrap(),
                                updated_at: note.updated_at,
                                deleted: note.deleted,
                            };

                            let decrypted_note: commands::NoteResponse = note_data.into();

                            handle.emit("conflict", decrypted_note).unwrap();
                            handle.emit("sync-status", SyncStatus::Error("Conflict".to_string())).unwrap();
                        }
                    };
                }
            },
            None => note.insert(&conn).unwrap()
        }
    });

    let all_notes = db::operations::get_notes(&conn, workspace.id.unwrap()).unwrap();
    let notes_metadata: Vec<commands::NoteMetadata> = all_notes.into_iter().map(|n| commands::NoteMetadata::from_note(n, &workspace.master_encryption_key)).collect();

    handle.emit("new_note_metadata", &notes_metadata).unwrap();

    Ok(max_updated_at)
}

pub async fn send_latest_notes(
    state: &Mutex<AppState>,
    workspace: Workspace,
    handle: &AppHandle,
) -> Result<Option<i64>, Box<dyn std::error::Error + Send + Sync>> {
    let unsynced_notes: Vec<Note> = {
        let state = state.lock().await;
        let conn = state.database.lock().await;

        //TODO: Optimise that with a database query
        Note::select_all(&conn, workspace.id.unwrap()).unwrap()
            .into_iter().filter(|n| !n.synched).collect()
    };

    let max_updated_at = unsynced_notes.iter().map(|n| n.updated_at).max();

    if !unsynced_notes.is_empty() {
        debug!("sending modified notes...");

        handle.emit("sync-status", SyncStatus::Syncing).unwrap();

        let sent_notes = SentNotes {
            username: workspace.username.unwrap(),
            notes: unsynced_notes.into_iter().map(|n| n.into()).collect(),
            token: workspace.token.unwrap(),
            force: false
        };

        let results = sync::operations::send_notes(sent_notes, workspace.instance.unwrap()).await?;

        let state = state.lock().await;
        let conn = state.database.lock().await;

        results.into_iter().for_each(|result| {
            match result.status {
                shared::NoteStatus::Ok => {
                    let mut note = Note::select(&conn, result.uuid).unwrap().unwrap();
                    note.synched = true;
                    note.update(&conn).unwrap();
                },
                shared::NoteStatus::Conflict(conflicted_note) => {
                    info!("Note {:?} is in conflict (server side)", conflicted_note.uuid);

                    let note = db::schema::Note::from(conflicted_note);

                    let content_plaintext = crypt::decrypt_data(&note.content, &note.nonce, &workspace.master_encryption_key).unwrap();
                    let metadata_plaintext = crypt::decrypt_data(&note.metadata, &note.metadata_nonce, &workspace.master_encryption_key).unwrap();
                    let metadata: crypt::NoteMetadata = serde_json::from_slice(&metadata_plaintext).unwrap();

                    let note_data = NoteData {
                        id: note.uuid,
                        title: metadata.title,
                        parent_id: metadata.parent_id,
                        is_folder: metadata.is_folder,
                        folder_open: metadata.folder_open,
                        content: String::from_utf8(content_plaintext).unwrap(),
                        updated_at: note.updated_at,
                        deleted: note.deleted,
                    };

                    let decrypted_note: commands::NoteResponse = note_data.into();

                    handle.emit("conflict", decrypted_note).unwrap();
                }
            }
        });
    }

    Ok(max_updated_at)
}

pub async fn update_last_sync(state: &Mutex<AppState>, mut updated_workspace: Workspace, timestamp: i64) 
        -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut state = state.lock().await;
    
    updated_workspace.last_sync_at = timestamp+1;
    state.workspace = Some(updated_workspace.clone());
    
    let conn = state.database.lock().await;
    updated_workspace.update(&conn)?;

    Ok(())
}