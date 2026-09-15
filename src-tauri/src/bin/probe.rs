// Developer probe: prints the shape of live API payloads. Not part of the app UI.
use moviebox_tui::providers::{ProviderKind, ReleaseProvider};
use moviebox_tui::service::MovieBoxService;

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
        _ => {}
    }
}
