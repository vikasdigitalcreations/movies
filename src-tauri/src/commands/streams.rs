use crate::core::stream_pool::{merge_releases, pick_for_quality, sort_best_first};
use crate::core::types::*;
use crate::state::AppState;
use moviebox_tui::providers::moviebox::adapt::moviebox_resource_item_to_release;
use moviebox_tui::providers::{ProviderKind, Release, ReleaseProvider};
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

pub async fn collect_streams(service: &MovieBoxService, id: &str, season: usize, episode: usize, abs_index: usize) -> Result<Vec<Release>, String> {
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
    sort_best_first(&mut pool);
    if pool.is_empty() {
        return Err(match play_err {
            Some(e) => friendly(e),
            None => "No playable source was found for this title.".into(),
        });
    }
    Ok(pool)
}

#[tauri::command]
pub async fn streams(state: State<'_, AppState>, id: String, season: usize, episode: usize, abs_index: usize) -> CmdResult<Vec<StreamDto>> {
    let pool = collect_streams(&state.service, &id, season, episode, abs_index).await?;
    Ok(pool.iter().filter_map(StreamDto::from_release).collect())
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

fn norm(t: &str) -> String {
    moviebox_tui::providers::moviebox::clean_moviebox_title(t)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// "Try another source": search 4KHDHub for the same title + year and resolve a stream.
/// Only a confident match (same normalised title and same year) is used.
#[tauri::command]
pub async fn alternate_source(state: State<'_, AppState>, title: String, year: Option<String>, season: usize, episode: usize, preferred: u64) -> CmdResult<StreamDto> {
    let Some(fourk) = state.service.fourk_client.clone() else {
        return Err("No other source is available for this title.".into());
    };
    let results = tokio::time::timeout(Duration::from_secs(15), state.service.search_typed(ProviderKind::FourKHdHub, &title, 1))
        .await
        .map_err(|_| "No other source is available for this title.".to_string())?
        .unwrap_or_default();
    let want = norm(&title);
    let year = year.unwrap_or_default();
    let matched = results.into_iter().find(|c| {
        norm(&c.title) == want && (year.is_empty() || c.year.as_deref().unwrap_or("") == year)
    });
    let Some(item) = matched else {
        return Err("No other source is available for this title.".into());
    };
    let mut rels = tokio::time::timeout(Duration::from_secs(20), ReleaseProvider::episode_streams(&fourk, &item.id.value, season, episode))
        .await
        .map_err(|_| "The other source took too long to respond.".to_string())?
        .map_err(|_| "No other source is available for this title.".to_string())?;
    sort_best_first(&mut rels);
    let mut order: Vec<Release> = Vec::new();
    if let Some(best) = pick_for_quality(&rels, preferred) {
        order.push(best.clone());
    }
    order.extend(rels.into_iter());
    for rel in order.iter().take(4) {
        if let Ok(Ok(src)) = tokio::time::timeout(Duration::from_secs(18), fourk.resolve_release(rel)).await {
            let height = rel.resolution_u64();
            return Ok(StreamDto {
                label: format!("{height}p"),
                height,
                multi: false,
                size: rel.size_bytes,
                codec: rel.codec.clone(),
                url: src.url,
                headers: src.headers,
                resource_id: None,
                downloadable: false,
                source: "4KHDHub".into(),
            });
        }
    }
    Err("The other source isn't working right now either. Please try again later.".into())
}
