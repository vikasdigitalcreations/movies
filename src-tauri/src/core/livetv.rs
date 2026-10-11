//! LiveTV: free live channels read from public M3U lists.
//!
//! Two lists feed it, both kept by volunteers. Each entry is an `#EXTINF` line describing
//! a channel, optionally `#EXTVLCOPT` lines naming the user agent or referer its server
//! insists on, then the address of the stream.
//!
//! The lists overlap heavily -- the same channel turns up in both, and again inside one
//! list as an HD and an SD feed -- so the guide is not the lists laid end to end. Every
//! entry for one channel is folded into a single tile, and the addresses it came with are
//! kept as that tile's links, best first. The redundancy stops being clutter and becomes
//! the fallback: when a link is dead, which is the normal state of some share of any such
//! list, the player moves on to the next one.
//!
//! The lists are other people's text and end up in the video engine, so nothing is taken
//! on trust: only `http`/`https` addresses are kept, and only a user agent and a referer
//! are carried over as headers.
//!
//! The parser is our own, not the vendored `M3UParser`, because that one drops the two
//! things these lists rely on: the language tag and the per-channel headers.

use crate::core::types::StreamDto;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

/// The lists, in the order their entries are trusted when two disagree.
pub const SOURCES: [(&str, &str); 2] = [
    ("iptv-org-in", "https://iptv-org.github.io/iptv/countries/in.m3u"),
    ("freeiptv-india", "https://raw.githubusercontent.com/akumar36/freeiptv/refs/heads/main/india.m3u"),
];

/// Channels go off air daily, so a copy older than this is fetched again.
const REFRESH_AFTER: Duration = Duration::from_secs(6 * 3600);
const MAX_LIST_BYTES: usize = 15 * 1024 * 1024;
const MAX_LINKS: usize = 5;
const UA: &str = "MovieBox (Windows; personal media app)";
/// Sent to a channel whose list entry names no user agent.
pub const STREAM_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Shown in this order; a category with no channels is left out.
const CATEGORIES: [&str; 12] = [
    "News", "Entertainment", "Movies", "Music", "Sports", "Kids", "Religious", "Documentary", "Lifestyle", "Education", "Business", "Other",
];

const LANGUAGES: [&str; 15] = [
    "Hindi", "English", "Tamil", "Telugu", "Kannada", "Malayalam", "Marathi", "Bengali", "Gujarati", "Punjabi", "Odia", "Assamese", "Urdu", "Bhojpuri", "International",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveChannel {
    /// Stable across refreshes: the channel's name reduced to letters and digits.
    pub id: String,
    pub name: String,
    pub logo: Option<String>,
    pub category: String,
    pub language: Option<String>,
    /// Best picture height any link claims; 0 when none says.
    pub height: u64,
    /// The channel's links, best first. Never empty.
    pub streams: Vec<StreamDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveGuide {
    pub channels: Vec<LiveChannel>,
    pub categories: Vec<String>,
    pub languages: Vec<String>,
    /// Unix seconds: when the oldest list in use was downloaded.
    pub updated: u64,
    /// A list could not be downloaded, so an older saved copy (or nothing) stood in.
    pub stale: bool,
}

// ---------------------------------------------------------------- parsing

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Entry {
    pub name: String,
    pub logo: String,
    pub group: String,
    pub language: String,
    pub user_agent: String,
    pub referrer: String,
    pub url: String,
}

/// The value of `name="…"` on an `#EXTINF` line. The name must start a word, so asking
/// for `tvg-logo` does not answer with `x-tvg-logo`.
fn attr<'a>(line: &'a str, name: &str) -> &'a str {
    let mut from = 0;
    while let Some(i) = line[from..].find(name) {
        let at = from + i;
        let rest = &line[at + name.len()..];
        let starts_word = at == 0 || line[..at].ends_with(|c: char| c.is_whitespace());
        if starts_word {
            for quote in ['"', '\''] {
                if let Some(value) = rest.strip_prefix('=').and_then(|r| r.strip_prefix(quote)) {
                    if let Some(end) = value.find(quote) {
                        return &value[..end];
                    }
                }
            }
        }
        from = at + name.len();
    }
    ""
}

/// The comma that ends the attributes and starts the channel's name: the first one that
/// is not inside a quoted value (a user agent has commas of its own).
fn title_comma(line: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ',' => return Some(i),
            _ => {}
        }
    }
    line.find(',')
}

/// One list has entries whose user agent was pasted with the option name still attached,
/// sometimes twice over.
fn strip_option_prefix(mut v: &str) -> &str {
    loop {
        let t = v.trim();
        match t.strip_prefix("#EXTVLCOPT:").and_then(|r| r.split_once('=')) {
            Some((_, value)) => v = value,
            None => return t,
        }
    }
}

pub fn parse(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut cur = Entry::default();
    let mut protected = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#EXTINF:") {
            cur = Entry {
                logo: attr(line, "tvg-logo").to_string(),
                group: attr(line, "group-title").to_string(),
                language: attr(line, "tvg-language").to_string(),
                user_agent: strip_option_prefix(attr(line, "http-user-agent")).to_string(),
                referrer: strip_option_prefix(attr(line, "http-referrer")).to_string(),
                name: title_comma(line).map(|i| line[i + 1..].trim().to_string()).unwrap_or_default(),
                url: String::new(),
            };
            protected = false;
        } else if let Some(opt) = line.strip_prefix("#EXTVLCOPT:") {
            if let Some((key, value)) = opt.split_once('=') {
                let value = strip_option_prefix(value);
                match key.trim().to_ascii_lowercase().as_str() {
                    "http-user-agent" => cur.user_agent = value.to_string(),
                    "http-referrer" | "http-referer" => cur.referrer = value.to_string(),
                    _ => {}
                }
            }
        } else if line.starts_with("#KODIPROP:") {
            // A licence key means DRM, which the player cannot open.
            protected |= line.to_ascii_lowercase().contains("license");
        } else if !line.starts_with('#') {
            // Kodi's form: address|User-Agent=…&Referer=…
            let (url, extra) = line.split_once('|').unwrap_or((line, ""));
            for pair in extra.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    match k.trim().to_ascii_lowercase().as_str() {
                        "user-agent" => cur.user_agent = v.trim().to_string(),
                        "referer" | "referrer" => cur.referrer = v.trim().to_string(),
                        _ => {}
                    }
                }
            }
            cur.url = url.trim().to_string();
            let entry = std::mem::take(&mut cur);
            if !protected && !entry.name.is_empty() {
                out.push(entry);
            }
            protected = false;
        }
    }
    out
}

// ---------------------------------------------------------------- tidying

/// What a list's own channel title says once the notes it carries are taken off it.
#[derive(Debug, Default, PartialEq)]
struct Title {
    name: String,
    height: u64,
    geo_blocked: bool,
    part_time: bool,
}

const QUALITY_WORDS: [&str; 5] = ["hd", "sd", "fhd", "uhd", "4k"];

/// "Aaj Tak (1080p) [Geo-blocked]" -> name "Aaj Tak", 1080, geo-blocked.
fn read_title(raw: &str) -> Title {
    let mut t = Title::default();
    let mut name = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(open) = rest.find(['(', '[']) {
        let close = if rest[open..].starts_with('(') { ')' } else { ']' };
        let Some(len) = rest[open..].find(close) else { break };
        let inner = rest[open + 1..open + len].trim().to_ascii_lowercase();
        let digits = inner.trim_end_matches(['p', 'i']);
        let is_height = digits.len() != inner.len() && (3..=4).contains(&digits.len()) && digits.chars().all(|c| c.is_ascii_digit());
        name.push_str(&rest[..open]);
        if is_height {
            t.height = t.height.max(digits.parse().unwrap_or(0));
        } else if close == ']' {
            t.geo_blocked |= inner.contains("geo");
            t.part_time |= inner.contains("24/7");
        } else if inner == "india" {
            // One list's way of telling a channel from a foreign namesake. Every channel
            // here is in a guide for India already.
        } else {
            // Any other bracket is part of the name: "Discovery (Tamil)".
            name.push_str(&rest[open..open + len + 1]);
        }
        rest = &rest[open + len + 1..];
    }
    name.push_str(rest);
    let mut words: Vec<&str> = name.split_whitespace().collect();
    // "Aastha HD" and "Aastha" are one channel on two feeds; the tile shows the plain name.
    while words.len() > 1 && words.last().is_some_and(|w| QUALITY_WORDS.contains(&w.to_ascii_lowercase().as_str())) {
        words.pop();
    }
    t.name = words.join(" ");
    t
}

/// What two entries must share to be the same channel: the name without spacing,
/// punctuation, case or a quality word.
fn channel_key(name: &str) -> String {
    let lower = name.to_lowercase().replace('&', " and ").replace('+', " plus ");
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !QUALITY_WORDS.contains(w))
        .collect()
}

/// Addresses that differ only in the case of the host are one address.
fn url_key(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
            format!("{}://{}{}", scheme.to_ascii_lowercase(), host.to_ascii_lowercase(), path)
        }
        None => url.to_string(),
    }
}

fn is_web_url(url: &str) -> bool {
    let lower = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://")) && url.len() <= 2000 && !url.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// A header value from a list, or nothing if it could not be sent as one line.
fn header_value(v: &str) -> Option<String> {
    let v = v.trim();
    (!v.is_empty() && v.len() <= 400 && !v.chars().any(|c| c.is_control())).then(|| v.to_string())
}

/// A list's group, split into the language it was filed under (if it names one) and one
/// of our categories. `weak` marks a catch-all that a more exact entry should override.
fn read_group(group: &str, language_tag: &str) -> (Option<String>, &'static str, bool) {
    let (prefix, topic) = group
        .split_once(" - ")
        .or_else(|| group.split_once(" | "))
        .map(|(a, b)| (a.trim(), b.trim()))
        .unwrap_or(("", group.trim()));
    let known = |s: &str| LANGUAGES.iter().find(|l| l.eq_ignore_ascii_case(s.trim())).map(|l| l.to_string());
    let language = known(prefix).or_else(|| known(language_tag.split(';').next().unwrap_or("")));
    let topic = topic.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    let (category, weak) = match topic.as_str() {
        "news" | "legislative" | "weather" => ("News", false),
        "entertainment" | "series" | "comedy" | "classic" | "family" => ("Entertainment", false),
        "general" => ("Entertainment", true),
        "movies" => ("Movies", false),
        "music" => ("Music", false),
        "sports" => ("Sports", false),
        "kids" | "animation" => ("Kids", false),
        "religious" => ("Religious", false),
        "documentary" | "science" => ("Documentary", false),
        "lifestyle" | "cooking" | "travel" | "culture" | "outdoor" => ("Lifestyle", false),
        "education" => ("Education", false),
        "business" => ("Business", false),
        _ => ("Other", true),
    };
    (language, category, weak)
}

struct Link {
    url: String,
    headers: Vec<(String, String)>,
    height: u64,
    geo_blocked: bool,
    part_time: bool,
}

struct Building {
    name: String,
    logo: Option<String>,
    category: &'static str,
    weak_category: bool,
    language: Option<String>,
    links: Vec<Link>,
}

/// Folds the lists into one guide. `lists` are in trust order: where two entries for one
/// channel disagree about its name or logo, the earlier list wins.
pub fn build(lists: &[Vec<Entry>]) -> (Vec<LiveChannel>, Vec<String>, Vec<String>) {
    let mut order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, Building> = HashMap::new();
    // Which channel each address already belongs to.
    let mut url_owner: HashMap<String, String> = HashMap::new();

    for entry in lists.iter().flatten() {
        if !is_web_url(&entry.url) {
            continue;
        }
        let address = url_key(&entry.url);
        let title = read_title(&entry.name);
        let (language, category, weak) = read_group(&entry.group, &entry.language);
        let logo = Some(entry.logo.trim()).filter(|l| is_web_url(l)).map(str::to_string);

        // An address seen before is the same broadcast whatever this entry calls it, so it
        // adds no link and no tile. What it knows about the channel is still kept: the
        // second list repeats most of the first one's addresses, and is the only one that
        // says which language a channel is in.
        let known = url_owner.get(&address).cloned();
        let key = known.clone().unwrap_or_else(|| Some(channel_key(&title.name)).filter(|k| !k.is_empty()).unwrap_or_else(|| address.clone()));
        let ch = by_key.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Building { name: title.name.clone(), logo: None, category, weak_category: weak, language: None, links: Vec::new() }
        });
        if ch.logo.is_none() {
            ch.logo = logo;
        }
        if ch.language.is_none() {
            ch.language = language;
        }
        if ch.weak_category && !weak {
            ch.category = category;
            ch.weak_category = false;
        }
        if known.is_some() {
            if let Some(link) = ch.links.iter_mut().find(|l| url_key(&l.url) == address) {
                link.height = link.height.max(title.height);
            }
            continue;
        }
        let mut headers = vec![("User-Agent".to_string(), header_value(&entry.user_agent).unwrap_or_else(|| STREAM_UA.to_string()))];
        if let Some(r) = header_value(&entry.referrer) {
            headers.push(("Referer".to_string(), r));
        }
        ch.links.push(Link { url: entry.url.clone(), headers, height: title.height, geo_blocked: title.geo_blocked, part_time: title.part_time });
        url_owner.insert(address, key);
    }

    let mut channels: Vec<LiveChannel> = order
        .into_iter()
        .filter_map(|key| by_key.remove(&key).map(|b| (key, b)))
        .map(|(id, mut b)| {
            // Stable, so links that tie keep the order the lists gave them.
            b.links.sort_by_key(|l| (l.geo_blocked, l.part_time, std::cmp::Reverse(l.height)));
            b.links.truncate(MAX_LINKS);
            let streams = b
                .links
                .into_iter()
                .map(|l| StreamDto {
                    label: if l.height > 0 { format!("{}p", l.height) } else { "Live".into() },
                    height: l.height,
                    multi: false,
                    size: None,
                    codec: None,
                    url: l.url,
                    headers: l.headers,
                    resource_id: None,
                    downloadable: false,
                    source: "LiveTV".into(),
                })
                .collect::<Vec<_>>();
            LiveChannel {
                id,
                name: b.name,
                logo: b.logo,
                category: b.category.to_string(),
                language: b.language,
                height: streams.iter().map(|s| s.height).max().unwrap_or(0),
                streams,
            }
        })
        .collect();
    channels.sort_by_cached_key(|c| c.name.to_lowercase());

    let categories = CATEGORIES.iter().filter(|c| channels.iter().any(|ch| ch.category == **c)).map(|c| c.to_string()).collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for l in channels.iter().filter_map(|c| c.language.as_deref()) {
        *counts.entry(l).or_default() += 1;
    }
    let mut languages: Vec<(&str, usize)> = counts.into_iter().collect();
    // "International" is not a language; it sits last however many channels it has.
    languages.sort_by(|a, b| (a.0 == "International").cmp(&(b.0 == "International")).then(b.1.cmp(&a.1)).then(a.0.cmp(b.0)));
    let languages: Vec<String> = languages.into_iter().map(|(l, _)| l.to_string()).collect();
    (channels, categories, languages)
}

// ---------------------------------------------------------------- fetching

fn lists_dir() -> PathBuf {
    let dir = crate::core::settings::app_dir().join("livetv");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

async fn download(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let mut resp = client.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?;
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_LIST_BYTES {
            return Err("the channel list is too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    // An error page served with a 200 must not replace a good saved copy.
    if !text.contains("#EXTINF") {
        return Err("that address did not return a channel list".into());
    }
    Ok(text)
}

struct Loaded {
    text: String,
    fetched: SystemTime,
    stale: bool,
}

/// One list: a recent saved copy if there is one, else a download, else whatever saved
/// copy exists however old. A channel list a day out of date still mostly works; no list
/// at all does not.
async fn load_list(client: &reqwest::Client, name: &str, url: &str, force: bool) -> Option<Loaded> {
    let path = lists_dir().join(format!("{name}.m3u"));
    let saved_at = tokio::fs::metadata(&path).await.ok().and_then(|m| m.modified().ok());
    let fresh = saved_at.and_then(|t| t.elapsed().ok()).is_some_and(|age| age < REFRESH_AFTER);
    if fresh && !force {
        if let Ok(text) = tokio::fs::read_to_string(&path).await {
            return Some(Loaded { text, fetched: saved_at.unwrap_or_else(SystemTime::now), stale: false });
        }
    }
    match download(client, url).await {
        Ok(text) => {
            let tmp = path.with_extension("m3u.part");
            if tokio::fs::write(&tmp, &text).await.is_ok() {
                let _ = tokio::fs::rename(&tmp, &path).await;
            }
            Some(Loaded { text, fetched: SystemTime::now(), stale: false })
        }
        Err(e) => {
            log::warn!("LiveTV list {name} could not be downloaded: {e}");
            let bytes = tokio::fs::read(&path).await.ok()?;
            Some(Loaded { text: String::from_utf8_lossy(&bytes).into_owned(), fetched: saved_at.unwrap_or(UNIX_EPOCH), stale: true })
        }
    }
}

static GUIDE: Mutex<Option<(Instant, LiveGuide)>> = Mutex::const_new(None);

/// The channel guide. Held in memory for the session and rebuilt when it is older than
/// the lists are allowed to be, or when `force` asks for a fresh download.
pub async fn guide(force: bool) -> Result<LiveGuide, String> {
    // Held across the download on purpose: a second caller waits and gets the same guide.
    let mut held = GUIDE.lock().await;
    if let Some((at, g)) = held.as_ref() {
        if !force && !g.stale && at.elapsed() < REFRESH_AFTER {
            return Ok(g.clone());
        }
    }
    let client = client()?;
    let loaded = futures::future::join_all(SOURCES.iter().map(|(name, url)| load_list(&client, name, url, force))).await;
    let stale = loaded.iter().any(|l| l.as_ref().is_none_or(|l| l.stale));
    let lists: Vec<Vec<Entry>> = loaded.iter().map(|l| l.as_ref().map(|l| parse(&l.text)).unwrap_or_default()).collect();
    let (channels, categories, languages) = build(&lists);
    if channels.is_empty() {
        return Err("Couldn't load the channel list. Please check your internet connection and try again.".into());
    }
    let updated = loaded
        .iter()
        .flatten()
        .filter_map(|l| l.fetched.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .min()
        .unwrap_or(0);
    let g = LiveGuide { channels, categories, languages, updated, stale };
    *held = Some((Instant::now(), g.clone()));
    Ok(g)
}

/// Drops the guide and the saved lists, for Settings -> Clear cache.
pub async fn forget() {
    *GUIDE.lock().await = None;
    let _ = tokio::fs::remove_dir_all(lists_dir()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    const UA_LINE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

    fn entry(name: &str, group: &str, url: &str) -> Entry {
        Entry { name: name.into(), group: group.into(), url: url.into(), ..Default::default() }
    }

    #[test]
    fn reads_an_entry_and_the_options_under_it() {
        let text = format!(
            "#EXTM3U x-tvg-url=\"https://example.com/epg.xml\"\n\
             #EXTINF:-1 tvg-id=\"248\" tvg-logo=\"https://img.example/mtv.png\" tvg-language=\"Hindi\" group-title=\"Hindi - Entertainment\",MTV (396p)\n\
             #EXTVLCOPT:http-user-agent={UA_LINE}\n\
             #EXTVLCOPT:http-referrer=https://bhoomtv.org/\n\
             https://example.com/mtv/index.m3u8\n"
        );
        let got = parse(&text);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "MTV (396p)");
        assert_eq!(got[0].logo, "https://img.example/mtv.png");
        assert_eq!(got[0].group, "Hindi - Entertainment");
        assert_eq!(got[0].language, "Hindi");
        assert_eq!(got[0].user_agent, UA_LINE);
        assert_eq!(got[0].referrer, "https://bhoomtv.org/");
        assert_eq!(got[0].url, "https://example.com/mtv/index.m3u8");
    }

    /// Copied from the iptv-org list: the user agent sits in an attribute, has commas of
    /// its own, and was pasted with the option name still on the front, twice.
    #[test]
    fn survives_a_user_agent_pasted_with_its_option_name() {
        let text = format!(
            "#EXTINF:-1 tvg-id=\"Aastha.in@SD\" tvg-logo=\"https://i.imgur.com/IqgrV92.png\" http-user-agent=\"#EXTVLCOPT:http-user-agent={UA_LINE}\" group-title=\"Religious\",Aastha (576p)\n\
             #EXTVLCOPT:http-user-agent=#EXTVLCOPT:http-user-agent={UA_LINE}\n\
             http://103.175.73.12:8080/live/338/master.m3u8\n"
        );
        let got = parse(&text);
        assert_eq!(got[0].name, "Aastha (576p)");
        assert_eq!(got[0].group, "Religious");
        assert_eq!(got[0].user_agent, UA_LINE);
    }

    #[test]
    fn options_do_not_leak_into_the_next_channel() {
        let text = "#EXTINF:-1,One\n#EXTVLCOPT:http-referrer=https://one.example/\nhttps://a.example/1.m3u8\n#EXTINF:-1,Two\nhttps://a.example/2.m3u8\n";
        let got = parse(text);
        assert_eq!(got[0].referrer, "https://one.example/");
        assert_eq!(got[1].referrer, "");
    }

    #[test]
    fn reads_headers_given_kodi_style_and_skips_drm() {
        let text = "#EXTINF:-1,Piped\nhttps://a.example/p.m3u8|User-Agent=VLC/3.0&Referer=https://r.example/\n\
                    #EXTINF:-1,Locked\n#KODIPROP:inputstream.adaptive.license_key=abc\nhttps://a.example/drm.mpd\n\
                    #EXTINF:-1,After\nhttps://a.example/after.m3u8\n";
        let got = parse(text);
        assert_eq!(got.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Piped", "After"]);
        assert_eq!(got[0].url, "https://a.example/p.m3u8");
        assert_eq!(got[0].user_agent, "VLC/3.0");
        assert_eq!(got[0].referrer, "https://r.example/");
    }

    #[test]
    fn takes_the_notes_off_a_title() {
        assert_eq!(read_title("Aaj Tak (1080p)"), Title { name: "Aaj Tak".into(), height: 1080, ..Default::default() });
        assert_eq!(read_title("ET Now (720p) [Geo-blocked]"), Title { name: "ET Now".into(), height: 720, geo_blocked: true, part_time: false });
        assert_eq!(read_title("Sansad TV [Not 24/7]"), Title { name: "Sansad TV".into(), height: 0, geo_blocked: false, part_time: true });
        assert_eq!(read_title("History TV18 HD").name, "History TV18");
        // A bracket that is not a note stays in the name, and a channel called "HD" keeps it.
        assert_eq!(read_title("HMTV (India) (720p)").name, "HMTV");
        assert_eq!(read_title("Discovery (Tamil) (576p)").name, "Discovery (Tamil)");
        assert_eq!(read_title("HD").name, "HD");
    }

    #[test]
    fn the_same_channel_gets_the_same_key_however_it_is_written() {
        assert_eq!(channel_key("Aastha HD"), channel_key("aastha"));
        assert_eq!(channel_key("News18 India"), channel_key("News 18 India"));
        assert_eq!(channel_key(&read_title("HMTV (India) (720p)").name), channel_key("HMTV"));
        assert_eq!(channel_key("&TV"), channel_key("And TV"));
        assert_eq!(channel_key("9 Plus News"), channel_key("9+ News"));
        assert_ne!(channel_key("Discovery (Tamil)"), channel_key("Discovery (Hindi)"));
        assert_ne!(channel_key("Zee News"), channel_key("Zee 24 News"));
    }

    #[test]
    fn folds_two_lists_into_one_tile_per_channel() {
        let first = vec![
            Entry { logo: "https://img.example/aastha.png".into(), ..entry("Aastha (576p)", "Religious", "http://a.example/sd.m3u8") },
            entry("Aastha HD (720p)", "Religious", "https://a.example/hd.m3u8"),
            entry("ET Now (720p) [Geo-blocked]", "Business", "https://b.example/etnow.m3u8"),
        ];
        let second = vec![
            // Same address as the first list: nothing new.
            entry("ET Now (720p)", "English - Business", "https://B.example/etnow.m3u8"),
            // Same channel, a third address.
            entry("Aastha (1080p)", "Hindi - Religious", "https://c.example/aastha.m3u8"),
            // The first list's address again under another name: an alias, not a channel.
            entry("Aastha Bhakti", "Hindi - Religious", "http://a.example/sd.m3u8"),
            entry("Zee Cinema (1080p)", "Hindi - Movies", "https://z.example/cinema.m3u8"),
        ];
        let (channels, categories, languages) = build(&[first, second]);
        assert_eq!(channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Aastha", "ET Now", "Zee Cinema"]);

        let aastha = &channels[0];
        assert_eq!(aastha.id, "aastha");
        assert_eq!(aastha.logo.as_deref(), Some("https://img.example/aastha.png"));
        assert_eq!(aastha.language.as_deref(), Some("Hindi"), "the second list knows the language the first does not");
        assert_eq!(aastha.height, 1080);
        assert_eq!(aastha.streams.iter().map(|s| s.url.as_str()).collect::<Vec<_>>(), ["https://c.example/aastha.m3u8", "https://a.example/hd.m3u8", "http://a.example/sd.m3u8"]);

        assert_eq!(channels[1].streams.len(), 1);
        assert_eq!(channels[1].language.as_deref(), Some("English"), "a repeated address adds no link but still says the language");
        assert_eq!(categories, ["Movies", "Religious", "Business"]);
        assert_eq!(languages, ["Hindi", "English"]);
    }

    #[test]
    fn a_catch_all_category_gives_way_to_an_exact_one() {
        let (channels, _, _) = build(&[
            vec![entry("Colors", "General", "https://a.example/1.m3u8"), entry("Mystery", "Undefined", "https://a.example/3.m3u8")],
            vec![entry("Colors HD", "Hindi - Movies", "https://a.example/2.m3u8"), entry("Sony Pal", "International | Series", "https://a.example/4.m3u8")],
        ]);
        let by_name = |n: &str| channels.iter().find(|c| c.name == n).unwrap();
        assert_eq!(by_name("Colors").category, "Movies");
        assert_eq!(by_name("Mystery").category, "Other");
        assert_eq!(by_name("Sony Pal").category, "Entertainment");
        assert_eq!(by_name("Sony Pal").language.as_deref(), Some("International"));
    }

    #[test]
    fn a_link_that_is_blocked_or_part_time_goes_last() {
        let (channels, _, _) = build(&[vec![
            entry("Times Now (1080p) [Geo-blocked]", "News", "https://a.example/geo.m3u8"),
            entry("Times Now (480p) [Not 24/7]", "News", "https://a.example/part.m3u8"),
            entry("Times Now (720p)", "News", "https://a.example/plain.m3u8"),
        ]]);
        assert_eq!(channels[0].streams.iter().map(|s| s.url.as_str()).collect::<Vec<_>>(), ["https://a.example/plain.m3u8", "https://a.example/part.m3u8", "https://a.example/geo.m3u8"]);
    }

    /// The lists are untrusted text bound for the video engine.
    #[test]
    fn keeps_only_web_addresses_and_one_line_headers() {
        let hostile = vec![
            entry("File", "News", "file:///C:/Windows/win.ini"),
            entry("Edl", "News", "edl://https://a.example/x.m3u8"),
            entry("Spaced", "News", "https://a.example/a b.m3u8"),
            Entry { logo: "javascript:alert(1)".into(), user_agent: "ok\r\nX-Injected: 1".into(), referrer: "https://r.example/".into(), ..entry("Fine", "News", "HTTPS://a.example/ok.m3u8") },
        ];
        let (channels, _, _) = build(&[hostile]);
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "Fine");
        assert_eq!(channels[0].logo, None);
        assert_eq!(channels[0].streams[0].headers, [("User-Agent".to_string(), STREAM_UA.to_string()), ("Referer".to_string(), "https://r.example/".to_string())]);
    }
}
