use shared::{LoginRequestParams, Note, SelectNoteParams, SelectNotesParams, SentNotes, User};
use tauri_plugin_log::log::{trace, debug};

pub async fn send_notes(notes: SentNotes, instance: String) -> Result<Vec<shared::SentNotesResult>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let response = client.post(instance + "/notes").json(&notes).send().await?.error_for_status()?;

    return Ok(response.json().await.unwrap())
}

pub async fn select_notes(params: SelectNotesParams, instance: String) -> Result<Vec<Note>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let response = client.get(instance + "/notes").query(&params).send().await?.error_for_status()?;

    Ok(response.json().await.unwrap())
}

pub async fn select_note(params: SelectNoteParams, instance: String) -> Result<Note, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let response = client.get(instance + "/note").query(&params).send().await?.error_for_status()?;

    Ok(response.json().await.unwrap())
}

pub async fn create_account(user: User, instance: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client.post(instance + "/create_account").json(&user).send().await.unwrap();

    Ok(())
}

pub async fn login_request(params: LoginRequestParams, instance: String) -> Result<shared::LoginRequest, Box<dyn std::error::Error>>{
    let client = reqwest::Client::new();

    let response = client.get(instance + "/login").query(&params).send().await?.json().await.unwrap();

    Ok(response)
}

pub async fn login(params: shared::LoginParams, instance: String) -> Result<shared::Login, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let response = client.post(instance + "/login").json(&params).send().await?.error_for_status()?;

    return Ok(response.json().await.unwrap())
}