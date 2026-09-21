// Developer probe: prints the shape of live API payloads. Not part of the app UI.
use moviebox_tui::providers::{ProviderKind, ReleaseProvider, ResolutionIntent};
use moviebox_tui::service::MovieBoxService;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

fn short(v: &serde_json::Value, n: usize) -> String {
    v.to_string().chars().take(n).collect()
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let svc = MovieBoxService::new();
    match args.get(1).map(String::as_str).unwrap_or("home") {
        "home" => {
            for tab in ["0", "1", "2", "3", "4", "5", "6", "7", "8"] {
                match svc.client.get_homepage(tab, 1).await {
                    Ok(v) => {
                        let groups = v.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default();
                        println!("TAB {tab}: {} groups", groups.len());
                        for g in groups.iter().take(16) {
                            let keys: Vec<String> = g.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                            let subj = g.get("subjects").and_then(|s| s.as_array()).map(|a| a.len()).unwrap_or(0);
                            let banners = g.get("banner").and_then(|b| b.get("banners")).and_then(|b| b.as_array()).map(|a| a.len()).unwrap_or(0);
                            let custom = g.get("customData").and_then(|c| c.get("items")).and_then(|i| i.as_array()).map(|a| a.len()).unwrap_or(0);
                            println!(
                                "   type={} title={} subjects={} banners={} custom={} keys={:?}",
                                g.get("type").map(|t| t.to_string()).unwrap_or_default(),
                                g.get("title").map(|t| t.to_string()).unwrap_or_default(),
                                subj, banners, custom, keys
                            );
                        }
                    }
                    Err(e) => println!("TAB {tab}: error {e}"),
                }
            }
        }
        "raw" => {
            let tab = args.get(2).cloned().unwrap_or_else(|| "0".into());
            let v = svc.client.get_homepage(&tab, 1).await.unwrap();
            println!("{}", short(&v, 6000));
        }
        "search" => {
            let q = args.get(2).cloned().unwrap_or_else(|| "Inception".into());
            let r = svc.search_typed(ProviderKind::MovieBox, &q, 1).await;
            println!("{:#?}", r.map(|v| v.into_iter().take(5).collect::<Vec<_>>()));
        }
        "details" => {
            let id = args.get(2).unwrap();
            let d = svc.details_typed(ProviderKind::MovieBox, id).await.unwrap();
            println!(
                "title={} type={:?} year={:?} dur={:?} genres={:?} dubs={:?} seasons={}",
                d.title, d.media_type, d.year, d.duration, d.genres, d.dubs, d.seasons.len()
            );
            for s in d.seasons.iter().take(3) {
                println!("  S{} eps={} first={:?}", s.number, s.episodes.len(), s.episodes.first());
            }
            println!("poster={:?} stars={:?} imdb={:?}", d.poster_url, d.stars, d.imdb_rating);
            let raw = svc.client.get_details(id).await.unwrap();
            println!("RAW {}", short(&raw, 2500));
        }
        "streams" => {
            let id = args.get(2).unwrap();
            let se: usize = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(0);
            let ep: usize = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(0);
            match ReleaseProvider::episode_streams(&svc.client, id, se, ep).await {
                Ok(v) => {
                    println!("play-info releases: {}", v.len());
                    for x in &v {
                        let m = x.mirrors.first();
                        println!(
                            "  {} q={:?} size={:?} rid={:?} url={:?} hdrs={:?}",
                            x.filename, x.quality, x.size_bytes, x.resource_id,
                            m.map(|m| m.resolver_url.chars().take(100).collect::<String>()),
                            m.map(|m| m.headers.iter().map(|h| h.0.clone()).collect::<Vec<_>>())
                        );
                    }
                }
                Err(e) => println!("play-info err {e}"),
            }
            let page = if ep > 0 { (ep - 1) / 20 + 1 } else { 1 };
            match svc.client.fetch_resource_page(id, se, ep, 0, page).await {
                Ok((items, pager)) => {
                    println!("resource page: {} items pager={}", items.len(), pager);
                    for it in items.iter().take(4) {
                        println!("  {}", short(it, 700));
                    }
                }
                Err(e) => println!("resource err {e}"),
            }
            println!("resolutions {:?}", svc.fetch_collection_resolutions(id).await);
        }
        "fourk" => {
            let q = args.get(2).cloned().unwrap_or_else(|| "Inception".into());
            let r = svc.search_typed(ProviderKind::FourKHdHub, &q, 1).await;
            println!("{:#?}", r.map(|v| v.into_iter().take(5).collect::<Vec<_>>()));
        }
        // Sample many titles and report, for each, whether MovieBox still serves a real
        // stream: `cargo run --bin probe -- survey`. Answers "why doesn't it play?"
        // with numbers instead of guesses.
        "survey" => {
            let titles: Vec<String> = if args.len() > 2 {
                args[2..].to_vec()
            } else {
                [
                    "Inception", "Oppenheimer", "The Dark Knight", "Dune: Part Two",
                    "Interstellar", "Barbie", "Joker", "Avatar", "Deadpool",
                    "Breaking Bad", "Stranger Things", "The Boys", "Wednesday",
                    "Jawan", "Pathaan", "Animal", "3 Idiots",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect()
            };
            let mut ok = 0usize;
            let mut no_hit = 0usize;
            let mut no_stream = 0usize;
            let mut advert_only = 0usize;
            let http = svc.client.http_client();
            for t in &titles {
                let hits = svc.search_typed(ProviderKind::MovieBox, t, 1).await.unwrap_or_default();
                let Some(first) = hits.into_iter().next() else {
                    println!("{t:<18} NO SEARCH HIT");
                    no_hit += 1;
                    continue;
                };
                let is_series = matches!(first.media_type, moviebox_tui::providers::MediaType::Series);
                let (se, ep) = if is_series { (1, 1) } else { (0, 0) };
                let rels = ReleaseProvider::episode_streams(&svc.client, &first.id.value, se, ep)
                    .await
                    .unwrap_or_default();
                let mirrors: Vec<&moviebox_tui::providers::SourceMirror> =
                    rels.iter().flat_map(|r| r.mirrors.iter()).collect();
                if mirrors.is_empty() {
                    println!("{t:<18} NO STREAM (search hit: {})", first.title);
                    no_stream += 1;
                    continue;
                }
                let advert = |u: &str| u.to_ascii_lowercase().contains("aoneroom.com/other/");
                let real: Vec<&&moviebox_tui::providers::SourceMirror> =
                    mirrors.iter().filter(|m| !advert(&m.resolver_url)).collect();
                if real.is_empty() {
                    println!("{t:<18} ADVERT CLIP ONLY ({} mirrors)", mirrors.len());
                    advert_only += 1;
                    continue;
                }
                let m = real[0];
                let dash = m.resolver_url.contains(".mpd");
                let mut req = http.get(&m.resolver_url).header("Range", "bytes=0-255");
                for (k, v) in &m.headers {
                    req = req.header(k.as_str(), v.as_str());
                }
                let status = match req.send().await {
                    Ok(r) => r.status().as_u16(),
                    Err(e) => {
                        println!("{t:<18} FETCH ERROR {e}");
                        0
                    }
                };
                // CloudFront signs these; the deadline sits in a query parameter.
                let expiry = m
                    .resolver_url
                    .split(|c| c == '?' || c == '&')
                    .filter_map(|kv| kv.split_once('='))
                    .find(|(k, _)| matches!(k.to_ascii_lowercase().as_str(), "expires" | "exp" | "expire" | "e" | "et"))
                    .and_then(|(_, v)| v.parse::<i64>().ok());
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let life = expiry.map(|e| format!("{} min", (e - now) / 60)).unwrap_or_else(|| "unknown".into());
                println!(
                    "{t:<18} OK  dash={dash} http={status} mirrors={} valid_for={life}",
                    mirrors.len()
                );
                if (200..300).contains(&status) {
                    ok += 1;
                }
            }
            println!(
                "
{} titles: {} playable, {} no search hit, {} no stream, {} advert only",
                titles.len(), ok, no_hit, no_stream, advert_only
            );
            // One fixed line for scripts: the auto-update workflow compares it before and
            // after a re-vendor to decide whether a new upstream version is safe to ship.
            println!("SURVEY playable={ok} total={}", titles.len());
        }
        // Print a mirror's full URL and headers, and decode the CloudFront policy so the
        // exact expiry deadline is visible: `cargo run --bin probe -- mirror <id> [se] [ep]`.
        "mirror" => {
            let id = args.get(2).unwrap();
            let se: usize = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(0);
            let ep: usize = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(0);
            let rels = ReleaseProvider::episode_streams(&svc.client, id, se, ep).await.unwrap_or_default();
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
            for r in &rels {
                for m in &r.mirrors {
                    println!("URL {}", m.resolver_url);
                    for (k, v) in &m.headers {
                        println!("  {k}: {v}");
                        if k.eq_ignore_ascii_case("cookie") {
                            for part in v.split(';') {
                                if let Some((key, val)) = part.trim().split_once('=') {
                                    if key.eq_ignore_ascii_case("CloudFront-Expires") {
                                        if let Ok(e) = val.parse::<i64>() {
                                            println!("    -> expires in {} min", (e - now) / 60);
                                        }
                                    }
                                    if key.eq_ignore_ascii_case("CloudFront-Policy") {
                                        let fixed: String = val.replace('-', "+").replace('_', "=").replace('~', "/");
                                        if let Ok(raw) = BASE64.decode(fixed.as_bytes()) {
                                            println!("    policy {}", String::from_utf8_lossy(&raw));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // Resolve a 4KHDHub release end to end, so "is the fallback alive?" has an answer:
        // `cargo run --bin probe -- fourkplay "<title>"`.
        "fourkplay" => {
            let q = args.get(2).cloned().unwrap_or_else(|| "Inception".into());
            let Some(fourk) = svc.fourk_client.clone() else {
                println!("4KHDHub client unavailable");
                return;
            };
            let hits = svc.search_typed(ProviderKind::FourKHdHub, &q, 1).await.unwrap_or_default();
            let Some(item) = hits.into_iter().next() else {
                println!("{q}: no 4KHDHub hit");
                return;
            };
            println!("{q} -> {} ({:?}) {}", item.title, item.year, item.id.value);
            match ReleaseProvider::episode_streams(&fourk, &item.id.value, 0, 0).await {
                Ok(rels) => {
                    println!("  {} releases", rels.len());
                    for rel in rels.iter().take(5) {
                        print!("  {}p {} -> ", rel.resolution_u64(), rel.filename);
                        match fourk.resolve_release(rel, ResolutionIntent::Playback).await {
                            Ok(src) => println!("OK {}", src.url.chars().take(90).collect::<String>()),
                            Err(e) => println!("FAILED: {e}"),
                        }
                    }
                }
                Err(e) => println!("  episode_streams failed: {e}"),
            }
        }
        // Show the raw mirror links 4KHDHub hands out, before any resolving, so a dead
        // fallback can be told apart from a broken resolver:
        // `cargo run --bin probe -- fourkmirrors "<title>"`.
        "fourkmirrors" => {
            let q = args.get(2).cloned().unwrap_or_else(|| "Inception".into());
            let Some(fourk) = svc.fourk_client.clone() else { return };
            let hits = svc.search_typed(ProviderKind::FourKHdHub, &q, 1).await.unwrap_or_default();
            let Some(item) = hits.into_iter().next() else { return };
            let rels = ReleaseProvider::episode_streams(&fourk, &item.id.value, 0, 0).await.unwrap_or_default();
            let http = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0 Safari/537.36")
                .build()
                .unwrap();
            for rel in rels.iter().take(2) {
                println!("RELEASE {}p {}", rel.resolution_u64(), rel.filename);
                for m in &rel.mirrors {
                    println!("  mirror {} {}", m.label, m.resolver_url);
                    match tokio::time::timeout(std::time::Duration::from_secs(20), http.get(&m.resolver_url).send()).await {
                        Ok(Ok(r)) => {
                            let st = r.status();
                            let body = r.text().await.unwrap_or_default();
                            println!("HTTP {st} body={} bytes", body.len());
                        }
                        Ok(Err(e)) => println!("ERR {e}"),
                        Err(_) => println!("TIMEOUT after 20s"),
                    }
                }
            }
        }
        // Compare a raw search against MovieBox's own autocomplete, to work out the best
        // rescue for a query that returns nothing: `cargo run --bin probe -- rescue "Barbie"`.
        "rescue" => {
            let q = args.get(2).cloned().unwrap_or_else(|| "Barbie".into());
            let direct = svc.search_typed(ProviderKind::MovieBox, &q, 1).await.unwrap_or_default();
            println!("direct search '{q}': {} hits", direct.len());
            for c in direct.iter().take(3) {
                println!("   {} ({:?})", c.title, c.year);
            }
            let sugg = svc.suggest(&q).await.unwrap_or_default();
            println!("suggest '{q}': {sugg:?}");
            for s in sugg.iter().take(3) {
                let hits = svc.search_typed(ProviderKind::MovieBox, s, 1).await.unwrap_or_default();
                println!("   search '{s}': {} hits -> {:?}", hits.len(), hits.first().map(|c| (c.title.clone(), c.year.clone())));
            }
        }
        // Walk the app's own failover for each title and say exactly where it stops:
        // `cargo run --bin probe -- chain "Inception" "The Boys" ...`
        "chain" => {
            let titles: Vec<String> = if args.len() > 2 {
                args[2..].to_vec()
            } else {
                [
                    "Inception", "Dune: Part Two", "Barbie", "Oppenheimer", "The Batman",
                    "Breaking Bad", "Stranger Things", "The Boys", "Wednesday", "Loki",
                    "Jawan", "Animal", "Kalki 2898 AD", "Stree 2", "Munjya",
                    "Godzilla x Kong", "Furiosa", "Civil War", "The Fall Guy", "Twisters",
                ]
                .iter().map(|s| s.to_string()).collect()
            };
            let mut served = 0usize;
            for t in &titles {
                let hits = svc.search_typed(ProviderKind::MovieBox, t, 1).await.unwrap_or_default();
                let Some(first) = hits.into_iter().next() else {
                    println!("{t:<20} | no MovieBox hit");
                    continue;
                };
                let is_series = matches!(first.media_type, moviebox_tui::providers::MediaType::Series);
                let (se, ep) = if is_series { (1, 1) } else { (0, 0) };
                let raw = first.title.clone();
                let clean = moviebox_tui::providers::moviebox::clean_moviebox_title(&raw).to_string();
                let year = first.year.clone().unwrap_or_default();

                // tier 1
                let rels = ReleaseProvider::episode_streams(&svc.client, &first.id.value, se, ep).await.unwrap_or_default();
                let mirrors: Vec<_> = rels.iter().flat_map(|r| r.mirrors.iter()).collect();
                let real = mirrors.iter().filter(|m| !m.resolver_url.to_ascii_lowercase().contains("aoneroom.com/other/")).count();
                let tier1 = if real > 0 { "MovieBox OK" } else if !mirrors.is_empty() { "advert only" } else { "no stream" };

                // tier 2, through the app's own matcher so this measures shipped behaviour
                let t2 = moviebox_lib::commands::streams::fourk_streams(
                    &svc, &raw, first.year.as_deref(), se, ep, 0, 2,
                )
                .await;
                let mut line = match &t2 {
                    Ok(list) => format!("4KHDHub {} stream(s)", list.len()),
                    Err(e) => format!("4KHDHub: {e}"),
                };
                // tier 3, only consulted when the first two had nothing, as in the app
                let mut t3_ok = false;
                if real == 0 && t2.is_err() {
                    let t3 = moviebox_lib::commands::streams::dramachi_streams(&svc, &raw, first.year.as_deref(), se, ep).await;
                    t3_ok = t3.is_ok();
                    line += &match t3 {
                        Ok(list) => format!(" | Dramachi {} stream(s) at {}p", list.len(), list[0].height),
                        Err(e) => format!(" | Dramachi: {e}"),
                    };
                }
                if real > 0 || t2.is_ok() || t3_ok { served += 1; }
                let _ = clean;
                println!("{t:<20} | {tier1:<11} | mb=\"{raw}\" ({year}) | {line}");
            }
            println!("\n{} of {} titles have a playable tier", served, titles.len());
        }
        // Try the Dramachi provider (added upstream in v0.1.22) on a few titles:
        // `probe dramachi "Squid Game" "Naruto"`. Reports search hit, episodes and a byte fetch.
        // The app's Dramachi tier on its own, with MovieBox-style titles:
        // `probe dtier "Parasite [Hindi]" 2019 0 0`
        "dtier" => {
            let t = args.get(2).cloned().unwrap_or_default();
            let y = args.get(3).cloned().filter(|y| !y.is_empty());
            let se: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
            let ep: usize = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
            match moviebox_lib::commands::streams::dramachi_streams(&svc, &t, y.as_deref(), se, ep).await {
                Ok(list) => for s in list { println!("{t} -> {} {}p size={:?} {}", s.source, s.height, s.size, s.url); },
                Err(e) => println!("{t} -> {e}"),
            }
        }
        "dramachi" => {
            let titles: Vec<String> = if args.len() > 2 { args[2..].to_vec() } else {
                ["Squid Game", "Crash Landing on You", "Naruto", "One Piece", "Demon Slayer",
                 "Parasite", "Doraemon", "Shin Chan", "Inception", "Jawan"].iter().map(|s| s.to_string()).collect()
            };
            let http = reqwest::Client::new();
            for t in &titles {
                let hits = match svc.search_typed(ProviderKind::Dramachi, t, 1).await {
                    Ok(v) => v,
                    Err(e) => { println!("{t:<22} search error: {e}"); continue; }
                };
                let Some(first) = hits.first() else { println!("{t:<22} no hit"); continue; };
                let id = first.id.value.clone();
                let d = match svc.details_typed(ProviderKind::Dramachi, &id).await {
                    Ok(d) => d,
                    Err(e) => { println!("{t:<22} hit={:?} details error: {e}", first.title); continue; }
                };
                let (se, ep) = d.seasons.first().and_then(|s| s.episodes.first().map(|e| (s.number, e.number))).unwrap_or((0, 0));
                let rels = ReleaseProvider::episode_streams(&svc.dramachi_client, &id, se, ep).await;
                match rels {
                    Ok(r) if !r.is_empty() => {
                        let m = &r[0].mirrors[0];
                        let mut req = http.get(&m.resolver_url).header("Range", "bytes=0-255");
                        for (k, v) in &m.headers { req = req.header(k.as_str(), v.as_str()); }
                        let st = req.send().await.map(|x| x.status().as_u16()).unwrap_or(0);
                        println!("{t:<22} hit={:?} ({:?}) seasons={} S{se}E{ep}: {} release(s) q={:?} http={st}", d.title, d.year, d.seasons.len(), r.len(), r[0].quality);
                    }
                    Ok(_) => println!("{t:<22} hit={:?} no streams", d.title),
                    Err(e) => println!("{t:<22} hit={:?} streams error: {e}", d.title),
                }
            }
        }
        // Check the addon bridge: title -> IMDb id -> streams.
        // `cargo run --bin probe -- addons "Inception" 2010`
        "addons" => {
            let title = args.get(2).cloned().unwrap_or_else(|| "Inception".into());
            let year = args.get(3).cloned();
            for a in moviebox_tui::config::load_addons() {
                println!("addon {} enabled={} stream={} catalog={}", a.name, a.enabled, a.provides_stream, a.provides_catalog);
            }
            match moviebox_lib::commands::addons::addon_streams_for(&title, year.as_deref(), false, 0, 0).await {
                Ok(list) => {
                    println!("{} stream(s)", list.len());
                    for s in list.iter().take(6) {
                        println!("   {} {} {}", s.label, s.source, s.url.chars().take(70).collect::<String>());
                    }
                }
                Err(e) => println!("addon streams: {e}"),
            }
        }
        _ => {}
    }
}
