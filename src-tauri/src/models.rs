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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GameSource {
    Steam,
    Manual,
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
}

impl Default for Settings {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self {
            steam_path: home.join("Library/Application Support/Steam"),
            data_path: home.join("Library/Application Support/Catleap"),
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
