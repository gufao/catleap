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

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SteamInstallPhase {
    InitializingPrefix,
    InstallingMono,
    InstallingGecko,
    ConfiguringPrefix,
    DownloadingInstaller { bytes_done: u64, bytes_total: u64 },
    LaunchingInstaller,
    Done,
    Failed { error: String },
}

/// Run wineboot --init then trigger Mono/Gecko auto-install via wineboot --update.
/// Idempotent: if `<prefix>/system.reg` exists, returns Ok immediately without
/// emitting any phase events.
pub fn bootstrap_prefix(
    data_path: &Path,
    wine_binary: &Path,
    cancelled: Arc<AtomicBool>,
    mut emit_phase: impl FnMut(SteamInstallPhase),
) -> Result<(), String> {
    use crate::wine::wine_command;

    let prefix = runtime_prefix_path(data_path);

    if prefix.join("system.reg").exists() {
        return Ok(());
    }

    std::fs::create_dir_all(&prefix).map_err(|e| format!("mkdir prefix: {e}"))?;

    let check_cancel = || -> Result<(), String> {
        if cancelled.load(Ordering::Relaxed) {
            Err("cancelled".into())
        } else {
            Ok(())
        }
    };

    // 1. wineboot --init
    emit_phase(SteamInstallPhase::InitializingPrefix);
    let status = wine_command(wine_binary)
        .arg("wineboot")
        .arg("--init")
        .env("WINEPREFIX", &prefix)
        .env("WINEARCH", "win64")
        .env("WINEDEBUG", "-all")
        .status()
        .map_err(|e| format!("wineboot --init: {e}"))?;
    if !status.success() {
        return Err(format!("wineboot --init exit {}", status.code().unwrap_or(-1)));
    }
    check_cancel()?;

    // 2. Mono (wineboot --update triggers Mono auto-download when missing)
    emit_phase(SteamInstallPhase::InstallingMono);
    let _ = wine_command(wine_binary)
        .arg("wineboot")
        .arg("--update")
        .env("WINEPREFIX", &prefix)
        .env("WINEARCH", "win64")
        .env("WINEDEBUG", "-all")
        .status();
    check_cancel()?;

    // 3. Gecko triggers on first browser instantiation; force it now via reg
    //    query that touches IE-related paths.
    emit_phase(SteamInstallPhase::InstallingGecko);
    let _ = wine_command(wine_binary)
        .args(["reg", "query", r"HKEY_CURRENT_USER\Software\Wine\MSHTML"])
        .env("WINEPREFIX", &prefix)
        .env("WINEARCH", "win64")
        .env("WINEDEBUG", "-all")
        .status();
    check_cancel()?;

    // 4. Set Windows version to win10 — some Steam features check the OS.
    emit_phase(SteamInstallPhase::ConfiguringPrefix);
    let _ = wine_command(wine_binary)
        .args([
            "reg", "add", r"HKEY_CURRENT_USER\Software\Wine",
            "/v", "Version", "/d", "win10", "/f",
        ])
        .env("WINEPREFIX", &prefix)
        .env("WINEARCH", "win64")
        .env("WINEDEBUG", "-all")
        .status();

    Ok(())
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

    #[test]
    fn bootstrap_prefix_skips_when_system_reg_exists() {
        let tmp = TempDir::new().unwrap();
        let prefix = runtime_prefix_path(tmp.path());
        std::fs::create_dir_all(&prefix).unwrap();
        std::fs::write(prefix.join("system.reg"), b"existing").unwrap();

        let cancelled = Arc::new(AtomicBool::new(false));
        let mut phases_emitted = 0;
        let result = bootstrap_prefix(
            tmp.path(),
            std::path::Path::new("/nonexistent/wine64"),
            cancelled,
            |_| { phases_emitted += 1; },
        );
        assert!(result.is_ok());
        assert_eq!(phases_emitted, 0, "should skip emitting any phase when prefix already exists");
    }
}
