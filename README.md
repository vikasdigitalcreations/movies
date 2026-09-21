# MovieBox

A Windows desktop app for browsing, streaming and downloading movies and series, with a built-in video player. No VLC, mpv or other software to install.

Built around the open-source terminal app [MovieBox-Tui](https://github.com/mesamirh/MovieBox-Tui) v0.1.22, which is vendored in `vendor/moviebox-tui` and reused for all catalog, stream, download, history and favorites logic. This project replaces its terminal interface with a Netflix-style desktop UI.

## What it does

- Browse a Netflix-style home screen (hero banner plus rows of posters), Movies, Series and Anime tabs.
- Search with live suggestions and an All/Movies/Series filter.
- Open a title for its description, rating, cast, genres, seasons, episodes and audio languages (dubs).
- Play in a built-in player that renders inside the app window (mpv engine, bundled).
- Download movies, episodes or whole seasons, with a queue that survives restarts.
- Keep history, resume points and a My List, shared with the MovieBox-Tui terminal app.
- Update itself: every launch checks for a new version and installs it.

## Key features

| Area | Highlights |
|---|---|
| Player | Auto-hiding controls, 0.25x–8x pitch-corrected speed, subtitle and audio menus, quality switch keeping position, Up Next countdown, sleep timer, night mode (volume normalisation), mini player, in-player episode list, screenshots, full shortcut table (`?`) |
| Streaming | Four sources tried in order -- MovieBox, then 4KHDHub on a confident title + year match, then Dramachi (anime, K-dramas and cartoons, low quality), then any Stremio addons you add -- so one provider going dark does not stop playback. Picks the best quality at or below the preferred one, large read-ahead buffer for weak connections, "Switch to 720p?" after repeated stalls. MovieBox's "update the app" advert clips are recognised and never played |
| Extra sources | Settings -> Extra sources installs a Stremio addon from its link, turns it off, or removes it. Only directly playable streams are used; magnet and torrent links are dropped |
| Downloads | Resumable downloads of both direct files and DASH streams (segments fetched, then joined by the bundled ffmpeg), pause/resume/cancel, 2 at a time (configurable), free-space check before a season, subtitles saved next to the video, tidy `Show\Season 01\` names, Windows notification on finish |
| Updates | Checks the GitHub release feed on every launch, then downloads, installs and restarts. Manual check in Settings → About |
| App | First-run welcome tour, Help and troubleshooting page, right-click menus on posters, full keyboard navigation, Ctrl +/− zoom, offline banner, single instance, remembered window size |
| Packaging | Per-user NSIS installer (no admin), embedded WebView2 bootstrapper, desktop and Start-menu shortcuts, Getting Started guide, portable zip |

## Quick start

```bash
npm install
npx tauri-plugin-libmpv-api setup-lib
powershell -ExecutionPolicy Bypass -File scripts/fetch-ffmpeg.ps1
npm run tauri dev
```

Release build and publish (this is also what makes installed copies update themselves):

```bash
powershell -ExecutionPolicy Bypass -File scripts/publish-release.ps1 -Notes "What changed"
```

Output lands in `release/`: `MovieBox_1.1.1_x64-setup.exe`, `MovieBox_1.1.1_x64_portable.zip` and `latest.json`. Full details in SETUP_GUIDE.md.

## For the person receiving the app

Double-click `MovieBox_1.1.1_x64-setup.exe`. The installer is not code-signed, so Windows shows "Windows protected your PC" once — click **More info → Run anyway**. A Getting Started page opens after install and explains that dialog. If the installer is blocked entirely, the portable zip runs from any folder without installing. After this one install, new versions arrive on their own.

## Docs

- MASTERPLAN.md — vision, scope, architecture, roadmap, decisions
- PROGRESS.md — current status, what's done, what's next
- TECHNICAL.md — full stack, folder structure, data flow, key modules
- SETUP_GUIDE.md — prerequisites, build, release, troubleshooting
- API.md — every Tauri command, event and external API consumed
- CHANGELOG.md — dated change history
- BUILD.md — short build cheat sheet

## Licensing

MovieBox-Tui is MIT OR Apache-2.0. The bundled `libmpv-2.dll` is an LGPL build of mpv (zhongfly) and the bundled `ffmpeg.exe` is an LGPL build from [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds); both ship unmodified alongside the app. `tauri-plugin-libmpv` is MPL-2.0. No accounts, no telemetry.

## IDE setup

VS Code with the Tauri and rust-analyzer extensions.
