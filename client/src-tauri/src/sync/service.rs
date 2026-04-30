use anyhow::{Context, Result};
use serde::Serialize;
use shared::{SelectNotesParams, SentNotes};
use tokio::{sync::Mutex, time::Duration};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::log::{debug, error, info};

use crate::{
    AppState, commands,
    crypt::{self, NoteData},
    db::{self, schema::{Note, Workspace}},
    sync,
};

/// Sync state emitted to the frontend via the `sync-status` Tauri event.
#[derive(Clone, Serialize)]
pub enum SyncStatus {
    Synched,
    Syncing,
    Error,
    Offline,
    NotConnected,
}

/// Background sync loop: every second, pulls new notes from the server then pushes unsynced ones.
/// Emits `sync-status` and `new_note_metadata` events to the frontend as state changes.
pub async fn run(handle: AppHandle) {
    let state = handle.state::<Mutex<AppState>>();

    loop {
        'sync: {
            let workspace = {
                let state = state.lock().await;
                state.workspace.clone()
            };

            if let Some(workspace) = workspace {
                if workspace.token.is_some() && workspace.instance.is_some() {
                    let current_last_seen = workspace.last_sync_at;

                    match receive_latest_notes(&state, workspace.clone(), current_last_seen, &handle).await {
                        Ok(max_ts) => {
                            if let Some(ts) = max_ts {
                                if let Err(e) = update_last_sync(&state, workspace.clone(), ts).await {
                                    error!("{e:#}");
                                    emit(&handle, "sync-status", SyncStatus::Error);
                                    break 'sync;
                                }
                            }
                        }
                        Err(e) => {
                            if e.downcast_ref::<reqwest::Error>().map_or(false, |e| e.is_connect()) {
                                emit(&handle, "sync-status", SyncStatus::Offline);
                                info!("Couldn't connect to server");
                            } else {
                                emit(&handle, "sync-status", SyncStatus::Error);
                                error!("{e:#}");
                            }
                            break 'sync;
                        }
                    }

                    match send_latest_notes(&state, workspace.clone(), &handle).await {
                        Ok(max_ts) => {
                            if let Some(ts) = max_ts {
                                if let Err(e) = update_last_sync(&state, workspace.clone(), ts).await {
                                    error!("{e:#}");
                                    emit(&handle, "sync-status", SyncStatus::Error);
                                    break 'sync;
                                }
                            }
                        }
                        Err(e) => {
                            if e.downcast_ref::<reqwest::Error>().map_or(false, |e| e.is_connect()) {
                                emit(&handle, "sync-status", SyncStatus::Offline);
                                info!("Couldn't connect to server");
                            } else {
                                emit(&handle, "sync-status", SyncStatus::Error);
                                error!("{e:#}");
                            }
                            break 'sync;
                        }
                    }

                    emit(&handle, "sync-status", SyncStatus::Synched);
                } else {
                    emit(&handle, "sync-status", SyncStatus::NotConnected);
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Emits a Tauri event, logging on failure (non-critical path).
fn emit<S: Serialize + Clone>(handle: &AppHandle, event: &str, payload: S) {
    if let Err(e) = handle.emit(event, payload) {
        error!("Failed to emit '{}' event: {e}", event);
    }
}

/// Fetches notes updated after `last_seen` from the server, stores them locally,
/// and emits `new_note_metadata`. Returns the highest `updated_at` among received notes.
pub async fn receive_latest_notes(
    state: &Mutex<AppState>,
    workspace: Workspace,
    last_seen: i64,
    handle: &AppHandle,
) -> Result<Option<i64>> {
    let params = SelectNotesParams {
        username: workspace.username.clone().context("Workspace has no username")?,
        token: hex::encode(workspace.token.clone().context("Workspace has no token")?),
        updated_at: last_seen,
    };

    let notes = sync::operations::select_notes(params, workspace.instance.clone().context("Workspace has no instance")?)
        .await?;

    if notes.is_empty() {
        return Ok(None);
    }

    let max_updated_at = notes.iter().map(|n| n.server_received_at).max();

    let state = state.lock().await;
    let conn = state.database.lock().await;

    for note in notes {
        debug!("note received: {}, {}", note.uuid, note.updated_at);

        let mut note = db::schema::Note::from(note);
        note.id_workspace = Some(workspace.id);

        match Note::select(&conn, note.uuid.clone()).context("Failed to look up note in database")? {
            Some(sn) => {
                if note.updated_at > sn.updated_at {
                    match sn.synched {
                        true => note.update(&conn).context("Failed to update received note")?,
                        false => {
                            info!("Note {:?} is in conflict (client side)", note.uuid);

                            let decrypted_note = decrypt_note_for_emit(&note, &workspace)?;
                            emit(handle, "conflict", decrypted_note);
                        }
                    }
                }
            }
            None => note.insert(&conn).context("Failed to insert received note")?,
        }
    }

    let all_notes = db::operations::get_notes(&conn, workspace.id)?;

    let notes_metadata = all_notes
        .into_iter()
        .map(|n| commands::NoteMetadata::from_note(n, &workspace.master_encryption_key))
        .collect::<Result<Vec<_>>>()?;

    emit(handle, "new_note_metadata", &notes_metadata);

    Ok(max_updated_at)
}

/// Collects all unsynced local notes and pushes them to the server.
/// Marks successfully uploaded notes as synced; emits `conflict` for any conflicting ones.
/// Returns the highest `updated_at` among sent notes.
pub async fn send_latest_notes(
    state: &Mutex<AppState>,
    workspace: Workspace,
    handle: &AppHandle,
) -> Result<Option<i64>> {
    let unsynced_notes: Vec<Note> = {
        let state = state.lock().await;
        let conn = state.database.lock().await;

        //TODO: Optimise that with a database query
        Note::select_all(&conn, workspace.id)
            .context("Failed to read notes from database")?
            .into_iter()
            .filter(|n| !n.synched)
            .collect()
    };

    let mut max_server_received_at: Option<i64> = None;

    if !unsynced_notes.is_empty() {
        debug!("sending modified notes...");

        emit(handle, "sync-status", SyncStatus::Syncing);

        let sent_notes = SentNotes {
            username: workspace.username.clone().context("Workspace has no username")?,
            notes: unsynced_notes.into_iter().map(|n| n.into()).collect(),
            token: workspace.token.clone().context("Workspace has no token")?,
            force: false,
        };

        let results = sync::operations::send_notes(
            sent_notes,
            workspace.instance.clone().context("Workspace has no instance")?,
        )
        .await?;

        let state = state.lock().await;
        let conn = state.database.lock().await;

        for result in results {
            match result.status {
                shared::NoteStatus::Ok(server_received_at) => {
                    let mut note = Note::select(&conn, result.uuid.clone())
                        .context("Failed to find sent note in database")?
                        .ok_or_else(|| anyhow::anyhow!("Sent note '{}' not found", result.uuid))?;
                    note.synched = true;
                    note.update(&conn).context("Failed to mark note as synched")?;

                    max_server_received_at = Some(
                        max_server_received_at
                            .map_or(server_received_at, |m: i64| m.max(server_received_at)),
                    );
                }
                shared::NoteStatus::Conflict(conflicted_note) => {
                    info!("Note {:?} is in conflict (server side)", conflicted_note.uuid);
                    let note = db::schema::Note::from(conflicted_note);
                    let decrypted_note = decrypt_note_for_emit(&note, &workspace)?;
                    emit(handle, "conflict", decrypted_note);
                }
            }
        }
    }

    Ok(max_server_received_at)
}

/// Advances `last_sync_at` to `timestamp + 1` in both the in-memory state and the database.
pub async fn update_last_sync(
    state: &Mutex<AppState>,
    mut updated_workspace: Workspace,
    timestamp: i64,
) -> Result<()> {
    let mut state = state.lock().await;

    updated_workspace.last_sync_at = timestamp + 1;
    state.workspace = Some(updated_workspace.clone());

    let conn = state.database.lock().await;
    updated_workspace
        .update(&conn)
        .context("Failed to persist last sync timestamp")?;

    Ok(())
}

/// Decrypts a note into a frontend-ready NoteResponse, used before emitting conflict events.
fn decrypt_note_for_emit(note: &Note, workspace: &Workspace) -> Result<commands::NoteResponse> {
    let content_plaintext = crypt::decrypt_data(&note.content, &note.nonce, &workspace.master_encryption_key)
        .context("Failed to decrypt conflicted note content")?;
    let metadata_plaintext = crypt::decrypt_data(&note.metadata, &note.metadata_nonce, &workspace.master_encryption_key)
        .context("Failed to decrypt conflicted note metadata")?;
    let metadata: crypt::NoteMetadata = serde_json::from_slice(&metadata_plaintext)
        .context("Failed to parse conflicted note metadata")?;

    let note_data = NoteData {
        id: note.uuid.clone(),
        title: metadata.title,
        parent_id: metadata.parent_id,
        is_folder: metadata.is_folder,
        folder_open: metadata.folder_open,
        content: String::from_utf8(content_plaintext).context("Note content is not valid UTF-8")?,
        updated_at: note.updated_at,
        deleted: note.deleted,
    };

    Ok(commands::NoteResponse::from(note_data))
}
