# Progress — MovieBox

Last updated: 2026-09-20

## Current status

v1.1.1 is built, published and installed on this PC. The release feed is live at
`https://github.com/vikasdigitalcreations/movies/releases/latest/download/latest.json`, and the
app updated itself from 1.1.0 to 1.1.1 unattended: 1.1.0 started at 00:52:36, found the new
version, installed it and relaunched as 1.1.1 at 00:53:12.

This version fixes the two things that made v1.0.0 unusable in practice — MovieBox's servers had
started answering every direct file link with a 21-second "Update now. Keep watching." advert,
which both played instead of films and downloaded instead of them — restores downloads through the
DASH stream, and adds auto-update so the next breakage can be fixed without asking anyone to
reinstall.

## Done

- **Scaffold and vendoring** — Tauri 2 + React 19 + TypeScript + Vite + Tailwind 4; MovieBox-Tui v0.1.20 vendored at `vendor/moviebox-tui` as a path dependency. No `pub` edits to the vendored source were needed.
- **Player spike** — libmpv renders inside the Tauri window with the WebView2 overlay drawing controls on top. Confirmed with a native `PrintWindow` capture of the running window.
- **Backend** — 32 Tauri commands across catalog, streams, library, downloads and system; `download://progress` and `download://removed` events; 10-minute home cache, details cache, friendly error mapping.
- **Frontend** — Home / Movies / Series / Anime, Search, Details, Player, Downloads, Settings, Help, My List, Continue Watching; toasts, context menus, welcome tour, offline banner, skeletons.
- **Player** — full shortcut table, speed 0.25x–8x with `scaletempo2`, subtitle and audio menus, subtitle styling, quality switch keeping position, resume prompt with 5 s rewind, Up Next countdown, sleep timer, night mode, mini player, in-player episode list, screenshots, keep-awake.
- **Downloads** — persistent queue (`downloads.json`), resume, pause/cancel with cleanup, URL refresh on 401/403/404/410, free-space check, subtitles saved beside the video, tidy folder names, completion notification. Since 1.1.0 this also covers DASH streams (`core/dash.rs` + the bundled ffmpeg).
- **Advert clip blocked (1.1.0)** — `core/stream_pool::is_notice_url` drops MovieBox's "update the app" links before they reach the player or the queue.
- **Auto-update (1.1.0)** — launch check, countdown, install, restart; `scripts/publish-release.ps1` builds and publishes the release the app reads.
- **Packaging** — app icon, `Getting Started.html`, NSIS hooks (guide shortcut, opens once after a non-silent install), per-user install mode, embedded WebView2 bootstrapper, portable zip script, ffmpeg sidecar.
- **Verification** (2026-09-15, v1.0.0): installer 23/23, portable 4/4, player shortcuts 22/22, library/navigation/offline 11/11, resume 4/4, series/autoplay/sleep 3/3 after the fix, downloads 6/7 (a test-path assumption, since corrected).
- **Verification** (2026-09-20, v1.1.0/v1.1.1, dev app and installed build): Rust unit tests 15/15; `provider_health` and `dash_health` pass against the live API; hero "Play" opens a real 55-minute episode; the player's back button lands on the details page and stays there, and a second Back reaches Home; a 523 MB download ran, paused at 89.3 MB, resumed at 89.3 MB (not from zero), finished as a 2:28:07 MP4 carrying video and audio, and played offline from the Downloads page; the installed build reported "You're on the latest version" from Settings and then updated itself 1.1.0 -> 1.1.1 unattended.

## In progress

- Nothing. The working tree is committed and the release is published.

## Next

1. Send `release/MovieBox_1.1.1_x64-setup.exe` to the friend once. v1.0.0 has no updater, so that
   first hop is manual; after it, new versions arrive by themselves.
2. Optional: strip `probe.exe` (a development-only API probe) from the bundle — it adds about
   11 MB to the installer for no user-facing reason.
3. Watch for the provider changing again. `provider_health` and `dash_health` answer that in
   seconds, and `scripts/publish-release.ps1` ships the fix.

## Known issues / blockers

| Issue | Detail | Workaround |
|---|---|---|
| MovieBox serves one quality | The API now grants a single signed DASH manifest per title, so the quality list is "Best available" rather than 1080p/720p/480p | None needed; mpv adapts within the manifest |
| 4KHDHub is dead | "Try another source" finds titles but every mirror it resolves reports "dead or expired" (checked 2026-09-20) | The fallback fails politely; MovieBox is the working source |
| Some titles have no stream | e.g. Dune: Part Two returns an empty stream list from MovieBox | The app says so plainly; nothing to play |
| SmartScreen warning | The installer is unsigned, so Windows shows "Windows protected your PC" on first run | Documented in `Getting Started.html`: More info → Run anyway. Portable zip as backup |
| libmpv-wrapper crash | Calling `get_property` with the `node` format from the frontend causes an access violation | Observe node properties instead of polling them |
| Clean-machine test not run | Windows Sandbox is not installed on this PC and enabling it needs admin plus a reboot | Substituted a silent per-user install → verify → uninstall cycle on this machine |
| Dev app locks the DLL | A running dev build holds `src-tauri/lib/libmpv-2.dll`, so Rust builds fail with "file in use (os error 32)" | Stop the dev app before building |
| Single app identifier | The dev build and an installed build share `com.moviebox.desktop`, so single-instance hands over to whichever is running | Stop the dev app before testing an installed build |
| Updater needs a public repo | The app fetches `latest.json` with no credentials, so the releases have to be publicly readable | `vikasdigitalcreations/movies` is public for this reason |
