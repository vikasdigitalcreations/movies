# Setup Guide — MovieBox

Last updated: 2026-09-20

Two audiences here: the person **building** the app (needs the dev tools), and the person **receiving** it (needs only the installer). The receiving part is at the bottom.

## Prerequisites (build machine)

| Requirement | Notes |
|---|---|
| Windows 10/11 x64 | The only supported target |
| Rust stable, MSVC toolchain | Install with `rustup-init.exe`, default host `x86_64-pc-windows-msvc` |
| Visual Studio 2022 Build Tools | C++ workload plus Windows SDK. `vs_BuildTools.exe --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`. Around 5–6 GB, needs admin |
| Node.js 24 + npm | For Vite and the Tauri CLI |
| git + GitHub CLI (`gh`), logged in | Publishing releases, which is what makes installed copies update themselves |
| Microsoft Edge WebView2 | Already part of Windows 10/11; the installer embeds a bootstrapper for machines without it |

## Installation

```bash
npm install
```

```bash
npx tauri-plugin-libmpv-api setup-lib
```

```bash
powershell -ExecutionPolicy Bypass -File scripts/fetch-ffmpeg.ps1
```

The second command downloads `libmpv-2.dll` (LGPL build) and `libmpv-wrapper.dll` into `src-tauri/lib/`. The third downloads the LGPL ffmpeg build into `src-tauri/bin/`, which the app ships as a sidecar and uses to join downloaded DASH streams into one file. All three are gitignored, so run them on every fresh clone — without the DLLs the app builds but cannot play anything, and without ffmpeg downloads fail at the last step.

The vendored crate is already in `vendor/moviebox-tui` (MovieBox-Tui v0.1.20, unmodified). To refresh it:

```bash
git clone --depth 1 --branch v0.1.20 https://github.com/mesamirh/MovieBox-Tui vendor/moviebox-tui
```

## Environment setup

Nothing to configure for development. There is no `.env`, no API key and no account. Settings live in `%APPDATA%\MovieBox\gui_settings.json` and are created on first run with sensible defaults.

Publishing a release needs one file that is deliberately **not** in the repository: the updater signing key at `%USERPROFILE%\.tauri\moviebox_updater.key`. It was created with `npx tauri signer generate -w "$env:USERPROFILE\.tauri\moviebox_updater.key"` and its public half is in `src-tauri/tauri.conf.json`. Back it up: if it is lost, every installed copy will refuse future updates and has to be reinstalled by hand.

## Run locally

```bash
npm run tauri dev
```

Vite serves the UI on `http://localhost:1420` and Tauri opens the window. The first Rust build takes several minutes; later ones are incremental.

To drive the running app over the Chrome DevTools Protocol (how the automated test passes worked), set `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` before launching.

## Test

```bash
cd src-tauri && cargo test --lib
```

```bash
npx tsc --noEmit
```

15 Rust unit tests cover the stream pool (merging, ordering, quality pick, advert-clip filtering), DASH manifest parsing and download path building and cleanup. There is no automated UI test suite; the end-to-end passes were scripted ad hoc against a running build.

Two checks talk to the live API and are skipped by default:

```bash
cd src-tauri && cargo test --lib provider_health -- --ignored --nocapture
```

```bash
cd src-tauri && cargo test --lib dash_health -- --ignored --nocapture
```

The first says whether MovieBox still returns a real, fetchable stream; the second downloads a few segments and muxes them, proving the whole download path. Run them first whenever "it stopped playing" is reported — they tell you within seconds whether the provider changed again.

## Build a release

Stop any running dev app first — it holds `src-tauri/lib/libmpv-2.dll` open and the build fails with "file in use (os error 32)".

```bash
powershell -ExecutionPolicy Bypass -File scripts/publish-release.ps1 -Notes "What changed"
```

That one command builds the app with the updater key set, makes the portable zip, writes `latest.json` and creates (or updates) the GitHub release the app reads. To build without publishing:

```bash
npm run tauri build
```

```bash
powershell -File scripts/make-portable.ps1
```

Results:

- `src-tauri/target/release/bundle/nsis/MovieBox_1.1.1_x64-setup.exe` — the installer (also copied to `release/`)
- `release/MovieBox_1.1.1_x64_portable.zip` — unzip-and-run build with `lib/`, `docs/` and a "READ ME FIRST.txt"
- `release/latest.json` — the update feed, when publishing

The installer is per-user (`installMode: currentUser`), so it never asks for admin. It creates desktop and Start-menu shortcuts, a "MovieBox - Getting Started" Start-menu shortcut, and opens the guide once after a non-silent install.

## Deploy

There is no server. Send `release/MovieBox_1.1.1_x64-setup.exe` to the recipient once (email, USB, cloud drive), and the portable zip as a backup if their antivirus or policy blocks installers. From then on the app updates itself from GitHub Releases, so a fix only needs `scripts/publish-release.ps1`.

The update feed must stay publicly readable — the app fetches it with no credentials — which is why `vikasdigitalcreations/movies` is a public repository. A release built without the signing key will be refused by every installed copy.

To bump the version, edit `version` in both `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`, then rebuild. The portable script reads the version from `tauri.conf.json`.

## Common tasks

| Task | How |
|---|---|
| Regenerate app icons | `npx tauri icon src-tauri/app-icon.png` |
| Add a Tauri command | Write it in `src-tauri/src/commands/*`, register it in the `invoke_handler!` list in `src-tauri/src/lib.rs`, add a typed wrapper in `src/lib/api.ts` |
| Add a permission | Append to `src-tauri/capabilities/default.json` |
| Change the mpv startup options | `src/lib/player.ts`, in `playerInit` |
| Change download naming | `build_path` in `src-tauri/src/core/downloads.rs` |
| Read the app log | `%LOCALAPPDATA%\com.moviebox.desktop\logs\moviebox.log`, or Help → Report a problem |
| Test an installed build | Stop the dev app first — both share the `com.moviebox.desktop` identifier, so single-instance hands over to whichever is already running |

## Troubleshooting

| Problem | Fix |
|---|---|
| `error: linker link.exe not found` | Install the VS 2022 C++ workload and Windows SDK, then open a new terminal |
| Build fails with "file in use (os error 32)" | A dev app is running and holding `libmpv-2.dll`. Close it and rebuild |
| App window opens but nothing plays | `src-tauri/lib/` is missing the DLLs. Run `npx tauri-plugin-libmpv-api setup-lib` |
| Window is blank on the recipient's PC | WebView2 is missing. Use the installer (it embeds the bootstrapper) or install WebView2 from `https://go.microsoft.com/fwlink/p/?LinkId=2124703` |
| "Windows protected your PC" on install | Expected — the build is unsigned. More info → Run anyway. This is documented in `Getting Started.html` |
| Downloads fail with HTTP 428 | The request used a browser-like user agent. Downloads and direct playback must send the MovieBox client agent; the player falls back to `libmpv` |
| A film plays a short "Update now" advert | A link MovieBox substitutes for the real file slipped through. Check `is_notice_url` in `core/stream_pool.rs` against the URL in the log, and run `provider_health` |
| A download ends with "couldn't be put together" | ffmpeg is missing or failed. Re-run `scripts/fetch-ffmpeg.ps1`; the ffmpeg error is in the app log |
| The app never offers an update | The release has no `latest.json`, the repo is private again, or the installer was built without `TAURI_SIGNING_PRIVATE_KEY`. Open the feed URL in a browser to check |
| Crash in libmpv-wrapper | Do not call `get_property` with the `node` format from the frontend. Observe node properties instead |
| Playback stutters on a weak connection | The player already buffers 256 MiB / 60 s ahead and offers "Switch to 720p?" after repeated stalls; lower the preferred quality in Settings to make it permanent |

## For the person receiving the app

1. Double-click `MovieBox_1.1.1_x64-setup.exe` and click through it. No admin password is needed.
2. If Windows shows a blue "Windows protected your PC" box, click **More info**, then **Run anyway**. It only happens once.
3. A MovieBox icon appears on the desktop. Double-click it.
4. A short welcome tour explains searching, opening a title, playing and downloading. It can be reopened from Help.

After that, new versions install themselves: when you open MovieBox and one is waiting, a small card counts down and the app restarts into the new version.

Nothing else needs installing — no VLC, no mpv, no codecs. To uninstall, use Windows Settings → Apps → MovieBox, which removes the program and both shortcuts.
