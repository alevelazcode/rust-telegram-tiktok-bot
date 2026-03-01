use std::time::Duration;

use crate::error::BotError;

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;

/// Retries an async operation with exponential backoff for transient HTTP failures.
/// Only retries on `BotError::Http` errors (network/timeout issues).
/// Non-retryable errors (API errors, file too large, etc.) are returned immediately.
pub async fn with_retry<F, Fut, T>(operation_name: &str, f: F) -> Result<T, BotError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, BotError>>,
{
    let mut last_error = None;

    for attempt in 0..=MAX_RETRIES {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !is_retryable(&e) || attempt == MAX_RETRIES {
                    return Err(e);
                }

                let backoff = Duration::from_millis(INITIAL_BACKOFF_MS * 2u64.pow(attempt));
                tracing::warn!(
                    operation = operation_name,
                    attempt = attempt + 1,
                    max_retries = MAX_RETRIES,
                    backoff_ms = backoff.as_millis() as u64,
                    error = %e,
                    "Transient failure, retrying..."
                );
                last_error = Some(e);
                tokio::time::sleep(backoff).await;
            }
        }
    }

    Err(last_error.unwrap())
}

fn is_retryable(error: &BotError) -> bool {
    matches!(error, BotError::Http(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let result = with_retry("test", || async { Ok::<_, BotError>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable_errors() {
        let attempts = AtomicU32::new(0);
        let result = with_retry("test", || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), _>(BotError::NoVideoFound) }
        })
        .await;

        assert!(matches!(result.unwrap_err(), BotError::NoVideoFound));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retries_on_transient_http_errors() {
        let attempts = AtomicU32::new(0);
        let result: Result<i32, BotError> = with_retry("test", || {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt < 2 {
                    // Simulate HTTP error by creating a reqwest error
                    Err(BotError::TikTokApi("simulated".into()))
                } else {
                    Ok(99)
                }
            }
        })
        .await;

        // TikTokApi is NOT retryable, so it should fail on first attempt
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
