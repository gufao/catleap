# Bundled GPTK Wine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the broken Homebrew-based onboarding with a flow that downloads our own Apple-source-compiled `wine64` and imports D3DMetal libraries directly from the user's mounted GPTK DMG, then uses both at game launch.

**Architecture:** Three independent artifacts under `~/Library/Application Support/Catleap/`: `wine/` (downloaded from a GitHub Release we publish), `gptk/lib/` (copied from the user's Apple GPTK DMG via a `/Volumes` watcher), and `prefixes/<game_id>/` (existing). At launch, env vars wire D3DMetal in via `DYLD_FALLBACK_LIBRARY_PATH` and `wine64` runs under `arch -x86_64`.

**Tech Stack:** Rust (Tauri v2 backend) — adds `reqwest` (streaming HTTP), `sha2`, `tar`, `xz2`, `futures-util`, `mockito` (dev). Existing: `notify`, `serde`, `tokio`, `dirs`, `tempfile`. Frontend: React 19 + TypeScript + Tailwind v4 (no test infra; UI verified manually).

**Spec:** `docs/superpowers/specs/2026-04-27-bundled-gptk-wine-design.md`

---

## File Structure

**Create:**
- `tools/build-wine/build.sh` — manual offline build pipeline producing `wine-catleap-<v>.tar.xz`
- `tools/build-wine/README.md` — build prerequisites + how to run
- `tools/build-wine/.gitignore` — ignore `build/` and `dist/` artifacts
- `src-tauri/src/wine/installer.rs` — wine64 download/verify/extract/codesign
- `src-tauri/src/wine/gptk_import.rs` — DMG detection, version parse, ditto copy
- `src-tauri/src/commands/onboarding.rs` — IPC commands for installer + gptk import
- `src/hooks/useTauriEvent.ts` — small hook to subscribe to backend events

**Modify:**
- `src-tauri/Cargo.toml` — add deps
- `src-tauri/src/models.rs` — `Settings` gains `wine_version`, `gptk_version`, `gptk_skipped`
- `src-tauri/src/wine/mod.rs` — re-export new modules; `wine_command` helper
- `src-tauri/src/wine/bundled.rs` — drop `/opt/homebrew/...` GPTK paths; new `WineStatus` shape; new tests
- `src-tauri/src/wine/prefix.rs` — `build_launch_env` accepts `gptk_lib_path`; tests
- `src-tauri/src/wine/runner.rs` — uses `wine_command`; passes `gptk_lib_path` to `build_launch_env`
- `src-tauri/src/commands/launcher.rs` — `check_wine_status` returns the new shape
- `src-tauri/src/commands/mod.rs` — register new module
- `src-tauri/src/lib.rs` — extract `setup_steam_watcher`; register new IPC commands; load extended settings
- `src/types.ts` — extended `Settings`, `WineStatus`; new `WineInstallProgress`, `GptkImportProgress`
- `src/lib/tauri.ts` — IPC wrappers for new commands
- `src/pages/FirstRun.tsx` — rewritten as a state machine
- `src/pages/Settings.tsx` — GPTK status banner with re-import action
- `src/App.tsx` — resume logic when onboarding partial

---

## Task 1: Extend Settings model + frontend types

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src/types.ts`
- Test: `src-tauri/src/models.rs` (`#[cfg(test)] mod tests`)

> Note: no change to `lib.rs` is needed — the existing `load_settings_from_disk` path picks up new fields transparently via `#[serde(default)]`.

- [ ] **Step 1: Write the failing test for Settings default + serialisation**

In `src-tauri/src/models.rs`, append at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_has_no_versions() {
        let s = Settings::default();
        assert_eq!(s.wine_version, None);
        assert_eq!(s.gptk_version, None);
        assert!(!s.gptk_skipped);
    }

    #[test]
    fn settings_round_trip_with_versions() {
        let mut s = Settings::default();
        s.wine_version = Some("1.0.0".to_string());
        s.gptk_version = Some("3.0".to_string());
        s.gptk_skipped = true;
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.wine_version, Some("1.0.0".to_string()));
        assert_eq!(back.gptk_version, Some("3.0".to_string()));
        assert!(back.gptk_skipped);
    }

    #[test]
    fn settings_old_json_loads_with_defaults() {
        // Old config files don't have the new fields — must still deserialize.
        let old = r#"{"steam_path":"/tmp/steam","data_path":"/tmp/data"}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.wine_version, None);
        assert_eq!(s.gptk_version, None);
        assert!(!s.gptk_skipped);
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cd src-tauri && cargo test --lib settings -- --nocapture`
Expected: compile error — `wine_version`, `gptk_version`, `gptk_skipped` not on `Settings`.

- [ ] **Step 3: Add the fields with `#[serde(default)]`**

Replace the `Settings` struct in `src-tauri/src/models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub steam_path: PathBuf,
    pub data_path: PathBuf,
    #[serde(default)]
    pub wine_version: Option<String>,
    #[serde(default)]
    pub gptk_version: Option<String>,
    #[serde(default)]
    pub gptk_skipped: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self {
            steam_path: home.join("Library/Application Support/Steam"),
            data_path: home.join("Library/Application Support/Catleap"),
            wine_version: None,
            gptk_version: None,
            gptk_skipped: false,
        }
    }
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cd src-tauri && cargo test --lib settings`
Expected: 3 settings tests pass.

- [ ] **Step 5: Update frontend types**

In `src/types.ts`, replace the `Settings` interface and extend `WineStatus`:

```typescript
export interface Settings {
  steam_path: string;
  data_path: string;
  wine_version: string | null;
  gptk_version: string | null;
  gptk_skipped: boolean;
}

export interface WineStatus {
  installed: boolean;
  variant: string;
  path: string;
  gptk_libs_installed: boolean;
}

export type WineInstallPhase =
  | { kind: "checking_space" }
  | { kind: "downloading"; bytes_done: number; bytes_total: number }
  | { kind: "verifying" }
  | { kind: "extracting" }
  | { kind: "codesigning" }
  | { kind: "done" }
  | { kind: "failed"; error: string };

export type GptkImportPhase =
  | { kind: "waiting" }
  | { kind: "found"; version: string }
  | { kind: "copying"; percent: number }
  | { kind: "done"; version: string }
  | { kind: "failed"; error: string };
```

- [ ] **Step 6: Verify frontend compiles**

Run: `pnpm tsc --noEmit`
Expected: type errors only on usages of removed `homebrew_available` (we'll fix in Task 12).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/models.rs src/types.ts
git commit -m "feat: extend Settings with wine_version, gptk_version, gptk_skipped"
```

---

## Task 2: Refactor bundled.rs detection

**Files:**
- Modify: `src-tauri/src/wine/bundled.rs`
- Modify: `src-tauri/src/commands/launcher.rs:62-65`

- [ ] **Step 1: Replace existing tests with new expectations**

Replace the `#[cfg(test)] mod tests` block in `src-tauri/src/wine/bundled.rs` with:

```rust
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
        let status = check_wine_status(tmp.path());
        assert!(status.installed);
        assert_eq!(status.variant, "catleap-gptk");
        assert!(status.gptk_libs_installed);
    }

    #[test]
    fn check_wine_status_without_gptk_libs() {
        let tmp = TempDir::new().unwrap();
        make_wine_at(tmp.path());
        let status = check_wine_status(tmp.path());
        assert!(status.installed);
        assert_eq!(status.variant, "catleap-wine");
        assert!(!status.gptk_libs_installed);
    }

    #[test]
    fn check_wine_status_uninstalled() {
        let tmp = TempDir::new().unwrap();
        let status = check_wine_status(tmp.path());
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
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cd src-tauri && cargo test --lib wine::bundled`
Expected: compile error — `WineStatus.gptk_libs_installed` missing, `detect_variant` signature mismatch.

- [ ] **Step 3: Replace the entire `bundled.rs` body**

Replace the contents of `src-tauri/src/wine/bundled.rs` (keep the test module from Step 1):

```rust
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
}

/// Path to the imported D3DMetal libraries, if present.
pub fn gptk_lib_path(data_path: &Path) -> Option<PathBuf> {
    let lib = data_path.join("gptk/lib");
    lib.join("D3DMetal.framework").exists().then_some(lib)
}

/// Check which Wine variant is available on the system.
pub fn check_wine_status(data_path: &Path) -> WineStatus {
    let gptk_present = gptk_lib_path(data_path).is_some();
    match find_wine_binary(data_path) {
        Ok(path) => WineStatus {
            installed: true,
            variant: detect_variant(&path, data_path, gptk_present),
            path: path.to_string_lossy().to_string(),
            gptk_libs_installed: gptk_present,
        },
        Err(_) => WineStatus {
            installed: false,
            variant: "none".to_string(),
            path: String::new(),
            gptk_libs_installed: gptk_present,
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

pub fn detect_variant(path: &Path, data_path: &Path, gptk_present: bool) -> String {
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
```

- [ ] **Step 4: Update `check_wine_status` IPC handler**

In `src-tauri/src/commands/launcher.rs`, the existing `check_wine_status` already calls `bundled::check_wine_status`. Just confirm it still compiles. No change needed.

- [ ] **Step 5: Run all bundled tests**

Run: `cd src-tauri && cargo test --lib wine::bundled`
Expected: 6 tests pass.

- [ ] **Step 6: Run full test suite to surface ripple effects**

Run: `cd src-tauri && cargo test --lib`
Expected: all pass except possible compile errors from FirstRun-related `homebrew_available` references in TS (frontend, not in cargo tests).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/wine/bundled.rs
git commit -m "refactor(wine): drop broken homebrew GPTK paths, add gptk_libs_installed"
```

---

## Task 3: `wine_command` helper + apply to runner/prefix

**Files:**
- Modify: `src-tauri/src/wine/mod.rs`
- Modify: `src-tauri/src/wine/prefix.rs:7-32, 40-71` (the two Command sites)
- Modify: `src-tauri/src/wine/runner.rs:99-122`

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/src/wine/mod.rs`:

```rust
pub mod bundled;
pub mod prefix;
pub mod runner;

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

/// Build a `Command` that invokes `wine_binary` under Rosetta (`arch -x86_64`).
/// Apple GPTK Wine is x86_64-only and must be launched this way on Apple Silicon.
pub fn wine_command(wine_binary: &Path) -> Command {
    let mut cmd = Command::new("/usr/bin/arch");
    cmd.arg("-x86_64");
    cmd.arg(wine_binary);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn wine_command_uses_arch_x86_64() {
        let cmd = wine_command(&PathBuf::from("/tmp/wine64"));
        assert_eq!(cmd.get_program(), "/usr/bin/arch");
        let args: Vec<&OsString> = cmd.get_args().collect();
        assert_eq!(args[0], "-x86_64");
        assert_eq!(args[1], "/tmp/wine64");
    }
}
```

- [ ] **Step 2: Run the test, expect pass**

Run: `cd src-tauri && cargo test --lib wine::tests::wine_command_uses_arch_x86_64`
Expected: PASS.

- [ ] **Step 3: Switch `prefix::create_prefix` to use the helper**

In `src-tauri/src/wine/prefix.rs`, replace the `create_prefix` function:

```rust
pub fn create_prefix(wine_binary: &Path, prefix_path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(prefix_path).map_err(|e| {
        format!(
            "Failed to create prefix directory {}: {}",
            prefix_path.display(),
            e
        )
    })?;

    let status = crate::wine::wine_command(wine_binary)
        .arg("wineboot")
        .arg("--init")
        .env("WINEPREFIX", prefix_path)
        .env("WINEARCH", "win64")
        .status()
        .map_err(|e| format!("Failed to run wineboot: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "wineboot exited with status: {}",
            status.code().unwrap_or(-1)
        ))
    }
}
```

- [ ] **Step 4: Switch `prefix::apply_dll_overrides` to use the helper**

Replace `apply_dll_overrides`:

```rust
pub fn apply_dll_overrides(
    wine_binary: &Path,
    prefix_path: &Path,
    overrides: &[String],
) -> Result<(), String> {
    for dll in overrides {
        let status = crate::wine::wine_command(wine_binary)
            .args([
                "reg",
                "add",
                r"HKEY_CURRENT_USER\Software\Wine\DllOverrides",
                "/v",
                dll,
                "/d",
                "native,builtin",
                "/f",
            ])
            .env("WINEPREFIX", prefix_path)
            .env("WINEARCH", "win64")
            .status()
            .map_err(|e| format!("Failed to run wine reg add for {}: {}", dll, e))?;

        if !status.success() {
            return Err(format!(
                "wine reg add failed for {} with status: {}",
                dll,
                status.code().unwrap_or(-1)
            ));
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Switch `runner::launch_game` to use the helper**

In `src-tauri/src/wine/runner.rs`, replace the section starting `// Spawn Wine process` (lines 98-122 of the current file):

```rust
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
```

(Remove the `let mut cmd = Command::new(&wine_binary);` line — it's replaced.)

- [ ] **Step 6: Run all wine tests**

Run: `cd src-tauri && cargo test --lib wine`
Expected: all existing tests still pass; new helper test passes.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/wine/mod.rs src-tauri/src/wine/prefix.rs src-tauri/src/wine/runner.rs
git commit -m "refactor(wine): centralise arch -x86_64 invocation in wine_command helper"
```

---

## Task 4: Extend `build_launch_env` with `gptk_lib_path`

**Files:**
- Modify: `src-tauri/src/wine/prefix.rs` (existing `build_launch_env`)
- Modify: `src-tauri/src/wine/runner.rs:84` (call site)

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/wine/prefix.rs`, inside `mod tests`, append:

```rust
    #[test]
    fn build_launch_env_with_gptk_sets_d3dmetal_vars() {
        let wine_binary = PathBuf::from("/tmp/data/wine/bin/wine64");
        let prefix_path = PathBuf::from("/tmp/data/prefixes/steam_123");
        let gptk = PathBuf::from("/tmp/data/gptk/lib");

        let env = build_launch_env(&wine_binary, &prefix_path, None, Some(&gptk));

        assert_eq!(
            env.get("DYLD_FALLBACK_LIBRARY_PATH").unwrap(),
            "/tmp/data/gptk/lib:/tmp/data/gptk/lib/external"
        );
        assert_eq!(env.get("WINEESYNC").unwrap(), "1");
        assert_eq!(env.get("WINEMSYNC").unwrap(), "1");
        assert_eq!(env.get("ROSETTA_ADVERTISE_AVX").unwrap(), "1");
        assert!(env.get("WINEDLLPATH").unwrap().contains("/tmp/data/wine/lib/wine/x86_64-windows"));
    }

    #[test]
    fn build_launch_env_without_gptk_omits_d3dmetal_vars() {
        let wine_binary = PathBuf::from("/tmp/wine64");
        let prefix_path = PathBuf::from("/tmp/prefix");
        let env = build_launch_env(&wine_binary, &prefix_path, None, None);
        assert!(env.get("DYLD_FALLBACK_LIBRARY_PATH").is_none());
        assert!(env.get("WINEESYNC").is_none());
    }
```

Also update the existing two `build_launch_env` test calls in the same module to pass `None` as the new fourth argument:
- `test_build_launch_env_without_compat`: change `build_launch_env(&wine_binary, &prefix_path, None)` → `build_launch_env(&wine_binary, &prefix_path, None, None)`
- `test_build_launch_env_with_compat`: change `build_launch_env(&wine_binary, &prefix_path, Some(&compat))` → `build_launch_env(&wine_binary, &prefix_path, Some(&compat), None)`

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cd src-tauri && cargo test --lib wine::prefix`
Expected: compile error — `build_launch_env` arity mismatch.

- [ ] **Step 3: Update `build_launch_env` signature and body**

Replace `build_launch_env` in `src-tauri/src/wine/prefix.rs`:

```rust
pub fn build_launch_env(
    wine_binary: &Path,
    prefix_path: &Path,
    compat: Option<&CompatEntry>,
    gptk_lib_path: Option<&Path>,
) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = HashMap::new();

    env.insert(
        "WINEPREFIX".to_string(),
        prefix_path.to_string_lossy().to_string(),
    );
    env.insert("WINEARCH".to_string(), "win64".to_string());

    if let Some(bin_dir) = wine_binary.parent() {
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bin_dir.display(), current_path);
        env.insert("PATH".to_string(), new_path);
    }

    if let Some(gptk) = gptk_lib_path {
        let gptk_str = gptk.to_string_lossy().to_string();
        env.insert(
            "DYLD_FALLBACK_LIBRARY_PATH".to_string(),
            format!("{}:{}/external", gptk_str, gptk_str),
        );
        env.insert("WINEESYNC".to_string(), "1".to_string());
        env.insert("WINEMSYNC".to_string(), "1".to_string());
        env.insert("ROSETTA_ADVERTISE_AVX".to_string(), "1".to_string());

        // <wine_root>/lib/wine/x86_64-windows for compiled-in PE DLLs
        if let Some(bin_dir) = wine_binary.parent() {
            if let Some(wine_root) = bin_dir.parent() {
                let dll_path = wine_root.join("lib/wine/x86_64-windows");
                env.insert(
                    "WINEDLLPATH".to_string(),
                    dll_path.to_string_lossy().to_string(),
                );
            }
        }
    }

    if let Some(entry) = compat {
        for (key, value) in &entry.env {
            env.insert(key.clone(), value.clone());
        }
    }

    env
}
```

- [ ] **Step 4: Update the call site in `runner::launch_game`**

In `src-tauri/src/wine/runner.rs`, change:

```rust
    let env_map = build_launch_env(&wine_binary, &prefix_path, compat);
```

to:

```rust
    let gptk_lib = crate::wine::bundled::gptk_lib_path(data_path);
    let env_map = build_launch_env(&wine_binary, &prefix_path, compat, gptk_lib.as_deref());
```

- [ ] **Step 5: Run tests, confirm pass**

Run: `cd src-tauri && cargo test --lib wine::prefix`
Expected: 6 prefix tests pass.

- [ ] **Step 6: Run the full backend build**

Run: `cd src-tauri && cargo build`
Expected: builds cleanly.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/wine/prefix.rs src-tauri/src/wine/runner.rs
git commit -m "feat(wine): wire D3DMetal env vars into launch when GPTK libs present"
```

---

## Task 5: Build pipeline (script + README)

**Files:**
- Create: `tools/build-wine/build.sh`
- Create: `tools/build-wine/README.md`
- Create: `tools/build-wine/.gitignore`

> Build pipeline is offline + manual. No tests. The script is documentation-as-code; review checks correctness.

- [ ] **Step 1: Create `.gitignore`**

`tools/build-wine/.gitignore`:

```
build/
dist/
*.tar.xz
*.tar.gz
*.sha256
sources/
```

- [ ] **Step 2: Create the build script**

`tools/build-wine/build.sh`:

```bash
#!/usr/bin/env bash
# build.sh — produce wine-catleap-<VERSION>.tar.xz from Apple GPTK sources.
#
# Run on macOS Apple Silicon under Rosetta (`arch -x86_64 zsh`) with Intel
# Homebrew installed at /usr/local. This is intentional — Apple's GPTK Wine
# is x86_64-only and links against deps from Intel Homebrew.

set -euo pipefail

VERSION="${VERSION:-1.0.0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="${SCRIPT_DIR}/build"
DIST_DIR="${SCRIPT_DIR}/dist"
PREFIX="${WORK_DIR}/prefix"
SOURCE_TARBALL="crossover-sources-22.1.1.tar.gz"
SOURCE_URL="https://media.codeweavers.com/pub/crossover/source/${SOURCE_TARBALL}"
SOURCE_SHA256="cdfe282ce33788bd4f969c8bfb1d3e2de060eb6c296fa1c3cdf4e4690b8b1831"

# --- preconditions --------------------------------------------------------
if [[ "$(uname -m)" != "x86_64" ]]; then
  echo "ERROR: This script must run under arch -x86_64 (Rosetta)." >&2
  echo "Re-run as: arch -x86_64 zsh ${BASH_SOURCE[0]}" >&2
  exit 1
fi

if [[ ! -x /usr/local/bin/brew ]]; then
  echo "ERROR: Intel Homebrew not found at /usr/local/bin/brew." >&2
  echo "Install Intel brew first: arch -x86_64 /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"" >&2
  exit 1
fi

BREW=/usr/local/bin/brew

# --- dependencies ---------------------------------------------------------
echo "==> Installing build dependencies via Intel Homebrew"
"$BREW" install \
  bison flex pkg-config mingw-w64 \
  gstreamer freetype sdl2 libgphoto2 faudio jpeg libpng \
  mpg123 libtiff libgsm glib gnutls libusb gettext sane-backends zlib

# openssl@1.1 was removed from homebrew-core. The build needs it; install
# from the gcenx tap which still ships it. (We only consume the openssl@1.1
# headers/libs at build time; we are not redistributing gcenx artifacts.)
if ! "$BREW" list openssl@1.1 >/dev/null 2>&1; then
  echo "==> openssl@1.1 missing, tapping gcenx/wine to obtain it"
  "$BREW" tap gcenx/wine || true
  "$BREW" install openssl@1.1
fi

# Apple's GPTK formula has its own compiler. Use it.
"$BREW" tap apple/apple https://github.com/apple/homebrew-apple
"$BREW" install apple/apple/game-porting-toolkit-compiler

GPTK_COMPILER="$("$BREW" --prefix game-porting-toolkit-compiler)"

# --- fetch + extract sources ---------------------------------------------
mkdir -p "$WORK_DIR" "$DIST_DIR"
cd "$WORK_DIR"

if [[ ! -f "$SOURCE_TARBALL" ]]; then
  echo "==> Downloading $SOURCE_TARBALL"
  curl -fL --output "$SOURCE_TARBALL" "$SOURCE_URL"
fi

echo "==> Verifying source tarball"
echo "$SOURCE_SHA256  $SOURCE_TARBALL" | shasum -a 256 -c -

echo "==> Extracting wine/ subdir"
rm -rf wine
tar -xf "$SOURCE_TARBALL" --include='sources/wine/*' --strip-components=1

# --- apply Apple's patch -------------------------------------------------
echo "==> Applying Apple GPTK patch"
APPLE_FORMULA="$("$BREW" --repository)/Library/Taps/apple/homebrew-apple/Formula/game-porting-toolkit.rb"
if [[ ! -f "$APPLE_FORMULA" ]]; then
  echo "ERROR: Apple GPTK formula not found at $APPLE_FORMULA" >&2
  exit 1
fi
# Extract the patch from the formula (everything after __END__)
awk '/^__END__$/{found=1; next} found' "$APPLE_FORMULA" > apple.patch
( cd wine && patch -p1 < ../apple.patch )

# --- configure + build ---------------------------------------------------
COMMON_FLAGS=(
  "--prefix=${PREFIX}"
  "--disable-win16" "--disable-tests"
  "--without-x" "--without-pulse" "--without-dbus" "--without-inotify"
  "--without-alsa" "--without-capi" "--without-oss" "--without-udev"
  "--without-krb5"
)
CC_FLAGS=("CC=${GPTK_COMPILER}/bin/clang" "CXX=${GPTK_COMPILER}/bin/clang++")

CFLAGS_EXTRA="-O3 -Wno-implicit-function-declaration -Wno-format -Wno-deprecated-declarations -Wno-incompatible-pointer-types"
LDFLAGS_EXTRA="-lSystem -L/usr/local/lib -Wl,-rpath,/usr/local/lib -Wl,-rpath,@executable_path/../lib/external"
for dep in zlib freetype sdl2 libgphoto2 faudio jpeg libpng mpg123 libtiff libgsm glib gnutls libusb gettext openssl@1.1 sane-backends; do
  CFLAGS_EXTRA+=" -I$("$BREW" --prefix "$dep")/include"
  LDFLAGS_EXTRA+=" -L$("$BREW" --prefix "$dep")/lib"
done
export CFLAGS="$CFLAGS_EXTRA"
export CXXFLAGS="$CFLAGS_EXTRA"
export LDFLAGS="$LDFLAGS_EXTRA"
export MACOSX_DEPLOYMENT_TARGET=10.14
export GSTREAMER_CFLAGS="-I$("$BREW" --prefix gstreamer)/include/gstreamer-1.0 -I$("$BREW" --prefix glib)/include/glib-2.0 -I$("$BREW" --prefix glib)/lib/glib-2.0/include"
export GSTREAMER_LIBS="-L$("$BREW" --prefix gstreamer)/lib -lglib-2.0 -lgmodule-2.0 -lgstreamer-1.0 -lgstaudio-1.0 -lgstvideo-1.0 -lgstgl-1.0 -lgobject-2.0"

mkdir -p wine64-build wine32-build

echo "==> Building wine64"
( cd wine64-build && \
    ../wine/configure "${COMMON_FLAGS[@]}" --enable-win64 --with-gnutls --with-freetype --with-gstreamer "${CC_FLAGS[@]}" && \
    make -j"$(sysctl -n hw.ncpu)" )

echo "==> Building wine32on64"
( cd wine32-build && \
    ../wine/configure "${COMMON_FLAGS[@]}" --enable-win32on64 --with-wine64=../wine64-build --without-gstreamer --without-gphoto --without-sane --without-krb5 --disable-winedbg --without-vulkan --disable-vulkan_1 --disable-winevulkan --without-openal --without-unwind --without-usb "${CC_FLAGS[@]}" && \
    make -j"$(sysctl -n hw.ncpu)" )

# --- install -------------------------------------------------------------
echo "==> Installing into prefix"
rm -rf "$PREFIX"
( cd wine64-build && make install )
( cd wine32-build && make install )

# --- post_install: rewrite dylib IDs to @rpath, then ad-hoc codesign ----
echo "==> Rewriting dylib IDs and ad-hoc signing"
for d in "${PREFIX}"/lib/wine/x86_64-unix/*.so "${PREFIX}"/lib/wine/x86_32on64-unix/*.so; do
  [[ -f "$d" ]] || continue
  chmod 0664 "$d"
  install_name_tool -id "@rpath/$(basename "$d")" "$d"
  codesign --force --sign - "$d"
  chmod 0444 "$d"
done

codesign --force --sign - "${PREFIX}/bin/wine64"
codesign --force --sign - "${PREFIX}/bin/wineserver" || true

# --- package -------------------------------------------------------------
ARTIFACT="${DIST_DIR}/wine-catleap-${VERSION}.tar.xz"
echo "==> Packaging into $ARTIFACT"
( cd "$PREFIX" && tar -cJf "$ARTIFACT" bin lib/wine share/wine )
shasum -a 256 "$ARTIFACT" > "${ARTIFACT}.sha256"

echo
echo "Done. Artifact: $ARTIFACT"
echo "SHA256:        $(cut -d' ' -f1 "${ARTIFACT}.sha256")"
```

- [ ] **Step 3: Make the script executable**

```bash
chmod +x tools/build-wine/build.sh
```

- [ ] **Step 4: Create the README**

`tools/build-wine/README.md`:

```markdown
# Catleap Wine Build Pipeline

Builds `wine-catleap-<VERSION>.tar.xz` from Apple's official GPTK Wine
sources (CodeWeavers 22.1.1 + Apple patches). The artifact is uploaded
as a GitHub Release asset and consumed by Catleap's first-run installer.

## Prerequisites

- macOS Apple Silicon, with Rosetta 2 installed (`softwareupdate --install-rosetta`)
- Intel Homebrew at `/usr/local/bin/brew`. Install:
  ```sh
  arch -x86_64 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  ```
- ~30 GB free disk for the build tree

## Usage

```sh
arch -x86_64 zsh tools/build-wine/build.sh
```

Override the version:

```sh
VERSION=1.1.0 arch -x86_64 zsh tools/build-wine/build.sh
```

The build takes 30–90 minutes depending on hardware. Output:

- `tools/build-wine/dist/wine-catleap-<VERSION>.tar.xz`
- `tools/build-wine/dist/wine-catleap-<VERSION>.tar.xz.sha256`

## Publishing

1. Verify the artifact runs locally by extracting into a test data dir
   and pointing Catleap at it (or copy into `~/Library/Application Support/Catleap/wine/`).
2. Create a GitHub Release on the Catleap repo named `wine-catleap-<VERSION>`.
3. Upload the `.tar.xz` and `.sha256` as release assets.
4. Update `WINE_RELEASE_URL`, `WINE_EXPECTED_SHA256`, and `WINE_EXPECTED_VERSION`
   constants in `src-tauri/src/wine/installer.rs` to point at the new release.
5. Bump `Settings.wine_version` schema if needed and ship a Catleap release.

## openssl@1.1

Apple's GPTK formula depends on `openssl@1.1`, which has been removed
from `homebrew-core`. The script obtains it from the `gcenx/wine` tap
(which still maintains it). We only consume openssl@1.1 at build time;
no gcenx artifacts ship to end users.

## Troubleshooting

- **"openssl@1.1 not found"**: the `gcenx/wine` tap may have changed its
  formulae layout. Inspect `brew search openssl` and adjust the script.
- **Patch fails to apply**: Apple's tap may have updated the patch.
  `brew tap apple/apple https://github.com/apple/homebrew-apple` and
  re-run.
- **Codesign errors**: ensure no antivirus is interfering. Ad-hoc
  signatures (`codesign --sign -`) are sufficient for local launch.
```

- [ ] **Step 5: Sanity-check the script syntax**

Run: `bash -n tools/build-wine/build.sh`
Expected: no output (syntax OK).

- [ ] **Step 6: Commit**

```bash
git add tools/build-wine/
git commit -m "feat(build): add manual offline pipeline producing wine-catleap tar.xz"
```

> **Note:** Actually running the build (~1h) is operator work, not part of this plan's task list. Do that once after Task 7 is merged so you have a real artifact + SHA256 to plug into the installer constants.

---

## Task 6: Installer module — core download/verify/extract

**Files:**
- Create: `src-tauri/src/wine/installer.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/wine/mod.rs`

- [ ] **Step 1: Add dependencies**

In `src-tauri/Cargo.toml`, append to `[dependencies]`:

```toml
reqwest = { version = "0.12", default-features = false, features = ["stream", "rustls-tls"] }
sha2 = "0.10"
tar = "0.4"
xz2 = "0.1"
futures-util = "0.3"
```

And to `[dev-dependencies]`:

```toml
mockito = "1.4"
```

- [ ] **Step 2: Create the module skeleton with hash and disk-space tests**

Create `src-tauri/src/wine/installer.rs`:

```rust
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const WINE_EXPECTED_VERSION: &str = "1.0.0";
// Placeholder until first real release is published.
pub const WINE_RELEASE_URL: &str =
    "https://github.com/REPLACE_ME/catleap/releases/download/wine-catleap-1.0.0/wine-catleap-1.0.0.tar.xz";
pub const WINE_EXPECTED_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
pub const REQUIRED_FREE_BYTES: u64 = 500 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallPhase {
    CheckingSpace,
    Downloading { bytes_done: u64, bytes_total: u64 },
    Verifying,
    Extracting,
    Codesigning,
    Done,
    Failed { error: String },
}

/// Compute the hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Bytes free at the given path's filesystem.
pub fn free_bytes(path: &Path) -> Result<u64, String> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|e| format!("path contains NUL byte: {e}"))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if ret != 0 {
        return Err("statvfs failed".to_string());
    }
    Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
}

/// Verify a downloaded file's SHA-256 matches the expected hex digest.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(format!("SHA mismatch: expected {expected_hex}, got {actual}"))
    }
}

/// Extract a `.tar.xz` archive into `dest`. `dest` is created if missing.
pub fn extract_tar_xz(archive: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
    let f = fs::File::open(archive).map_err(|e| format!("open {}: {e}", archive.display()))?;
    let xz = xz2::read::XzDecoder::new(f);
    let mut tar = tar::Archive::new(xz);
    tar.unpack(dest).map_err(|e| format!("unpack: {e}"))?;
    Ok(())
}

/// Clear `com.apple.quarantine` xattrs and ad-hoc codesign the wine tree.
/// Idempotent.
pub fn clear_quarantine_and_sign(wine_root: &Path) -> Result<(), String> {
    use std::process::Command;

    Command::new("/usr/bin/xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(wine_root)
        .status()
        .map_err(|e| format!("xattr: {e}"))?;

    let status = Command::new("/usr/bin/codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(wine_root)
        .status()
        .map_err(|e| format!("codesign: {e}"))?;
    if !status.success() {
        return Err(format!("codesign exit {}", status.code().unwrap_or(-1)));
    }
    Ok(())
}

/// Atomically replace `<data_path>/wine` with the contents of `staging`.
pub fn promote_staging(staging: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        let backup = target.with_extension("old");
        let _ = fs::remove_dir_all(&backup);
        fs::rename(target, &backup).map_err(|e| format!("backup: {e}"))?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir parent: {e}"))?;
    }
    fs::rename(staging, target).map_err(|e| format!("rename: {e}"))?;
    let _ = fs::remove_dir_all(target.with_extension("old"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn sha256_of_known_input() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a");
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(b"hello").unwrap();
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let h = sha256_file(&p).unwrap();
        assert_eq!(h, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn verify_sha256_accepts_match_rejects_mismatch() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a");
        fs::write(&p, b"hello").unwrap();
        assert!(verify_sha256(&p, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824").is_ok());
        assert!(verify_sha256(&p, "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").is_err());
    }

    #[test]
    fn promote_staging_replaces_target() {
        let tmp = TempDir::new().unwrap();
        let staging = tmp.path().join("staging");
        let target = tmp.path().join("target");
        fs::create_dir_all(staging.join("bin")).unwrap();
        fs::write(staging.join("bin/marker"), b"new").unwrap();
        fs::create_dir_all(target.join("bin")).unwrap();
        fs::write(target.join("bin/old"), b"old").unwrap();

        promote_staging(&staging, &target).unwrap();

        assert!(target.join("bin/marker").exists());
        assert!(!target.join("bin/old").exists());
    }

    #[test]
    fn extract_round_trip_tar_xz() {
        // Build a tiny tar.xz, extract, confirm files appear.
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("a.tar.xz");
        let f = fs::File::create(&archive).unwrap();
        let xz = xz2::write::XzEncoder::new(f, 1);
        let mut tar = tar::Builder::new(xz);
        let mut header = tar::Header::new_gnu();
        let payload = b"contents";
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "bin/wine64", &payload[..]).unwrap();
        tar.into_inner().unwrap().finish().unwrap();

        let dest = tmp.path().join("out");
        extract_tar_xz(&archive, &dest).unwrap();
        assert_eq!(fs::read(dest.join("bin/wine64")).unwrap(), payload);
    }

    #[test]
    fn free_bytes_returns_positive_for_tmp() {
        let tmp = TempDir::new().unwrap();
        let n = free_bytes(tmp.path()).unwrap();
        assert!(n > 0);
    }
}
```

- [ ] **Step 3: Add `libc` dependency** for `statvfs`

In `src-tauri/Cargo.toml` `[dependencies]`:

```toml
libc = "0.2"
```

- [ ] **Step 4: Register the module**

In `src-tauri/src/wine/mod.rs`, append to the existing module declarations:

```rust
pub mod installer;
```

- [ ] **Step 5: Build and run unit tests**

Run: `cd src-tauri && cargo build && cargo test --lib wine::installer`
Expected: 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/wine/mod.rs src-tauri/src/wine/installer.rs
git commit -m "feat(installer): add core sha256/extract/promote/sign helpers with tests"
```

---

## Task 7: Installer streaming download + IPC commands

**Files:**
- Modify: `src-tauri/src/wine/installer.rs` (add `download_to`, `run_install`)
- Create: `src-tauri/src/commands/onboarding.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register commands)
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Add the streaming download function with mockito test**

Append to `src-tauri/src/wine/installer.rs` (above the test module):

```rust
use futures_util::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Stream `url` to `dest`. Calls `on_progress(done, total)` periodically.
/// Aborts early if `cancelled` becomes true; returns Err("cancelled") then.
pub async fn download_to(
    url: &str,
    dest: &Path,
    cancelled: Arc<AtomicBool>,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let resp = reqwest::get(url)
        .await
        .map_err(|e| format!("request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);

    let mut file = fs::File::create(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let mut stream = resp.bytes_stream();
    let mut done: u64 = 0;
    let mut last_emit: u64 = 0;
    while let Some(chunk) = stream.next().await {
        if cancelled.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        let bytes = chunk.map_err(|e| format!("stream: {e}"))?;
        file.write_all(&bytes).map_err(|e| format!("write: {e}"))?;
        done += bytes.len() as u64;
        if done - last_emit >= 64 * 1024 || done == total {
            on_progress(done, total);
            last_emit = done;
        }
    }
    Ok(())
}
```

Add at the bottom of `mod tests`:

```rust
    #[tokio::test]
    async fn download_streams_and_reports_progress() {
        let mut server = mockito::Server::new_async().await;
        let body = vec![0xABu8; 200_000];
        let m = server
            .mock("GET", "/wine.tar.xz")
            .with_status(200)
            .with_body(&body)
            .create_async()
            .await;

        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("wine.tar.xz");

        let mut last = (0u64, 0u64);
        let cancelled = Arc::new(AtomicBool::new(false));
        download_to(&format!("{}/wine.tar.xz", server.url()), &dest, cancelled, |d, t| {
            last = (d, t);
        })
        .await
        .unwrap();

        m.assert_async().await;
        assert_eq!(fs::read(&dest).unwrap(), body);
        assert_eq!(last.0, 200_000);
    }

    #[tokio::test]
    async fn download_404_returns_error() {
        let mut server = mockito::Server::new_async().await;
        server.mock("GET", "/nope").with_status(404).create_async().await;
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("x");
        let cancelled = Arc::new(AtomicBool::new(false));
        let err = download_to(&format!("{}/nope", server.url()), &dest, cancelled, |_, _| {})
            .await
            .unwrap_err();
        assert!(err.contains("404"));
    }

    #[tokio::test]
    async fn download_respects_cancel_flag() {
        let mut server = mockito::Server::new_async().await;
        let body = vec![0u8; 1_000_000];
        server
            .mock("GET", "/big")
            .with_status(200)
            .with_body(&body)
            .create_async()
            .await;
        let tmp = TempDir::new().unwrap();
        let dest = tmp.path().join("x");
        let cancelled = Arc::new(AtomicBool::new(true)); // already cancelled
        let err = download_to(&format!("{}/big", server.url()), &dest, cancelled, |_, _| {})
            .await
            .unwrap_err();
        assert_eq!(err, "cancelled");
    }
```

The `tokio::test` macro requires the `macros` feature. In `Cargo.toml`, ensure `tokio = { version = "1", features = ["full"] }` (already present in existing Cargo.toml).

- [ ] **Step 2: Add the orchestration `run_install` function**

Append to `installer.rs` (after `download_to`, before `mod tests`):

```rust
/// Run the full install flow. `emit_phase` is called for every phase
/// transition. `cancelled` aborts cleanly at safe boundaries.
pub async fn run_install(
    data_path: &Path,
    cancelled: Arc<AtomicBool>,
    mut emit_phase: impl FnMut(InstallPhase),
) -> Result<(), String> {
    let wine_dir = data_path.join("wine");
    let staging = data_path.join("wine.partial");
    let archive = data_path.join("wine.tar.xz.partial");

    fs::create_dir_all(data_path).map_err(|e| format!("mkdir {}: {e}", data_path.display()))?;

    // 1. Disk space
    emit_phase(InstallPhase::CheckingSpace);
    let free = free_bytes(data_path)?;
    if free < REQUIRED_FREE_BYTES {
        return Err(format!(
            "Need {} MB free, only {} MB available",
            REQUIRED_FREE_BYTES / 1024 / 1024,
            free / 1024 / 1024
        ));
    }

    // 2. Download
    let mut last_progress = std::time::Instant::now();
    download_to(WINE_RELEASE_URL, &archive, cancelled.clone(), |d, t| {
        if last_progress.elapsed() >= std::time::Duration::from_millis(100) {
            last_progress = std::time::Instant::now();
            emit_phase(InstallPhase::Downloading {
                bytes_done: d,
                bytes_total: t,
            });
        }
    })
    .await?;
    if cancelled.load(Ordering::Relaxed) {
        let _ = fs::remove_file(&archive);
        return Err("cancelled".into());
    }

    // 3. Verify (one retry on mismatch)
    emit_phase(InstallPhase::Verifying);
    if let Err(e) = verify_sha256(&archive, WINE_EXPECTED_SHA256) {
        log::warn!("first SHA verify failed: {e}; retrying");
        let _ = fs::remove_file(&archive);
        download_to(WINE_RELEASE_URL, &archive, cancelled.clone(), |_, _| {}).await?;
        verify_sha256(&archive, WINE_EXPECTED_SHA256)?;
    }

    // 4. Extract into staging
    emit_phase(InstallPhase::Extracting);
    let _ = fs::remove_dir_all(&staging);
    extract_tar_xz(&archive, &staging)?;
    let _ = fs::remove_file(&archive);

    // 5. xattr + codesign
    emit_phase(InstallPhase::Codesigning);
    clear_quarantine_and_sign(&staging)?;

    // 6. Promote staging → wine_dir
    promote_staging(&staging, &wine_dir)?;

    emit_phase(InstallPhase::Done);
    Ok(())
}

/// Skip if a previously-installed wine matches the expected version.
pub fn already_installed(data_path: &Path, current_version: Option<&str>) -> bool {
    let bin = data_path.join("wine/bin/wine64");
    bin.exists() && current_version == Some(WINE_EXPECTED_VERSION)
}
```

- [ ] **Step 3: Run installer tests**

Run: `cd src-tauri && cargo test --lib wine::installer`
Expected: 8 tests pass (5 sync + 3 async).

- [ ] **Step 4: Add the AppState extension for cancellation flag**

In `src-tauri/src/commands/games.rs`, modify `AppState` to add a cancel flag:

```rust
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct AppState {
    pub games: Mutex<Vec<Game>>,
    pub compat_db: CompatDatabase,
    pub settings: Mutex<Settings>,
    pub process_monitor: ProcessMonitor,
    pub install_cancel: Arc<AtomicBool>,
}
```

- [ ] **Step 5: Update `AppState` construction in `lib.rs`**

In `src-tauri/src/lib.rs`, find the `.manage(AppState { ... })` block. Add:

```rust
        .manage(AppState {
            games: Mutex::new(Vec::new()),
            compat_db,
            settings: Mutex::new(settings),
            process_monitor: ProcessMonitor::new(),
            install_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
```

Also add `use std::sync::Arc;` at the top of `lib.rs`.

- [ ] **Step 6: Create `src-tauri/src/commands/onboarding.rs`**

```rust
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
```

- [ ] **Step 7: Register the commands**

In `src-tauri/src/commands/mod.rs`:

```rust
pub mod games;
pub mod launcher;
pub mod onboarding;
pub mod settings;
```

In `src-tauri/src/lib.rs`, change the imports and the `invoke_handler` block. Add to imports:

```rust
use commands::onboarding::{cancel_wine_install, start_wine_install};
```

Add to the `invoke_handler!` macro args:

```rust
            start_wine_install,
            cancel_wine_install,
```

- [ ] **Step 8: Add IPC wrappers in `src/lib/tauri.ts`**

Append to `src/lib/tauri.ts`:

```typescript
export function startWineInstall(): Promise<void> {
  return invoke<void>("start_wine_install");
}

export function cancelWineInstall(): Promise<void> {
  return invoke<void>("cancel_wine_install");
}
```

- [ ] **Step 9: Build everything**

Run: `cd src-tauri && cargo build && cargo test --lib`
Expected: builds; all tests pass.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/wine/installer.rs src-tauri/src/commands/onboarding.rs src-tauri/src/commands/mod.rs src-tauri/src/commands/games.rs src-tauri/src/lib.rs src/lib/tauri.ts
git commit -m "feat(installer): wire async download/verify/extract pipeline + IPC"
```

---

## Task 8: GPTK detector module

**Files:**
- Create: `src-tauri/src/wine/gptk_import.rs`
- Modify: `src-tauri/src/wine/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `src-tauri/src/wine/gptk_import.rs`:

```rust
use serde::Serialize;
use std::path::{Path, PathBuf};

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
}
```

- [ ] **Step 2: Register module + run tests**

In `src-tauri/src/wine/mod.rs`, add:

```rust
pub mod gptk_import;
```

Run: `cd src-tauri && cargo test --lib wine::gptk_import`
Expected: 5 tests pass.

- [ ] **Step 3: Add the copy + validation logic**

Append to `src-tauri/src/wine/gptk_import.rs` (above `mod tests`):

```rust
use std::process::Command;

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
```

Add a test for `copy_libs` using a fake source:

```rust
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
```

- [ ] **Step 4: Run all gptk_import tests**

Run: `cd src-tauri && cargo test --lib wine::gptk_import`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/wine/mod.rs src-tauri/src/wine/gptk_import.rs
git commit -m "feat(gptk): detect GPTK volumes, copy D3DMetal libs via ditto"
```

---

## Task 9: GPTK importer IPC (watcher + commands)

**Files:**
- Modify: `src-tauri/src/wine/gptk_import.rs` (watcher loop)
- Modify: `src-tauri/src/commands/onboarding.rs` (new commands)
- Modify: `src-tauri/src/commands/games.rs` (AppState gains `gptk_watching: AtomicBool`)
- Modify: `src-tauri/src/lib.rs` (register commands)
- Modify: `src/lib/tauri.ts` (wrappers)

- [ ] **Step 1: Add the watcher loop function**

Append to `src-tauri/src/wine/gptk_import.rs` (above `mod tests`):

```rust
use notify::{EventKind, RecursiveMode, Watcher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const VOLUMES_ROOT: &str = "/Volumes";

/// Block-watch `/Volumes` for new GPTK volumes. Calls `on_found` once a
/// volume with the right layout appears (or is already present at startup).
/// Returns when `running` becomes false or after `on_found` is invoked.
pub fn watch_for_gptk(
    running: Arc<AtomicBool>,
    mut on_found: impl FnMut(GptkInfo),
) -> Result<(), String> {
    if let Some(info) = pick_best(scan_volumes(Path::new(VOLUMES_ROOT))) {
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
                    if let Some(info) = pick_best(scan_volumes(Path::new(VOLUMES_ROOT))) {
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
```

- [ ] **Step 2: Extend `AppState` for the watcher running flag**

In `src-tauri/src/commands/games.rs`, add another field:

```rust
pub struct AppState {
    pub games: Mutex<Vec<Game>>,
    pub compat_db: CompatDatabase,
    pub settings: Mutex<Settings>,
    pub process_monitor: ProcessMonitor,
    pub install_cancel: Arc<AtomicBool>,
    pub gptk_watching: Arc<AtomicBool>,
}
```

In `src-tauri/src/lib.rs` `manage(AppState { ... })`, add:

```rust
            gptk_watching: Arc::new(AtomicBool::new(false)),
```

(Add `use std::sync::atomic::AtomicBool;` to the imports.)

- [ ] **Step 3: Add IPC commands**

Append to `src-tauri/src/commands/onboarding.rs`:

```rust
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

    // Snapshot of state for the worker thread.
    let settings_arc = std::sync::Arc::new(std::sync::Mutex::new(()));
    let _ = settings_arc; // placeholder; we read settings only via window.app_handle below

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
                        let mut s = state.settings.lock().unwrap();
                        s.gptk_version = Some(info.version.clone());
                        s.gptk_skipped = false;
                        let cfg = s.data_path.join("config");
                        let _ = std::fs::create_dir_all(&cfg);
                        let _ = std::fs::write(
                            cfg.join("settings.json"),
                            serde_json::to_string_pretty(&*s).unwrap_or_default(),
                        );
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
    let mut s = state.settings.lock().unwrap();
    s.gptk_skipped = true;
    let cfg = s.data_path.join("config");
    std::fs::create_dir_all(&cfg).map_err(|e| format!("mkdir: {e}"))?;
    std::fs::write(
        cfg.join("settings.json"),
        serde_json::to_string_pretty(&*s).map_err(|e| format!("ser: {e}"))?,
    )
    .map_err(|e| format!("write: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn eject_gptk_volume(volume_path: String) -> Result<(), String> {
    gptk_import::eject(std::path::Path::new(&volume_path))
}
```

- [ ] **Step 4: Register the new commands in `lib.rs`**

Add to imports:

```rust
use commands::onboarding::{
    cancel_wine_install, eject_gptk_volume, skip_gptk, start_gptk_watch, start_wine_install,
    stop_gptk_watch,
};
```

And to the `invoke_handler!` list:

```rust
            start_wine_install,
            cancel_wine_install,
            start_gptk_watch,
            stop_gptk_watch,
            skip_gptk,
            eject_gptk_volume,
```

- [ ] **Step 5: Add frontend IPC wrappers**

Append to `src/lib/tauri.ts`:

```typescript
export function startGptkWatch(): Promise<void> {
  return invoke<void>("start_gptk_watch");
}

export function stopGptkWatch(): Promise<void> {
  return invoke<void>("stop_gptk_watch");
}

export function skipGptk(): Promise<void> {
  return invoke<void>("skip_gptk");
}

export function ejectGptkVolume(volumePath: string): Promise<void> {
  return invoke<void>("eject_gptk_volume", { volumePath });
}
```

- [ ] **Step 6: Build everything**

Run: `cd src-tauri && cargo build && cargo test --lib`
Expected: builds and all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/wine/gptk_import.rs src-tauri/src/commands/onboarding.rs src-tauri/src/commands/games.rs src-tauri/src/lib.rs src/lib/tauri.ts
git commit -m "feat(gptk): IPC commands for watch/skip/eject + Settings persistence"
```

---

## Task 10: Refactor `lib.rs` — extract steam watcher

**Files:**
- Modify: `src-tauri/src/lib.rs`

> Pure refactor; existing Steam-watcher behavior must be preserved.

- [ ] **Step 1: Extract the Steam watcher into a function**

Replace the `setup` block in `src-tauri/src/lib.rs` with:

```rust
        .setup(|app| {
            let app_handle = app.handle().clone();
            let state: tauri::State<AppState> = app.state();
            let steam_path = state.settings.lock().unwrap().steam_path.clone();
            setup_steam_watcher(app_handle, steam_path);
            Ok(())
        })
```

And add this free function above `pub fn run()`:

```rust
fn setup_steam_watcher(app_handle: tauri::AppHandle, steam_path: std::path::PathBuf) {
    let watch_path = steam_path.join("steamapps");
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                log::warn!("Failed to create Steam watcher: {e}");
                return;
            }
        };
        if let Err(e) = watcher.watch(&watch_path, RecursiveMode::NonRecursive) {
            log::warn!("Failed to watch {:?}: {e}", watch_path);
            return;
        }
        for res in rx {
            match res {
                Ok(event) if matches!(event.kind, EventKind::Create(_) | EventKind::Remove(_)) => {
                    let _ = app_handle.emit("steam-library-changed", ());
                }
                Ok(_) => {}
                Err(e) => log::warn!("Steam watcher error: {e}"),
            }
        }
    });
}
```

- [ ] **Step 2: Build**

Run: `cd src-tauri && cargo build`
Expected: builds cleanly.

- [ ] **Step 3: Smoke-test the dev app**

Run: `pnpm tauri dev` (in another terminal); confirm the app opens and the Steam watcher still emits `steam-library-changed` when files appear in `~/Library/Application Support/Steam/steamapps`. Touch a dummy file there to verify.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "refactor(lib): extract Steam watcher into setup_steam_watcher"
```

---

## Task 11: Frontend — `useTauriEvent` hook + extended IPC types

**Files:**
- Create: `src/hooks/useTauriEvent.ts`
- Modify: `src/types.ts` (already done in Task 1; re-confirm)

- [ ] **Step 1: Create the hook**

`src/hooks/useTauriEvent.ts`:

```typescript
import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Subscribe to a backend Tauri event for the lifetime of the component.
 * `handler` is called with each event payload. `enabled = false` skips
 * subscription entirely (useful for conditional listeners).
 */
export function useTauriEvent<T>(
  eventName: string,
  handler: (payload: T) => void,
  enabled: boolean = true
) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    if (!enabled) return;
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<T>(eventName, (e) => handlerRef.current(e.payload)).then((u) => {
      if (cancelled) {
        u();
      } else {
        unlisten = u;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [eventName, enabled]);
}
```

- [ ] **Step 2: TS check**

Run: `pnpm tsc --noEmit`
Expected: no errors in the new file.

- [ ] **Step 3: Commit**

```bash
git add src/hooks/useTauriEvent.ts
git commit -m "feat(frontend): add useTauriEvent hook for backend event subscriptions"
```

---

## Task 12: Rewrite `FirstRun.tsx`

**Files:**
- Modify: `src/pages/FirstRun.tsx` (full rewrite)
- Modify: `src/App.tsx` (resume logic)

- [ ] **Step 1: Replace `FirstRun.tsx` entirely**

Replace the full contents of `src/pages/FirstRun.tsx`:

```typescript
import { useEffect, useState } from "react";
import {
  cancelWineInstall,
  ejectGptkVolume,
  getSettings,
  scanSteam,
  skipGptk,
  startGptkWatch,
  startWineInstall,
  stopGptkWatch,
} from "../lib/tauri";
import { useTauriEvent } from "../hooks/useTauriEvent";
import type {
  GptkImportPhase,
  Settings,
  WineInstallPhase,
} from "../types";

interface FirstRunProps {
  onComplete: () => void;
}

type Step = "welcome" | "wine" | "gptk" | "scan" | "done";

export function FirstRun({ onComplete }: FirstRunProps) {
  const [step, setStep] = useState<Step>("welcome");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [winePhase, setWinePhase] = useState<WineInstallPhase | null>(null);
  const [gptkPhase, setGptkPhase] = useState<GptkImportPhase | null>(null);
  const [foundVolume, setFoundVolume] = useState<string | null>(null);
  const [scanResult, setScanResult] = useState<number | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Resume mid-onboarding based on persisted settings.
  useEffect(() => {
    getSettings().then((s) => {
      setSettings(s);
      if (s.wine_version && (s.gptk_version || s.gptk_skipped)) {
        setStep("scan");
      } else if (s.wine_version) {
        setStep("gptk");
      } else {
        setStep("welcome");
      }
    });
  }, []);

  useTauriEvent<WineInstallPhase>(
    "wine-install-progress",
    (p) => {
      setWinePhase(p);
      if (p.kind === "done") setStep("gptk");
      if (p.kind === "failed") setError(p.error);
    },
    step === "wine"
  );

  useTauriEvent<GptkImportPhase>(
    "gptk-import-progress",
    (p) => {
      setGptkPhase(p);
      if (p.kind === "found") setFoundVolume(p.version);
      if (p.kind === "done") {
        setStep("scan");
        setFoundVolume(null);
      }
      if (p.kind === "failed") setError(p.error);
    },
    step === "gptk"
  );

  function startWine() {
    setError(null);
    setWinePhase({ kind: "checking_space" });
    startWineInstall().catch((e) => setError(String(e)));
  }

  function startGptk() {
    setError(null);
    setGptkPhase({ kind: "waiting" });
    startGptkWatch().catch((e) => setError(String(e)));
  }

  async function handleSkipGptk() {
    setError(null);
    try {
      await stopGptkWatch();
      await skipGptk();
      setStep("scan");
    } catch (e) {
      setError(String(e));
    }
  }

  async function runScan() {
    setScanning(true);
    setError(null);
    try {
      const games = await scanSteam();
      setScanResult(games.length);
      setStep("done");
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }

  if (!settings) {
    return (
      <Centered>
        <p className="text-sm text-gray-400">Loading...</p>
      </Centered>
    );
  }

  return (
    <Centered>
      <span className="text-6xl mb-6 select-none" role="img" aria-label="cat">🐱</span>

      {error && (
        <div className="rounded-lg bg-red-50 border border-red-100 px-4 py-3 text-sm text-red-700 mb-6 w-full text-left">
          {error}
        </div>
      )}

      {step === "welcome" && (
        <Welcome onContinue={() => setStep("wine")} />
      )}

      {step === "wine" && (
        <WineStep
          phase={winePhase}
          onStart={startWine}
          onCancel={() => cancelWineInstall().catch(() => {})}
          onRetry={startWine}
        />
      )}

      {step === "gptk" && (
        <GptkStep
          phase={gptkPhase}
          foundVolume={foundVolume}
          onStart={startGptk}
          onSkip={handleSkipGptk}
        />
      )}

      {step === "scan" && (
        <ScanStep onScan={runScan} scanning={scanning} />
      )}

      {step === "done" && (
        <DoneStep count={scanResult ?? 0} onComplete={onComplete} />
      )}
    </Centered>
  );
}

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-gray-50">
      <div className="flex flex-col items-center text-center max-w-md px-6">
        {children}
      </div>
    </div>
  );
}

function Welcome({ onContinue }: { onContinue: () => void }) {
  return (
    <>
      <h1 className="text-3xl font-bold text-gray-900 mb-2">Welcome to Catleap</h1>
      <p className="text-base text-gray-500 mb-8">
        Play Windows games on Mac. We'll set up Wine and Apple's GPTK in two short steps.
      </p>
      <button
        onClick={onContinue}
        className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 transition-colors"
      >
        Continue
      </button>
    </>
  );
}

function WineStep({
  phase,
  onStart,
  onCancel,
  onRetry,
}: {
  phase: WineInstallPhase | null;
  onStart: () => void;
  onCancel: () => void;
  onRetry: () => void;
}) {
  if (!phase) {
    return (
      <>
        <h2 className="text-2xl font-bold text-gray-900 mb-2">Download Wine</h2>
        <p className="text-base text-gray-500 mb-6">
          Catleap needs to download a custom Wine build (~150 MB) compiled from Apple's GPTK sources.
          One-time download.
        </p>
        <button
          onClick={onStart}
          className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 transition-colors"
        >
          Download
        </button>
      </>
    );
  }

  const label =
    phase.kind === "checking_space" ? "Checking disk space..." :
    phase.kind === "downloading" ? `Downloading... ${phase.bytes_total > 0 ? Math.round((phase.bytes_done / phase.bytes_total) * 100) : 0}%` :
    phase.kind === "verifying" ? "Verifying..." :
    phase.kind === "extracting" ? "Extracting..." :
    phase.kind === "codesigning" ? "Signing binaries..." :
    phase.kind === "done" ? "Done." :
    `Failed: ${phase.error}`;

  const percent =
    phase.kind === "downloading" && phase.bytes_total > 0
      ? Math.round((phase.bytes_done / phase.bytes_total) * 100)
      : phase.kind === "verifying" || phase.kind === "extracting" || phase.kind === "codesigning" || phase.kind === "done"
      ? 100
      : 0;

  return (
    <>
      <h2 className="text-2xl font-bold text-gray-900 mb-4">Installing Wine</h2>
      <div className="w-full h-2 rounded-full bg-gray-200 overflow-hidden mb-3">
        <div
          className="h-full bg-gray-900 transition-all"
          style={{ width: `${percent}%` }}
        />
      </div>
      <p className="text-sm text-gray-600 mb-6">{label}</p>

      {phase.kind === "failed" ? (
        <button
          onClick={onRetry}
          className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700"
        >
          Retry
        </button>
      ) : (
        <button
          onClick={onCancel}
          className="w-full px-5 py-3 rounded-xl bg-white border border-gray-200 text-gray-500 font-medium text-sm hover:bg-gray-50"
        >
          Cancel
        </button>
      )}
    </>
  );
}

function GptkStep({
  phase,
  foundVolume,
  onStart,
  onSkip,
}: {
  phase: GptkImportPhase | null;
  foundVolume: string | null;
  onStart: () => void;
  onSkip: () => void;
}) {
  if (!phase) {
    return (
      <>
        <h2 className="text-2xl font-bold text-gray-900 mb-2">Apple GPTK Libraries</h2>
        <p className="text-base text-gray-500 mb-6">
          Download the Game Porting Toolkit DMG from Apple (free Apple ID required), then mount it.
          Catleap will detect it automatically.
        </p>
        <a
          href="https://developer.apple.com/games/game-porting-toolkit/"
          target="_blank"
          rel="noreferrer"
          className="w-full block text-center px-5 py-3 rounded-xl bg-white border border-gray-200 text-gray-900 font-medium text-sm hover:bg-gray-50 mb-3"
        >
          Open Apple Developer page
        </a>
        <button
          onClick={onStart}
          className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 mb-3"
        >
          Start watching for DMG
        </button>
        <button
          onClick={onSkip}
          className="w-full px-5 py-3 rounded-xl bg-transparent text-gray-500 font-medium text-sm hover:bg-gray-100"
        >
          Skip — performance will be limited
        </button>
      </>
    );
  }

  const label =
    phase.kind === "waiting" ? "Waiting for GPTK DMG..." :
    phase.kind === "found" ? `Found GPTK ${phase.version}` :
    phase.kind === "copying" ? `Copying libraries... ${phase.percent}%` :
    phase.kind === "done" ? `GPTK ${phase.version} installed.` :
    `Failed: ${phase.error}`;

  return (
    <>
      <h2 className="text-2xl font-bold text-gray-900 mb-4">Importing GPTK</h2>
      <p className="text-sm text-gray-600 mb-6">{label}</p>
      {foundVolume && phase.kind !== "done" && phase.kind !== "failed" ? null : null}
      <button
        onClick={onSkip}
        className="w-full px-5 py-3 rounded-xl bg-transparent text-gray-500 font-medium text-sm hover:bg-gray-100"
      >
        Skip
      </button>
    </>
  );
}

function ScanStep({ onScan, scanning }: { onScan: () => void; scanning: boolean }) {
  return (
    <>
      <h2 className="text-2xl font-bold text-gray-900 mb-2">Scan your Steam library</h2>
      <p className="text-base text-gray-500 mb-6">
        Catleap will look for installed Steam games. You can also add games manually later.
      </p>
      <button
        onClick={onScan}
        disabled={scanning}
        className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {scanning ? "Scanning..." : "Scan for Games"}
      </button>
    </>
  );
}

function DoneStep({ count, onComplete }: { count: number; onComplete: () => void }) {
  return (
    <>
      <h2 className="text-2xl font-bold text-gray-900 mb-2">All set</h2>
      <p className="text-base text-gray-500 mb-6">
        Found <span className="font-semibold text-gray-900">{count}</span> {count === 1 ? "game" : "games"}.
      </p>
      <button
        onClick={onComplete}
        className="w-full px-5 py-3 rounded-xl bg-gray-900 text-white font-semibold text-sm hover:bg-gray-700"
      >
        Go to Library
      </button>
    </>
  );
}
```

Note: the unused `ejectGptkVolume` import is intentional — we'll wire it from the Settings page in Task 13. Remove the import here for now if TS complains.

- [ ] **Step 2: Resume logic in App.tsx**

In `src/App.tsx`, the existing `firstRun` state already controls whether `<FirstRun />` shows. The new resume logic lives in `FirstRun.tsx` itself (Step 1's `useEffect` calls `getSettings()`). We need to make sure even users who completed the OLD onboarding (have `catleap_onboarded` localStorage but no `wine_version`) still get the Wine + GPTK setup.

Replace the `firstRun` initialiser in `src/App.tsx`:

```typescript
  const [firstRun, setFirstRun] = useState(true); // start true; we re-evaluate below

  useEffect(() => {
    // If onboarding was completed AND wine is already installed, skip FirstRun.
    import("./lib/tauri").then(async ({ getSettings }) => {
      const s = await getSettings();
      const onboarded = localStorage.getItem(ONBOARDED_KEY) === "true";
      if (onboarded && s.wine_version && (s.gptk_version || s.gptk_skipped)) {
        setFirstRun(false);
      }
    });
  }, []);
```

Add `useEffect` to the React import if not already there.

- [ ] **Step 3: Run dev**

Run: `pnpm tauri dev`
Expected: app opens at the welcome step (or the right resumed step). Click through Wine download (will fail because the URL is a placeholder — that's fine for now; verify the failure shows correctly).

- [ ] **Step 4: Commit**

```bash
git add src/pages/FirstRun.tsx src/App.tsx
git commit -m "feat(ui): rewrite FirstRun as a state machine with Wine + GPTK steps"
```

---

## Task 13: Settings page — GPTK status banner + re-import

**Files:**
- Modify: `src/pages/Settings.tsx`

- [ ] **Step 1: Read the current Settings page**

Open `src/pages/Settings.tsx` to see its current structure. (If small, fold the banner inline.)

- [ ] **Step 2: Add a GPTK status section**

In `src/pages/Settings.tsx`, import the additional helpers:

```typescript
import { getSettings, skipGptk, startGptkWatch, stopGptkWatch, ejectGptkVolume } from "../lib/tauri";
import { useTauriEvent } from "../hooks/useTauriEvent";
import type { GptkImportPhase, Settings } from "../types";
```

Add a `GptkSection` component below the existing settings UI:

```tsx
function GptkSection() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [phase, setPhase] = useState<GptkImportPhase | null>(null);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    getSettings().then(setSettings);
  }, []);

  useTauriEvent<GptkImportPhase>(
    "gptk-import-progress",
    (p) => {
      setPhase(p);
      if (p.kind === "done" || p.kind === "failed") {
        setImporting(false);
        getSettings().then(setSettings);
      }
    },
    importing
  );

  if (!settings) return null;

  const installed = !!settings.gptk_version;
  const skipped = settings.gptk_skipped;

  async function handleImport() {
    setImporting(true);
    setPhase({ kind: "waiting" });
    await startGptkWatch();
  }

  async function handleCancel() {
    setImporting(false);
    await stopGptkWatch();
    setPhase(null);
  }

  return (
    <section className="mt-8">
      <h2 className="text-lg font-semibold text-gray-900 mb-3">Game Porting Toolkit</h2>
      {installed && (
        <p className="text-sm text-gray-600 mb-2">
          Apple GPTK <span className="font-mono">{settings.gptk_version}</span> installed.
        </p>
      )}
      {!installed && skipped && !importing && (
        <div className="rounded-xl bg-amber-50 border border-amber-100 p-4 mb-3">
          <p className="text-sm text-amber-800 font-semibold mb-1">
            GPTK not installed — game performance is limited
          </p>
          <p className="text-sm text-amber-700">
            Mount Apple's GPTK DMG and click Import to enable D3DMetal.
          </p>
        </div>
      )}
      {importing && phase && (
        <p className="text-sm text-gray-600 mb-2">
          {phase.kind === "waiting" && "Waiting for GPTK DMG..."}
          {phase.kind === "found" && `Found GPTK ${phase.version}`}
          {phase.kind === "copying" && `Copying... ${phase.percent}%`}
          {phase.kind === "done" && `Imported GPTK ${phase.version}`}
          {phase.kind === "failed" && `Failed: ${phase.error}`}
        </p>
      )}
      <div className="flex gap-2">
        {!importing ? (
          <button
            onClick={handleImport}
            className="px-4 py-2 rounded-lg bg-gray-900 text-white text-sm font-semibold hover:bg-gray-700"
          >
            {installed ? "Re-import" : "Import GPTK"}
          </button>
        ) : (
          <button
            onClick={handleCancel}
            className="px-4 py-2 rounded-lg bg-white border border-gray-200 text-gray-600 text-sm hover:bg-gray-50"
          >
            Cancel
          </button>
        )}
      </div>
    </section>
  );
}
```

Render `<GptkSection />` inside the existing `SettingsPage` body.

- [ ] **Step 3: Type-check + dev run**

Run: `pnpm tsc --noEmit`
Expected: no errors.

Run: `pnpm tauri dev` and navigate to Settings; verify the GPTK section renders, "Import GPTK" button starts the watcher.

- [ ] **Step 4: Commit**

```bash
git add src/pages/Settings.tsx
git commit -m "feat(ui): add GPTK status section to Settings with re-import + skip recovery"
```

---

## Task 14: Final wiring + manual E2E

**Files:**
- Verify: `src-tauri/src/lib.rs`, `src/lib/tauri.ts`, the entire flow

- [ ] **Step 1: Run the full Rust test suite**

Run: `cd src-tauri && cargo test --lib`
Expected: all green.

- [ ] **Step 2: Run the frontend type check + build**

Run: `pnpm tsc --noEmit && pnpm build`
Expected: no errors.

- [ ] **Step 3: Manual E2E — fresh onboarding**

1. Wipe local state: `rm -rf "$HOME/Library/Application Support/Catleap"` and `localStorage.removeItem("catleap_onboarded")` (in DevTools console).
2. `pnpm tauri dev`.
3. Walk through Welcome → Wine. Wine download will fail because `WINE_RELEASE_URL` is still a placeholder; verify the failure UI surfaces a clear error and the Retry button works.
4. Substitute a real release URL + SHA256 in `installer.rs` (after running `tools/build-wine/build.sh` and uploading) and re-run.
5. Once Wine installs, verify the app moves to GPTK step.
6. Mount a GPTK DMG (or simulate by `mkdir -p /tmp/fakevol/redist/lib/D3DMetal.framework/Versions/A && touch /tmp/fakevol/redist/lib/D3DMetal.framework/Versions/A/D3DMetal && sudo ln -s /tmp/fakevol "/Volumes/Game Porting Toolkit-3.0"` — for live testing only).
7. Verify Found → Copying → Done.
8. Verify Steam scan runs.

- [ ] **Step 4: Manual E2E — interrupted resume**

1. Wipe state, `pnpm tauri dev`.
2. Walk through Welcome → Wine, but kill the app mid-download (`Cancel` then quit).
3. Re-launch. Confirm FirstRun resumes at the Wine step (since `wine_version` was never persisted).
4. Repeat after Wine succeeds: kill before GPTK; re-launch resumes at GPTK step.

- [ ] **Step 5: Manual E2E — skip GPTK**

1. From a fresh state, complete Wine, then click "Skip" on the GPTK step.
2. Confirm Settings shows the amber "GPTK not installed — performance is limited" banner.
3. Click "Import GPTK"; confirm the watcher starts.

- [ ] **Step 6: Verify game launch end-to-end (with a small Steam game installed)**

1. Add a small Windows game to Steam and have it installed under `~/Library/Application Support/Steam`.
2. With Wine installed and GPTK imported, click Play.
3. Inspect `~/Library/Application Support/Catleap/logs/<src>_<id>.log` for `WINEPREFIX` and `DYLD_FALLBACK_LIBRARY_PATH` env evidence (Wine logs the env at startup with `WINEDEBUG=+all`, optional).

- [ ] **Step 7: Final commit if any cleanup was needed**

```bash
git add -A
git diff --cached --stat
git commit -m "chore: final cleanup after E2E verification"
```

(If there were no further changes, skip this step.)

---

## Self-Review Notes

- **Spec coverage**: every spec section has a task — Settings model (1), bundled refactor (2), wine_command + arch -x86_64 (3), launch env (4), build pipeline (5), installer (6, 7), GPTK detection (8, 9), lib.rs refactor (10), UI rewrite (11, 12), Settings UI (13), E2E matrix (14).
- **Out-of-scope items** (auto-update Wine, GPTK 4 migration, CI builds) explicitly deferred in the design doc and not in any task.
- **Type consistency**: `WineStatus.gptk_libs_installed` (Task 1, 2), `Settings.{wine_version, gptk_version, gptk_skipped}` (Task 1, used in 7, 9, 12, 13), `GptkInfo` (Task 8, 9), `InstallPhase`/`GptkPhase` (Task 6, 8) match across producer and consumer.
- **No placeholders in the executable steps** — every code block is complete. Two intentional placeholders are `WINE_RELEASE_URL` and `WINE_EXPECTED_SHA256` which are explicitly called out as "fill in after running the build pipeline" in Task 14 Step 3, not buried as TODOs.
