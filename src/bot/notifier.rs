use teloxide::prelude::*;
use teloxide::types::{ChatAction, InputFile, MessageId, ReplyParameters};

use crate::error::BotError;

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
