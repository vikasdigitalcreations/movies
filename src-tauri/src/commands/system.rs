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
        // The PIN and the lock list are owned by their own commands. Carrying them
        // through the ordinary settings round trip would hand the hash to the UI and
        // let any stale copy of the settings object wipe them.
        v.pin_hash = s.pin_hash.clone();
        v.pin_salt = s.pin_salt.clone();
        v.locked_addons = s.locked_addons.clone();
        v.adult_enabled = s.adult_enabled;
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


// ---------------------------------------------------------------- PIN

/// Hash a PIN with its salt. Not a secret-keeping measure -- anyone with the machine can
/// edit `gui_settings.json` -- but it keeps the PIN itself off disk and out of the UI.
fn pin_digest(pin: &str, salt: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(salt.as_bytes());
    h.update(pin.as_bytes());
    format!("{:x}", h.finalize())
}

fn new_salt() -> String {
    use sha2::{Digest, Sha256};
    let seed = format!(
        "{:?}-{:?}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default(),
        std::process::id()
    );
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    format!("{:x}", h.finalize())
}

/// Whether a PIN has been set. Never returns the PIN or its hash.
#[tauri::command]
pub async fn pin_is_set(state: State<'_, AppState>) -> CmdResult<bool> {
    Ok(state.settings.read().await.pin_hash.is_some())
}

/// Set the PIN, or change it. Changing one requires the current PIN.
#[tauri::command]
pub async fn pin_set(state: State<'_, AppState>, pin: String, current: Option<String>) -> CmdResult<()> {
    let pin = pin.trim().to_string();
    if pin.len() < 4 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err("Choose a PIN of at least 4 digits.".into());
    }
    let mut s = state.settings.write().await;
    if let (Some(hash), Some(salt)) = (s.pin_hash.clone(), s.pin_salt.clone()) {
        let ok = current.map(|c| pin_digest(c.trim(), &salt) == hash).unwrap_or(false);
        if !ok {
            return Err("That isn't the current PIN.".into());
        }
    }
    let salt = new_salt();
    s.pin_hash = Some(pin_digest(&pin, &salt));
    s.pin_salt = Some(salt);
    let copy = s.clone();
    drop(s);
    settings::save(&copy);
    Ok(())
}

#[tauri::command]
pub async fn pin_verify(state: State<'_, AppState>, pin: String) -> CmdResult<bool> {
    let s = state.settings.read().await;
    match (s.pin_hash.as_ref(), s.pin_salt.as_ref()) {
        (Some(hash), Some(salt)) => Ok(&pin_digest(pin.trim(), salt) == hash),
        // No PIN set means nothing is locked, so there is nothing to refuse.
        _ => Ok(true),
    }
}

/// Remove the PIN. Unlocks every locked addon, since nothing would guard them.
#[tauri::command]
pub async fn pin_clear(state: State<'_, AppState>, current: String) -> CmdResult<()> {
    let mut s = state.settings.write().await;
    if let (Some(hash), Some(salt)) = (s.pin_hash.clone(), s.pin_salt.clone()) {
        if pin_digest(current.trim(), &salt) != hash {
            return Err("That isn't the current PIN.".into());
        }
    }
    s.pin_hash = None;
    s.pin_salt = None;
    s.locked_addons.clear();
    // Nothing would guard the section any more, so close it too.
    s.adult_enabled = false;
    let copy = s.clone();
    drop(s);
    settings::save(&copy);
    Ok(())
}

/// Hide or show one addon's catalogues behind the PIN.
#[tauri::command]
pub async fn addon_set_locked(state: State<'_, AppState>, url: String, locked: bool) -> CmdResult<()> {
    let mut s = state.settings.write().await;
    if locked && s.pin_hash.is_none() {
        return Err("Set a PIN first, otherwise there is nothing to unlock it with.".into());
    }
    s.locked_addons.retain(|u| u != &url);
    if locked {
        s.locked_addons.push(url);
    }
    let copy = s.clone();
    drop(s);
    settings::save(&copy);
    Ok(())
}
