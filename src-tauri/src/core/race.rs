//! How the app waits on several sources at once.
//!
//! Streams come from tiers in a fixed order of preference (MovieBox, then 4KHDHub, then
//! Dramachi, then the user's addons). Asking them one after another means a title only
//! the last tier carries costs the sum of every failure before it -- 13 seconds measured
//! for Parasite. These helpers ask them together but keep the same order of preference.
use futures::future::BoxFuture;
use futures::stream::{FuturesUnordered, StreamExt};
use std::future::Future;
use std::time::Duration;

/// Run every tier at once and return the best one that succeeds.
///
/// "Best" means the earliest tier in `tiers`. A tier that succeeds is returned the moment
/// every tier above it has failed, so nothing waits for a tier that cannot win. If a lower
/// tier has succeeded but a higher one is still working, the higher one gets `grace` longer
/// to finish; after that the best result in hand is returned and the rest are dropped. That
/// keeps a 4K release from being thrown away for a 360p one that arrived two seconds sooner,
/// without letting one slow tier hold everything up.
///
/// When every tier fails, their errors come back in tier order.
pub async fn pick_best<'a, T, E>(tiers: Vec<BoxFuture<'a, Result<T, E>>>, grace: Duration) -> Result<T, Vec<E>> {
    let n = tiers.len();
    let mut pending: FuturesUnordered<_> = tiers
        .into_iter()
        .enumerate()
        .map(|(i, f)| async move { (i, f.await) })
        .collect();
    let mut done: Vec<Option<Result<T, E>>> = (0..n).map(|_| None).collect();
    let mut deadline: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;

    loop {
        // The earliest tier that succeeded, and whether anything above it is undecided.
        let best_ok = done.iter().position(|d| matches!(d, Some(Ok(_))));
        if let Some(i) = best_ok {
            if done[..i].iter().all(|d| d.is_some()) {
                return match done[i].take() {
                    Some(Ok(v)) => Ok(v),
                    _ => unreachable!("position() found an Ok here"),
                };
            }
            if deadline.is_none() {
                deadline = Some(Box::pin(tokio::time::sleep(grace)));
            }
        }
        if pending.is_empty() {
            // Nothing left to wait for and nothing succeeded.
            return Err(done.into_iter().filter_map(|d| d.and_then(|r| r.err())).collect());
        }
        tokio::select! {
            Some((i, r)) = pending.next() => done[i] = Some(r),
            _ = async { deadline.as_mut().unwrap().await }, if deadline.is_some() => {
                let i = done.iter().position(|d| matches!(d, Some(Ok(_)))).expect("the deadline only starts once a tier succeeded");
                return match done[i].take() {
                    Some(Ok(v)) => Ok(v),
                    _ => unreachable!(),
                };
            }
        }
    }
}

/// Ask `backups` for help early, without waiting for `primary` to fail first.
///
/// `primary` runs alone unless it takes longer than `after` (or fails), at which point
/// `backups` starts alongside it. If `primary` succeeds, that is the answer and `backups`
/// is dropped, so a healthy source never causes a request to the others. If it fails, the
/// backups' answer stands in, and it is already partly done.
pub async fn hedged<T, P, B, F1, F2>(primary: F1, backups: F2, after: Duration) -> Result<T, (P, B)>
where
    F1: Future<Output = Result<T, P>>,
    F2: Future<Output = Result<T, B>>,
{
    tokio::pin!(primary);
    tokio::pin!(backups);
    let delay = tokio::time::sleep(after);
    tokio::pin!(delay);
    let mut started = false;
    let mut early = None;
    let failed = loop {
        tokio::select! {
            r = &mut primary => match r {
                Ok(v) => return Ok(v),
                Err(p) => break p,
            },
            _ = &mut delay, if !started => started = true,
            r = &mut backups, if started && early.is_none() => early = Some(r),
        }
    };
    let b = match early {
        Some(r) => r,
        None => backups.await,
    };
    b.map_err(|e| (failed, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn tier(after: u64, out: Result<&'static str, &'static str>) -> BoxFuture<'static, Result<&'static str, &'static str>> {
        Box::pin(async move {
            tokio::time::sleep(ms(after)).await;
            out
        })
    }

    #[tokio::test]
    async fn the_best_tier_wins_even_when_it_is_slowest_within_the_grace() {
        let t = Instant::now();
        // The lowest tier answers first; the top tier follows well inside the grace period.
        let got = pick_best(vec![tier(80, Ok("4k")), tier(500, Err("none")), tier(10, Ok("360p"))], ms(300)).await;
        assert_eq!(got, Ok("4k"));
        assert!(t.elapsed() < ms(280), "should not have waited out the grace period");
    }

    #[tokio::test]
    async fn a_lower_tier_is_returned_at_once_when_everything_above_it_failed() {
        let t = Instant::now();
        let got = pick_best(vec![tier(20, Err("a")), tier(40, Err("b")), tier(60, Ok("c")), tier(5000, Ok("slow"))], ms(5000)).await;
        assert_eq!(got, Ok("c"));
        assert!(t.elapsed() < ms(400), "must not wait for the tier below the winner");
    }

    #[tokio::test]
    async fn a_slow_top_tier_is_dropped_after_the_grace_period() {
        let t = Instant::now();
        let got = pick_best(vec![tier(3000, Ok("4k")), tier(20, Ok("360p"))], ms(150)).await;
        assert_eq!(got, Ok("360p"));
        let took = t.elapsed();
        assert!(took >= ms(140) && took < ms(1000), "took {took:?}");
    }

    #[tokio::test]
    async fn all_tiers_failing_reports_every_error_in_order() {
        let got: Result<&str, Vec<&str>> = pick_best(vec![tier(30, Err("one")), tier(5, Err("two")), tier(15, Err("three"))], ms(100)).await;
        assert_eq!(got, Err(vec!["one", "two", "three"]));
    }

    #[tokio::test]
    async fn no_tiers_is_an_empty_failure() {
        let got: Result<&str, Vec<&str>> = pick_best(Vec::new(), ms(10)).await;
        assert_eq!(got, Err(vec![]));
    }

    #[tokio::test]
    async fn a_healthy_primary_never_wakes_the_backups() {
        let woke = Arc::new(AtomicBool::new(false));
        let w = woke.clone();
        let got: Result<&str, (&str, &str)> = hedged(
            async { tokio::time::sleep(ms(20)).await; Ok("moviebox") },
            async move { w.store(true, Ordering::SeqCst); Ok("backup") },
            ms(200),
        )
        .await;
        assert_eq!(got, Ok("moviebox"));
        assert!(!woke.load(Ordering::SeqCst), "backups must not run when the primary is quick");
    }

    #[tokio::test]
    async fn a_slow_primary_that_fails_finds_the_backups_already_running() {
        let t = Instant::now();
        let got: Result<&str, (&str, &str)> = hedged(
            async { tokio::time::sleep(ms(400)).await; Err("moviebox down") },
            async { tokio::time::sleep(ms(300)).await; Ok("backup") },
            ms(100),
        )
        .await;
        assert_eq!(got, Ok("backup"));
        // Sequential would be 400 + 300 = 700 ms; hedged is the primary's 400 ms.
        let took = t.elapsed();
        assert!(took >= ms(390) && took < ms(600), "took {took:?}");
    }

    #[tokio::test]
    async fn a_primary_that_fails_fast_starts_the_backups_immediately() {
        let got: Result<&str, (&str, &str)> = hedged(async { Err("nope") }, async { Ok("backup") }, ms(5000)).await;
        assert_eq!(got, Ok("backup"));
    }

    #[tokio::test]
    async fn a_slow_primary_still_beats_backups_that_finished_first() {
        let got: Result<&str, (&str, &str)> = hedged(
            async { tokio::time::sleep(ms(300)).await; Ok("moviebox") },
            async { tokio::time::sleep(ms(50)).await; Ok("backup") },
            ms(100),
        )
        .await;
        assert_eq!(got, Ok("moviebox"));
    }

    #[tokio::test]
    async fn both_failing_returns_both_errors() {
        let got: Result<&str, (&str, &str)> = hedged(async { Err("p") }, async { Err("b") }, ms(50)).await;
        assert_eq!(got, Err(("p", "b")));
    }
}
