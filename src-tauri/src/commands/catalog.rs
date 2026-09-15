use crate::core::types::*;
use crate::state::AppState;
use moviebox_tui::providers::moviebox::adapt::moviebox_subject_json_to_catalog_item;
use moviebox_tui::providers::{MediaType, ProviderKind};
use serde_json::Value;
use std::collections::HashSet;
use tauri::State;

/// Walk a JSON value collecting objects that look like subjects (have subjectId + title).
fn collect_subjects<'a>(v: &'a Value, out: &mut Vec<&'a Value>, depth: usize) {
    if depth > 5 {
        return;
    }
    match v {
        Value::Object(map) => {
            if map.contains_key("subjectId") && map.contains_key("title") && map.contains_key("subjectType") {
                out.push(v);
                return;
            }
            for (_, child) in map {
                collect_subjects(child, out, depth + 1);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                collect_subjects(child, out, depth + 1);
            }
        }
        _ => {}
    }
}

fn is_video_subject(v: &Value) -> bool {
    matches!(v.get("subjectType").and_then(|s| s.as_i64()), Some(1) | Some(2) | Some(7))
}

pub fn parse_home(payload: &Value) -> Vec<HomeRow> {
    let mut rows = Vec::new();
    let Some(groups) = payload.get("items").and_then(|i| i.as_array()) else {
        return rows;
    };
    let mut hero_seen = false;
    for g in groups {
        let kind = g.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if matches!(kind, "FILTER" | "POST_LIST" | "MUSIC_CHARTS" | "LIVE_LIST") {
            continue;
        }
        let title = clean_title(g.get("title").and_then(|t| t.as_str()).unwrap_or(""));
        let mut subjects = Vec::new();
        for key in ["banner", "subjects", "customData", "rankingData", "rankingListData", "rankings", "verticalStyleSubjects", "playListData"] {
            if let Some(child) = g.get(key) {
                collect_subjects(child, &mut subjects, 0);
            }
        }
        let mut seen = HashSet::new();
        let items: Vec<Card> = subjects
            .into_iter()
            .filter(|s| is_video_subject(s))
            .filter_map(moviebox_subject_json_to_catalog_item)
            .filter(|c| c.poster_url.is_some())
            .filter(|c| seen.insert(c.id.value.clone()))
            .map(Card::from)
            .collect();
        if items.len() < 3 {
            continue;
        }
        let is_banner = kind == "BANNER" && !hero_seen;
        if is_banner {
            hero_seen = true;
        }
        let lower = title.to_lowercase();
        if lower.contains("short") || lower.contains("cricket") || lower.contains("wwe") {
            continue;
        }
        rows.push(HomeRow {
            title: if is_banner { "Featured".into() } else { title },
            kind: if is_banner { "hero".into() } else { "row".into() },
            items,
        });
    }
    rows
}

#[tauri::command]
pub async fn home(state: State<'_, AppState>, tab: String) -> CmdResult<Vec<HomeRow>> {
    if let Some(rows) = state.home_cache.lock().await.get(&tab).filter(|(t, _)| t.elapsed().as_secs() < 600).map(|(_, r)| r.clone()) {
        return Ok(rows);
    }
    let payload = state.service.client.get_homepage(&tab, 1).await.map_err(friendly)?;
    let rows = parse_home(&payload);
    if rows.is_empty() {
        return Err("Couldn't load titles right now. Please check your internet connection.".into());
    }
    state.home_cache.lock().await.insert(tab, (std::time::Instant::now(), rows.clone()));
    Ok(rows)
}

#[tauri::command]
pub async fn search(state: State<'_, AppState>, query: String, page: usize, filter: String) -> CmdResult<Vec<Card>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let items = state
        .service
        .search_typed(ProviderKind::MovieBox, q, page.max(1))
        .await
        .map_err(|e| e.user_message(ProviderKind::MovieBox))?;
    Ok(items
        .into_iter()
        .filter(|c| match filter.as_str() {
            "movie" => c.media_type == MediaType::Movie,
            "series" => c.media_type == MediaType::Series,
            _ => true,
        })
        .map(Card::from)
        .collect())
}

#[tauri::command]
pub async fn suggest(state: State<'_, AppState>, query: String) -> CmdResult<Vec<String>> {
    if query.trim().len() < 2 {
        return Ok(vec![]);
    }
    let mut out: Vec<String> = state.service.suggest(query.trim()).await.unwrap_or_default().into_iter().map(|s| clean_title(&s)).collect();
    let mut seen = HashSet::new();
    out.retain(|s| seen.insert(s.to_lowercase()));
    out.truncate(8);
    Ok(out)
}

#[tauri::command]
pub async fn details(state: State<'_, AppState>, id: String) -> CmdResult<DetailsDto> {
    let d = state
        .service
        .details_typed(ProviderKind::MovieBox, &id)
        .await
        .map_err(|e| friendly(e.user_message(ProviderKind::MovieBox)))?;
    let fav = {
        let favs = state.favorites.lock().await;
        favs.items.iter().any(|f| f.subject_id == d.id.value)
    };
    state.details_cache.lock().await.insert(id, d.clone());
    Ok(DetailsDto::from_details(&d, fav))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_groups_into_rows() {
        let payload = serde_json::json!({"items": [
            {"type": "BANNER", "title": "IN Banner", "banner": {"banners": [
                {"subject": {"subjectId": "1", "title": "A", "subjectType": 1, "cover": {"url": "u"}}},
                {"subject": {"subjectId": "2", "title": "B", "subjectType": 2, "cover": {"url": "u"}}},
                {"subject": {"subjectId": "3", "title": "C", "subjectType": 1, "cover": {"url": "u"}}}
            ]}},
            {"type": "FILTER", "title": "Categories"},
            {"type": "SUBJECTS_MOVIE", "title": "🔥Trending Now", "subjects": [
                {"subjectId": "4", "title": "D", "subjectType": 1, "cover": {"url": "u"}},
                {"subjectId": "5", "title": "E", "subjectType": 1, "cover": {"url": "u"}},
                {"subjectId": "6", "title": "F", "subjectType": 6, "cover": {"url": "u"}},
                {"subjectId": "7", "title": "G", "subjectType": 2, "cover": {"url": "u"}}
            ]}
        ]});
        let rows = parse_home(&payload);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, "hero");
        assert_eq!(rows[1].title, "Trending Now");
        assert_eq!(rows[1].items.len(), 3);
    }
}
