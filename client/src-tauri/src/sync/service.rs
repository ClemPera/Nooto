use anyhow::{Context, Result};
use serde::Serialize;
use shared::{ProtocolMismatch, SelectNotesParams, SentNotes};
use std::time::Instant;
use tokio::{sync::Mutex, time::Duration};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::log::{debug, error, info};

use crate::{
    AppState, commands,
    crypt::{self, NoteData},
    db::{
        self,
        schema::{Note, Workspace},
    },
    sync,
};

/// Interval between each sync
const SYNC_INTERVAL: Duration = Duration::from_secs(1);

/// How often the server protocol version is checked during normal sync.
const VERSION_CHECK_INTERVAL: Duration = Duration::from_secs(300);
/// How often the server protocol version is rechecked while sync is blocked.
const VERSION_RECHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Sync state emitted to the frontend via the `sync-status` Tauri event.
#[derive(Clone, Serialize)]
pub enum SyncStatus {
    Synched,
    Syncing,
    Error,
    Offline,
    NotConnected,
    UpgradeRequired,
}

/// Reports a failed sync request; return true if ProtocolMismatch error
async fn handle_sync_error(state: &Mutex<AppState>, handle: &AppHandle, e: &anyhow::Error) -> bool {
    if let Some(mismatch) = e.downcast_ref::<ProtocolMismatch>() {
        emit(handle, "version-mismatch", mismatch.clone());
        emit(handle, "sync-status", SyncStatus::UpgradeRequired);
        return true;
    }

    if clear_session_if_unauthorized(state, handle, e).await {
        return false;
    }

    if e.downcast_ref::<reqwest::Error>()
        .map_or(false, |e| e.is_connect())
    {
        emit(handle, "sync-status", SyncStatus::Offline);
        info!("Couldn't connect to server");
    } else {
        emit(handle, "sync-status", SyncStatus::Error);
        error!("{e:#}");
    }

    false
}

/// Clears stored credentials and notifies the frontend when the server rejects the session.
/// Returns true when the caller should stop this sync tick.
async fn clear_session_if_unauthorized(
    state: &Mutex<AppState>,
    handle: &AppHandle,
    e: &anyhow::Error,
) -> bool {
    let status = e
        .downcast_ref::<reqwest::Error>()
        .and_then(reqwest::Error::status);

    if !matches!(
        status,
        Some(reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN)
    ) {
        return false;
    }

    info!("Session rejected by server, clearing stored credentials");

    if let Err(e) = commands::clear_session(state).await {
        error!("Failed to clear session: {e:?}");
    }

    emit(handle, "sync-status", SyncStatus::NotConnected);
    emit(handle, "session-expired", ());
    true
}

/// Protocol compatibility state carried between sync ticks.
#[derive(Default)]
struct ProtocolState {
    checked_instance: Option<String>,
    last_check: Option<Instant>,
    blocked: bool,
}

impl ProtocolState {
    /// Whether the server protocol should be checked for this instance at `now`.
    fn check_due(&self, instance: &str, now: Instant) -> bool {
        let interval = if self.blocked {
            VERSION_RECHECK_INTERVAL
        } else {
            VERSION_CHECK_INTERVAL
        };

        self.checked_instance.as_deref() != Some(instance)
            || self
                .last_check
                .map_or(true, |at| now.duration_since(at) >= interval)
    }
}

/// Background sync loop: every second, pulls new notes from the server then pushes unsynced ones.
/// Emits `sync-status` and `new_note_metadata` events to the frontend as state changes.
pub async fn run(handle: AppHandle) {
    let state = handle.state::<Mutex<AppState>>();
    let mut protocol = ProtocolState::default();

    loop {
        sync_tick(&state, &handle, &mut protocol).await;
        tokio::time::sleep(SYNC_INTERVAL).await;
    }
}

/// One sync attempt: checks protocol compatibility, then pulls and pushes notes.
async fn sync_tick(state: &Mutex<AppState>, handle: &AppHandle, protocol: &mut ProtocolState) {
    let workspace = {
        let state = state.lock().await;
        state.workspace.clone()
    };
    let Some(workspace) = workspace else {
        return;
    };

    let (Some(_), Some(instance)) = (&workspace.token, &workspace.instance) else {
        emit(handle, "sync-status", SyncStatus::NotConnected);
        return;
    };

    if !check_protocol(state, handle, protocol, instance).await {
        return;
    }

    if !sync_notes(state, handle, &workspace, protocol).await {
        return;
    }

    emit(handle, "sync-status", SyncStatus::Synched);
}

/// Checks the server protocol when due; returns false when this tick must stop.
async fn check_protocol(
    state: &Mutex<AppState>,
    handle: &AppHandle,
    protocol: &mut ProtocolState,
    instance: &str,
) -> bool {
    if protocol.blocked {
        emit(handle, "sync-status", SyncStatus::UpgradeRequired);
    }

    if !protocol.check_due(instance, Instant::now()) {
        return !protocol.blocked;
    }

    match sync::version::ensure_server_compatible(instance).await {
        Ok(_) => {
            protocol.checked_instance = Some(instance.to_owned());
            protocol.last_check = Some(Instant::now());

            if protocol.blocked {
                info!("Server protocol check passed, resuming sync");
                protocol.blocked = false;
            }

            true
        }
        Err(e) => {
            if e.downcast_ref::<ProtocolMismatch>().is_some() || !protocol.blocked {
                if handle_sync_error(state, handle, &e).await {
                    protocol.checked_instance = Some(instance.to_owned());
                    protocol.blocked = true;
                }
            } else {
                error!("Protocol recheck failed: {e:#}");
            }

            protocol.last_check = Some(Instant::now());
            false
        }
    }
}

/// Pulls remote notes then pushes local ones; returns false when this tick must stop.
async fn sync_notes(
    state: &Mutex<AppState>,
    handle: &AppHandle,
    workspace: &Workspace,
    protocol: &mut ProtocolState,
) -> bool {
    let received =
        receive_latest_notes(state, workspace.clone(), workspace.last_sync_at, handle).await;

    if !apply_sync_result(state, handle, workspace, protocol, received).await {
        return false;
    }

    let sent = send_latest_notes(state, workspace.clone(), handle).await;
    apply_sync_result(state, handle, workspace, protocol, sent).await
}

/// Advances `last_sync_at` when the request returned a timestamp; reports failures.
async fn apply_sync_result(
    state: &Mutex<AppState>,
    handle: &AppHandle,
    workspace: &Workspace,
    protocol: &mut ProtocolState,
    result: Result<Option<i64>>,
) -> bool {
    match result {
        Ok(Some(timestamp)) => match update_last_sync(state, workspace.clone(), timestamp).await {
            Ok(()) => true,
            Err(e) => {
                error!("{e:#}");
                emit(handle, "sync-status", SyncStatus::Error);
                false
            }
        },
        Ok(None) => true,
        Err(e) => {
            if handle_sync_error(state, handle, &e).await {
                protocol.blocked = true;
                protocol.last_check = Some(Instant::now());
            }

            false
        }
    }
}

/// Emits a Tauri event, logging on failure (non-critical path).
fn emit<S: Serialize + Clone>(handle: &AppHandle, event: &str, payload: S) {
    if let Err(e) = handle.emit(event, payload) {
        error!("Failed to emit '{}' event: {e}", event);
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
    let token = workspace.token.clone().context("Workspace has no token")?;

    let params = SelectNotesParams {
        username: workspace
            .username
            .clone()
            .context("Workspace has no username")?,
        updated_at: last_seen,
    };

    let notes = sync::operations::select_notes(
        params,
        &token,
        workspace
            .instance
            .clone()
            .context("Workspace has no instance")?,
    )
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

        match Note::select(&conn, note.uuid.clone())
            .context("Failed to look up note in database")?
        {
            Some(sn) => {
                if note.updated_at > sn.updated_at {
                    match sn.synched {
                        true => note
                            .update(&conn)
                            .context("Failed to update received note")?,
                        false => {
                            info!("Note {:?} is in conflict (client side)", note.uuid);

                            let decrypted_note = decrypt_note_for_emit(&note, &workspace)?;
                            emit(handle, "conflict", decrypted_note);
                        }
                    }
                }
            }
            None => note
                .insert(&conn)
                .context("Failed to insert received note")?,
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
            username: workspace
                .username
                .clone()
                .context("Workspace has no username")?,
            notes: unsynced_notes.into_iter().map(|n| n.into()).collect(),
            token: workspace.token.clone().context("Workspace has no token")?,
            force: false,
        };

        let results = sync::operations::send_notes(
            sent_notes,
            workspace
                .instance
                .clone()
                .context("Workspace has no instance")?,
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
                    note.update(&conn)
                        .context("Failed to mark note as synched")?;

                    max_server_received_at = Some(
                        max_server_received_at
                            .map_or(server_received_at, |m: i64| m.max(server_received_at)),
                    );
                }
                shared::NoteStatus::Conflict(conflicted_note) => {
                    info!(
                        "Note {:?} is in conflict (server side)",
                        conflicted_note.uuid
                    );
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
    let content_plaintext =
        crypt::decrypt_data(&note.content, &note.nonce, &workspace.master_encryption_key)
            .context("Failed to decrypt conflicted note content")?;
    let metadata_plaintext = crypt::decrypt_data(
        &note.metadata,
        &note.metadata_nonce,
        &workspace.master_encryption_key,
    )
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

    fn protocol_state(
        blocked: bool,
        checked_instance: Option<&str>,
        checked_ago: Option<Duration>,
    ) -> (ProtocolState, Instant) {
        let now = Instant::now();

        (
            ProtocolState {
                checked_instance: checked_instance.map(str::to_owned),
                last_check: checked_ago.map(|ago| now - ago),
                blocked,
            },
            now,
        )
    }

    #[test]
    fn check_due_when_never_checked() {
        let (protocol, now) = protocol_state(false, None, None);
        assert!(protocol.check_due("https://example.com", now));
    }

    #[test]
    fn check_due_when_instance_changed() {
        let (protocol, now) = protocol_state(
            false,
            Some("https://old.example.com"),
            Some(Duration::from_secs(1)),
        );
        assert!(protocol.check_due("https://new.example.com", now));
    }

    #[test]
    fn check_due_respects_normal_interval() {
        let (protocol, now) = protocol_state(
            false,
            Some("https://example.com"),
            Some(VERSION_CHECK_INTERVAL - Duration::from_secs(1)),
        );
        assert!(!protocol.check_due("https://example.com", now));

        let (protocol, now) = protocol_state(
            false,
            Some("https://example.com"),
            Some(VERSION_CHECK_INTERVAL),
        );
        assert!(protocol.check_due("https://example.com", now));
    }

    #[test]
    fn check_due_respects_blocked_interval() {
        let (protocol, now) = protocol_state(
            true,
            Some("https://example.com"),
            Some(VERSION_RECHECK_INTERVAL - Duration::from_secs(1)),
        );
        assert!(!protocol.check_due("https://example.com", now));

        let (protocol, now) = protocol_state(
            true,
            Some("https://example.com"),
            Some(VERSION_RECHECK_INTERVAL),
        );
        assert!(protocol.check_due("https://example.com", now));
    }
}
