use crate::commands::games::AppState;
use crate::wine::installer::{self, InstallPhase, WINE_EXPECTED_VERSION};
use std::sync::atomic::Ordering;
use tauri::{Emitter, State, Window};

#[tauri::command]
pub async fn start_wine_install(
    window: Window,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let data_path = state.settings.lock().unwrap().data_path.clone();
    let already = installer::already_installed(
        &data_path,
        state.settings.lock().unwrap().wine_version.as_deref(),
    );
    if already {
        let _ = window.emit("wine-install-progress", InstallPhase::Done);
        return Ok(());
    }

    state.install_cancel.store(false, Ordering::Relaxed);
    let cancel = state.install_cancel.clone();

    let win = window.clone();
    let result = installer::run_install(&data_path, cancel, move |phase| {
        let _ = win.emit("wine-install-progress", phase);
    })
    .await;

    if result.is_ok() {
        let mut s = state.settings.lock().unwrap();
        s.wine_version = Some(WINE_EXPECTED_VERSION.to_string());
        let cfg_dir = s.data_path.join("config");
        let _ = std::fs::create_dir_all(&cfg_dir);
        let _ = std::fs::write(
            cfg_dir.join("settings.json"),
            serde_json::to_string_pretty(&*s).unwrap_or_default(),
        );
    } else if let Err(e) = &result {
        let _ = window.emit(
            "wine-install-progress",
            InstallPhase::Failed { error: e.clone() },
        );
    }
    result
}

#[tauri::command]
pub fn cancel_wine_install(state: State<'_, AppState>) -> Result<(), String> {
    state.install_cancel.store(true, Ordering::Relaxed);
    Ok(())
}
