use crate::commands::games::AppState;
use crate::wine::bundled;
use crate::wine::steam_runtime::{
    self, runtime_prefix_path, steam_exe_path, SteamInstallPhase, STEAM_RUNTIME_ID,
};
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager, State, Window};

#[tauri::command]
pub async fn start_steam_install(
    window: Window,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.steam_installing.swap(true, Ordering::Relaxed) {
        return Ok(()); // already running
    }
    state.steam_install_cancel.store(false, Ordering::Relaxed);

    let data_path = state.settings.lock().unwrap().data_path.clone();
    let wine_binary = match bundled::find_wine_binary(&data_path) {
        Ok(p) => p,
        Err(e) => {
            state.steam_installing.store(false, Ordering::Relaxed);
            return Err(e);
        }
    };

    let cancel = state.steam_install_cancel.clone();
    let win = window.clone();
    let app = window.app_handle().clone();

    let result = steam_runtime::run_install(&data_path, &wine_binary, cancel, move |phase| {
        let _ = win.emit("steam-install-progress", phase);
    })
    .await;

    if result.is_ok() {
        if let Some(state) = app.try_state::<AppState>() {
            let snapshot = match state.settings.lock() {
                Ok(mut s) => {
                    s.steam_runtime_installed = true;
                    Some(s.clone())
                }
                Err(_) => None,
            };
            if let Some(snap) = snapshot {
                if let Err(e) = crate::commands::settings::save_settings_to_disk(&snap) {
                    log::error!("Failed to persist steam_runtime_installed: {e}");
                }
            }
        }
        let _ = window.emit("steam-install-progress", SteamInstallPhase::Done);
    } else if let Err(e) = &result {
        let _ = window.emit(
            "steam-install-progress",
            SteamInstallPhase::Failed { error: e.clone() },
        );
    }

    state.steam_installing.store(false, Ordering::Relaxed);
    result
}

#[tauri::command]
pub fn cancel_steam_install(state: State<'_, AppState>) -> Result<(), String> {
    state.steam_install_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn launch_steam_runtime(state: State<'_, AppState>) -> Result<(), String> {
    use crate::wine::wine_command;
    use std::process::Stdio;

    let data_path = state.settings.lock().unwrap().data_path.clone();
    let wine_binary = bundled::find_wine_binary(&data_path)?;
    let prefix = runtime_prefix_path(&data_path);
    let steam_exe = steam_exe_path(&data_path);
    if !steam_exe.exists() {
        return Err("Steam runtime not installed".into());
    }

    let logs_dir = data_path.join("logs");
    std::fs::create_dir_all(&logs_dir).ok();
    let log = std::fs::File::create(logs_dir.join("_steam_runtime.log"))
        .map_err(|e| format!("create log: {e}"))?;
    let log_dup = log.try_clone().map_err(|e| format!("dup log: {e}"))?;

    let env_map = crate::wine::prefix::build_launch_env(
        &wine_binary,
        &prefix,
        None,
        bundled::gptk_lib_path(&data_path).as_deref(),
    );

    let mut cmd = wine_command(&wine_binary);
    // -no-cef-sandbox: Chromium sandbox doesn't work cleanly through Wine on
    // macOS — without this Steam often shows a "SteamWebHelper not responding"
    // dialog on first launch.
    //
    // Do NOT pass -silent: that flag makes Steam start minimised to the
    // system tray (Dock icon on macOS) with no visible main window — users
    // think the click did nothing.
    cmd.arg(&steam_exe).arg("-no-cef-sandbox");
    cmd.current_dir(&prefix);
    cmd.stdout(Stdio::from(log)).stderr(Stdio::from(log_dup));
    cmd.env_clear();
    for (k, v) in &env_map {
        cmd.env(k, v);
    }

    let child = cmd.spawn().map_err(|e| format!("spawn Steam.exe: {e}"))?;
    state.process_monitor.track(STEAM_RUNTIME_ID.into(), child);
    Ok(())
}

#[tauri::command]
pub fn stop_steam_runtime(state: State<'_, AppState>) -> Result<(), String> {
    let _ = state.process_monitor.stop(STEAM_RUNTIME_ID);
    Ok(())
}

#[tauri::command]
pub fn reset_steam_runtime(state: State<'_, AppState>) -> Result<(), String> {
    // 1. Stop Steam.exe if running.
    let _ = state.process_monitor.stop(STEAM_RUNTIME_ID);
    // Defensive: pkill any wine processes inside the prefix.
    let data_path = state.settings.lock().unwrap().data_path.clone();
    let prefix = runtime_prefix_path(&data_path);
    let _ = std::process::Command::new("/usr/bin/pkill")
        .arg("-f")
        .arg(prefix.to_string_lossy().as_ref())
        .status();

    // 2. Wipe the prefix.
    if prefix.exists() {
        std::fs::remove_dir_all(&prefix)
            .map_err(|e| format!("remove prefix: {e}"))?;
    }

    // 3. Persist the flag.
    let snapshot = {
        let mut s = state.settings.lock().unwrap();
        s.steam_runtime_installed = false;
        s.clone()
    };
    crate::commands::settings::save_settings_to_disk(&snapshot)?;
    Ok(())
}
