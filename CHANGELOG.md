# Changelog — MovieBox

Newest first. Dates are the day the work landed.

## 2026-09-20 — 1.1.0

### Fixed
- **Playing a film showed an advert instead of the film.** MovieBox's servers now answer every direct file link with the same 21-second "Update now. Keep watching." clip (`macdn.aoneroom.com/other/…`), for movies and episodes alike. Those links are recognised and dropped (`core/stream_pool::is_notice_url`), so playback uses the real DASH stream. Confirmed against the live API: the clip is 952 KB and 21 s long whatever title is asked for.
- **Downloads were saving that advert**, ~1 MB of it, labelled with the film's real size. The download queue now refuses notice links outright.
- **The back button bounced straight back into the video.** "Play" from the home banner or a poster's right-click menu carries a one-shot `autoplay` flag in the history entry; returning from the player re-mounted the details page, which saw the flag again and reopened the video. The flag is now cleared as soon as it is used (`src/pages/Details.tsx`).

### Added
- **Auto-update.** Every launch checks the release feed and offers the new version with a short countdown, then installs it and restarts (`src/components/Updater.tsx`, `tauri-plugin-updater`). Settings → About has a manual "Check for updates". Releases are published with `scripts/publish-release.ps1`.
- **Downloads work again, through the DASH stream.** `core/dash.rs` reads the manifest, downloads the video and audio segments (resumable — segment counts are kept in `<file>.part.json`) and the bundled ffmpeg joins them into one playable file. `scripts/fetch-ffmpeg.ps1` fetches the LGPL ffmpeg build that ships as a Tauri sidecar.
- Two network health checks that are skipped by default: `provider_health` (MovieBox still returns a real stream) and `dash_health` (the whole download path, end to end).

### Changed
- Quality list: MovieBox only grants one signed DASH manifest per title now, so the download dialog offers "Best available" rather than a list of file sizes.

## 2026-09-16

### Added
- Project documentation set: `README.md`, `MASTERPLAN.md`, `PROGRESS.md`, `TECHNICAL.md`, `SETUP_GUIDE.md`, `API.md`, `CHANGELOG.md` — so the project can be picked up without reading the code. `README.md` replaced the Vite/Tauri scaffold text.
- `CLAUDE.md` with the docs-update rule.

## 2026-09-15

### Fixed (uncommitted — not in the shipped v1.0.0 artifacts)
- History items marked watched were stored with `timestamp: 0` and therefore sorted as 1970 in Continue Watching. `to_item` now stamps the current time, because `HistoryManager::mark_watched` keeps the item's own timestamp (`src-tauri/src/commands/library.rs`).

### Fixed
- Sleep timer no longer autoplays the next episode: firing it now pauses playback and cancels Up Next, guarded by a `sleepDone` flag (`src/pages/Player.tsx`). Commit `1cfdea9`.
- Cancelling a download left `.part.0` / `.part.1` segment files and the `.srt` behind. `cleanup_files` now removes every `<name>.part*`, the matching subtitle files, and any directories left empty; covered by a unit test (`src-tauri/src/core/downloads.rs`). Commit `27e5c3f`.
- Downloads failed with HTTP 428 because a browser-like user agent was sent. Downloads and direct playback now use the MovieBox client agent, and the player falls back to `libmpv` — the CDN rejects browser agents that lack browser headers. Commit `3db7e03`.
- End of playback was never detected: `keep-open=yes` suppresses `end-file(eof)`, so Up Next and "mark watched" never ran. Now driven by the `eof-reached` property. Commit `3db7e03`.
- Seeks landed 0.4–1.8 s past the target; added `hr-seek=yes`, bringing deltas within 0.03 s. Commit `3db7e03`.
- Stale player event handlers replaced with refs, so callbacks always see current state. Commit `3db7e03`.
- Search icon alignment and horizontal overflow on the details page. Commit `bf03c6d`.

### Removed
- Empty `startMenuFolder` from the NSIS config, which produced a stray shortcut folder. Commit `3db7e03`.

### Added
- Build notes (`BUILD.md`). Commit `776f20f`.
- App icon, one-page `Getting Started.html`, NSIS installer hooks (Start-menu guide shortcut, opens the guide once after a non-silent install), bundle configuration, and `scripts/make-portable.ps1` for the portable zip. Commits `4305d59`, `bf03c6d`.
- Milestones A/B/C in one pass — backend commands, persistent download queue, the full React UI and the built-in player. Commit `18d8f78`.
- Tauri scaffold, MovieBox-Tui v0.1.20 vendored as a path dependency, and the libmpv player spike that proved video renders inside the window under a transparent WebView2 overlay. Commit `6ff82df`.

### Verified
- Installer pass 23/23 (silent per-user install, shortcuts, registry entry, streaming playback with the bundled mpv, download killed mid-flight and resumed on relaunch, cancel cleanup, silent uninstall leaving no shortcuts or files).
- Portable zip 4/4, player shortcuts 22/22, library/navigation/offline 11/11, resume 4/4, series/autoplay/sleep 10/11 then 3/3 after the sleep-timer fix, downloads 6/7 (the failure was a test-path assumption, since corrected).
- `cargo test --lib` 10/10, `tsc --noEmit` and `vite build` clean.
- Clean-machine testing in Windows Sandbox was not possible (not installed on this PC, enabling it needs admin and a reboot); substituted a silent install → verify → uninstall cycle for the current user.

### Changed
- Automated test data removed from `%APPDATA%` afterwards; the user's own MovieBox-Tui history was preserved.
