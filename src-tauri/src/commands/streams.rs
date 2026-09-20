use crate::core::stream_pool::{drop_notice_mirrors, merge_releases, pick_for_quality, sort_best_first};
use crate::core::types::*;
use crate::state::AppState;
use moviebox_tui::providers::moviebox::adapt::moviebox_resource_item_to_release;
use moviebox_tui::providers::{ProviderKind, Release, ReleaseProvider, ResolutionIntent};
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

/// Streams for one episode or movie, MovieBox first and the other source behind it.
///
/// The failover lives here rather than in the player so that downloads get it too: when
/// MovieBox withholds a title, "Download" should find the same alternative that "Play"
/// does. `title`/`year` are what the other source is searched by; without them the
/// failover is skipped and only MovieBox is consulted.
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
) -> CmdResult<Vec<StreamDto>> {
    let problem = match collect_streams(&state.service, &id, season, episode, abs_index).await {
        Ok(pool) => {
            let out: Vec<StreamDto> = pool.iter().filter_map(StreamDto::from_release).collect();
            if !out.is_empty() {
                return Ok(out);
            }
            StreamProblem::NotCarried
        }
        Err(p) => p,
    };
    let Some(title) = title.filter(|t| !t.trim().is_empty()) else {
        return Err(problem.message());
    };
    if let Ok(list) = fourk_streams(&state.service, &title, year.as_deref(), season, episode, preferred.unwrap_or(0), 4).await {
        return Ok(list);
    }
    // Last, whatever the user's own addons offer. They are tried after the built-in
    // sources because they are slower and entirely user-configured, but they are the
    // only tier that keeps working when both built-in providers go dark.
    let is_series = season > 0 || episode > 0;
    crate::commands::addons::addon_streams_for(&title, year.as_deref(), is_series, season, episode)
        .await
        .map_err(|_| problem.message())
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
    let path = state.service.download_subtitle_file(&url, &[]).await?;
    Ok(path.to_string_lossy().into_owned())
}

pub fn norm_title(t: &str) -> String {
    moviebox_tui::providers::moviebox::clean_moviebox_title(t)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Resolve playable 4KHDHub streams for a title, best first.
///
/// 4KHDHub uses its own ids, so the title is matched by normalised name and year and
/// only a confident match is accepted -- playing the wrong film is worse than playing
/// nothing. Resolving a mirror costs a round trip through their redirector, so only the
/// first few releases are tried, and they are tried together rather than one after another.
async fn fourk_streams(
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
    let results = tokio::time::timeout(Duration::from_secs(15), service.search_typed(ProviderKind::FourKHdHub, title, 1))
        .await
        .map_err(|_| "The other source took too long to respond.".to_string())?
        .unwrap_or_default();
    let want = norm_title(title);
    let year = year.unwrap_or_default();
    let matched = results
        .into_iter()
        .find(|c| norm_title(&c.title) == want && (year.is_empty() || c.year.as_deref().unwrap_or("") == year));
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

/// The non-MovieBox tiers, in order, for callers outside the `streams` command
/// (the download queue refreshing a link that died mid-transfer).
pub async fn other_source_streams(
    service: &MovieBoxService,
    title: &str,
    year: Option<&str>,
    season: usize,
    episode: usize,
    preferred: u64,
) -> Result<Vec<StreamDto>, String> {
    if let Ok(list) = fourk_streams(service, title, year, season, episode, preferred, 4).await {
        return Ok(list);
    }
    crate::commands::addons::addon_streams_for(title, year, season > 0 || episode > 0, season, episode).await
}

/// "Try another source", kept for the player's mid-playback retry.
#[tauri::command]
pub async fn alternate_source(state: State<'_, AppState>, title: String, year: Option<String>, season: usize, episode: usize, preferred: u64) -> CmdResult<StreamDto> {
    let mut list = fourk_streams(&state.service, &title, year.as_deref(), season, episode, preferred, 4).await?;
    Ok(list.remove(0))
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
