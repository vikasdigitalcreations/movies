# Technical — MovieBox

Last updated: 2026-09-21

## Stack summary

| Layer | Tech | Version |
|---|---|---|
| Shell | Tauri | 2 |
| Backend | Rust (MSVC toolchain), edition 2021 | stable |
| Vendored logic | moviebox-tui (path dependency), edition 2024, rust-version 1.90.0 | 0.1.20 |
| Frontend | React + TypeScript | 19.1 / ~6.0.3 |
| Bundler | Vite | ^8.0.16 |
| Styling | Tailwind CSS (Vite plugin) | ^4.3.3 |
| State | zustand | ^5.0.15 |
| Routing | react-router-dom (HashRouter) | ^7.18.3 |
| Icons | lucide-react | ^1.46.0 |
| Video | libmpv via tauri-plugin-libmpv | 0.3.2 |
| Muxing | ffmpeg (LGPL build, bundled as a Tauri sidecar) | latest BtbN build |
| Updates | tauri-plugin-updater against GitHub Releases | 2.11 |
| Webview | Microsoft Edge WebView2 (system) | n/a |
| Installer | NSIS via Tauri bundler, per-user | n/a |

Target platform: Windows 10/11 x64 only. Product name `MovieBox`, bundle identifier `com.moviebox.desktop`, version `1.1.1`.

## Languages

- Rust (backend, `src-tauri/src`, plus the vendored crate).
- TypeScript + TSX (frontend, `src`).
- CSS (Tailwind 4 theme tokens, `src/styles/app.css`).
- PowerShell (`scripts/make-portable.ps1`, `scripts/fetch-ffmpeg.ps1`, `scripts/publish-release.ps1`).
- NSIS script (`src-tauri/installer/hooks.nsh`).
- HTML (`index.html`, `src-tauri/docs/Getting Started.html`).

## Frameworks & libraries

### Rust (`src-tauri/Cargo.toml`)

| Package | Version | Used for |
|---|---|---|
| tauri (feature `protocol-asset`) | 2 | App shell, commands, events, window management |
| tauri-plugin-libmpv | 0.3.2 | Embeds libmpv in the window (wid mode), property and command bridge |
| tauri-plugin-single-instance | 2 | A second launch focuses the existing window instead of opening another |
| tauri-plugin-window-state | 2 | Remembers window size and position |
| tauri-plugin-notification | 2 | Windows toast when a download finishes |
| tauri-plugin-dialog | 2 | Folder picker, confirmations |
| tauri-plugin-opener | 2 | Open folders and links |
| tauri-plugin-os | 2 | Platform info |
| tauri-plugin-log | 2 | Rotating log file (2 MB, keeps 3) in the app log dir |
| tauri-plugin-updater | 2.11 | Launch-time update check, download and install, signature-verified |
| tauri-plugin-process | 2.3 | Restart after an update is installed |
| moviebox-tui | 0.1.20 (path `../vendor/moviebox-tui`) | Providers, service, download engine, history, favorites |
| serde / serde_json | 1 | DTO serialisation (camelCase across the bridge) |
| tokio | 1 (`rt-multi-thread`, `macros`, `sync`, `time`, `fs`) | Async runtime for commands and the download queue |
| reqwest | 0.12 (`rustls-tls-webpki-roots`, `stream`) | HTTP for download workers and connectivity checks |
| futures | 0.3 | Stream combinators in the download path |
| log | 0.4 | Logging facade |
| dirs | 6 | Config, download and pictures directories |
| sha2 | 0.10 | Hashing for cache keys |
| fs2 | 0.4 | Free disk space before a download |
| quick-xml | 0.42 | Reading DASH manifests in `core/dash.rs` |
| windows-sys (`Win32_System_Power`) | 0.59 | `SetThreadExecutionState` keep-awake |
| tauri-build | 2 | Build script |

Release profile: `codegen-units = 1`, `lto = "thin"`, `opt-level = 3`, `panic = "abort"`, `strip = true`.

### Frontend (`package.json`)

| Package | Version | Used for |
|---|---|---|
| react / react-dom | ^19.1.0 | UI |
| react-router-dom | ^7.18.3 | HashRouter routing |
| zustand | ^5.0.15 | App and player stores |
| lucide-react | ^1.46.0 | Icons |
| @tauri-apps/api | ^2 | `invoke`, `listen`, webview zoom |
| tauri-plugin-libmpv-api | ^0.3.2 | mpv init, properties, commands, events |
| @tauri-apps/plugin-dialog, -log, -notification, -opener, -os, -window-state | ^2.x | Plugin bindings |
| tailwindcss + @tailwindcss/vite | ^4.3.3 | Styling |
| vite + @vitejs/plugin-react | ^8.0.16 / ^6.0.2 | Dev server (port 1420) and build |
| typescript | ~6.0.3 | Type checking |
| @tauri-apps/cli | ^2 | `tauri dev` and `tauri build` |

## Dev tools & dependencies

- Rust stable via `rustup` (MSVC host).
- Visual Studio 2022 Build Tools, C++ workload plus Windows SDK (required to link).
- Node.js 24 and npm.
- `npx tauri-plugin-libmpv-api setup-lib` downloads `libmpv-2.dll` and `libmpv-wrapper.dll` into `src-tauri/lib/`. These DLLs are gitignored and must be fetched again on a fresh clone.
- NSIS is downloaded by the Tauri bundler on the first `tauri build`.

## External services / APIs

| Service | Purpose | Auth method |
|---|---|---|
| MovieBox / aoneroom API (`api*.aoneroom.com`, `api.inmoviebox.com`) | Homepage tabs, search, suggestions, details, resource pages, play info, subtitles | None. Requests are signed by the vendored crate and need the MovieBox client user agent |
| MovieBox CDN (`*.hakunaymatata.com` and peers) | DASH manifests and their video/audio segments | Per-stream `Referer`, `User-Agent` and `Cookie` headers returned with the play info |
| 4KHDHub | Second source, reached through the `greenmotors.club` mediator | None |
| Cinemeta (`v3-cinemeta.strem.io`) | Turns a title and year into an IMDb id so addons can be asked | None |
| Stremio addons | Third source, whatever the user installs | Whatever that addon requires |
| GitHub Releases (`github.com/vikasdigitalcreations/movies`) | `latest.json` update feed and the installer it points at | None; the repo is public so the app needs no token. Updates are rejected unless signed by the project key |

Since September 2026 the API answers every **direct file** link (`macdn.aoneroom.com/other/…`) with a 21-second "Update now. Keep watching." advert instead of the video — the same clip for every title, movies and episodes alike. `core/stream_pool::is_notice_url` recognises those links and drops them, so only the signed DASH manifest is used. On 2026-09-20 the substitution spread to the DASH manifests too and MovieBox stopped serving video altogether -- a survey of 17 popular titles went from 15 playable to 0 inside an hour, unchanged by a fresh token or a clean cache. Streams therefore fall back to 4KHDHub and then to the user's addons, and the app plays through the outage. The outage was still in force that evening (0 of 17 playable), and `probe chain` puts the app's overall reach at **14 of 20 popular titles**; the misses are Indian releases, which 4KHDHub does not carry. Matching a title across sources is deliberately strict on the name and loose on the year: MovieBox decorates a series title with the seasons it carries and reports the season's year rather than the series' first year, so `norm_title` strips season markers and the year is used to rank candidates instead of to exclude them. The CDN returns **HTTP 428** for browser-like user agents and 206 for curl/okhttp/libmpv-style agents, so downloads and direct playback send the MovieBox client agent (`service.client.user_agent()`) and the player falls back to `libmpv`. No user data, account or email is ever sent to these services.

## Folder structure

```
MovieBoxApp/
  src/                          React frontend
    App.tsx                     HashRouter shell, global keys, download event listeners
    main.tsx                    React entry
    pages/
      Home.tsx                  Hero and rows; one component serves Home/Movies/Series/Anime
      Search.tsx                Search box, suggestions, filter, result grid
      Details.tsx               Title page: seasons, episodes, dubs, Play/My List/Download
      Player.tsx                Full player UI drawn over the mpv layer
      Downloads.tsx             Active and Completed tabs
      Settings.tsx              Playback, downloads, library, shortcuts, about
      Help.tsx                  FAQ, shortcut tables, troubleshooting, report a problem
      Library.tsx               My List and Continue Watching pages
    components/
      Sidebar.tsx               Navigation with download badge
      Row.tsx, PosterCard.tsx   Horizontal rows and poster tiles
      Overlays.tsx              Toasts, context menu, offline banner, welcome tour
      DownloadDialog.tsx        Quality picker, size and free-space check
      ui.tsx                    Shared primitives (buttons, dialogs, skeletons)
    lib/
      api.ts                    Typed invoke wrappers and every DTO interface
      player.ts                 mpv init options, property and command helpers
      shortcuts.ts              PLAYER_SHORTCUTS and APP_SHORTCUTS tables
      hooks.ts                  Focus movement, typing detection, misc hooks
      format.ts                 Sizes, durations, speeds
    store/
      app.ts                    Settings, downloads, history, favorites, toasts, online state
      player.ts                 Player session state
    styles/app.css              Tailwind theme tokens
  src-tauri/                    Rust backend
    src/lib.rs                  Plugin registration, state setup, command registry
    src/main.rs                 Binary entry (moviebox)
    src/state.rs                AppState: service, history, favorites, settings, caches, queue
    src/bin/probe.rs            Standalone API probe used during development
    src/commands/
      catalog.rs                home, search, suggest, details, plus homepage JSON parsing
      streams.rs                streams, subtitles, fetch_subtitle, alternate_source
      library.rs                history_* and favorites_* commands
      downloads.rs              download_* commands (thin wrappers over the manager)
      system.rs                 settings, system info, free space, folders, logs, cache, keep-awake, online check
    src/core/
      stream_pool.rs            Merge/dedupe/order releases, quality pick, advert-clip filter (unit-tested)
      dash.rs                   DASH manifest parsing, resumable segment download, ffmpeg mux (unit-tested)
      downloads.rs              Persistent queue, workers, retries, cleanup, notifications (unit-tested)
      settings.rs               GuiSettings struct, load and save gui_settings.json
      types.rs                  DTOs shared with the frontend, friendly error mapping
    capabilities/default.json   30 permissions (window, webview zoom, opener, dialog, notification, libmpv, ...)
    lib/                        libmpv-2.dll, libmpv-wrapper.dll (gitignored, fetched by setup-lib)
    bin/                        ffmpeg sidecar (gitignored, fetched by scripts/fetch-ffmpeg.ps1)
    docs/Getting Started.html   One-page guide, opened after install
    installer/hooks.nsh         NSIS post-install and pre-uninstall hooks
    icons/                      App icons generated by tauri icon
    tauri.conf.json             Window, bundle, NSIS and resource configuration
  vendor/moviebox-tui/          MovieBox-Tui v0.1.22, unmodified
  scripts/make-portable.ps1     Stages the portable folder and zips it
  release/                      Built installer and portable zip (gitignored)
```

## Data flow

**Browsing:** page calls `api.*` in `lib/api.ts` → `invoke` → Rust command → `MovieBoxService` (vendored) → MovieBox API → camelCase DTO → page. Home results are cached in memory for 10 minutes per tab; details are cached for the session. Both caches are dropped by `clear_cache`.

**Playing:** Details → `streams` tries four sources in order and takes the first that answers: MovieBox, then 4KHDHub matched on title and year, then Dramachi on the same exact-title rule, then the user's Stremio addons (matched through Cinemeta to an IMDb id). MovieBox's real stream is a DASH manifest whose address is base64-encoded inside the `Edge-Cache-Cookie` it returns as `signCookie`; the same cookie goes to mpv as a header. Dramachi films arrive in parts and are joined into one mpv `edl://` timeline. The failover lives in the command rather than the player so downloads share it. MovieBox releases then go through `stream_pool`, which merges mirrors, drops duplicates and orders best-first → the page picks the best at or below the preferred quality → `Player.tsx` calls `playerInit`, then loads the URL with its headers → mpv renders into the child window while React draws the controls above it. Progress is written through `history_progress` every 10 s and on exit; at 90% or more the title is marked watched.

**Downloading:** `DownloadDialog` → `download_add` → `DownloadManager` builds the destination path, checks free space, persists the task to `downloads.json`, then a worker takes one of two paths. A direct file goes through the vendored resumable `download::download()` with the MovieBox user agent; partial files are `<dest>.part`, `<dest>.part.json` and `<dest>.part.N`, and a 401/403/404/410 triggers a fresh URL fetch and a retry. A DASH stream goes through `core/dash.rs`: the manifest is parsed, the chosen video and audio segments are appended to `<dest>.part.video` and `<dest>.part.audio` with the segment counts kept in `<dest>.part.json` (so pause and resume continue rather than restart), and the bundled ffmpeg copies both into the final file. Progress is emitted as `download://progress` on every tick, removal as `download://removed`. On completion the subtitle is saved beside the video and a Windows notification fires.

**Updating:** on launch `src/components/Updater.tsx` asks `tauri-plugin-updater` to read `latest.json` from the GitHub release. A newer version is downloaded, its signature checked against the public key in `tauri.conf.json`, installed by the NSIS installer in passive mode, and the app restarts itself. Failures are logged and ignored so a bad check never blocks watching.

**Startup:** `AppState::new` loads settings, history and favorites, then `downloads.kick()` restarts anything that was still downloading when the app last closed.

## Configuration & env vars

The app needs **no environment variables**. Everything is stored in files.

| Path | Purpose |
|---|---|
| `%APPDATA%\MovieBox\gui_settings.json` | GUI settings (quality, subtitle language and size, autoplay, seek step, download dir, volume, zoom, tour done) |
| `%APPDATA%\MovieBox\downloads.json` | Download queue, so it survives restarts |
| `%APPDATA%\moviebox-tui\history.json` | Watch history and resume points, shared with the terminal app |
| `%APPDATA%\moviebox-tui\favorites.json` | My List, shared with the terminal app |
| `%LOCALAPPDATA%\com.moviebox.desktop\logs\moviebox.log` | Rotating log (2 MB, 3 kept), opened by "Report a problem" |
| `%USERPROFILE%\Downloads\MovieBox\` | Default download root (configurable in Settings) |
| `%USERPROFILE%\Pictures\MovieBox\` | Player screenshots (Ctrl+S) |
| MovieBox-Tui cache dir | Poster and metadata cache, emptied by "Clear cache" |

| Variable | Purpose | Required |
|---|---|---|
| `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` | Development only. Set to `--remote-debugging-port=9222` before launching to drive the app over the Chrome DevTools Protocol during automated tests | No |

No secrets, keys or tokens exist in this project.

## Key modules

| File | Responsibility |
|---|---|
| `src-tauri/src/lib.rs` | Registers plugins, builds `AppState`, restarts the queue, lists all 37 commands |
| `src-tauri/src/state.rs` | Shared state: service, history and favorites mutexes, settings `RwLock`, home and details caches, download manager |
| `src-tauri/src/core/downloads.rs` | Queue persistence, worker scheduling, path building, URL refresh, cleanup of `.part*` and subtitle files, notifications |
| `src-tauri/src/core/stream_pool.rs` | `merge_releases`, `sort_best_first`, `pick_for_quality`, `is_direct_downloadable`, `is_notice_url`/`drop_notice_mirrors`, ported from the TUI request layer |
| `src-tauri/src/core/dash.rs` | `parse_manifest` (SegmentTemplate, `$Number$`/`$Time$`, timelines), resumable `download_dash`, `ffmpeg_path` and the mux step |
| `src/components/Updater.tsx` | Launch update check with a countdown, progress card, restart, and the manual check used by Settings → About |
| `src-tauri/src/core/types.rs` | Every DTO crossing the bridge plus `friendly()`, which turns provider errors into plain sentences |
| `src-tauri/src/commands/catalog.rs` | Homepage JSON walking (group types, subject detection), search, suggest and details with caching |
| `src-tauri/src/commands/streams.rs` | Stream collection per episode, the four-tier failover and its `StreamProblem` reasons, subtitle listing and fetching, the confident-match-only 4KHDHub and Dramachi resolution |
| `src-tauri/src/commands/addons.rs` | Stremio addon install/remove/toggle, the Cinemeta id bridge, and stream aggregation across enabled addons |
| `src/components/AddonsSettings.tsx` | Settings → Extra sources: paste a link, toggle, remove |
| `src-tauri/src/bin/probe.rs` | Developer probe. `survey` counts playable titles, `chain` walks the app's own failover per title and says where it stops, `dramachi` measures that provider's coverage and `dtier` runs the app's Dramachi tier on one title, `mirror` decodes a stream's CloudFront policy, `fourkplay`/`fourkmirrors` separate a dead source from a stale resolver, `addons` checks the id bridge |
| `src-tauri/src/commands/system.rs` | Settings, paths, free space, folder and log opening, cache clearing, connectivity, and a dedicated keep-awake thread (the Windows execution state is per-thread) |
| `src/lib/player.ts` | mpv initial options, observed properties, typed property and command helpers |
| `src/pages/Player.tsx` | Player behaviour: shortcuts, OSD, menus, Up Next, sleep timer, night mode, mini player, resume |
| `src/store/app.ts` | Global store and the single source of truth for settings and the download list |

## Scheduling / jobs / deployment infra

No server, no CI and no scheduled job. Releases are built locally by `scripts/publish-release.ps1`, which signs the installer with the updater key, writes `release/latest.json` and pushes both to GitHub Releases with `gh`. Installed copies read that feed on every launch, so the only manual distribution step is the very first install (`MovieBox_1.1.1_x64-setup.exe`, with the portable zip as a fallback).

The updater signing key lives at `%USERPROFILE%\.tauri\moviebox_updater.key` and is **not** in the repository. Losing it means installed copies will refuse every future update, and the only fix is reinstalling by hand.
