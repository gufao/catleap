use std::fs;
use std::path::{Path, PathBuf};
use crate::models::{Game, GameSource, GameStatus, SteamApp};
use crate::steam::parser::{parse_acf, parse_library_folders};

pub fn scan_steam_library(steam_path: &Path) -> Result<Vec<SteamApp>, String> {
    if !steam_path.exists() {
        return Err(format!(
            "Steam path does not exist: {}",
            steam_path.display()
        ));
    }

    let library_folders_path = steam_path
        .join("steamapps")
        .join("libraryfolders.vdf");

    // Collect all library paths to scan
    let mut library_paths: Vec<PathBuf> = Vec::new();

    if library_folders_path.exists() {
        let content = fs::read_to_string(&library_folders_path).map_err(|e| {
            format!("Failed to read libraryfolders.vdf: {}", e)
        })?;
        let paths = parse_library_folders(&content)?;
        for p in paths {
            library_paths.push(PathBuf::from(p));
        }
    }

    // Always include the default steamapps dir of the given steam_path
    let default_steamapps = steam_path.join("steamapps");
    if !library_paths.contains(&steam_path.to_path_buf()) {
        library_paths.push(steam_path.to_path_buf());
    }

    let mut apps: Vec<SteamApp> = Vec::new();

    for lib_path in &library_paths {
        let steamapps_dir = if lib_path == steam_path {
            default_steamapps.clone()
        } else {
            lib_path.join("steamapps")
        };

        if !steamapps_dir.exists() {
            continue;
        }

        let entries = fs::read_dir(&steamapps_dir).map_err(|e| {
            format!("Failed to read directory {}: {}", steamapps_dir.display(), e)
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                if fname.starts_with("appmanifest_") && fname.ends_with(".acf") {
                    match fs::read_to_string(&path) {
                        Ok(content) => match parse_acf(&content) {
                            Ok(app) => apps.push(app),
                            Err(e) => {
                                log::warn!("Failed to parse {}: {}", path.display(), e);
                            }
                        },
                        Err(e) => {
                            log::warn!("Failed to read {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }
    }

    Ok(apps)
}

pub fn steam_app_to_game(app: &SteamApp, steam_path: &Path) -> Game {
    let install_dir = steam_path
        .join("steamapps")
        .join("common")
        .join(&app.install_dir);

    Game {
        id: format!("steam_{}", app.appid),
        name: app.name.clone(),
        source: GameSource::Steam,
        status: GameStatus::Unknown,
        install_dir,
        executable: None,
        size_bytes: app.size_on_disk,
        is_running: false,
        notes: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_acf(dir: &Path, appid: &str, name: &str, install_dir: &str, size: Option<u64>) {
        let size_line = size
            .map(|s| format!("\t\"SizeOnDisk\"\t\"{}\"\n", s))
            .unwrap_or_default();
        let content = format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\"{appid}\"\n\t\"name\"\t\"{name}\"\n\t\"installdir\"\t\"{install_dir}\"\n{size_line}}}\n"
        );
        let filename = format!("appmanifest_{}.acf", appid);
        fs::write(dir.join(filename), content).unwrap();
    }

    #[test]
    fn test_scan_steam_library() {
        let temp = TempDir::new().unwrap();
        let steam_path = temp.path().to_path_buf();
        let steamapps_dir = steam_path.join("steamapps");
        fs::create_dir_all(&steamapps_dir).unwrap();

        // Create a minimal libraryfolders.vdf pointing only at the default path
        let vdf_content = format!(
            "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\"{}\"\n\t}}\n}}\n",
            steam_path.display()
        );
        fs::write(steamapps_dir.join("libraryfolders.vdf"), vdf_content).unwrap();

        // Create two fake ACF files
        create_acf(&steamapps_dir, "1245620", "Elden Ring", "ELDEN RING", Some(50_000_000_000));
        create_acf(&steamapps_dir, "730", "Counter-Strike 2", "Counter-Strike Global Offensive", None);

        let apps = scan_steam_library(&steam_path).unwrap();
        assert_eq!(apps.len(), 2);

        let elden = apps.iter().find(|a| a.appid == "1245620").unwrap();
        assert_eq!(elden.name, "Elden Ring");
        assert_eq!(elden.size_on_disk, Some(50_000_000_000));

        let cs2 = apps.iter().find(|a| a.appid == "730").unwrap();
        assert_eq!(cs2.name, "Counter-Strike 2");
        assert_eq!(cs2.size_on_disk, None);
    }

    #[test]
    fn test_scan_nonexistent_dir() {
        let result = scan_steam_library(Path::new("/nonexistent/path/to/steam"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_steam_app_to_game() {
        let app = SteamApp {
            appid: "1245620".to_string(),
            name: "Elden Ring".to_string(),
            install_dir: "ELDEN RING".to_string(),
            size_on_disk: Some(50_000_000_000),
        };
        let steam_path = Path::new("/fake/steam");
        let game = steam_app_to_game(&app, steam_path);

        assert_eq!(game.id, "steam_1245620");
        assert_eq!(game.name, "Elden Ring");
        assert_eq!(game.source, GameSource::Steam);
        assert_eq!(game.status, GameStatus::Unknown);
        assert_eq!(
            game.install_dir,
            PathBuf::from("/fake/steam/steamapps/common/ELDEN RING")
        );
        assert_eq!(game.size_bytes, Some(50_000_000_000));
        assert!(!game.is_running);
        assert!(game.notes.is_none());
        assert!(game.executable.is_none());
    }
}
