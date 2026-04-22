use crate::compat::database;
use crate::models::{CompatDatabase, Game, GameSource, GameStatus, Settings};
use crate::process::monitor::ProcessMonitor;
use crate::steam::scanner;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub games: Mutex<Vec<Game>>,
    pub compat_db: CompatDatabase,
    pub settings: Mutex<Settings>,
    pub process_monitor: ProcessMonitor,
}

#[tauri::command]
pub fn list_games(state: State<AppState>) -> Vec<Game> {
    state.games.lock().unwrap().clone()
}

#[tauri::command]
pub fn scan_steam(state: State<AppState>) -> Result<Vec<Game>, String> {
    let steam_path = {
        let settings = state.settings.lock().unwrap();
        settings.steam_path.clone()
    };

    let steam_apps = scanner::scan_steam_library(&steam_path)?;

    let mut scanned_games: Vec<Game> = steam_apps
        .iter()
        .map(|app| scanner::steam_app_to_game(app, &steam_path))
        .collect();

    database::apply_compat_data(&mut scanned_games, &state.compat_db);

    // Merge: keep manual games, replace steam games with freshly scanned ones
    let mut games = state.games.lock().unwrap();
    let manual_games: Vec<Game> = games
        .iter()
        .filter(|g| g.source == GameSource::Manual)
        .cloned()
        .collect();

    let mut merged = scanned_games;
    merged.extend(manual_games);
    *games = merged.clone();

    Ok(merged)
}

#[tauri::command]
pub fn add_manual_game(
    state: State<AppState>,
    name: String,
    executable_path: String,
) -> Result<Game, String> {
    let exe_path = PathBuf::from(&executable_path);

    if !exe_path.exists() {
        return Err(format!(
            "Executable does not exist: {}",
            executable_path
        ));
    }

    let install_dir = exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));

    let game = Game {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        source: GameSource::Manual,
        status: GameStatus::Unknown,
        install_dir,
        executable: Some(exe_path),
        size_bytes: None,
        is_running: false,
        notes: None,
    };

    state.games.lock().unwrap().push(game.clone());

    Ok(game)
}

#[tauri::command]
pub fn remove_game(state: State<AppState>, game_id: String) -> Result<(), String> {
    let mut games = state.games.lock().unwrap();
    let original_len = games.len();
    games.retain(|g| g.id != game_id);

    if games.len() == original_len {
        return Err(format!("Game not found: {}", game_id));
    }

    Ok(())
}

#[tauri::command]
pub fn read_game_log(state: State<AppState>, game_id: String) -> Result<String, String> {
    let data_path = state.settings.lock().unwrap().data_path.clone();
    let logs_dir = data_path.join("logs");

    // Determine source prefix from game
    let source = {
        let games = state.games.lock().unwrap();
        games
            .iter()
            .find(|g| g.id == game_id)
            .map(|g| format!("{:?}", g.source).to_lowercase())
            .unwrap_or_else(|| "unknown".to_string())
    };

    let log_path = logs_dir.join(format!("{}_{}.log", source, game_id));

    if !log_path.exists() {
        return Ok(String::new());
    }

    fs::read_to_string(&log_path)
        .map_err(|e| format!("Failed to read log: {}", e))
}
