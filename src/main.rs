mod bot;
mod config;
mod error;
mod security;
mod tiktok;

use config::AppConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stdout)
        .init();

    tracing::info!("Starting TikTok Telegram Bot...");

    let config = AppConfig::load()?;

    tracing::info!("Configuration loaded successfully");

    bot::run(config).await;

    Ok(())
}
