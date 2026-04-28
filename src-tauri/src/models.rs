use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GameStatus {
    Compatible,
    Experimental,
    Incompatible,
    Unknown,
}

// snake_case is wire-compatible with the old `lowercase` rule for the
// single-word variants (Steam → "steam", Manual → "manual") and correctly
// produces "steam_wine" for the new multi-word variant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GameSource {
    Steam,
    SteamWine,
    Manual,
}

impl GameSource {
    /// Stable, snake_case-style identifier used for filesystem paths and
    /// log filenames. Distinct from `Debug` (which is PascalCase) and
    /// equivalent to the serde wire form. Always use this when building
    /// paths from a `GameSource` so all three contexts agree.
    pub fn as_path_str(&self) -> &'static str {
        match self {
            GameSource::Steam => "steam",
            GameSource::SteamWine => "steam_wine",
            GameSource::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub source: GameSource,
    pub status: GameStatus,
    pub install_dir: PathBuf,
    pub executable: Option<PathBuf>,
    pub size_bytes: Option<u64>,
    pub is_running: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatEntry {
    pub appid: String,
    pub name: String,
    pub status: GameStatus,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub dll_overrides: Vec<String>,
    #[serde(default)]
    pub launch_args: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatDatabase {
    pub version: String,
    pub games: Vec<CompatEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub steam_path: PathBuf,
    pub data_path: PathBuf,
    #[serde(default)]
    pub wine_version: Option<String>,
    #[serde(default)]
    pub gptk_version: Option<String>,
    #[serde(default)]
    pub gptk_skipped: bool,
    #[serde(default)]
    pub steam_runtime_installed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self {
            steam_path: home.join("Library/Application Support/Steam"),
            data_path: home.join("Library/Application Support/Catleap"),
            wine_version: None,
            gptk_version: None,
            gptk_skipped: false,
            steam_runtime_installed: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SteamApp {
    pub appid: String,
    pub name: String,
    pub install_dir: String,
    pub size_on_disk: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_has_no_versions() {
        let s = Settings::default();
        assert_eq!(s.wine_version, None);
        assert_eq!(s.gptk_version, None);
        assert!(!s.gptk_skipped);
    }

    #[test]
    fn settings_round_trip_with_versions() {
        let mut s = Settings::default();
        s.wine_version = Some("1.0.0".to_string());
        s.gptk_version = Some("3.0".to_string());
        s.gptk_skipped = true;
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.wine_version, Some("1.0.0".to_string()));
        assert_eq!(back.gptk_version, Some("3.0".to_string()));
        assert!(back.gptk_skipped);
    }

    #[test]
    fn settings_old_json_loads_with_defaults() {
        // Old config files don't have the new fields — must still deserialize.
        let old = r#"{"steam_path":"/tmp/steam","data_path":"/tmp/data"}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.wine_version, None);
        assert_eq!(s.gptk_version, None);
        assert!(!s.gptk_skipped);
    }

    #[test]
    fn settings_default_has_steam_runtime_off() {
        let s = Settings::default();
        assert!(!s.steam_runtime_installed);
    }

    #[test]
    fn settings_old_json_loads_steam_runtime_default() {
        let old = r#"{"steam_path":"/tmp/s","data_path":"/tmp/d"}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert!(!s.steam_runtime_installed);
    }

    #[test]
    fn game_source_serializes_with_underscore() {
        let s = serde_json::to_string(&GameSource::SteamWine).unwrap();
        assert_eq!(s, "\"steam_wine\"");
        let m = serde_json::to_string(&GameSource::Manual).unwrap();
        assert_eq!(m, "\"manual\"");
        let st = serde_json::to_string(&GameSource::Steam).unwrap();
        assert_eq!(st, "\"steam\"");
    }

    #[test]
    fn game_source_as_path_str_matches_wire_form() {
        assert_eq!(GameSource::Steam.as_path_str(), "steam");
        assert_eq!(GameSource::SteamWine.as_path_str(), "steam_wine");
        assert_eq!(GameSource::Manual.as_path_str(), "manual");
    }
}
