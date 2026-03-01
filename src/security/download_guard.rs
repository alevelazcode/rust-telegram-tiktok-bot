use std::sync::Arc;
use tokio::sync::Semaphore;

/// Global limit for concurrent video downloads.
/// Prevents resource exhaustion (memory, disk I/O, network).
const MAX_CONCURRENT_DOWNLOADS: usize = 5;

pub type DownloadSemaphore = Arc<Semaphore>;

pub fn create_download_semaphore() -> DownloadSemaphore {
    Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS))
}
