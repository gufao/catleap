use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Info about the detected Wine/GPTK installation
#[derive(Debug, Clone, Serialize)]
pub struct WineStatus {
    pub installed: bool,
    pub variant: String,
    pub path: String,
    pub homebrew_available: bool,
}

/// Check which Wine/GPTK variant is available on the system
pub fn check_wine_status(data_path: &Path) -> WineStatus {
    match find_wine_binary(data_path) {
        Ok(path) => {
            let variant = detect_variant(&path);
            WineStatus {
                installed: true,
                variant,
                path: path.to_string_lossy().to_string(),
                homebrew_available: is_homebrew_available(),
            }
        }
        Err(_) => WineStatus {
            installed: false,
            variant: "none".to_string(),
            path: String::new(),
            homebrew_available: is_homebrew_available(),
        },
    }
}

/// Locate a Wine binary, checking locations in priority order.
/// Priority: Bundled → GPTK (Homebrew) → wine-crossover → CrossOver.app → PATH
pub fn find_wine_binary(data_path: &Path) -> Result<PathBuf, String> {
    // 1. Bundled wine (inside Catleap data dir)
    let bundled = data_path.join("wine/bin/wine64");
    if bundled.exists() {
        return Ok(bundled);
    }

    // 2. Apple GPTK via official Homebrew tap (apple/apple)
    let gptk_official = PathBuf::from("/opt/homebrew/opt/game-porting-toolkit/bin/wine64");
    if gptk_official.exists() {
        return Ok(gptk_official);
    }

    // 3. Apple GPTK alternate Cellar path
    let gptk_cellar = PathBuf::from("/opt/homebrew/Cellar/game-porting-toolkit");
    if gptk_cellar.exists() {
        // Find the wine64 binary inside the versioned directory
        if let Ok(entries) = std::fs::read_dir(&gptk_cellar) {
            for entry in entries.flatten() {
                let wine = entry.path().join("bin/wine64");
                if wine.exists() {
                    return Ok(wine);
                }
            }
        }
    }

    // 4. Generic Homebrew wine64
    let homebrew = PathBuf::from("/opt/homebrew/bin/wine64");
    if homebrew.exists() {
        return Ok(homebrew);
    }

    // 5. CrossOver.app (commercial)
    let crossover_app = PathBuf::from(
        "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64",
    );
    if crossover_app.exists() {
        return Ok(crossover_app);
    }

    // 6. Any wine64 in PATH
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

    Err("No Wine/GPTK found. Install via: brew tap apple/apple http://github.com/apple/homebrew-apple && brew install apple/apple/game-porting-toolkit".to_string())
}

fn detect_variant(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    if path_str.contains("game-porting-toolkit") {
        "gptk".to_string()
    } else if path_str.contains("CrossOver.app") {
        "crossover".to_string()
    } else if path_str.contains("/opt/homebrew") {
        "homebrew-wine".to_string()
    } else {
        "wine".to_string()
    }
}

fn is_homebrew_available() -> bool {
    Command::new("/opt/homebrew/bin/brew")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_find_wine_binary_bundled() {
        let tmp = TempDir::new().unwrap();
        let bin_dir = tmp.path().join("wine").join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let wine_path = bin_dir.join("wine64");
        std::fs::write(&wine_path, b"").unwrap();

        let found = find_wine_binary(tmp.path()).unwrap();
        assert_eq!(found, wine_path);
    }

    #[test]
    fn test_detect_variant_gptk() {
        let path = PathBuf::from("/opt/homebrew/opt/game-porting-toolkit/bin/wine64");
        assert_eq!(detect_variant(&path), "gptk");
    }

    #[test]
    fn test_detect_variant_crossover() {
        let path = PathBuf::from("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64");
        assert_eq!(detect_variant(&path), "crossover");
    }

    #[test]
    fn test_check_wine_status_no_wine() {
        let tmp = TempDir::new().unwrap();
        let status = check_wine_status(tmp.path());
        // May or may not find system wine, but should not panic
        let _ = status;
    }
}
