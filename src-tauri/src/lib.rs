pub mod commands;
pub mod compat;
pub mod models;
pub mod steam;

use commands::games::{add_manual_game, list_games, remove_game, scan_steam, AppState};
use compat::database;
use models::Settings;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let compat_db = database::load_embedded_database()
        .expect("Failed to load embedded compat database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            games: Mutex::new(Vec::new()),
            compat_db,
            settings: Mutex::new(Settings::default()),
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            scan_steam,
            add_manual_game,
            remove_game,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
