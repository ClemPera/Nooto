use chrono::{DateTime, Utc};
use serde::Serialize;
use shared::{SelectNoteParams, SentNotes};
use tokio::{sync::Mutex, time::Duration};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::log::{debug, warn};

use crate::{AppState, commands, crypt, db::{self, schema::{Note, Workspace}}, sync};

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
    // Track highest updated_at received from the server rather than Local::now(),
    // to avoid clock skew between devices causing notes to be missed.
    let mut last_seen: i64 = DateTime::<Utc>::MIN_UTC.timestamp();

    loop {
        'sync: {
            let workspace = {
                let state = state.lock().await;
                state.workspace.clone()
            };

            if let Some(workspace) = workspace {
                if workspace.id.is_some() && workspace.token.is_some() && workspace.instance.is_some() {
                    match receive_latest_notes(&state, workspace.clone(), last_seen, &handle).await {
                        Ok(max_ts) => {
                            if let Some(ts) = max_ts {
                                last_seen = ts;
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

                    match send_latest_notes(&state, workspace, &handle).await {
                        Ok(_) => {},
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
                    last_seen = DateTime::<Utc>::MIN_UTC.timestamp();
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
) -> Result<Option<i64>, Box<dyn std::error::Error>> {
    let params = SelectNoteParams {
        username: workspace.username.clone().unwrap(),
        token: hex::encode(workspace.token.clone().unwrap()),
        updated_at: last_seen,
    };

    let notes = sync::operations::select_notes(params, workspace.instance.clone().unwrap()).await?;

    if notes.is_empty() {
        return Ok(None);
    }

    let max_updated_at = notes.iter().map(|n| n.updated_at).max();

    let mut detected_conflicts: Vec<(db::schema::Note, db::schema::Note)> = vec![];

    {
        let state = state.lock().await;
        let conn = state.database.lock().await;

        for note in notes {
            debug!("note received: {}, {}", note.title, note.updated_at);

            let mut db_note = db::schema::Note::from(note);
            db_note.id_workspace = workspace.id;

            match db::schema::Note::select(&conn, db_note.uuid.clone()).unwrap() {
                Some(local_note) => {
                    if db_note.updated_at > local_note.updated_at {
                        if local_note.synched {
                            db_note.update(&conn).unwrap();
                        } else {
                            detected_conflicts.push((db_note, local_note));
                        }
                    }
                },
                None => db_note.insert(&conn).unwrap(),
            }
        }

        let all_notes = db::operations::get_notes(&conn, workspace.id.unwrap()).unwrap();
        let notes_metadata: Vec<commands::NoteMetadata> = all_notes.into_iter().map(commands::NoteMetadata::from).collect();
        handle.emit("new_note_metadata", &notes_metadata).unwrap();
    }

    for (server_note, local_note) in detected_conflicts {
        let mek = {
            let state = state.lock().await;
            state.workspace.clone().unwrap().master_encryption_key
        };

        let local_decrypted = crypt::decrypt_note(local_note, mek)?;
        let server_decrypted = crypt::decrypt_note(server_note.clone(), mek)?;

        {
            let mut state = state.lock().await;
            state.conflicts.insert(server_note.uuid.clone(), server_note.into());
        }

        handle.emit("conflict", commands::ConflictPayload {
            uuid: local_decrypted.id.clone(),
            local: commands::ConflictNoteVersion {
                title: local_decrypted.title,
                content: local_decrypted.content,
                updated_at: local_decrypted.updated_at * 1000,
            },
            server: commands::ConflictNoteVersion {
                title: server_decrypted.title,
                content: server_decrypted.content,
                updated_at: server_decrypted.updated_at * 1000,
            },
        }).unwrap();
    }

    Ok(max_updated_at)
}

pub async fn send_latest_notes(
    state: &Mutex<AppState>,
    workspace: Workspace,
    handle: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let unsynced_notes: Vec<Note> = {
        let state = state.lock().await;
        let conn = state.database.lock().await;

        //TODO: Optimise that with a database query
        Note::select_all(&conn, workspace.id.unwrap()).unwrap()
            .into_iter().filter(|n| !n.synched).collect()
    };

    if !unsynced_notes.is_empty() {
        debug!("sending modified notes...");

        handle.emit("sync-status", SyncStatus::Syncing).unwrap();

        let sent_notes = SentNotes {
            username: workspace.username.unwrap(),
            notes: unsynced_notes.into_iter().map(|n| n.into()).collect(),
            token: workspace.token.unwrap(),
        };

        let results = sync::operations::send_notes(sent_notes, workspace.instance.unwrap()).await?;

        let mut conflict_results: Vec<(String, shared::Note)> = vec![];

        {
            let state = state.lock().await;
            let conn = state.database.lock().await;

            for result in results {
                match result.status {
                    shared::NoteStatus::Ok => {
                        let mut note = Note::select(&conn, result.uuid).unwrap().unwrap();
                        note.synched = true;
                        note.update(&conn).unwrap();
                    },
                    shared::NoteStatus::Conflict(server_note) => {
                        conflict_results.push((result.uuid, server_note));
                    }
                }
            }
        }

        for (uuid, server_note) in conflict_results {
            let (local_decrypted, mek) = {
                let state = state.lock().await;
                let mek = state.workspace.clone().unwrap().master_encryption_key;
                let conn = state.database.lock().await;
                let local_note = Note::select(&conn, uuid.clone()).unwrap().unwrap();
                (crypt::decrypt_note(local_note, mek)?, mek)
            };

            let server_db_note = Note::from(server_note.clone());
            let server_decrypted = crypt::decrypt_note(server_db_note, mek)?;

            {
                let mut state = state.lock().await;
                state.conflicts.insert(uuid.clone(), server_note);
            }

            handle.emit("conflict", commands::ConflictPayload {
                uuid,
                local: commands::ConflictNoteVersion {
                    title: local_decrypted.title,
                    content: local_decrypted.content,
                    updated_at: local_decrypted.updated_at * 1000,
                },
                server: commands::ConflictNoteVersion {
                    title: server_decrypted.title,
                    content: server_decrypted.content,
                    updated_at: server_decrypted.updated_at * 1000,
                },
            }).unwrap();
        }
    }

    Ok(())
}
