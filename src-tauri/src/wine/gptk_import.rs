use notify::{EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct GptkInfo {
    pub volume: PathBuf,
    pub lib_path: PathBuf,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GptkPhase {
    Waiting,
    Found { version: String },
    Copying,
    Done { version: String },
    Failed { error: String },
}

/// Look inside a mounted volume for the Apple GPTK D3DMetal libs.
/// Returns `None` if the volume doesn't have the expected layout.
///
/// Apple ships GPTK 3 with the framework under
/// `redist/lib/external/D3DMetal.framework`. We copy the whole `redist/lib/`
/// tree (`external/` + `wine/`) into the user's data dir.
pub fn detect_gptk_in_volume(volume: &Path) -> Option<GptkInfo> {
    let lib = volume.join("redist/lib");
    let framework = lib.join("external/D3DMetal.framework");
    if !framework.exists() {
        return None;
    }
    Some(GptkInfo {
        volume: volume.to_path_buf(),
        lib_path: lib,
        version: parse_volume_version(volume).unwrap_or_else(|| "unknown".into()),
    })
}

/// Extract a version string from the volume directory name.
/// Handles both Apple naming conventions:
/// - "Game Porting Toolkit-3.0"
/// - "Evaluation environment for Windows games 2.1"
pub fn parse_volume_version(volume: &Path) -> Option<String> {
    let name = volume.file_name()?.to_string_lossy();
    let rest = if let Some(r) = name.strip_prefix("Game Porting Toolkit-") {
        r
    } else if let Some(r) = name.strip_prefix("Evaluation environment for Windows games ") {
        r
    } else {
        return None;
    };
    // Trim any trailing suffix after a space (e.g. "3.0 beta" → "3.0").
    let token = rest.split_whitespace().next().unwrap_or(rest);
    Some(token.to_string())
}

/// Scan `/Volumes` (or a substitute root) and return all GPTK volumes found.
pub fn scan_volumes(volumes_root: &Path) -> Vec<GptkInfo> {
    let entries = match std::fs::read_dir(volumes_root) {
        Ok(it) => it,
        Err(_) => return vec![],
    };
    let mut out = vec![];
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            if let Some(info) = detect_gptk_in_volume(&entry.path()) {
                out.push(info);
            }
        }
    }
    out
}

/// Pick the highest-versioned GPTK from a set. Unknown versions rank last.
pub fn pick_best(infos: &[GptkInfo]) -> Option<GptkInfo> {
    infos
        .iter()
        .max_by(|a, b| version_rank(&a.version).cmp(&version_rank(&b.version)))
        .cloned()
}

fn version_rank(v: &str) -> (u8, u32, u32, u32) {
    if v == "unknown" {
        return (0, 0, 0, 0);
    }
    let mut parts = v.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (1, major, minor, patch)
}

/// Copy `<volume>/redist/lib/` into `<data_path>/gptk/lib/` using `ditto`
/// to preserve framework bundle resource forks and symlinks.
pub fn copy_libs(info: &GptkInfo, data_path: &Path) -> Result<(), String> {
    let dst = data_path.join("gptk/lib");
    let staging = data_path.join("gptk/lib.partial");

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }

    // Always start staging from a clean state. The previous live install at `dst`
    // is left untouched until the new copy passes validation.
    let _ = std::fs::remove_dir_all(&staging);

    let status = Command::new("/usr/bin/ditto")
        .arg("-V")
        .arg(&info.lib_path)
        .arg(&staging)
        .status()
        .map_err(|e| format!("ditto: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("ditto exit {}", status.code().unwrap_or(-1)));
    }

    let probe = staging.join("external/D3DMetal.framework/Versions/A/D3DMetal");
    if !probe.exists() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("post-copy validation failed: {} missing", probe.display()));
    }

    // Promote: rename old install aside, move staging into place, drop the backup.
    if dst.exists() {
        let backup = dst.with_extension("old");
        let _ = std::fs::remove_dir_all(&backup);
        std::fs::rename(&dst, &backup)
            .map_err(|e| format!("backup {}: {e}", dst.display()))?;
    }
    std::fs::rename(&staging, &dst)
        .map_err(|e| format!("promote: {e}"))?;
    let _ = std::fs::remove_dir_all(dst.with_extension("old"));

    Ok(())
}

const VOLUMES_ROOT: &str = "/Volumes";

/// Block-watch `/Volumes` for new GPTK volumes. Calls `on_found` once a
/// volume with the right layout appears (or is already present at startup).
/// Returns when `running` becomes false or after `on_found` is invoked.
pub fn watch_for_gptk(
    running: Arc<AtomicBool>,
    mut on_found: impl FnMut(GptkInfo),
) -> Result<(), String> {
    if let Some(info) = pick_best(&scan_volumes(Path::new(VOLUMES_ROOT))) {
        on_found(info);
        return Ok(());
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher =
        notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| format!("watcher: {e}"))?;
    watcher
        .watch(Path::new(VOLUMES_ROOT), RecursiveMode::NonRecursive)
        .map_err(|e| format!("watch /Volumes: {e}"))?;

    while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(std::time::Duration::from_millis(500)) {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    if let Some(info) = pick_best(&scan_volumes(Path::new(VOLUMES_ROOT))) {
                        on_found(info);
                        return Ok(());
                    }
                }
            }
            Ok(Err(_)) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }
    Ok(())
}

/// Eject a mounted DMG via `hdiutil detach`.
pub fn eject(volume: &Path) -> Result<(), String> {
    let status = Command::new("/usr/bin/hdiutil")
        .arg("detach")
        .arg(volume)
        .status()
        .map_err(|e| format!("hdiutil: {e}"))?;
    if !status.success() {
        return Err(format!("hdiutil exit {}", status.code().unwrap_or(-1)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_gptk_volume(root: &Path, name: &str) -> PathBuf {
        let v = root.join(name);
        let fw = v.join("redist/lib/external/D3DMetal.framework/Versions/A");
        std::fs::create_dir_all(&fw).unwrap();
        std::fs::write(fw.join("D3DMetal"), b"").unwrap();
        v
    }

    #[test]
    fn detect_finds_present_framework() {
        let tmp = TempDir::new().unwrap();
        let v = make_gptk_volume(tmp.path(), "Game Porting Toolkit-3.0");
        let info = detect_gptk_in_volume(&v).unwrap();
        assert_eq!(info.version, "3.0");
    }

    #[test]
    fn detect_returns_none_when_framework_missing() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("Game Porting Toolkit-3.0/redist/lib")).unwrap();
        assert!(detect_gptk_in_volume(&tmp.path().join("Game Porting Toolkit-3.0")).is_none());
    }

    #[test]
    fn parse_handles_both_naming_conventions() {
        assert_eq!(parse_volume_version(Path::new("/Volumes/Game Porting Toolkit-3.0")), Some("3.0".into()));
        assert_eq!(parse_volume_version(Path::new("/Volumes/Evaluation environment for Windows games 2.1")), Some("2.1".into()));
        assert_eq!(parse_volume_version(Path::new("/Volumes/Macintosh HD")), None);
    }

    #[test]
    fn parse_strips_trailing_suffix() {
        assert_eq!(
            parse_volume_version(Path::new("/Volumes/Game Porting Toolkit-3.0 beta")),
            Some("3.0".into())
        );
    }

    #[test]
    fn scan_volumes_finds_only_matching() {
        let tmp = TempDir::new().unwrap();
        make_gptk_volume(tmp.path(), "Game Porting Toolkit-3.0");
        std::fs::create_dir_all(tmp.path().join("Some Other DMG")).unwrap();
        let found = scan_volumes(tmp.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].version, "3.0");
    }

    #[test]
    fn pick_best_prefers_higher_version() {
        let a = GptkInfo { volume: "/v/a".into(), lib_path: "/v/a/redist/lib".into(), version: "2.1".into() };
        let b = GptkInfo { volume: "/v/b".into(), lib_path: "/v/b/redist/lib".into(), version: "3.0".into() };
        let c = GptkInfo { volume: "/v/c".into(), lib_path: "/v/c/redist/lib".into(), version: "unknown".into() };
        let infos = vec![a, b, c];
        let best = pick_best(&infos).unwrap();
        assert_eq!(best.volume, PathBuf::from("/v/b"));
    }

    #[test]
    fn copy_libs_succeeds_when_source_valid() {
        let tmp = TempDir::new().unwrap();
        let volume = make_gptk_volume(tmp.path(), "Game Porting Toolkit-3.0");
        let info = detect_gptk_in_volume(&volume).unwrap();
        let data = TempDir::new().unwrap();
        copy_libs(&info, data.path()).unwrap();
        assert!(data.path().join("gptk/lib/external/D3DMetal.framework/Versions/A/D3DMetal").exists());
    }

    #[test]
    fn copy_libs_validates_post_copy() {
        // Source missing the inner D3DMetal binary — copy should fail.
        let tmp = TempDir::new().unwrap();
        let v = tmp.path().join("Bad-1.0");
        let fw_dir = v.join("redist/lib/external/D3DMetal.framework");
        std::fs::create_dir_all(&fw_dir).unwrap();
        // intentionally no Versions/A/D3DMetal
        let info = GptkInfo { volume: v.clone(), lib_path: v.join("redist/lib"), version: "1.0".into() };
        let data = TempDir::new().unwrap();
        assert!(copy_libs(&info, data.path()).is_err());
    }
}
