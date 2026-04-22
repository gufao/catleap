use std::path::{Path, PathBuf};
use std::process::Command;

/// Locate a Wine binary, checking bundled and system locations in order.
pub fn find_wine_binary(data_path: &Path) -> Result<PathBuf, String> {
    // 1. Bundled wine
    let bundled = data_path.join("wine").join("bin").join("wine64");
    if bundled.exists() {
        return Ok(bundled);
    }

    // 2. Homebrew
    let homebrew = PathBuf::from("/opt/homebrew/bin/wine64");
    if homebrew.exists() {
        return Ok(homebrew);
    }

    // 3. CrossOver
    let crossover = PathBuf::from(
        "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64",
    );
    if crossover.exists() {
        return Ok(crossover);
    }

    // 4. `which wine64`
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

    Err("No Wine binary found. Install Wine via Homebrew (`brew install --cask wine-stable`) or CrossOver.".to_string())
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
}
