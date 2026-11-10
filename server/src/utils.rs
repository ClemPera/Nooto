use std::sync::{Arc, OnceLock};
use std::time::Duration;

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
use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

use crate::AppError;
use crate::constants;
use crate::schema;

/// Name of the `server_secret` row holding the fake-salt HMAC key.
const FAKE_SALT_SECRET_NAME: &str = "fake_login_salt";

/// Env var holding the memory budget shared by concurrent Argon2 runs, in MiB.
const ARGON2_MEMORY_ENV: &str = "NOOTO_ARGON2_MEMORY_MIB";

/// Memory cost of one Argon2 hash, derived from the crate default (19 MiB).
const MIB_PER_HASH: usize = argon2::Params::DEFAULT_M_COST as usize / 1024;

/// Memory budget, in MiB, used when `NOOTO_ARGON2_MEMORY_MIB` is unset.
const DEFAULT_ARGON2_MEMORY_MIB: usize = 256;

static FAKE_SALT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();
static DUMMY_LOGIN_HASH: OnceLock<String> = OnceLock::new();
static ARGON2_LIMITER: OnceLock<Result<Arc<Semaphore>, String>> = OnceLock::new();

/// Fails fast at startup when the Argon2 budget is misconfigured.
pub fn init_argon2_limiter() -> anyhow::Result<()> {
    let permits = argon2_permits_from_env()?;

    // Already built by an earlier call; the configured budget is identical either way.
    let _ = ARGON2_LIMITER.set(Ok(Arc::new(Semaphore::new(permits))));

    Ok(())
}

/// Takes one Argon2 slot, or fails immediately when every slot is busy.
pub fn try_acquire_argon2_permit() -> Result<OwnedSemaphorePermit, AppError> {
    match argon2_limiter()?.try_acquire_owned() {
        Ok(permit) => Ok(permit),
        Err(TryAcquireError::NoPermits) => {
            Err(AppError::service_unavailable("Server busy, try again later"))
        }
        Err(TryAcquireError::Closed) => {
            Err(AppError::internal(anyhow::anyhow!("Argon2 limiter closed")))
        }
    }
}

/// Shared limiter, built once from the configured memory budget.
fn argon2_limiter() -> Result<Arc<Semaphore>, AppError> {
    ARGON2_LIMITER
        .get_or_init(|| {
            argon2_permits_from_env()
                .map(|permits| Arc::new(Semaphore::new(permits)))
                .map_err(|err| err.to_string())
        })
        .clone()
        .map_err(|err| AppError::internal(anyhow::anyhow!("Invalid {ARGON2_MEMORY_ENV}: {err}")))
}

fn argon2_permits_from_env() -> anyhow::Result<usize> {
    let raw = match std::env::var(ARGON2_MEMORY_ENV) {
        Ok(raw) => Some(raw),
        Err(std::env::VarError::NotPresent) => None,
        Err(err) => {
            return Err(anyhow::Error::new(err).context(format!("Invalid {ARGON2_MEMORY_ENV}")))
        }
    };

    Ok(parse_argon2_budget(raw.as_deref())? / MIB_PER_HASH)
}

fn parse_argon2_budget(raw: Option<&str>) -> anyhow::Result<usize> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_ARGON2_MEMORY_MIB);
    };

    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(DEFAULT_ARGON2_MEMORY_MIB);
    }

    let budget: usize = raw
        .parse()
        .with_context(|| format!("{ARGON2_MEMORY_ENV} must be an integer number of MiB"))?;

    anyhow::ensure!(
        budget >= MIB_PER_HASH,
        "{ARGON2_MEMORY_ENV} must be at least {MIB_PER_HASH} MiB"
    );

    Ok(budget)
}

/// Failure while taking a connection from the pool.
#[derive(Debug)]
pub enum PoolError {
    /// No connection became available before the deadline.
    Timeout,
    /// The pool or the server returned an error.
    Db(mysql_async::Error),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::Timeout => write!(
                f,
                "Timed out after {}s waiting for a database connection",
                constants::DB_CONN_TIMEOUT_SECS
            ),
            PoolError::Db(err) => write!(f, "Failed to get DB connection: {err}"),
        }
    }
}

impl std::error::Error for PoolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PoolError::Db(err) => Some(err),
            PoolError::Timeout => None,
        }
    }
}

/// Bounded `Pool::get_conn`, so a saturated or unreachable pool cannot hang a request.
pub async fn get_conn(pool: &Pool) -> Result<Conn, PoolError> {
    match tokio::time::timeout(
        Duration::from_secs(constants::DB_CONN_TIMEOUT_SECS),
        pool.get_conn(),
    )
    .await
    {
        Ok(Ok(conn)) => Ok(conn),
        Ok(Err(err)) => Err(PoolError::Db(err)),
        Err(_) => Err(PoolError::Timeout),
    }
}

impl From<PoolError> for AppError {
    fn from(err: PoolError) -> Self {
        match err {
            PoolError::Timeout => AppError::service_unavailable("Database busy, try again later"),
            PoolError::Db(err) => AppError::internal(anyhow::Error::new(err)),
        }
    }
}

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

/// True when the error is MySQL `ER_DUP_ENTRY`, i.e. a unique index rejected the insert.
pub fn is_duplicate_entry(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<mysql_async::Error>(),
        Some(mysql_async::Error::Server(e)) if e.code == 1062
    )
}

/// `harden_login_hash` on the blocking pool, so Argon2 never stalls an async worker.
pub async fn harden_login_hash_blocking(login_hash: String) -> Result<String, AppError> {
    let hashed = tokio::task::spawn_blocking(move || harden_login_hash(&login_hash))
        .await
        .map_err(|e| AppError::internal(anyhow::anyhow!("Login hash task failed: {e}")))?;
    Ok(hashed?)
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

/// `verify_login_hash` on the blocking pool, so Argon2 never stalls an async worker.
pub async fn verify_login_hash_blocking(
    login_hash: &str,
    stored_password_hash: &str,
) -> Result<bool, AppError> {
    let login_hash = login_hash.to_owned();
    let stored_password_hash = stored_password_hash.to_owned();
    tokio::task::spawn_blocking(move || verify_login_hash(&login_hash, &stored_password_hash))
        .await
        .map_err(|e| AppError::internal(anyhow::anyhow!("Login hash verification task failed: {e}")))?
}

/// One-time migration: rehashes accounts below CURRENT_PASSWORD_HASH_VERSION, then becomes a no-op.
//TODO: remove this fn, password_hash_version, and its write in insert_user() once no account is below version 2.
pub async fn rehash_legacy_password_hashes(pool: &Pool) -> anyhow::Result<()> {
    let mut conn = get_conn(pool).await.context("Failed to get DB connection")?;

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

/// Loads the fake-salt HMAC key from the DB, creating it on first startup.
pub async fn load_or_create_fake_salt_secret(pool: &Pool) -> anyhow::Result<()> {
    let mut conn = get_conn(pool).await.context("Failed to get DB connection")?;

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

    // --- is_duplicate_entry ---

    fn server_error(code: u16) -> anyhow::Error {
        mysql_async::Error::Server(mysql_async::ServerError {
            code,
            message: "duplicate".to_string(),
            state: "23000".to_string(),
        })
        .into()
    }

    #[test]
    fn duplicate_entry_matches_1062() {
        assert!(is_duplicate_entry(&server_error(1062)));
    }

    #[test]
    fn duplicate_entry_matches_through_context() {
        let err = server_error(1062).context("Failed to insert user");
        assert!(is_duplicate_entry(&err));
    }

    #[test]
    fn duplicate_entry_ignores_other_server_errors() {
        assert!(!is_duplicate_entry(&server_error(1452)));
    }

    #[test]
    fn duplicate_entry_ignores_unrelated_errors() {
        assert!(!is_duplicate_entry(&anyhow::anyhow!("nope")));
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

    // --- get_conn ---

    #[test]
    fn pool_timeout_maps_to_503_with_retry_after() {
        let err: AppError = PoolError::Timeout.into();
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(err.retry_after, Some(1));
    }

    // --- argon2 limiter ---

    #[test]
    fn argon2_budget_defaults_to_256_mib() {
        assert_eq!(parse_argon2_budget(None).unwrap(), 256);
        assert_eq!(parse_argon2_budget(Some("")).unwrap(), 256);
        assert_eq!(parse_argon2_budget(Some("  ")).unwrap(), 256);
    }

    #[test]
    fn argon2_budget_accepts_explicit_values() {
        assert_eq!(parse_argon2_budget(Some("512")).unwrap(), 512);
    }

    #[test]
    fn argon2_budget_rejects_values_below_one_hash() {
        let too_small = (MIB_PER_HASH - 1).to_string();
        assert!(parse_argon2_budget(Some(&too_small)).is_err());
    }

    #[test]
    fn argon2_budget_rejects_non_numeric_values() {
        assert!(parse_argon2_budget(Some("lots")).is_err());
    }

    #[test]
    fn default_budget_yields_the_documented_permit_count() {
        assert_eq!(DEFAULT_ARGON2_MEMORY_MIB / MIB_PER_HASH, 13);
    }

    #[test]
    fn saturated_limiter_returns_503_with_retry_after() {
        let limiter = argon2_limiter().unwrap();
        let mut held = Vec::new();
        while let Ok(permit) = limiter.clone().try_acquire_owned() {
            held.push(permit);
        }

        let err = try_acquire_argon2_permit().unwrap_err();
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(err.retry_after, Some(1));

        drop(held);
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
