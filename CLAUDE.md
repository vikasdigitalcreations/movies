# CLAUDE.md — MovieBox

## Project

A Tauri 2 Windows app wrapping the vendored MovieBox-Tui v0.1.22 crate (`vendor/moviebox-tui`) with a React UI and an embedded libmpv player. Read `README.md`, then `TECHNICAL.md` and `API.md` before changing code.

## Ground rules

- `F:\DC\Movies\Movies.exe` (the original terminal app) must stay untouched.
- Do not edit anything under `vendor/moviebox-tui` — it is upstream source, re-vendored as a whole.
- Stop any running dev app before a Rust build; it holds `src-tauri/lib/libmpv-2.dll` open.
- New Tauri commands need three edits: the command file, the `invoke_handler!` list in `src-tauri/src/lib.rs`, and a typed wrapper in `src/lib/api.ts`.
- Never call `get_property` with the `node` format from the frontend — it faults in `libmpv-wrapper.dll`. Observe those properties instead.
- Downloads and direct playback must send the MovieBox client user agent; the CDN answers HTTP 428 to browser-like agents.
- MovieBox answers every direct file link with a 21-second "Update now" advert. Never make an `aoneroom.com/other/…` link playable again -- `core/stream_pool::is_notice_url` drops them, and only the DASH manifest is real.
- Streams come from five tiers in `commands::streams::streams`: MovieBox, then 4KHDHub, then YouTube's official channels, then Dramachi, then the user's addons. The backups are raced with `core::race`, best rank winning. Put new failover there, not in the player, so downloads get it too.
- Never end an mpv file while it is seeking. libmpv deadlocks and every later `loadfile` is accepted but never opens, until the app restarts. A resume is a seek (`start=N`). Use `mpv.stop()` and `load()` from `lib/player.ts`, which pause and wait for `seeking` to clear; never send a raw `stop` or `loadfile … replace`.
- A stream match must be strict on the name **and** the year, and a source that cannot state a year is not matched. The YouTube source matched the wrong film twice in a 28-title trial ("Don" 1978 → the 2003 Telugu film *Don Seenu*; "Zanjeer" 1973 → the 2013 remake) before it was made to require an agreeing year. Only channels listed in `core/youtube.rs::OFFICIAL_CHANNELS` count, by channel id.
- yt-dlp is fetched on demand and verified against its published SHA-256; do not bundle it, and do not run it without `kill_on_drop` and no console window.
- In `.github/workflows/auto-update.yml` the `build` job compiles upstream code and must never receive a secret; only `publish` (environment `release`, `main` only) holds `TAURI_UPDATER_KEY`.
- Addon matching is strict on normalised title and year. Loosening it plays the wrong film, which is worse than playing nothing.
- The Adults section only exists behind a PIN: it cannot be switched on without one, and clearing the PIN switches it off. Keep that invariant -- `adult_set_enabled` and `pin_clear` both enforce it.
- `settings_set` must never carry `pin_hash`, `pin_salt`, `locked_addons` or `adult_enabled`. Those belong to their own commands so the UI cannot wipe them and the hash stays in the backend.
- Adult sources are the platforms' own public APIs, never scrapers. Eporner's files answer 403 outside its embed; show the embed rather than working around it.
- Before theorising about "it stopped playing", measure it: `cargo run --bin probe -- survey` counts playable titles, and `failover_health` / `provider_health` / `dash_health` (`cargo test --lib <name> -- --ignored`) check one path each. Providers change without warning -- MovieBox went from 15 of 17 titles playable to 0 within an hour on 2026-09-20.
- The updater signing key lives at `%USERPROFILE%\.tauri\moviebox_updater.key` and must never be committed.
- No accounts, telemetry or user identifiers. Nothing about the user is sent to any service.

## Git

End every session by committing the work and pushing it to GitHub on its own branch, never straight to `main`. Tell the user the branch name; offer a PR rather than opening one unasked.

## Docs rule

After every code change, update the project docs (PROGRESS.md and CHANGELOG.md always; TECHNICAL.md, API.md, SETUP_GUIDE.md, README.md, MASTERPLAN.md when affected) using the project-docs skill.
