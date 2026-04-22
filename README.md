# Catleap

Free, open-source macOS game launcher. Play Windows games on your Mac.

## Features
- Zero configuration — click Play, game runs
- Steam library detection — automatically finds your installed games
- Compatibility database — curated configs for popular games
- Manual game support — add any Windows .exe
- Isolated Wine prefixes — per-game, no conflicts

## Requirements
- macOS 14+ (Sonoma or later)
- Apple Silicon (M1+)
- Wine or GPTK installed (bundled in release builds)

## Development
```bash
pnpm install
pnpm tauri dev
pnpm tauri build
```

## License
MIT
