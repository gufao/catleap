use crate::commands::games::AppState;
use tauri::State;

/// Launch a game by ID and begin tracking its process.
#[tauri::command]
pub fn play_game(state: State<AppState>, game_id: String) -> Result<(), String> {
    let (game, data_path) = {
        let games = state.games.lock().unwrap();
        let game = games
            .iter()
            .find(|g| g.id == game_id)
            .cloned()
            .ok_or_else(|| format!("Game not found: {}", game_id))?;
        let data_path = state.settings.lock().unwrap().data_path.clone();
        (game, data_path)
    };

    let child = crate::wine::runner::launch_game(&game, &data_path, &state.compat_db)?;

    state
        .process_monitor
        .track(game_id.clone(), child);

    // Mark the game as running in the games list
    let mut games = state.games.lock().unwrap();
    if let Some(g) = games.iter_mut().find(|g| g.id == game_id) {
        g.is_running = true;
    }

    Ok(())
}

/// Stop a running game by ID.
#[tauri::command]
pub fn stop_game(state: State<AppState>, game_id: String) -> Result<(), String> {
    state.process_monitor.stop(&game_id)?;

    let mut games = state.games.lock().unwrap();
    if let Some(g) = games.iter_mut().find(|g| g.id == game_id) {
        g.is_running = false;
    }

    Ok(())
}

/// Return the IDs of all currently running games, and synchronise is_running flags.
#[tauri::command]
pub fn get_running_games(state: State<AppState>) -> Vec<String> {
    let running_ids = state.process_monitor.running_game_ids();

    let mut games = state.games.lock().unwrap();
    for game in games.iter_mut() {
        game.is_running = running_ids.contains(&game.id);
    }

    running_ids
}
