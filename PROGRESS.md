# Progress — MovieBox

Last updated: 2026-09-16

## Current status

Feature-complete at v1.0.0 and verified end to end. `release/MovieBox_1.0.0_x64-setup.exe` (37 MB) and `release/MovieBox_1.0.0_x64_portable.zip` (44 MB) were built on 2026-09-15 and passed the installer, portable, player, library, downloads and series test passes. One bug fix (history timestamp) is written but **not committed and not in those artifacts**, so a final rebuild and retest is outstanding before hand-over.

## Done

- **Scaffold and vendoring** — Tauri 2 + React 19 + TypeScript + Vite + Tailwind 4; MovieBox-Tui v0.1.20 vendored at `vendor/moviebox-tui` as a path dependency. No `pub` edits to the vendored source were needed.
- **Player spike** — libmpv renders inside the Tauri window with the WebView2 overlay drawing controls on top. Confirmed with a native `PrintWindow` capture of the running window.
- **Backend** — 32 Tauri commands across catalog, streams, library, downloads and system; `download://progress` and `download://removed` events; 10-minute home cache, details cache, friendly error mapping.
- **Frontend** — Home / Movies / Series / Anime, Search, Details, Player, Downloads, Settings, Help, My List, Continue Watching; toasts, context menus, welcome tour, offline banner, skeletons.
- **Player** — full shortcut table, speed 0.25x–8x with `scaletempo2`, subtitle and audio menus, subtitle styling, quality switch keeping position, resume prompt with 5 s rewind, Up Next countdown, sleep timer, night mode, mini player, in-player episode list, screenshots, keep-awake.
- **Downloads** — persistent queue (`downloads.json`), multi-segment resume, pause/resume/cancel with cleanup, URL refresh on 401/403/404/410, free-space check, subtitles saved beside the video, tidy folder names, completion notification.
- **Packaging** — app icon, `Getting Started.html`, NSIS hooks (guide shortcut, opens once after a non-silent install), per-user install mode, embedded WebView2 bootstrapper, portable zip script.
- **Verification** (2026-09-15): installer 23/23 (install → play → crash mid-download → resume → uninstall), portable 4/4, player shortcuts 22/22, library/navigation/offline 11/11, resume 4/4, series/autoplay/sleep 10/11 then 3/3 after the fix, downloads 6/7 (the failure was a test-path assumption, since corrected), Rust unit tests 10/10, `tsc --noEmit` and `vite build` clean.
- **Cleanup** — automated test data removed from `%APPDATA%`; the user's own MovieBox-Tui history (Avengers: Endgame, Breaking Bad, Dhurandhar 2) preserved.

## In progress

- `src-tauri/src/commands/library.rs` — `to_item` now stamps the current time instead of `0`. Edited in the working tree, **not committed, not compiled, not shipped**. Without it, a title marked watched is stored with `timestamp: 0` and sorts as 1970 in Continue Watching.

## Next

1. `cd src-tauri && cargo test --lib` and `npx tsc --noEmit`.
2. `npm run tauri build`, then `powershell -File scripts/make-portable.ps1`.
3. Re-run the installer test pass against the new setup exe.
4. Commit the fix and tag the shipped build.
5. Hand over `release/MovieBox_1.0.0_x64-setup.exe`.

## Known issues / blockers

| Issue | Detail | Workaround |
|---|---|---|
| History timestamp | Items marked watched are saved with `timestamp: 0` in the shipped v1.0.0 artifacts | Fix written; rebuild pending (see In progress) |
| SmartScreen warning | The installer is unsigned, so Windows shows "Windows protected your PC" on first run | Documented in `Getting Started.html`: More info → Run anyway. Portable zip as backup |
| libmpv-wrapper crash | Calling `get_property` with the `node` format from the frontend causes an access violation | Observe node properties instead of polling them; noted in BUILD.md |
| Clean-machine test not run | Windows Sandbox is not installed on this PC and enabling it needs admin plus a reboot | Substituted a silent per-user install → verify → uninstall cycle on this machine |
| Dev app locks the DLL | A running dev build holds `src-tauri/lib/libmpv-2.dll`, so Rust builds fail with "file in use (os error 32)" | Stop the dev app before building |
| Single app identifier | The dev build and an installed build share `com.moviebox.desktop`, so single-instance hands over to whichever is running | Stop the dev app before testing an installed build |
