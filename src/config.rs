use config::{Config, ConfigError, Environment};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub teloxide_token: String,

    #[serde(default = "default_tikwm_api_url")]
    pub tikwm_api_url: String,

    /// Comma-separated list of authorized user IDs. Empty = allow all users.
    #[serde(default, deserialize_with = "deserialize_u64_csv")]
    pub authorized_users: HashSet<u64>,

    /// Comma-separated list of authorized chat IDs. Empty = allow all chats.
    #[serde(default, deserialize_with = "deserialize_i64_csv")]
    pub authorized_chats: HashSet<i64>,
}

fn default_tikwm_api_url() -> String {
    "https://www.tikwm.com/api/".to_string()
}

fn deserialize_u64_csv<'de, D>(deserializer: D) -> Result<HashSet<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer).unwrap_or_default();
    Ok(s.split(',')
        .filter_map(|id| id.trim().parse().ok())
        .collect())
}

fn deserialize_i64_csv<'de, D>(deserializer: D) -> Result<HashSet<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer).unwrap_or_default();
    Ok(s.split(',')
        .filter_map(|id| id.trim().parse().ok())
        .collect())
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(Environment::default())
            .build()?;

        config.try_deserialize::<AppConfig>()
    }

    /// Check if a user is authorized (empty whitelist = allow all).
    pub fn is_user_authorized(&self, user_id: u64) -> bool {
        self.authorized_users.is_empty() || self.authorized_users.contains(&user_id)
    }

    /// Check if a chat is authorized (empty whitelist = allow all).
    pub fn is_chat_authorized(&self, chat_id: i64) -> bool {
        self.authorized_chats.is_empty() || self.authorized_chats.contains(&chat_id)
    }
}
