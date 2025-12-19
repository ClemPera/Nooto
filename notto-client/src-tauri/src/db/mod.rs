use std::{path::PathBuf};

use tokio::sync::Mutex;

use rusqlite::Connection;
use tauri_plugin_log::log::{debug, trace};

pub mod operations;
pub mod schema;

pub fn init(db_path: PathBuf) -> Result<Mutex<Connection>, Box<dyn std::error::Error>> {
    debug!("creating/opening database at {db_path:?}");
    let conn = Connection::open(db_path).unwrap();
    trace!("db create correctly: {conn:?}");
    
    conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
    
    // Create tables
    schema::Note::create(&conn)?;
    schema::User::create(&conn)?;
    schema::Common::create(&conn)?;
    trace!("Tables have been created correctly");

    Ok(Mutex::new(conn))
}