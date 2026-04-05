use std::env;

use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dotenv::dotenv;
use mysql_async::{Conn, Pool};
use rand_core::{OsRng, TryRngCore};
use shared::SentNotesResult;

use crate::schema::User;

mod schema;

/// Application error returned by all handlers.
/// Internal errors are logged server-side and return a generic 500 to the client.
pub struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    pub fn internal(err: anyhow::Error) -> Self {
        eprintln!("Internal error: {err:#}");
        AppError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Internal server error".to_string(),
        }
    }

    pub fn not_found() -> Self {
        AppError { status: StatusCode::NOT_FOUND, message: "Not found".to_string() }
    }

    pub fn unauthorized() -> Self {
        AppError { status: StatusCode::UNAUTHORIZED, message: "Unauthorized".to_string() }
    }

    pub fn forbidden() -> Self {
        AppError { status: StatusCode::FORBIDDEN, message: "Forbidden".to_string() }
    }

    pub fn unprocessable() -> Self {
        AppError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: "Unprocessable entity".to_string(),
        }
    }

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
async fn main() {
    dotenv().ok();
    //Env var should be like mysql://user:pass%20word@localhost/database_name
    let pool = Pool::new(
        env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set")
            .as_str(),
    );

    let app = Router::new()
        .route("/notes", post(send_notes))
        .route("/notes", get(select_notes))
        .route("/note", get(select_note))
        .route("/create_account", post(insert_user))
        // .route("/user", put()) //Update user
        .route("/login", get(login_request))
        .route("/login", post(login))
        // .route("/user_recovery", get()) //Request recovery stuff
        // .route("/user_recovery", post()) //check recovery hash
        // .route("/data_recovery", get()) //Request recovery stuff
        // .route("/data_recovery", post()) //store new recovery stuff
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind TCP listener");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}

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

    for ut in user_tokens {
        if ut.token == token {
            return Ok(());
        }
    }

    Err(AppError::forbidden())
}

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
                    updated_note.update(&mut conn).await.map_err(AppError::from)?;

                    result.push(SentNotesResult {
                        uuid: updated_note.uuid,
                        status: shared::NoteStatus::Ok,
                    });
                }
            }
            None => {
                let mut srv_note: schema::Note = note.into();
                srv_note.id_user = Some(user_id);

                srv_note.insert(&mut conn).await.map_err(AppError::from)?;

                result.push(SentNotesResult {
                    uuid: srv_note.uuid,
                    status: shared::NoteStatus::Ok,
                });
            }
        }
    }

    Ok(Json(result))
}

async fn select_notes(
    State(pool): State<Pool>,
    Query(params): Query<shared::SelectNotesParams>,
) -> Result<Json<Vec<shared::Note>>, AppError> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    let token = hex::decode(&params.token)
        .map_err(|_| AppError::bad_request("Invalid token format"))?;

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

async fn select_note(
    State(pool): State<Pool>,
    Query(params): Query<shared::SelectNoteParams>,
) -> Result<Json<shared::Note>, AppError> {
    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    let token = hex::decode(&params.token)
        .map_err(|_| AppError::bad_request("Invalid token format"))?;

    user_verify(&mut conn, params.username.clone(), token).await?;

    let user = User::select(&mut conn, params.username)
        .await
        .map_err(AppError::from)?
        .ok_or_else(AppError::unprocessable)?;

    let user_id = user.id.ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    let note = schema::Note::select(&mut conn, user_id, params.note_id)
        .await
        .map_err(AppError::from)?
        .ok_or_else(AppError::not_found)?;

    Ok(Json(note.into()))
}

async fn insert_user(
    State(pool): State<Pool>,
    Json(user): Json<shared::User>,
) -> Result<(), AppError> {
    println!("received insert_user");
    let user: schema::User = user.into();

    let mut conn = pool
        .get_conn()
        .await
        .context("Failed to get DB connection")?;

    user.insert(&mut conn).await.map_err(AppError::from)?;

    println!("insert_user: completed");

    Ok(())
}

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
        .ok_or_else(AppError::not_found)?;

    Ok(Json(shared::LoginRequest {
        salt_auth: user.salt_auth,
        salt_server_auth: user.salt_server_auth,
    }))
}

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
        .map_err(AppError::from)?
        .ok_or_else(AppError::not_found)?;

    if params.login_hash != user.stored_password_hash {
        return Err(AppError::unauthorized());
    }

    let mut token = vec![0u8; 32];
    OsRng
        .try_fill_bytes(&mut token)
        .map_err(|e| AppError::internal(anyhow::anyhow!("Failed to generate token: {e}")))?;

    let user_id = user.id.ok_or_else(|| AppError::internal(anyhow::anyhow!("User has no ID")))?;

    let user_token = schema::UserToken {
        id: None,
        id_user: user_id,
        token,
    };

    user_token.insert(&mut conn).await.map_err(AppError::from)?;

    Ok(Json(shared::Login {
        salt_data: user.salt_data,
        encrypted_mek_password: user.encrypted_mek_password,
        mek_password_nonce: user.mek_password_nonce,
        token: user_token.token,
    }))
}
