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
    pub server_received_at: i64,
    pub deleted: bool
}

/// Query parameters for `GET /notes`
#[derive(Deserialize, Serialize, Debug)]
pub struct SelectNotesParams {
    pub username: String,
    pub updated_at: i64
}

/// Query parameters for `GET /note`
#[derive(Deserialize, Serialize, Debug)]
pub struct SelectNoteParams {
    pub username: String,
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
#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum NoteStatus {
    Ok(i64),
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

/// Payload for `POST /logout` — revokes a single session token.
#[derive(Deserialize, Serialize, Debug)]
pub struct LogoutParams {
    pub username: String,
    pub token: Vec<u8>,
}

/// Bumped only on breaking wire changes; UI-only releases must not bump it.
pub const PROTOCOL_VERSION: u32 = 1;

/// Header carrying the client's protocol version on every API request.
pub const PROTOCOL_VERSION_HEADER: &str = "x-nooto-protocol-version";

/// Server information returned by `GET /version`.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ServerInfo {
    pub server_version: String,
    pub protocol_version: u32,
    pub min_client_protocol_version: u32,
}

/// Returned by the server when the client's protocol version is not accepted.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProtocolMismatch {
    pub code: String,
    pub client_outdated: bool,
    pub client_protocol_version: Option<u32>,
    pub server: Option<ServerInfo>,
}

/// Result of comparing a client protocol version against a server's accepted range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolCompatibility {
    Compatible,
    ClientOutdated,
    ServerOutdated,
}

impl ServerInfo {
    pub fn check_client(&self, client_protocol_version: u32) -> ProtocolCompatibility {
        if client_protocol_version < self.min_client_protocol_version {
            ProtocolCompatibility::ClientOutdated
        } else if client_protocol_version > self.protocol_version {
            ProtocolCompatibility::ServerOutdated
        } else {
            ProtocolCompatibility::Compatible
        }
    }
}

impl ProtocolMismatch {
    /// Builds the payload; a missing client or server predates protocol checks.
    pub fn new(client_protocol_version: Option<u32>, server: Option<ServerInfo>) -> Self {
        let client_outdated = match (&server, client_protocol_version) {
            (Some(info), Some(client)) => {
                info.check_client(client) == ProtocolCompatibility::ClientOutdated
            }
            (Some(_), None) => true,
            (None, _) => false,
        };

        ProtocolMismatch {
            code: "protocol_mismatch".to_string(),
            client_outdated,
            client_protocol_version,
            server,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server_info(protocol_version: u32, min_client_protocol_version: u32) -> ServerInfo {
        ServerInfo {
            server_version: "0.3.0".to_string(),
            protocol_version,
            min_client_protocol_version,
        }
    }

    #[test]
    fn client_below_minimum_is_outdated() {
        let info = server_info(2, 2);
        assert_eq!(info.check_client(1), ProtocolCompatibility::ClientOutdated);
    }

    #[test]
    fn client_above_server_is_server_outdated() {
        let info = server_info(1, 1);
        assert_eq!(info.check_client(2), ProtocolCompatibility::ServerOutdated);
    }

    #[test]
    fn client_in_accepted_range_is_compatible() {
        let info = server_info(2, 1);
        assert_eq!(info.check_client(1), ProtocolCompatibility::Compatible);
        assert_eq!(info.check_client(2), ProtocolCompatibility::Compatible);
    }

    #[test]
    fn missing_client_version_marks_client_outdated() {
        let mismatch = ProtocolMismatch::new(None, Some(server_info(1, 1)));
        assert!(mismatch.client_outdated);
    }

    #[test]
    fn newer_client_does_not_mark_client_outdated() {
        let mismatch = ProtocolMismatch::new(Some(2), Some(server_info(1, 1)));
        assert!(!mismatch.client_outdated);
    }

    #[test]
    fn missing_server_does_not_mark_client_outdated() {
        let mismatch = ProtocolMismatch::new(Some(1), None);
        assert!(!mismatch.client_outdated);
        assert_eq!(mismatch.code, "protocol_mismatch");
    }
}
