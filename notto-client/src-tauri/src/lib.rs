// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
// #[tauri::command(rename_all = "snake_case")]

use std::{env, thread::sleep, time::Duration};

use tokio::sync::Mutex;

use aes_gcm::{Aes256Gcm, Key};
use rusqlite::Connection;
use tauri::Manager;
use tauri_plugin_log::log::{LevelFilter, debug};

use crate::db::schema;

mod commands;
mod crypt;
mod db;
mod sync;

#[derive(Debug)]
pub struct AppState {
    database: Mutex<Connection>,
    workspace: Option<db::schema::Workspace>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_level = env::var("NOTTO_LOG")
        .ok()
        .and_then(|s| s.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info);

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                // .level(tauri_plugin_log::log::LevelFilter::Trace)
                .level(log_level)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db_path = app.path().app_data_dir().unwrap().join("notto.db");

            let app_state = Mutex::new(AppState {
                database: db::init(db_path).unwrap(),
                workspace: None,
            });

            let app_handle_clone = app.app_handle().clone();
            tauri::async_runtime::spawn(sync::service::run(app_handle_clone));

            app.manage(app_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::init,
            commands::create_note,
            commands::get_note,
            commands::edit_note,
            commands::get_all_notes_metadata,
            commands::create_workspace,
            commands::get_workspaces,
            commands::set_logged_workspace,
            commands::get_logged_workspace,
            commands::sync_create_account,
            commands::sync_login,
            commands::sync_logout,
            commands::logout,
            commands::get_version,
            commands::delete_note,
            commands::restore_note,
            commands::get_latest_note_id,
            commands::handle_conflict
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
