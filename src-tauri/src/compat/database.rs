use std::path::Path;
use crate::models::{CompatDatabase, CompatEntry, Game};

pub fn load_database(path: &Path) -> Result<CompatDatabase, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read compat database at {}: {}", path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse compat database: {}", e))
}

pub fn load_embedded_database() -> Result<CompatDatabase, String> {
    let content = include_str!("../../resources/compat.json");
    serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse embedded compat database: {}", e))
}

pub fn lookup_game<'a>(db: &'a CompatDatabase, appid: &str) -> Option<&'a CompatEntry> {
    db.games.iter().find(|g| g.appid == appid)
}

pub fn apply_compat_data(games: &mut [Game], db: &CompatDatabase) {
    for game in games.iter_mut() {
        // Extract appid from game id: format is "steam_<appid>"
        if let Some(appid) = game.id.strip_prefix("steam_") {
            if let Some(entry) = lookup_game(db, appid) {
                game.status = entry.status.clone();
                game.notes = entry.notes.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{GameSource, GameStatus};
    use std::path::PathBuf;

    fn make_game(appid: &str, name: &str) -> Game {
        Game {
            id: format!("steam_{}", appid),
            name: name.to_string(),
            source: GameSource::Steam,
            status: GameStatus::Unknown,
            install_dir: PathBuf::from("/fake/steam/steamapps/common/game"),
            executable: None,
            size_bytes: None,
            is_running: false,
            notes: None,
        }
    }

    #[test]
    fn test_lookup_existing_game() {
        let db = load_embedded_database().unwrap();
        let entry = lookup_game(&db, "1245620");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name, "Elden Ring");
        assert_eq!(entry.status, GameStatus::Compatible);
    }

    #[test]
    fn test_lookup_missing_game() {
        let db = load_embedded_database().unwrap();
        let entry = lookup_game(&db, "9999999");
        assert!(entry.is_none());
    }

    #[test]
    fn test_apply_compat_data() {
        let db = load_embedded_database().unwrap();

        // One game that matches (Elden Ring), one that doesn't
        let mut games = vec![
            make_game("1245620", "Elden Ring"),
            make_game("99999", "Unknown Game"),
        ];

        apply_compat_data(&mut games, &db);

        let elden = games.iter().find(|g| g.id == "steam_1245620").unwrap();
        assert_eq!(elden.status, GameStatus::Compatible);
        assert!(elden.notes.is_some());

        let unknown = games.iter().find(|g| g.id == "steam_99999").unwrap();
        assert_eq!(unknown.status, GameStatus::Unknown);
        assert!(unknown.notes.is_none());
    }

    #[test]
    fn test_load_embedded_database() {
        let db = load_embedded_database().unwrap();
        assert_eq!(db.version, "0.1.0");
        assert_eq!(db.games.len(), 10);

        // Spot-check a few entries
        let overwatch = lookup_game(&db, "2357570").unwrap();
        assert_eq!(overwatch.name, "Overwatch 2");
        assert_eq!(overwatch.status, GameStatus::Incompatible);

        let stardew = lookup_game(&db, "413150").unwrap();
        assert_eq!(stardew.name, "Stardew Valley");
        assert_eq!(stardew.status, GameStatus::Compatible);
    }
}
