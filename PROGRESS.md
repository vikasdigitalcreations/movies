# Progress — MovieBox

Last updated: 2026-09-21

## Current status

v1.3.2 is built on this PC. v1.2.0 is the published release, so installed copies
elsewhere are still on a version that cannot play MovieBox.

**MovieBox is back.** It never stopped serving video. From 2026-09-20 afternoon its
play-info answer carries the "update the app" advert in `url` and the real stream only
inside `signCookie`, in a new `Edge-Cache-Cookie=urlprefix=<base64>` form. Upstream
MovieBox-Tui v0.1.22 (released 2026-09-20 18:30 UTC) decodes it; 1.3.2 vendors that.
Survey of 17 popular titles: **15 playable** (0 the evening before). `probe chain` over 20
titles: **19 with a playable source** (14 in 1.3.1); the one miss, Barbie, is a probe
artefact -- the probe skips the app's search rescue.

Streams now come from four sources in order: MovieBox, 4KHDHub, Dramachi, then the
user's Stremio addons. Dramachi is new and low quality (360p-540p) but covers anime,
K-dramas and cartoons, which had no fallback at all while MovieBox was dark.

## Done

- **Scaffold and vendoring** — Tauri 2 + React 19 + TypeScript + Vite + Tailwind 4; MovieBox-Tui vendored at `vendor/moviebox-tui` as a path dependency (v0.1.20 at first, v0.1.21 since). No `pub` edits to the vendored source were needed.
- **Player spike** — libmpv renders inside the Tauri window with the WebView2 overlay drawing controls on top. Confirmed with a native `PrintWindow` capture of the running window.
- **Backend** — 32 Tauri commands across catalog, streams, library, downloads and system; `download://progress` and `download://removed` events; 10-minute home cache, details cache, friendly error mapping.
- **Frontend** — Home / Movies / Series / Anime, Search, Details, Player, Downloads, Settings, Help, My List, Continue Watching; toasts, context menus, welcome tour, offline banner, skeletons.
- **Player** — full shortcut table, speed 0.25x–8x with `scaletempo2`, subtitle and audio menus, subtitle styling, quality switch keeping position, resume prompt with 5 s rewind, Up Next countdown, sleep timer, night mode, mini player, in-player episode list, screenshots, keep-awake.
- **Downloads** — persistent queue (`downloads.json`), resume, pause/cancel with cleanup, URL refresh on 401/403/404/410, free-space check, subtitles saved beside the video, tidy folder names, completion notification. Since 1.1.0 this also covers DASH streams (`core/dash.rs` + the bundled ffmpeg).
- **Advert clip blocked (1.1.0)** — `core/stream_pool::is_notice_url` drops MovieBox's "update the app" links before they reach the player or the queue.
- **Auto-update (1.1.0)** — launch check, countdown, install, restart; `scripts/publish-release.ps1` builds and publishes the release the app reads.
- **Packaging** — app icon, `Getting Started.html`, NSIS hooks (guide shortcut, opens once after a non-silent install), per-user install mode, embedded WebView2 bootstrapper, portable zip script, ffmpeg sidecar.
- **Verification** (2026-09-15, v1.0.0): installer 23/23, portable 4/4, player shortcuts 22/22, library/navigation/offline 11/11, resume 4/4, series/autoplay/sleep 3/3 after the fix, downloads 6/7 (a test-path assumption, since corrected).
- **Verification** (2026-09-20, v1.1.0/v1.1.1, dev app and installed build): Rust unit tests 15/15; `provider_health` and `dash_health` pass against the live API; hero "Play" opens a real 55-minute episode; the player's back button lands on the details page and stays there, and a second Back reaches Home; a 523 MB download ran, paused at 89.3 MB, resumed at 89.3 MB (not from zero), finished as a 2:28:07 MP4 carrying video and audio, and played offline from the Downloads page; the installed build reported "You're on the latest version" from Settings and then updated itself 1.1.0 -> 1.1.1 unattended.
- **Vendor upgrade to v0.1.21 (unreleased)** -- our copy was byte-identical to upstream, so a clean tree replacement. Revives 4KHDHub, whose links moved behind a `greenmotors.club` interstitial that the v0.1.20 resolver could not follow: Inception went from 0 of 7 releases to 5 of 5.
- **Three-tier failover (unreleased)** -- moved out of the player into `commands::streams::streams`, so downloads reach the same alternatives playback does. Errors distinguish MovieBox withholding a title, not carrying it, and the request failing.
- **Vendor upgrade to v0.1.22 (1.3.2)** -- clean tree replacement. Brings back MovieBox by reading its new signed-cookie DASH format, and adds the Dramachi provider.
- **Dramachi tier (1.3.2)** -- fourth source, exact-title match, film parts joined into one `edl://` timeline.
- **Stremio addons (unreleased)** -- Settings -> Extra sources; five commands; Cinemeta bridges MovieBox ids to IMDb ids. Magnet links are dropped.
- **Search rescue (unreleased)** -- a first page with no results is retried once with one extra word, which recovers titles like Barbie that MovieBox carries but will not return for the bare name.
- **Measurement tooling (unreleased)** -- `probe survey|mirror|fourkplay|fourkmirrors|rescue|addons` and the `failover_health` test, so "it stopped playing" is answered with numbers in about a minute.
- **Adults section (1.3.0, unreleased)** -- RedGifs (direct mp4, plays and downloads here) and Eporner (browsed natively, played in its own embed because its files 403 outside it), behind a PIN that also gates any addon the user locks.
- **Verification** (2026-09-20, dev app over CDP, Adults section): enabling without a PIN, searching while switched off, a 2-digit PIN and a non-numeric PIN are each refused with their own message; a wrong PIN is rejected and the right one unlocks; RedGifs returned 25 items and played an 8-second clip at 1920p in MovieBox's own player; Eporner returned 30 items and opened its embed, which Esc closes; in-section search returned 30 results; removing the PIN with the wrong PIN failed, and with the right one it cleared the PIN, switched the section off, refused further searches and removed the sidebar entry.
- **Verification** (2026-09-20, installed v1.2.0 over CDP, MovieBox dark): the update feed parses with no BOM and points at 1.2.0; `system_info` reports 1.2.0; `streams` returned four 4KHDHub releases in 10.6 s, best 2160p and downloadable; "Start over" played Inception at 2160p with the clock advancing 0:33 -> 0:48 over fifteen seconds; Back landed on the details page and stayed there, and a second Back left it.
- **Verification** (2026-09-20, dev app over CDP, MovieBox dark): Rust tests 14/14 and `tsc` clean; `addons_list` returns Cinemeta through real IPC; six addon guard paths (bad URL, dead host, subtitles-only addon, duplicate, removing Cinemeta, no stream addon) each fail with their own message and persist nothing; `streams` returned three 4KHDHub releases in 10.7 s; the resolved URL answered HTTP 206 as `video/x-matroska`; Details -> Play played Inception at 2160p with the clock advancing 0:25 -> 0:37 over twelve seconds.

## In progress

- Nothing.

## Next

1. Publish 1.3.2 (`scripts/publish-release.ps1`). Until then every installed copy
   elsewhere is on 1.2.0, which cannot read MovieBox's new stream format.
2. Close the gap between an upstream fix and installed copies. The MovieBox fix sat
   upstream for a day before anyone rebuilt. A scheduled GitHub Actions job could
   re-vendor each new upstream tag, run the tests and `probe survey`, and publish when
   they pass -- but it needs the updater key as a repository secret, which is the
   owner's decision.
3. Find and document a working HTTP-streaming addon. The addon path is verified as far
   as the id bridge and every guard, but no addon that serves direct HTTP streams was
   installed, so aggregation across a live stream addon is still untested.
4. Optional: strip `probe.exe` from the bundle (about 11 MB) -- though it is now the
   diagnostic tool, so shipping it may be worth the size.
5. Track upstream. Seven releases landed in the month to v0.1.21; a pinned vendor copy
   goes stale within days of a provider change.

## Known issues / blockers

| Issue | Detail | Workaround |
|---|---|---|
| MovieBox serves one quality | When MovieBox worked it granted a single signed DASH manifest per title, so the quality list read "Best available" rather than 1080p/720p/480p | None needed; mpv adapts within the manifest. 4KHDHub offers real 2160p/1080p choices |
| MovieBox changes its stream format without notice | 2026-09-20: the real stream moved into a signed cookie and the `url` field became an advert. Old builds play the advert or nothing | Re-vendor the newest upstream tag, rebuild, publish. `probe survey` tells within a minute whether a new format has landed |
| Dramachi is low quality | 360p-540p, original-language audio, no separate subtitles, films split into parts | Last built-in tier only; parts are joined for playback, not offered for download |
| Addon streaming unproven end to end | The id bridge, install guards and aggregation call are verified, but no live HTTP-streaming addon was installed | Torrent-only addons resolve to nothing playable by design |
| Some titles have no stream | e.g. Dune: Part Two returns an empty stream list from MovieBox | The app says so plainly; nothing to play |
| SmartScreen warning | The installer is unsigned, so Windows shows "Windows protected your PC" on first run | Documented in `Getting Started.html`: More info → Run anyway. Portable zip as backup |
| libmpv-wrapper crash | Calling `get_property` with the `node` format from the frontend causes an access violation | Observe node properties instead of polling them |
| Clean-machine test not run | Windows Sandbox is not installed on this PC and enabling it needs admin plus a reboot | Substituted a silent per-user install → verify → uninstall cycle on this machine |
| Dev app locks the DLL | A running dev build holds `src-tauri/lib/libmpv-2.dll`, so Rust builds fail with "file in use (os error 32)" | Stop the dev app before building |
| Single app identifier | The dev build and an installed build share `com.moviebox.desktop`, so single-instance hands over to whichever is running | Stop the dev app before testing an installed build |
| Updater needs a public repo | The app fetches `latest.json` with no credentials, so the releases have to be publicly readable | `vikasdigitalcreations/movies` is public for this reason |
