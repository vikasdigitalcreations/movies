# Changelog — MovieBox

Newest first. Dates are the day the work landed.

## 2026-09-21 — 1.3.3

### Fixed
- **The player could stop working until the app was restarted.** A resume is a seek (`start=N`), and libmpv deadlocks if the file is torn down while it is still seeking: its core thread waits for the demuxer forever, after which every `loadfile` is accepted but never begins opening. Reproduced with the bundled libmpv (2026-09-14 build) by loading a stream at `start=4217` and sending `stop` about 1.5 s later; with a `seeking` check first, the same sequence was safe across seven timings. `lib/player.ts` now pauses and waits for `seeking` to clear (8 s at most) before `stop` and before every `loadfile`, so leaving the player, switching quality and switching episode are all covered.
- **A player that had already been left could still start a video.** The 60-second "this source isn't working" timer and the retry chain belonged to the old page and could load the fallback source into the engine the next page was using. They now check that the page is still there.
- **A download refresh could be handed a film it cannot fetch.** `refresh_url` took the first result from the backup sources, which can be a film the player joins from parts. It now takes only downloadable results.

### Added
- **YouTube's official film channels as a source** (`core/youtube.rs`, `core/ytdlp.rs`, `youtube_streams`). It searches YouTube for the title, keeps only uploads from a fixed list of verified distributor channels (Goldmines, Shemaroo, Shemaroo Movies, Ultra Movie Parlour, Eros Universe, T-Series) that run 70 minutes or more, and plays them at 360p to 1080p by joining the video and audio files with mpv's `!new_stream` (`edl://`). Films only. Matching is strict on the name and on the year: an upload with no year, or with a year that disagrees, is refused, because the first version of this rule played the wrong film twice in a 28-title trial ("Don" 1978 matched the 2003 Telugu film *Don Seenu*; "Zanjeer" 1973 matched the 2013 remake). The one allowance is a Hindi dub, uploaded years after the film. The helper program, yt-dlp, is not shipped: it is fetched on first use from yt-dlp's GitHub releases, checked against the SHA-256 that release publishes, kept in `%LOCALAPPDATA%\MovieBox\tools`, and refreshed at most every three days. Settings → Extra sources → "Free full movies from YouTube" switches it off. Coverage trial, 28 popular titles: 4 found (Uppena, Dear Comrade, Mother India, Amar Akbar Anthony 1977, each checked to be the right upload), 2 of them on no other source. MovieBox lists both of those (Mother India, Dear Comrade) without a stream, so they are now playable from the app. Big releases (Vikram, KGF 2, RRR, Dangal, 3 Idiots, Inception) are not on these channels.
- **Automatic updates** (`.github/workflows/auto-update.yml`, `scripts/auto-update-docs.py`). Every four hours a cheap job compares the vendored MovieBox-Tui version with upstream's latest release. When upstream is newer, a build job re-vendors it, runs the unit tests, type-checks the UI, surveys 17 popular titles before and after, downloads and muxes a real sample through the DASH downloader, builds the installer and rehearses signing with a throwaway key. Only then does a separate publish job, the only one that holds the updater key, sign it, push the change to its own `auto/vendor-<tag>` branch and cut the release. A failure opens an issue and releases nothing. Dry run on GitHub's servers (2026-09-21): MovieBox answers from there (survey 15 of 17 before and after), the download check passed in 12.6 s, the installer built and the feed validated.
- `scripts/publish-release.ps1` takes `-Target` (the commit the release tag points at; without it GitHub tagged main's head, so the v1.3.2 tag sits on an old commit) and `-DryRun` (sign and write the feed, upload nothing).
- `probe youtube` and `probe ytcover`; `probe survey` ends with one fixed `SURVEY playable=N total=M` line for scripts.

### Changed
- **Play starts sooner.** Opening a title's page now asks MovieBox for the stream that Play will use (`api.prefetchStreams`), so Play finds it ready; the Player starts resolving the stream in parallel with starting the video engine instead of after it. Prefetch asks MovieBox alone (`primaryOnly`), so merely looking at a page never sets the heavier backup scrapers going, and an entry lives two minutes and is used once. Measured in the installed app, click to first moving frame on a fresh start: Inception 3.1 s, Oppenheimer 2.3 s, 3 Idiots 1.6 s (1.3.2: 3.6, 2.8 and 1.7 s). A modest gain: finding the stream is no longer what Play waits for. The rest is mpv opening the DASH manifest, which reads the first pieces of all three qualities in turn. Dropping the 3 s initial buffer (`cache-pause-initial`) would save only about 0.3 s more, so it stays, for smooth starts on slow connections.
- **The backup sources are asked together, not one after another** (`core/race.rs`). They start when MovieBox has failed or been silent for 5 s, and the best answer by rank wins, with a 12 s grace for a better tier when a worse one has already answered (at 4 s, the first setting tried, Oppenheimer came back as Dramachi 540p instead of 4KHDHub 2160p). A healthy MovieBox never causes a request to the others. Titles MovieBox does not carry, measured through the app's own `streams` command: Parasite 6.6 s (12.9 s in 1.3.2), Naruto 9.4 s (12.5 s), Oppenheimer 14.5 s for 4KHDHub 2160p (13.7 s), and a title no source has 6.9 s (4.2 s; the YouTube search adds a few seconds to a total miss).
- Tier order is now MovieBox, 4KHDHub, YouTube, Dramachi, then the user's addons. "Try another source" in the player uses the same set, addons included.

## 2026-09-21 — 1.3.2

### Fixed
- **MovieBox plays again.** It had not stopped serving video: since 2026-09-20 its play-info answer puts the "update the app" advert in the `url` field and hands out the real stream only through `signCookie`, in a new `Edge-Cache-Cookie=urlprefix=<base64>:sign=…:t=…` form instead of the CloudFront policy the old code could read. Upstream MovieBox-Tui v0.1.22 decodes that prefix into the DASH manifest address (`…/dash/<id>/index.mpd`). Re-vendoring it took the survey of 17 popular titles from **0 playable to 15**, and `probe chain` over 20 titles from **14 with a playable source to 19**, Jawan, Animal, Kalki 2898 AD, Stree 2 and Munjya included. Ten anime and cartoon titles (Doraemon, Motu Patlu, Oggy, Demon Slayer, One Piece and others) all play from MovieBox, mostly in Hindi. The DASH downloader needed no change: `dash_health` downloads and muxes a sample through the new cookie.
- The "nothing found" message pointed at "Settings → Addons"; the panel is called **Extra sources**.

### Added
- **Dramachi as a fourth source**, after 4KHDHub and before the user's addons (`dramachi_streams` in `src-tauri/src/commands/streams.rs`). Added upstream in v0.1.22, it serves direct files for anime, K-dramas, cartoons and some films -- 8 of 10 test titles -- but only at 360p-540p, which is why it comes last. It uses the same exact-title rule as 4KHDHub, and drops the year Dramachi appends to film names ("Parasite 2019") only when it agrees with the year sought. Films arrive split into parts of about an hour; these are joined into one `edl://` timeline so they play and seek as one file, and are marked not downloadable because the downloader fetches a single file.
- `probe dramachi` (coverage of the Dramachi provider) and `probe dtier` (the app's Dramachi tier on one title); `probe chain` now walks the Dramachi tier too.

### Changed
- Vendored MovieBox-Tui v0.1.21 → v0.1.22, a clean tree replacement (our copy was byte-identical to upstream v0.1.21). `download_subtitle_file` gained a preferred-filename argument; the app passes `None`.

## 2026-09-20 — 1.3.1

### Fixed
- **Series never found a fallback, so with MovieBox dark most of them would not play.** MovieBox labels a series with the seasons it carries ("The Boys [Hindi] S1-S5") and reports the year of the season it is showing rather than the year the series began; 4KHDHub lists it under the plain name and the original year. The matcher demanded an identical title and an identical year, so every one of those comparisons failed. Season markers now come off the title, and the year is a preference rather than a gate -- exact year first, then nearest, and for a film only within a year. The title itself still has to match exactly, because playing the wrong film is worse than playing nothing. Measured over 20 popular titles: **10 had a playable source before, 14 after**; Breaking Bad, The Boys, Wednesday and Loki all went from nothing to 2160p (`src-tauri/src/commands/streams.rs`).
- **The Adults section would not scroll.** It was the one page without a scroll container, so anything past the first rows was unreachable by mouse or keyboard. It now scrolls like every other page, remembers its position, and its tiles take arrow-key focus (`src/pages/Adults.tsx`).
- **The PIN was asked for again every time you came back from a clip.** The unlock lived in the page's own state, and opening a clip leaves that page for the player, which unmounts it. It now lives in the app store, so it is asked for once per launch as the Settings panel always claimed. Changing the PIN, removing it, or switching the section off drops the unlock (`src/store/app.ts`, `src/components/AdultSettings.tsx`).
- **"Please try again in a little while" was sent to titles that would never work.** When nothing is found and no stream addon is installed, the app now says so and points at Settings → Addons instead of inviting another attempt.

### Added
- `probe chain` walks the app's own failover for a list of titles and prints where each one stops, through the real matcher rather than a copy of it (`src-tauri/src/bin/probe.rs`).

## 2026-09-20 — 1.3.0

### Added
- **An Adults section, behind a PIN.** Off until switched on in Settings, and it cannot be switched on without a PIN, so it never appears unguarded. Removing the PIN switches it off and clears every addon lock. It stays out of Home, Search and Continue Watching (`src/pages/Adults.tsx`, `src-tauri/src/commands/adult.rs`).
  - **RedGifs** through its documented API: an anonymous token is cached for the session, and posts resolve to a direct `.mp4`, so they play in MovieBox's own player and can be downloaded.
  - **Eporner** through its keyless API v2 for browsing, and its own embed for playback. Its file URLs answer 403 anywhere but the embed, and defeating that would be both fragile and rude, so the embed is shown in a sandboxed frame instead. Esc closes it.
  - Both are the platforms' own APIs rather than scrapers, so they keep whatever moderation those platforms run. No account, no key, nothing identifying sent.
  - The PIN is stored only as a salted SHA-256 and never reaches the UI. It is a household lock, not encryption, and the Settings panel says so.
- **Browsable addon catalogues.** `addon_catalogs` and `addon_catalog_items` expose what an installed addon offers, and any addon can be locked behind the same PIN. Addon items reuse the Details and Player screens through namespaced `addon:` ids.
- The player can now be handed a stream directly (`Session.directStream`), which is what the Adults section plays through.

### Changed
- `settings_set` no longer carries the PIN, the addon lock list or the Adults switch. They are owned by their own commands, so a stale settings object in the UI cannot wipe them and the hash never leaves the backend.

## 2026-09-20 — 1.2.0

### Fixed
- **MovieBox stopped serving video partway through the day and the app had no way to ride it out.** A survey of 17 popular titles scored 15 playable at 14:53 and 0 at 15:40, every one answering with the "update the app" advert; a fresh token, a clean cache and the previous vendored version all behaved the same, so this is their servers. Failover moved out of the player and into the `streams` command, where every caller gets it: MovieBox first, then 4KHDHub by title and year, then the user's addons. Downloads now reach the same alternative that playback does (`src-tauri/src/commands/streams.rs`).
- **"Try another source" had been failing on every title.** 4KHDHub moved its HubCloud/HubDrive links behind a `greenmotors.club` interstitial that hides the real URL under two base64 layers and a scrambled alphabet; the vendored v0.1.20 resolver could not follow it and reported "dead or expired". Upgrading to v0.1.21 took its mediator unpacker: Inception went from 0 of 7 releases to 5 of 5, at 2160p and 1080p.
- **Searching a title MovieBox actually carries could return nothing.** "Barbie" finds none, "Barbie movie" puts Barbie (2023) first. A first page with no results is retried once with a single extra word (`src-tauri/src/commands/catalog.rs`).
- **A download that started on the other source could not recover a dead link.** `refresh_url` only ever asked MovieBox, so a 4KHDHub worker link dying halfway ended the download instead of resuming it. It now falls back through the same tiers the stream came from (`src-tauri/src/core/downloads.rs`).
- Error messages now distinguish three cases that used to read alike, because the user can act on the difference: MovieBox withholding a title it has, MovieBox not carrying it at all, and the request itself failing (`StreamProblem`).

### Added
- **Stremio addons as a third source.** Settings → Extra sources installs an addon from its link, turns it off, or removes it. Addons key on IMDb ids, so Cinemeta (seeded by default, not removable) turns a title and year into one. Magnet and torrent links are dropped by the vendored adapter, so everything offered is directly playable. New commands `addons_list`, `addons_add`, `addons_remove`, `addons_toggle`, `addon_streams` (`src-tauri/src/commands/addons.rs`, `src/components/AddonsSettings.tsx`).
- Four read-only probe modes that answer "why isn't it playing?" with numbers: `survey`, `mirror`, `fourkplay`, `fourkmirrors`, plus `rescue` and `addons` (`src-tauri/src/bin/probe.rs`).
- `failover_health`, a skipped-by-default network test that walks MovieBox then the other source and prints which one answered.

### Changed
- Vendored MovieBox-Tui upgraded v0.1.20 → v0.1.21. Our copy was byte-identical to upstream, so it was a clean tree replacement. `resolve_release` now takes a `ResolutionIntent`, preferring seekable CDNs for playback and rejecting exhausted download workers immediately.

## 2026-09-20 — 1.1.1

### Fixed
- The update check at launch could fail on a connection that wasn't ready yet and then stay quiet until the next launch. It now retries once, 15 seconds later (`src/components/Updater.tsx`).

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
