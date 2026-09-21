# Masterplan — MovieBox

Last updated: 2026-09-21

## Vision / problem solved

`F:\DC\Movies\Movies.exe` is a compiled copy of MovieBox-Tui v0.1.20: a Rust **terminal** app driven by typed commands (`/browse`, `/history`, `/settings`) that also requires mpv or VLC to be installed separately. That is unusable for a non-technical person.

MovieBox keeps all of that app's working logic and replaces the terminal with a normal Windows application: a dark, Netflix-style interface, a player built into the window, a Downloads page, and a single Setup file to send to someone else. The goal is that the recipient installs one file, double-clicks a desktop icon, and starts watching without asking anyone for help.

## Goals

- One installer, nothing else to install (no mpv, no VLC, no codecs, no runtime).
- Everything reachable with a mouse, and separately with only the keyboard.
- Plain-language errors and recovery paths; never a stack trace or a raw HTTP code.
- Reuse the vendored crate rather than reimplementing providers, so upstream fixes stay portable.
- Original `F:\DC\Movies\Movies.exe` stays untouched and keeps working.

## Non-goals

- Live TV, BDIX and Addons from the upstream TUI — deliberately not exposed.
- Accounts, sync, telemetry or analytics of any kind.
- macOS and Linux builds. Windows 10/11 x64 only.
- Code signing (a certificate costs money), so a one-time SmartScreen prompt is expected and documented instead of hidden.

## Scope

**In:** Home/Movies/Series/Anime browsing, search with suggestions, details with seasons and dubs, streaming playback with subtitles and audio tracks, downloads (single, episode, season) with a persistent queue, history/resume/My List shared with the TUI, settings, help and onboarding, NSIS installer plus portable zip.

**Out:** anything listed under non-goals, plus a stream picker on the main Play path — quality selection is automatic unless the user opens "Choose quality".

## Architecture overview

```
+---------------------------------------------------------------+
|  Tauri window (transparent)                                    |
|                                                                |
|   mpv child window  (libmpv, wid embedding, renders video)     |
|   +--------------------------------------------------------+  |
|   |  WebView2 layer (React UI, draws controls over video)   |  |
|   +--------------------------------------------------------+  |
+---------------------------------------------------------------+
            |  invoke(command) / listen(event)
            v
+---------------------------------------------------------------+
|  Rust backend (src-tauri)                                      |
|   commands/  catalog · streams · library · downloads · system  |
|   core/      stream_pool · downloads (queue) · settings · types|
|   state.rs   AppState: service, history, favorites, caches     |
+---------------------------------------------------------------+
            |  path dependency
            v
+---------------------------------------------------------------+
|  vendor/moviebox-tui v0.1.22 (MovieBoxService, providers,      |
|  download, history, favorites)  ->  MovieBox + 4KHDHub APIs    |
+---------------------------------------------------------------+
```

Video and UI are two stacked native layers: mpv draws into a child window, and the WebView2 layer above it is transparent except for the controls. That is why `tauri.conf.json` sets `"transparent": true`, and why screenshots of the window need `PrintWindow(hwnd, hdc, 2)` rather than a GDI screen copy.

## Roadmap / phases

| Phase | Deliverables | Status |
|---|---|---|
| 0 — Dev tools | Rust (MSVC), VS 2022 C++ Build Tools on the build PC | Done |
| 1 — Player spike | Prove libmpv renders inside the Tauri window with a web overlay before any UI exists | Done |
| 2 — Vendor + port | Clone MovieBox-Tui v0.1.20, add as path dependency, port stream pool with unit tests | Done |
| A — "It works" | Home, Search, Details, Play, history and resume | Done |
| B — "It's complete" | Downloads queue, My List, Settings, full shortcut table, Up Next, subtitle/audio menus | Done |
| C — "It's polished" | Welcome tour, Help/troubleshooting, context menus, keyboard navigation, offline banner, mini player, sleep timer, night mode, notifications | Done |
| D — Package | Icon, Getting Started guide, NSIS hooks, installer, portable zip | Done |
| E — Verify | Player, library, downloads, series, installer and portable test passes | Done |
| F — Ship | Rebuild with the pending history-timestamp fix, retest, hand over the installer | Done (v1.0.0, 2026-09-15) |
| G — Keep it working | Block MovieBox's "update the app" advert clips, download DASH streams with a bundled ffmpeg, auto-update from GitHub Releases | Code done and tested in the dev app; the 1.1.0 release build is next |

## Key decisions

| Decision | Reason | Date |
|---|---|---|
| Tauri 2 rather than Electron | ~37 MB installer instead of ~150 MB, native Rust backend that can call the vendored crate directly, WebView2 already present on Windows 10/11 | 2026-09-15 |
| libmpv embedded in the window, not HTML `<video>` | Sources are often MKV / HEVC / AC3 / EAC3, which WebView2 cannot decode; mpv plays everything | 2026-09-15 |
| Prove the player before building any UI | Embedding was the single highest-risk piece; a fallback (`tauri-plugin-mpv`, mpv.exe over IPC) was ready if it failed. It worked, so the fallback was never used | 2026-09-15 |
| Vendor MovieBox-Tui as a path dependency | Upstream API/provider fixes can be pulled in by re-vendoring; no provider logic is duplicated here. `fetch_resource_page` and `moviebox_resource_item_to_release` were already public, so no source edits were needed | 2026-09-15 |
| Share the TUI's config directory (`%APPDATA%\moviebox-tui`) | History and My List stay in sync with the original terminal app | 2026-09-15 |
| MovieBox client user agent for downloads and direct playback | The CDN answers HTTP 428 to browser-like user agents; curl/okhttp/libmpv-style agents get 206 | 2026-09-15 |
| 4KHDHub fallback only on normalised title **and** year | Its IDs differ from MovieBox's, so a loose match would silently play the wrong film. Without a confident match the app says "No other source is available" | 2026-09-15 |
| Per-user NSIS install, no admin | The recipient may not have an administrator account; also avoids UAC on first run | 2026-09-15 |
| Ship unsigned and explain SmartScreen | Certificates cost money. A Getting Started page shows the exact dialog and the "More info → Run anyway" path, and a portable zip is the backup | 2026-09-15 |
| Persist the download queue to `downloads.json` | A crash or reboot mid-download resumes instead of restarting; verified by killing the app mid-download | 2026-09-15 |
| Drop MovieBox's advert links instead of playing them | The API replaced every direct file with a 21-second "Update now. Keep watching." clip. Filtering them leaves only the signed DASH manifest, which is the real film. The alternative — showing the advert — looks like the app is broken | 2026-09-20 |
| Bundle ffmpeg (LGPL) to download DASH | With direct files gone, a download means fetching video and audio segments separately and joining them. ffmpeg does that reliably; the alternative was writing a muxer or dropping the Downloads feature. Costs about 50 MB of installer | 2026-09-20 |
| Fetch segments ourselves, mux at the end | Letting ffmpeg pull the manifest would have been less code but not resumable. Downloading segment by segment keeps pause/resume and honest progress for multi-GB files | 2026-09-20 |
| Auto-update from public GitHub Releases | The provider will break again; without self-update every fix means re-sending an installer. The app fetches `latest.json` with no credentials, so the repository is public and updates are signature-checked | 2026-09-20 |
