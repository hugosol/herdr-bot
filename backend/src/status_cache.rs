//! Completion-based, single-flight local Git status cache, keyed by actual worktree root.
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::git::{GitError, GitStatus};
use crate::zones;

type StatusResult = Result<GitStatus, GitError>;
type SharedResult = Result<Arc<GitStatus>, GitError>;
#[derive(Default)]
struct Entry {
    generation: u64,
    value: Option<(Instant, Arc<GitStatus>)>,
    flight: Option<Arc<Flight>>,
}
#[derive(Default)]
struct Flight {
    result: Mutex<Option<SharedResult>>,
    completed: Condvar,
}
pub struct StatusCache {
    ttl: Duration,
    entries: Mutex<HashMap<String, Entry>>,
}

pub(crate) fn key(path: &str) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| Path::new(path).to_path_buf());
    zones::path_key(&zones::display_path(&canonical))
}

impl StatusCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn invalidate(&self, path: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        let entry = entries.entry(key(path)).or_default();
        entry.generation = entry.generation.wrapping_add(1);
        entry.value = None;
        // Existing callers wait for their flight, reject its generation, then join the replacement.
        entry.flight = None;
    }

    pub fn read(
        &self,
        path: &str,
        force: bool,
        mut load: impl FnMut() -> StatusResult,
    ) -> StatusResult {
        let key = key(path);
        let mut force = force;
        loop {
            let (flight, generation, owner) = {
                let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
                let entry = entries.entry(key.clone()).or_default();
                if let Some(flight) = &entry.flight {
                    (Arc::clone(flight), entry.generation, false)
                } else if !force
                    && entry
                        .value
                        .as_ref()
                        .is_some_and(|(at, _)| at.elapsed() < self.ttl)
                {
                    let mut status = (*entry.value.as_ref().unwrap().1).clone();
                    path.clone_into(&mut status.zone);
                    return Ok(status);
                } else {
                    let flight = Arc::new(Flight::default());
                    entry.flight = Some(Arc::clone(&flight));
                    (flight, entry.generation, true)
                }
            };
            let result = if owner {
                let result = load().map(Arc::new);
                {
                    let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
                    let entry = entries.get_mut(&key).unwrap();
                    if entry.generation == generation {
                        entry.value = result
                            .as_ref()
                            .ok()
                            .map(|status| (Instant::now(), Arc::clone(status)));
                        entry.flight = None;
                    }
                    *flight.result.lock().unwrap_or_else(|p| p.into_inner()) = Some(result.clone());
                }
                flight.completed.notify_all();
                result
            } else {
                let mut result = flight.result.lock().unwrap_or_else(|p| p.into_inner());
                while result.is_none() {
                    result = flight
                        .completed
                        .wait(result)
                        .unwrap_or_else(|p| p.into_inner());
                }
                result.as_ref().unwrap().clone()
            };
            let entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
            if entries.get(&key).unwrap().generation != generation {
                force = false;
                continue;
            }
            return result.map(|status| {
                let mut status = (*status).clone();
                path.clone_into(&mut status.zone);
                status
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;

    fn status(branch: &str) -> GitStatus {
        GitStatus {
            zone: "loader registration".to_string(),
            repository: true,
            branch: Some(branch.to_string()),
            detached: false,
            changes: Vec::new(),
            conflict: false,
            pending_count: 0,
            review_token: Some(branch.to_string()),
            blocked_reason: None,
            sync: crate::sync::SyncFacts::default(),
        }
    }

    #[test]
    fn completion_ttl_reads_do_not_renew_and_force_bypasses_fresh_value() {
        let cache = StatusCache::new(Duration::from_secs(10));
        let calls = AtomicUsize::new(0);
        let load = || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(status("main"))
        };
        assert_eq!(
            cache.read("cache-zone", false, load).unwrap().zone,
            "cache-zone"
        );
        let completed = cache.entries.lock().unwrap()[&key("cache-zone")]
            .value
            .as_ref()
            .unwrap()
            .0;
        cache.read("cache-zone", false, load).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            cache.entries.lock().unwrap()[&key("cache-zone")]
                .value
                .as_ref()
                .unwrap()
                .0,
            completed
        );
        cache.read("cache-zone", true, load).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        cache
            .entries
            .lock()
            .unwrap()
            .get_mut(&key("cache-zone"))
            .unwrap()
            .value
            .as_mut()
            .unwrap()
            .0 = Instant::now() - Duration::from_secs(11);
        cache.read("cache-zone", false, load).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn errors_are_not_cached_as_fresh_success() {
        let cache = StatusCache::new(Duration::from_secs(10));
        assert!(
            cache
                .read("error-zone", false, || Err(GitError::Failed(
                    "unavailable".into()
                )))
                .is_err()
        );
        assert_eq!(
            cache
                .read("error-zone", false, || Ok(status("recovered")))
                .unwrap()
                .branch
                .as_deref(),
            Some("recovered")
        );
    }

    #[test]
    fn invalidation_rejects_inflight_results_and_uses_replacement() {
        let cache = Arc::new(StatusCache::new(Duration::from_secs(10)));
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let worker_cache = Arc::clone(&cache);
        let worker = std::thread::spawn(move || {
            worker_cache
                .read("race-zone", false, || {
                    started_tx.send(()).unwrap();
                    finish_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(status("stale"))
                })
                .unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        cache.invalidate("race-zone");
        assert_eq!(
            cache
                .read("race-zone", false, || Ok(status("fresh")))
                .unwrap()
                .branch
                .as_deref(),
            Some("fresh")
        );
        finish_tx.send(()).unwrap();
        assert_eq!(worker.join().unwrap().branch.as_deref(), Some("fresh"));
    }

    #[test]
    fn concurrent_failure_is_shared_but_the_next_read_retries() {
        let cache = Arc::new(StatusCache::new(Duration::from_secs(10)));
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let owner_cache = Arc::clone(&cache);
        let owner = std::thread::spawn(move || {
            owner_cache.read("failure-flight", false, || {
                started_tx.send(()).unwrap();
                finish_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                Err(GitError::Failed("shared failure".into()))
            })
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let follower_cache = Arc::clone(&cache);
        let follower = std::thread::spawn(move || {
            follower_cache.read("failure-flight", true, || {
                panic!("failure must also be shared")
            })
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "follower did not join");
            let joined = {
                let entries = cache.entries.lock().unwrap();
                Arc::strong_count(entries[&key("failure-flight")].flight.as_ref().unwrap()) >= 3
            };
            if joined {
                break;
            }
            std::thread::yield_now();
        }
        finish_tx.send(()).unwrap();
        assert_eq!(
            owner.join().unwrap().unwrap_err().message(),
            "shared failure"
        );
        assert_eq!(
            follower.join().unwrap().unwrap_err().message(),
            "shared failure"
        );
        assert_eq!(
            cache
                .read("failure-flight", false, || Ok(status("retry")))
                .unwrap()
                .branch
                .as_deref(),
            Some("retry")
        );
    }

    #[test]
    fn forced_and_ordinary_overlapping_reads_share_a_single_flight() {
        let cache = Arc::new(StatusCache::new(Duration::from_secs(10)));
        let (started_tx, started_rx) = mpsc::channel();
        let (finish_tx, finish_rx) = mpsc::channel();
        let owner_cache = Arc::clone(&cache);
        let owner = std::thread::spawn(move || {
            owner_cache
                .read("flight-zone", true, || {
                    started_tx.send(()).unwrap();
                    finish_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(status("shared"))
                })
                .unwrap()
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let follower_cache = Arc::clone(&cache);
        let follower = std::thread::spawn(move || {
            follower_cache
                .read("flight-zone", true, || {
                    panic!("overlapping force must share the flight")
                })
                .unwrap()
        });
        let ordinary_cache = Arc::clone(&cache);
        let ordinary = std::thread::spawn(move || {
            ordinary_cache
                .read("flight-zone", false, || {
                    panic!("ordinary reads must share the flight")
                })
                .unwrap()
        });
        // The flight itself is reference counted; wait for both followers, not for a timing guess.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(
                Instant::now() < deadline,
                "followers did not join the existing flight"
            );
            let joined = {
                let entries = cache.entries.lock().unwrap();
                Arc::strong_count(entries[&key("flight-zone")].flight.as_ref().unwrap()) >= 4
            };
            if joined {
                break;
            }
            std::thread::yield_now();
        }
        finish_tx.send(()).unwrap();
        assert_eq!(owner.join().unwrap().branch.as_deref(), Some("shared"));
        assert_eq!(follower.join().unwrap().branch.as_deref(), Some("shared"));
        assert_eq!(ordinary.join().unwrap().branch.as_deref(), Some("shared"));
    }
}
