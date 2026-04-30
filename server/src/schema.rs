use anyhow::{Context, Result};
use mysql_async::{
    Conn, FromRowError, Row, params,
    prelude::{FromRow, Queryable},
};
use serde::{Deserialize, Serialize};

/// Server-side note row as stored in the `note` table.
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Note {
    pub uuid: String,
    pub id_user: Option<u32>,
    pub content: Vec<u8>,
    pub nonce: Vec<u8>,
    pub metadata: Vec<u8>,
    pub metadata_nonce: Vec<u8>,
    pub updated_at: i64,
    pub deleted: bool,
    pub server_received_at: i64,
}

impl FromRow for Note {
    fn from_row_opt(row: Row) -> Result<Self, FromRowError> {
        Ok(Note {
            uuid: row.get(0).ok_or(FromRowError(row.clone()))?,
            id_user: row.get(1).ok_or(FromRowError(row.clone()))?,
            content: row.get(2).ok_or(FromRowError(row.clone()))?,
            nonce: row.get(3).ok_or(FromRowError(row.clone()))?,
            metadata: row.get(4).ok_or(FromRowError(row.clone()))?,
            metadata_nonce: row.get(5).ok_or(FromRowError(row.clone()))?,
            updated_at: row.get(6).ok_or(FromRowError(row.clone()))?,
            deleted: row.get(7).ok_or(FromRowError(row.clone()))?,
            server_received_at: row.get(8).ok_or(FromRowError(row.clone()))?,
        })
    }
}

impl From<shared::Note> for Note {
    fn from(note: shared::Note) -> Self {
        Note {
            uuid: note.uuid,
            id_user: None,
            content: note.content,
            nonce: note.nonce,
            metadata: note.metadata,
            metadata_nonce: note.metadata_nonce,
            updated_at: note.updated_at,
            deleted: note.deleted,
            server_received_at: 0,
        }
    }
}

impl Into<shared::Note> for Note {
    fn into(self) -> shared::Note {
        shared::Note {
            uuid: self.uuid,
            content: self.content,
            nonce: self.nonce,
            metadata: self.metadata,
            metadata_nonce: self.metadata_nonce,
            updated_at: self.updated_at,
            server_received_at: self.server_received_at,
            deleted: self.deleted,
        }
    }
}

impl Note {
    //TODO: pub async fn create(&self, conn: &mut Conn) {}

    /// Fetches a single note by user ID and UUID. Returns `None` if not found.
    pub async fn select(conn: &mut Conn, id_user: u32, uuid: String) -> Result<Option<Self>> {
        conn.exec_first(
            "SELECT * FROM note WHERE id_user = :id_user AND uuid = :uuid",
            params!(
                "id_user" => id_user,
                "uuid" => uuid
            ),
        )
        .await
        .context("Failed to select note")
    }

    /// Inserts a new note row. `updated_at` is the client's logical edit time; `server_received_at` must be
    /// set by the caller to the current server time before calling.
    pub async fn insert(&self, conn: &mut Conn) -> Result<()> {
        conn.exec_drop(
            "INSERT INTO note (uuid, id_user, content, nonce, metadata, metadata_nonce, updated_at, deleted, server_received_at) \
            VALUES (:uuid, :id_user, :content, :nonce, :metadata, :metadata_nonce, :updated_at, :deleted, :server_received_at)",
            params!(
                "uuid" => &self.uuid,
                "id_user" => &self.id_user,
                "content" => &self.content,
                "nonce" => &self.nonce,
                "metadata" => &self.metadata,
                "metadata_nonce" => &self.metadata_nonce,
                "updated_at" => &self.updated_at,
                "deleted" => &self.deleted,
                "server_received_at" => &self.server_received_at,
            ),
        )
        .await
        .context("Failed to insert note")
    }

    /// Updates an existing note's content and metadata. `updated_at` is the client's logical edit time;
    /// `server_received_at` must be set by the caller to the current server time before calling.
    pub async fn update(&self, conn: &mut Conn) -> Result<()> {
        conn.exec_drop(
            "UPDATE note \
            SET content = :content, nonce = :nonce, metadata = :metadata, metadata_nonce = :metadata_nonce, updated_at = :updated_at, deleted = :deleted, server_received_at = :server_received_at \
            WHERE uuid = :uuid",
            params!(
                "content" => &self.content,
                "nonce" => &self.nonce,
                "metadata" => &self.metadata,
                "metadata_nonce" => &self.metadata_nonce,
                "updated_at" => &self.updated_at,
                "deleted" => &self.deleted,
                "server_received_at" => &self.server_received_at,
                "uuid" => &self.uuid,
            ),
        )
        .await
        .context("Failed to update note")
    }

    /// Returns all notes for a user received by the server after `after_datetime` (Unix timestamp).
    /// Filters on `server_received_at` so results are ordered by server arrival, not client clocks.
    pub async fn select_all_from_user(
        conn: &mut Conn,
        id_user: u32,
        after_datetime: i64,
    ) -> Result<Vec<Self>> {
        conn.exec(
            "SELECT * FROM note WHERE id_user = :id_user AND server_received_at > :after",
            params!(
                "id_user" => id_user,
                "after" => after_datetime
            ),
        )
        .await
        .context("Failed to select notes")
    }
}

/// Server-side user row as stored in the `user` table.
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

impl FromRow for User {
    fn from_row_opt(row: Row) -> Result<Self, FromRowError> {
        Ok(User {
            id: row.get(0).ok_or(FromRowError(row.clone()))?,
            username: row.get(1).ok_or(FromRowError(row.clone()))?,
            stored_password_hash: row.get(2).ok_or(FromRowError(row.clone()))?,
            stored_recovery_hash: row.get(3).ok_or(FromRowError(row.clone()))?,
            encrypted_mek_password: row.get(4).ok_or(FromRowError(row.clone()))?,
            mek_password_nonce: row.get(5).ok_or(FromRowError(row.clone()))?,
            encrypted_mek_recovery: row.get(6).ok_or(FromRowError(row.clone()))?,
            mek_recovery_nonce: row.get(7).ok_or(FromRowError(row.clone()))?,
            salt_auth: row.get(8).ok_or(FromRowError(row.clone()))?,
            salt_data: row.get(9).ok_or(FromRowError(row.clone()))?,
            salt_recovery_auth: row.get(10).ok_or(FromRowError(row.clone()))?,
            salt_recovery_data: row.get(11).ok_or(FromRowError(row.clone()))?,
            salt_server_auth: row.get(12).ok_or(FromRowError(row.clone()))?,
            salt_server_recovery: row.get(13).ok_or(FromRowError(row.clone()))?,
        })
    }
}

impl From<shared::User> for User {
    fn from(user: shared::User) -> Self {
        User {
            id: user.id,
            username: user.username,
            stored_password_hash: user.stored_password_hash,
            stored_recovery_hash: user.stored_recovery_hash,
            encrypted_mek_password: user.encrypted_mek_password,
            mek_password_nonce: user.mek_password_nonce,
            encrypted_mek_recovery: user.encrypted_mek_recovery,
            mek_recovery_nonce: user.mek_recovery_nonce,
            salt_auth: user.salt_auth,
            salt_data: user.salt_data,
            salt_recovery_auth: user.salt_recovery_auth,
            salt_recovery_data: user.salt_recovery_data,
            salt_server_auth: user.salt_server_auth,
            salt_server_recovery: user.salt_server_recovery,
        }
    }
}

impl User {
    //TODO: pub async fn create(&self, conn: &mut Conn) {}

    /// Fetches a user by username. Returns `None` if not found.
    pub async fn select(conn: &mut Conn, username: String) -> Result<Option<Self>> {
        conn.exec_first(
            "SELECT * FROM user WHERE username = :username",
            params!(
                "username" => username
            ),
        )
        .await
        .context("Failed to select user")
    }

    /// Inserts a new user row with all encryption material.
    pub async fn insert(&self, conn: &mut Conn) -> Result<()> {
        conn.exec_drop(
            "INSERT INTO user (username, stored_password_hash, stored_recovery_hash, encrypted_mek_password, mek_password_nonce,
                encrypted_mek_recovery, mek_recovery_nonce, salt_auth, salt_data, salt_recovery_auth, salt_recovery_data, salt_server_auth, salt_server_recovery) \
            VALUES (:username, :stored_password_hash, :stored_recovery_hash, :encrypted_mek_password, :mek_password_nonce, :encrypted_mek_recovery, :mek_recovery_nonce, :salt_auth, \
                :salt_data, :salt_recovery_auth, :salt_recovery_data, :salt_server_auth, :salt_server_recovery)",
            params!(
                "username" => &self.username,
                "stored_password_hash" => &self.stored_password_hash,
                "stored_recovery_hash" => &self.stored_recovery_hash,
                "encrypted_mek_password" => &self.encrypted_mek_password,
                "mek_password_nonce" => &self.mek_password_nonce,
                "encrypted_mek_recovery" => &self.encrypted_mek_recovery,
                "mek_recovery_nonce" => &self.mek_recovery_nonce,
                "salt_auth" => &self.salt_auth,
                "salt_data" => &self.salt_data,
                "salt_recovery_auth" => &self.salt_recovery_auth,
                "salt_recovery_data" => &self.salt_recovery_data,
                "salt_server_auth" => &self.salt_server_auth,
                "salt_server_recovery" => &self.salt_server_recovery,
            ),
        )
        .await
        .context("Failed to insert user")
    }
}

/// Session token row linking a random token to a user.
#[derive(Deserialize, Serialize, Debug)]
pub struct UserToken {
    pub id: Option<u32>,
    pub id_user: u32,
    pub token: Vec<u8>,
}

impl FromRow for UserToken {
    fn from_row_opt(row: Row) -> Result<Self, FromRowError> {
        Ok(UserToken {
            id: row.get(0).ok_or(FromRowError(row.clone()))?,
            id_user: row.get(1).ok_or(FromRowError(row.clone()))?,
            token: row.get(2).ok_or(FromRowError(row.clone()))?,
        })
    }
}

impl UserToken {
    //TODO: pub async fn create(&self, conn: &mut Conn) {}

    /// Inserts a new session token for the user.
    pub async fn insert(&self, conn: &mut Conn) -> Result<()> {
        conn.exec_drop(
            "INSERT INTO user_token (id_user, token) \
            VALUES (:id_user, :token)",
            params!(
                "id_user" => &self.id_user,
                "token" => &self.token,
            ),
        )
        .await
        .context("Failed to insert user token")
    }

    /// Returns all session tokens for the given user ID.
    pub async fn select(conn: &mut Conn, id: u32) -> Result<Vec<Self>> {
        conn.exec(
            "SELECT * FROM user_token WHERE id_user = :id_user",
            params!(
                "id_user" => id
            ),
        )
        .await
        .context("Failed to select user tokens")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_shared_note() -> shared::Note {
        shared::Note {
            uuid: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            content: vec![1, 2, 3, 4],
            nonce: vec![5, 6, 7, 8],
            metadata: vec![9, 10, 11],
            metadata_nonce: vec![12, 13, 14],
            updated_at: 1700000000,
            server_received_at: 0,
            deleted: false,
        }
    }

    fn sample_shared_user() -> shared::User {
        shared::User {
            id: Some(42),
            username: "alice".to_string(),
            stored_password_hash: "hash_abc".to_string(),
            stored_recovery_hash: "recovery_hash_abc".to_string(),
            encrypted_mek_password: vec![1, 2, 3],
            mek_password_nonce: vec![4, 5, 6],
            encrypted_mek_recovery: vec![7, 8, 9],
            mek_recovery_nonce: vec![10, 11, 12],
            salt_auth: "salt_auth".to_string(),
            salt_data: "salt_data".to_string(),
            salt_recovery_auth: "salt_recovery_auth".to_string(),
            salt_recovery_data: "salt_recovery_data".to_string(),
            salt_server_auth: "salt_server_auth".to_string(),
            salt_server_recovery: "salt_server_recovery".to_string(),
        }
    }

    // --- Note conversions ---

    #[test]
    fn note_from_shared_preserves_fields() {
        let shared = sample_shared_note();
        let note = Note::from(shared.clone());

        assert_eq!(note.uuid, shared.uuid);
        assert_eq!(note.content, shared.content);
        assert_eq!(note.nonce, shared.nonce);
        assert_eq!(note.metadata, shared.metadata);
        assert_eq!(note.metadata_nonce, shared.metadata_nonce);
        assert_eq!(note.updated_at, shared.updated_at);
        assert_eq!(note.deleted, shared.deleted);
    }

    #[test]
    fn note_from_shared_sets_id_user_to_none() {
        let note = Note::from(sample_shared_note());
        assert!(note.id_user.is_none());
    }

    #[test]
    fn note_into_shared_preserves_fields() {
        let note = Note {
            uuid: "test-uuid".to_string(),
            id_user: Some(1),
            content: vec![10, 20],
            nonce: vec![30, 40],
            metadata: vec![50, 60],
            metadata_nonce: vec![70, 80],
            updated_at: 9999,
            deleted: true,
            server_received_at: 1234,
        };

        let shared: shared::Note = note.clone().into();

        assert_eq!(shared.uuid, note.uuid);
        assert_eq!(shared.content, note.content);
        assert_eq!(shared.nonce, note.nonce);
        assert_eq!(shared.metadata, note.metadata);
        assert_eq!(shared.metadata_nonce, note.metadata_nonce);
        assert_eq!(shared.updated_at, note.updated_at);
        assert_eq!(shared.server_received_at, note.server_received_at);
        assert_eq!(shared.deleted, note.deleted);
    }

    #[test]
    fn note_roundtrip_from_shared_and_back() {
        let original = sample_shared_note();
        let server_note = Note::from(original.clone());
        let roundtripped: shared::Note = server_note.into();

        assert_eq!(roundtripped.uuid, original.uuid);
        assert_eq!(roundtripped.content, original.content);
        assert_eq!(roundtripped.nonce, original.nonce);
        assert_eq!(roundtripped.metadata, original.metadata);
        assert_eq!(roundtripped.metadata_nonce, original.metadata_nonce);
        assert_eq!(roundtripped.updated_at, original.updated_at);
        assert_eq!(roundtripped.deleted, original.deleted);
    }

    // --- User conversion ---

    #[test]
    fn user_from_shared_preserves_all_fields() {
        let shared = sample_shared_user();
        let user = User::from(shared.clone());

        assert_eq!(user.id, shared.id);
        assert_eq!(user.username, shared.username);
        assert_eq!(user.stored_password_hash, shared.stored_password_hash);
        assert_eq!(user.stored_recovery_hash, shared.stored_recovery_hash);
        assert_eq!(user.encrypted_mek_password, shared.encrypted_mek_password);
        assert_eq!(user.mek_password_nonce, shared.mek_password_nonce);
        assert_eq!(user.encrypted_mek_recovery, shared.encrypted_mek_recovery);
        assert_eq!(user.mek_recovery_nonce, shared.mek_recovery_nonce);
        assert_eq!(user.salt_auth, shared.salt_auth);
        assert_eq!(user.salt_data, shared.salt_data);
        assert_eq!(user.salt_recovery_auth, shared.salt_recovery_auth);
        assert_eq!(user.salt_recovery_data, shared.salt_recovery_data);
        assert_eq!(user.salt_server_auth, shared.salt_server_auth);
        assert_eq!(user.salt_server_recovery, shared.salt_server_recovery);
    }

    #[test]
    fn user_from_shared_without_id() {
        let mut shared = sample_shared_user();
        shared.id = None;
        let user = User::from(shared);
        assert!(user.id.is_none());
    }
}
