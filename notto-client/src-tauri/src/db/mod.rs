use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::Connection;
use tauri_plugin_log::log::{debug, trace};
use tokio::sync::Mutex;

pub mod operations;
pub mod schema;

pub fn init(db_path: PathBuf) -> Result<Mutex<Connection>> {
    debug!("creating/opening database at {db_path:?}");
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {:?}", db_path))?;
    trace!("db create correctly: {conn:?}");

    conn.execute("PRAGMA foreign_keys = ON", [])
        .context("Failed to enable foreign keys")?;

    schema::Note::create(&conn)?;
    schema::Workspace::create(&conn)?;
    schema::Common::create(&conn)?;
    trace!("Tables have been created correctly");

    Ok(Mutex::new(conn))
}
