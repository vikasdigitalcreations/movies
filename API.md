# API — MovieBox

Last updated: 2026-09-16

## Overview

There is no HTTP server. The app's internal interface is the **Tauri command bridge**: the React frontend calls Rust commands with `invoke(name, args)` and receives JSON, and the backend pushes updates as events.

- Typed wrappers for every command: `src/lib/api.ts` (`api.home(...)`, `api.downloadAdd(...)`, ...).
- Registration: the `invoke_handler!` list in `src-tauri/src/lib.rs` (32 commands).
- **Argument names are camelCase** over the bridge; Rust snake_case parameters are converted automatically (`abs_index` is sent as `absIndex`).
- **Responses are camelCase** — every DTO is `#[serde(rename_all = "camelCase")]`.
- **Errors:** every command returns `Result<T, String>`. The rejected value is a ready-to-show sentence produced by `friendly()` in `src-tauri/src/core/types.rs` ("Can't reach the server. Please check your internet connection and try again.", "The server is busy right now...", "Sorry, this title isn't available right now."). The frontend renders it through `errText()`.
- **Auth:** none. No accounts, tokens or user identity anywhere in the app.

## Shared shapes

```ts
Card      { id, title, year?, poster?, mediaType: "movie" | "series" }
HomeRow   { title, kind: "hero" | "row", items: Card[] }
Details   { id, title, mediaType, year?, description?, imdbRating?, director?, stars?,
            poster?, duration?, genres[], seasons: [{ number, episodes: [{ number, title? }] }],
            dubs: [{ subjectId, label }], isFavorite }
Stream    { label, height, multi, size?, codec?, url, headers: [string, string][],
            resourceId?, downloadable, source }
Subtitle  { name, url }
HistoryItem { id, title, poster?, mediaType, year, season, episode,
              progress, duration?, completed, updated }
PlayRef   { id, title, poster?, mediaType, year?, season, episode }
DownloadTask { id, subjectId, title, year?, poster?, mediaType, season, episode, absIndex,
               episodeTitle?, height, url, headers, subtitleUrl?, subtitleLang?, path,
               status: "queued"|"downloading"|"paused"|"done"|"failed",
               downloaded, total?, speed, error?, added }
```

## Catalog

### `home(tab: string) -> HomeRow[]`
Homepage rows for a tab. Tab ids: `"0"` Home, `"2"` Movies, `"5"` Series, `"8"` Anime. Cached in memory for 10 minutes per tab. Group types `FILTER`, `POST_LIST`, `MUSIC_CHARTS` and `LIVE_LIST` are skipped, items without a poster are dropped, and the first banner group becomes `kind: "hero"`. Errors when the response yields no rows ("Couldn't load titles right now...").

### `search(query: string, page: number, filter: string) -> Card[]`
Paged search. `filter` is `"all"`, `"movie"` or `"series"`. An empty query returns an empty list.

### `suggest(query: string) -> string[]`
Autocomplete strings for the search box.

### `details(id: string) -> Details`
Full metadata including seasons, episodes, dub languages and whether the title is in My List. Cached per session.

## Streams

### `streams(id, season: number, episode: number, absIndex: number) -> Stream[]`
All playable releases for one movie or episode, already merged, deduplicated and ordered best-first by `core/stream_pool`. `multi: true` marks a multi-resolution DASH manifest (labelled "Auto"); `downloadable: true` marks a direct file that the download queue can take. `headers` must be passed to mpv or the download worker unchanged.

### `subtitles(id, resourceId: string, dubIds: string[], season, episode) -> Subtitle[]`
External caption tracks for a release. Language labels are sanitised for display.

### `fetch_subtitle(url: string) -> string`
Downloads one subtitle to a local file (some URLs need headers mpv cannot send) and returns the path for `sub-add`.

### `alternate_source(title, year: string | null, season, episode, preferred: number) -> Stream`
"Try another source." Searches 4KHDHub and uses a result **only** when the normalised title matches and, when a year is known, the year matches too. Otherwise it rejects with "No other source is available for this title." Timeouts: 15 s search, 20 s stream resolution.

## Library

History and favorites are stored by the vendored crate in `%APPDATA%\moviebox-tui`, shared with the MovieBox-Tui terminal app.

| Command | Signature | Purpose |
|---|---|---|
| `history_list` | `() -> HistoryItem[]` | Continue Watching, newest first |
| `history_get` | `(id, season, episode) -> HistoryItem \| null` | Resume point for one title or episode |
| `history_watched` | `(id, episodes: [number, number][]) -> boolean[]` | Watched ticks for a list of `[season, episode]` pairs |
| `history_start` | `(item: PlayRef, position: number) -> void` | Records that playback began |
| `history_progress` | `(item: PlayRef, position, duration \| null) -> boolean` | Saves progress; returns `true` when it crossed 90% and marked the item watched |
| `history_mark_watched` | `(item: PlayRef) -> void` | Marks watched without playing |
| `history_remove` | `(id) -> void` | Removes every entry for a title |
| `history_clear` | `() -> void` | Clears all history |
| `favorites_list` | `() -> Card[]` | My List, newest first |
| `favorites_toggle` | `(card: Card) -> boolean` | Toggles; returns the new state |
| `favorites_clear` | `() -> void` | Empties My List |

## Downloads

| Command | Signature | Purpose |
|---|---|---|
| `download_list` | `() -> DownloadTask[]` | The whole queue, active and finished |
| `download_add` | `(task: NewDownload) -> DownloadTask` | Queues one item; builds the destination path and checks free space |
| `download_pause` | `(id) -> void` | Stops the worker, keeps the partial file |
| `download_resume` | `(id) -> void` | Requeues a paused or failed task |
| `download_remove` | `(id, deleteFile: boolean) -> void` | Removes the task; with `deleteFile` also deletes `.part*`, the video and its subtitle, then any directories left empty |

`NewDownload` is `DownloadTask` minus the runtime fields (`id`, `path`, `status`, `downloaded`, `total`, `speed`, `error`, `added`), plus an optional `size`.

Destination naming (`build_path`):

- Movie: `MovieBox\Title (2024)\Title (2024) 1080p.mp4`
- Episode: `MovieBox\Show\Season 01\Show - S01E03 - Episode Title.mp4`

Concurrency comes from `simultaneousDownloads` (default 2). Partial state is `<dest>.part`, `<dest>.part.json` and `<dest>.part.N` segment files. A 401/403/404/410 makes the worker re-fetch a fresh URL and retry.

### Events

| Event | Payload | When |
|---|---|---|
| `download://progress` | `DownloadTask` | Any state or progress change; the store upserts by `id` |
| `download://removed` | `string` (task id) | A task was removed |

## System

| Command | Signature | Purpose |
|---|---|---|
| `settings_get` | `() -> Settings` | Reads `gui_settings.json` |
| `settings_set` | `(value: Settings) -> Settings` | Persists atomically (temp file + rename) and rekicks the queue |
| `system_info` | `() -> { version, downloadDir, logDir, screenshotDir, freeBytes? }` | Shown in Settings → About |
| `free_space_for` | `(path) -> number \| null` | Free bytes on the nearest existing ancestor of a path |
| `open_folder` | `(path) -> void` | Opens Explorer; selects the file when the path is a file |
| `open_logs` | `() -> void` | Opens the log folder ("Report a problem") |
| `clear_cache` | `() -> void` | Drops the home and details caches and deletes the MovieBox-Tui cache directory |
| `keep_awake` | `(display: boolean, system: boolean) -> void` | Prevents sleep while playing or downloading, via a dedicated thread (the Windows execution state is per-thread) |
| `check_online` | `() -> boolean` | Connectivity probe behind the offline banner |

`Settings` fields: `preferredQuality` (0 = best), `subtitleLanguage`, `autoplayNext`, `rememberSpeed`, `seekStep`, `downloadDir?`, `simultaneousDownloads`, `subtitleSize`, `subtitleBackground`, `volume`, `lastSpeed`, `nightMode`, `uiZoom`, `tourDone`.

## Player bridge (plugin, not an app command)

`src/lib/player.ts` wraps `tauri-plugin-libmpv-api`: `init` with the startup options, `setProperty` / `getProperty`, `command`, `observeProperties` and `listenEvents`. Observed properties include `pause`, `time-pos`, `duration`, `volume`, `mute`, `speed`, `paused-for-cache`, `cache-buffering-state`, `demuxer-cache-time`, `cache-speed`, `eof-reached`, `track-list`, `sid`, `aid`, `sub-delay`, `chapter-list`, `video-params`, `idle-active`.

Two constraints learned the hard way:

- Never call `get_property` with the `node` format from the frontend — it faults inside `libmpv-wrapper.dll`. Observe those properties instead.
- End of playback must be detected from the `eof-reached` property, because `keep-open=yes` suppresses the `end-file(eof)` event.

## External APIs consumed

| API | Endpoint | Purpose | Auth | Limits |
|---|---|---|---|---|
| MovieBox / aoneroom | `api*.aoneroom.com`, `api.inmoviebox.com` — homepage, search, suggest, details, resource page, play info, captions | All catalog and stream metadata | None; signed by the vendored crate, needs the MovieBox client user agent | Undocumented. Busy responses are surfaced as "The server is busy right now" |
| MovieBox CDN | `*.hakunaymatata.com` and peers — DASH `.mpd` and direct MP4 | Video delivery | Per-stream `Referer`, `User-Agent`, `Cookie` headers from the play info | Returns HTTP 428 to browser-like user agents; 206 to curl/okhttp/libmpv-style agents |
| 4KHDHub | Search and stream resolution | Fallback source only, on a confident title + year match | None | 15 s search / 20 s resolve timeouts in this app |

Nothing about the user is sent to any of these: no account, no identifier, no email, no telemetry.
