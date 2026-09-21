use crate::core::race::{hedged, pick_best};
use crate::core::stream_pool::{drop_notice_mirrors, merge_releases, pick_for_quality, sort_best_first};
use crate::core::types::*;
use crate::state::AppState;
use moviebox_tui::providers::moviebox::adapt::moviebox_resource_item_to_release;
use moviebox_tui::providers::{ProviderKind, Release, ReleaseProvider, ResolutionIntent};
use futures::future::BoxFuture;
use std::time::Duration;
use moviebox_tui::service::MovieBoxService;
use tauri::State;

/// Fetch direct-file releases (one per resolution) from the resource list.
/// `abs_index` is the 0-based position of the episode across all seasons (0 for movies).
async fn resource_releases(service: &MovieBoxService, id: &str, season: usize, episode: usize, abs_index: usize) -> Vec<Release> {
    let client = service.client.clone();
    let is_movie = season == 0 && episode == 0;
    let mut out = Vec::new();
    if is_movie {
        for page in 1..=5usize {
            match tokio::time::timeout(Duration::from_secs(15), client.fetch_resource_page(id, 0, 0, 0, page)).await {
                Ok(Ok((items, pager))) => {
                    out.extend(items.iter().map(moviebox_resource_item_to_release));
                    if !pager.get("hasMore").and_then(|v| v.as_bool()).unwrap_or(false) {
                        break;
                    }
                }
                _ => break,
            }
        }
        return out;
    }
    let resolutions = service.fetch_collection_resolutions(id).await.unwrap_or_default();
    let est = abs_index / 20 + 1;
    let futs = resolutions.iter().map(|&res| {
        let client = client.clone();
        let id = id.to_string();
        async move {
            let mut found = Vec::new();
            for page in [est, est + 1, est.saturating_sub(1).max(1), est + 2] {
                if let Ok(Ok((items, _))) = tokio::time::timeout(Duration::from_secs(15), client.fetch_resource_page(&id, 0, 0, res, page)).await {
                    let rels: Vec<Release> = items
                        .iter()
                        .map(moviebox_resource_item_to_release)
                        .filter(|r| r.season == Some(season) && r.episode == Some(episode))
                        .collect();
                    if !rels.is_empty() {
                        found = rels;
                        break;
                    }
                }
            }
            found
        }
    });
    for rels in futures::future::join_all(futs).await {
        out.extend(rels);
    }
    out
}

/// Why MovieBox produced nothing playable. The distinction matters: a title MovieBox
/// does not carry is hopeless, while one it merely withholds is usually available from
/// the other source, and the wording the user sees should not be the same.
#[derive(Debug)]
pub enum StreamProblem {
    /// MovieBox answered only with its "Update now. Keep watching." advert clip.
    OnlyAdvert,
    /// MovieBox has the page but lists no stream at all.
    NotCarried,
    /// The request itself failed (offline, timeout, provider error).
    Provider(String),
}

impl StreamProblem {
    /// What to show when there is no other source to fall back on either.
    fn message(&self) -> String {
        match self {
            Self::OnlyAdvert => "MovieBox is only serving its \"update the app\" advert for this title, and no other source has it. Try again later, or pick another title.".into(),
            Self::NotCarried => "No source has this title right now.".into(),
            Self::Provider(e) => e.clone(),
        }
    }
}

pub async fn collect_streams(service: &MovieBoxService, id: &str, season: usize, episode: usize, abs_index: usize) -> Result<Vec<Release>, StreamProblem> {
    let client = service.client.clone();
    let (play, direct) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(20), ReleaseProvider::episode_streams(&client, id, season, episode)),
        resource_releases(service, id, season, episode, abs_index)
    );
    let mut pool: Vec<Release> = Vec::new();
    let mut play_err = None;
    match play {
        Ok(Ok(rels)) => merge_releases(&mut pool, rels, season, episode),
        Ok(Err(e)) => play_err = Some(e.user_message(ProviderKind::MovieBox)),
        Err(_) => play_err = Some("timed out".into()),
    }
    merge_releases(&mut pool, direct, season, episode);
    pool.retain(|r| !r.mirrors.is_empty());
    // MovieBox hands out an "Update now. Keep watching." advert clip instead of the
    // real file on every direct link; never let one through to the player or a download.
    let had_notice = drop_notice_mirrors(&mut pool);
    sort_best_first(&mut pool);
    if pool.is_empty() {
        return Err(match (play_err, had_notice) {
            // The advert clip means MovieBox holds the title but is withholding the file.
            // That is a different problem from not carrying it at all, and the caller acts
            // on the difference, so record which one it is instead of one vague string.
            (_, true) => StreamProblem::OnlyAdvert,
            (Some(e), false) => StreamProblem::Provider(friendly(e)),
            (None, false) => StreamProblem::NotCarried,
        });
    }
    Ok(pool)
}

/// How long MovieBox gets to answer alone before the other sources start looking too.
/// It normally answers in under a second, so a healthy MovieBox never causes a request
/// to the others; this only matters when it is slow or down.
const HEDGE_AFTER: Duration = Duration::from_secs(5);

/// How much longer a better source gets when a worse one has already answered. A 4K
/// release from 4KHDHub is worth a wait over a 540p file from Dramachi, but a slow tier
/// must not hold the whole result hostage. 4KHDHub resolves several mirrors and routinely
/// needs 10 s or more: with 4 s here, Oppenheimer came back as Dramachi 540p instead of
/// 4KHDHub 2160p, which the one-after-another order used to find.
const BACKUP_GRACE: Duration = Duration::from_secs(12);

/// Streams for one episode or movie, MovieBox first and the other sources behind it.
///
/// The failover lives here rather than in the player so that downloads get it too: when
/// MovieBox withholds a title, "Download" should find the same alternative that "Play"
/// does. `title`/`year` are what the other sources are searched by; without them the
/// failover is skipped and only MovieBox is consulted.
///
/// The other sources are not tried one after another once MovieBox has failed. They start
/// looking as soon as MovieBox has failed or been slow for `HEDGE_AFTER`, and are asked
/// together, so a title only the last one carries costs the slowest of them rather than
/// the sum. Order of preference is unchanged. `primary_only` asks MovieBox alone; the
/// prefetch that runs when a title's page opens uses it, so merely looking at a title
/// never sets the heavier 4KHDHub scraper going.
#[tauri::command]
pub async fn streams(
    state: State<'_, AppState>,
    id: String,
    season: usize,
    episode: usize,
    abs_index: usize,
    title: Option<String>,
    year: Option<String>,
    preferred: Option<u64>,
    primary_only: Option<bool>,
) -> CmdResult<Vec<StreamDto>> {
    let primary = async {
        match collect_streams(&state.service, &id, season, episode, abs_index).await {
            Ok(pool) => {
                let out: Vec<StreamDto> = pool.iter().filter_map(StreamDto::from_release).collect();
                if out.is_empty() {
                    Err(StreamProblem::NotCarried)
                } else {
                    Ok(out)
                }
            }
            Err(p) => Err(p),
        }
    };
    let title = title.filter(|t| !t.trim().is_empty());
    let Some(title) = title.filter(|_| !primary_only.unwrap_or(false)) else {
        return primary.await.map_err(|p| p.message());
    };
    let youtube = state.settings.read().await.youtube_source;
    let backups = backup_streams(&state.service, &title, year.as_deref(), season, episode, preferred.unwrap_or(0), youtube);
    hedged(primary, backups, HEDGE_AFTER).await.map_err(|(problem, _)| dead_end(&problem))
}

/// Every source except MovieBox, asked together, best one wins (see `pick_best`).
///
/// Tier order is 4KHDHub, YouTube's official film channels, Dramachi, then the user's
/// addons. YouTube outranks Dramachi because its films are 720p to 1080p against Dramachi's
/// 360p to 540p. The addons come last because they are slower and entirely user-configured,
/// but they are the only tier that keeps working when every built-in provider goes dark.
/// `youtube` is the user's setting; a tier that is switched off stays in its place, so the
/// others keep their rank.
async fn backup_streams(
    service: &MovieBoxService,
    title: &str,
    year: Option<&str>,
    season: usize,
    episode: usize,
    preferred: u64,
    youtube: bool,
) -> Result<Vec<StreamDto>, String> {
    let is_series = season > 0 || episode > 0;
    let tiers: Vec<BoxFuture<'_, Result<Vec<StreamDto>, String>>> = vec![
        Box::pin(fourk_streams(service, title, year, season, episode, preferred, 4)),
        Box::pin(async move {
            if youtube {
                youtube_streams(title, year, season, episode).await
            } else {
                Err("YouTube is switched off in Settings.".to_string())
            }
        }),
        Box::pin(dramachi_streams(service, title, year, season, episode)),
        Box::pin(crate::commands::addons::addon_streams_for(title, year, is_series, season, episode)),
    ];
    pick_best(tiers, BACKUP_GRACE).await.map_err(|errors| errors.into_iter().last().unwrap_or_default())
}

/// What to say when every source came back empty.
///
/// The old wording ended in "try again later", which is what sent people round the same
/// title over and over. When the user has no addons installed there is a last tier
/// sitting unused, and pointing at it is more use than asking them to wait.
fn dead_end(problem: &StreamProblem) -> String {
    let has_addons = moviebox_tui::config::load_addons().iter().any(|a| a.enabled && a.provides_stream);
    if has_addons {
        return problem.message();
    }
    match problem {
        StreamProblem::Provider(e) => e.clone(),
        _ => "No source has this title right now. Adding a stream addon in Settings \u{2192} Extra sources gives the app somewhere else to look.".into(),
    }
}

#[tauri::command]
pub async fn subtitles(state: State<'_, AppState>, id: String, resource_id: String, dub_ids: Vec<String>, season: usize, episode: usize) -> CmdResult<Vec<SubtitleDto>> {
    let res = tokio::time::timeout(
        Duration::from_secs(15),
        state.service.get_ext_captions(&id, &resource_id, &dub_ids, season, episode),
    )
    .await
    .map_err(|_| "Subtitles took too long to load.".to_string())?
    .map_err(friendly)?;
    Ok(res
        .into_iter()
        .map(|s| SubtitleDto { name: moviebox_tui::tui::text::sanitize_language_label(&s.name), url: s.url })
        .collect())
}

/// Download a subtitle to a local file mpv can load (URLs may need headers).
#[tauri::command]
pub async fn fetch_subtitle(state: State<'_, AppState>, url: String) -> CmdResult<String> {
    let path = state.service.download_subtitle_file(&url, &[], None).await?;
    Ok(path.to_string_lossy().into_owned())
}

pub fn norm_title(t: &str) -> String {
    strip_season(moviebox_tui::providers::moviebox::clean_moviebox_title(t))
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Drop a trailing season marker from a title.
///
/// MovieBox labels a series with the seasons it is carrying -- "Breaking Bad [Hindi] S5",
/// "The Boys [Hindi] S1-S5" -- while the other source lists the series under its plain
/// name. Comparing the two without removing the marker never matches, which is why series
/// used to fall through to no source at all.
pub fn strip_season(t: &str) -> &str {
    let trimmed = t.trim_end_matches(|c: char| c.is_whitespace() || c == '-');
    let cut = trimmed.rfind(|c: char| c.is_whitespace()).map(|i| i + 1).unwrap_or(0);
    let tail = &trimmed[cut..];
    let looks_like_season = {
        let low = tail.to_ascii_lowercase();
        let body = low.strip_prefix('s').unwrap_or("");
        !body.is_empty()
            && body.chars().all(|c| c.is_ascii_digit() || c == '-')
            && body.chars().any(|c| c.is_ascii_digit())
    };
    if cut > 0 && looks_like_season {
        // "... Season 3" leaves the word behind once the number is gone.
        let head = trimmed[..cut].trim_end();
        return head.strip_suffix("Season").map(str::trim_end).unwrap_or(head);
    }
    // "Breaking Bad Season 3" -- the number is a bare word rather than "S3".
    if cut > 0 && tail.chars().all(|c| c.is_ascii_digit()) {
        let head = trimmed[..cut].trim_end();
        if let Some(h) = head.strip_suffix("Season").or_else(|| head.strip_suffix("season")) {
            return h.trim_end();
        }
    }
    trimmed
}

/// How far apart two release years are, when both are known.
fn year_gap(a: &str, b: &str) -> Option<i32> {
    let (a, b) = (a.trim().parse::<i32>().ok()?, b.trim().parse::<i32>().ok()?);
    Some((a - b).abs())
}

/// Resolve playable 4KHDHub streams for a title, best first.
///
/// 4KHDHub uses its own ids, so the title is matched by normalised name and year and
/// only a confident match is accepted -- playing the wrong film is worse than playing
/// nothing. Resolving a mirror costs a round trip through their redirector, so only the
/// first few releases are tried, and they are tried together rather than one after another.
pub async fn fourk_streams(
    service: &MovieBoxService,
    title: &str,
    year: Option<&str>,
    season: usize,
    episode: usize,
    preferred: u64,
    limit: usize,
) -> Result<Vec<StreamDto>, String> {
    let Some(fourk) = service.fourk_client.clone() else {
        return Err("No other source is available for this title.".into());
    };
    // Search by the plain name first. MovieBox decorates its own titles ("Inception
    // [Hindi]", "The Boys [Hindi] S1-S5"), and those decorations are noise to the other
    // source's search; the raw title is kept as a second attempt in case cleaning it
    // removed something that mattered.
    let clean = strip_season(moviebox_tui::providers::moviebox::clean_moviebox_title(title)).trim().to_string();
    let want = norm_title(title);
    let year = year.unwrap_or_default();
    let is_series = season > 0 || episode > 0;

    let mut results = Vec::new();
    for q in [clean.as_str(), title].iter().take(if clean.eq_ignore_ascii_case(title) { 1 } else { 2 }) {
        let hits = tokio::time::timeout(Duration::from_secs(15), service.search_typed(ProviderKind::FourKHdHub, q, 1))
            .await
            .map_err(|_| "The other source took too long to respond.".to_string())?
            .unwrap_or_default();
        results.extend(hits);
        if results.iter().any(|c| norm_title(&c.title) == want) {
            break;
        }
    }

    // The title must match exactly -- playing the wrong film is worse than playing
    // nothing. The year is a preference rather than a gate: for a series MovieBox
    // reports the year of the season it is showing, not the year the series began, so
    // demanding an exact year meant no series ever found a fallback.
    let mut named: Vec<_> = results.iter().filter(|c| norm_title(&c.title) == want).collect();
    named.sort_by_key(|c| match (year.is_empty(), c.year.as_deref()) {
        (true, _) => 0,
        (false, Some(y)) => year_gap(y, &year).unwrap_or(99),
        (false, None) => 50,
    });
    let matched = named.into_iter().find(|c| {
        if year.is_empty() || is_series {
            return true;
        }
        match c.year.as_deref() {
            // A one-year drift is ordinary: festival year against release year.
            Some(y) => year_gap(y, &year).map(|g| g <= 1).unwrap_or(false),
            None => true,
        }
    });
    let Some(item) = matched else {
        return Err("No other source has this title.".into());
    };
    let mut rels = tokio::time::timeout(Duration::from_secs(20), ReleaseProvider::episode_streams(&fourk, &item.id.value, season, episode))
        .await
        .map_err(|_| "The other source took too long to respond.".to_string())?
        .map_err(|_| "No other source has this title.".to_string())?;
    sort_best_first(&mut rels);
    // Preferred quality first, then everything else as written.
    let mut order: Vec<Release> = Vec::new();
    if let Some(best) = pick_for_quality(&rels, preferred) {
        order.push(best.clone());
    }
    for r in rels {
        if order.first().map(|f| f.filename != r.filename).unwrap_or(true) {
            order.push(r);
        }
    }
    order.truncate(limit);

    let attempts = order.iter().map(|rel| {
        let fourk = fourk.clone();
        async move {
            let src = tokio::time::timeout(Duration::from_secs(18), fourk.resolve_release(rel, ResolutionIntent::Playback))
                .await
                .ok()?
                .ok()?;
            let height = rel.resolution_u64();
            Some(StreamDto {
                label: format!("{height}p"),
                height,
                multi: false,
                size: rel.size_bytes,
                codec: rel.codec.clone(),
                url: src.url,
                headers: src.headers,
                resource_id: None,
                // 4KHDHub hands back a plain file, which the ordinary downloader handles.
                downloadable: true,
                source: "4KHDHub".into(),
            })
        }
    });
    let out: Vec<StreamDto> = futures::future::join_all(attempts).await.into_iter().flatten().collect();
    if out.is_empty() {
        return Err("The other source isn't working right now either. Please try again later.".into());
    }
    Ok(out)
}

/// The non-MovieBox tiers, for callers outside the `streams` command (the download queue
/// refreshing a link that died mid-transfer).
pub async fn other_source_streams(
    service: &MovieBoxService,
    title: &str,
    year: Option<&str>,
    season: usize,
    episode: usize,
    preferred: u64,
) -> Result<Vec<StreamDto>, String> {
    // A download needs one file it can fetch; YouTube's films are two files joined by the
    // player, so this path never asks for them.
    backup_streams(service, title, year, season, episode, preferred, false).await
}

/// "Try another source", kept for the player's mid-playback retry.
#[tauri::command]
pub async fn alternate_source(state: State<'_, AppState>, title: String, year: Option<String>, season: usize, episode: usize, preferred: u64) -> CmdResult<StreamDto> {
    let youtube = state.settings.read().await.youtube_source;
    let mut list = backup_streams(&state.service, &title, year.as_deref(), season, episode, preferred, youtube).await?;
    Ok(list.remove(0))
}

/// Streams from Dramachi, the last built-in source.
///
/// Its files top out at 540p, so it only ever stands in when MovieBox and 4KHDHub both
/// come back empty. It is strongest exactly where they are weakest: anime, K-dramas and
/// children's cartoons. The same exact-title rule as 4KHDHub applies.
pub async fn dramachi_streams(
    service: &MovieBoxService,
    title: &str,
    year: Option<&str>,
    season: usize,
    episode: usize,
) -> Result<Vec<StreamDto>, String> {
    let clean = strip_season(moviebox_tui::providers::moviebox::clean_moviebox_title(title)).trim().to_string();
    let want = norm_title(title);
    let year = year.unwrap_or_default();
    let is_series = season > 0 || episode > 0;
    let hits = tokio::time::timeout(Duration::from_secs(15), service.search_typed(ProviderKind::Dramachi, &clean, 1))
        .await
        .map_err(|_| "The other source took too long to respond.".to_string())?
        .unwrap_or_default();
    let Some(item) = hits.iter().find(|c| dramachi_title_matches(&c.title, &want, year, is_series)) else {
        return Err("No other source has this title.".into());
    };
    let rels = tokio::time::timeout(Duration::from_secs(20), ReleaseProvider::episode_streams(&service.dramachi_client, &item.id.value, season, episode))
        .await
        .map_err(|_| "The other source took too long to respond.".to_string())?
        .map_err(|_| "No other source has this title.".to_string())?;
    let mut rels: Vec<Release> = rels.into_iter().filter(|r| !r.mirrors.is_empty()).collect();
    // A film comes back in parts ("Parasite_2019_001", "_002", about an hour each), not
    // as alternatives. Playing the first alone would stop halfway, so the parts are
    // joined into one timeline that mpv plays and seeks as a single file.
    if !is_series && rels.len() > 1 {
        rels.sort_by(|a, b| a.filename.cmp(&b.filename));
        let urls: Vec<&str> = rels.iter().map(|r| r.mirrors[0].resolver_url.as_str()).collect();
        let Some(mut whole) = StreamDto::from_release(&rels[0]) else {
            return Err("No other source has this title.".into());
        };
        whole.url = edl_join(&urls);
        whole.size = rels.iter().map(|r| r.size_bytes).sum();
        // The downloader fetches one file; a joined timeline is not one.
        whole.downloadable = false;
        return Ok(vec![whole]);
    }
    sort_best_first(&mut rels);
    let out: Vec<StreamDto> = rels.iter().filter_map(StreamDto::from_release).collect();
    if out.is_empty() {
        return Err("No other source has this title.".into());
    }
    Ok(out)
}

/// Streams from the official YouTube channels of film distributors. Films only: those
/// channels do not carry series episodes, and matching an episode by name would guess.
pub async fn youtube_streams(title: &str, year: Option<&str>, season: usize, episode: usize) -> Result<Vec<StreamDto>, String> {
    if season > 0 || episode > 0 {
        return Err("YouTube's film channels carry films, not episodes.".into());
    }
    let clean = strip_season(moviebox_tui::providers::moviebox::clean_moviebox_title(title)).trim().to_string();
    crate::core::youtube::find(&clean, &norm_title(title), year.unwrap_or_default()).await
}

/// An mpv `edl://` address that plays several files back to back as one. Each entry is
/// length-prefixed (`%N%`) so a `,` or `;` inside a URL cannot split it.
fn edl_join(urls: &[&str]) -> String {
    let parts: Vec<String> = urls.iter().map(|u| format!("%{}%{}", u.len(), u)).collect();
    format!("edl://{}", parts.join(";"))
}

/// Dramachi appends the release year to film names ("Parasite 2019", "Inception 2010").
/// A trailing year is dropped before comparing, but only when it agrees with the year
/// being looked for, so "Blade Runner 2049" is still read as a name, not as a date.
fn dramachi_title_matches(candidate: &str, want: &str, year: &str, is_series: bool) -> bool {
    if norm_title(candidate) == want {
        return true;
    }
    let Some((head, tail)) = candidate.trim().rsplit_once(' ') else {
        return false;
    };
    let is_year = tail.len() == 4 && tail.chars().all(|c| c.is_ascii_digit());
    if !is_year || norm_title(head) != want {
        return false;
    }
    is_series || year.is_empty() || year_gap(tail, year).map(|g| g <= 1).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn season_markers_come_off_the_title() {
        // MovieBox labels a series with the seasons it carries; the other source does not.
        assert_eq!(norm_title("Breaking Bad [Hindi] S5"), "breakingbad");
        assert_eq!(norm_title("The Boys [Hindi] S1-S5"), "theboys");
        assert_eq!(norm_title("Wednesday [Hindi] S2"), "wednesday");
        assert_eq!(norm_title("Loki Season 2"), "loki");
        assert_eq!(norm_title("Inception [Hindi]"), "inception");
    }

    #[test]
    fn a_title_that_ends_in_a_number_is_not_a_season() {
        // "Stree 2" and "3 Idiots" are names, not season markers.
        assert_eq!(norm_title("Stree 2"), "stree2");
        assert_eq!(norm_title("Dune: Part Two"), "duneparttwo");
        assert_eq!(norm_title("3 Idiots"), "3idiots");
    }

    #[test]
    fn dramachi_years_come_off_only_when_they_agree() {
        assert!(dramachi_title_matches("Parasite 2019", "parasite", "2019", false));
        assert!(dramachi_title_matches("Inception 2010", "inception", "", false));
        assert!(dramachi_title_matches("Squid Game", "squidgame", "2021", true));
        // A year that disagrees is a different film.
        assert!(!dramachi_title_matches("Dune 1984", "dune", "2021", false));
        // A number that belongs to the name stays part of it.
        assert!(dramachi_title_matches("Blade Runner 2049", "bladerunner2049", "2017", false));
        assert!(!dramachi_title_matches("Naruto The Movie 2 Bonds 2008", "naruto", "2002", true));
    }

    #[test]
    fn film_parts_join_into_one_timeline() {
        assert_eq!(
            edl_join(&["https://a/x_001.mp4", "https://a/x;2.mp4"]),
            "edl://%19%https://a/x_001.mp4;%17%https://a/x;2.mp4"
        );
    }

    #[test]
    fn years_compare_by_distance() {
        assert_eq!(year_gap("2024", "2023"), Some(1));
        assert_eq!(year_gap("2019", "2024"), Some(5));
        assert_eq!(year_gap("", "2024"), None);
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Network check, not part of the normal suite:
    /// `cargo test --lib failover_health -- --ignored --nocapture`
    /// Walks the path the app takes when MovieBox withholds a title: MovieBox first,
    /// then the other source. Passes if either one yields a playable stream, and prints
    /// which, so a provider outage is visible rather than silent.
    #[tokio::test]
    #[ignore]
    async fn failover_health() {
        let service = MovieBoxService::new();
        let hits = service.search_typed(ProviderKind::MovieBox, "Inception", 1).await.expect("search works");
        let first = hits.first().expect("at least one hit");
        let year = first.year.clone();

        match collect_streams(&service, &first.id.value, 0, 0, 0).await {
            Ok(pool) => println!("MovieBox: {} playable release(s)", pool.len()),
            Err(p) => {
                println!("MovieBox: nothing playable ({p:?})");
                let alt = fourk_streams(&service, &first.title, year.as_deref(), 0, 0, 0, 4)
                    .await
                    .expect("the other source covers the gap");
                for s in &alt {
                    println!("  4KHDHub {} {}", s.label, s.url.chars().take(70).collect::<String>());
                }
                assert!(!alt.is_empty());
            }
        }
    }

    /// Network check, not part of the normal suite:
    /// `cargo test --lib provider_health -- --ignored --nocapture`
    /// Confirms MovieBox still returns a real, fetchable stream (and not the
    /// "Update now. Keep watching." advert clip) for a well-known title.
    #[tokio::test]
    #[ignore]
    async fn provider_health() {
        let service = MovieBoxService::new();
        let hits = service
            .search_typed(ProviderKind::MovieBox, "Inception", 1)
            .await
            .expect("search works");
        let first = hits.first().expect("at least one hit");
        println!("{} ({:?}) id={}", first.title, first.year, first.id.value);

        let pool = collect_streams(&service, &first.id.value, 0, 0, 0)
            .await
            .expect("a playable stream");
        for r in &pool {
            let m = &r.mirrors[0];
            println!("  {}p multi={} {}", r.resolution_u64(), r.is_multi_resolution(), m.resolver_url);
            assert!(!crate::core::stream_pool::is_notice_url(&m.resolver_url), "notice clip leaked into the pool");
        }

        let m = &pool[0].mirrors[0];
        let mut req = service.client.http_client().get(&m.resolver_url).header("Range", "bytes=0-1023");
        for (k, v) in &m.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let status = req.send().await.expect("stream reachable").status();
        println!("  first mirror -> {status}");
        assert!(status.is_success(), "stream answered {status}");
    }
}
