use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Maximum number of videos that can wait in queue per user.
/// With 1 actively processing, a user can have at most 5 total (1 active + 4 waiting).
const MAX_QUEUED_PER_USER: usize = 4;

pub(crate) struct UserEntry {
    /// Single-permit semaphore: only one video processes at a time per user.
    semaphore: Arc<Semaphore>,
    /// How many tasks are currently waiting for the semaphore.
    waiting: AtomicUsize,
}

/// Per-user processing queue. Ensures each user's videos are processed
/// sequentially (one at a time) and limits how many can wait in line.
pub type UserQueue = Arc<DashMap<u64, Arc<UserEntry>>>;

pub fn create_user_queue() -> UserQueue {
    Arc::new(DashMap::new())
}

/// RAII guard returned when a user's video gets its turn to process.
/// Releasing this guard (via drop) frees the slot for the next queued video.
pub struct UserSlotGuard {
    _permit: OwnedSemaphorePermit,
}

/// Waits for the user's processing slot. Returns a guard that releases the slot on drop.
///
/// - If the user has fewer than `MAX_QUEUED_PER_USER` waiting, this will block
///   until it's this video's turn.
/// - If the queue is full, returns `None` immediately.
pub async fn acquire_user_slot(queue: &UserQueue, user_id: u64) -> Option<UserSlotGuard> {
    let entry = queue
        .entry(user_id)
        .or_insert_with(|| {
            Arc::new(UserEntry {
                semaphore: Arc::new(Semaphore::new(1)),
                waiting: AtomicUsize::new(0),
            })
        })
        .clone();

    // Atomically increment the waiting counter; reject if queue is full.
    let prev_waiting = entry.waiting.fetch_add(1, Ordering::SeqCst);
    if prev_waiting >= MAX_QUEUED_PER_USER {
        entry.waiting.fetch_sub(1, Ordering::SeqCst);
        return None;
    }

    // Wait for our turn (blocks until the current video finishes).
    let permit = entry
        .semaphore
        .clone()
        .acquire_owned()
        .await
        .ok()?;

    // We got the slot — no longer waiting.
    entry.waiting.fetch_sub(1, Ordering::SeqCst);

    Some(UserSlotGuard { _permit: permit })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn single_user_gets_slot() {
        let queue = create_user_queue();
        let guard = acquire_user_slot(&queue, 1).await;
        assert!(guard.is_some());
    }

    #[tokio::test]
    async fn second_request_waits_then_proceeds() {
        let queue = create_user_queue();
        let guard1 = acquire_user_slot(&queue, 1).await.unwrap();

        let queue_clone = queue.clone();
        let handle = tokio::spawn(async move {
            acquire_user_slot(&queue_clone, 1).await
        });

        // Give the spawned task time to start waiting
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Release first slot
        drop(guard1);

        let guard2 = handle.await.unwrap();
        assert!(guard2.is_some());
    }

    #[tokio::test]
    async fn different_users_are_independent() {
        let queue = create_user_queue();
        let _guard1 = acquire_user_slot(&queue, 1).await;
        let guard2 = acquire_user_slot(&queue, 2).await;
        assert!(guard2.is_some());
    }

    #[tokio::test]
    async fn rejects_when_queue_full() {
        let queue = create_user_queue();

        // Hold the processing slot
        let _active = acquire_user_slot(&queue, 1).await.unwrap();

        // Fill the queue (4 waiting)
        let mut handles = Vec::new();
        for _ in 0..MAX_QUEUED_PER_USER {
            let q = queue.clone();
            handles.push(tokio::spawn(async move {
                acquire_user_slot(&q, 1).await
            }));
        }

        // Give tasks time to start waiting
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // The 5th waiting request should be rejected
        let rejected = acquire_user_slot(&queue, 1).await;
        assert!(rejected.is_none());

        // Clean up: drop the active guard so waiting tasks can proceed
        drop(_active);
        for h in handles {
            let _ = h.await;
        }
    }
}
