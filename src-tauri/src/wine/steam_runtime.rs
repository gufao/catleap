use std::path::{Path, PathBuf};

/// Path to the shared Wine prefix Catleap uses for Steam-Windows.
pub fn runtime_prefix_path(data_path: &Path) -> PathBuf {
    data_path.join("prefixes/_steam_runtime")
}

/// Path to Steam.exe inside the runtime prefix.
pub fn steam_exe_path(data_path: &Path) -> PathBuf {
    runtime_prefix_path(data_path)
        .join("drive_c/Program Files (x86)/Steam/Steam.exe")
}

/// Path to the cached SteamSetup.exe download.
pub fn cached_installer_path(data_path: &Path) -> PathBuf {
    data_path.join("cache/SteamSetup.exe")
}

/// Path to Steam-Windows' steamapps directory inside the runtime prefix.
pub fn steamapps_path(data_path: &Path) -> PathBuf {
    runtime_prefix_path(data_path)
        .join("drive_c/Program Files (x86)/Steam/steamapps")
}

/// True iff Steam.exe exists at the expected path.
pub fn is_installed(data_path: &Path) -> bool {
    steam_exe_path(data_path).exists()
}

/// Sentinel id used in ProcessMonitor for Steam-Windows itself
/// (as distinct from Wine-Steam game ids).
pub const STEAM_RUNTIME_ID: &str = "_steam_runtime";

use crate::compat::database::apply_compat_data;
use crate::models::{CompatDatabase, Game, GameSource, GameStatus};
use crate::steam::scanner::scan_steam_library;

/// Scan Steam-Windows' library inside the runtime prefix.
/// Returns Games with `source: SteamWine` and ids `"steam_wine_<appid>"`.
pub fn scan_wine_steam(
    data_path: &Path,
    compat_db: &CompatDatabase,
) -> Result<Vec<Game>, String> {
    let steam_root = runtime_prefix_path(data_path)
        .join("drive_c/Program Files (x86)/Steam");
    if !steam_root.exists() {
        return Ok(vec![]);
    }

    // Reuse the existing scanner — it parses libraryfolders.vdf + appmanifest_*.acf.
    let apps = scan_steam_library(&steam_root)?;

    let steamapps = steam_root.join("steamapps");
    let mut games: Vec<Game> = apps
        .iter()
        .map(|app| Game {
            id: format!("steam_wine_{}", app.appid),
            name: app.name.clone(),
            source: GameSource::SteamWine,
            status: GameStatus::Unknown,
            install_dir: steamapps.join("common").join(&app.install_dir),
            executable: None,
            size_bytes: app.size_on_disk,
            is_running: false,
            notes: None,
        })
        .collect();

    apply_compat_data(&mut games, compat_db);
    Ok(games)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::models::{CompatDatabase, GameSource};

    fn empty_compat_db() -> CompatDatabase {
        CompatDatabase { version: "test".into(), games: vec![] }
    }

    fn write_appmanifest(steamapps: &Path, appid: &str, name: &str, installdir: &str) {
        let acf = format!(
            r#""AppState"
{{
    "appid"        "{appid}"
    "name"        "{name}"
    "installdir"        "{installdir}"
    "SizeOnDisk"        "1234567"
}}"#
        );
        std::fs::write(steamapps.join(format!("appmanifest_{appid}.acf")), acf).unwrap();
    }

    fn write_library_folders(steamapps: &Path) {
        let vdf = format!(
            r#""libraryfolders"
{{
    "0"
    {{
        "path"        "{}"
    }}
}}"#,
            steamapps.parent().unwrap().display()
        );
        std::fs::write(steamapps.join("libraryfolders.vdf"), vdf).unwrap();
    }

    #[test]
    fn scan_wine_steam_empty_when_prefix_missing() {
        let tmp = TempDir::new().unwrap();
        let games = scan_wine_steam(tmp.path(), &empty_compat_db()).unwrap();
        assert!(games.is_empty());
    }

    #[test]
    fn scan_wine_steam_returns_games_with_correct_id_and_source() {
        let tmp = TempDir::new().unwrap();
        let steamapps = steamapps_path(tmp.path());
        std::fs::create_dir_all(steamapps.join("common/Stardew Valley")).unwrap();
        write_library_folders(&steamapps);
        write_appmanifest(&steamapps, "413150", "Stardew Valley", "Stardew Valley");

        let games = scan_wine_steam(tmp.path(), &empty_compat_db()).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].id, "steam_wine_413150");
        assert_eq!(games[0].name, "Stardew Valley");
        assert_eq!(games[0].source, GameSource::SteamWine);
        assert!(games[0].install_dir.ends_with("Stardew Valley"));
    }

    #[test]
    fn paths_are_under_data_path() {
        let dp = Path::new("/tmp/data");
        assert_eq!(runtime_prefix_path(dp), Path::new("/tmp/data/prefixes/_steam_runtime"));
        assert!(steam_exe_path(dp).starts_with(runtime_prefix_path(dp)));
        assert!(steamapps_path(dp).starts_with(runtime_prefix_path(dp)));
        assert_eq!(cached_installer_path(dp), Path::new("/tmp/data/cache/SteamSetup.exe"));
    }

    #[test]
    fn is_installed_false_when_steam_exe_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_installed(tmp.path()));
    }

    #[test]
    fn is_installed_true_when_steam_exe_present() {
        let tmp = TempDir::new().unwrap();
        let exe = steam_exe_path(tmp.path());
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        assert!(is_installed(tmp.path()));
    }
}
