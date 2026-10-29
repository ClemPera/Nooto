use std::sync::OnceLock;

use anyhow::Context;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::http::{HeaderMap, header::AUTHORIZATION};
use hmac::{Hmac, KeyInit, Mac};
use mysql_async::{Conn, Pool, params, prelude::Queryable};
use rand::{TryRng, rngs::SysRng};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::AppError;
use crate::constants;
use crate::schema;

/// Verifies `token` is valid and not idle-expired for `username`.
pub async fn user_verify(conn: &mut Conn, username: String, token: Vec<u8>) -> Result<(), AppError> {
    //TODO: this could return user honestly
    let user = schema::User::select(conn, username)
        .await
        .map_err(AppError::from)?
        .ok_or_else(AppError::unprocessable)?;

    let user_id = user
        .id
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    let user_tokens = schema::UserToken::select(conn, user_id)
        .await
        .map_err(AppError::from)?;

    let now = chrono::Local::now().to_utc().timestamp();

    for ut in user_tokens {
        if bool::from(ut.token.ct_eq(&token)) {
            let idle_secs = now - ut.last_used_at;

            if idle_secs > constants::SESSION_TOKEN_MAX_AGE_SECS {
                schema::UserToken::delete(conn, user_id, &ut.token)
                    .await
                    .map_err(AppError::from)?;
                return Err(AppError::unauthorized(
                    "Session expired, please log in again",
                ));
            }

            if idle_secs > constants::SESSION_TOKEN_REFRESH_AFTER_SECS {
                schema::UserToken::touch(conn, user_id, &ut.token, now)
                    .await
                    .map_err(AppError::from)?;
            }

            return Ok(());
        }
    }

    Err(AppError::forbidden())
}

/// Re-hashes login_hash with a fresh salt (defense in depth against a DB leak).
pub fn harden_login_hash(login_hash: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(login_hash.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to hash login_hash: {e}"))
}

/// Verifies login_hash against a hardened (Argon2id) stored_password_hash.
pub fn verify_login_hash(login_hash: &str, stored_password_hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(stored_password_hash).map_err(|e| {
        AppError::internal(anyhow::anyhow!("Failed to parse stored password hash: {e}"))
    })?;

    Ok(Argon2::default()
        .verify_password(login_hash.as_bytes(), &parsed)
        .is_ok())
}

/// One-time migration: rehashes accounts below CURRENT_PASSWORD_HASH_VERSION, then becomes a no-op.
//TODO: remove this fn, password_hash_version, and its write in insert_user() once no account is below version 2.
pub async fn rehash_legacy_password_hashes(pool: &Pool) -> anyhow::Result<()> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    let legacy_users = schema::User::select_outdated_password_hashes(
        &mut conn,
        constants::CURRENT_PASSWORD_HASH_VERSION,
    )
    .await?;

    if legacy_users.is_empty() {
        println!("Password hash migration: no legacy accounts to rehash");
        return Ok(());
    }

    let total = legacy_users.len();
    let mut migrated = 0;

    for user in legacy_users {
        let Some(user_id) = user.id else { continue };

        let hardened = harden_login_hash(&user.stored_password_hash)?;
        schema::User::update_password_hash(
            &mut conn,
            user_id,
            &hardened,
            constants::CURRENT_PASSWORD_HASH_VERSION,
        )
        .await?;
        migrated += 1;
    }

    println!("Password hash migration: rehashed {migrated}/{total} legacy accounts");

    Ok(())
}

/// Name of the `server_secret` row holding the fake-salt HMAC key.
const FAKE_SALT_SECRET_NAME: &str = "fake_login_salt";

static FAKE_SALT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();
static DUMMY_LOGIN_HASH: OnceLock<String> = OnceLock::new();

/// Loads the fake-salt HMAC key from the DB, creating it on first startup.
pub async fn load_or_create_fake_salt_secret(pool: &Pool) -> anyhow::Result<()> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    let existing: Option<Vec<u8>> = conn
        .exec_first(
            "SELECT value FROM server_secret WHERE name = :name",
            params! { "name" => FAKE_SALT_SECRET_NAME },
        )
        .await
        .context("Failed to read fake salt secret")?;

    let secret = match existing {
        Some(secret) => secret,
        None => {
            let mut secret = vec![0u8; 32];
            SysRng
                .try_fill_bytes(&mut secret)
                .map_err(|e| anyhow::anyhow!("Failed to generate fake salt secret: {e}"))?;

            conn.exec_drop(
                "INSERT INTO server_secret (name, value) VALUES (:name, :value)",
                params! { "name" => FAKE_SALT_SECRET_NAME, "value" => secret.clone() },
            )
            .await
            .context("Failed to store fake salt secret")?;

            secret
        }
    };

    FAKE_SALT_SECRET
        .set(secret)
        .map_err(|_| anyhow::anyhow!("Fake salt secret already initialised"))?;

    Ok(())
}

/// Builds a random Argon2 hash used to equalise login timing for unknown usernames.
pub fn init_dummy_login_hash() -> anyhow::Result<()> {
    let mut password = [0u8; 32];
    SysRng
        .try_fill_bytes(&mut password)
        .map_err(|e| anyhow::anyhow!("Failed to generate dummy login password: {e}"))?;

    let hash = harden_login_hash(&hex::encode(password))?;

    DUMMY_LOGIN_HASH
        .set(hash)
        .map_err(|_| anyhow::anyhow!("Dummy login hash already initialised"))?;

    Ok(())
}

/// Argon2 hash of a random value, verified against when the username is unknown.
pub fn dummy_login_hash() -> Result<&'static str, AppError> {
    DUMMY_LOGIN_HASH
        .get()
        .map(String::as_str)
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("Dummy login hash not initialised")))
}

fn fake_salt(secret: &[u8], label: &[u8], username: &str) -> anyhow::Result<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret)
        .map_err(|e| anyhow::anyhow!("Invalid HMAC key: {e}"))?;
    mac.update(label);
    mac.update(username.as_bytes());
    let bytes = mac.finalize().into_bytes();

    SaltString::encode_b64(&bytes[..16])
        .map(|salt| salt.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to encode fake salt: {e}"))
}

/// Stable fake salts for unknown usernames so `GET /login` can't confirm account existence.
pub fn fake_login_salts(username: &str) -> Result<(String, String), AppError> {
    let secret = FAKE_SALT_SECRET
        .get()
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("Fake salt secret not initialised")))?;

    Ok((
        fake_salt(secret, b"nooto/login/salt_auth\0", username)?,
        fake_salt(secret, b"nooto/login/salt_server_auth\0", username)?,
    ))
}

/// Extracts and hex-decodes a bearer token from the `Authorization` header.
pub fn bearer_token_from_headers(headers: &HeaderMap) -> Result<Vec<u8>, AppError> {
    let value = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::unauthorized("Missing or malformed Authorization header"))?;

    hex::decode(value).map_err(|_| AppError::bad_request("Invalid token format"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, StatusCode};

    fn auth_headers(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        headers
    }

    // --- harden_login_hash ---

    #[test]
    fn harden_login_hash_produces_argon2id_hash() {
        let hash = harden_login_hash("hunter2").unwrap();
        assert!(hash.starts_with("$argon2id$"));
    }

    #[test]
    fn harden_login_hash_verifies_against_original() {
        let hash = harden_login_hash("hunter2").unwrap();
        assert!(verify_login_hash("hunter2", &hash).unwrap());
    }

    #[test]
    fn harden_login_hash_uses_unique_salts() {
        let first = harden_login_hash("hunter2").unwrap();
        let second = harden_login_hash("hunter2").unwrap();

        assert_ne!(first, second);
        assert!(verify_login_hash("hunter2", &first).unwrap());
        assert!(verify_login_hash("hunter2", &second).unwrap());
    }

    // --- verify_login_hash ---

    #[test]
    fn verify_login_hash_rejects_wrong_hash() {
        let hash = harden_login_hash("hunter2").unwrap();
        assert!(!verify_login_hash("hunter3", &hash).unwrap());
    }

    #[test]
    fn verify_login_hash_malformed_stored_hash_is_internal_error() {
        let err = verify_login_hash("hunter2", "not-a-phc-hash").unwrap_err();
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.message, "Internal server error");
    }

    // --- fake_login_salts ---

    fn init_test_secret() {
        let _ = FAKE_SALT_SECRET.set(vec![7u8; 32]);
    }

    #[test]
    fn fake_login_salts_are_stable_per_username() {
        init_test_secret();
        assert_eq!(fake_login_salts("alice").unwrap(), fake_login_salts("alice").unwrap());
    }

    #[test]
    fn fake_login_salts_differ_per_username() {
        init_test_secret();
        assert_ne!(fake_login_salts("alice").unwrap(), fake_login_salts("bob").unwrap());
    }

    #[test]
    fn fake_login_salts_have_distinct_auth_and_server_salts() {
        init_test_secret();
        let (salt_auth, salt_server_auth) = fake_login_salts("alice").unwrap();
        assert_ne!(salt_auth, salt_server_auth);
    }

    #[test]
    fn fake_login_salts_parse_as_salt_strings() {
        init_test_secret();
        let (salt_auth, salt_server_auth) = fake_login_salts("alice").unwrap();
        assert_eq!(salt_auth.len(), 22);
        assert!(SaltString::from_b64(&salt_auth).is_ok());
        assert!(SaltString::from_b64(&salt_server_auth).is_ok());
    }

    // --- bearer_token_from_headers ---

    #[test]
    fn bearer_token_decodes_valid_hex() {
        let token = bearer_token_from_headers(&auth_headers("Bearer 0a0b1c")).unwrap();
        assert_eq!(token, vec![0x0a, 0x0b, 0x1c]);
    }

    #[test]
    fn bearer_token_missing_header_is_unauthorized() {
        let err = bearer_token_from_headers(&HeaderMap::new()).unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn bearer_token_wrong_scheme_is_unauthorized() {
        let err = bearer_token_from_headers(&auth_headers("Basic 0a0b")).unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn bearer_token_scheme_is_case_sensitive() {
        let err = bearer_token_from_headers(&auth_headers("bearer 0a0b")).unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn bearer_token_empty_payload_is_unauthorized() {
        let err = bearer_token_from_headers(&auth_headers("Bearer ")).unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn bearer_token_invalid_hex_is_bad_request() {
        let err = bearer_token_from_headers(&auth_headers("Bearer zz")).unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert_eq!(err.message, "Invalid token format");
    }

    #[test]
    fn bearer_token_odd_length_hex_is_bad_request() {
        let err = bearer_token_from_headers(&auth_headers("Bearer abc")).unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }
}
