use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// How long a URL stays blocked after processing finishes.
/// Prevents re-processing when the same link is sent again shortly after.
const COOLDOWN: Duration = Duration::from_secs(60);

/// State of a tracked URL.
pub(crate) enum UrlState {
    /// Currently being downloaded/processed.
    Processing,
    /// Finished processing at the given instant; blocked until cooldown expires.
    Completed(Instant),
}

/// Tracks URLs currently being processed **and** recently completed to prevent
/// duplicate downloads. Uses `DashMap` for lock-free concurrent access.
pub type InflightTracker = Arc<DashMap<String, UrlState>>;

pub fn create_inflight_tracker() -> InflightTracker {
    Arc::new(DashMap::new())
}

/// RAII guard that transitions the URL from `Processing` to `Completed` on drop.
/// Ensures cooldown starts on both success and error paths.
pub struct InflightGuard {
    tracker: InflightTracker,
    url: String,
}

impl InflightGuard {
    /// Tries to mark a URL as in-flight. Returns `Some(guard)` if the URL is
    /// not currently processing and is not in its post-completion cooldown.
    pub fn try_acquire(tracker: &InflightTracker, url: &str) -> Option<Self> {
        use dashmap::mapref::entry::Entry;

        match tracker.entry(url.to_string()) {
            Entry::Occupied(mut entry) => match entry.get() {
                UrlState::Processing => return None,
                UrlState::Completed(t) => {
                    if t.elapsed() < COOLDOWN {
                        return None;
                    }
                    // Cooldown expired — re-acquire
                    entry.insert(UrlState::Processing);
                }
            },
            Entry::Vacant(entry) => {
                entry.insert(UrlState::Processing);
            }
        }

        Some(Self {
            tracker: Arc::clone(tracker),
            url: url.to_string(),
        })
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        // Transition to cooldown state rather than removing.
        self.tracker
            .insert(self.url.clone(), UrlState::Completed(Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_first_request() {
        let tracker = create_inflight_tracker();
        let guard = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/123");
        assert!(guard.is_some());
    }

    #[test]
    fn rejects_duplicate_url() {
        let tracker = create_inflight_tracker();
        let _guard = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/123");
        let duplicate = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/123");
        assert!(duplicate.is_none());
    }

    #[test]
    fn rejects_during_cooldown() {
        let tracker = create_inflight_tracker();
        {
            let _guard = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/123");
            // guard dropped here — URL enters cooldown
        }
        // Immediately after: still within cooldown
        let retry = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/123");
        assert!(retry.is_none(), "should reject during cooldown period");
    }

    #[test]
    fn allows_after_cooldown_expires() {
        let tracker = create_inflight_tracker();

        // Manually insert a completed entry with an expired timestamp
        tracker.insert(
            "https://tiktok.com/v/123".to_string(),
            UrlState::Completed(Instant::now() - COOLDOWN - Duration::from_secs(1)),
        );

        let guard = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/123");
        assert!(guard.is_some(), "should allow after cooldown expires");
    }

    #[test]
    fn different_urls_are_independent() {
        let tracker = create_inflight_tracker();
        let _g1 = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/111");
        let g2 = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/222");
        assert!(g2.is_some());
    }
}
