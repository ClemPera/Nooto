use mysql_async::{
    Conn, FromRowError, Row, params,
    prelude::{FromRow, Queryable, WithParams},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct Note {
    pub uuid: Vec<u8>,
    pub id_user: Option<u32>, //Server id user
    pub title: String,
    pub content: Vec<u8>,
    pub nonce: Vec<u8>,
    pub updated_at: i64,
}

impl FromRow for Note {
    fn from_row_opt(row: Row) -> Result<Self, FromRowError> {
        Ok(Note {
            uuid: row.get(0).ok_or(FromRowError(row.clone()))?,
            id_user: row.get(1).ok_or(FromRowError(row.clone()))?,
            title: row.get(2).ok_or(FromRowError(row.clone()))?,
            content: row.get(3).ok_or(FromRowError(row.clone()))?,
            nonce: row.get(4).ok_or(FromRowError(row.clone()))?,
            updated_at: row.get(5).ok_or(FromRowError(row.clone()))?,
        })
    }
}

impl From<shared::Note> for Note {
    fn from(note: shared::Note) -> Self {
        Note {
            uuid: note.uuid,
            id_user: None,
            title: note.title,
            content: note.content,
            nonce: note.nonce,
            updated_at: note.updated_at
        }
    }
}

impl Into<shared::Note> for Note {
    fn into(self) -> shared::Note {
        shared::Note {
            uuid: self.uuid,
            content: self.content,
            nonce: self.nonce,
            title: self.title,
            updated_at: self.updated_at
        }
    }
}

impl Note {
    //TODO: pub async fn create(&self, conn: &mut Conn) {}

    pub async fn select(&self, conn: &mut Conn) -> Option<Self> {
        conn.exec_first(
            "SELECT * FROM note WHERE uuid = :uuid",
            params!(
                "uuid" => &self.uuid
            ),
        )
        .await
        .unwrap()
    }

    pub async fn insert(&self, conn: &mut Conn) {
        conn.exec_drop(
            "INSERT INTO note (uuid, id_user, title, content, nonce, updated_at) 
            VALUES (:uuid, :id_user, :title, :content, :nonce, :updated_at)",
            params!(
                "uuid" => &self.uuid,
                "id_user" => &self.id_user,
                "title" => &self.title,
                "content" => &self.content,
                "nonce" => &self.nonce,
                "updated_at" => &self.updated_at
            ),
        )
        .await
        .unwrap();
    }

    pub async fn update(&self, conn: &mut Conn) {
        conn.exec_drop(
            "UPDATE note 
            SET title = :title, content = :content, nonce = :nonce, updated_at = :updated_at 
            WHERE uuid = :uuid",
            params!(
                "title" => &self.title,
                "content" => &self.content,
                "nonce" => &self.nonce,
                "updated_at" => &self.updated_at,
                "uuid" => &self.uuid
            ),
        )
        .await
        .unwrap();
    }

    pub async fn select_all_from_user(conn: &mut Conn, id_user: u32, after_datetime: i64) -> Vec<Self> {
        conn.exec(
            "SELECT * FROM note WHERE id_user = :id_user AND updated_at >= :updated_at",
            params!(
                "id_user" => id_user,
                "updated_at" => after_datetime
            ),
        )
        .await
        .unwrap()
    }
}

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

    pub async fn select(conn: &mut Conn, username: String) -> Option<Self> {
        conn.exec_first(
            "SELECT * FROM user WHERE username = :username",
            params!(
                "username" => username
            ),
        )
        .await
        .unwrap()
    }

    pub async fn insert(&self, conn: &mut Conn) {
        conn.exec_drop("INSERT INTO user (username, stored_password_hash, stored_recovery_hash, encrypted_mek_password, mek_password_nonce,
                encrypted_mek_recovery, mek_recovery_nonce, salt_auth, salt_data, salt_recovery_auth, salt_recovery_data, salt_server_auth, salt_server_recovery) 
            VALUES (:username, :stored_password_hash, :stored_recovery_hash, :encrypted_mek_password, :mek_password_nonce, :encrypted_mek_recovery, :mek_recovery_nonce, :salt_auth, 
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
            )).await.unwrap();
    }
}

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

    pub async fn insert(&self, conn: &mut Conn) {
        conn.exec_drop(
            "INSERT INTO user_token (id_user, token) 
            VALUES (:id_user, :token)",
            params!(
                "id_user" => &self.id_user,
                "token" => &self.token,
            ),
        )
        .await
        .unwrap();
    }

    pub async fn select(conn: &mut Conn, id: u32) -> Vec<Self> {
        conn.exec(
            "SELECT * FROM user_token WHERE id_user = :id_user",
            params!(
                "id_user" => id
            ),
        )
        .await
        .unwrap()
    }
}
