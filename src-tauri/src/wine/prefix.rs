use crate::models::CompatEntry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Create a new Wine prefix by running `wineboot --init`.
pub fn create_prefix(wine_binary: &Path, prefix_path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(prefix_path).map_err(|e| {
        format!(
            "Failed to create prefix directory {}: {}",
            prefix_path.display(),
            e
        )
    })?;

    let status = crate::wine::wine_command(wine_binary)
        .arg("wineboot")
        .arg("--init")
        .env("WINEPREFIX", prefix_path)
        .env("WINEARCH", "win64")
        .status()
        .map_err(|e| format!("Failed to run wineboot: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "wineboot exited with status: {}",
            status.code().unwrap_or(-1)
        ))
    }
}

/// Check whether a prefix exists by looking for system.reg.
pub fn prefix_exists(prefix_path: &Path) -> bool {
    prefix_path.join("system.reg").exists()
}

/// Apply DLL overrides via `wine reg add`.
pub fn apply_dll_overrides(
    wine_binary: &Path,
    prefix_path: &Path,
    overrides: &[String],
) -> Result<(), String> {
    for dll in overrides {
        let status = crate::wine::wine_command(wine_binary)
            .args([
                "reg",
                "add",
                r"HKEY_CURRENT_USER\Software\Wine\DllOverrides",
                "/v",
                dll,
                "/d",
                "native,builtin",
                "/f",
            ])
            .env("WINEPREFIX", prefix_path)
            .env("WINEARCH", "win64")
            .status()
            .map_err(|e| format!("Failed to run wine reg add for {}: {}", dll, e))?;

        if !status.success() {
            return Err(format!(
                "wine reg add failed for {} with status: {}",
                dll,
                status.code().unwrap_or(-1)
            ));
        }
    }
    Ok(())
}

/// Configure a prefix using settings from a CompatEntry.
pub fn configure_prefix(
    wine_binary: &Path,
    prefix_path: &Path,
    compat: &CompatEntry,
) -> Result<(), String> {
    if !compat.dll_overrides.is_empty() {
        apply_dll_overrides(wine_binary, prefix_path, &compat.dll_overrides)?;
    }
    Ok(())
}

/// Remove a Wine prefix directory.
pub fn delete_prefix(prefix_path: &Path) -> Result<(), String> {
    if prefix_path.exists() {
        std::fs::remove_dir_all(prefix_path).map_err(|e| {
            format!(
                "Failed to delete prefix {}: {}",
                prefix_path.display(),
                e
            )
        })?;
    }
    Ok(())
}

/// Build the environment map needed to launch a game with Wine.
pub fn build_launch_env(
    wine_binary: &Path,
    prefix_path: &Path,
    compat: Option<&CompatEntry>,
    gptk_lib_path: Option<&Path>,
) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = HashMap::new();

    env.insert(
        "WINEPREFIX".to_string(),
        prefix_path.to_string_lossy().to_string(),
    );
    env.insert("WINEARCH".to_string(), "win64".to_string());

    // Add the wine binary's parent directory to PATH so helper tools are found
    if let Some(bin_dir) = wine_binary.parent() {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bin_dir.display(), current_path);
        env.insert("PATH".to_string(), new_path);
    }

    if let Some(gptk) = gptk_lib_path {
        let gptk_str = gptk.to_string_lossy().to_string();
        env.insert(
            "DYLD_FALLBACK_LIBRARY_PATH".to_string(),
            format!("{}:{}/external", gptk_str, gptk_str),
        );
        env.insert("WINEESYNC".to_string(), "1".to_string());
        env.insert("WINEMSYNC".to_string(), "1".to_string());
        env.insert("ROSETTA_ADVERTISE_AVX".to_string(), "1".to_string());

        // <wine_root>/lib/wine/x86_64-windows for compiled-in PE DLLs
        if let Some(bin_dir) = wine_binary.parent() {
            if let Some(wine_root) = bin_dir.parent() {
                let dll_path = wine_root.join("lib/wine/x86_64-windows");
                env.insert(
                    "WINEDLLPATH".to_string(),
                    dll_path.to_string_lossy().to_string(),
                );
            }
        }
    }

    if let Some(entry) = compat {
        for (key, value) in &entry.env {
            env.insert(key.clone(), value.clone());
        }
    }

    env
}

/// Return the canonical path for a game's Wine prefix.
pub fn get_prefix_path(data_path: &Path, game_id: &str, source: &str) -> PathBuf {
    data_path
        .join("prefixes")
        .join(format!("{}_{}", source, game_id))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{GameStatus};
    use tempfile::TempDir;

    fn make_compat_entry(dll_overrides: Vec<&str>, env: Vec<(&str, &str)>) -> CompatEntry {
        CompatEntry {
            appid: "12345".to_string(),
            name: "Test Game".to_string(),
            status: GameStatus::Compatible,
            env: env
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            dll_overrides: dll_overrides.into_iter().map(|s| s.to_string()).collect(),
            launch_args: vec![],
            notes: None,
        }
    }

    #[test]
    fn test_prefix_exists_false() {
        let tmp = TempDir::new().unwrap();
        let prefix = tmp.path().join("nonexistent_prefix");
        assert!(!prefix_exists(&prefix));
    }

    #[test]
    fn test_prefix_exists_true() {
        let tmp = TempDir::new().unwrap();
        let prefix = tmp.path().join("my_prefix");
        std::fs::create_dir_all(&prefix).unwrap();
        std::fs::write(prefix.join("system.reg"), b"").unwrap();
        assert!(prefix_exists(&prefix));
    }

    #[test]
    fn test_get_prefix_path() {
        let data_path = PathBuf::from("/tmp/catleap_data");
        let result = get_prefix_path(&data_path, "1245620", "steam");
        assert_eq!(result, PathBuf::from("/tmp/catleap_data/prefixes/steam_1245620"));
    }

    #[test]
    fn test_build_launch_env_without_compat() {
        let wine_binary = PathBuf::from("/opt/homebrew/bin/wine64");
        let prefix_path = PathBuf::from("/tmp/catleap_data/prefixes/steam_123");
        let env = build_launch_env(&wine_binary, &prefix_path, None, None);

        assert_eq!(
            env.get("WINEPREFIX").unwrap(),
            "/tmp/catleap_data/prefixes/steam_123"
        );
        assert_eq!(env.get("WINEARCH").unwrap(), "win64");
        // No extra compat keys
        assert!(env.get("DXVK_HUD").is_none());
    }

    #[test]
    fn test_build_launch_env_with_compat() {
        let wine_binary = PathBuf::from("/opt/homebrew/bin/wine64");
        let prefix_path = PathBuf::from("/tmp/catleap_data/prefixes/steam_123");
        let compat = make_compat_entry(vec![], vec![("DXVK_HUD", "1"), ("WINEDEBUG", "-all")]);

        let env = build_launch_env(&wine_binary, &prefix_path, Some(&compat), None);

        assert_eq!(env.get("WINEPREFIX").unwrap(), "/tmp/catleap_data/prefixes/steam_123");
        assert_eq!(env.get("WINEARCH").unwrap(), "win64");
        assert_eq!(env.get("DXVK_HUD").unwrap(), "1");
        assert_eq!(env.get("WINEDEBUG").unwrap(), "-all");
    }

    #[test]
    fn build_launch_env_with_gptk_sets_d3dmetal_vars() {
        let wine_binary = PathBuf::from("/tmp/data/wine/bin/wine64");
        let prefix_path = PathBuf::from("/tmp/data/prefixes/steam_123");
        let gptk = PathBuf::from("/tmp/data/gptk/lib");

        let env = build_launch_env(&wine_binary, &prefix_path, None, Some(&gptk));

        assert_eq!(
            env.get("DYLD_FALLBACK_LIBRARY_PATH").unwrap(),
            "/tmp/data/gptk/lib:/tmp/data/gptk/lib/external"
        );
        assert_eq!(env.get("WINEESYNC").unwrap(), "1");
        assert_eq!(env.get("WINEMSYNC").unwrap(), "1");
        assert_eq!(env.get("ROSETTA_ADVERTISE_AVX").unwrap(), "1");
        assert!(env.get("WINEDLLPATH").unwrap().contains("/tmp/data/wine/lib/wine/x86_64-windows"));
    }

    #[test]
    fn build_launch_env_without_gptk_omits_d3dmetal_vars() {
        let wine_binary = PathBuf::from("/tmp/wine64");
        let prefix_path = PathBuf::from("/tmp/prefix");
        let env = build_launch_env(&wine_binary, &prefix_path, None, None);
        assert!(env.get("DYLD_FALLBACK_LIBRARY_PATH").is_none());
        assert!(env.get("WINEESYNC").is_none());
    }
}
