# MovieBox (desktop) – build notes

A Windows app around the open-source MovieBox-Tui v0.1.22 (vendored in `vendor/moviebox-tui`).

## Requirements (already installed on the build PC)
- Rust stable (MSVC) – `rustup`
- Visual Studio 2022 Build Tools, C++ workload
- Node.js 24 + npm

## Develop
```
npm install
npx tauri-plugin-libmpv-api setup-lib   # downloads libmpv-2.dll + libmpv-wrapper.dll into src-tauri/lib
powershell -ExecutionPolicy Bypass -File scripts/fetch-ffmpeg.ps1   # ffmpeg sidecar, used to join downloaded DASH streams
npm run tauri dev
```

## Test
```
cd src-tauri && cargo test --lib
npx tsc --noEmit
```

## Release
```
powershell -ExecutionPolicy Bypass -File scripts/publish-release.ps1 -Notes "What changed"   # build + sign + portable zip + GitHub release
# or, without publishing:
npm run tauri build                      # -> src-tauri/target/release/bundle/nsis/MovieBox_<version>_x64-setup.exe
powershell -File scripts/make-portable.ps1   # -> release/MovieBox_<version>_x64_portable.zip (+ copy of the installer)
```

## Layout
- `src-tauri/src/commands/*` – Tauri commands (catalog, streams, library, downloads, system)
- `src-tauri/src/core/*` – stream pool (ported from the TUI), download queue, settings, DTOs
- `src/pages/*` – Home, Search, Details, Player, Downloads, Settings, Help, My List, Continue Watching
- `src/lib/player.ts` – mpv control through `tauri-plugin-libmpv`
- `src-tauri/docs/Getting Started.html` – one-page guide opened after install
- `src-tauri/installer/hooks.nsh` – installer extras (guide shortcut)

## Notes
- User data: `%APPDATA%\MovieBox` (settings, download queue) and `%APPDATA%\moviebox-tui` (history, favorites – shared with the terminal app).
- Logs: `%LOCALAPPDATA%\com.moviebox.desktop\logs`.
- Do not call `get_property` with the `node` format from the frontend; it crashes libmpv-wrapper. Observe node properties instead.
