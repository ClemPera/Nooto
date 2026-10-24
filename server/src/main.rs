use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Query, Request as AxumRequest, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dotenvy::dotenv;
use mysql_async::Pool;
use rand::{TryRng, rngs::SysRng};
use shared::SentNotesResult;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor};

use crate::schema::User;

mod utils;
mod schema;
mod migrations;
mod constants;

/// Application error returned by all handlers.
/// Internal errors are logged server-side and return a generic 500 to the client.
#[derive(Debug)]
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

    utils::rehash_legacy_password_hashes(&pool)
        .await
        .context("Failed to rehash legacy password hashes")?;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind TCP listener");

    axum::serve(
        listener,
        app(pool)?.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");

    Ok(())
}

/// Builds the router; `/version` stays public so old clients can discover the protocol range.
fn app(pool: Pool) -> anyhow::Result<Router> {
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
        .route_layer(GovernorLayer::new(auth_governor_conf));

    let api_routes = Router::new()
        .route("/notes", post(send_notes))
        .route("/notes", get(select_notes))
        .route("/note", get(select_note))
        .merge(auth_routes)
        .route_layer(middleware::from_fn(protocol_middleware))
        .with_state(pool);

    Ok(Router::new()
        .route("/version", get(version))
        .merge(api_routes))
}

fn server_info() -> shared::ServerInfo {
    shared::ServerInfo {
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: shared::PROTOCOL_VERSION,
        min_client_protocol_version: constants::MIN_CLIENT_PROTOCOL_VERSION,
    }
}

async fn version() -> impl IntoResponse {
    ([(header::CACHE_CONTROL, "no-store")], Json(server_info()))
}

fn check_client_protocol(headers: &HeaderMap) -> Result<(), shared::ProtocolMismatch> {
    let client_protocol_version = headers
        .get(shared::PROTOCOL_VERSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u32>().ok());

    let server = server_info();

    match client_protocol_version {
        Some(version)
            if server.check_client(version) == shared::ProtocolCompatibility::Compatible =>
        {
            Ok(())
        }
        _ => Err(shared::ProtocolMismatch::new(client_protocol_version, Some(server))),
    }
}

async fn protocol_middleware(request: AxumRequest, next: Next) -> Response {
    match check_client_protocol(request.headers()) {
        Ok(()) => next.run(request).await,
        Err(mismatch) => (StatusCode::UPGRADE_REQUIRED, Json(mismatch)).into_response(),
    }
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

    utils::user_verify(&mut conn, sent_notes.username.clone(), sent_notes.token).await?;

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

    let token = utils::bearer_token_from_headers(&headers)?;

    utils::user_verify(&mut conn, params.username.clone(), token).await?;

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

    let token = utils::bearer_token_from_headers(&headers)?;

    utils::user_verify(&mut conn, params.username.clone(), token).await?;

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
    user.stored_password_hash = utils::harden_login_hash(&user.stored_password_hash)?;
    user.password_hash_version = constants::CURRENT_PASSWORD_HASH_VERSION;

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
        Some(user) if utils::verify_login_hash(&params.login_hash, &user.stored_password_hash)? => user,
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

    utils::user_verify(&mut conn, params.username.clone(), params.token.clone()).await?;

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
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, header};
    use axum::response::IntoResponse;
    use tower::ServiceExt;

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

    #[test]
    fn server_info_matches_constants() {
        let info = server_info();
        assert_eq!(info.server_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.protocol_version, shared::PROTOCOL_VERSION);
        assert_eq!(
            info.min_client_protocol_version,
            constants::MIN_CLIENT_PROTOCOL_VERSION
        );
    }

    fn headers_with_protocol(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(shared::PROTOCOL_VERSION_HEADER, value.parse().unwrap());
        headers
    }

    #[test]
    fn missing_protocol_header_is_client_outdated() {
        let mismatch = check_client_protocol(&HeaderMap::new()).unwrap_err();
        assert_eq!(mismatch.code, "protocol_mismatch");
        assert!(mismatch.client_outdated);
        assert_eq!(mismatch.client_protocol_version, None);
    }

    #[test]
    fn matching_protocol_header_is_accepted() {
        assert!(check_client_protocol(&headers_with_protocol("1")).is_ok());
    }

    #[test]
    fn older_protocol_header_is_client_outdated() {
        let mismatch = check_client_protocol(&headers_with_protocol("0")).unwrap_err();
        assert!(mismatch.client_outdated);
        assert_eq!(mismatch.client_protocol_version, Some(0));
    }

    #[test]
    fn newer_protocol_header_is_server_outdated() {
        let mismatch = check_client_protocol(&headers_with_protocol("999")).unwrap_err();
        assert!(!mismatch.client_outdated);
        assert_eq!(mismatch.client_protocol_version, Some(999));
    }

    #[test]
    fn malformed_protocol_header_is_client_outdated() {
        let mismatch = check_client_protocol(&headers_with_protocol("not-a-version")).unwrap_err();
        assert!(mismatch.client_outdated);
        assert_eq!(mismatch.client_protocol_version, None);
    }

    fn test_app() -> Router {
        app(Pool::new("mysql://user:pass@localhost/db")).unwrap()
    }

    async fn body_bytes(response: Response) -> Vec<u8> {
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec()
    }

    #[tokio::test]
    async fn version_route_returns_server_info() {
        let response = test_app()
            .oneshot(Request::builder().uri("/version").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");

        let info: shared::ServerInfo = serde_json::from_slice(&body_bytes(response).await).unwrap();
        assert_eq!(info.server_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.protocol_version, shared::PROTOCOL_VERSION);
        assert_eq!(
            info.min_client_protocol_version,
            constants::MIN_CLIENT_PROTOCOL_VERSION
        );
    }

    #[tokio::test]
    async fn notes_without_protocol_header_returns_426() {
        let response = test_app()
            .oneshot(Request::builder().uri("/notes").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);

        let mismatch: shared::ProtocolMismatch =
            serde_json::from_slice(&body_bytes(response).await).unwrap();
        assert_eq!(mismatch.code, "protocol_mismatch");
        assert!(mismatch.client_outdated);
    }
}
