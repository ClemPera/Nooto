use anyhow::{Context, Result};
use shared::{LoginRequestParams, Note, SelectNoteParams, SelectNotesParams, SentNotes, User};
use tauri_plugin_log::log::{debug, trace};

pub async fn send_notes(notes: SentNotes, instance: String) -> Result<Vec<shared::SentNotesResult>> {
    let client = reqwest::Client::new();

    let response = client
        .post(instance + "/notes")
        .json(&notes)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected the notes")?;

    response
        .json()
        .await
        .context("Failed to parse server response")
}

pub async fn select_notes(params: SelectNotesParams, instance: String) -> Result<Vec<Note>> {
    let client = reqwest::Client::new();

    let response = client
        .get(instance + "/notes")
        .query(&params)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected the notes request")?;

    response
        .json()
        .await
        .context("Failed to parse server response")
}

pub async fn select_note(params: SelectNoteParams, instance: String) -> Result<Note> {
    let client = reqwest::Client::new();

    let response = client
        .get(instance + "/note")
        .query(&params)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected the note request")?;

    response
        .json()
        .await
        .context("Failed to parse server response")
}

pub async fn create_account(user: User, instance: String) -> Result<()> {
    let client = reqwest::Client::new();

    client
        .post(instance + "/create_account")
        .json(&user)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected account creation")?;

    Ok(())
}

pub async fn login_request(params: LoginRequestParams, instance: String) -> Result<shared::LoginRequest> {
    let client = reqwest::Client::new();

    let response = client
        .get(instance + "/login")
        .query(&params)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected login request")?;

    response
        .json()
        .await
        .context("Failed to parse login request response")
}

pub async fn login(params: shared::LoginParams, instance: String) -> Result<shared::Login> {
    let client = reqwest::Client::new();

    let response = client
        .post(instance + "/login")
        .json(&params)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Login failed - check your credentials")?;

    response
        .json()
        .await
        .context("Failed to parse login response")
}
