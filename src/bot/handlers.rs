use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use teloxide::prelude::*;
use teloxide::types::ReplyParameters;
use std::sync::Mutex;

use crate::bot::caption::build_caption;
use crate::bot::compressor::compress_video;
use crate::bot::notifier::ChatNotifier;
use crate::bot::progress;
use crate::config::AppConfig;
use crate::error::BotError;
use crate::security::download_guard::DownloadSemaphore;
use crate::security::inflight_tracker::{InflightGuard, InflightTracker};
use crate::security::rate_limiter::UserRateLimiter;
use crate::security::retry::with_retry;
use crate::security::url_validator::validate_download_url;
use crate::tiktok::detector::extract_tiktok_urls;
use crate::tiktok::downloader::{download_to_file, fetch_video_info};

/// Minimum interval between progress message edits to avoid Telegram rate limits.
const PROGRESS_UPDATE_INTERVAL_MS: u64 = 2000;

/// Messages older than this (in seconds) are ignored on bot restart.
const MAX_MESSAGE_AGE_SECS: i64 = 120;

/// Timeout waiting for a download slot before returning an error.
const DOWNLOAD_SEMAPHORE_TIMEOUT_SECS: u64 = 30;

pub async fn handle_message(
    bot: Bot,
    msg: Message,
    config: AppConfig,
    client: Client,
    rate_limiter: Arc<UserRateLimiter>,
    download_semaphore: DownloadSemaphore,
    inflight_tracker: InflightTracker,
) -> Result<(), BotError> {
    let text = match msg.text() {
        Some(t) => t,
        None => return Ok(()),
    };

    // Skip stale messages (e.g. queued while bot was offline)
    let now = chrono::Utc::now().timestamp();
    let msg_timestamp = msg.date.timestamp();
    if (now - msg_timestamp) > MAX_MESSAGE_AGE_SECS {
        tracing::debug!(age_secs = now - msg_timestamp, "Skipping stale message");
        return Ok(());
    }

    let tiktok_urls = extract_tiktok_urls(text);
    if tiktok_urls.is_empty() {
        return Ok(());
    }

    // Authorization check
    let user_id = match msg.from.as_ref() {
        Some(user) => user.id,
        None => return Ok(()),
    };

    if !config.is_user_authorized(user_id.0) || !config.is_chat_authorized(msg.chat.id.0) {
        tracing::warn!(
            user_id = user_id.0,
            chat_id = msg.chat.id.0,
            "Unauthorized access attempt"
        );
        return Ok(()); // Silent rejection — don't leak bot existence
    }

    // Per-user rate limiting
    if rate_limiter.check_key(&user_id).is_err() {
        tracing::warn!(user_id = user_id.0, "Rate limited");
        bot.send_message(msg.chat.id, BotError::RateLimited.user_friendly_message())
            .reply_parameters(ReplyParameters::new(msg.id))
            .await?;
        return Ok(());
    }

    let notifier = ChatNotifier::new(&bot, msg.chat.id, msg.id);

    for tiktok_url in &tiktok_urls {
        // Skip URLs already being processed (e.g. same link sent multiple times)
        let Some(_guard) = InflightGuard::try_acquire(&inflight_tracker, tiktok_url) else {
            tracing::info!(url = %tiktok_url, "Skipping duplicate URL already in progress");
            continue;
        };

        match process_tiktok_url(&notifier, &config, &client, tiktok_url, &download_semaphore)
            .await
        {
            Ok(()) => {}
            Err(e) => {
                tracing::error!(url = %tiktok_url, error = %e, "Failed to process TikTok URL");
                let _ = notifier.send_error(&e.user_friendly_message()).await;
            }
        }
        // _guard is dropped here, removing the URL from the tracker
    }

    Ok(())
}

async fn process_tiktok_url(
    notifier: &ChatNotifier<'_>,
    config: &AppConfig,
    client: &Client,
    tiktok_url: &str,
    download_semaphore: &DownloadSemaphore,
) -> Result<(), BotError> {
    // Step 1: Fetch video info from API (with retry for transient failures)
    tracing::info!(url = %tiktok_url, "Fetching video info");
    let api_url = config.tikwm_api_url.clone();
    let tiktok_url_owned = tiktok_url.to_string();
    let info = with_retry("fetch_video_info", || {
        fetch_video_info(client, &api_url, &tiktok_url_owned)
    })
    .await?;

    // Step 2: Send progress message
    let initial_text = progress::build_initial_message(&info);
    let progress_msg_id = notifier.send_progress_message(&initial_text).await?;

    // Step 3: Validate video URL against SSRF before downloading
    validate_download_url(&info.video_url)?;

    // Step 4: Acquire download permit (limits concurrent downloads globally)
    let permit = tokio::time::timeout(
        Duration::from_secs(DOWNLOAD_SEMAPHORE_TIMEOUT_SECS),
        download_semaphore.acquire(),
    )
    .await
    .map_err(|_| BotError::TooManyDownloads)?
    .map_err(|_| BotError::TooManyDownloads)?;

    // Step 5: Show "uploading video" indicator + download with progress
    notifier.show_upload_action().await;
    tracing::info!("Downloading video via streaming...");

    let notifier_bot = notifier.bot().clone();
    let chat_id = notifier.chat_id();
    let last_update = Arc::new(Mutex::new(tokio::time::Instant::now()));
    let info_arc = Arc::new(info);

    let info_ref = Arc::clone(&info_arc);
    let last_update_ref = Arc::clone(&last_update);

    let video_url = info_arc.video_url.clone();
    let downloaded = with_retry("download_video", || {
        let client = client.clone();
        let video_url = video_url.clone();
        let bot_inner = notifier_bot.clone();
        let info_inner = Arc::clone(&info_ref);
        let last_update_inner = Arc::clone(&last_update_ref);

        async move {
            download_to_file(&client, &video_url, move |prog| {
                // Check throttle interval BEFORE spawning to avoid unnecessary task overhead.
                // std::sync::Mutex + try_lock: non-blocking, never holds across .await.
                let Ok(mut last) = last_update_inner.try_lock() else {
                    return; // Lock contended — a previous update is being sent
                };
                if last.elapsed().as_millis() < PROGRESS_UPDATE_INTERVAL_MS as u128 {
                    return;
                }
                *last = tokio::time::Instant::now();
                drop(last);

                let bot_c = bot_inner.clone();
                let info_c = Arc::clone(&info_inner);
                tokio::spawn(async move {
                    let text = progress::build_download_text(&info_c, &prog);
                    let _ = bot_c
                        .edit_message_text(chat_id, progress_msg_id, text)
                        .await;
                });
            })
            .await
        }
    })
    .await?;

    // Release download permit
    drop(permit);

    let original_size = downloaded.actual_size;
    tracing::info!(
        size_mb = format!("{:.1}", original_size as f64 / (1024.0 * 1024.0)).as_str(),
        "Video downloaded"
    );

    // Step 6: Compress if the video exceeds Telegram's 50 MB limit
    const TELEGRAM_UPLOAD_LIMIT: u64 = 50 * 1024 * 1024;

    let (send_path, send_size) = if original_size > TELEGRAM_UPLOAD_LIMIT {
        let duration = info_arc.metadata.duration_secs.ok_or_else(|| {
            BotError::FileTooLarge {
                size_mb: original_size as f64 / (1024.0 * 1024.0),
            }
        })?;

        notifier
            .update_progress(
                progress_msg_id,
                "\u{1f3ac} Compressing video to fit Telegram limit...",
            )
            .await;

        let compressed = compress_video(&downloaded.file_path, duration).await?;
        let size = compressed.actual_size;
        tracing::info!(
            original_mb = format!("{:.1}", original_size as f64 / (1024.0 * 1024.0)).as_str(),
            compressed_mb = format!("{:.1}", size as f64 / (1024.0 * 1024.0)).as_str(),
            "Video compressed"
        );
        (compressed.file_path.clone(), size)
    } else {
        (downloaded.file_path.clone(), original_size)
    };

    // Step 7: Update progress message
    notifier
        .update_progress(progress_msg_id, "\u{1f4e4} Sending video to chat...")
        .await;

    // Step 8: Send video with rich caption and metadata
    let caption = build_caption(&info_arc.metadata, send_size);
    notifier
        .send_video(
            &send_path,
            &caption,
            info_arc.metadata.duration_secs,
        )
        .await?;

    // Step 8: Clean up progress message
    notifier.delete_message(progress_msg_id).await;

    tracing::info!("\u{2705} Video sent successfully");

    Ok(())
}
