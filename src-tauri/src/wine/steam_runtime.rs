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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
