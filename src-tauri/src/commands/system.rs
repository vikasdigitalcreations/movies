use crate::core::settings::{self, GuiSettings};
use crate::core::types::CmdResult;
use crate::state::AppState;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{Manager, State};

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> CmdResult<GuiSettings> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
pub async fn settings_set(state: State<'_, AppState>, value: GuiSettings) -> CmdResult<GuiSettings> {
    let mut v = value;
    v.simultaneous_downloads = v.simultaneous_downloads.clamp(1, 5);
    v.seek_step = v.seek_step.clamp(1, 120);
    v.ui_zoom = v.ui_zoom.clamp(0.6, 2.0);
    {
        let mut s = state.settings.write().await;
        *s = v.clone();
    }
    settings::save(&v);
    state.downloads.kick();
    Ok(v)
}

pub fn download_root(custom: Option<&str>) -> PathBuf {
    match custom.filter(|s| !s.trim().is_empty()) {
        Some(dir) => {
            let p = PathBuf::from(dir);
            let is_mb = p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("MovieBox")).unwrap_or(false);
            if is_mb { p } else { p.join("MovieBox") }
        }
        None => dirs::download_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("MovieBox"),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub version: String,
    pub download_dir: String,
    pub log_dir: String,
    pub screenshot_dir: String,
    pub free_bytes: Option<u64>,
}

fn existing_ancestor(p: &Path) -> PathBuf {
    let mut cur = p.to_path_buf();
    while !cur.exists() {
        match cur.parent() {
            Some(parent) => cur = parent.to_path_buf(),
            None => break,
        }
    }
    cur
}

pub fn free_space(p: &Path) -> Option<u64> {
    fs2::available_space(existing_ancestor(p)).ok()
}

pub fn screenshot_dir() -> PathBuf {
    let d = dirs::picture_dir().unwrap_or_else(|| PathBuf::from(".")).join("MovieBox");
    let _ = std::fs::create_dir_all(&d);
    d
}

#[tauri::command]
pub async fn system_info(app: tauri::AppHandle, state: State<'_, AppState>) -> CmdResult<SystemInfo> {
    let s = state.settings.read().await.clone();
    let dl = download_root(s.download_dir.as_deref());
    let log_dir = app.path().app_log_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(SystemInfo {
        version: app.package_info().version.to_string(),
        free_bytes: free_space(&dl),
        download_dir: dl.to_string_lossy().into_owned(),
        log_dir,
        screenshot_dir: screenshot_dir().to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub async fn free_space_for(path: String) -> CmdResult<Option<u64>> {
    Ok(free_space(Path::new(&path)))
}

#[tauri::command]
pub async fn open_folder(path: String) -> CmdResult<()> {
    let p = PathBuf::from(&path);
    let target = if p.is_file() { p.parent().map(|x| x.to_path_buf()).unwrap_or(p) } else { p };
    let _ = std::fs::create_dir_all(&target);
    if p_is_file(&path) {
        std::process::Command::new("explorer").arg(format!("/select,{}", path)).spawn().map_err(|e| e.to_string())?;
    } else {
        std::process::Command::new("explorer").arg(target).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn p_is_file(p: &str) -> bool {
    Path::new(p).is_file()
}

#[tauri::command]
pub async fn open_logs(app: tauri::AppHandle) -> CmdResult<()> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(&dir);
    std::process::Command::new("explorer").arg(dir).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn clear_cache(state: State<'_, AppState>) -> CmdResult<()> {
    state.home_cache.lock().await.clear();
    state.details_cache.lock().await.clear();
    let dir = moviebox_tui::config::cache_dir();
    if dir.exists() {
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
    Ok(())
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Wants from the player (display + system) and from downloads (system only).
static PLAYER_AWAKE: AtomicBool = AtomicBool::new(false);
static DOWNLOADS_AWAKE: AtomicBool = AtomicBool::new(false);
static AWAKE_THREAD: OnceLock<()> = OnceLock::new();

/// Windows keeps the execution state per thread, so one dedicated thread owns it.
fn ensure_awake_thread() {
    AWAKE_THREAD.get_or_init(|| {
        std::thread::Builder::new()
            .name("keep-awake".into())
            .spawn(|| loop {
                #[cfg(windows)]
                unsafe {
                    use windows_sys::Win32::System::Power::*;
                    let player = PLAYER_AWAKE.load(Ordering::Relaxed);
                    let downloads = DOWNLOADS_AWAKE.load(Ordering::Relaxed);
                    let mut flags = ES_CONTINUOUS;
                    if player || downloads {
                        flags |= ES_SYSTEM_REQUIRED;
                    }
                    if player {
                        flags |= ES_DISPLAY_REQUIRED;
                    }
                    SetThreadExecutionState(flags);
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            })
            .ok();
    });
}

/// Keep the display and PC awake while the player is open.
#[tauri::command]
pub fn keep_awake(display: bool, system: bool) {
    PLAYER_AWAKE.store(display || system, Ordering::Relaxed);
    ensure_awake_thread();
}

/// Downloads keep the PC from sleeping (the screen may still turn off).
pub fn set_awake(_display: bool, system: bool) {
    DOWNLOADS_AWAKE.store(system, Ordering::Relaxed);
    ensure_awake_thread();
}

#[tauri::command]
pub async fn check_online(state: State<'_, AppState>) -> CmdResult<bool> {
    let ok = tokio::time::timeout(
        std::time::Duration::from_secs(6),
        state.service.http_client().head("https://www.google.com/generate_204").send(),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false);
    Ok(ok)
}
