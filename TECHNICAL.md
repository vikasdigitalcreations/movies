# Technical — MovieBox

Last updated: 2026-09-16

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
| Webview | Microsoft Edge WebView2 (system) | n/a |
| Installer | NSIS via Tauri bundler, per-user | n/a |

Target platform: Windows 10/11 x64 only. Product name `MovieBox`, bundle identifier `com.moviebox.desktop`, version `1.0.0`.

## Languages

- Rust (backend, `src-tauri/src`, plus the vendored crate).
- TypeScript + TSX (frontend, `src`).
- CSS (Tailwind 4 theme tokens, `src/styles/app.css`).
- PowerShell (`scripts/make-portable.ps1`).
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
| moviebox-tui | 0.1.20 (path `../vendor/moviebox-tui`) | Providers, service, download engine, history, favorites |
| serde / serde_json | 1 | DTO serialisation (camelCase across the bridge) |
| tokio | 1 (`rt-multi-thread`, `macros`, `sync`, `time`, `fs`) | Async runtime for commands and the download queue |
| reqwest | 0.12 (`rustls-tls-webpki-roots`, `stream`) | HTTP for download workers and connectivity checks |
| futures | 0.3 | Stream combinators in the download path |
| log | 0.4 | Logging facade |
| dirs | 6 | Config, download and pictures directories |
| sha2 | 0.10 | Hashing for cache keys |
| fs2 | 0.4 | Free disk space before a download |
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
| MovieBox CDN (`*.hakunaymatata.com` and peers) | Video segments and direct MP4 files | Per-stream `Referer`, `User-Agent` and `Cookie` headers returned with the play info |
| 4KHDHub | "Try another source" fallback only | None |

The CDN returns **HTTP 428** for browser-like user agents and 206 for curl/okhttp/libmpv-style agents, so downloads and direct playback send the MovieBox client agent (`service.client.user_agent()`) and the player falls back to `libmpv`. No user data, account or email is ever sent to these services.

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
      stream_pool.rs            Merge/dedupe/order releases, quality pick, downloadable check (unit-tested)
      downloads.rs              Persistent queue, workers, retries, cleanup, notifications (unit-tested)
      settings.rs               GuiSettings struct, load and save gui_settings.json
      types.rs                  DTOs shared with the frontend, friendly error mapping
    capabilities/default.json   28 permissions (window, webview zoom, opener, dialog, notification, libmpv, ...)
    lib/                        libmpv-2.dll, libmpv-wrapper.dll (gitignored, fetched by setup-lib)
    docs/Getting Started.html   One-page guide, opened after install
    installer/hooks.nsh         NSIS post-install and pre-uninstall hooks
    icons/                      App icons generated by tauri icon
    tauri.conf.json             Window, bundle, NSIS and resource configuration
  vendor/moviebox-tui/          MovieBox-Tui v0.1.20, unmodified
  scripts/make-portable.ps1     Stages the portable folder and zips it
  release/                      Built installer and portable zip (gitignored)
```

## Data flow

**Browsing:** page calls `api.*` in `lib/api.ts` → `invoke` → Rust command → `MovieBoxService` (vendored) → MovieBox API → camelCase DTO → page. Home results are cached in memory for 10 minutes per tab; details are cached for the session. Both caches are dropped by `clear_cache`.

**Playing:** Details → `streams` collects releases for the episode → `stream_pool` merges mirrors, drops duplicates and orders best-first → the page picks the best at or below the preferred quality → `Player.tsx` calls `playerInit`, then loads the URL with its headers → mpv renders into the child window while React draws the controls above it. Progress is written through `history_progress` every 10 s and on exit; at 90% or more the title is marked watched.

**Downloading:** `DownloadDialog` → `download_add` → `DownloadManager` builds the destination path, checks free space, persists the task to `downloads.json`, then a worker runs the vendored resumable `download::download()` with the MovieBox user agent. Progress is emitted as `download://progress` on every tick, removal as `download://removed`. Partial files are `<dest>.part`, `<dest>.part.json` and `<dest>.part.N`. A 401/403/404/410 triggers a fresh URL fetch and a retry. On completion the subtitle is saved beside the video and a Windows notification fires.

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
| `src-tauri/src/lib.rs` | Registers plugins, builds `AppState`, restarts the queue, lists all 32 commands |
| `src-tauri/src/state.rs` | Shared state: service, history and favorites mutexes, settings `RwLock`, home and details caches, download manager |
| `src-tauri/src/core/downloads.rs` | Queue persistence, worker scheduling, path building, URL refresh, cleanup of `.part*` and subtitle files, notifications |
| `src-tauri/src/core/stream_pool.rs` | `merge_releases`, `sort_best_first`, `pick_for_quality`, `is_direct_downloadable`, ported from the TUI request layer |
| `src-tauri/src/core/types.rs` | Every DTO crossing the bridge plus `friendly()`, which turns provider errors into plain sentences |
| `src-tauri/src/commands/catalog.rs` | Homepage JSON walking (group types, subject detection), search, suggest and details with caching |
| `src-tauri/src/commands/streams.rs` | Stream collection per episode, subtitle listing and fetching, the confident-match-only 4KHDHub fallback |
| `src-tauri/src/commands/system.rs` | Settings, paths, free space, folder and log opening, cache clearing, connectivity, and a dedicated keep-awake thread (the Windows execution state is per-thread) |
| `src/lib/player.ts` | mpv initial options, observed properties, typed property and command helpers |
| `src/pages/Player.tsx` | Player behaviour: shortcuts, OSD, menus, Up Next, sleep timer, night mode, mini player, resume |
| `src/store/app.ts` | Global store and the single source of truth for settings and the download list |

## Scheduling / jobs / deployment infra

None. There is no server, no CI and no scheduled job. Distribution is a file handed to the recipient: `release/MovieBox_1.0.0_x64-setup.exe`, with `release/MovieBox_1.0.0_x64_portable.zip` as the fallback. Both are produced locally with the commands in SETUP_GUIDE.md.
