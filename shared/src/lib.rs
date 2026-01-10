use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct User {
    pub id: Option<u32>,
    pub username: String,
    pub stored_password_hash: String,
    pub stored_recovery_hash: String,
    pub encrypted_mek_password: Vec<u8>,
    pub mek_password_nonce: Vec<u8>,
    pub encrypted_mek_recovery: Vec<u8>,
    pub mek_recovery_nonce: Vec<u8>,
    pub salt_auth: String,
    pub salt_data: String,
    pub salt_recovery_auth: String,
    pub salt_recovery_data: String,
    pub salt_server_auth: String,
    pub salt_server_recovery: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Note {
    pub uuid: Vec<u8>,
    pub title: String,
    pub content: Vec<u8>,
    pub nonce: Vec<u8>,
    pub updated_at: i64,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SelectNoteParams {
    pub username: String,
    pub token: String,
    pub updated_at: i64
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SentNotes {
    pub notes: Vec<Note>,
    pub token: Vec<u8>,
    pub username: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum NoteStatus {
    Ok,
    Conflict,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SentNotesResult {
    pub uuid: Vec<u8>,
    pub status: NoteStatus
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LoginRequestParams {
    pub username: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LoginRequest {
    pub salt_auth: String,
    pub salt_server_auth: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LoginParams {
    pub username: String,
    pub login_hash: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Login {
    pub salt_data: String,
    pub encrypted_mek_password: Vec<u8>,
    pub mek_password_nonce: Vec<u8>,
    pub token: Vec<u8>,
}
