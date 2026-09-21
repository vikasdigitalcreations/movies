use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GuiSettings {
    pub preferred_quality: u64,
    pub subtitle_language: String,
    pub autoplay_next: bool,
    pub remember_speed: bool,
    pub seek_step: u64,
    pub download_dir: Option<String>,
    pub simultaneous_downloads: usize,
    pub subtitle_size: u32,
    pub subtitle_background: bool,
    pub volume: f64,
    pub last_speed: f64,
    pub night_mode: bool,
    pub ui_zoom: f64,
    pub tour_done: bool,
    /// Salted SHA-256 of the PIN that unlocks locked addon sections. `None` = no PIN set.
    /// The PIN itself is never stored, and it guards nothing but this app's own UI.
    pub pin_hash: Option<String>,
    pub pin_salt: Option<String>,
    /// Manifest URLs of addons whose catalogues stay hidden until the PIN is entered.
    pub locked_addons: Vec<String>,
    /// Whether the Adults section is shown at all. Only ever true alongside a PIN.
    pub adult_enabled: bool,
    /// Look for a title on the official YouTube channels of film distributors when no
    /// other source has it. Needs the yt-dlp helper, fetched the first time it is used.
    pub youtube_source: bool,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            preferred_quality: 0,
            subtitle_language: "English".into(),
            autoplay_next: true,
            remember_speed: false,
            seek_step: 10,
            download_dir: None,
            simultaneous_downloads: 2,
            subtitle_size: 46,
            subtitle_background: false,
            volume: 100.0,
            last_speed: 1.0,
            night_mode: false,
            ui_zoom: 1.0,
            tour_done: false,
            pin_hash: None,
            pin_salt: None,
            locked_addons: Vec::new(),
            adult_enabled: false,
            youtube_source: true,
        }
    }
}

pub fn app_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("MovieBox");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn path() -> PathBuf {
    app_dir().join("gui_settings.json")
}

pub fn load() -> GuiSettings {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(settings: &GuiSettings) {
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let tmp = path().with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(tmp, path());
        }
    }
}
