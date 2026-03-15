use std::env;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use dotenv::dotenv;
use mysql_async::{Conn, Pool};
use rand_core::{OsRng, TryRngCore};
use shared::SentNotesResult;

use crate::schema::User;

mod schema;

#[tokio::main]
async fn main() {
    dotenv().ok();
    //Env var should be like mysql://user:pass%20word@localhost/database_name
    let pool = Pool::new(env::var("DATABASE_URL").unwrap().as_str());

    let app = Router::new()
        .route("/notes", post(send_notes))
        .route("/notes", get(select_notes))
        .route("/note", get(select_note))
        .route("/create_account", post(insert_user)) //Create account
        // .route("/user", put()) //Update user
        .route("/login", get(login_request)) //Request login
        .route("/login", post(login)) //Check login hash
        // .route("/user_recovery", get()) //Request recovery stuff
        // .route("/user_recovery", post()) //check recovery hash
        // .route("/data_recovery", get()) //Request recovery stuff
        // .route("/data_recovery", post()) //store new recovery stuff
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn user_verify(conn: &mut Conn, username: String, token: Vec<u8>) -> Result<(), StatusCode> {
    //TODO: this could return user honestly
    let user = match schema::User::select(conn, username).await {
        Some(u) => u,
        None => return Err(StatusCode::UNPROCESSABLE_ENTITY),
    };

    let user_tokens = schema::UserToken::select(conn, user.id.unwrap()).await;

    for ut in user_tokens {
        if ut.token == token {
            return Ok(());
        }
    }

    Err(StatusCode::FORBIDDEN)
}

async fn send_notes(
    State(pool): State<Pool>,
    Json(sent_notes): Json<shared::SentNotes>,
) -> Result<Json<Vec<SentNotesResult>>, StatusCode> {
    let mut conn = pool.get_conn().await.unwrap();

    user_verify(&mut conn, sent_notes.username.clone(), sent_notes.token).await?;

    let user = User::select(&mut conn, sent_notes.username).await.unwrap();

    let mut result: Vec<SentNotesResult> = vec![];

    for note in sent_notes.notes {
        println!("The user sent us some notes");

        match schema::Note::select(&mut conn, user.id.unwrap(), note.clone().uuid).await {
            Some(selected_note) => {
                if selected_note.updated_at > note.updated_at && !sent_notes.force {
                    result.push(SentNotesResult {
                        uuid: selected_note.uuid.clone(),
                        status: shared::NoteStatus::Conflict(selected_note.clone().into()),
                    });
                    println!("user {:?} has a conflict on note {:?}", user.id, selected_note.uuid)
                } else {
                    let mut updated_note: schema::Note = note.into();
                    updated_note.id_user = user.id;
                    updated_note.update(&mut conn).await;

                    result.push(SentNotesResult {
                        uuid: updated_note.uuid,
                        status: shared::NoteStatus::Ok,
                    });
                }
            }
            None => {
                let mut srv_note: schema::Note = note.into();
                srv_note.id_user = user.id;

                srv_note.insert(&mut conn).await;

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
) -> Result<Json<Vec<shared::Note>>, StatusCode> {
    let mut conn = pool.get_conn().await.unwrap();
    user_verify(
        &mut conn,
        params.username.clone(),
        hex::decode(params.token).unwrap(),
    )
    .await?;

    let user = User::select(&mut conn, params.username).await.unwrap();

    let notes =
        schema::Note::select_all_from_user(&mut conn, user.id.unwrap(), params.updated_at).await;

    let notes: Vec<shared::Note> = notes.into_iter().map(|note| note.into()).collect();

    if !notes.is_empty() {
        println!("Some notes are sent to user");
    }

    Ok(Json(notes))
}

async fn select_note(
    State(pool): State<Pool>,
    Query(params): Query<shared::SelectNoteParams>,
) -> Result<Json<shared::Note>, StatusCode> {
    let mut conn = pool.get_conn().await.unwrap();
    user_verify(
        &mut conn,
        params.username.clone(),
        hex::decode(params.token).unwrap(),
    )
    .await?;

    let user = User::select(&mut conn, params.username).await.unwrap();

    let note = schema::Note::select(&mut conn, user.id.unwrap(), params.note_id).await
        .ok_or(StatusCode::NOT_FOUND)?;
    
    Ok(Json(note.into()))
}

async fn insert_user(State(pool): State<Pool>, Json(user): Json<shared::User>) {
    println!("received insert_user");
    let user: schema::User = user.into();

    let mut conn = pool.get_conn().await.unwrap();

    user.insert(&mut conn).await;
    println!("insert_user: completed");
}

async fn login_request(
    State(pool): State<Pool>,
    Query(params): Query<shared::LoginRequestParams>,
) -> Result<Json<shared::LoginRequest>, StatusCode> {
    let mut conn = pool.get_conn().await.unwrap();

    match schema::User::select(&mut conn, params.username).await {
        Some(user) => {
            return Ok(Json(shared::LoginRequest {
                salt_auth: user.salt_auth,
                salt_server_auth: user.salt_server_auth,
            }));
        }
        None => return Err(StatusCode::NOT_FOUND),
    };
}

#[axum::debug_handler]
async fn login(
    State(pool): State<Pool>,
    Json(params): Json<shared::LoginParams>,
) -> Result<Json<shared::Login>, StatusCode> {
    let mut conn = pool.get_conn().await.unwrap();

    //Check if login_hash is correct
    let user = schema::User::select(&mut conn, params.username)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    if params.login_hash != user.stored_password_hash {
        return Err(StatusCode::UNAUTHORIZED);
    }

    //Generate token
    let mut token = vec![0u8; 32];
    OsRng.try_fill_bytes(&mut token).unwrap();

    //Store token
    let user_token = schema::UserToken {
        id: None,
        id_user: user.id.unwrap(),
        token,
    };

    user_token.insert(&mut conn).await;

    //Response
    Ok(Json(shared::Login {
        salt_data: user.salt_data,
        encrypted_mek_password: user.encrypted_mek_password,
        mek_password_nonce: user.mek_password_nonce,
        token: user_token.token,
    }))
}
