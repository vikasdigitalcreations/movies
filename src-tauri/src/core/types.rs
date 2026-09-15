use moviebox_tui::providers::{CatalogItem, MediaDetails, MediaType, Release};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub title: String,
    pub year: Option<String>,
    pub poster: Option<String>,
    pub media_type: String,
}

impl From<CatalogItem> for Card {
    fn from(c: CatalogItem) -> Self {
        Card {
            id: c.id.value,
            title: clean_title(&c.title),
            year: c.year,
            poster: c.poster_url,
            media_type: media_type_str(c.media_type).into(),
        }
    }
}

pub fn media_type_str(m: MediaType) -> &'static str {
    match m {
        MediaType::Movie => "movie",
        MediaType::Series => "series",
    }
}

pub fn clean_title(t: &str) -> String {
    let stripped = moviebox_tui::providers::models::strip_emojis(t);
    let s = stripped.trim();
    if s.is_empty() {
        t.to_string()
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeRow {
    pub title: String,
    pub kind: String,
    pub items: Vec<Card>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeDto {
    pub number: usize,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonDto {
    pub number: usize,
    pub episodes: Vec<EpisodeDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DubDto {
    pub subject_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailsDto {
    pub id: String,
    pub title: String,
    pub media_type: String,
    pub year: Option<String>,
    pub description: Option<String>,
    pub imdb_rating: Option<String>,
    pub director: Option<String>,
    pub stars: Option<String>,
    pub poster: Option<String>,
    pub duration: Option<String>,
    pub genres: Vec<String>,
    pub seasons: Vec<SeasonDto>,
    pub dubs: Vec<DubDto>,
    pub is_favorite: bool,
}

impl DetailsDto {
    pub fn from_details(d: &MediaDetails, is_favorite: bool) -> Self {
        DetailsDto {
            id: d.id.value.clone(),
            title: clean_title(&d.title),
            media_type: if d.is_series() { "series" } else { "movie" }.into(),
            year: d.year.clone(),
            description: d.description.clone(),
            imdb_rating: d.imdb_rating.clone().filter(|r| !r.is_empty() && r != "0"),
            director: d.director.clone().filter(|s| !s.is_empty()),
            stars: d.stars.clone().filter(|s| !s.is_empty()),
            poster: d.poster_url.clone(),
            duration: d.duration.clone().filter(|s| !s.is_empty()),
            genres: d.genres.clone(),
            seasons: d
                .seasons
                .iter()
                .map(|s| SeasonDto {
                    number: s.number,
                    episodes: s
                        .episodes
                        .iter()
                        .map(|e| EpisodeDto { number: e.number, title: e.title.clone() })
                        .collect(),
                })
                .collect(),
            dubs: d
                .dubs
                .iter()
                .map(|a| DubDto { subject_id: a.subject_id.clone(), label: a.label.clone() })
                .collect(),
            is_favorite,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamDto {
    pub label: String,
    pub height: u64,
    pub multi: bool,
    pub size: Option<u64>,
    pub codec: Option<String>,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub resource_id: Option<String>,
    pub downloadable: bool,
    pub source: String,
}

impl StreamDto {
    pub fn from_release(r: &Release) -> Option<Self> {
        let m = r.mirrors.first()?;
        let multi = r.is_multi_resolution();
        let height = r.resolution_u64();
        Some(StreamDto {
            label: if multi { "Auto".into() } else { format!("{height}p") },
            height: if multi { 0 } else { height },
            multi,
            size: r.size_bytes,
            codec: r.codec.clone(),
            url: m.resolver_url.clone(),
            headers: m.headers.clone(),
            resource_id: r.resource_id.clone(),
            downloadable: super::stream_pool::is_direct_downloadable(r),
            source: r.provider.label().to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleDto {
    pub name: String,
    pub url: String,
}

pub type CmdResult<T> = Result<T, String>;

/// Plain-language error text for the UI.
pub fn friendly(err: impl std::fmt::Display) -> String {
    let raw = err.to_string();
    let lower = raw.to_lowercase();
    if lower.contains("network") || lower.contains("dns") || lower.contains("connect") || lower.contains("timed out") || lower.contains("timeout") {
        "Can't reach the server. Please check your internet connection and try again.".into()
    } else if lower.contains("rate limit") {
        "The server is busy right now. Please wait a moment and try again.".into()
    } else if lower.contains("not found") {
        "Sorry, this title isn't available right now.".into()
    } else {
        format!("Something went wrong: {raw}")
    }
}
