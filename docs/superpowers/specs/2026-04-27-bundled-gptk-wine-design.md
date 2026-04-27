# Bundled GPTK Wine — Design

**Status**: approved for plan-writing
**Date**: 2026-04-27
**Author**: Augusto Linhares (with Claude)

## Problem

Catleap's current onboarding instructs users to install Apple Game Porting Toolkit via the official `apple/apple/game-porting-toolkit` Homebrew formula. This is broken end-to-end:

- The formula depends on `openssl@1.1`, which has been removed from Homebrew core: `brew install` fails immediately.
- The formula requires `x86_64` architecture and installs only under `/usr/local/...` (Intel brew). `bundled.rs` only searches `/opt/homebrew/...` (arm64 brew), so even if a user got the install to work via Intel brew, Catleap would not detect it.
- Apple does not distribute a `wine64` binary directly: their official DMG ships only the proprietary D3DMetal libraries, scripts, and a README.

The user has rejected the third-party `gcenx/wine` cask as an installation path for end users.

## Goals

- Users install Catleap and run Windows games **without ever touching Homebrew**.
- The `wine64` we ship is built from Apple's official GPTK Wine sources (CodeWeavers 22.1.1 + Apple's patches), not a third-party fork.
- Apple's D3DMetal libraries are imported from the user's own legally-downloaded GPTK DMG and used at game launch for DirectX→Metal translation.
- The first-run experience is linear: download Wine → import GPTK DMG → scan games.

## Non-goals (deferred)

- Auto-update of the bundled Wine binary in-place (manual re-download via re-onboarding for now).
- Per-game Wine versions (Proton-style).
- GPTK 4+ migration (revisit when Apple ships it).
- CI/GitHub Actions automation of the build pipeline (manual builds in MVP).
- Re-distributing or bundling D3DMetal libraries (Apple licensing forbids).

## Architecture

Three independent artifacts, each managed by a different code path:

```
~/Library/Application Support/Catleap/
├── wine/                ← downloaded on first-run from our GitHub Release
│   ├── bin/wine64
│   ├── bin/wineserver, wineboot
│   ├── lib/wine/x86_64-unix/*.so
│   └── lib/wine/x86_32on64-unix/*.so
├── gptk/                ← copied from user's mounted Apple GPTK DMG
│   └── lib/
│       ├── D3DMetal.framework/
│       └── external/*.dylib
└── prefixes/<game_id>/  ← created on demand at game launch (existing)
```

Three artifacts: `wine` (~150 MB, ours), `gptk` (~20 MB, Apple's, user-supplied), `prefixes` (per-game, lazy).

Build pipeline lives in `tools/build-wine/` in this repo and is run manually offline. It produces `wine-catleap-<version>.tar.xz` published as a GitHub Release asset.

## Components

### Build pipeline (`tools/build-wine/`)

`build.sh` runs on Apple Silicon under Rosetta (`arch -x86_64 zsh`) with Intel Homebrew at `/usr/local`. It:

1. Verifies x86_64 environment and Intel Homebrew presence; aborts otherwise.
2. Installs build dependencies via Intel brew: `gstreamer`, `freetype`, `sdl2`, `gnutls`, `openssl@1.1`, `mingw-w64`, `bison`, `flex`, `pkg-config`, plus the runtime deps from the Apple formula.
3. Downloads `crossover-sources-22.1.1.tar.gz` from `media.codeweavers.com` and extracts the `wine/` subdirectory (matching the `TarballDownloadStrategy` in the Apple formula).
4. Applies Apple's patch (extracted from the formula's `__END__` block).
5. Configures and builds Wine64 with Apple's flags, then Wine32on64.
6. Installs into a temporary `--prefix`, runs the `post_install` rpath-fix step (rewrites dylib IDs to `@rpath/...`, codesigns ad-hoc).
7. Packages a tar.xz containing only `bin/`, `lib/wine/`, and `share/wine/`.
8. Emits a sibling `.sha256` file.

**`openssl@1.1` workaround**: pinned via a tap or archived bottle; the script fails fast with a clear error if absent. Exact mechanism decided in implementation.

**Versioning**: `wine-catleap-<X.Y.Z>` semver owned by us (independent of Apple's GPTK numbering). Major bumps when changing source base; minor for patch changes; patch for rebuild-only.

**Distribution**: GitHub Release on the Catleap repo with the tar.xz and SHA256 as assets. `WINE_RELEASE_URL` and `WINE_EXPECTED_SHA256` are constants in `installer.rs`.

### First-run installer (`src-tauri/src/wine/installer.rs`, new)

New IPC commands:

```rust
#[tauri::command]
pub async fn start_wine_install(window: Window, state: State<AppState>) -> Result<(), String>;

#[tauri::command]
pub fn cancel_wine_install(state: State<AppState>) -> Result<(), String>;
```

Emits `wine-install-progress` events to the frontend:

```rust
enum InstallPhase {
    CheckingSpace,
    Downloading { bytes_done: u64, bytes_total: u64 },
    Verifying,
    Extracting,
    Codesigning,
    Done,
    Failed(String),
}
```

**Flow** (background tokio task):
1. `CheckingSpace`: requires ≥ 500 MB free at `data_path`; fail with clear message otherwise.
2. `Downloading`: streams `WINE_RELEASE_URL` to `<data_path>/wine.tar.xz.partial` via `reqwest`. Emits progress every 64 KB.
3. `Verifying`: computes SHA-256 of partial file; compares to `WINE_EXPECTED_SHA256`. On mismatch, deletes and retries once; second mismatch is a hard fail.
4. `Extracting`: tar+xz-decompress into `<data_path>/wine/`. Existing `wine/` is wiped first.
5. `Codesigning`: `xattr -dr com.apple.quarantine <wine_root>` and `codesign --force --deep --sign - <wine_root>` to ensure binaries launch.
6. `Done`: persists `Settings.wine_version` to `settings.json`.

**Idempotency**: if `<data_path>/wine/bin/wine64` exists and `Settings.wine_version` matches the constant, the entire install is skipped.

**Cancel**: drops the in-flight reqwest stream and removes any `.partial` files.

**Crates added**: `reqwest = { version = "0.12", features = ["stream", "rustls-tls"] }`, `sha2 = "0.10"`, `tar = "0.4"`, `xz2 = "0.1"`, `futures-util = "0.3"`.

### GPTK DMG detector (`src-tauri/src/wine/gptk_import.rs`, new)

New IPC commands:

```rust
#[tauri::command]
pub fn start_gptk_watch(window: Window, state: State<AppState>) -> Result<(), String>;

#[tauri::command]
pub fn stop_gptk_watch(state: State<AppState>) -> Result<(), String>;

#[tauri::command]
pub fn skip_gptk(state: State<AppState>) -> Result<(), String>;
```

Emits `gptk-import-progress` events:

```rust
enum GptkPhase {
    Waiting,                       // no GPTK volume found yet
    Found { version: String },     // detected; copying starting
    Copying { percent: u8 },
    Done { version: String },
    Failed(String),
}
```

**Initial scan**: lists `/Volumes/*` and checks each for `redist/lib/D3DMetal.framework`.

**Watcher**: `notify::recommended_watcher` on `/Volumes` with `RecursiveMode::NonRecursive`. Create events trigger re-scan.

**Detection**:
```rust
fn detect_gptk_in_volume(path: &Path) -> Option<GptkInfo> {
    let lib = path.join("redist/lib");
    let framework = lib.join("D3DMetal.framework");
    framework.exists().then(|| GptkInfo {
        lib_path: lib,
        version: parse_version_from_volume_name(path).unwrap_or_else(|| "unknown".to_string()),
    })
}
```

**Volume name parsing** handles both historical Apple naming conventions: `"Game Porting Toolkit-<version>"` (3.x format) and `"Evaluation environment for Windows games <version>"` (2.x format). Falls back to `"unknown"` if neither matches.

**Multiple volumes**: highest parsed semver wins; if any version is `"unknown"` it ranks last; ties broken by mount order.

**Copy**: spawns `ditto -V <volume>/redist/lib/ <data_path>/gptk/lib/`. `ditto` preserves resource forks, symlinks, and extended attributes which a pure-Rust copy would mishandle for `.framework` bundles. Streams `ditto`'s stderr to estimate progress (best-effort percent, falls back to indeterminate spinner).

**Post-copy validation**: confirms `<data_path>/gptk/lib/D3DMetal.framework/Versions/A/D3DMetal` exists. On failure, removes partial `gptk/` dir and emits `Failed`.

**Idempotency**: if `<data_path>/gptk/lib/D3DMetal.framework` already exists with the same parsed version, the copy is skipped (UI shows "GPTK <version> already installed; click to re-import").

**Skip path**: `skip_gptk()` sets `Settings.gptk_skipped = true`. Game launches proceed without D3DMetal env vars; Settings page surfaces a "GPTK not installed — performance limited" banner with an "Import GPTK now" button that re-enters the import flow and clears `gptk_skipped` on success.

**Eject helper**: post-completion, frontend offers "Eject GPTK DMG"; calls `hdiutil detach <volume>` via a small IPC command.

### Detection refactor (`src-tauri/src/wine/bundled.rs`)

Remove all paths under `/opt/homebrew/opt/game-porting-toolkit/...` and `/opt/homebrew/Cellar/game-porting-toolkit/...`. Those never worked for GPTK and only confuse maintenance.

New search order:
1. `<data_path>/wine/bin/wine64` (our bundled, primary)
2. `/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64` (commercial fallback, kept for users who already have it)
3. `which wine64` (last-resort PATH lookup)

Extend `WineStatus`:
```rust
pub struct WineStatus {
    pub installed: bool,
    pub variant: String,
    pub path: String,
    pub gptk_libs_installed: bool,   // <data_path>/gptk/lib/D3DMetal.framework
}
```

`homebrew_available` is removed — the new FirstRun no longer surfaces brew install commands so the field has no consumer.

`detect_variant`:
- Path under `<data_path>/wine` AND GPTK libs present → `"catleap-gptk"`
- Path under `<data_path>/wine` without GPTK libs → `"catleap-wine"`
- CrossOver.app → `"crossover"`
- Other → `"wine"`

### Game launch integration (`src-tauri/src/wine/runner.rs`, `prefix.rs`)

`build_launch_env` accepts the GPTK lib path as a new parameter:

```rust
pub fn build_launch_env(
    wine_binary: &Path,
    prefix_path: &Path,
    compat: Option<&CompatEntry>,
    gptk_lib_path: Option<&Path>,    // new
) -> HashMap<String, String>
```

When `gptk_lib_path` is `Some(p)`, sets:

| Env var | Value |
|---|---|
| `DYLD_FALLBACK_LIBRARY_PATH` | `<p>:<p>/external` |
| `WINEDLLPATH` | `<wine_root>/lib/wine/x86_64-windows` |
| `WINEESYNC` | `1` |
| `WINEMSYNC` | `1` |
| `ROSETTA_ADVERTISE_AVX` | `1` |

Existing `WINEPREFIX`, `WINEARCH`, `PATH` unchanged.

**Critical: x86_64 invocation**. Apple's GPTK Wine is x86_64-only. Every `Command::new(&wine_binary)` site in `prefix.rs::create_prefix`, `prefix.rs::apply_dll_overrides`, and `runner.rs::launch_game` becomes:

```rust
Command::new("/usr/bin/arch").args(["-x86_64", wine_binary.to_str().unwrap(), ...])
```

Centralised behind a helper `wine_command(wine_binary: &Path) -> Command` to avoid drift.

### First-run UI (`src/pages/FirstRun.tsx`, rewrite)

Replaces the current Homebrew-based content. Linear flow with a state machine:

1. **Welcome** — short pitch, "Continue" button.
2. **Wine install** — explains the ~150 MB download, "Continue" or "Cancel". On Continue: progress UI subscribed to `wine-install-progress`. Errors show details + Retry.
3. **GPTK import** — explains the legal-download requirement, links to `developer.apple.com/games/game-porting-toolkit/`, shows "Waiting for DMG..." then "Found GPTK <version>!" then "Copying...". `Skip for now` button. Subscribed to `gptk-import-progress`.
4. **Steam scan** — existing flow, kept as-is.
5. **Done** — "Go to Library".

Removes: the duplicated `<p>` block at lines 143–148, the `http://` URLs, the old "Wine/GPTK not found" branch.

State persisted via `localStorage.catleap_onboarded` plus the new `Settings.wine_version`/`Settings.gptk_version` fields. If `wine_version` is set but `gptk_version` is missing and the user did not skip, FirstRun resumes at step 3 on relaunch.

## Data flow

```
First run:
  user clicks Continue → start_wine_install (async)
    → reqwest stream → SHA verify → tar+xz extract → codesign → emit Done
  user clicks Continue → start_gptk_watch
    → notify::Watcher on /Volumes → user mounts DMG
    → detect_gptk_in_volume → ditto copy → emit Done
  scan Steam (existing)

Game launch:
  play_game(id)
    → wine_binary = find_wine_binary(data_path)
    → gptk_lib_path = data_path/gptk/lib if exists
    → build_launch_env(wine_binary, prefix, compat, gptk_lib_path)
    → wine_command(wine_binary) with env_clear + envs
    → spawn → ProcessMonitor::track
```

## Failure modes

| Failure | Detection | Recovery |
|---|---|---|
| Wine release URL 404 | reqwest status | UI error + Retry; logs URL |
| Wine SHA mismatch | post-download hash | Auto-retry once; second failure is hard fail with "report this" |
| Disk full | pre-check + ENOSPC mid-write | UI prompts to free space |
| Quarantine blocks wine64 | `wineboot --init` exit code | `xattr -dr` + retry; manual fallback instructions if still failing |
| GPTK volume unmounted mid-copy | `ditto` exit code | Re-detect when remounted; user sees "lost connection to volume" |
| Unsupported GPTK DMG layout | `D3DMetal.framework` missing | "Unsupported GPTK version: <volume name>" + issue tracker link |
| User skips GPTK | flag in settings | Launch without D3DMetal env vars; Settings shows "performance limited" |
| Apple GPTK Wine source unavailable on rebuild | build script fail | Build pipeline error; ship a previous tar.xz until resolved |

## Versioning and updates

`Settings` gains:

```rust
pub wine_version: Option<String>,
pub gptk_version: Option<String>,
pub gptk_skipped: bool,
```

At startup `lib.rs::run` compares `settings.wine_version` to the compiled-in `WINE_EXPECTED_VERSION` constant. Mismatch surfaces a non-blocking banner offering re-install. Catleap version bumps that ship a new Wine simply bump the constant.

GPTK version is informational only — no auto-detection of newer versions; user must manually re-import.

## Refactoring included

- Extract the existing Steam file-watcher inline thread in `lib.rs::run` to a `setup_steam_watcher(app_handle, steam_path)` function so the new `setup_volumes_watcher` is symmetric.
- Centralise wine command construction behind `fn wine_command(wine_binary: &Path) -> Command` in `wine/mod.rs`.

No other refactors. Anything outside the GPTK story stays as-is.

## Testing

Unit tests:
- `installer.rs`: mock HTTP server (`mockito`); cases for 404, SHA mismatch, partial-then-success, cancel mid-stream.
- `gptk_import.rs`: tempdir-based fake `/Volumes` structure; cases for present, absent, multiple-versions-pick-highest.
- `bundled.rs`: existing tests extended with a `gptk_libs_installed` true/false case.
- `prefix.rs::build_launch_env`: new test asserting `DYLD_FALLBACK_LIBRARY_PATH` and friends only appear when `gptk_lib_path = Some`.
- `wine/mod.rs::wine_command`: asserts `arch -x86_64` prefix.

Manual E2E pre-release matrix:
- Fresh onboarding (no prior state) → full flow.
- Onboarding interrupted at wine download → relaunch resumes at correct step.
- Onboarding interrupted at GPTK import → relaunch resumes correctly.
- GPTK already installed before launch (DMG mounted) → detected by initial scan.
- GPTK skipped → game launches without D3DMetal, Settings reflects state.
- Re-import GPTK after a new Apple release → new version replaces old.

## Open implementation questions (for plan phase)

- Exact mechanism for sourcing `openssl@1.1` in the build script.
- Whether to ad-hoc codesign at extraction time or rely on the build's pre-signing.
- `ditto` progress estimation — read its stderr or use indeterminate UI.
- Hosting cost / bandwidth concerns for the GitHub Release tarball if Catleap gets traction (revisit only if it becomes real).

## Milestones

1. Build pipeline lands first; produces a working tar.xz manually.
2. Installer + UI ships behind the existing onboarding flag.
3. GPTK import + launch integration ship together (one cannot be useful without the other).
4. `bundled.rs` legacy paths removed only after the new flow is verified end-to-end.
