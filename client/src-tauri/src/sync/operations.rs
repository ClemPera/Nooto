use anyhow::{Context, Result};
use shared::{
    Image, LoginRequestParams, Note, SelectImageParams, SelectNoteImagesParams, SelectNoteParams,
    SelectNotesParams, SendImage, SentNotes, User,
};

/// `POST /notes` — uploads a batch of notes and returns per-note results (Ok or Conflict).
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

/// `GET /notes` — fetches notes updated after the timestamp in `params`.
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

/// `GET /note` — fetches a single note by UUID.
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

/// `POST /image` — uploads a single encrypted image.
pub async fn send_image(image: SendImage, instance: String) -> Result<()> {
    let client = reqwest::Client::new();

    client
        .post(instance + "/image")
        .json(&image)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected the image")?;

    Ok(())
}

/// `GET /image` — fetches a single image by UUID.
pub async fn select_image(params: SelectImageParams, instance: String) -> Result<Image> {
    let client = reqwest::Client::new();

    let response = client
        .get(instance + "/image")
        .query(&params)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected the image request")?;

    response
        .json()
        .await
        .context("Failed to parse server response")
}

/// `DELETE /image` — deletes a single image by UUID. A no-op server-side if it doesn't exist.
pub async fn delete_image(params: SelectImageParams, instance: String) -> Result<()> {
    let client = reqwest::Client::new();

    client
        .delete(instance + "/image")
        .query(&params)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected the delete request")?;

    Ok(())
}

/// `GET /images` — lists every image UUID the server currently has for a note.
pub async fn select_note_images(params: SelectNoteImagesParams, instance: String) -> Result<Vec<String>> {
    let client = reqwest::Client::new();

    let response = client
        .get(instance + "/images")
        .query(&params)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Server rejected the image list request")?;

    response
        .json()
        .await
        .context("Failed to parse server response")
}

/// `POST /create_account` — registers a new user on the server.
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

/// `GET /login` — retrieves the salts needed to derive the login hash before authenticating.
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

/// `POST /login` — submits the login hash and returns the session token and MEK material.
pub async fn login(params: shared::LoginParams, instance: String) -> Result<shared::Login> {
    let client = reqwest::Client::new();

    let response = client
        .post(instance + "/login")
        .json(&params)
        .send()
        .await
        .context("Could not reach the server")?
        .error_for_status()
        .context("Login failed")?;

    response
        .json()
        .await
        .context("Failed to parse login response")
}
