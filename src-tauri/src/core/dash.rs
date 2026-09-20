//! Downloading MovieBox's DASH streams.
//!
//! MovieBox no longer serves direct files — every direct link now answers with a
//! 21-second "Update now" advert (see `stream_pool::is_notice_url`). What is left is
//! a signed DASH manifest, where video and audio arrive as separate segment lists.
//!
//! So a download here means: read the manifest, append the video segments into one
//! part file and the audio segments into another, then mux the two into a single
//! playable file with the bundled ffmpeg. Segment counts are written to
//! `<file>.part.json` after every segment, so a paused or interrupted download
//! continues where it stopped instead of starting over.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

pub fn is_dash_url(url: &str) -> bool {
    let base = url.split('?').next().unwrap_or(url);
    base.ends_with(".mpd") || url.contains("/dash/")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub init: String,
    pub segments: Vec<String>,
    pub height: u64,
    pub bandwidth: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub video: Track,
    pub audio: Option<Track>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct State {
    video_segments: usize,
    video_bytes: u64,
    audio_segments: usize,
    audio_bytes: u64,
}

pub enum Outcome {
    Completed { bytes: u64 },
    Paused { bytes: u64 },
}

// ---------------------------------------------------------------- manifest ----

#[derive(Debug, Default, Clone)]
struct Template {
    init: String,
    media: String,
    start_number: u64,
    /// One entry per segment: the `$Time$` value, when the manifest uses a timeline.
    times: Vec<u64>,
    /// Segment count when the manifest numbers its segments.
    count: usize,
}

#[derive(Debug, Default, Clone)]
struct Rep {
    id: String,
    bandwidth: u64,
    height: u64,
    template: Option<Template>,
}

fn attr(tag: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    tag.attributes()
        .flatten()
        .find(|a| a.key.as_ref().eq_ignore_ascii_case(name))
        .map(|a| a.value.into_owned())
}

fn num(tag: &quick_xml::events::BytesStart, name: &str) -> Option<u64> {
    attr(tag, name).and_then(|v| v.trim().parse().ok())
}

/// `$RepresentationID$`, `$Number$`, `$Number%05d$` and `$Time$` in a segment name.
fn expand(pattern: &str, rep_id: &str, number: u64, time: u64) -> String {
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut rest = pattern;
    while let Some(start) = rest.find('$') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('$') else {
            out.push('$');
            rest = after;
            continue;
        };
        let token = &after[..end];
        rest = &after[end + 1..];
        let (name, fmt) = match token.split_once('%') {
            Some((n, f)) => (n, Some(f)),
            None => (token, None),
        };
        let value = match name {
            "RepresentationID" => {
                out.push_str(rep_id);
                continue;
            }
            "Number" => number,
            "Time" => time,
            "Bandwidth" => 0,
            "" => {
                out.push('$');
                continue;
            }
            _ => 0,
        };
        match fmt.and_then(|f| f.trim_end_matches('d').trim_start_matches('0').parse::<usize>().ok().or(Some(f.trim_end_matches('d').len()))) {
            Some(width) => out.push_str(&format!("{value:0width$}")),
            None => out.push_str(&value.to_string()),
        }
    }
    out.push_str(rest);
    out
}

/// Everything up to and including the last `/` of the manifest URL.
fn base_of(mpd_url: &str) -> String {
    let no_query = mpd_url.split('?').next().unwrap_or(mpd_url);
    match no_query.rfind('/') {
        Some(i) => no_query[..=i].to_string(),
        None => String::new(),
    }
}

fn join(base: &str, rel: &str) -> String {
    if rel.starts_with("http://") || rel.starts_with("https://") {
        rel.to_string()
    } else {
        format!("{base}{rel}")
    }
}

/// Read a manifest and pick one video track (the best at or below `max_height`,
/// or simply the best when `max_height` is 0) plus the best audio track.
pub fn parse_manifest(xml: &str, mpd_url: &str, max_height: u64) -> Result<Plan, String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut video: Vec<Rep> = Vec::new();
    let mut audio: Vec<Rep> = Vec::new();
    let mut kind = String::new(); // "video" | "audio" | ""
    let mut set_template: Option<Template> = None;
    let mut rep: Option<Rep> = None;
    let mut in_timeline = false;
    let mut timeline: Vec<u64> = Vec::new();
    let mut next_time: u64 = 0;
    let mut buf = Vec::new();

    let finish_rep = |rep: &mut Option<Rep>, set_template: &Option<Template>, timeline: &mut Vec<u64>, video: &mut Vec<Rep>, audio: &mut Vec<Rep>, kind: &str| {
        let Some(mut r) = rep.take() else { return };
        if r.template.is_none() {
            r.template = set_template.clone();
        }
        if let Some(t) = r.template.as_mut() {
            if !timeline.is_empty() {
                t.times = std::mem::take(timeline);
                t.count = t.times.len();
            }
        }
        match kind {
            "video" => video.push(r),
            "audio" => audio.push(r),
            _ => {}
        }
    };

    loop {
        let event = reader.read_event_into(&mut buf).map_err(|e| format!("The video list from the server couldn't be read ({e})."))?;
        match event {
            Event::Eof => break,
            Event::Start(ref t) | Event::Empty(ref t) => {
                let empty = matches!(event, Event::Empty(_));
                let name = t.name().as_ref().to_ascii_lowercase();
                match name.as_str() {
                    "adaptationset" => {
                        kind = attr(t, "contentType")
                            .or_else(|| attr(t, "mimeType").map(|m| m.split('/').next().unwrap_or("").to_string()))
                            .unwrap_or_default()
                            .to_ascii_lowercase();
                        set_template = None;
                    }
                    "segmenttemplate" => {
                        let tpl = Template {
                            init: attr(t, "initialization").unwrap_or_default(),
                            media: attr(t, "media").unwrap_or_default(),
                            start_number: num(t, "startNumber").unwrap_or(1),
                            times: Vec::new(),
                            count: 0,
                        };
                        match rep.as_mut() {
                            Some(r) => r.template = Some(tpl),
                            None => set_template = Some(tpl),
                        }
                        next_time = 0;
                        timeline.clear();
                    }
                    "representation" => {
                        let mime = attr(t, "mimeType").unwrap_or_default().to_ascii_lowercase();
                        if kind.is_empty() && !mime.is_empty() {
                            kind = mime.split('/').next().unwrap_or("").to_string();
                        }
                        rep = Some(Rep {
                            id: attr(t, "id").unwrap_or_default(),
                            bandwidth: num(t, "bandwidth").unwrap_or(0),
                            height: num(t, "height").unwrap_or(0),
                            template: None,
                        });
                        timeline.clear();
                        next_time = 0;
                        if empty {
                            finish_rep(&mut rep, &set_template, &mut timeline, &mut video, &mut audio, &kind);
                        }
                    }
                    "segmenttimeline" => {
                        in_timeline = true;
                    }
                    "s" if in_timeline => {
                        if let Some(t0) = num(t, "t") {
                            next_time = t0;
                        }
                        let d = num(t, "d").unwrap_or(0);
                        let repeat = attr(t, "r").and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
                        for _ in 0..=repeat.max(0) {
                            timeline.push(next_time);
                            next_time += d;
                        }
                    }
                    _ => {}
                }
            }
            Event::End(ref t) => {
                let name = t.name().as_ref().to_ascii_lowercase();
                match name.as_str() {
                    "representation" => finish_rep(&mut rep, &set_template, &mut timeline, &mut video, &mut audio, &kind),
                    "segmenttimeline" => in_timeline = false,
                    "adaptationset" => {
                        kind.clear();
                        set_template = None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }

    let base = base_of(mpd_url);
    let build = |r: &Rep| -> Option<Track> {
        let t = r.template.as_ref()?;
        if t.media.is_empty() {
            return None;
        }
        let count = if t.count > 0 { t.count } else { 0 };
        if count == 0 {
            return None;
        }
        let segments = (0..count)
            .map(|i| {
                let number = t.start_number + i as u64;
                let time = t.times.get(i).copied().unwrap_or(0);
                join(&base, &expand(&t.media, &r.id, number, time))
            })
            .collect();
        Some(Track {
            init: join(&base, &expand(&t.init, &r.id, t.start_number, 0)),
            segments,
            height: r.height,
            bandwidth: r.bandwidth,
        })
    };

    let pick_video = video
        .iter()
        .filter(|r| max_height == 0 || r.height == 0 || r.height <= max_height)
        .max_by_key(|r| (r.height, r.bandwidth))
        .or_else(|| video.iter().min_by_key(|r| (r.height, r.bandwidth)))
        .ok_or_else(|| "This title's video list is empty.".to_string())?;
    let video_track = build(pick_video).ok_or_else(|| "This title's video can't be downloaded yet.".to_string())?;
    let audio_track = audio.iter().max_by_key(|r| r.bandwidth).and_then(build);

    Ok(Plan { video: video_track, audio: audio_track })
}

// ---------------------------------------------------------------- download ----

fn state_path(dest: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part.json", dest.to_string_lossy()))
}

fn part_path(dest: &Path, kind: &str) -> PathBuf {
    PathBuf::from(format!("{}.part.{kind}", dest.to_string_lossy()))
}

fn load_state(dest: &Path) -> State {
    std::fs::read_to_string(state_path(dest))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(dest: &Path, s: &State) {
    if let Ok(json) = serde_json::to_string(s) {
        let _ = std::fs::write(state_path(dest), json);
    }
}

/// Append one track's remaining segments to its part file.
/// Returns false when the caller asked to stop.
async fn fetch_track(
    client: &reqwest::Client,
    track: &Track,
    file: &Path,
    done: &mut usize,
    bytes: &mut u64,
    cancel: &Arc<AtomicBool>,
    // (bytes just written, segments finished, bytes in the file)
    mut on_bytes: impl FnMut(u64, usize, u64),
) -> Result<bool, String> {
    // A fresh start writes the init segment first; a resume trusts the recorded length.
    let mut handle = if *done == 0 {
        let mut f = tokio::fs::File::create(file).await.map_err(|e| e.to_string())?;
        let init = client.get(&track.init).send().await.map_err(net_err)?;
        if !init.status().is_success() {
            return Err(format!("server returned HTTP {}", init.status()));
        }
        let data = init.bytes().await.map_err(net_err)?;
        f.write_all(&data).await.map_err(|e| e.to_string())?;
        *bytes = data.len() as u64;
        on_bytes(data.len() as u64, 0, *bytes);
        f
    } else {
        let f = tokio::fs::OpenOptions::new().write(true).open(file).await.map_err(|e| e.to_string())?;
        // Drop anything written past the last confirmed segment.
        f.set_len(*bytes).await.map_err(|e| e.to_string())?;
        let mut f = f;
        f.seek(std::io::SeekFrom::Start(*bytes)).await.map_err(|e| e.to_string())?;
        f
    };

    while *done < track.segments.len() {
        if cancel.load(Ordering::SeqCst) {
            handle.flush().await.ok();
            return Ok(false);
        }
        let url = &track.segments[*done];
        let resp = client.get(url).send().await.map_err(net_err)?;
        if !resp.status().is_success() {
            return Err(format!("server returned HTTP {}", resp.status()));
        }
        let data = resp.bytes().await.map_err(net_err)?;
        handle.write_all(&data).await.map_err(|e| e.to_string())?;
        *done += 1;
        *bytes += data.len() as u64;
        on_bytes(data.len() as u64, *done, *bytes);
    }
    handle.flush().await.ok();
    Ok(true)
}

fn net_err(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "the connection timed out".to_string()
    } else {
        e.to_string()
    }
}

/// Download a DASH stream into `dest`, resuming whatever is already there.
/// `report(downloaded, speed)` is called as bytes arrive.
#[allow(clippy::too_many_arguments)]
pub async fn download_dash(
    client: &reqwest::Client,
    mpd_url: &str,
    dest: &Path,
    max_height: u64,
    cancel: Arc<AtomicBool>,
    mut report: impl FnMut(u64, f64),
) -> Result<Outcome, String> {
    let xml = client
        .get(mpd_url)
        .send()
        .await
        .map_err(net_err)?
        .error_for_status()
        .map_err(net_err)?
        .text()
        .await
        .map_err(net_err)?;
    let plan = parse_manifest(&xml, mpd_url, max_height)?;

    let video_part = part_path(dest, "video");
    let audio_part = part_path(dest, "audio");
    let mut state = load_state(dest);
    // A part file that vanished (or a manifest with a different segment count) starts over.
    let audio_mismatch = state.audio_segments > plan.audio.as_ref().map(|a| a.segments.len()).unwrap_or(0)
        || (state.audio_segments > 0 && !audio_part.exists());
    if (state.video_segments > 0 && (!video_part.exists() || state.video_segments > plan.video.segments.len())) || audio_mismatch {
        state = State::default();
    }

    let started = std::time::Instant::now();
    let base_bytes = state.video_bytes + state.audio_bytes;
    let mut total = base_bytes;
    let mut last = std::time::Instant::now();
    let mut tick = |delta: u64, state: &State, dest: &Path| {
        total += delta;
        if last.elapsed().as_millis() >= 700 {
            last = std::time::Instant::now();
            let secs = started.elapsed().as_secs_f64().max(0.001);
            report(total, (total.saturating_sub(base_bytes)) as f64 / secs);
            save_state(dest, state);
        }
    };

    {
        let mut done = state.video_segments;
        let mut bytes = state.video_bytes;
        let mut snapshot = state.clone();
        let finished = fetch_track(client, &plan.video, &video_part, &mut done, &mut bytes, &cancel, |d, segments, written| {
            snapshot.video_segments = segments;
            snapshot.video_bytes = written;
            tick(d, &snapshot, dest);
        })
        .await;
        state.video_segments = done;
        state.video_bytes = bytes;
        save_state(dest, &state);
        if !finished? {
            return Ok(Outcome::Paused { bytes: total });
        }
    }

    if let Some(audio) = plan.audio.as_ref() {
        let mut done = state.audio_segments;
        let mut bytes = state.audio_bytes;
        let mut snapshot = state.clone();
        let finished = fetch_track(client, audio, &audio_part, &mut done, &mut bytes, &cancel, |d, segments, written| {
            snapshot.audio_segments = segments;
            snapshot.audio_bytes = written;
            tick(d, &snapshot, dest);
        })
        .await;
        state.audio_segments = done;
        state.audio_bytes = bytes;
        save_state(dest, &state);
        if !finished? {
            return Ok(Outcome::Paused { bytes: total });
        }
    }

    // Both tracks are here: join them into one file people can actually play.
    mux(&video_part, plan.audio.as_ref().map(|_| audio_part.as_path()), dest)?;
    let bytes = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(total);
    let _ = std::fs::remove_file(&video_part);
    let _ = std::fs::remove_file(&audio_part);
    let _ = std::fs::remove_file(state_path(dest));
    Ok(Outcome::Completed { bytes })
}

// -------------------------------------------------------------------- mux ----

/// The ffmpeg that ships beside the app (a Tauri sidecar), or one on PATH in dev.
pub fn ffmpeg_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("ffmpeg.exe"));
            candidates.push(dir.join("ffmpeg-x86_64-pc-windows-msvc.exe"));
            candidates.push(dir.join("../../../bin/ffmpeg-x86_64-pc-windows-msvc.exe"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("bin/ffmpeg-x86_64-pc-windows-msvc.exe"));
        candidates.push(cwd.join("src-tauri/bin/ffmpeg-x86_64-pc-windows-msvc.exe"));
    }
    candidates.into_iter().find(|p| p.exists())
}

fn mux(video: &Path, audio: Option<&Path>, dest: &Path) -> Result<(), String> {
    let ffmpeg = ffmpeg_path().ok_or_else(|| "The video joiner (ffmpeg) is missing from this installation.".to_string())?;
    let mut cmd = std::process::Command::new(ffmpeg);
    cmd.arg("-y").arg("-loglevel").arg("error").arg("-i").arg(video);
    if let Some(a) = audio {
        cmd.arg("-i").arg(a);
    }
    cmd.arg("-c").arg("copy").arg("-movflags").arg("+faststart").arg(dest);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash
    }
    let out = cmd.output().map_err(|e| format!("The video joiner couldn't start ({e})."))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        log::warn!("ffmpeg mux failed: {}", err.trim());
        return Err("The downloaded video couldn't be put together.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MPD: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static" mediaPresentationDuration="PT1M">
  <Period id="0" start="PT0.0S">
    <AdaptationSet id="0" contentType="video">
      <Representation id="0" mimeType="video/mp4" codecs="hev1" bandwidth="350000" width="1146" height="480">
        <SegmentTemplate timescale="24000" initialization="init-stream$RepresentationID$.m4s" media="chunk-stream$RepresentationID$-$Number%05d$.m4s" startNumber="1">
          <SegmentTimeline>
            <S t="0" d="142142" />
            <S d="143143" r="2" />
          </SegmentTimeline>
        </SegmentTemplate>
      </Representation>
      <Representation id="2" mimeType="video/mp4" codecs="hev1" bandwidth="900000" width="1920" height="1080">
        <SegmentTemplate timescale="24000" initialization="init-stream$RepresentationID$.m4s" media="chunk-stream$RepresentationID$-$Number%05d$.m4s" startNumber="1">
          <SegmentTimeline>
            <S t="0" d="142142" r="3" />
          </SegmentTimeline>
        </SegmentTemplate>
      </Representation>
    </AdaptationSet>
    <AdaptationSet id="1" contentType="audio">
      <Representation id="1" mimeType="audio/mp4" codecs="mp4a.40.2" bandwidth="128000">
        <SegmentTemplate timescale="48000" initialization="init-stream$RepresentationID$.m4s" media="chunk-stream$RepresentationID$-$Number%05d$.m4s" startNumber="1">
          <SegmentTimeline>
            <S t="0" d="288768" r="1" />
          </SegmentTimeline>
        </SegmentTemplate>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;

    #[test]
    fn picks_best_video_and_audio() {
        let plan = parse_manifest(MPD, "https://cdn.test/dash/abc/index.mpd", 0).unwrap();
        assert_eq!(plan.video.height, 1080);
        assert_eq!(plan.video.segments.len(), 4);
        assert_eq!(plan.video.init, "https://cdn.test/dash/abc/init-stream2.m4s");
        assert_eq!(plan.video.segments[0], "https://cdn.test/dash/abc/chunk-stream2-00001.m4s");
        assert_eq!(plan.video.segments[3], "https://cdn.test/dash/abc/chunk-stream2-00004.m4s");
        let audio = plan.audio.unwrap();
        assert_eq!(audio.segments.len(), 2);
        assert_eq!(audio.init, "https://cdn.test/dash/abc/init-stream1.m4s");
    }

    #[test]
    fn honours_a_quality_ceiling() {
        let plan = parse_manifest(MPD, "https://cdn.test/dash/abc/index.mpd", 720).unwrap();
        assert_eq!(plan.video.height, 480);
        assert_eq!(plan.video.segments.len(), 4);
    }

    #[test]
    fn expands_segment_patterns() {
        assert_eq!(expand("chunk-$RepresentationID$-$Number%05d$.m4s", "2", 7, 0), "chunk-2-00007.m4s");
        assert_eq!(expand("seg-$Time$.m4s", "0", 1, 142142), "seg-142142.m4s");
        assert_eq!(expand("plain.m4s", "0", 1, 0), "plain.m4s");
    }

    /// Network check, not part of the normal suite:
    /// `cargo test --lib dash_health -- --ignored --nocapture`
    /// Takes a real MovieBox manifest, downloads the first few segments of each
    /// track and muxes them, proving the whole download path end to end.
    #[tokio::test]
    #[ignore]
    async fn dash_health() {
        use moviebox_tui::service::MovieBoxService;
        let service = MovieBoxService::new();
        let hits = service
            .search_typed(moviebox_tui::providers::ProviderKind::MovieBox, "Inception", 1)
            .await
            .expect("search");
        let id = &hits.first().expect("a hit").id.value;
        let pool = crate::commands::streams::collect_streams(&service, id, 0, 0, 0).await.expect("streams");
        let rel = pool.iter().find(|r| is_dash_url(&r.mirrors[0].resolver_url)).expect("a DASH stream");
        let mirror = &rel.mirrors[0];

        let mut builder = moviebox_tui::net::http_client_builder();
        let mut map = reqwest::header::HeaderMap::new();
        for (k, v) in &mirror.headers {
            if k.eq_ignore_ascii_case("user-agent") {
                builder = builder.user_agent(v);
            } else if let (Ok(n), Ok(val)) = (reqwest::header::HeaderName::from_bytes(k.as_bytes()), reqwest::header::HeaderValue::from_str(v)) {
                map.insert(n, val);
            }
        }
        let client = builder.default_headers(map).build().unwrap();

        let xml = client.get(&mirror.resolver_url).send().await.unwrap().text().await.unwrap();
        let mut plan = parse_manifest(&xml, &mirror.resolver_url, 0).expect("manifest parses");
        println!("video {}p, {} segments; audio: {:?}", plan.video.height, plan.video.segments.len(), plan.audio.as_ref().map(|a| a.segments.len()));
        assert!(!plan.video.segments.is_empty());

        plan.video.segments.truncate(3);
        if let Some(a) = plan.audio.as_mut() {
            a.segments.truncate(3);
        }

        let dir = std::env::temp_dir().join("moviebox-dash-health");
        let _ = std::fs::create_dir_all(&dir);
        let dest = dir.join("sample.mp4");
        let vpart = part_path(&dest, "video");
        let apart = part_path(&dest, "audio");
        let cancel = Arc::new(AtomicBool::new(false));

        let (mut d, mut b) = (0usize, 0u64);
        fetch_track(&client, &plan.video, &vpart, &mut d, &mut b, &cancel, |_, _, _| {}).await.expect("video segments");
        println!("video part: {b} bytes in {d} segments");
        if let Some(a) = plan.audio.as_ref() {
            let (mut d2, mut b2) = (0usize, 0u64);
            fetch_track(&client, a, &apart, &mut d2, &mut b2, &cancel, |_, _, _| {}).await.expect("audio segments");
            println!("audio part: {b2} bytes in {d2} segments");
        }

        mux(&vpart, plan.audio.as_ref().map(|_| apart.as_path()), &dest).expect("ffmpeg mux");
        let size = std::fs::metadata(&dest).unwrap().len();
        println!("muxed {} bytes -> {}", size, dest.display());
        assert!(size > 100_000, "muxed file looks empty");

        // The point of muxing is one file holding both tracks: check that it does.
        let probe = std::process::Command::new(ffmpeg_path().expect("ffmpeg"))
            .arg("-hide_banner")
            .arg("-i")
            .arg(&dest)
            .output()
            .expect("ffmpeg runs");
        let info = String::from_utf8_lossy(&probe.stderr).to_string();
        println!("{}", info.lines().filter(|l| l.contains("Stream #") || l.contains("Duration")).collect::<Vec<_>>().join("
"));
        assert!(info.contains("Video:"), "no video track in the muxed file");
        assert!(info.contains("Audio:"), "no audio track in the muxed file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recognises_dash_links() {
        assert!(is_dash_url("https://sacdn.hakunaymatata.com/dash/1_0_0_1080_h265_559/index.mpd"));
        assert!(!is_dash_url("https://cdn.test/movie.mp4?sign=1"));
    }
}

