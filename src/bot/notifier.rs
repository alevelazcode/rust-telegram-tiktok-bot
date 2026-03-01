use std::time::Duration;

use reqwest::Client;
use teloxide::prelude::*;
use teloxide::types::{ChatAction, InputFile, MessageId, ReplyParameters};

use crate::error::BotError;
use crate::security::url_validator::validate_download_url;
use crate::tiktok::downloader::VideoInfo;

/// Max time to wait for a thumbnail download before skipping it.
const THUMBNAIL_TIMEOUT_SECS: u64 = 5;

/// Maximum thumbnail size (5 MB). Prevents memory exhaustion from oversized images.
const MAX_THUMBNAIL_BYTES: usize = 5 * 1024 * 1024;

/// Responsible for all Telegram message interactions during video processing.
/// Follows SRP: only handles user-facing notifications, not download logic.
pub struct ChatNotifier<'a> {
    bot: &'a Bot,
    chat_id: ChatId,
    reply_to: MessageId,
}

impl<'a> ChatNotifier<'a> {
    pub fn new(bot: &'a Bot, chat_id: ChatId, reply_to: MessageId) -> Self {
        Self {
            bot,
            chat_id,
            reply_to,
        }
    }

    pub fn bot(&self) -> &Bot {
        self.bot
    }

    pub fn chat_id(&self) -> ChatId {
        self.chat_id
    }

    /// Downloads thumbnail locally and sends it, with a short timeout.
    /// Avoids passing CDN URLs directly to Telegram (which may fail due to
    /// expired tokens, region blocks, or required headers).
    pub async fn send_thumbnail(&self, info: &VideoInfo, client: &Client) {
        let Some(ref cover_url) = info.metadata.cover_url else {
            return;
        };

        // SSRF check: the cover URL comes from the API and could point anywhere
        if validate_download_url(cover_url).is_err() {
            tracing::warn!(url = %cover_url, "Thumbnail URL failed SSRF validation");
            return;
        }

        let result = tokio::time::timeout(Duration::from_secs(THUMBNAIL_TIMEOUT_SECS), async {
            let response = client.get(cover_url).send().await.ok()?;

            // Reject oversized thumbnails before reading into memory
            if let Some(len) = response.content_length() {
                if len > MAX_THUMBNAIL_BYTES as u64 {
                    return None;
                }
            }

            let bytes = response.bytes().await.ok()?;
            if bytes.len() > MAX_THUMBNAIL_BYTES {
                return None;
            }
            Some(bytes)
        })
        .await;

        match result {
            Ok(Some(bytes)) => {
                let _ = self
                    .bot
                    .send_photo(self.chat_id, InputFile::memory(bytes).file_name("cover.jpg"))
                    .await;
            }
            _ => {
                tracing::debug!(url = %cover_url, "Thumbnail download skipped (timeout or error)");
            }
        }
    }

    pub async fn send_progress_message(&self, text: &str) -> Result<MessageId, BotError> {
        let msg = self
            .bot
            .send_message(self.chat_id, text)
            .reply_parameters(ReplyParameters::new(self.reply_to))
            .await?;
        Ok(msg.id)
    }

    pub async fn update_progress(&self, progress_msg_id: MessageId, text: &str) {
        let _ = self
            .bot
            .edit_message_text(self.chat_id, progress_msg_id, text)
            .await;
    }

    pub async fn show_upload_action(&self) {
        let _ = self
            .bot
            .send_chat_action(self.chat_id, ChatAction::UploadVideo)
            .await;
    }

    pub async fn send_video(
        &self,
        file_path: &std::path::Path,
        caption: &str,
        duration: Option<u32>,
    ) -> Result<(), BotError> {
        let input_file = InputFile::file(file_path).file_name("tiktok_video.mp4");

        let mut request = self
            .bot
            .send_video(self.chat_id, input_file)
            .supports_streaming(true)
            .reply_parameters(ReplyParameters::new(self.reply_to));

        if let Some(d) = duration {
            request = request.duration(d);
        }

        if !caption.is_empty() {
            request = request.caption(caption);
        }

        request.await?;
        Ok(())
    }

    pub async fn delete_message(&self, msg_id: MessageId) {
        let _ = self.bot.delete_message(self.chat_id, msg_id).await;
    }

    pub async fn send_error(&self, message: &str) -> Result<(), BotError> {
        self.bot
            .send_message(self.chat_id, message)
            .reply_parameters(ReplyParameters::new(self.reply_to))
            .await?;
        Ok(())
    }
}
