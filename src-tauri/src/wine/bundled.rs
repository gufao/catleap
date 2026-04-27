use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Info about the detected Wine/GPTK installation.
#[derive(Debug, Clone, Serialize)]
pub struct WineStatus {
    pub installed: bool,
    pub variant: String,
    pub path: String,
    pub gptk_libs_installed: bool,
    pub installed_version: Option<String>,
    pub expected_version: String,
}

/// Path to the imported D3DMetal libraries, if present.
pub fn gptk_lib_path(data_path: &Path) -> Option<PathBuf> {
    let lib = data_path.join("gptk/lib");
    lib.join("D3DMetal.framework").exists().then_some(lib)
}

/// Check which Wine variant is available on the system.
pub fn check_wine_status(data_path: &Path, installed_version: Option<String>) -> WineStatus {
    let gptk_present = gptk_lib_path(data_path).is_some();
    match find_wine_binary(data_path) {
        Ok(path) => WineStatus {
            installed: true,
            variant: detect_variant(&path, data_path, gptk_present),
            path: path.to_string_lossy().to_string(),
            gptk_libs_installed: gptk_present,
            installed_version: installed_version.clone(),
            expected_version: crate::wine::installer::WINE_EXPECTED_VERSION.to_string(),
        },
        Err(_) => WineStatus {
            installed: false,
            variant: "none".to_string(),
            path: String::new(),
            gptk_libs_installed: gptk_present,
            installed_version,
            expected_version: crate::wine::installer::WINE_EXPECTED_VERSION.to_string(),
        },
    }
}

/// Locate a Wine binary in priority order.
/// 1. Bundled (`<data_path>/wine/bin/wine64`)
/// 2. CrossOver.app
/// 3. wine64 in PATH (last resort)
pub fn find_wine_binary(data_path: &Path) -> Result<PathBuf, String> {
    let bundled = data_path.join("wine/bin/wine64");
    if bundled.exists() {
        return Ok(bundled);
    }

    let crossover = PathBuf::from(
        "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64",
    );
    if crossover.exists() {
        return Ok(crossover);
    }

    if let Ok(output) = Command::new("which").arg("wine64").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                let path = PathBuf::from(&path_str);
                if path.exists() {
                    return Ok(path);
                }
            }
        }
    }

    Err("Wine not found. Catleap will download it during onboarding.".to_string())
}

pub(crate) fn detect_variant(path: &Path, data_path: &Path, gptk_present: bool) -> String {
    let bundled_root = data_path.join("wine");
    if path.starts_with(&bundled_root) {
        return if gptk_present { "catleap-gptk" } else { "catleap-wine" }.to_string();
    }
    let s = path.to_string_lossy();
    if s.contains("CrossOver.app") {
        return "crossover".to_string();
    }
    "wine".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_wine_at(root: &std::path::Path) -> std::path::PathBuf {
        let bin_dir = root.join("wine").join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let wine = bin_dir.join("wine64");
        std::fs::write(&wine, b"").unwrap();
        wine
    }

    #[test]
    fn finds_bundled_wine_first() {
        let tmp = TempDir::new().unwrap();
        let wine = make_wine_at(tmp.path());
        let found = find_wine_binary(tmp.path()).unwrap();
        assert_eq!(found, wine);
    }

    #[test]
    fn missing_wine_returns_clear_error() {
        let tmp = TempDir::new().unwrap();
        let err = find_wine_binary(tmp.path()).unwrap_err();
        assert!(err.contains("Wine not found"), "got: {err}");
    }

    #[test]
    fn check_wine_status_detects_gptk_libs_present() {
        let tmp = TempDir::new().unwrap();
        make_wine_at(tmp.path());
        let fw = tmp.path().join("gptk/lib/D3DMetal.framework");
        std::fs::create_dir_all(&fw).unwrap();
        let status = check_wine_status(tmp.path(), None);
        assert!(status.installed);
        assert_eq!(status.variant, "catleap-gptk");
        assert!(status.gptk_libs_installed);
    }

    #[test]
    fn check_wine_status_without_gptk_libs() {
        let tmp = TempDir::new().unwrap();
        make_wine_at(tmp.path());
        let status = check_wine_status(tmp.path(), None);
        assert!(status.installed);
        assert_eq!(status.variant, "catleap-wine");
        assert!(!status.gptk_libs_installed);
    }

    #[test]
    fn check_wine_status_uninstalled() {
        let tmp = TempDir::new().unwrap();
        let status = check_wine_status(tmp.path(), None);
        assert!(!status.installed);
        assert_eq!(status.variant, "none");
        assert!(!status.gptk_libs_installed);
    }

    #[test]
    fn variant_for_crossover_path() {
        let p = std::path::Path::new(
            "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64",
        );
        let tmp = TempDir::new().unwrap();
        // detect_variant takes data_path so it can decide bundled vs other
        assert_eq!(detect_variant(p, tmp.path(), false), "crossover");
    }
}
