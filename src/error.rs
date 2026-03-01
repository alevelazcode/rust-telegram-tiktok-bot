use thiserror::Error;

#[derive(Debug, Error)]
pub enum BotError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("TikTok API error: {0}")]
    TikTokApi(String),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Telegram API error: {0}")]
    Telegram(#[from] teloxide::RequestError),

    #[error("No video found in the API response")]
    NoVideoFound,

    #[error("Video too large: {size_mb:.1} MB (Telegram limit: 50 MB)")]
    FileTooLarge { size_mb: f64 },

    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Too many requests from this user")]
    RateLimited,

    #[error("Too many concurrent downloads")]
    TooManyDownloads,

    #[error("The download URL failed safety validation")]
    UnsafeUrl,
}

impl BotError {
    pub fn user_friendly_message(&self) -> String {
        match self {
            BotError::Http(e) => {
                if e.is_timeout() {
                    "\u{23f0} The download timed out. The video server is responding too slowly. Please try again later.".to_string()
                } else if e.is_connect() {
                    "\u{1f50c} Could not connect to the video server. The server may be temporarily down. Please try again in a moment.".to_string()
                } else {
                    "\u{26a0}\u{fe0f} Could not download the video. The link may have expired or the server is temporarily unavailable. Please try again.".to_string()
                }
            }
            BotError::TikTokApi(msg) => format!("\u{26a0}\u{fe0f} TikTok returned an error: {}. The video may be private, deleted, or region-restricted.", msg),
            BotError::NoVideoFound => "\u{274c} No downloadable video was found. The link may be invalid, the video may be private, or it may have been removed.".to_string(),
            BotError::FileTooLarge { size_mb } => format!("\u{1f4e6} The video is too large ({:.1} MB). Telegram only allows files up to 50 MB.", size_mb),
            BotError::Io(_) => "\u{26a0}\u{fe0f} A temporary file error occurred on the server. Please try again.".to_string(),
            BotError::Telegram(_) => "\u{26a0}\u{fe0f} Could not send the video through Telegram. Please try again in a moment.".to_string(),
            BotError::Json(_) => "\u{26a0}\u{fe0f} Received an unexpected response from the download server. Please try again later.".to_string(),
            BotError::RateLimited => "\u{23f3} You're sending too many requests. Please wait a minute before trying again.".to_string(),
            BotError::TooManyDownloads => "\u{1f6a6} The bot is currently busy with other downloads. Please try again in a moment.".to_string(),
            BotError::UnsafeUrl => "\u{1f6ab} The video URL could not be verified as safe. Please try a different link.".to_string(),
            BotError::Config(_) | BotError::UrlParse(_) => "\u{26a0}\u{fe0f} An internal error occurred. Please try again later.".to_string(),
        }
    }
}
