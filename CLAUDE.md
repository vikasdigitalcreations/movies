# CLAUDE.md — MovieBox

## Project

A Tauri 2 Windows app wrapping the vendored MovieBox-Tui v0.1.20 crate (`vendor/moviebox-tui`) with a React UI and an embedded libmpv player. Read `README.md`, then `TECHNICAL.md` and `API.md` before changing code.

## Ground rules

- `F:\DC\Movies\Movies.exe` (the original terminal app) must stay untouched.
- Do not edit anything under `vendor/moviebox-tui` — it is upstream source, re-vendored as a whole.
- Stop any running dev app before a Rust build; it holds `src-tauri/lib/libmpv-2.dll` open.
- New Tauri commands need three edits: the command file, the `invoke_handler!` list in `src-tauri/src/lib.rs`, and a typed wrapper in `src/lib/api.ts`.
- Never call `get_property` with the `node` format from the frontend — it faults in `libmpv-wrapper.dll`. Observe those properties instead.
- Downloads and direct playback must send the MovieBox client user agent; the CDN answers HTTP 428 to browser-like agents.
- MovieBox answers every direct file link with a 21-second "Update now" advert. Never make an `aoneroom.com/other/…` link playable again — `core/stream_pool::is_notice_url` drops them, and only the DASH manifest is real.
- Before theorising about "it stopped playing", run the two live checks: `cargo test --lib provider_health -- --ignored` and the same with `dash_health`.
- The updater signing key lives at `%USERPROFILE%\.tauri\moviebox_updater.key` and must never be committed.
- No accounts, telemetry or user identifiers. Nothing about the user is sent to any service.

## Git

End every session by committing the work and pushing it to GitHub on its own branch, never straight to `main`. Tell the user the branch name; offer a PR rather than opening one unasked.

## Docs rule

After every code change, update the project docs (PROGRESS.md and CHANGELOG.md always; TECHNICAL.md, API.md, SETUP_GUIDE.md, README.md, MASTERPLAN.md when affected) using the project-docs skill.
