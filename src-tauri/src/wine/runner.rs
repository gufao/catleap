// Wine game runner — implementation in Task 11
use crate::models::{CompatDatabase, Game};
use std::path::Path;
use std::process::Child;

/// Find the most likely main executable in a directory.
/// Returns the .exe with the shortest filename (stem).
pub fn find_main_executable(install_dir: &Path) -> Result<std::path::PathBuf, String> {
    let mut exes: Vec<std::path::PathBuf> = Vec::new();

    for entry in std::fs::read_dir(install_dir)
        .map_err(|e| format!("Failed to read directory {}: {}", install_dir.display(), e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("exe") {
                    exes.push(path);
                }
            }
        }
    }

    if exes.is_empty() {
        return Err(format!(
            "No .exe files found in {}",
            install_dir.display()
        ));
    }

    // Sort by filename length (shortest first), then alphabetically for stability
    exes.sort_by(|a, b| {
        let a_name = a.file_name().unwrap_or_default().to_string_lossy().len();
        let b_name = b.file_name().unwrap_or_default().to_string_lossy().len();
        a_name.cmp(&b_name).then_with(|| a.cmp(b))
    });

    Ok(exes.into_iter().next().unwrap())
}

/// Launch a game with Wine. Returns the spawned Child process.
pub fn launch_game(
    game: &Game,
    data_path: &Path,
    compat_db: &CompatDatabase,
) -> Result<Child, String> {
    use crate::compat::database::lookup_game;
    use crate::wine::bundled::find_wine_binary;
    use crate::wine::prefix::{
        build_launch_env, configure_prefix, create_prefix, get_prefix_path, prefix_exists,
    };
    use std::fs;
    use std::process::Stdio;

    // Locate Wine binary
    let wine_binary = find_wine_binary(data_path)?;

    // Determine the executable to launch
    let exe_path = match &game.executable {
        Some(exe) => exe.clone(),
        None => find_main_executable(&game.install_dir)?,
    };

    // Determine prefix path
    let source_str = format!("{:?}", game.source).to_lowercase();
    let prefix_path = get_prefix_path(data_path, &game.id, &source_str);

    // Lookup compat entry (game id format: "steam_<appid>")
    let compat = game
        .id
        .strip_prefix("steam_")
        .and_then(|appid| lookup_game(compat_db, appid));

    // Create prefix if it doesn't exist
    if !prefix_exists(&prefix_path) {
        create_prefix(&wine_binary, &prefix_path)?;
        if let Some(entry) = compat {
            configure_prefix(&wine_binary, &prefix_path, entry)?;
        }
    }

    // Build environment
    let env_map = build_launch_env(&wine_binary, &prefix_path, compat);

    // Set up log file
    let logs_dir = data_path.join("logs");
    fs::create_dir_all(&logs_dir)
        .map_err(|e| format!("Failed to create logs directory: {}", e))?;

    let log_file_path = logs_dir.join(format!("{}_{}.log", source_str, game.id));
    let log_file = fs::File::create(&log_file_path)
        .map_err(|e| format!("Failed to create log file {}: {}", log_file_path.display(), e))?;
    let log_file_stderr = log_file
        .try_clone()
        .map_err(|e| format!("Failed to clone log file handle: {}", e))?;

    // Spawn Wine process under arch -x86_64
    let mut cmd = crate::wine::wine_command(&wine_binary);
    cmd.arg(&exe_path);

    if let Some(entry) = compat {
        for arg in &entry.launch_args {
            cmd.arg(arg);
        }
    }

    cmd.current_dir(&game.install_dir)
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_file_stderr));

    cmd.env_clear();
    for (k, v) in &env_map {
        cmd.env(k, v);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn Wine process: {}", e))?;

    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_find_main_executable() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("launcher.exe"), b"").unwrap();
        std::fs::write(tmp.path().join("game.exe"), b"").unwrap();
        std::fs::write(tmp.path().join("readme.txt"), b"").unwrap();

        let found = find_main_executable(tmp.path()).unwrap();
        // "game.exe" (8 chars) < "launcher.exe" (12 chars)
        assert_eq!(found.file_name().unwrap().to_string_lossy(), "game.exe");
    }

    #[test]
    fn test_find_main_executable_no_exe() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("readme.txt"), b"").unwrap();

        let result = find_main_executable(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No .exe files found"));
    }
}
