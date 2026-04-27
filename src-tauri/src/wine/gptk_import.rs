use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    Copying { percent: u8 },
    Done { version: String },
    Failed { error: String },
}

/// Look inside a mounted volume for the Apple GPTK D3DMetal libs.
/// Returns `None` if the volume doesn't have the expected layout.
pub fn detect_gptk_in_volume(volume: &Path) -> Option<GptkInfo> {
    let lib = volume.join("redist/lib");
    let framework = lib.join("D3DMetal.framework");
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
    if let Some(rest) = name.strip_prefix("Game Porting Toolkit-") {
        return Some(rest.to_string());
    }
    if let Some(rest) = name.strip_prefix("Evaluation environment for Windows games ") {
        return Some(rest.to_string());
    }
    None
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
pub fn pick_best(infos: Vec<GptkInfo>) -> Option<GptkInfo> {
    let mut sorted = infos;
    sorted.sort_by(|a, b| version_rank(&b.version).cmp(&version_rank(&a.version)));
    sorted.into_iter().next()
}

fn version_rank(v: &str) -> (u8, u32, u32) {
    if v == "unknown" {
        return (0, 0, 0);
    }
    let mut parts = v.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (1, major, minor)
}

/// Copy `<volume>/redist/lib/` into `<data_path>/gptk/lib/` using `ditto`
/// to preserve framework bundle resource forks and symlinks.
pub fn copy_libs(info: &GptkInfo, data_path: &Path) -> Result<(), String> {
    let dst = data_path.join("gptk/lib");
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let _ = std::fs::remove_dir_all(&dst);

    let status = Command::new("/usr/bin/ditto")
        .arg("-V")
        .arg(&info.lib_path)
        .arg(&dst)
        .status()
        .map_err(|e| format!("ditto: {e}"))?;
    if !status.success() {
        return Err(format!("ditto exit {}", status.code().unwrap_or(-1)));
    }

    let probe = dst.join("D3DMetal.framework/Versions/A/D3DMetal");
    if !probe.exists() {
        let _ = std::fs::remove_dir_all(&dst);
        return Err(format!("post-copy validation failed: {} missing", probe.display()));
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
        let fw = v.join("redist/lib/D3DMetal.framework/Versions/A");
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
        let best = pick_best(vec![a, b.clone(), c]).unwrap();
        assert_eq!(best.volume, b.volume);
    }

    #[test]
    fn copy_libs_succeeds_when_source_valid() {
        let tmp = TempDir::new().unwrap();
        let volume = make_gptk_volume(tmp.path(), "Game Porting Toolkit-3.0");
        let info = detect_gptk_in_volume(&volume).unwrap();
        let data = TempDir::new().unwrap();
        copy_libs(&info, data.path()).unwrap();
        assert!(data.path().join("gptk/lib/D3DMetal.framework/Versions/A/D3DMetal").exists());
    }

    #[test]
    fn copy_libs_validates_post_copy() {
        // Source missing the inner D3DMetal binary — copy should fail.
        let tmp = TempDir::new().unwrap();
        let v = tmp.path().join("Bad-1.0");
        let fw_dir = v.join("redist/lib/D3DMetal.framework");
        std::fs::create_dir_all(&fw_dir).unwrap();
        // intentionally no Versions/A/D3DMetal
        let info = GptkInfo { volume: v.clone(), lib_path: v.join("redist/lib"), version: "1.0".into() };
        let data = TempDir::new().unwrap();
        assert!(copy_libs(&info, data.path()).is_err());
    }
}
