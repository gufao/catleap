use crate::commands::games::AppState;
use crate::models::Settings;
use std::fs;
use tauri::State;

/// Persist `settings` to `<data_path>/config/settings.json`.
/// Returns Err with a clear message on any I/O failure.
pub fn save_settings_to_disk(settings: &Settings) -> Result<(), String> {
    let config_dir = settings.data_path.join("config");
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config dir: {}", e))?;

    let settings_path = config_dir.join("settings.json");
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(&settings_path, json)
        .map_err(|e| format!("Failed to write settings: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn update_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    save_settings_to_disk(&settings)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}
