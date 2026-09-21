//! The yt-dlp helper program, fetched when it is first needed and kept current.
//!
//! YouTube changes often enough that a copy baked into the installer would be stale within
//! weeks, and yt-dlp's own releases follow YouTube within days. So nothing ships: the
//! first time the YouTube source is needed the current release is downloaded from
//! yt-dlp's GitHub releases page, checked against the SHA-256 that release publishes, and
//! kept in `%LOCALAPPDATA%\MovieBox\tools`. At most once every three days the published
//! checksum is compared with the local copy and a newer release replaces it. Both are
//! plain downloads from GitHub; nothing about the user is sent.
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime};

const RELEASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const RECHECK_AFTER: Duration = Duration::from_secs(3 * 24 * 3600);

/// One download at a time: two searches starting together must not fetch the program twice.
static FETCH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn tools_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("MovieBox").join("tools")
}

pub fn exe_path() -> PathBuf {
    tools_dir().join("yt-dlp.exe")
}

fn marker_path() -> PathBuf {
    tools_dir().join("yt-dlp.checked")
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("MovieBox")
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

/// The hash a release publishes for `yt-dlp.exe`, from its `SHA2-256SUMS` file
/// (`<hash> *<file name>` per line).
pub fn published_hash(sums: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, name) = l.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == "yt-dlp.exe" && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

fn checked_recently() -> bool {
    std::fs::metadata(marker_path())
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .map(|age| age < RECHECK_AFTER)
        .unwrap_or(false)
}

fn touch_marker() {
    let _ = std::fs::create_dir_all(tools_dir());
    let _ = std::fs::write(marker_path(), b"");
}

/// Path to a usable yt-dlp, downloading or refreshing it if need be. If the network is
/// down, an existing copy is used as it is; with no copy at all this is an error.
pub async fn ensure() -> Result<PathBuf, String> {
    let exe = exe_path();
    if exe.exists() && checked_recently() {
        return Ok(exe);
    }
    let _one_at_a_time = FETCH.lock().await;
    // Another caller may have finished the job while this one waited.
    if exe.exists() && checked_recently() {
        return Ok(exe);
    }
    match refresh(&exe).await {
        Ok(()) => Ok(exe),
        Err(e) if exe.exists() => {
            log::warn!("yt-dlp refresh failed, using the copy already here: {e}");
            Ok(exe)
        }
        Err(e) => Err(format!("Couldn't fetch the YouTube helper: {e}")),
    }
}

async fn refresh(exe: &PathBuf) -> Result<(), String> {
    let http = client()?;
    let sums = http
        .get(format!("{RELEASE}/SHA2-256SUMS"))
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;
    let want = published_hash(&sums).ok_or("the release has no checksum for yt-dlp.exe")?;

    if let Ok(local) = tokio::fs::read(exe).await {
        if sha256_hex(&local) == want {
            touch_marker();
            return Ok(());
        }
    }

    let bytes = http
        .get(format!("{RELEASE}/yt-dlp.exe"))
        .timeout(Duration::from_secs(180))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    if sha256_hex(&bytes) != want {
        return Err("the download did not match its published checksum".into());
    }
    tokio::fs::create_dir_all(tools_dir()).await.map_err(|e| e.to_string())?;
    // Write beside the target and rename, so an interrupted download never leaves a
    // half-written program where the real one should be.
    let part = exe.with_extension("exe.part");
    tokio::fs::write(&part, &bytes).await.map_err(|e| e.to_string())?;
    tokio::fs::rename(&part, exe).await.map_err(|e| {
        let _ = std::fs::remove_file(&part);
        e.to_string()
    })?;
    touch_marker();
    log::info!("yt-dlp updated ({} bytes)", bytes.len());
    Ok(())
}

/// Run yt-dlp and return what it printed. The process is killed if the caller stops
/// waiting (a search that lost the race to a faster source), and never opens a window.
pub async fn run(args: &[&str], limit: Duration) -> Result<String, String> {
    let exe = ensure().await?;
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(args)
        .arg("--no-warnings")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let child = cmd.spawn().map_err(|e| format!("couldn't start yt-dlp: {e}"))?;
    let out = tokio::time::timeout(limit, child.wait_with_output())
        .await
        .map_err(|_| "YouTube took too long to answer.".to_string())?
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().rev().find(|l| l.contains("ERROR")).unwrap_or("yt-dlp failed").trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_published_hash_for_the_windows_build_only() {
        let sums = "\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa *yt-dlp\n\
66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a *yt-dlp.exe\n\
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb *yt-dlp_x86.exe\n";
        assert_eq!(published_hash(sums).as_deref(), Some("66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a"));
    }

    #[test]
    fn a_missing_or_malformed_line_gives_no_hash() {
        assert_eq!(published_hash(""), None);
        assert_eq!(published_hash("nothex *yt-dlp.exe"), None);
        assert_eq!(published_hash("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa *yt-dlp"), None);
    }

    #[test]
    fn hashes_match_the_known_digest() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
