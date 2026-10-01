use anyhow::Context;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::http::{HeaderMap, header::AUTHORIZATION};
use mysql_async::{Conn, Pool};
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

/// Extracts and hex-decodes a bearer token from the `Authorization` header.
pub fn bearer_token_from_headers(headers: &HeaderMap) -> Result<Vec<u8>, AppError> {
    let value = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::unauthorized("Missing or malformed Authorization header"))?;

    hex::decode(value).map_err(|_| AppError::bad_request("Invalid token format"))
}
