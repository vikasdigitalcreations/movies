use crate::core::types::*;
use crate::state::AppState;
use moviebox_tui::favorites::FavoriteItem;
use moviebox_tui::history::WatchHistoryItem;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDto {
    pub id: String,
    pub title: String,
    pub poster: Option<String>,
    pub media_type: String,
    pub year: String,
    pub season: usize,
    pub episode: usize,
    pub progress: u64,
    pub duration: Option<u64>,
    pub completed: bool,
    pub updated: u64,
}

impl From<&WatchHistoryItem> for HistoryDto {
    fn from(i: &WatchHistoryItem) -> Self {
        HistoryDto {
            id: i.subject_id.clone(),
            title: clean_title(&i.title),
            poster: i.cover_url.clone(),
            media_type: if i.stype == 2 { "series" } else { "movie" }.into(),
            year: i.release_year.clone(),
            season: i.season,
            episode: i.episode,
            progress: i.progress_seconds,
            duration: i.duration_seconds,
            completed: i.completed,
            updated: i.timestamp,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRef {
    pub id: String,
    pub title: String,
    pub poster: Option<String>,
    pub media_type: String,
    pub year: Option<String>,
    pub season: usize,
    pub episode: usize,
}

fn to_item(r: &PlayRef) -> WatchHistoryItem {
    WatchHistoryItem {
        provider: "moviebox".into(),
        subject_id: r.id.clone(),
        title: r.title.clone(),
        cover_url: r.poster.clone(),
        stype: if r.media_type == "series" { 2 } else { 1 },
        release_year: r.year.clone().unwrap_or_default(),
        season: r.season,
        episode: r.episode,
        timestamp: 0,
        duration_seconds: None,
        progress_seconds: 0,
        completed: false,
    }
}

#[tauri::command]
pub async fn history_list(state: State<'_, AppState>) -> CmdResult<Vec<HistoryDto>> {
    let h = state.history.lock().await;
    let mut items: Vec<HistoryDto> = h.recent.iter().map(HistoryDto::from).collect();
    items.sort_by(|a, b| b.updated.cmp(&a.updated));
    Ok(items)
}

/// Resume info for one title (movie) or one episode.
#[tauri::command]
pub async fn history_get(state: State<'_, AppState>, id: String, season: usize, episode: usize) -> CmdResult<Option<HistoryDto>> {
    let h = state.history.lock().await;
    Ok(h.recent
        .iter()
        .find(|i| i.subject_id == id && (i.stype == 1 || (i.season == season && i.episode == episode)))
        .map(HistoryDto::from))
}

/// Watched flags for a list of episodes of one series.
#[tauri::command]
pub async fn history_watched(state: State<'_, AppState>, id: String, episodes: Vec<(usize, usize)>) -> CmdResult<Vec<bool>> {
    let h = state.history.lock().await;
    Ok(episodes.iter().map(|(s, e)| h.is_watched("moviebox", &id, *s, *e)).collect())
}

#[tauri::command]
pub async fn history_start(state: State<'_, AppState>, item: PlayRef, position: u64) -> CmdResult<()> {
    let mut h = state.history.lock().await;
    h.record_start(&to_item(&item), position);
    Ok(())
}

#[tauri::command]
pub async fn history_progress(state: State<'_, AppState>, item: PlayRef, position: u64, duration: Option<u64>) -> CmdResult<bool> {
    let completed = duration.map(|d| d > 0 && position as f64 >= d as f64 * 0.9).unwrap_or(false);
    let mut h = state.history.lock().await;
    let mut it = to_item(&item);
    it.duration_seconds = duration;
    if completed {
        it.progress_seconds = position;
        h.update_progress(it.clone(), position, duration, true);
        h.mark_watched(it);
    } else {
        h.update_progress(it, position, duration, false);
    }
    Ok(completed)
}

#[tauri::command]
pub async fn history_mark_watched(state: State<'_, AppState>, item: PlayRef) -> CmdResult<()> {
    let mut h = state.history.lock().await;
    h.mark_watched(to_item(&item));
    Ok(())
}

#[tauri::command]
pub async fn history_remove(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut h = state.history.lock().await;
    let targets: Vec<(usize, usize)> = h.recent.iter().filter(|i| i.subject_id == id).map(|i| (i.season, i.episode)).collect();
    for (s, e) in targets {
        h.remove("moviebox", &id, s, e);
    }
    Ok(())
}

#[tauri::command]
pub async fn history_clear(state: State<'_, AppState>) -> CmdResult<()> {
    state.history.lock().await.clear();
    Ok(())
}

#[tauri::command]
pub async fn favorites_list(state: State<'_, AppState>) -> CmdResult<Vec<Card>> {
    let f = state.favorites.lock().await;
    let mut items = f.items.clone();
    items.sort_by(|a, b| b.added_at.cmp(&a.added_at));
    Ok(items
        .iter()
        .map(|i| Card {
            id: i.subject_id.clone(),
            title: clean_title(&i.title),
            year: Some(i.release_year.clone()).filter(|y| !y.is_empty()),
            poster: i.cover_url.clone(),
            media_type: if i.stype == 2 { "series" } else { "movie" }.into(),
        })
        .collect())
}

/// Toggle My List. Returns the new state (true = in list).
#[tauri::command]
pub async fn favorites_toggle(state: State<'_, AppState>, card: Card) -> CmdResult<bool> {
    let item = FavoriteItem {
        provider: "moviebox".into(),
        subject_id: card.id.clone(),
        title: card.title.clone(),
        cover_url: card.poster.clone(),
        stype: if card.media_type == "series" { 2 } else { 1 },
        release_year: card.year.clone().unwrap_or_default(),
        added_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    let mut f = state.favorites.lock().await;
    let now = f.toggle(item);
    f.save();
    Ok(now)
}

#[tauri::command]
pub async fn favorites_clear(state: State<'_, AppState>) -> CmdResult<()> {
    let mut f = state.favorites.lock().await;
    f.clear();
    f.save();
    Ok(())
}
