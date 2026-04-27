use crate::commands::games::AppState;
use crate::wine::installer::{self, InstallPhase, WINE_EXPECTED_VERSION};
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager, State, Window};

/// Run the wine install pipeline.
///
/// Emits `wine-install-progress` events for every phase transition,
/// including a final `Failed { error }` event on failure. The Promise
/// returned to the frontend ALSO rejects on failure with the same error
/// string, so callers should handle either the event listener OR the
/// awaited Promise — not both — to avoid double error reporting.
#[tauri::command]
pub async fn start_wine_install(
    window: Window,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (data_path, wine_version_owned) = {
        let s = state.settings.lock().unwrap();
        (s.data_path.clone(), s.wine_version.clone())
    };
    let already = installer::already_installed(&data_path, wine_version_owned.as_deref());
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
        let snapshot = {
            let mut s = state.settings.lock().unwrap();
            s.wine_version = Some(WINE_EXPECTED_VERSION.to_string());
            s.clone()
        };
        if let Err(e) = crate::commands::settings::save_settings_to_disk(&snapshot) {
            log::error!("Failed to persist wine_version after install: {e}");
        }
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

use crate::wine::gptk_import::{self, GptkPhase};
use std::sync::atomic::Ordering as Ord2;

#[tauri::command]
pub fn start_gptk_watch(window: Window, state: State<'_, AppState>) -> Result<(), String> {
    if state.gptk_watching.swap(true, Ord2::Relaxed) {
        return Ok(()); // already watching
    }
    let running = state.gptk_watching.clone();
    let data_path = state.settings.lock().unwrap().data_path.clone();
    let win = window.clone();
    let app = window.app_handle().clone();

    std::thread::spawn(move || {
        let _ = win.emit("gptk-import-progress", GptkPhase::Waiting);
        let result = gptk_import::watch_for_gptk(running.clone(), |info| {
            let _ = win.emit(
                "gptk-import-progress",
                GptkPhase::Found {
                    version: info.version.clone(),
                },
            );
            let _ = win.emit(
                "gptk-import-progress",
                GptkPhase::Copying { percent: 0 },
            );
            match gptk_import::copy_libs(&info, &data_path) {
                Ok(_) => {
                    if let Some(state) = app.try_state::<AppState>() {
                        let snapshot = {
                            let mut s = state.settings.lock().unwrap();
                            s.gptk_version = Some(info.version.clone());
                            s.gptk_skipped = false;
                            s.clone()
                        };
                        if let Err(e) = crate::commands::settings::save_settings_to_disk(&snapshot) {
                            log::error!("Failed to persist gptk_version: {e}");
                        }
                    }
                    let _ = win.emit(
                        "gptk-import-progress",
                        GptkPhase::Done { version: info.version },
                    );
                }
                Err(e) => {
                    let _ = win.emit(
                        "gptk-import-progress",
                        GptkPhase::Failed { error: e },
                    );
                }
            }
            running.store(false, Ord2::Relaxed);
        });
        if let Err(e) = result {
            let _ = win.emit(
                "gptk-import-progress",
                GptkPhase::Failed { error: e },
            );
        }
        running.store(false, Ord2::Relaxed);
    });
    Ok(())
}

#[tauri::command]
pub fn stop_gptk_watch(state: State<'_, AppState>) -> Result<(), String> {
    state.gptk_watching.store(false, Ord2::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn skip_gptk(state: State<'_, AppState>) -> Result<(), String> {
    let snapshot = {
        let mut s = state.settings.lock().unwrap();
        s.gptk_skipped = true;
        s.clone()
    };
    crate::commands::settings::save_settings_to_disk(&snapshot)?;
    Ok(())
}

#[tauri::command]
pub fn eject_gptk_volume(volume_path: String) -> Result<(), String> {
    gptk_import::eject(std::path::Path::new(&volume_path))
}
