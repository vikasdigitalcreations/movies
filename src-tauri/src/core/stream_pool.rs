//! Ported from MovieBox-Tui `src/tui/app/requests.rs` (EpisodeStreamsReady): merge
//! releases from several fetches, drop duplicates, order best quality first.
use moviebox_tui::providers::Release;

fn base_link(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// Merge `incoming` into `pool` for one (season, episode), de-duplicating by link
/// (ignoring query strings) or by filename. Existing entries without mirrors adopt
/// the incoming mirrors (and their headers).
pub fn merge_releases(pool: &mut Vec<Release>, incoming: Vec<Release>, season: usize, episode: usize) {
    for mut item in incoming {
        let (mut se, mut ep) = (item.season.unwrap_or(0), item.episode.unwrap_or(0));
        if season == 0 && episode == 0 {
            se = 0;
            ep = 0;
        } else if se == 0 && ep == 0 {
            se = season;
            ep = episode;
        }
        if se != season || ep != episode {
            continue;
        }
        item.season = Some(se);
        item.episode = Some(ep);
        let link = item.direct_url().unwrap_or("").to_string();
        let mut exists = false;
        for existing in pool.iter_mut() {
            let other = existing.direct_url().unwrap_or("");
            let same_link = !link.is_empty() && base_link(&link) == base_link(other);
            let same_name = !item.filename.is_empty() && item.filename == existing.filename;
            if same_link || same_name {
                if existing.mirrors.is_empty() && !item.mirrors.is_empty() {
                    existing.mirrors = item.mirrors.clone();
                }
                if existing.size_bytes.is_none() {
                    existing.size_bytes = item.size_bytes;
                }
                if existing.resource_id.is_none() {
                    existing.resource_id = item.resource_id.clone();
                }
                exists = true;
                break;
            }
        }
        if !exists {
            pool.push(item);
        }
    }
    sort_best_first(pool);
}

/// Multi-resolution (DASH) streams first (mpv adapts), then highest resolution.
pub fn sort_best_first(pool: &mut [Release]) {
    pool.sort_by_key(|r| {
        (
            std::cmp::Reverse(r.is_multi_resolution()),
            std::cmp::Reverse(r.resolution_u64()),
        )
    });
}

/// Pick the best playable release at or below `preferred` (0 = best available).
pub fn pick_for_quality(pool: &[Release], preferred: u64) -> Option<&Release> {
    let playable: Vec<&Release> = pool.iter().filter(|r| !r.mirrors.is_empty()).collect();
    if preferred == 0 {
        return playable.first().copied();
    }
    playable
        .iter()
        .copied()
        .filter(|r| !r.is_multi_resolution() && r.resolution_u64() <= preferred)
        .max_by_key(|r| r.resolution_u64())
        .or_else(|| playable.iter().copied().find(|r| r.is_multi_resolution()))
        .or_else(|| playable.iter().copied().min_by_key(|r| r.resolution_u64()))
}

/// True when the release is a single file a plain HTTP client can download.
pub fn is_direct_downloadable(r: &Release) -> bool {
    match r.direct_url() {
        Some(u) => {
            let base = base_link(u);
            !(base.ends_with(".mpd") || u.contains("/dash/") || base.ends_with(".m3u8"))
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moviebox_tui::providers::{ProviderKind, SourceMirror};

    fn rel(name: &str, q: &str, url: &str, headers: Vec<(String, String)>) -> Release {
        Release {
            provider: ProviderKind::MovieBox,
            filename: name.into(),
            quality: Some(q.into()),
            codec: None,
            language: None,
            size_bytes: None,
            season: None,
            episode: None,
            mirrors: if url.is_empty() {
                vec![]
            } else {
                vec![SourceMirror {
                    label: "x".into(),
                    resolver_url: url.into(),
                    headers,
                    direct_file: true,
                }]
            },
            resource_id: None,
        }
    }

    #[test]
    fn dedupes_by_link_ignoring_query_and_by_filename() {
        let mut pool = vec![];
        merge_releases(&mut pool, vec![rel("A", "720p", "https://h/a.mp4?t=1", vec![])], 0, 0);
        merge_releases(&mut pool, vec![rel("B", "720p", "https://h/a.mp4?t=2", vec![])], 0, 0);
        merge_releases(&mut pool, vec![rel("A", "720p", "https://h/other.mp4", vec![])], 0, 0);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn orders_best_quality_first_with_multi_on_top() {
        let mut pool = vec![];
        merge_releases(
            &mut pool,
            vec![
                rel("a", "480p", "https://h/1", vec![]),
                rel("b", "1080p", "https://h/2", vec![]),
                rel("c", "multi", "https://h/3.mpd", vec![]),
                rel("d", "720p", "https://h/4", vec![]),
            ],
            0,
            0,
        );
        let q: Vec<_> = pool.iter().map(|r| r.quality.clone().unwrap()).collect();
        assert_eq!(q, vec!["multi", "1080p", "720p", "480p"]);
    }

    #[test]
    fn keeps_headers_and_adopts_mirrors() {
        let hdr = vec![
            ("Cookie".to_string(), "k=v".to_string()),
            ("Referer".to_string(), "https://r".to_string()),
        ];
        let mut pool = vec![rel("same", "720p", "", vec![])];
        merge_releases(&mut pool, vec![rel("same", "720p", "https://h/x", hdr.clone())], 0, 0);
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0].mirrors[0].headers, hdr);
    }

    #[test]
    fn filters_other_episodes() {
        let mut e = rel("ep2", "720p", "https://h/e2", vec![]);
        e.season = Some(1);
        e.episode = Some(2);
        let mut pool = vec![];
        merge_releases(&mut pool, vec![e], 1, 3);
        assert!(pool.is_empty());
    }

    #[test]
    fn picks_quality_at_or_below_preference() {
        let pool = vec![
            rel("b", "1080p", "https://h/2", vec![]),
            rel("d", "720p", "https://h/4", vec![]),
            rel("a", "480p", "https://h/1", vec![]),
        ];
        assert_eq!(pick_for_quality(&pool, 720).unwrap().filename, "d");
        assert_eq!(pick_for_quality(&pool, 0).unwrap().filename, "b");
        assert_eq!(pick_for_quality(&pool, 360).unwrap().filename, "a");
    }

    #[test]
    fn dash_is_not_direct_downloadable() {
        assert!(!is_direct_downloadable(&rel("x", "multi", "https://h/a.mpd?x=1", vec![])));
        assert!(is_direct_downloadable(&rel("x", "720p", "https://h/a.mp4?sign=1", vec![])));
    }
}
