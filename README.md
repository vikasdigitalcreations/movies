# MovieBox

A Windows desktop app for browsing, streaming and downloading movies and series, with a built-in video player. No VLC, mpv or other software to install.

Built around the open-source terminal app [MovieBox-Tui](https://github.com/mesamirh/MovieBox-Tui) v0.1.20, which is vendored in `vendor/moviebox-tui` and reused for all catalog, stream, download, history and favorites logic. This project replaces its terminal interface with a Netflix-style desktop UI.

## What it does

- Browse a Netflix-style home screen (hero banner plus rows of posters), Movies, Series and Anime tabs.
- Search with live suggestions and an All/Movies/Series filter.
- Open a title for its description, rating, cast, genres, seasons, episodes and audio languages (dubs).
- Play in a built-in player that renders inside the app window (mpv engine, bundled).
- Download movies, episodes or whole seasons, with a queue that survives restarts.
- Keep history, resume points and a My List, shared with the MovieBox-Tui terminal app.

## Key features

| Area | Highlights |
|---|---|
| Player | Auto-hiding controls, 0.25x–8x pitch-corrected speed, subtitle and audio menus, quality switch keeping position, Up Next countdown, sleep timer, night mode (volume normalisation), mini player, in-player episode list, screenshots, full shortcut table (`?`) |
| Streaming | Picks the best quality at or below the preferred one, large read-ahead buffer for weak connections, "Switch to 720p?" after repeated stalls, "Try another source" fallback to 4KHDHub on a confident title + year match |
| Downloads | Resumable multi-segment downloads, pause/resume/cancel, 2 at a time (configurable), free-space check before a season, subtitles saved next to the video, tidy `Show\Season 01\` names, Windows notification on finish |
| App | First-run welcome tour, Help and troubleshooting page, right-click menus on posters, full keyboard navigation, Ctrl +/− zoom, offline banner, single instance, remembered window size |
| Packaging | Per-user NSIS installer (no admin), embedded WebView2 bootstrapper, desktop and Start-menu shortcuts, Getting Started guide, portable zip |

## Quick start

```bash
npm install
npx tauri-plugin-libmpv-api setup-lib
npm run tauri dev
```

Release build:

```bash
npm run tauri build
powershell -File scripts/make-portable.ps1
```

Output lands in `release/`: `MovieBox_1.0.0_x64-setup.exe` and `MovieBox_1.0.0_x64_portable.zip`. Full details in SETUP_GUIDE.md.

## For the person receiving the app

Double-click `MovieBox_1.0.0_x64-setup.exe`. The installer is not code-signed, so Windows shows "Windows protected your PC" once — click **More info → Run anyway**. A Getting Started page opens after install and explains that dialog. If the installer is blocked entirely, the portable zip runs from any folder without installing.

## Docs

- MASTERPLAN.md — vision, scope, architecture, roadmap, decisions
- PROGRESS.md — current status, what's done, what's next
- TECHNICAL.md — full stack, folder structure, data flow, key modules
- SETUP_GUIDE.md — prerequisites, build, release, troubleshooting
- API.md — every Tauri command, event and external API consumed
- CHANGELOG.md — dated change history
- BUILD.md — short build cheat sheet

## Licensing

MovieBox-Tui is MIT OR Apache-2.0. The bundled `libmpv-2.dll` is an LGPL build of mpv (zhongfly), shipped unmodified alongside the app. `tauri-plugin-libmpv` is MPL-2.0. No accounts, no telemetry.

## IDE setup

VS Code with the Tauri and rust-analyzer extensions.
