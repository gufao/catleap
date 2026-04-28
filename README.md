# Catleap 🐱

Free, open-source macOS game launcher for running Windows games via Apple's Game Porting Toolkit. Click Play, your game runs.

> **Status:** alpha. The first-run installer downloads a custom Wine build from this repo's GitHub Releases. No Homebrew required for end users.

## Features

- **Zero Homebrew** — Catleap downloads its own Wine on first run; users never touch `brew`.
- **Steam library detection** — automatically scans your installed Steam games.
- **GPTK D3DMetal integration** — mount Apple's GPTK DMG once, Catleap copies the libraries into its data dir and wires them into every game launch (DirectX 11/12 → Metal).
- **Compatibility database** — curated env vars and DLL overrides for popular games.
- **Manual game support** — add any Windows `.exe` outside Steam.
- **Isolated Wine prefixes** — per-game, no cross-contamination.

## Requirements

- macOS 14 (Sonoma) or newer
- Apple Silicon (M1 or later)
- ~500 MB free disk for the Wine binary (downloaded automatically)
- Free Apple Developer account (sign in at developer.apple.com) to download the GPTK DMG once

## How it works

```
~/Library/Application Support/Catleap/
├── wine/                  ← downloaded from GitHub Releases on first run
│   ├── bin/wine64
│   └── lib/wine/...
├── gptk/lib/              ← copied from your mounted Apple GPTK DMG
│   ├── D3DMetal.framework/
│   └── external/*.dylib
└── prefixes/<game_id>/    ← created on demand at game launch
```

At launch, Catleap sets `DYLD_FALLBACK_LIBRARY_PATH` to the GPTK libs and runs `wine64` under `arch -x86_64`. The Wine binary is compiled from Apple's official GPTK Wine sources (CodeWeavers 22.1.1 + Apple's patch) — no third-party Wine forks.

## First run

1. Install Catleap (release coming; for now, [build from source](#development)).
2. Click **Continue** through the welcome screen.
3. Wait ~1 minute for Catleap to download Wine (~150 MB).
4. Open [developer.apple.com/games/game-porting-toolkit/](https://developer.apple.com/games/game-porting-toolkit/), sign in with your Apple ID, download the GPTK DMG.
5. Mount the DMG. Catleap detects it automatically and imports the D3DMetal libraries (~20 MB).
6. Catleap scans your Steam library. Done.

If you skip step 4–5, games still launch but without Apple's DirectX-to-Metal translation — performance will be limited. You can import GPTK later from Settings.

## Development

```sh
pnpm install
pnpm tauri dev      # run with hot reload
pnpm tauri build    # produce a distributable .app
```

The frontend is React + TypeScript + Tailwind v4. The backend is Rust + Tauri v2. Run `cargo test --lib` from `src-tauri/` for the backend test suite.

## Building the Wine artifact

The `wine-catleap-<version>.tar.xz` artifact that the first-run installer downloads is built by the [`Build Wine`](.github/workflows/build-wine.yml) GitHub Actions workflow on a native Intel runner (`macos-15-intel`).

**To cut a new Wine release:**

```sh
gh workflow run build-wine.yml -f version=1.0.1
```

…or push a tag matching `wine-catleap-v*`:

```sh
git tag wine-catleap-v1.0.1
git push origin wine-catleap-v1.0.1
```

The workflow takes ~60 minutes. When it completes:
- A GitHub Release is published with the `.tar.xz` and `.sha256` attached.
- A PR is opened updating `WINE_EXPECTED_VERSION`, `WINE_RELEASE_URL`, and `WINE_EXPECTED_SHA256` in `src-tauri/src/wine/installer.rs`. Merge it to ship the new build.

**Building locally (fallback)** — see [`tools/build-wine/README.md`](tools/build-wine/README.md). Requires Intel Homebrew at `/usr/local/` and Rosetta 2.

## Architecture

- `src-tauri/src/wine/installer.rs` — streaming download, SHA256 verify, tar.xz extract, ad-hoc codesign, atomic staging promote.
- `src-tauri/src/wine/gptk_import.rs` — `/Volumes` watcher, GPTK DMG version parsing, `ditto`-based copy with stage-and-promote.
- `src-tauri/src/wine/runner.rs` + `prefix.rs` — game launch with `arch -x86_64`, GPTK env vars when D3DMetal libs are present.
- `src-tauri/src/wine/bundled.rs` — Wine binary discovery (bundled → CrossOver → PATH).
- `src-tauri/src/commands/onboarding.rs` — IPC commands consumed by the FirstRun state machine.
- `src/pages/FirstRun.tsx` — five-step state machine: welcome → wine → gptk → scan → done.
- `tools/build-wine/build.sh` — manual offline build pipeline (also driven by CI).

## Acknowledgements

- Apple — for the [Game Porting Toolkit](https://developer.apple.com/games/game-porting-toolkit/) (D3DMetal, the patched Wine, the original toolchain).
- CodeWeavers — for the upstream Wine sources Apple builds on top of.
- [gcenx/game-porting-toolkit](https://github.com/Gcenx/game-porting-toolkit) — Catleap's bundled `wine64` is a repackaging of gcenx's actively-maintained build of the same Apple/CodeWeavers sources, kept compiling on current macOS toolchains.

## License

[MIT](LICENSE)
