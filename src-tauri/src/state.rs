use crate::core::downloads::DownloadManager;
use crate::core::settings::{self, GuiSettings};
use crate::core::types::HomeRow;
use moviebox_tui::favorites::FavoritesManager;
use moviebox_tui::history::HistoryManager;
use moviebox_tui::providers::MediaDetails;
use moviebox_tui::service::MovieBoxService;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

pub struct AppState {
    pub service: MovieBoxService,
    pub history: Mutex<HistoryManager>,
    pub favorites: Mutex<FavoritesManager>,
    pub settings: Arc<RwLock<GuiSettings>>,
    pub home_cache: Mutex<HashMap<String, (Instant, Vec<HomeRow>)>>,
    pub details_cache: Mutex<HashMap<String, MediaDetails>>,
    pub downloads: Arc<DownloadManager>,
}

impl AppState {
    pub fn new(app: tauri::AppHandle) -> Self {
        let service = MovieBoxService::new();
        let settings = Arc::new(RwLock::new(settings::load()));
        let downloads = DownloadManager::new(app, service.clone(), settings.clone());
        Self {
            service,
            history: Mutex::new(HistoryManager::new()),
            favorites: Mutex::new(FavoritesManager::new()),
            settings,
            home_cache: Mutex::new(HashMap::new()),
            details_cache: Mutex::new(HashMap::new()),
            downloads,
        }
    }
}
