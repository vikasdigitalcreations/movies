//! Download queue on top of MovieBox-Tui's resumable `download::download()`.
//! Persisted to downloads.json so the queue survives restarts.
use crate::core::settings::{app_dir, GuiSettings};
use crate::core::stream_pool::pick_for_quality;
use moviebox_tui::download::{download, safe_file_stem, DownloadOutcome};
use moviebox_tui::service::MovieBoxService;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Queued,
    Downloading,
    Paused,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub subject_id: String,
    pub title: String,
    pub year: Option<String>,
    pub poster: Option<String>,
    pub media_type: String,
    pub season: usize,
    pub episode: usize,
    pub abs_index: usize,
    pub episode_title: Option<String>,
    pub height: u64,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub subtitle_url: Option<String>,
    pub subtitle_lang: Option<String>,
    pub path: String,
    pub status: Status,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: f64,
    pub error: Option<String>,
    pub added: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub subject_id: String,
    pub title: String,
    pub year: Option<String>,
    pub poster: Option<String>,
    pub media_type: String,
    pub season: usize,
    pub episode: usize,
    pub abs_index: usize,
    pub episode_title: Option<String>,
    pub height: u64,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub size: Option<u64>,
    pub subtitle_url: Option<String>,
    pub subtitle_lang: Option<String>,
}

pub struct DownloadManager {
    app: tauri::AppHandle,
    service: MovieBoxService,
    settings: Arc<RwLock<GuiSettings>>,
    tasks: Mutex<Vec<Task>>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// ids whose worker should delete files when it stops
    removing: Mutex<Vec<String>>,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
}

fn store_path() -> PathBuf {
    app_dir().join("downloads.json")
}

/// `MovieBox\Title (2024)\Title (2024) 1080p.mp4` or
/// `MovieBox\Show\Season 01\Show - S01E03 - Episode Title.mp4`
pub fn build_path(root: &std::path::Path, t: &NewTask, ext: &str) -> PathBuf {
    let title = safe_file_stem(&t.title);
    if t.media_type == "series" {
        let ep_title = t
            .episode_title
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!(" - {}", safe_file_stem(s)))
            .unwrap_or_default();
        root.join(&title)
            .join(format!("Season {:02}", t.season))
            .join(format!("{title} - S{:02}E{:02}{ep_title}.{ext}", t.season, t.episode))
    } else {
        let named = match t.year.as_deref().filter(|y| !y.is_empty()) {
            Some(y) => format!("{title} ({y})"),
            None => title.clone(),
        };
        let q = if t.height > 0 { format!(" {}p", t.height) } else { String::new() };
        root.join(&named).join(format!("{named}{q}.{ext}"))
    }
}

fn ext_from_url(url: &str) -> String {
    let base = url.split('?').next().unwrap_or(url);
    base.rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| matches!(e.as_str(), "mp4" | "mkv" | "webm" | "avi" | "mov" | "m4v"))
        .unwrap_or_else(|| "mp4".into())
}

impl DownloadManager {
    pub fn new(app: tauri::AppHandle, service: MovieBoxService, settings: Arc<RwLock<GuiSettings>>) -> Arc<Self> {
        let mut tasks: Vec<Task> = std::fs::read_to_string(store_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        for t in tasks.iter_mut() {
            if t.status == Status::Downloading {
                t.status = Status::Queued;
            }
            t.speed = 0.0;
        }
        Arc::new(Self {
            app,
            service,
            settings,
            tasks: Mutex::new(tasks),
            cancels: Mutex::new(HashMap::new()),
            removing: Mutex::new(Vec::new()),
        })
    }

    fn persist(&self) {
        let tasks = self.tasks.lock().unwrap().clone();
        if let Ok(json) = serde_json::to_string_pretty(&tasks) {
            let tmp = store_path().with_extension("json.tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(tmp, store_path());
            }
        }
    }

    fn emit(&self, task: &Task) {
        let _ = self.app.emit("download://progress", task);
    }

    pub fn list(&self) -> Vec<Task> {
        self.tasks.lock().unwrap().clone()
    }

    fn update<F: FnOnce(&mut Task)>(&self, id: &str, f: F) -> Option<Task> {
        let mut tasks = self.tasks.lock().unwrap();
        let t = tasks.iter_mut().find(|t| t.id == id)?;
        f(t);
        Some(t.clone())
    }

    pub async fn add(self: &Arc<Self>, new: NewTask) -> Result<Task, String> {
        if crate::core::stream_pool::is_notice_url(&new.url) {
            return Err("This title can't be downloaded right now — MovieBox only offers its \"update the app\" clip instead of the file. You can still watch it online.".into());
        }
        let root = crate::commands::system::download_root(self.settings.read().await.download_dir.as_deref());
        let path = build_path(&root, &new, &ext_from_url(&new.url));
        {
            let tasks = self.tasks.lock().unwrap();
            if let Some(existing) = tasks.iter().find(|t| t.path == path.to_string_lossy()) {
                if existing.status != Status::Failed {
                    return Err(if existing.status == Status::Done {
                        "You've already downloaded this.".into()
                    } else {
                        "This is already in your downloads.".into()
                    });
                }
            }
        }
        if let (Some(size), Some(free)) = (new.size, crate::commands::system::free_space(&root)) {
            if size + 200 * 1024 * 1024 > free {
                return Err(format!(
                    "Not enough free space. This needs {} but only {} is free.",
                    human(size),
                    human(free)
                ));
            }
        }
        let task = Task {
            id: format!("{}-{}-{}-{}", new.subject_id, new.season, new.episode, now()),
            subject_id: new.subject_id,
            title: new.title,
            year: new.year,
            poster: new.poster,
            media_type: new.media_type,
            season: new.season,
            episode: new.episode,
            abs_index: new.abs_index,
            episode_title: new.episode_title,
            height: new.height,
            url: new.url,
            headers: new.headers,
            subtitle_url: new.subtitle_url,
            subtitle_lang: new.subtitle_lang,
            path: path.to_string_lossy().into_owned(),
            status: Status::Queued,
            downloaded: 0,
            total: new.size,
            speed: 0.0,
            error: None,
            added: now(),
        };
        {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.retain(|t| !(t.path == task.path && t.status == Status::Failed));
            tasks.push(task.clone());
        }
        self.persist();
        self.emit(&task);
        self.kick();
        Ok(task)
    }

    pub fn pause(self: &Arc<Self>, id: &str) {
        if let Some(flag) = self.cancels.lock().unwrap().get(id) {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(t) = self.update(id, |t| {
            if t.status != Status::Done {
                t.status = Status::Paused;
                t.speed = 0.0;
            }
        }) {
            self.emit(&t);
        }
        self.persist();
        self.kick();
    }

    pub fn resume(self: &Arc<Self>, id: &str) {
        if let Some(t) = self.update(id, |t| {
            if matches!(t.status, Status::Paused | Status::Failed) {
                t.status = Status::Queued;
                t.error = None;
            }
        }) {
            self.emit(&t);
        }
        self.persist();
        self.kick();
    }

    /// Cancel (active) or delete (finished). Removes partial files; `delete_file` also removes a finished video.
    pub fn remove(self: &Arc<Self>, id: &str, delete_file: bool) {
        let running = self.cancels.lock().unwrap().get(id).cloned();
        let task = {
            let mut tasks = self.tasks.lock().unwrap();
            let pos = tasks.iter().position(|t| t.id == id);
            pos.map(|p| tasks.remove(p))
        };
        if let Some(flag) = running {
            self.removing.lock().unwrap().push(id.to_string());
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(t) = task {
            let p = PathBuf::from(&t.path);
            let finished = t.status == Status::Done;
            if !finished || delete_file {
                // unfinished: drop partial data and its subtitle; finished + delete: drop everything
                cleanup_files(&p, finished && delete_file, true);
            }
            let _ = self.app.emit("download://removed", &t.id);
        }
        self.persist();
        self.kick();
    }

    pub fn active_count(&self) -> usize {
        self.tasks.lock().unwrap().iter().filter(|t| matches!(t.status, Status::Queued | Status::Downloading)).count()
    }

    /// Start queued tasks up to the configured limit.
    pub fn kick(self: &Arc<Self>) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let limit = this.settings.read().await.simultaneous_downloads.max(1);
            let to_start: Vec<Task> = {
                let mut tasks = this.tasks.lock().unwrap();
                let running = tasks.iter().filter(|t| t.status == Status::Downloading).count();
                let mut picked = Vec::new();
                for t in tasks.iter_mut() {
                    if running + picked.len() >= limit {
                        break;
                    }
                    if t.status == Status::Queued {
                        t.status = Status::Downloading;
                        picked.push(t.clone());
                    }
                }
                picked
            };
            crate::commands::system::set_awake(false, this.active_count() > 0);
            for t in to_start {
                this.emit(&t);
                let worker = this.clone();
                tauri::async_runtime::spawn(async move { worker.run(t).await });
            }
        });
    }

    async fn refresh_url(&self, t: &Task) -> Option<(String, Vec<(String, String)>)> {
        // Signed links expire; fetch a fresh one for the same quality. A download that
        // started on another source has to be refreshed from that source, not MovieBox,
        // or a worker link dying halfway would end the download instead of resuming it.
        if let Ok(pool) = crate::commands::streams::collect_streams(&self.service, &t.subject_id, t.season, t.episode, t.abs_index).await {
            let direct: Vec<_> = pool
                .into_iter()
                .filter(|r| r.direct_url().map(|u| !crate::core::stream_pool::is_notice_url(u)).unwrap_or(false))
                .collect();
            if let Some(rel) = direct.iter().find(|r| r.resolution_u64() == t.height).or_else(|| pick_for_quality(&direct, t.height)) {
                if let Some(m) = rel.mirrors.first() {
                    return Some((m.resolver_url.clone(), m.headers.clone()));
                }
            }
        }
        let alt = crate::commands::streams::other_source_streams(&self.service, &t.title, t.year.as_deref(), t.season, t.episode, t.height).await.ok()?;
        // Only single files can be downloaded; a film the player joins from parts cannot.
        let s = alt.iter().filter(|s| s.downloadable).find(|s| s.height == t.height).or_else(|| alt.iter().find(|s| s.downloadable))?;
        Some((s.url.clone(), s.headers.clone()))
    }

    async fn run(self: Arc<Self>, task: Task) {
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.lock().unwrap().insert(task.id.clone(), cancel.clone());
        let dest = PathBuf::from(&task.path);
        if let Some(parent) = dest.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let mut url = task.url.clone();
        let mut headers = task.headers.clone();

        // Subtitle next to the video (best effort).
        if let Some(sub) = task.subtitle_url.clone() {
            let lang = task
                .subtitle_lang
                .as_deref()
                .and_then(moviebox_tui::providers::moviebox::title::language_to_code)
                .unwrap_or("en");
            let sub_ext = sub
                .split('?')
                .next()
                .and_then(|b| b.rsplit('.').next())
                .map(|e| e.to_ascii_lowercase())
                .filter(|e| matches!(e.as_str(), "srt" | "vtt" | "ass" | "ssa"))
                .unwrap_or_else(|| "srt".into());
            let sub_path = dest.with_extension(format!("{lang}.{sub_ext}"));
            if !sub_path.exists() {
                if let Ok(Ok(resp)) = tokio::time::timeout(std::time::Duration::from_secs(20), self.service.http_client().get(&sub).header("User-Agent", self.service.client.user_agent()).send()).await {
                    if let Ok(bytes) = resp.bytes().await {
                        let _ = tokio::fs::write(&sub_path, bytes).await;
                    }
                }
            }
        }

        // MovieBox only serves DASH now: video and audio arrive as separate segment
        // lists that the bundled ffmpeg joins back together once both are here.
        if crate::core::dash::is_dash_url(&url) {
            let client = build_client(&headers, self.service.client.user_agent());
            let id = task.id.clone();
            let this = self.clone();
            let res = crate::core::dash::download_dash(&client, &url, &dest, task.height, cancel.clone(), move |downloaded, speed| {
                if let Some(t) = this.update(&id, |t| {
                    t.downloaded = downloaded;
                    t.speed = speed;
                    // The size the API reports covers the video only; once the audio
                    // track pushes past it, trust what has actually arrived.
                    if t.total.is_some_and(|total| downloaded > total) {
                        t.total = Some(downloaded);
                    }
                }) {
                    this.emit(&t);
                }
            })
            .await
            .map(|o| match o {
                crate::core::dash::Outcome::Completed { bytes } => DownloadOutcome::Completed { bytes },
                crate::core::dash::Outcome::Paused { bytes } => DownloadOutcome::Paused { bytes },
            });
            self.finish(task, dest, res.map_err(|e| e.to_string())).await;
            return;
        }

        let mut refreshed = false;
        let result = loop {
            let client = build_client(&headers, self.service.client.user_agent());
            let id = task.id.clone();
            let this = self.clone();
            let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(2);
            let res = download(&client, &url, &dest, cancel.clone(), move |p| {
                if last_emit.elapsed().as_millis() >= 700 {
                    last_emit = std::time::Instant::now();
                    if let Some(t) = this.update(&id, |t| {
                        t.downloaded = p.downloaded;
                        if p.total.is_some() {
                            t.total = p.total;
                        }
                        t.speed = p.bytes_per_second;
                    }) {
                        this.emit(&t);
                    }
                }
            })
            .await;
            match &res {
                Err(e) if !refreshed && !cancel.load(Ordering::SeqCst) => {
                    let msg = e.to_string();
                    if msg.contains("403") || msg.contains("404") || msg.contains("410") || msg.contains("401") {
                        refreshed = true;
                        if let Some((u, h)) = self.refresh_url(&task).await {
                            url = u;
                            headers = h;
                            let _ = self.update(&task.id, |t| {
                                t.url = url.clone();
                                t.headers = headers.clone();
                            });
                            continue;
                        }
                    }
                    break res;
                }
                _ => break res,
            }
        };
        self.finish(task, dest, result.map_err(|e| e.to_string())).await;
    }

    /// Shared ending for both download paths: cleanup, status, notification, next task.
    async fn finish(self: Arc<Self>, task: Task, dest: PathBuf, result: Result<DownloadOutcome, String>) {
        self.cancels.lock().unwrap().remove(&task.id);

        let was_removed = {
            let mut r = self.removing.lock().unwrap();
            let pos = r.iter().position(|x| x == &task.id);
            pos.map(|p| r.remove(p)).is_some()
        };
        if was_removed {
            cleanup_files(&dest, false, true);
            self.kick();
            return;
        }

        let updated = match result {
            Ok(DownloadOutcome::Completed { bytes }) => self.update(&task.id, |t| {
                t.status = Status::Done;
                t.downloaded = bytes;
                t.total = Some(bytes);
                t.speed = 0.0;
            }),
            Ok(DownloadOutcome::Paused { bytes }) => self.update(&task.id, |t| {
                t.downloaded = bytes;
                t.speed = 0.0;
                if t.status == Status::Downloading {
                    t.status = Status::Paused;
                }
            }),
            Err(e) => {
                log::warn!("download failed for {}: {e}", task.title);
                self.update(&task.id, |t| {
                    t.status = Status::Failed;
                    t.speed = 0.0;
                    t.error = Some(crate::core::types::friendly(&e));
                })
            }
        };
        self.persist();
        if let Some(t) = updated {
            self.emit(&t);
            if t.status == Status::Done {
                notify_done(&self.app, &t);
            }
        }
        self.kick();
    }
}

fn notify_done(app: &tauri::AppHandle, t: &Task) {
    use tauri_plugin_notification::NotificationExt;
    let body = if t.media_type == "series" {
        format!("{} S{:02}E{:02} is ready to watch.", t.title, t.season, t.episode)
    } else {
        format!("{} is ready to watch.", t.title)
    };
    let _ = app.notification().builder().title("Download finished").body(body).show();
}

/// Removes a download's partial data: `<file>.part`, `<file>.part.json` and segment files `<file>.part.N`.
/// `remove_video` also removes the finished video. `remove_subs` removes `<stem>.<lang>.srt|vtt|ass` next to it.
/// Empty folders left behind (episode season folder, show folder) are removed too.
fn cleanup_files(dest: &std::path::Path, remove_video: bool, remove_subs: bool) {
    let Some(dir) = dest.parent() else { return };
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = dest.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let is_part = n.starts_with(&format!("{name}.part"));
            let is_sub = remove_subs
                && n.starts_with(&format!("{stem}."))
                && (n.ends_with(".srt") || n.ends_with(".vtt") || n.ends_with(".ass") || n.ends_with(".ssa"));
            if is_part || is_sub {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    if remove_video {
        let _ = std::fs::remove_file(dest);
    }
    let _ = std::fs::remove_dir(dir);
    if let Some(up) = dir.parent() {
        let _ = std::fs::remove_dir(up);
    }
}

fn build_client(headers: &[(String, String)], fallback_ua: &str) -> reqwest::Client {
    let mut builder = moviebox_tui::net::http_client_builder().connect_timeout(std::time::Duration::from_secs(15));
    let mut map = reqwest::header::HeaderMap::new();
    let mut ua = false;
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("user-agent") {
            ua = true;
            builder = builder.user_agent(v);
        } else if let (Ok(n), Ok(val)) = (reqwest::header::HeaderName::from_bytes(k.as_bytes()), reqwest::header::HeaderValue::from_str(v)) {
            map.insert(n, val);
        }
    }
    if !ua {
        builder = builder.user_agent(fallback_ua);
    }
    builder.default_headers(map).build().unwrap_or_default()
}

pub fn human(b: u64) -> String {
    let gb = b as f64 / 1024.0 / 1024.0 / 1024.0;
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", b as f64 / 1024.0 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nt(media: &str) -> NewTask {
        NewTask {
            subject_id: "1".into(),
            title: "Show: Name?".into(),
            year: Some("2024".into()),
            poster: None,
            media_type: media.into(),
            season: 1,
            episode: 3,
            abs_index: 2,
            episode_title: Some("Pilot".into()),
            height: 1080,
            url: "https://x/a.mp4?sign=1".into(),
            headers: vec![],
            size: None,
            subtitle_url: None,
            subtitle_lang: None,
        }
    }

    #[test]
    fn tidy_paths() {
        let root = std::path::Path::new("D:/MovieBox");
        let s = build_path(root, &nt("series"), "mp4");
        assert!(s.ends_with("Show_ Name/Season 01/Show_ Name - S01E03 - Pilot.mp4"), "{s:?}");
        let m = build_path(root, &nt("movie"), "mkv");
        assert!(m.ends_with("Show_ Name (2024)/Show_ Name (2024) 1080p.mkv"), "{m:?}");
    }

    #[test]
    fn cleanup_removes_segments_and_subs() {
        let dir = std::env::temp_dir().join(format!("mb_cleanup_{}", std::process::id())).join("Show").join("Season 01");
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("Show - S01E02.mp4");
        for f in ["Show - S01E02.mp4.part", "Show - S01E02.mp4.part.json", "Show - S01E02.mp4.part.0", "Show - S01E02.mp4.part.1", "Show - S01E02.en.srt"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        std::fs::write(dir.join("Show - S01E01.mp4"), b"keep").unwrap();
        cleanup_files(&dest, false, true);
        let left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, vec!["Show - S01E01.mp4".to_string()]);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn ext_detection() {
        assert_eq!(ext_from_url("https://h/x/file.MKV?sign=2"), "mkv");
        assert_eq!(ext_from_url("https://h/x/stream"), "mp4");
    }
}
