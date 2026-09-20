//! Commands for the Adults section. Every one refuses unless the caller says the PIN
//! has been accepted this session.
//!
//! The PIN is a household lock, not a security boundary: anyone who can edit
//! `gui_settings.json` can clear it. It exists so the section cannot be opened by
//! accident, or by whoever else uses the machine, which is what it was asked for.

use crate::core::adult::{self, Playback, Source};
use crate::core::types::{Card, CmdResult, StreamDto};
use crate::state::AppState;
use serde::Serialize;
use tauri::State;

fn source_from(name: &str) -> Result<Source, String> {
    match name.to_ascii_lowercase().as_str() {
        "eporner" => Ok(Source::Eporner),
        "redgifs" => Ok(Source::RedGifs),
        _ => Err("Unknown source.".into()),
    }
}

/// The section is off unless it has been switched on *and* a PIN exists to guard it.
async fn require_unlocked(state: &State<'_, AppState>, unlocked: bool) -> Result<(), String> {
    let s = state.settings.read().await;
    if !s.adult_enabled {
        return Err("The Adults section is switched off in Settings.".into());
    }
    if s.pin_hash.is_none() {
        return Err("Set a PIN in Settings before opening this section.".into());
    }
    drop(s);
    if !unlocked {
        return Err("Enter the PIN to open this section.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn adult_search(state: State<'_, AppState>, source: String, query: String, page: usize, unlocked: bool) -> CmdResult<Vec<Card>> {
    require_unlocked(&state, unlocked).await?;
    adult::search(source_from(&source)?, &query, page).await
}

/// How the UI should open one item: our own player, or a webview showing the site's
/// embed for sources that will not serve their files anywhere else.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdultPlayback {
    pub kind: &'static str,
    pub stream: Option<StreamDto>,
    pub embed_url: Option<String>,
    pub source: String,
}

#[tauri::command]
pub async fn adult_playback(state: State<'_, AppState>, id: String, unlocked: bool) -> CmdResult<AdultPlayback> {
    require_unlocked(&state, unlocked).await?;
    let source = adult::decode_id(&id).map(|(s, _)| s).ok_or("That clip's link is not valid.")?;
    Ok(match adult::playback(&id).await? {
        Playback::Stream(s) => AdultPlayback {
            kind: "stream",
            stream: Some(*s),
            embed_url: None,
            source: source.label().into(),
        },
        Playback::Embed(u) => AdultPlayback {
            kind: "embed",
            stream: None,
            embed_url: Some(u),
            source: source.label().into(),
        },
    })
}

/// Turn the whole section on or off. Turning it on requires a PIN to already exist,
/// so it can never appear unguarded.
#[tauri::command]
pub async fn adult_set_enabled(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    let mut s = state.settings.write().await;
    if enabled && s.pin_hash.is_none() {
        return Err("Set a PIN first — the section is only shown behind one.".into());
    }
    s.adult_enabled = enabled;
    let copy = s.clone();
    drop(s);
    crate::core::settings::save(&copy);
    Ok(())
}

/// Whether the sidebar should show the section at all.
#[tauri::command]
pub async fn adult_is_enabled(state: State<'_, AppState>) -> CmdResult<bool> {
    let s = state.settings.read().await;
    Ok(s.adult_enabled && s.pin_hash.is_some())
}
