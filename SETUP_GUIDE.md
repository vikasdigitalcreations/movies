# Setup Guide — MovieBox

Last updated: 2026-09-16

Two audiences here: the person **building** the app (needs the dev tools), and the person **receiving** it (needs only the installer). The receiving part is at the bottom.

## Prerequisites (build machine)

| Requirement | Notes |
|---|---|
| Windows 10/11 x64 | The only supported target |
| Rust stable, MSVC toolchain | Install with `rustup-init.exe`, default host `x86_64-pc-windows-msvc` |
| Visual Studio 2022 Build Tools | C++ workload plus Windows SDK. `vs_BuildTools.exe --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`. Around 5–6 GB, needs admin |
| Node.js 24 + npm | For Vite and the Tauri CLI |
| git | Only needed to re-vendor MovieBox-Tui |
| Microsoft Edge WebView2 | Already part of Windows 10/11; the installer embeds a bootstrapper for machines without it |

## Installation

```bash
npm install
```

```bash
npx tauri-plugin-libmpv-api setup-lib
```

The second command downloads `libmpv-2.dll` (LGPL build) and `libmpv-wrapper.dll` into `src-tauri/lib/`. They are gitignored, so run it on every fresh clone — without them the app builds but cannot play anything.

The vendored crate is already in `vendor/moviebox-tui` (MovieBox-Tui v0.1.20, unmodified). To refresh it:

```bash
git clone --depth 1 --branch v0.1.20 https://github.com/mesamirh/MovieBox-Tui vendor/moviebox-tui
```

## Environment setup

Nothing to configure. There is no `.env`, no API key and no account. Settings live in `%APPDATA%\MovieBox\gui_settings.json` and are created on first run with sensible defaults.

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

10 Rust unit tests cover the stream pool (merging, ordering, quality pick, downloadable detection) and download path building and cleanup. There is no automated UI test suite; the end-to-end passes were scripted ad hoc against a running build.

## Build a release

Stop any running dev app first — it holds `src-tauri/lib/libmpv-2.dll` open and the build fails with "file in use (os error 32)".

```bash
npm run tauri build
```

```bash
powershell -File scripts/make-portable.ps1
```

Results:

- `src-tauri/target/release/bundle/nsis/MovieBox_1.0.0_x64-setup.exe` — the installer (also copied to `release/`)
- `release/MovieBox_1.0.0_x64_portable.zip` — unzip-and-run build with `lib/`, `docs/` and a "READ ME FIRST.txt"

The installer is per-user (`installMode: currentUser`), so it never asks for admin. It creates desktop and Start-menu shortcuts, a "MovieBox - Getting Started" Start-menu shortcut, and opens the guide once after a non-silent install.

## Deploy

There is no server. Send `release/MovieBox_1.0.0_x64-setup.exe` to the recipient (email, USB, cloud drive). Send the portable zip as a backup if their antivirus or policy blocks installers.

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
| Crash in libmpv-wrapper | Do not call `get_property` with the `node` format from the frontend. Observe node properties instead |
| Playback stutters on a weak connection | The player already buffers 256 MiB / 60 s ahead and offers "Switch to 720p?" after repeated stalls; lower the preferred quality in Settings to make it permanent |

## For the person receiving the app

1. Double-click `MovieBox_1.0.0_x64-setup.exe` and click through it. No admin password is needed.
2. If Windows shows a blue "Windows protected your PC" box, click **More info**, then **Run anyway**. It only happens once.
3. A MovieBox icon appears on the desktop. Double-click it.
4. A short welcome tour explains searching, opening a title, playing and downloading. It can be reopened from Help.

Nothing else needs installing — no VLC, no mpv, no codecs. To uninstall, use Windows Settings → Apps → MovieBox, which removes the program and both shortcuts.
