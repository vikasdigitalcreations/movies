//! The two adult sources behind the PIN: Eporner and RedGifs.
//!
//! Both are the platforms' own documented APIs, not scrapers, so they keep whatever
//! moderation and compliance those platforms run. Neither needs an account or a key.
//!
//! They differ in one way that shapes everything here. RedGifs hands back a direct
//! `.mp4`, so it plays in our own player and can be downloaded. Eporner serves its files
//! only inside its embed (the file URL answers 403 anywhere else), so it is browsed
//! natively and played in a webview through the embed its API gives us. Defeating that
//! protection would be both fragile and rude, so we do not.
//!
//! Nothing identifying is sent to either: no account, no cookie we keep, no referer
//! beyond what the platform requires.

use crate::core::types::{Card, StreamDto};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const EPORNER_API: &str = "https://www.eporner.com/api/v2";
const REDGIFS_API: &str = "https://api.redgifs.com/v2";
/// RedGifs rejects the default reqwest agent; anything descriptive is accepted.
const UA: &str = "MovieBox/1.2 (Windows; personal media app)";

/// `adult:<source>:<id>` keeps these apart from MovieBox's bare numbers and from
/// `addon:` ids, and survives a trip through the router.
const PREFIX: &str = "adult:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Eporner,
    RedGifs,
}

impl Source {
    fn tag(self) -> &'static str {
        match self {
            Source::Eporner => "ep",
            Source::RedGifs => "rg",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Source::Eporner => "Eporner",
            Source::RedGifs => "RedGifs",
        }
    }
    fn from_tag(t: &str) -> Option<Self> {
        match t {
            "ep" => Some(Source::Eporner),
            "rg" => Some(Source::RedGifs),
            _ => None,
        }
    }
}

pub fn encode_id(source: Source, id: &str) -> String {
    format!("{PREFIX}{}:{id}", source.tag())
}

pub fn decode_id(id: &str) -> Option<(Source, String)> {
    let rest = id.strip_prefix(PREFIX)?;
    let (tag, rest) = rest.split_once(':')?;
    let source = Source::from_tag(tag)?;
    if rest.is_empty() {
        return None;
    }
    Some((source, rest.to_string()))
}

pub fn is_adult_id(id: &str) -> bool {
    id.starts_with(PREFIX)
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

fn hhmmss(total: u64) -> String {
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

// ----------------------------------------------------------------- Eporner

#[derive(Deserialize)]
struct EpSearch {
    #[serde(default)]
    videos: Vec<EpVideo>,
}

#[derive(Deserialize)]
struct EpVideo {
    id: String,
    title: String,
    #[serde(default)]
    length_sec: serde_json::Value,
    #[serde(default)]
    default_thumb: Option<EpThumb>,
}

#[derive(Deserialize)]
struct EpThumb {
    src: Option<String>,
}

/// `length_sec` comes back as a number for some videos and a string for others.
fn as_secs(v: &serde_json::Value) -> u64 {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

async fn eporner_search(query: &str, page: usize) -> Result<Vec<Card>, String> {
    let q = if query.trim().is_empty() { "all" } else { query.trim() };
    let order = if query.trim().is_empty() { "top-weekly" } else { "relevance" };
    let url = reqwest::Url::parse_with_params(
        &format!("{EPORNER_API}/video/search/"),
        &[
            ("query", q),
            ("per_page", "30"),
            ("page", &page.max(1).to_string()),
            ("thumbsize", "big"),
            ("order", order),
            ("gay", "0"),
            ("lq", "0"),
            ("format", "json"),
        ],
    )
    .map_err(|_| "Could not build that search.".to_string())?;
    let res: EpSearch = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| "Eporner isn't answering right now.".to_string())?
        .json()
        .await
        .map_err(|_| "Eporner sent something unexpected.".to_string())?;

    Ok(res
        .videos
        .into_iter()
        .map(|v| Card {
            id: encode_id(Source::Eporner, &v.id),
            title: v.title,
            year: Some(hhmmss(as_secs(&v.length_sec))),
            poster: v.default_thumb.and_then(|t| t.src),
            media_type: "movie".into(),
        })
        .collect())
}

/// The embed page the app shows in a webview. This is Eporner's own player, ads and all,
/// which is the deal in exchange for the API being free and keyless.
pub fn eporner_embed_url(id: &str) -> String {
    format!("https://www.eporner.com/embed/{id}")
}

// ----------------------------------------------------------------- RedGifs

/// RedGifs issues a short-lived anonymous token. It is cached because asking for one on
/// every search would be rude and slow; an hour is comfortably inside its lifetime.
static RG_TOKEN: Mutex<Option<(String, Instant)>> = Mutex::const_new(None);

async fn redgifs_token() -> Result<String, String> {
    let mut guard = RG_TOKEN.lock().await;
    if let Some((tok, got)) = guard.as_ref() {
        if got.elapsed() < Duration::from_secs(3000) {
            return Ok(tok.clone());
        }
    }
    #[derive(Deserialize)]
    struct Auth {
        token: String,
    }
    let auth: Auth = client()?
        .get(format!("{REDGIFS_API}/auth/temporary"))
        .send()
        .await
        .map_err(|_| "RedGifs isn't answering right now.".to_string())?
        .json()
        .await
        .map_err(|_| "RedGifs refused to start a session.".to_string())?;
    *guard = Some((auth.token.clone(), Instant::now()));
    Ok(auth.token)
}

#[derive(Deserialize)]
struct RgSearch {
    #[serde(default)]
    gifs: Vec<RgGif>,
}

#[derive(Deserialize)]
struct RgGif {
    id: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    // RedGifs sends an explicit null for these on some posts, and serde's `default`
    // covers a missing field but not a null one, so they have to be optional.
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    height: Option<u64>,
    #[serde(default)]
    urls: RgUrls,
}

#[derive(Deserialize, Default)]
struct RgUrls {
    hd: Option<String>,
    sd: Option<String>,
    thumbnail: Option<String>,
    poster: Option<String>,
}

impl RgGif {
    fn best(&self) -> Option<&String> {
        self.urls.hd.as_ref().or(self.urls.sd.as_ref())
    }
    /// RedGifs posts rarely have titles, so fall back to the tags, then the id.
    fn label(&self) -> String {
        let d = self.description.clone().unwrap_or_default();
        let d = d.trim();
        if !d.is_empty() {
            return d.chars().take(70).collect();
        }
        if !self.tags.is_empty() {
            return self.tags.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        }
        self.id.clone()
    }
}

async fn redgifs_search(query: &str, page: usize) -> Result<Vec<Card>, String> {
    let token = redgifs_token().await?;
    let q = query.trim();
    let url = reqwest::Url::parse_with_params(
        &format!("{REDGIFS_API}/gifs/search"),
        &[("search_text", q), ("order", "trending"), ("count", "30"), ("page", &page.max(1).to_string())],
    )
    .map_err(|_| "Could not build that search.".to_string())?;
    let res: RgSearch = client()?
        .get(url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|_| "RedGifs isn't answering right now.".to_string())?
        .json()
        .await
        .map_err(|_| "RedGifs sent something unexpected.".to_string())?;

    Ok(res
        .gifs
        .into_iter()
        .filter(|g| g.best().is_some())
        .map(|g| Card {
            id: encode_id(Source::RedGifs, &g.id),
            title: g.label(),
            year: Some(hhmmss(g.duration.unwrap_or(0.0) as u64)),
            poster: g.urls.poster.clone().or_else(|| g.urls.thumbnail.clone()),
            media_type: "movie".into(),
        })
        .collect())
}

/// The playable file for one RedGifs post. Its CDN serves ranges to any agent, so this
/// goes straight to the player and to the download queue.
async fn redgifs_stream(id: &str) -> Result<StreamDto, String> {
    let token = redgifs_token().await?;
    #[derive(Deserialize)]
    struct One {
        gif: RgGif,
    }
    let one: One = client()?
        .get(format!("{REDGIFS_API}/gifs/{id}"))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|_| "RedGifs isn't answering right now.".to_string())?
        .json()
        .await
        .map_err(|_| "That clip could not be loaded.".to_string())?;

    let url = one.gif.best().cloned().ok_or("That clip has no playable file.")?;
    let height = one.gif.height.unwrap_or(0);
    Ok(StreamDto {
        label: if height > 0 { format!("{height}p") } else { "Best available".into() },
        height,
        multi: false,
        size: None,
        codec: None,
        url,
        headers: vec![("User-Agent".into(), UA.into())],
        resource_id: None,
        downloadable: true,
        source: Source::RedGifs.label().into(),
    })
}

// ----------------------------------------------------------------- shared entry points

/// Search one source. An empty query means "what's popular", which is what the section
/// shows when it is first opened.
pub async fn search(source: Source, query: &str, page: usize) -> Result<Vec<Card>, String> {
    match source {
        Source::Eporner => eporner_search(query, page).await,
        Source::RedGifs => redgifs_search(query, page).await,
    }
}

/// How an item is played. Eporner cannot be handed to the player, so the caller opens
/// the embed instead; RedGifs behaves like any other stream.
pub enum Playback {
    Stream(Box<StreamDto>),
    Embed(String),
}

pub async fn playback(id: &str) -> Result<Playback, String> {
    let (source, raw) = decode_id(id).ok_or("That clip's link is not valid.")?;
    match source {
        Source::RedGifs => Ok(Playback::Stream(Box::new(redgifs_stream(&raw).await?))),
        Source::Eporner => Ok(Playback::Embed(eporner_embed_url(&raw))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_stay_distinct() {
        let e = encode_id(Source::Eporner, "abc123");
        let r = encode_id(Source::RedGifs, "xyz789");
        assert!(is_adult_id(&e) && is_adult_id(&r));
        assert_eq!(decode_id(&e).unwrap().0, Source::Eporner);
        assert_eq!(decode_id(&r).unwrap().1, "xyz789");
        // A MovieBox id and an addon id must never be mistaken for one of these.
        assert!(!is_adult_id("977486567826752424"));
        assert!(!is_adult_id("addon:https://x/manifest.json|movie|tt1"));
        assert!(decode_id("adult:zz:1").is_none());
        assert!(decode_id("adult:rg:").is_none());
    }

    #[test]
    fn durations_read_as_people_write_them() {
        assert_eq!(hhmmss(8), "0:08");
        assert_eq!(hhmmss(75), "1:15");
        assert_eq!(hhmmss(7219), "2:00:19");
    }

    #[test]
    fn length_parses_from_number_or_string() {
        assert_eq!(as_secs(&serde_json::json!(90)), 90);
        assert_eq!(as_secs(&serde_json::json!("90")), 90);
        assert_eq!(as_secs(&serde_json::json!(null)), 0);
    }

    /// Network check, skipped by default:
    /// `cargo test --lib adult_health -- --ignored --nocapture`
    /// Confirms both sources still answer and that RedGifs still yields a fetchable file.
    #[tokio::test]
    #[ignore]
    async fn adult_health() {
        let ep = search(Source::Eporner, "", 1).await.expect("eporner answers");
        println!("Eporner: {} items", ep.len());
        assert!(!ep.is_empty());
        assert!(ep.iter().all(|c| c.poster.is_some()));

        let rg = search(Source::RedGifs, "", 1).await.expect("redgifs answers");
        println!("RedGifs: {} items", rg.len());
        assert!(!rg.is_empty());

        match playback(&rg[0].id).await.expect("a playable clip") {
            Playback::Stream(s) => {
                println!("  stream {} -> {}", s.label, s.url.split('?').next().unwrap_or(""));
                let r = client().unwrap().get(&s.url).header("Range", "bytes=0-1023").send().await.expect("reachable");
                println!("  http {}", r.status());
                assert!(r.status().is_success());
            }
            Playback::Embed(_) => panic!("RedGifs should be a direct stream"),
        }

        match playback(&ep[0].id).await.expect("an eporner item") {
            Playback::Embed(u) => {
                println!("  embed {u}");
                assert!(u.starts_with("https://www.eporner.com/embed/"));
            }
            Playback::Stream(_) => panic!("Eporner should be an embed"),
        }
    }
}
