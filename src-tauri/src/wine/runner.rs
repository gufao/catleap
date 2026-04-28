// Wine game runner — implementation in Task 11
use crate::models::{CompatDatabase, Game};
use std::path::Path;
use std::process::{Child, Stdio};

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

/// Build the `arch -x86_64 wine64 Steam.exe -applaunch <appid> -silent` command
/// for a SteamWine game. Public-in-crate for testability; not the spawn call.
pub(crate) fn build_steam_runtime_command(
    game: &crate::models::Game,
    data_path: &std::path::Path,
    compat_db: &crate::models::CompatDatabase,
) -> Result<std::process::Command, String> {
    use crate::compat::database::lookup_game;
    use crate::wine::{bundled, prefix::build_launch_env, steam_runtime, wine_command};

    let appid = game
        .id
        .strip_prefix("steam_wine_")
        .ok_or_else(|| format!("invalid steam_wine id: {}", game.id))?;

    let wine_binary = bundled::find_wine_binary(data_path)?;
    let prefix_path = steam_runtime::runtime_prefix_path(data_path);
    let steam_exe = steam_runtime::steam_exe_path(data_path);

    if !steam_exe.exists() {
        return Err("Steam runtime not installed. Click Install Steam in the sidebar.".into());
    }

    let compat = lookup_game(compat_db, appid);
    let env_map = build_launch_env(
        &wine_binary,
        &prefix_path,
        compat,
        bundled::gptk_lib_path(data_path).as_deref(),
    );

    let mut cmd = wine_command(&wine_binary);
    cmd.arg(&steam_exe);
    cmd.arg("-applaunch");
    cmd.arg(appid);
    cmd.arg("-silent");
    cmd.current_dir(&prefix_path);
    cmd.env_clear();
    for (k, v) in &env_map {
        cmd.env(k, v);
    }
    Ok(cmd)
}

/// Launch a game with Wine. Returns the spawned Child process.
pub fn launch_game(
    game: &Game,
    data_path: &Path,
    compat_db: &CompatDatabase,
) -> Result<Child, String> {
    if matches!(game.source, crate::models::GameSource::SteamWine) {
        let logs_dir = data_path.join("logs");
        std::fs::create_dir_all(&logs_dir)
            .map_err(|e| format!("Failed to create logs dir: {}", e))?;
        let log_path = logs_dir.join(format!("{}.log", game.id));
        let log_file = std::fs::File::create(&log_path)
            .map_err(|e| format!("Failed to create log: {}", e))?;
        let log_dup = log_file.try_clone()
            .map_err(|e| format!("Failed to clone log handle: {}", e))?;

        let mut cmd = build_steam_runtime_command(game, data_path, compat_db)?;
        cmd.stdout(Stdio::from(log_file)).stderr(Stdio::from(log_dup));
        return cmd.spawn().map_err(|e| format!("Failed to spawn Steam.exe -applaunch: {e}"));
    }

    // ── existing logic for Manual / Steam (macOS) games follows unchanged ──
    use crate::compat::database::lookup_game;
    use crate::wine::bundled::find_wine_binary;
    use crate::wine::prefix::{
        build_launch_env, configure_prefix, create_prefix, get_prefix_path, prefix_exists,
    };
    use std::fs;

    // Locate Wine binary
    let wine_binary = find_wine_binary(data_path)?;

    // Determine the executable to launch
    let exe_path = match &game.executable {
        Some(exe) => exe.clone(),
        None => find_main_executable(&game.install_dir)?,
    };

    // Determine prefix path
    let source_str = game.source.as_path_str();
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
    let gptk_lib = crate::wine::bundled::gptk_lib_path(data_path);
    let env_map = build_launch_env(&wine_binary, &prefix_path, compat, gptk_lib.as_deref());

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

    #[test]
    fn launch_via_steam_runtime_builds_correct_command() {
        use crate::models::{Game, GameSource, GameStatus, CompatDatabase};
        use crate::wine::steam_runtime;

        let tmp = TempDir::new().unwrap();
        let wine_root = tmp.path().join("wine");
        std::fs::create_dir_all(wine_root.join("bin")).unwrap();
        std::fs::write(wine_root.join("bin/wine64"), b"").unwrap();
        let steam_exe = steam_runtime::steam_exe_path(tmp.path());
        std::fs::create_dir_all(steam_exe.parent().unwrap()).unwrap();
        std::fs::write(&steam_exe, b"").unwrap();

        let game = Game {
            id: "steam_wine_413150".into(),
            name: "Stardew".into(),
            source: GameSource::SteamWine,
            status: GameStatus::Unknown,
            install_dir: tmp.path().join("game"),
            executable: None,
            size_bytes: None,
            is_running: false,
            notes: None,
        };
        let compat = CompatDatabase { version: "t".into(), games: vec![] };

        let cmd = build_steam_runtime_command(&game, tmp.path(), &compat).unwrap();
        assert_eq!(cmd.get_program(), "/usr/bin/arch");
        let args: Vec<&std::ffi::OsStr> = cmd.get_args().collect();
        assert!(args.iter().any(|a| *a == "-x86_64"));
        assert!(args.iter().any(|a| a.to_string_lossy().contains("Steam.exe")));
        assert!(args.iter().any(|a| *a == "-applaunch"));
        assert!(args.iter().any(|a| *a == "413150"));
        assert!(args.iter().any(|a| *a == "-silent"));
    }
}
