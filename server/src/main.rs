use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dotenv::dotenv;
use mysql_async::{Conn, Pool};
use rand::{TryRng, rngs::SysRng};
use shared::SentNotesResult;
use subtle::ConstantTimeEq;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor};

use crate::schema::User;

mod schema;

mod migrations;

/// Application error returned by all handlers.
/// Internal errors are logged server-side and return a generic 500 to the client.
pub struct AppError {
    status: StatusCode,
    message: String,
}

//TODO: impl logging (info for most error)
impl AppError {
    /// Logs the full error chain and returns a generic 500 to avoid leaking internals.
    pub fn internal(err: anyhow::Error) -> Self {
        eprintln!("Internal error: {err:#}");
        AppError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Internal server error".to_string(),
        }
    }

    /// 404 with a caller-supplied message.
    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError { status: StatusCode::NOT_FOUND, message: msg.into() }
    }

    /// 401 with a caller-supplied message.
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        AppError { status: StatusCode::UNAUTHORIZED, message: msg.into() }
    }

    /// 403 — used when the token is present but doesn't match.
    pub fn forbidden() -> Self {
        AppError { status: StatusCode::FORBIDDEN, message: "Forbidden".to_string() }
    }

    /// 422 — used when an expected entity (e.g. the user record) is missing mid-request.
    pub fn unprocessable() -> Self {
        AppError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: "Unprocessable entity".to_string(),
        }
    }
    
    /// 409 — used when a resource already exists (e.g. duplicate username).
    pub fn conflict(msg: impl Into<String>) -> Self {
        AppError { status: StatusCode::CONFLICT, message: msg.into() }
    }

    /// 400 with a caller-supplied message (e.g. malformed token).
    pub fn bad_request(msg: impl Into<String>) -> Self {
        AppError { status: StatusCode::BAD_REQUEST, message: msg.into() }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::internal(err)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    //Env var should be like mysql://user:pass%20word@localhost/database_name
    let pool = Pool::new(
        env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set")
            .as_str(),
    );

    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection for migrations")?;

    migrations::run(&mut conn)
        .await
        .context("Failed to run database migrations")?;

    drop(conn);

    rehash_legacy_password_hashes(&pool)
        .await
        .context("Failed to rehash legacy password hashes")?;

    // Rate-limited below: /login and /create_account are what an attacker would brute-force.
    let auth_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(SmartIpKeyExtractor)
            .per_second(4)
            .burst_size(5)
            .finish()
            .context("Failed to build rate limiter config")?,
    );

    let auth_routes = Router::new()
        .route("/create_account", post(insert_user))
        // .route("/user", put()) //Update user
        .route("/login", get(login_request))
        .route("/login", post(login))
        .route("/logout", post(logout))
        // .route("/user_recovery", get()) //Request recovery stuff
        // .route("/user_recovery", post()) //check recovery hash
        // .route("/data_recovery", get()) //Request recovery stuff
        // .route("/data_recovery", post()) //store new recovery stuff
        .route_layer(GovernorLayer { config: auth_governor_conf });

    let app = Router::new()
        .route("/notes", post(send_notes))
        .route("/notes", get(select_notes))
        .route("/note", get(select_note))
        .merge(auth_routes)
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind TCP listener");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");

    Ok(())
}

/// Password hash format; see schema::User::password_hash_version.
const CURRENT_PASSWORD_HASH_VERSION: u8 = 2;

/// Re-hashes login_hash with a fresh salt (defense in depth against a DB leak).
fn harden_login_hash(login_hash: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(login_hash.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to hash login_hash: {e}"))
}

/// Verifies login_hash against a hardened (Argon2id) stored_password_hash.
fn verify_login_hash(login_hash: &str, stored_password_hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(stored_password_hash).map_err(|e| {
        AppError::internal(anyhow::anyhow!("Failed to parse stored password hash: {e}"))
    })?;

    Ok(Argon2::default()
        .verify_password(login_hash.as_bytes(), &parsed)
        .is_ok())
}

/// One-time migration: rehashes accounts below CURRENT_PASSWORD_HASH_VERSION, then becomes a no-op.
//TODO: remove this fn, password_hash_version, and its write in insert_user() once no account is below version 2.
async fn rehash_legacy_password_hashes(pool: &Pool) -> anyhow::Result<()> {
    let mut conn = pool.get_conn().await.context("Failed to get DB connection")?;

    let legacy_users =
        schema::User::select_outdated_password_hashes(&mut conn, CURRENT_PASSWORD_HASH_VERSION).await?;

    if legacy_users.is_empty() {
        println!("Password hash migration: no legacy accounts to rehash");
        return Ok(());
    }

    let total = legacy_users.len();
    let mut migrated = 0;

    for user in legacy_users {
        let Some(user_id) = user.id else { continue };

        let hardened = harden_login_hash(&user.stored_password_hash)?;
        schema::User::update_password_hash(&mut conn, user_id, &hardened, CURRENT_PASSWORD_HASH_VERSION).await?;
        migrated += 1;
    }

    println!("Password hash migration: rehashed {migrated}/{total} legacy accounts");

    Ok(())
}

/// Extracts and hex-decodes a bearer token from the `Authorization` header.
fn bearer_token_from_headers(headers: &HeaderMap) -> Result<Vec<u8>, AppError> {
    let value = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::unauthorized("Missing or malformed Authorization header"))?;

    hex::decode(value).map_err(|_| AppError::bad_request("Invalid token format"))
}

/// A token is rejected once it's been idle (unused) for longer than this.
const SESSION_TOKEN_MAX_AGE_SECS: i64 = 60 * 60 * 24 * 30;

/// Minimum gap between DB writes that refresh a token's age.
const SESSION_TOKEN_REFRESH_AFTER_SECS: i64 = 60 * 60 * 24;

/// Verifies `token` is valid and not idle-expired for `username`.
async fn user_verify(conn: &mut Conn, username: String, token: Vec<u8>) -> Result<(), AppError> {
    //TODO: this could return user honestly
    let user = schema::User::select(conn, username)
        .await
        .map_err(AppError::from)?
        .ok_or_else(AppError::unprocessable)?;

    let user_id = user.id.ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    let user_tokens = schema::UserToken::select(conn, user_id)
        .await
        .map_err(AppError::from)?;

    let now = chrono::Local::now().to_utc().timestamp();

    for ut in user_tokens {
        if bool::from(ut.token.ct_eq(&token)) {
            let idle_secs = now - ut.last_used_at;

            if idle_secs > SESSION_TOKEN_MAX_AGE_SECS {
                schema::UserToken::delete(conn, user_id, &ut.token)
                    .await
                    .map_err(AppError::from)?;
                return Err(AppError::unauthorized("Session expired, please log in again"));
            }

            if idle_secs > SESSION_TOKEN_REFRESH_AFTER_SECS {
                schema::UserToken::touch(conn, user_id, &ut.token, now)
                    .await
                    .map_err(AppError::from)?;
            }

            return Ok(());
        }
    }

    Err(AppError::forbidden())
}

/// `POST /notes` — upserts a batch of notes for the authenticated user.
/// Returns per-note results; conflicting notes (server newer and `force` is false) are flagged.
async fn send_notes(
    State(pool): State<Pool>,
    Json(sent_notes): Json<shared::SentNotes>,
) -> Result<Json<Vec<SentNotesResult>>, AppError> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    user_verify(&mut conn, sent_notes.username.clone(), sent_notes.token).await?;

    let user = User::select(&mut conn, sent_notes.username)
        .await
        .map_err(AppError::from)?
        .ok_or_else(AppError::unprocessable)?;

    let user_id = user.id.ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    let mut result: Vec<SentNotesResult> = vec![];

    for note in sent_notes.notes {
        println!("The user sent us some notes");

        match schema::Note::select(&mut conn, user_id, note.clone().uuid)
            .await
            .map_err(AppError::from)?
        {
            Some(selected_note) => {
                if selected_note.updated_at > note.updated_at && !sent_notes.force {
                    result.push(SentNotesResult {
                        uuid: selected_note.uuid.clone(),
                        status: shared::NoteStatus::Conflict(selected_note.clone().into()),
                    });
                    println!(
                        "user {:?} has a conflict on note {:?}",
                        user_id, selected_note.uuid
                    );
                } else {
                    let mut updated_note: schema::Note = note.into();
                    updated_note.id_user = Some(user_id);
                    updated_note.server_received_at = chrono::Local::now().to_utc().timestamp();
                    updated_note.update(&mut conn, user_id).await.map_err(AppError::from)?;

                    result.push(SentNotesResult {
                        uuid: updated_note.uuid,
                        status: shared::NoteStatus::Ok(updated_note.server_received_at),
                    });
                }
            }
            None => {
                let mut srv_note: schema::Note = note.into();
                srv_note.id_user = Some(user_id);
                srv_note.server_received_at = chrono::Local::now().to_utc().timestamp();

                srv_note.insert(&mut conn).await.map_err(AppError::from)?;

                result.push(SentNotesResult {
                    uuid: srv_note.uuid,
                    status: shared::NoteStatus::Ok(srv_note.server_received_at),
                });
            }
        }
    }

    Ok(Json(result))
}

/// `GET /notes` — returns all notes for the authenticated user updated after `params.updated_at`.
async fn select_notes(
    State(pool): State<Pool>,
    headers: HeaderMap,
    Query(params): Query<shared::SelectNotesParams>,
) -> Result<Json<Vec<shared::Note>>, AppError> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    let token = bearer_token_from_headers(&headers)?;

    user_verify(&mut conn, params.username.clone(), token).await?;

    let user = User::select(&mut conn, params.username)
        .await
        .map_err(AppError::from)?
        .ok_or_else(AppError::unprocessable)?;

    let user_id = user.id.ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    let notes = schema::Note::select_all_from_user(&mut conn, user_id, params.updated_at)
        .await
        .map_err(AppError::from)?;

    let notes: Vec<shared::Note> = notes.into_iter().map(|note| note.into()).collect();

    if !notes.is_empty() {
        println!("Some notes are sent to user");
    }

    Ok(Json(notes))
}

/// `GET /note` — returns a single note by UUID for the authenticated user.
async fn select_note(
    State(pool): State<Pool>,
    headers: HeaderMap,
    Query(params): Query<shared::SelectNoteParams>,
) -> Result<Json<shared::Note>, AppError> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    let token = bearer_token_from_headers(&headers)?;

    user_verify(&mut conn, params.username.clone(), token).await?;

    let user = User::select(&mut conn, params.username)
        .await
        .map_err(AppError::from)?
        .ok_or_else(AppError::unprocessable)?;

    let user_id = user.id.ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    let note = schema::Note::select(&mut conn, user_id, params.note_id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(||AppError::not_found("Note doesn't exist"))?;

    Ok(Json(note.into()))
}

/// `POST /create_account` — registers a new user. Returns 409 if the username is taken.
async fn insert_user(
    State(pool): State<Pool>,
    Json(user): Json<shared::User>,
) -> Result<(), AppError> {
    println!("received insert_user");
    let mut user: schema::User = user.into();
    user.stored_password_hash = harden_login_hash(&user.stored_password_hash)?;
    user.password_hash_version = CURRENT_PASSWORD_HASH_VERSION;

    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    if User::select(&mut conn, user.clone().username).await?.is_some() {
        return Err(AppError::conflict("This username already exist"))
    }

    user.insert(&mut conn).await.map_err(AppError::from)?;

    println!("insert_user: completed");

    Ok(())
}

/// `GET /login` — returns the salts the client needs to derive its login hash.
async fn login_request(
    State(pool): State<Pool>,
    Query(params): Query<shared::LoginRequestParams>,
) -> Result<Json<shared::LoginRequest>, AppError> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    let user = schema::User::select(&mut conn, params.username)
        .await
        .map_err(AppError::from)?
        .ok_or_else(||AppError::not_found("User doesn't exist"))?;

    Ok(Json(shared::LoginRequest {
        salt_auth: user.salt_auth,
        salt_server_auth: user.salt_server_auth,
    }))
}

/// `POST /login` — validates the login hash, issues a new session token, and returns the
/// material needed to decrypt the master encryption key client-side.
#[axum::debug_handler]
async fn login(
    State(pool): State<Pool>,
    Json(params): Json<shared::LoginParams>,
) -> Result<Json<shared::Login>, AppError> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    let user = schema::User::select(&mut conn, params.username)
        .await
        .map_err(AppError::from)?;

    // Same response for missing user and wrong password to avoid enumeration (CWE-203).
    let user = match user {
        Some(user) if verify_login_hash(&params.login_hash, &user.stored_password_hash)? => user,
        _ => return Err(AppError::unauthorized("Invalid username or password")),
    };

    let user_id = user.id.ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    let mut token = vec![0u8; 32];
    SysRng
        .try_fill_bytes(&mut token)
        .map_err(|e| AppError::internal(anyhow::anyhow!("Failed to generate token: {e}")))?;

    let user_token = schema::UserToken {
        id: None,
        id_user: user_id,
        token,
        last_used_at: chrono::Local::now().to_utc().timestamp(),
    };

    user_token.insert(&mut conn).await.map_err(AppError::from)?;

    Ok(Json(shared::Login {
        salt_data: user.salt_data,
        encrypted_mek_password: user.encrypted_mek_password,
        mek_password_nonce: user.mek_password_nonce,
        token: user_token.token,
    }))
}

/// `POST /logout` — revokes the presented session token.
async fn logout(
    State(pool): State<Pool>,
    Json(params): Json<shared::LogoutParams>,
) -> Result<StatusCode, AppError> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    user_verify(&mut conn, params.username.clone(), params.token.clone()).await?;

    let user = User::select(&mut conn, params.username)
        .await
        .map_err(AppError::from)?
        .ok_or_else(AppError::unprocessable)?;

    let user_id = user.id.ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    schema::UserToken::delete(&mut conn, user_id, &params.token)
        .await
        .map_err(AppError::from)?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    // --- AppError constructors ---

    #[test]
    fn app_error_internal_returns_500() {
        let e = AppError::internal(anyhow::anyhow!("boom"));
        assert_eq!(e.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(e.message, "Internal server error");
    }

    #[test]
    fn app_error_not_found() {
        let e = AppError::not_found("missing resource");
        assert_eq!(e.status, StatusCode::NOT_FOUND);
        assert_eq!(e.message, "missing resource");
    }

    #[test]
    fn app_error_unauthorized() {
        let e = AppError::unauthorized("bad credentials");
        assert_eq!(e.status, StatusCode::UNAUTHORIZED);
        assert_eq!(e.message, "bad credentials");
    }

    #[test]
    fn app_error_forbidden() {
        let e = AppError::forbidden();
        assert_eq!(e.status, StatusCode::FORBIDDEN);
        assert_eq!(e.message, "Forbidden");
    }

    #[test]
    fn app_error_unprocessable() {
        let e = AppError::unprocessable();
        assert_eq!(e.status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn app_error_conflict() {
        let e = AppError::conflict("already exists");
        assert_eq!(e.status, StatusCode::CONFLICT);
        assert_eq!(e.message, "already exists");
    }

    #[test]
    fn app_error_bad_request() {
        let e = AppError::bad_request("invalid input");
        assert_eq!(e.status, StatusCode::BAD_REQUEST);
        assert_eq!(e.message, "invalid input");
    }

    // --- From<anyhow::Error> ---

    #[test]
    fn app_error_from_anyhow_is_internal() {
        let e: AppError = anyhow::anyhow!("something failed").into();
        assert_eq!(e.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(e.message, "Internal server error");
    }

    // --- IntoResponse ---

    #[test]
    fn app_error_into_response_has_correct_status() {
        let e = AppError::not_found("nope");
        assert_eq!(e.into_response().status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn app_error_internal_into_response_has_500() {
        let e = AppError::internal(anyhow::anyhow!("internal"));
        assert_eq!(e.into_response().status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
