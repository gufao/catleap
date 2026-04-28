# Steam Runtime Inside Catleap — Design

**Status**: approved for plan-writing
**Date**: 2026-04-27
**Author**: Augusto Linhares (with Claude)

## Problem

Catleap users want to play Windows-only Steam games on Mac. Steam for macOS does not allow downloading Windows-only titles. The mainstream solution used by CrossOver, Whisky, and Heroic is to install **Steam for Windows** inside a Wine prefix; the Steam-Windows client thinks it is on Windows, lets the user download any Windows-only game, and the games run inside the same Wine environment alongside Steam itself.

Catleap does not currently provide this. There is no built-in way for a user to obtain Steam-Windows. Manual workarounds (terminal commands, `steamcmd` hacks) are unreasonable for an end-user product.

## Goals

- One-click "Install Steam" from Catleap's sidebar; the user does not interact with Wine prompts during bootstrap.
- One-click "Open Steam" once installed; user logs in through the familiar Steam-Windows UI.
- Games installed via Steam-Windows automatically appear in Catleap's Library as cards alongside macOS Steam and manual games, with a Play button per card.
- Per-game Play uses Steam-Windows internally (`Steam.exe -applaunch <appid>`) so DRM, achievements, cloud saves, and updates work normally.
- File watcher keeps the Library current as the user installs/uninstalls games inside Steam-Windows.
- "Reset Steam" recovery action wipes the runtime and re-installs cleanly.

## Non-goals (deferred)

- Catleap-side game install/uninstall via Steam (user manages that inside Steam-Windows for now).
- Cloud save sync UI in Catleap (Steam handles it transparently).
- Achievement / notification surfacing in Catleap.
- Multiple Steam accounts in parallel prefixes.
- macOS native Steam removal — keep both sources working in the Library.

## Architecture

```
~/Library/Application Support/Catleap/
├── wine/                      ← (existing) bundled Wine
├── gptk/                      ← (existing) imported D3DMetal libs
├── prefixes/
│   ├── manual_<uuid>/         ← (existing) per-game manual prefixes
│   ├── steam_<appid>/         ← (existing) per-game macOS Steam prefixes
│   └── _steam_runtime/        ← NEW shared prefix for Steam-Windows
│       └── drive_c/Program Files (x86)/Steam/
│           ├── Steam.exe
│           └── steamapps/
│               ├── libraryfolders.vdf
│               └── appmanifest_*.acf
└── cache/
    └── SteamSetup.exe         ← NEW cached installer
```

The leading underscore in `_steam_runtime` distinguishes runtime prefixes from per-game prefixes. Future runtime prefixes (e.g., GOG Galaxy) can use the same convention.

Three components do the work:
1. **`wine/steam_runtime.rs`** — pure-ish helpers: bootstrap, install, scan, is_installed, runtime path. No IPC.
2. **`commands/steam_runtime.rs`** — IPC commands the frontend calls; orchestrates `steam_runtime` helpers and emits progress events.
3. **`runner.rs`** — extended with a `match game.source` branch so `SteamWine` games launch via the shared runtime prefix and `Steam.exe -applaunch`.

## Components

### `wine/steam_runtime.rs` (new)

Public surface:

```rust
pub fn runtime_prefix_path(data_path: &Path) -> PathBuf;   // <data>/prefixes/_steam_runtime
pub fn steam_exe_path(data_path: &Path) -> PathBuf;        // .../Steam.exe
pub fn is_installed(data_path: &Path) -> bool;             // Steam.exe exists
pub fn cached_installer_path(data_path: &Path) -> PathBuf; // <data>/cache/SteamSetup.exe

pub async fn bootstrap_prefix(
    data_path: &Path,
    wine_binary: &Path,
    cancelled: Arc<AtomicBool>,
    mut emit_phase: impl FnMut(SteamInstallPhase),
) -> Result<(), String>;

pub async fn run_install(
    data_path: &Path,
    cancelled: Arc<AtomicBool>,
    emit_phase: impl FnMut(SteamInstallPhase),
) -> Result<(), String>;

pub fn scan_wine_steam(
    data_path: &Path,
    compat_db: &CompatDatabase,
) -> Result<Vec<Game>, String>;
```

`SteamInstallPhase` (serde-tagged for IPC):

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SteamInstallPhase {
    InitializingPrefix,
    InstallingMono,
    InstallingGecko,
    ConfiguringPrefix,
    DownloadingInstaller { bytes_done: u64, bytes_total: u64 },
    LaunchingInstaller,        // user sees Steam GUI now
    Done,
    Failed { error: String },
}
```

### Bootstrap sequence (`bootstrap_prefix`)

All steps run via `wine_command(wine_binary)` with `WINEPREFIX=<runtime_prefix>` and `WINEARCH=win64`. Each step emits its phase before starting.

1. **`InitializingPrefix`**: `wineboot --init`. Reuses existing `prefix.rs::create_prefix` logic conceptually but we do not call that directly because it is per-game tailored; instead duplicate the small block here. ~10–30 s.
2. **`InstallingMono`**: `wineboot --update` triggers Wine's auto-download of Wine Mono. We pre-set `WINEDEBUG=-all` to suppress noise. ~30 s, ~50 MB.
3. **`InstallingGecko`**: same pattern for Wine Gecko. ~20 s, ~40 MB.
4. **`ConfiguringPrefix`**: `wine reg add` calls to set:
   - `HKEY_CURRENT_USER\Software\Wine\DllOverrides`: a small set of `vcrun*` overrides Steam needs. Exact list pinned during implementation; if Steam works without we drop this step.
   - Optional: set Windows version to 10 via `winecfg`. Pinned during implementation.

Failures during 1–4 mean we wipe `_steam_runtime` and surface `Failed`. The bootstrap is idempotent: if `runtime_prefix_path/system.reg` exists, steps 1–4 skip and return Ok immediately.

### Install sequence (`run_install`)

Composes:

1. Acquire `install_running` flag (return early if already true).
2. Reset `install_cancel` flag.
3. `bootstrap_prefix(...)` — phases 1–4 above.
4. **`DownloadingInstaller`**: stream `https://cdn.cloudflare.steamstatic.com/client/installer/SteamSetup.exe` to `cache/SteamSetup.exe.partial`, verify size > 1 MB (no SHA — Valve does not publish one for the installer), promote to `cache/SteamSetup.exe`. Skip if already present and size sane.
5. **`LaunchingInstaller`**: spawn `wine64 cache/SteamSetup.exe` with full GPTK env. Do **not** pass `/S` silent flag — let the user see Steam's installer (familiar, Steam-branded, expected). `cmd.spawn()` returns; we `wait()` on the child in a background tokio task.
6. On installer exit:
   - exit 0 + `Steam.exe` present → `Done`, persist `Settings.steam_runtime_installed = true`, save settings.
   - non-zero or `Steam.exe` missing → `Failed`. Do not wipe prefix on user-cancelled installer (sometimes they want to restart). Subsequent `run_install` is idempotent and resumes.
7. Release `install_running` flag.

### Scan (`scan_wine_steam`)

```rust
pub fn scan_wine_steam(data_path: &Path, compat_db: &CompatDatabase) -> Result<Vec<Game>, String> {
    let prefix = runtime_prefix_path(data_path);
    let steamapps = prefix.join("drive_c/Program Files (x86)/Steam/steamapps");
    if !steamapps.exists() { return Ok(vec![]); }

    // Reuse the existing VDF parser from steam::parser
    let library_folders = parse_library_folders(&steamapps.join("libraryfolders.vdf"))?;
    let mut games = vec![];
    for folder in library_folders.iter().chain(once(&steamapps)) {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.file_name().and_then(|n| n.to_str())
                .map(|n| n.starts_with("appmanifest_") && n.ends_with(".acf"))
                .unwrap_or(false)
            {
                if let Ok(manifest) = parse_appmanifest(&path) {
                    games.push(Game {
                        id: format!("steam_wine_{}", manifest.appid),
                        name: manifest.name,
                        source: GameSource::SteamWine,
                        status: GameStatus::Unknown,  // overridden by compat_db below
                        install_dir: folder.join("common").join(&manifest.installdir),
                        executable: None,  // resolved at launch time inside the prefix
                        size_bytes: manifest.size_on_disk,
                        is_running: false,
                        notes: None,
                    });
                }
            }
        }
    }
    apply_compat_data(&mut games, compat_db);
    Ok(games)
}
```

Two helpers — `parse_library_folders` and `parse_appmanifest` — are added in `steam::parser`. Both reuse the existing `parse_vdf` lexer/tokenizer, so we avoid duplicating VDF logic.

### Integration with `commands::games::scan_steam`

Existing `scan_steam` IPC command currently scans only macOS Steam. After this design:

```rust
let mut games = scan_steam_macos(&settings.steam_path)?;
if steam_runtime::is_installed(&settings.data_path) {
    games.extend(steam_runtime::scan_wine_steam(&settings.data_path, &state.compat_db)?);
}
apply_compat_data(&mut games, &state.compat_db);
// merge with existing manual games as today
```

Frontend continues to call `scan_steam` and gets the unified list. No new IPC for scan.

### `runner.rs::launch_game` extension

```rust
pub fn launch_game(game: &Game, data_path: &Path, compat_db: &CompatDatabase) -> Result<Child, String> {
    match game.source {
        GameSource::SteamWine => launch_via_steam_runtime(game, data_path, compat_db),
        _ => launch_direct(game, data_path, compat_db),  // existing logic
    }
}

fn launch_via_steam_runtime(game: &Game, data_path: &Path, compat_db: &CompatDatabase) -> Result<Child, String> {
    let appid = game.id.strip_prefix("steam_wine_")
        .ok_or_else(|| format!("invalid SteamWine game id: {}", game.id))?;
    let wine_binary = bundled::find_wine_binary(data_path)?;
    let prefix = steam_runtime::runtime_prefix_path(data_path);
    let steam_exe = steam_runtime::steam_exe_path(data_path);

    if !steam_exe.exists() {
        return Err("Steam runtime not installed. Click Install Steam in the sidebar.".into());
    }

    let compat = lookup_game(compat_db, appid);
    let env_map = build_launch_env(
        &wine_binary, &prefix, compat,
        bundled::gptk_lib_path(data_path).as_deref(),
    );

    let log_file = open_log_for(&data_path, &game.id)?;
    let mut cmd = wine_command(&wine_binary);
    cmd.arg(&steam_exe).args(["-applaunch", appid, "-silent"]);
    cmd.current_dir(&prefix);
    cmd.stdout(Stdio::from(log_file.try_clone()?))
       .stderr(Stdio::from(log_file));
    cmd.env_clear();
    for (k, v) in &env_map { cmd.env(k, v); }
    cmd.spawn().map_err(|e| format!("spawn Steam.exe -applaunch: {e}"))
}
```

The shared prefix means `WINEPREFIX` differs from per-game launches; `build_launch_env` requires no signature change because it already takes `prefix_path: &Path`.

### ProcessMonitor adaptation

`Steam.exe -applaunch` exits in seconds; the actual game becomes a child of Steam-Windows. The existing `ProcessMonitor` tracks the spawned `Child` directly, which would mark the game as `not_running` immediately after the `-applaunch` Steam wrapper exits.

Solution: extend `ProcessMonitor` with a `track_steam_wine` variant that records `(game_id, install_dir)` instead of `(game_id, Child)`. The poll loop calls `pgrep -f <install_dir>` to determine liveness:

```rust
fn check_steam_wine_running(install_dir: &Path) -> bool {
    Command::new("/usr/bin/pgrep")
        .arg("-f")
        .arg(install_dir.to_string_lossy().as_ref())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

`get_running_games` IPC now does both checks — Child-based for Manual / macOS-Steam, install_dir-based for SteamWine — and returns the merged list of running game IDs. Frontend behaviour is unchanged.

**Steam runtime itself** (the long-lived `Steam.exe` from Open Steam) is tracked the same way under a sentinel id `_steam_runtime`. The Sidebar Steam item polls this id to alternate Open Steam ↔ Running.

### Frontend: SidebarSteam component

A new component `src/components/SidebarSteam.tsx` rendered inside `Sidebar` between "Library" and "Sources" sections. State machine driven by:

- `Settings.steam_runtime_installed` (loaded once on mount + after install events)
- A `steam-install-progress` event subscription (during install)
- A `_steam_runtime` running flag from `getRunningGames`

Visual states:

| Installed? | Installing? | Running? | Render |
|---|---|---|---|
| ❌ | ❌ | — | `[Install Steam]` button (primary) |
| ❌ | ✅ | — | spinner + phase label ("Setting up Wine...", "Installing Steam...") + Cancel link |
| ✅ | ❌ | ❌ | `[Open Steam]` button (primary) |
| ✅ | ❌ | ✅ | filled state "Steam running" + secondary `[Stop]` link |

A small text under the button shows installed game count once Steam is installed: "12 games installed". Clicking the count is a no-op for now (could open a filtered view in a later iteration).

### Frontend: Library Source filter

`Sidebar.tsx`'s `SourceFilter` union gains `"steam_wine"`. NavItem labels: All / Steam / Steam (Windows) / Manual.

`StatusBadge.tsx` (or its visual sibling on `GameCard.tsx`) renders a small icon/tag distinguishing the four sources. SteamWine cards visually adjacent to the existing Steam ones — no separate section needed.

### Frontend: Settings page additions

A new section near the GPTK status block:

```
Steam Runtime
─────────────
✓ Steam installed (12 games)
[ Reset Steam ]   [ Reinstall ]
```

`Reset Steam` confirms via `window.confirm("Delete the Steam runtime and all installed Steam-Windows games?")` then calls a new IPC `reset_steam_runtime` that:

1. Stops Steam-Windows if running (`process_monitor.stop("_steam_runtime")`).
2. Waits up to 5 s for the process to exit, force-kills if still alive.
3. Wipes `<data>/prefixes/_steam_runtime`.
4. Sets `Settings.steam_runtime_installed = false` and persists.

`Reinstall` is a quicker alternative — only wipes `<data>/cache/SteamSetup.exe` and re-runs `run_install` (prefix preserved).

### IPC commands (new)

```rust
#[tauri::command]
pub async fn start_steam_install(window: Window, state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
pub fn cancel_steam_install(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
pub fn launch_steam_runtime(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
pub fn stop_steam_runtime(state: State<'_, AppState>) -> Result<(), String>;

#[tauri::command]
pub fn reset_steam_runtime(state: State<'_, AppState>) -> Result<(), String>;
```

`start_steam_install` emits `steam-install-progress` events. `launch_steam_runtime` spawns `Steam.exe` (no `-applaunch`) into the shared prefix and registers it under id `_steam_runtime` in ProcessMonitor.

### AppState extensions

```rust
pub struct AppState {
    // ... existing fields ...
    pub steam_install_cancel: Arc<AtomicBool>,
    pub steam_installing: Arc<AtomicBool>,  // re-entrancy guard
}
```

### Settings model extension

```rust
pub struct Settings {
    // ... existing ...
    #[serde(default)]
    pub steam_runtime_installed: bool,
}
```

`#[serde(default)]` keeps backward compatibility with existing `settings.json` files.

### `GameSource` enum extension

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GameSource {
    Steam,
    SteamWine,   // new
    Manual,
}
```

The existing attribute is `rename_all = "lowercase"`. Switching to `"snake_case"` is wire-compatible for `Steam` and `Manual` (both serialise identically under both rules) and produces the desired `"steam_wine"` for the new variant. Frontend `types.ts` mirrors as `"steam" | "steam_wine" | "manual"`.

### File watcher (lib.rs)

Add a third watcher mirroring the Steam macOS one but pointing at the runtime prefix's steamapps folder:

```rust
fn setup_steam_runtime_watcher(app_handle: tauri::AppHandle, data_path: PathBuf) {
    let watch_path = runtime_prefix_path(&data_path)
        .join("drive_c/Program Files (x86)/Steam/steamapps");
    // notify::recommended_watcher on Create | Remove → emit "steam-library-changed"
}
```

The frontend already listens to `steam-library-changed` and refreshes the Library, so no UI change is needed.

The watcher only attaches when `is_installed(data_path)` is true; otherwise the path may not exist yet. After a successful install we emit a `steam-installed` event; the main process listens and lazily attaches the watcher then.

## Data flow

```
Click Install Steam:
  start_steam_install (IPC)
    → bootstrap_prefix → emit InitializingPrefix → InstallingMono → InstallingGecko → ConfiguringPrefix
    → download SteamSetup.exe → emit Downloading
    → spawn wine64 SteamSetup.exe → emit LaunchingInstaller
    → user clicks through Steam installer GUI
    → installer exits 0 + Steam.exe present
    → persist steam_runtime_installed=true, emit Done, attach steam_runtime_watcher

Click Open Steam:
  launch_steam_runtime → spawn wine64 Steam.exe in shared prefix
    → ProcessMonitor.track("_steam_runtime", child)
    → Steam window opens, user logs in / installs games

User installs game in Steam-Windows:
  steamapps/appmanifest_<appid>.acf created
    → file watcher fires → emit steam-library-changed
    → frontend refetches via scan_steam IPC
    → scan_wine_steam returns new SteamWine Game
    → Library card appears

Click Play on a SteamWine card:
  play_game(game_id="steam_wine_<appid>")
    → runner detects SteamWine source
    → launch_via_steam_runtime: spawn wine64 Steam.exe -applaunch <appid> -silent
    → Steam-Windows handles game launch (uses already-running Steam if alive)
    → ProcessMonitor.track_steam_wine(game_id, install_dir)
    → poll loop sees pgrep matches → is_running=true
    → game ends → pgrep no match → is_running=false
```

## Failure modes

| Failure | Detection | Recovery |
|---|---|---|
| `SteamSetup.exe` download 404 | reqwest status | UI banner + Retry; logs URL |
| `wineboot --init` fails | exit code | `Failed { error }`; suggest "Reset Steam" via Settings |
| Mono/Gecko download timeout | wineboot --update exit | log warning, continue; many games work without |
| User cancels Steam installer | exit code != 0 | reset state to "Install Steam"; prefix remains; idempotent retry |
| `Steam.exe` missing after install | post-check | `Failed { error }`; user invokes Reset Steam |
| `appmanifest_*.acf` parse error | scanner | log warning, skip that manifest, continue |
| `launch_via_steam_runtime` while Steam.exe not running | n/a | Steam-Windows is single-instance; invoking `Steam.exe -applaunch X` when Steam isn't running causes Steam to start AND launch the game. Empirically validated; if it ever fails, add explicit pre-spawn-and-wait. |
| Game's `install_dir` missing on disk | scanner | skip with warning; appmanifest may be stale |
| pgrep returns nothing for a game we just launched | poll | normal — until Steam actually starts the game (5–15 s); brief flicker acceptable |
| User clicks Install Steam twice | swap on `steam_installing` flag | second call returns Ok early without restarting |

## Versioning & updates

- `Settings.steam_runtime_installed: bool` — boolean only for now. We do not pin a Steam version because Steam-Windows self-updates via its own client, like on Windows.
- The Wine prefix itself is forward-compatible across Wine bumps; Catleap upgrades to newer `wine-catleap-*.tar.xz` reuse the same prefix.
- If Apple ships a new GPTK and the user re-imports D3DMetal libs, no prefix change is needed; new env vars take effect on next launch.

## Refactoring included

- `lib.rs::run` is large with three watchers now (Steam macOS, Volumes for GPTK, runtime Steam). Extract each into a named function. The Steam macOS watcher extraction was already planned in the GPTK spec; bundle here.
- `commands::games::scan_steam` becomes thin orchestration; rename `scan_steam_library` (the macOS one) explicitly to `scan_steam_macos` for symmetry with `scan_wine_steam`.
- `process::monitor::ProcessMonitor` gains a discriminated tracking entry (Child-tracked vs install-dir-tracked). Self-contained internal change.

No other refactors.

## Testing

Unit tests:

- `wine/steam_runtime.rs::runtime_prefix_path` — pure path construction.
- `wine/steam_runtime.rs::is_installed` — tempdir, fake `Steam.exe` present/absent.
- `wine/steam_runtime.rs::scan_wine_steam` — tempdir replicates `<prefix>/drive_c/Program Files (x86)/Steam/steamapps/` with `libraryfolders.vdf` and `appmanifest_*.acf` fixtures; asserts list of Games with `source: SteamWine`, correct ids, install_dirs.
- `steam::parser` — new helpers `parse_library_folders`, `parse_appmanifest`. Reuse existing fixtures.
- `wine/runner.rs::launch_via_steam_runtime` — does not spawn (no Wine in CI), but builds the `Command` and asserts: program is `arch`, args contain `Steam.exe`, `-applaunch`, the appid, `-silent`; env contains `WINEPREFIX=<runtime>` (not per-game).
- `process/monitor.rs::check_steam_wine_running` — abstract the pgrep call behind a trait so the test substitutes a fake "running" predicate; assert `is_running` flag flips appropriately.
- `commands/steam_runtime.rs` — async tests with `mockito` covering: SteamSetup.exe 404, partial download then promote, SHA-less length-only check, cancel mid-stream.

Manual E2E pre-release matrix:

1. Fresh: Install Steam → bootstrap silent (no Wine prompts visible to user) → installer GUI → walk through → Steam.exe present → sidebar shows Open Steam.
2. Open Steam → window opens → login → home page renders.
3. From within Steam-Windows: install a small Windows-only game (e.g., a free demo or cheap indie). Watch Catleap Library: card appears within seconds.
4. Click Play on the card → game launches → ProcessMonitor flips is_running → Stop button appears → Stop kills game. is_running clears.
5. Close Steam-Windows. Click Open Steam again → resumes session.
6. Settings → Reset Steam → confirm dialog → prefix wiped → sidebar back to Install Steam.
7. Re-install Steam after Reset → reuses cache/SteamSetup.exe (skips download) → fast bootstrap (Mono/Gecko already cached by Wine) → installer GUI → done.
8. Quit Catleap mid-bootstrap → relaunch → idempotent resume from where it left.

## Open implementation questions (for plan phase)

- Exact set of `vcrun*` overrides to apply during ConfiguringPrefix. Start empty; add as launch failures surface.
- Whether `Settings` should also persist a Steam-Windows username for display. Not a goal for MVP.
- pgrep approach is good for MVP; if it produces false positives across multiple Steam-Windows sessions on the same machine (unusual), upgrade to `lsof -p` rooted to Steam.exe's pid tree.
- Whether to bundle a `gameportingtoolkit-cmd` style wrapper script inside the prefix. Probably unnecessary because we set the env vars directly on launch.

## Milestones

1. `wine/steam_runtime.rs` skeleton + path helpers + bootstrap (passes 1–4).
2. Install pipeline (download + spawn installer + completion detection).
3. SidebarSteam component, IPC, install events end-to-end.
4. `scan_wine_steam` + parser additions; `scan_steam` aggregator merges sources.
5. `runner.rs::launch_via_steam_runtime` + ProcessMonitor pgrep variant.
6. Steam runtime watcher attached to `lib.rs`.
7. Settings page Reset Steam action.
8. Manual E2E from the matrix above.
