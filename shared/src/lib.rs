use serde::{Deserialize, Serialize};

/// Full user record exchanged between client and server during account creation.
#[derive(Deserialize, Serialize, Debug, Clone)]
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

/// Encrypted note as transmitted over the wire (content and metadata are ciphertext blobs).
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Note {
    pub uuid: String,
    pub content: Vec<u8>,
    pub nonce: Vec<u8>,
    pub metadata: Vec<u8>,
    pub metadata_nonce: Vec<u8>,
    pub updated_at: i64,
    pub deleted: bool
}

/// Query parameters for `GET /notes` — fetches notes updated after `updated_at`.
#[derive(Deserialize, Serialize, Debug)]
pub struct SelectNotesParams {
    pub username: String,
    pub token: String,
    pub updated_at: i64
}

/// Query parameters for `GET /note` — fetches a single note by UUID.
#[derive(Deserialize, Serialize, Debug)]
pub struct SelectNoteParams {
    pub username: String,
    pub token: String,
    pub note_id: String
}

/// Payload for `POST /notes` — a batch of notes to upsert on the server.
/// Set `force` to overwrite conflicts without confirmation.
#[derive(Deserialize, Serialize, Debug)]
pub struct SentNotes {
    pub notes: Vec<Note>,
    pub token: Vec<u8>,
    pub username: String,
    pub force: bool
}

/// Per-note outcome returned by `POST /notes`.
/// `Conflict` carries the server version so the client can present a diff.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum NoteStatus {
    Ok,
    Conflict(Note),
}

/// Server's response for a single note in a batch upload.
#[derive(Deserialize, Serialize, Debug)]
pub struct SentNotesResult {
    pub uuid: String,
    pub status: NoteStatus
}

/// Query parameters for `GET /login` — requests the salts needed to derive the login hash.
#[derive(Deserialize, Serialize, Debug)]
pub struct LoginRequestParams {
    pub username: String,
}

/// Server response to `GET /login` containing the salts required for password hashing.
#[derive(Deserialize, Serialize, Debug)]
pub struct LoginRequest {
    pub salt_auth: String,
    pub salt_server_auth: String,
}

/// Payload for `POST /login`.
#[derive(Deserialize, Serialize, Debug)]
pub struct LoginParams {
    pub username: String,
    pub login_hash: String,
}

/// Successful login response — contains the data needed to decrypt the master encryption key.
#[derive(Deserialize, Serialize, Debug)]
pub struct Login {
    pub salt_data: String,
    pub encrypted_mek_password: Vec<u8>,
    pub mek_password_nonce: Vec<u8>,
    pub token: Vec<u8>,
}
