use anyhow::{Context, Result};
use rusqlite::Connection;
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

/// Result of merging one server note into the local database.
enum MergeOutcome {
    Stored,
    Unchanged,
    Conflict(db::schema::Note),
}

/// Merges a note received from the server into `workspace_id`.
///
/// The lookup is workspace-scoped: the same uuid may legitimately exist in another
/// workspace on this installation and must not shadow this workspace's row.
fn merge_received_note(
    conn: &Connection,
    workspace_id: u32,
    note: shared::Note,
) -> Result<MergeOutcome> {
    let mut note = db::schema::Note::from(note);
    note.id_workspace = Some(workspace_id);

    match Note::select(conn, note.uuid.clone(), workspace_id)
        .context("Failed to look up note in database")?
    {
        Some(local) => {
            if note.updated_at > local.updated_at {
                match local.synched {
                    true => {
                        note.update(conn).context("Failed to update received note")?;
                        Ok(MergeOutcome::Stored)
                    }
                    false => Ok(MergeOutcome::Conflict(note)),
                }
            } else {
                Ok(MergeOutcome::Unchanged)
            }
        }
        None => {
            note.insert(conn).context("Failed to insert received note")?;
            Ok(MergeOutcome::Stored)
        }
    }
}

/// Fetches notes updated after `last_seen` from the server, stores them locally,
/// and emits `new_note_metadata`. Returns the highest `server_received_at` among received notes.
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

        match merge_received_note(&conn, workspace.id, note)? {
            MergeOutcome::Stored | MergeOutcome::Unchanged => {}
            MergeOutcome::Conflict(note) => {
                info!("Note {:?} is in conflict (client side)", note.uuid);

                let decrypted_note = decrypt_note_for_emit(&note, &workspace)?;
                emit(handle, "conflict", decrypted_note);
            }
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
/// Returns the highest `server_received_at` among sent notes.
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
                    let mut note = Note::select(&conn, result.uuid.clone(), workspace.id)
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
    updated_workspace: Workspace,
    timestamp: i64,
) -> Result<()> {
    let mut state = state.lock().await;
    let last_sync_at = timestamp + 1;

    {
        let conn = state.database.lock().await;
        Workspace::update_last_sync_at(&conn, updated_workspace.id, last_sync_at)
            .context("Failed to persist last sync timestamp")?;
    }

    // Only touch this field, the passed-in workspace can be a stale snapshot from
    // earlier in the sync tick and must not clobber concurrent changes to other fields.
    if let Some(workspace) = state.workspace.as_mut() {
        workspace.last_sync_at = last_sync_at;
    }

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
        pinned: metadata.pinned,
        content: String::from_utf8(content_plaintext).context("Note content is not valid UTF-8")?,
        updated_at: note.updated_at,
        deleted: note.deleted,
    };

    Ok(commands::NoteResponse::from(note_data))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        Note::create(&conn).unwrap();
        Workspace::create(&conn).unwrap();

        for id in [1u32, 2u32] {
            conn.execute(
                "INSERT INTO workspace (id, workspace_name, last_sync_at) VALUES (?, ?, 0)",
                (id, format!("ws{id}")),
            )
            .unwrap();
        }

        conn
    }

    fn local_note(workspace_id: u32, uuid: &str) -> Note {
        Note {
            uuid: uuid.to_string(),
            id_workspace: Some(workspace_id),
            content: vec![1, 2, 3],
            nonce: vec![4, 5, 6],
            metadata: vec![7, 8, 9],
            metadata_nonce: vec![10, 11, 12],
            updated_at: 100,
            synched: true,
            deleted: false,
        }
    }

    fn shared_note(uuid: &str, updated_at: i64) -> shared::Note {
        shared::Note {
            uuid: uuid.to_string(),
            content: vec![9, 8, 7],
            nonce: vec![6, 5, 4],
            metadata: vec![3, 2, 1],
            metadata_nonce: vec![0],
            updated_at,
            server_received_at: 0,
            deleted: false,
        }
    }

    #[test]
    fn merge_received_note_inserts_when_uuid_exists_in_another_workspace() {
        let conn = open_db();

        // The uuid already exists locally, but in workspace 1.
        local_note(1, "shared-uuid").insert(&conn).unwrap();

        let incoming = shared_note("shared-uuid", 100);
        let outcome = merge_received_note(&conn, 2, incoming).unwrap();

        assert!(matches!(outcome, MergeOutcome::Stored));

        let ws2 = Note::select(&conn, "shared-uuid".to_string(), 2).unwrap().unwrap();
        assert_eq!(ws2.content, vec![9, 8, 7]);
        assert_eq!(ws2.id_workspace, Some(2));
        assert!(ws2.synched);

        let ws1 = Note::select(&conn, "shared-uuid".to_string(), 1).unwrap().unwrap();
        assert_eq!(ws1.content, vec![1, 2, 3]);
    }

    #[test]
    fn merge_received_note_updates_synced_local_note_when_server_newer() {
        let conn = open_db();

        local_note(1, "n1").insert(&conn).unwrap();

        let incoming = shared_note("n1", 200);
        assert!(matches!(
            merge_received_note(&conn, 1, incoming).unwrap(),
            MergeOutcome::Stored
        ));

        let fetched = Note::select(&conn, "n1".to_string(), 1).unwrap().unwrap();
        assert_eq!(fetched.content, vec![9, 8, 7]);
        assert_eq!(fetched.updated_at, 200);
    }

    #[test]
    fn merge_received_note_reports_conflict_when_local_unsynced_and_server_newer() {
        let conn = open_db();

        let mut local = local_note(1, "n1");
        local.synched = false;
        local.content = vec![1, 1, 1];
        local.insert(&conn).unwrap();

        let incoming = shared_note("n1", 200);
        match merge_received_note(&conn, 1, incoming).unwrap() {
            MergeOutcome::Conflict(note) => assert_eq!(note.content, vec![9, 8, 7]),
            _ => panic!("expected a conflict outcome"),
        }

        let fetched = Note::select(&conn, "n1".to_string(), 1).unwrap().unwrap();
        assert_eq!(fetched.content, vec![1, 1, 1]);
        assert_eq!(fetched.updated_at, 100);
    }

    #[test]
    fn merge_received_note_ignores_older_or_equal_server_note() {
        let conn = open_db();

        local_note(1, "n1").insert(&conn).unwrap();

        assert!(matches!(
            merge_received_note(&conn, 1, shared_note("n1", 100)).unwrap(),
            MergeOutcome::Unchanged
        ));
        assert!(matches!(
            merge_received_note(&conn, 1, shared_note("n1", 99)).unwrap(),
            MergeOutcome::Unchanged
        ));

        let fetched = Note::select(&conn, "n1".to_string(), 1).unwrap().unwrap();
        assert_eq!(fetched.content, vec![1, 2, 3]);
        assert_eq!(fetched.updated_at, 100);
    }
}
