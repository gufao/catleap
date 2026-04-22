// Placeholder — implementation in Task 4
use std::path::Path;
use crate::models::{Game, SteamApp};

pub fn scan_steam_library(_steam_path: &Path) -> Result<Vec<SteamApp>, String> {
    Err("Not yet implemented".to_string())
}

pub fn steam_app_to_game(_app: &SteamApp, _steam_path: &Path) -> Game {
    unimplemented!("Implemented in Task 4")
}
