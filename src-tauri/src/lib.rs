pub mod commands;
pub mod compat;
pub mod models;
pub mod process;
pub mod steam;
pub mod wine;

use commands::games::{add_manual_game, list_games, read_game_log, remove_game, scan_steam, AppState};
use commands::launcher::{get_running_games, play_game, stop_game};
use commands::settings::{get_settings, update_settings};
use compat::database;
use models::Settings;
use notify::{EventKind, RecursiveMode, Watcher};
use process::monitor::ProcessMonitor;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

/// Load settings from disk at startup; falls back to defaults if the file
/// does not exist or cannot be parsed.
fn load_settings_from_disk() -> Settings {
    let default = Settings::default();
    let settings_path = default.data_path.join("config").join("settings.json");
    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            if let Ok(settings) = serde_json::from_str::<Settings>(&content) {
                log::info!("Loaded settings from {:?}", settings_path);
                return settings;
            }
        }
    }
    default
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let compat_db = database::load_embedded_database()
        .expect("Failed to load embedded compat database");

    let settings = load_settings_from_disk();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            games: Mutex::new(Vec::new()),
            compat_db,
            settings: Mutex::new(settings),
            process_monitor: ProcessMonitor::new(),
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            let state: tauri::State<AppState> = app.state();
            let steam_path = state.settings.lock().unwrap().steam_path.clone();
            let watch_path = steam_path.join("steamapps");

            std::thread::spawn(move || {
                let (tx, rx) = std::sync::mpsc::channel();

                let mut watcher = match notify::recommended_watcher(move |res| {
                    let _ = tx.send(res);
                }) {
                    Ok(w) => w,
                    Err(e) => {
                        log::warn!("Failed to create file watcher: {}", e);
                        return;
                    }
                };

                if let Err(e) = watcher.watch(&watch_path, RecursiveMode::NonRecursive) {
                    log::warn!("Failed to watch {:?}: {}", watch_path, e);
                    return;
                }

                for res in rx {
                    match res {
                        Ok(event) => {
                            let relevant = matches!(
                                event.kind,
                                EventKind::Create(_) | EventKind::Remove(_)
                            );
                            if relevant {
                                let _ = app_handle.emit("steam-library-changed", ());
                            }
                        }
                        Err(e) => {
                            log::warn!("Watch error: {}", e);
                        }
                    }
                }
            });

            Ok(())
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
            read_game_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
