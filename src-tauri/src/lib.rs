pub mod commands;
pub mod compat;
pub mod models;
pub mod process;
pub mod steam;
pub mod wine;

use commands::games::{add_manual_game, list_games, remove_game, scan_steam, AppState};
use commands::launcher::{get_running_games, play_game, stop_game};
use commands::settings::{get_settings, update_settings};
use compat::database;
use models::Settings;
use process::monitor::ProcessMonitor;
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
            process_monitor: ProcessMonitor::new(),
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            scan_steam,
            add_manual_game,
            remove_game,
            play_game,
            stop_game,
            get_running_games,
            get_settings,
            update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
