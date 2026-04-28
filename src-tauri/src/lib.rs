pub mod commands;
pub mod compat;
pub mod models;
pub mod process;
pub mod steam;
pub mod wine;

use commands::games::{add_manual_game, list_games, read_game_log, remove_game, scan_steam, AppState};
use commands::launcher::{check_wine_status, get_running_games, play_game, stop_game};
use commands::onboarding::{cancel_wine_install, eject_gptk_volume, skip_gptk, start_gptk_watch, start_wine_install, stop_gptk_watch};
use commands::settings::{get_settings, update_settings};
use commands::steam_runtime::{
    cancel_steam_install, launch_steam_runtime, reset_steam_runtime,
    start_steam_install, stop_steam_runtime,
};
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

fn setup_steam_watcher(app_handle: tauri::AppHandle, steam_path: std::path::PathBuf) {
    let watch_path = steam_path.join("steamapps");
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("Failed to create Steam watcher: {e}");
                return;
            }
        };
        if let Err(e) = watcher.watch(&watch_path, RecursiveMode::NonRecursive) {
            log::warn!("Failed to watch {:?}: {e}", watch_path);
            return;
        }
        for res in rx {
            match res {
                Ok(event) if matches!(event.kind, EventKind::Create(_) | EventKind::Remove(_)) => {
                    let _ = app_handle.emit("steam-library-changed", ());
                }
                Ok(_) => {}
                Err(e) => log::warn!("Steam watcher error: {e}"),
            }
        }
    });
}

fn setup_steam_runtime_watcher(app_handle: tauri::AppHandle, data_path: std::path::PathBuf) {
    let watch_path = data_path
        .join("prefixes/_steam_runtime/drive_c/Program Files (x86)/Steam/steamapps");
    std::thread::spawn(move || {
        // Wait until the prefix exists before attaching. Poll lazily.
        loop {
            if watch_path.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("Failed to create steam runtime watcher: {e}");
                return;
            }
        };
        if let Err(e) = watcher.watch(&watch_path, RecursiveMode::NonRecursive) {
            log::warn!("Failed to watch {:?}: {e}", watch_path);
            return;
        }
        for res in rx {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Remove(_)) {
                    let _ = app_handle.emit("steam-library-changed", ());
                }
            }
        }
    });
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
            install_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            gptk_watching: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            steam_install_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            steam_installing: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            let state: tauri::State<AppState> = app.state();
            let steam_path = state.settings.lock().unwrap().steam_path.clone();
            setup_steam_watcher(app_handle.clone(), steam_path);
            let data_path = state.settings.lock().unwrap().data_path.clone();
            setup_steam_runtime_watcher(app_handle, data_path);
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
            check_wine_status,
            start_wine_install,
            cancel_wine_install,
            start_gptk_watch,
            stop_gptk_watch,
            skip_gptk,
            eject_gptk_volume,
            start_steam_install,
            cancel_steam_install,
            launch_steam_runtime,
            stop_steam_runtime,
            reset_steam_runtime,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
