# TikTok Telegram Bot

A Telegram bot built in Rust that automatically detects TikTok links in chat messages, downloads the video, and sends it directly to the conversation. Designed for users in regions where TikTok is blocked or restricted.

## Features

- **Automatic link detection** — Recognizes TikTok URLs from `tiktok.com`, `vm.tiktok.com`, `vt.tiktok.com`, and `m.tiktok.com`
- **Rich metadata captions** — Videos are sent with title, author, views, likes, comments, shares, duration, file size, and music info
- **Real-time download progress** — Shows a live progress bar with percentage while downloading
- **Thumbnail preview** — Sends the video cover image before the download completes
- **Streaming downloads** — Downloads video in chunks with buffered I/O, keeping memory usage low
- **Retry with exponential backoff** — Automatically retries on transient network failures

## Security

- **Per-user rate limiting** — 5 requests per minute per user (governor)
- **Concurrent download limit** — Max 5 simultaneous downloads globally (tokio semaphore)
- **SSRF protection** — DNS resolution check blocks private/reserved IPs on all downloaded URLs (video + thumbnail)
- **User/chat whitelists** — Optional authorization via `AUTHORIZED_USERS` and `AUTHORIZED_CHATS`
- **URL hardening** — HTTPS-only, no credentials, no custom ports, max length, max 3 URLs per message
- **API response size limit** — Caps API responses at 1 MB to prevent memory exhaustion
- **Temp file cleanup** — Background task removes orphaned files every 60 seconds
- **Restrictive temp directory permissions** — 0o700 on Unix

## Project Structure

```
src/
├── main.rs                        # Entry point: env, tracing, config, bot startup
├── config.rs                      # AppConfig from environment variables
├── error.rs                       # BotError enum with user-friendly messages
├── bot/
│   ├── mod.rs                     # Dispatcher setup, HTTP client, dependency injection
│   ├── handlers.rs                # Message handler orchestration
│   ├── notifier.rs                # Telegram message interactions (SRP)
│   ├── caption.rs                 # Video caption formatting
│   └── progress.rs                # Download progress bar formatting
├── tiktok/
│   ├── mod.rs                     # Module re-exports
│   ├── detector.rs                # TikTok URL extraction and validation
│   ├── models.rs                  # Domain types (VideoInfo, VideoMetadata, etc.)
│   ├── api_client.rs              # TikWM API interaction and response parsing
│   └── downloader.rs              # Streaming video file download
└── security/
    ├── mod.rs                     # Module re-exports
    ├── rate_limiter.rs            # Per-user rate limiting (governor)
    ├── download_guard.rs          # Concurrent download semaphore
    ├── url_validator.rs           # SSRF protection with DNS check
    ├── retry.rs                   # Exponential backoff retry logic
    └── temp_cleaner.rs            # Background temp file cleanup
```

## Requirements

- Rust 2024 edition
- A Telegram bot token from [@BotFather](https://t.me/BotFather)

## Setup

1. Clone the repository:
   ```bash
   git clone <repo-url>
   cd telegram-bot
   ```

2. Create a `.env` file from the example:
   ```bash
   cp .env.example .env
   ```

3. Edit `.env` and add your bot token:
   ```env
   TELOXIDE_TOKEN=your_bot_token_here
   RUST_LOG=info
   ```

4. (Optional) Restrict access to specific users/chats:
   ```env
   AUTHORIZED_USERS=123456789,987654321
   AUTHORIZED_CHATS=-1001234567890
   ```

5. Build and run:
   ```bash
   cargo run --release
   ```

## Configuration

| Variable | Required | Default | Description |
|---|---|---|---|
| `TELOXIDE_TOKEN` | Yes | — | Telegram bot token from @BotFather |
| `RUST_LOG` | No | — | Log level (`info`, `debug`, `warn`, `error`) |
| `TIKWM_API_URL` | No | `https://www.tikwm.com/api/` | TikTok video resolution API endpoint |
| `AUTHORIZED_USERS` | No | (allow all) | Comma-separated user IDs |
| `AUTHORIZED_CHATS` | No | (allow all) | Comma-separated chat IDs |

## Testing

```bash
cargo test
```

59 unit tests covering URL detection, API parsing, caption formatting, progress display, SSRF validation, retry logic, and streaming downloads.

## License

MIT
