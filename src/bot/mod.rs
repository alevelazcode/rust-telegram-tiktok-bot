pub mod caption;
pub mod compressor;
pub mod handlers;
pub mod notifier;
pub mod progress;

use std::time::Duration;

use reqwest::Client;
use teloxide::prelude::*;

use crate::config::AppConfig;
use crate::error::BotError;
use crate::security::download_guard::create_download_semaphore;
use crate::security::inflight_tracker::create_inflight_tracker;
use crate::security::rate_limiter::create_rate_limiter;
use crate::security::temp_cleaner::spawn_temp_cleaner;
use crate::security::user_queue::create_user_queue;
use handlers::handle_message;

fn create_http_client() -> Result<Client, BotError> {
    let client = Client::builder()
        .pool_max_idle_per_host(8)
        .pool_idle_timeout(Duration::from_secs(60))
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(10))
        .tcp_keepalive(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("TikTokBot/1.0")
        .build()?;

    Ok(client)
}

pub async fn run(config: AppConfig) {
    // Build the bot with a generous upload timeout (default teloxide timeout is only 17s,
    // which is too short for uploading 50 MB videos).
    let bot_client = teloxide_core::net::default_reqwest_settings()
        .timeout(Duration::from_secs(300)) // 5 min for large video uploads
        .connect_timeout(Duration::from_secs(10))
        .tcp_keepalive(Duration::from_secs(60))
        .build()
        .expect("Failed to create bot HTTP client");
    let bot = Bot::with_client(&config.teloxide_token, bot_client);
    let client = create_http_client().expect("Failed to create HTTP client");
    let rate_limiter = create_rate_limiter();
    let download_semaphore = create_download_semaphore();
    let inflight_tracker = create_inflight_tracker();
    let user_queue = create_user_queue();

    // Start background temp file cleaner
    spawn_temp_cleaner();

    let handler = Update::filter_message().endpoint(handle_message);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![config, client, rate_limiter, download_semaphore, inflight_tracker, user_queue])
        .default_handler(|upd| async move {
            tracing::debug!("Unhandled update: {:?}", upd);
        })
        .error_handler(LoggingErrorHandler::with_custom_text(
            "Error in the dispatcher",
        ))
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
