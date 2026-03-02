use std::sync::Arc;

use dashmap::DashSet;

/// Tracks URLs currently being processed to prevent duplicate downloads.
/// When the same TikTok link is sent multiple times while it's still being
/// downloaded, subsequent requests are silently skipped.
///
/// Uses `DashSet` for lock-free concurrent access from multiple handler tasks.
pub type InflightTracker = Arc<DashSet<String>>;

pub fn create_inflight_tracker() -> InflightTracker {
    Arc::new(DashSet::new())
}

/// RAII guard that removes the URL from the tracker when dropped.
/// Ensures cleanup happens on both success and error paths.
pub struct InflightGuard {
    tracker: InflightTracker,
    url: String,
}

impl InflightGuard {
    /// Tries to mark a URL as in-flight. Returns `Some(guard)` if the URL
    /// was not already being processed, `None` if it's a duplicate.
    pub fn try_acquire(tracker: &InflightTracker, url: &str) -> Option<Self> {
        if tracker.insert(url.to_string()) {
            Some(Self {
                tracker: Arc::clone(tracker),
                url: url.to_string(),
            })
        } else {
            None
        }
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.tracker.remove(&self.url);
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
    fn allows_after_guard_dropped() {
        let tracker = create_inflight_tracker();
        {
            let _guard = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/123");
            assert!(tracker.contains("https://tiktok.com/v/123"));
        }
        // Guard dropped — URL should be removed
        assert!(!tracker.contains("https://tiktok.com/v/123"));
        let guard = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/123");
        assert!(guard.is_some());
    }

    #[test]
    fn different_urls_are_independent() {
        let tracker = create_inflight_tracker();
        let _g1 = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/111");
        let g2 = InflightGuard::try_acquire(&tracker, "https://tiktok.com/v/222");
        assert!(g2.is_some());
    }
}
