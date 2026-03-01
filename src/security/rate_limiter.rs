use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;
use std::sync::Arc;
use teloxide::types::UserId;

/// Per-user rate limiter: 5 requests per 60 seconds.
pub type UserRateLimiter = RateLimiter<
    UserId,
    dashmap::DashMap<UserId, governor::state::InMemoryState>,
    DefaultClock,
    NoOpMiddleware,
>;

pub fn create_rate_limiter() -> Arc<UserRateLimiter> {
    let quota = Quota::per_minute(nonzero!(5u32));
    Arc::new(RateLimiter::keyed(quota))
}
