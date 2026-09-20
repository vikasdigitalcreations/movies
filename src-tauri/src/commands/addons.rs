//! Stremio addons: a third source behind MovieBox and 4KHDHub.
//!
//! An addon is a URL the user pastes. MovieBox and 4KHDHub are fixed providers that we
//! have to follow whenever they change; addons are maintained by whoever wrote them, so
//! they keep working without us shipping a new version. The vendored crate already
//! speaks the protocol -- this module only exposes it to the UI and bridges the ids.
//!
//! The bridge is the awkward part. Addons key on IMDb ids (`tt1375666`) and MovieBox
//! uses its own numbers, so a title has to be looked up by name first. Cinemeta, the
//! official free metadata addon, answers exactly that question and is installed by
//! default for this reason.

use crate::core::types::{CmdResult, StreamDto};
use moviebox_tui::providers::addons::{aggregate_streams, AddonClient, InstalledAddon};
use moviebox_tui::providers::Release;
use serde::Serialize;
use std::time::Duration;

/// One installed addon, as the Settings page shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonDto {
    pub manifest_url: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub enabled: bool,
    pub provides_catalog: bool,
    pub provides_meta: bool,
    pub provides_stream: bool,
    /// Cinemeta is what the id bridge runs on, so the UI keeps it from being removed.
    pub core: bool,
}

impl From<&InstalledAddon> for AddonDto {
    fn from(a: &InstalledAddon) -> Self {
        AddonDto {
            manifest_url: a.manifest_url.clone(),
            name: a.name.clone(),
            version: a.version.clone(),
            description: a.description.clone(),
            enabled: a.enabled,
            provides_catalog: a.provides_catalog,
            provides_meta: a.provides_meta,
            provides_stream: a.provides_stream,
            core: a.is_core(),
        }
    }
}

fn list_installed() -> Vec<InstalledAddon> {
    moviebox_tui::config::load_addons()
}

#[tauri::command]
pub async fn addons_list() -> CmdResult<Vec<AddonDto>> {
    Ok(list_installed().iter().map(AddonDto::from).collect())
}

/// Install an addon from its manifest URL.
///
/// The manifest is fetched before anything is saved, so a typo or a dead host is
/// reported as such instead of leaving a broken entry behind. A Stremio install link
/// (`stremio://...`) is accepted too, since that is what most addons hand out.
#[tauri::command]
pub async fn addons_add(url: String) -> CmdResult<AddonDto> {
    let manifest_url = AddonClient::normalize_manifest_url(url.trim());
    if manifest_url.is_empty() {
        return Err("Paste the addon's URL first.".into());
    }
    let mut installed = list_installed();
    if installed.iter().any(|a| a.manifest_url == manifest_url) {
        return Err("That addon is already installed.".into());
    }
    let client = AddonClient::new();
    let manifest = tokio::time::timeout(Duration::from_secs(20), client.fetch_manifest(&manifest_url))
        .await
        .map_err(|_| "That addon didn't respond. Check the link and your connection.".to_string())?
        .map_err(|_| "That doesn't look like a working addon link.".to_string())?;

    let addon = InstalledAddon::from_manifest(manifest_url, &manifest);
    if !addon.provides_stream && !addon.provides_catalog && !addon.provides_meta {
        return Err("That addon doesn't offer anything MovieBox can use.".into());
    }
    let dto = AddonDto::from(&addon);
    installed.push(addon);
    moviebox_tui::config::save_addons(&installed);
    Ok(dto)
}

#[tauri::command]
pub async fn addons_remove(url: String) -> CmdResult<()> {
    let mut installed = list_installed();
    if installed.iter().any(|a| a.manifest_url == url && a.is_core()) {
        return Err("Cinemeta is needed to match titles to addons, so it can't be removed.".into());
    }
    installed.retain(|a| a.manifest_url != url);
    moviebox_tui::config::save_addons(&installed);
    Ok(())
}

#[tauri::command]
pub async fn addons_toggle(url: String, enabled: bool) -> CmdResult<()> {
    let mut installed = list_installed();
    for a in installed.iter_mut() {
        if a.manifest_url == url {
            a.enabled = enabled;
        }
    }
    moviebox_tui::config::save_addons(&installed);
    Ok(())
}

/// Find a title's IMDb id through Cinemeta (or any other enabled catalog addon).
///
/// Matching is deliberately strict -- same normalised title, and the same year when we
/// know it. A near miss here would play a different film, which is worse than playing
/// nothing at all.
async fn imdb_id_for(client: &AddonClient, title: &str, year: Option<&str>, is_series: bool) -> Option<String> {
    let want = super::streams::norm_title(title);
    let kind = if is_series { "series" } else { "movie" };
    for addon in list_installed().iter().filter(|a| a.enabled && (a.provides_catalog || a.provides_meta)) {
        let base = AddonClient::base_addon_url(&addon.manifest_url);
        let Ok(Ok(metas)) = tokio::time::timeout(Duration::from_secs(12), client.fetch_catalog_search(&base, kind, "top", title)).await else {
            continue;
        };
        let hit = metas.into_iter().find(|m| {
            let name = m.title.as_deref().unwrap_or(&m.name);
            super::streams::norm_title(name) == want
                && year.map(|y| m.release_info.as_deref().unwrap_or("").contains(y)).unwrap_or(true)
        });
        if let Some(m) = hit {
            if m.id.starts_with("tt") {
                return Some(m.id);
            }
        }
    }
    None
}

/// Every playable stream the enabled addons offer for one title.
///
/// Magnet and torrent links are dropped by the vendored adapter, so what comes back is
/// always something the player can open directly.
pub async fn addon_streams_for(title: &str, year: Option<&str>, is_series: bool, season: usize, episode: usize) -> Result<Vec<StreamDto>, String> {
    let installed = list_installed();
    if !installed.iter().any(|a| a.enabled && a.provides_stream) {
        return Err("No streaming addon is set up yet.".into());
    }
    let client = AddonClient::new();
    let Some(imdb) = imdb_id_for(&client, title, year, is_series).await else {
        return Err("No addon recognised this title.".into());
    };
    let (releases, _failed): (Vec<Release>, Vec<String>) =
        tokio::time::timeout(Duration::from_secs(25), aggregate_streams(&client, &installed, &imdb, season, episode, is_series))
            .await
            .map_err(|_| "The addons took too long to answer.".to_string())?;

    let mut out: Vec<StreamDto> = releases.iter().filter_map(StreamDto::from_release).collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.height));
    if out.is_empty() {
        return Err("The addons have nothing playable for this title.".into());
    }
    Ok(out)
}

/// Settings -> Addons "Test" button, and the player's manual retry.
#[tauri::command]
pub async fn addon_streams(title: String, year: Option<String>, is_series: bool, season: usize, episode: usize) -> CmdResult<Vec<StreamDto>> {
    addon_streams_for(&title, year.as_deref(), is_series, season, episode).await
}
