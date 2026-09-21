//! Free full films from the official YouTube channels of film distributors.
//!
//! Distributors such as Goldmines and Shemaroo upload complete films to their own verified
//! channels. This source searches YouTube for a title, keeps only results from a fixed list
//! of those channels, and hands the film to mpv. It is deliberately narrow: an unofficial
//! re-upload of a film is never offered, and a result that might be a different film with
//! the same name is dropped, because playing the wrong film is worse than playing nothing.
//!
//! yt-dlp does the talking to YouTube (see `ytdlp`); this module decides what counts as a
//! match and turns its answer into streams.
use crate::core::types::StreamDto;
use crate::core::ytdlp;
use serde::Deserialize;
use std::time::Duration;

/// The channels whose full-length uploads are the rights holder's own. Matched on channel
/// id, never on name, so a channel that merely calls itself "Goldmines" does not count.
/// Adding a channel here is the whole of adding a source.
pub const OFFICIAL_CHANNELS: &[(&str, &str)] = &[
    ("UCyoXW-Dse7fURq30EWl_CUA", "Goldmines"),
    ("UCBOmfqgTZi7yDp4-3Lr_3lA", "Shemaroo Movies"),
    ("UCF1JIbMUs6uqoZEY1Haw0GQ", "Shemaroo"),
    ("UCYauDsl-rswjQGpA0_-Z9Pw", "Ultra Movie Parlour"),
    ("UCX52tYZiEh_mHoFja3Veciw", "Eros Universe"),
    ("UCq-Fj5jknLsUf-MWSy4_brA", "T-Series"),
];

/// Shorter than this is a clip, a trailer or a song, not the film.
const MIN_FILM_SECS: f64 = 4200.0;

/// Upload titles containing these words are about a film, not the film.
const NOT_THE_FILM: &[&str] = &[
    "trailer", "teaser", "scene", "scenes", "jukebox", "song", "songs", "review", "reaction", "compilation", "promo", "interview", "making",
    "behind", "clip", "clips", "highlights",
];

/// Words that follow a film's name in an upload title without being part of another title
/// ("Kaithi Hindi Dubbed Full Movie"). Anything else after the name -- "Returns", "Part",
/// a number -- means a different film, such as "Maari 2" when "Maari" was wanted.
const FILLER: &[&str] = &[
    "hindi", "new", "full", "movie", "movies", "film", "dubbed", "hd", "fullhd", "4k", "latest", "superhit", "blockbuster", "south", "bollywood",
    "action", "romantic", "comedy", "thriller", "drama", "released", "official", "complete", "uncut", "original", "classic", "free", "watch",
    "in", "starring", "ft",
];

/// One line of `yt-dlp --flat-playlist -j` output.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Hit {
    pub id: String,
    pub title: String,
    pub channel_id: Option<String>,
    pub duration: Option<f64>,
}

pub fn parse_hits(stdout: &str) -> Vec<Hit> {
    stdout.lines().filter_map(|l| serde_json::from_str::<Hit>(l.trim()).ok()).filter(|h| !h.id.is_empty()).collect()
}

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

fn words(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase())
}

/// Four-digit numbers in a title that read as release years.
fn years_in(title: &str) -> Vec<i32> {
    words(title).filter(|w| w.len() == 4).filter_map(|w| w.parse::<i32>().ok()).filter(|y| (1920..=2100).contains(y)).collect()
}

/// The upload must state a year, and it must agree with the year sought.
///
/// A title with no year cannot be checked, and that is where the namesakes hide: measured
/// on real searches, "Don" (1978) matched the 2003 Telugu film *Don Seenu*, and "Zanjeer"
/// (1973) matched the 2013 remake, both from uploads that gave no year. So no year, or no
/// year sought, means no match. A film missed is better than a namesake played.
///
/// Agreement is the same year give or take one, with one allowance: a Hindi *dub* is
/// uploaded years after the film ("Uppena (Hindi) 2026 ... Hindi Dubbed Movie" for a 2021
/// film), so a later year up to ten years on is accepted when the title says "dubbed".
fn year_agrees(title: &str, want: &str) -> bool {
    let Ok(want) = want.trim().parse::<i32>() else { return false };
    let found = years_in(title);
    if found.iter().any(|y| (y - want).abs() <= 1) {
        return true;
    }
    let dubbed = words(title).any(|w| w == "dubbed");
    dubbed && found.iter().any(|y| *y > want && *y - want <= 10)
}

/// Does this upload title start with the film's name (or carry it in brackets)?
///
/// `want` is the requested title already reduced to lower-case letters and digits.
pub fn title_matches(candidate: &str, want: &str) -> bool {
    if want.is_empty() {
        return false;
    }
    if words(candidate).any(|w| NOT_THE_FILM.contains(&w.as_str())) {
        return false;
    }

    // 1. The name at the very start, ending on a word boundary.
    let mut acc = String::new();
    let chars: Vec<(usize, char)> = candidate.char_indices().collect();
    for (n, &(i, c)) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            continue;
        }
        acc.extend(c.to_lowercase());
        if !want.starts_with(&acc) {
            break;
        }
        let ends_word = chars.get(n + 1).map(|&(_, next)| !next.is_alphanumeric()).unwrap_or(true);
        if acc == want && ends_word {
            let rest = &candidate[i + c.len_utf8()..];
            return name_is_complete(rest);
        }
    }

    // 2. The name in brackets, for uploads titled "Local Name (Original Name) 2019".
    let mut rest = candidate;
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')') else { break };
        if norm(&rest[open + 1..open + close]) == want {
            return true;
        }
        rest = &rest[open + close + 1..];
    }
    false
}

/// What follows the name in an upload title: nothing, a separator, or a harmless word.
fn name_is_complete(rest: &str) -> bool {
    let trimmed = rest.trim_start();
    let Some(first) = trimmed.chars().next() else { return true };
    if !first.is_alphanumeric() {
        return true;
    }
    let word: String = trimmed.chars().take_while(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    if FILLER.contains(&word.as_str()) {
        return true;
    }
    // A year is fine; the year check judges it. Any other number is a sequel.
    word.len() == 4 && word.parse::<i32>().map(|y| (1920..=2100).contains(&y)).unwrap_or(false)
}

pub fn is_wanted(hit: &Hit, want: &str, want_year: &str) -> bool {
    hit.channel_id.as_deref().map(|id| OFFICIAL_CHANNELS.iter().any(|(c, _)| *c == id)).unwrap_or(false)
        && hit.duration.map(|d| d >= MIN_FILM_SECS).unwrap_or(false)
        && title_matches(&hit.title, want)
        && year_agrees(&hit.title, want_year)
}

// ---- turning yt-dlp's answer into streams ------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Format {
    format_id: String,
    protocol: String,
    vcodec: Option<String>,
    acodec: Option<String>,
    height: Option<f64>,
    tbr: Option<f64>,
    abr: Option<f64>,
    filesize: Option<f64>,
    filesize_approx: Option<f64>,
    language_preference: Option<f64>,
    url: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Info {
    formats: Vec<Format>,
}

impl Format {
    fn has(codec: &Option<String>) -> bool {
        codec.as_deref().map(|c| c != "none" && !c.is_empty()).unwrap_or(false)
    }
    fn is_video_only(&self) -> bool {
        Self::has(&self.vcodec) && !Self::has(&self.acodec) && self.protocol == "https" && self.height.is_some() && !self.url.is_empty()
    }
    fn is_audio_only(&self) -> bool {
        Self::has(&self.acodec) && !Self::has(&self.vcodec) && self.protocol == "https" && !self.url.is_empty() && !self.format_id.contains("drc")
    }
    fn size(&self) -> Option<u64> {
        self.filesize.or(self.filesize_approx).map(|s| s as u64)
    }
    /// h264 first: every graphics card decodes it. VP9 and AV1 only where nothing else exists.
    fn codec_rank(&self) -> u8 {
        let v = self.vcodec.as_deref().unwrap_or("");
        if v.starts_with("avc1") {
            0
        } else if v.starts_with("vp09") || v.starts_with("vp9") {
            1
        } else {
            2
        }
    }
    fn codec_name(&self) -> &'static str {
        match self.codec_rank() {
            0 => "h264",
            1 => "vp9",
            _ => "av1",
        }
    }
}

/// An mpv address that plays a video track and an audio track together.
///
/// YouTube serves anything above 360p as separate video and audio files. mpv's `edl://`
/// form joins them with `!new_stream`, so the player needs no special case. Each entry is
/// length-prefixed (`%N%`) so a `,` or `;` inside a URL cannot split it.
pub fn edl_pair(video: &str, audio: &str) -> String {
    format!("edl://%{}%{};!new_stream;%{}%{}", video.len(), video, audio.len(), audio)
}

/// One stream per quality up to 1080p, best first, from `yt-dlp -J` output.
pub fn build_streams(info_json: &str) -> Result<Vec<StreamDto>, String> {
    let info: Info = serde_json::from_str(info_json).map_err(|e| format!("unreadable answer from YouTube: {e}"))?;
    let mut audio: Vec<&Format> = info.formats.iter().filter(|f| f.is_audio_only()).collect();
    // The channel's own language first (yt-dlp scores the default track highest), then the clearest.
    audio.sort_by(|a, b| {
        b.language_preference
            .unwrap_or(0.0)
            .partial_cmp(&a.language_preference.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.abr.unwrap_or(0.0).partial_cmp(&a.abr.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal))
    });
    let Some(audio) = audio.first().copied() else {
        return Err("YouTube offered no audio for this film.".into());
    };

    let mut heights: Vec<u64> = info.formats.iter().filter(|f| f.is_video_only()).filter_map(|f| f.height).map(|h| h as u64).collect();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();
    heights.retain(|h| *h <= 1080);
    // 144p and 240p are not worth offering when 360p and up exist.
    if heights.iter().any(|h| *h >= 360) {
        heights.retain(|h| *h >= 360);
    }

    let mut out = Vec::new();
    for h in heights {
        let best = info
            .formats
            .iter()
            .filter(|f| f.is_video_only() && f.height.map(|x| x as u64) == Some(h))
            .min_by(|a, b| a.codec_rank().cmp(&b.codec_rank()).then(b.tbr.unwrap_or(0.0).partial_cmp(&a.tbr.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal)));
        let Some(v) = best else { continue };
        out.push(StreamDto {
            label: format!("{h}p"),
            height: h,
            multi: false,
            size: v.size().zip(audio.size()).map(|(a, b)| a + b),
            codec: Some(v.codec_name().to_string()),
            url: edl_pair(&v.url, &audio.url),
            headers: Vec::new(),
            resource_id: None,
            // Two files joined by the player, not one the downloader can fetch.
            downloadable: false,
            source: "YouTube".into(),
        });
    }
    if out.is_empty() {
        return Err("YouTube offered no playable video for this film.".into());
    }
    Ok(out)
}

/// Look for the film on the official channels and return streams for it.
///
/// `clean` is the title as it should be searched; `want` the same reduced for comparison.
pub async fn find(clean: &str, want: &str, year: &str) -> Result<Vec<StreamDto>, String> {
    let query = format!("ytsearch20:{clean} full movie");
    let listing = ytdlp::run(&[&query, "--flat-playlist", "-j"], Duration::from_secs(40)).await?;
    let hits = parse_hits(&listing);
    let Some(hit) = hits.iter().find(|h| is_wanted(h, want, year)) else {
        return Err("No other source has this title.".into());
    };
    let page = format!("https://www.youtube.com/watch?v={}", hit.id);
    let info = ytdlp::run(&["-J", "--no-playlist", &page], Duration::from_secs(45)).await?;
    build_streams(&info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(title: &str, channel: &str, secs: f64) -> Hit {
        Hit { id: "abc".into(), title: title.into(), channel_id: Some(channel.into()), duration: Some(secs) }
    }

    const GOLDMINES: &str = "UCyoXW-Dse7fURq30EWl_CUA";

    #[test]
    fn the_name_at_the_start_of_an_upload_title_matches() {
        assert!(title_matches("Maska - मस्का (FULL HD) Ram Pothineni & Hansika Motwani Superhit Romantic Movie", "maska"));
        assert!(title_matches("Oxygen (2026) New Released Hindi Dubbed Movie | Gopichand", "oxygen"));
        assert!(title_matches("Dear Comrade (2020) New Released Hindi Dubbed Full Movie", "dearcomrade"));
        assert!(title_matches("Kaithi Hindi Dubbed Full Movie", "kaithi"));
        assert!(title_matches("Sholay", "sholay"));
    }

    #[test]
    fn a_sequel_or_a_longer_title_is_a_different_film() {
        assert!(!title_matches("Maari 2 New Released Full Hindi Dubbed Movie", "maari"));
        assert!(!title_matches("Don 2 Full Movie", "don"));
        assert!(!title_matches("Dear Comrade Returns Full Movie", "dearcomrade"));
        assert!(!title_matches("Maskara Full Movie", "maska"));
        assert!(!title_matches("Baahubali The Beginning Full Movie", "baahubali"));
    }

    #[test]
    fn the_bracketed_original_name_matches() {
        assert!(title_matches("Madam Geeta Rani (Raatchasi) 2020 New Released Hindi Dubbed Full Movie", "raatchasi"));
        assert!(title_matches("Amar Akbhar Anthoni (Amar Akbar Anthony) 2019 New Hindi Dubbed Full Movie", "amarakbaranthony"));
        assert!(!title_matches("Madam Geeta Rani (Raatchasi) 2020", "geeta"));
    }

    #[test]
    fn clips_and_compilations_never_match() {
        assert!(!title_matches("Maska Full Movie Trailer", "maska"));
        assert!(!title_matches("Maska - All Songs Jukebox", "maska"));
        assert!(!title_matches("Maska Best Comedy Scenes", "maska"));
        assert!(!title_matches("Maska Movie Review", "maska"));
    }

    #[test]
    fn an_empty_name_matches_nothing() {
        assert!(!title_matches("Anything at all", ""));
    }

    #[test]
    fn a_year_that_agrees_is_required() {
        assert!(year_agrees("Dear Comrade (2020) New Released", "2019"), "one year out is fine");
        assert!(year_agrees("Dear Comrade (2019) New Released", "2019"));
        assert!(year_agrees("Mother India (1957) Hindi 4K Classic Superhit Full Movie", "1957"));
        assert!(!year_agrees("Don (1978) Full Movie", "2006"));
        assert!(!year_agrees("Zanjeer (2013) Full Movie 4K", "1973"), "the remake is not the original");
        assert!(year_agrees("Zanjeer (2013) Full Movie 4K", "2013"));
        assert!(!years_in("Sholay 1975 HD 1080 60fps 2160").contains(&1080));
        assert!(years_in("Sholay 1975 HD 1080 60fps").contains(&1975));
    }

    #[test]
    fn an_upload_with_no_year_is_refused_because_it_cannot_be_checked() {
        // Both were matched by name alone in a measured run, and both were the wrong film.
        assert!(!year_agrees("Don (Don Seenu) (4K ULTRA HD) - Full Movie | Ravi Teja, Srihari, Shriya Saran", "1978"));
        assert!(!year_agrees("Zanjeer - Full Blockbuster Action Movie (4K) - Ram Charan, Priyanka Chopra", "1973"));
        assert!(!year_agrees("Sholay Full Movie", "1975"));
        // Nor can anything be verified when the year sought is not known.
        assert!(!year_agrees("Dear Comrade (2020) New Released", ""));
    }

    #[test]
    fn a_dub_uploaded_years_later_is_accepted_only_when_it_says_dubbed() {
        assert!(year_agrees("Uppena (Hindi) 2026 New Released Hindi Dubbed Full Movie", "2021"));
        assert!(!year_agrees("Uppena (Hindi) 2026 New Released Full Movie", "2021"), "no 'dubbed', so it could be a remake");
        assert!(!year_agrees("Something (2035) Hindi Dubbed Full Movie", "2021"), "more than ten years on");
        assert!(!year_agrees("Uppena (2015) Hindi Dubbed Full Movie", "2021"), "an earlier film is never the later one");
    }

    #[test]
    fn a_namesake_from_an_official_channel_is_not_wanted() {
        let seenu = hit("Don (Don Seenu) (4K ULTRA HD) - Full Movie | Ravi Teja, Srihari", GOLDMINES, 8050.0);
        assert!(!is_wanted(&seenu, "don", "1978"));
        let remake = hit("Zanjeer - Full Blockbuster Action Movie (4K) - Ram Charan, Priyanka Chopra", "UCF1JIbMUs6uqoZEY1Haw0GQ", 6503.0);
        assert!(!is_wanted(&remake, "zanjeer", "1973"));
    }

    #[test]
    fn only_official_channels_long_enough_to_be_films_are_wanted() {
        let good = hit("Maska (2020) Hindi Dubbed Full Movie", GOLDMINES, 7156.0);
        assert!(is_wanted(&good, "maska", "2020"));
        assert!(!is_wanted(&hit("Maska (2020) Hindi Dubbed Full Movie", "UCsomeoneelse", 7156.0), "maska", "2020"), "a channel off the list");
        assert!(!is_wanted(&hit("Maska (2020) Hindi Dubbed Full Movie", GOLDMINES, 600.0), "maska", "2020"), "too short to be the film");
        assert!(!is_wanted(&Hit { channel_id: None, ..good.clone() }, "maska", "2020"), "no channel id");
        assert!(!is_wanted(&Hit { duration: None, ..good }, "maska", "2020"), "no duration");
    }

    #[test]
    fn reads_search_output_line_by_line() {
        let out = "{\"id\":\"a1\",\"title\":\"One\",\"channel_id\":\"c\",\"duration\":7000}\nnot json\n{\"id\":\"\",\"title\":\"no id\"}\n{\"id\":\"b2\",\"title\":\"Two\",\"duration\":null}\n";
        let hits = parse_hits(out);
        assert_eq!(hits.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), vec!["a1", "b2"]);
        assert_eq!(hits[1].duration, None);
    }

    fn fmt(id: &str, v: &str, a: &str, h: Option<u32>, tbr: f64, url: &str) -> String {
        let height = h.map(|h| h.to_string()).unwrap_or("null".into());
        format!("{{\"format_id\":\"{id}\",\"protocol\":\"https\",\"vcodec\":\"{v}\",\"acodec\":\"{a}\",\"height\":{height},\"tbr\":{tbr},\"abr\":{tbr},\"filesize\":1000,\"url\":\"{url}\"}}")
    }

    #[test]
    fn builds_one_stream_per_quality_with_h264_and_the_clearest_audio() {
        let formats = [
            fmt("160", "avc1.4d400c", "none", Some(144), 89.0, "https://v/144"),
            fmt("134", "avc1.4d401e", "none", Some(360), 364.0, "https://v/360h264"),
            fmt("243", "vp9", "none", Some(360), 900.0, "https://v/360vp9"),
            fmt("136", "avc1.64001f", "none", Some(720), 1340.0, "https://v/720"),
            fmt("137", "avc1.640028", "none", Some(1080), 2500.0, "https://v/1080"),
            fmt("571", "av01.0.16M.08", "none", Some(2160), 9000.0, "https://v/2160"),
            fmt("139", "none", "mp4a.40.5", None, 49.0, "https://a/low"),
            fmt("140", "none", "mp4a.40.2", None, 129.0, "https://a/mid"),
            fmt("140-drc", "none", "mp4a.40.2", None, 129.0, "https://a/drc"),
            "{\"format_id\":\"233\",\"protocol\":\"m3u8_native\",\"vcodec\":\"none\",\"acodec\":\"mp4a\",\"url\":\"https://a/hls\"}".to_string(),
        ];
        let json = format!("{{\"formats\":[{}]}}", formats.join(","));
        let streams = build_streams(&json).unwrap();
        assert_eq!(streams.iter().map(|s| s.height).collect::<Vec<_>>(), vec![1080, 720, 360], "best first, 144p and 2160p left out");
        assert_eq!(streams[2].codec.as_deref(), Some("h264"), "h264 wins over VP9 at the same height");
        assert_eq!(streams[0].url, edl_pair("https://v/1080", "https://a/mid"));
        assert_eq!(streams[0].size, Some(2000));
        assert!(streams.iter().all(|s| !s.downloadable && s.source == "YouTube" && s.headers.is_empty()));
    }

    #[test]
    fn only_tiny_formats_are_still_offered_when_nothing_bigger_exists() {
        let json = format!("{{\"formats\":[{},{}]}}", fmt("160", "avc1", "none", Some(144), 89.0, "https://v/144"), fmt("140", "none", "mp4a", None, 129.0, "https://a"));
        let s = build_streams(&json).unwrap();
        assert_eq!(s.iter().map(|s| s.height).collect::<Vec<_>>(), vec![144]);
    }

    #[test]
    fn a_film_with_no_audio_or_no_video_is_an_error() {
        let no_audio = format!("{{\"formats\":[{}]}}", fmt("136", "avc1", "none", Some(720), 1340.0, "https://v"));
        assert!(build_streams(&no_audio).is_err());
        let no_video = format!("{{\"formats\":[{}]}}", fmt("140", "none", "mp4a", None, 129.0, "https://a"));
        assert!(build_streams(&no_video).is_err());
        assert!(build_streams("not json").is_err());
    }

    #[test]
    fn the_joined_address_length_prefixes_both_parts() {
        let e = edl_pair("https://v/x?a=1,2;3", "https://a/y");
        assert_eq!(e, "edl://%19%https://v/x?a=1,2;3;!new_stream;%11%https://a/y");
    }
}
