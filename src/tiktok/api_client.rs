use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;

use crate::error::BotError;
use crate::tiktok::models::{AuthorInfo, VideoInfo, VideoMetadata, VideoStats};

/// Maximum file size we'll attempt to download (can be compressed afterward).
const MAX_DOWNLOAD_SIZE: u64 = 200 * 1024 * 1024; // 200 MB

/// Maximum API response size (1 MB). Prevents memory exhaustion from a
/// compromised or malicious API endpoint returning an oversized body.
const MAX_API_RESPONSE_BYTES: usize = 1024 * 1024;

// --- API response structs (private, deserialization only) ---

#[derive(Debug, Deserialize)]
struct TikWmResponse {
    code: i32,
    msg: String,
    data: Option<TikWmData>,
}

#[derive(Debug, Deserialize)]
struct TikWmData {
    play: Option<String>,
    title: Option<String>,
    duration: Option<u32>,
    size: Option<u64>,
    cover: Option<String>,
    origin_cover: Option<String>,
    create_time: Option<i64>,
    author: Option<TikWmAuthor>,
    music_info: Option<TikWmMusic>,
    play_count: Option<u64>,
    digg_count: Option<u64>,
    comment_count: Option<u64>,
    share_count: Option<u64>,
    download_count: Option<u64>,
    collect_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TikWmAuthor {
    unique_id: Option<String>,
    nickname: Option<String>,
    avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TikWmMusic {
    title: Option<String>,
    author: Option<String>,
}

// --- Parsing (pure, testable) ---

fn parse_api_response(json: &str) -> Result<VideoInfo, BotError> {
    let api_response: TikWmResponse = serde_json::from_str(json)?;

    if api_response.code != 0 {
        return Err(BotError::TikTokApi(api_response.msg));
    }

    let data = api_response.data.ok_or(BotError::NoVideoFound)?;
    let video_url = data.play.ok_or(BotError::NoVideoFound)?;

    let author = data.author.map(|a| AuthorInfo {
        username: a.unique_id,
        nickname: a.nickname,
        avatar_url: a.avatar,
    });

    let (music_title, music_author) = match data.music_info {
        Some(m) => (m.title, m.author),
        None => (None, None),
    };

    Ok(VideoInfo {
        video_url,
        metadata: VideoMetadata {
            title: data.title,
            duration_secs: data.duration,
            file_size_bytes: data.size.unwrap_or(0),
            cover_url: data.cover.or(data.origin_cover),
            create_time: data.create_time,
            author,
            stats: VideoStats {
                play_count: data.play_count,
                like_count: data.digg_count,
                comment_count: data.comment_count,
                share_count: data.share_count,
                download_count: data.download_count,
                collect_count: data.collect_count,
            },
            music_title,
            music_author,
        },
    })
}

// --- Helpers ---

/// Reads a response body up to `max_bytes`, returning an error if exceeded.
async fn read_limited_body(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<String, BotError> {
    // Early reject if Content-Length exceeds limit
    if let Some(len) = response.content_length() {
        if len > max_bytes as u64 {
            return Err(BotError::TikTokApi(format!(
                "Response too large: {} bytes",
                len
            )));
        }
    }

    let mut stream = response.bytes_stream();
    let mut buf = Vec::with_capacity(max_bytes.min(64 * 1024));

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if buf.len() + chunk.len() > max_bytes {
            return Err(BotError::TikTokApi(
                "Response body exceeded size limit".to_string(),
            ));
        }
        buf.extend_from_slice(&chunk);
    }

    String::from_utf8(buf).map_err(|_| BotError::TikTokApi("Invalid UTF-8 in response".to_string()))
}

// --- Public API ---

/// Fetches video info from the TikWM API for a given TikTok URL.
pub async fn fetch_video_info(
    client: &Client,
    api_url: &str,
    tiktok_url: &str,
) -> Result<VideoInfo, BotError> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("url", tiktok_url)
        .finish();

    let response = client
        .post(api_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;

    let response_text = read_limited_body(response, MAX_API_RESPONSE_BYTES).await?;
    let info = parse_api_response(&response_text)?;

    if info.metadata.file_size_bytes > MAX_DOWNLOAD_SIZE {
        return Err(BotError::FileTooLarge {
            size_mb: info.metadata.file_size_bytes as f64 / (1024.0 * 1024.0),
        });
    }

    Ok(info)
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parse_success_with_all_fields() {
        let json = serde_json::json!({
            "code": 0,
            "msg": "success",
            "data": {
                "play": "https://cdn.example.com/video.mp4",
                "title": "Test Video",
                "duration": 45,
                "size": 5242880,
                "cover": "https://cdn.example.com/cover.jpg",
                "create_time": 1700000000,
                "author": {
                    "unique_id": "testuser",
                    "nickname": "Test User",
                    "avatar": "https://cdn.example.com/avatar.jpg"
                },
                "music_info": { "title": "Song", "author": "Artist" },
                "play_count": 100000,
                "digg_count": 5000,
                "comment_count": 200,
                "share_count": 50,
                "download_count": 30,
                "collect_count": 10
            }
        });

        let info = parse_api_response(&json.to_string()).unwrap();
        assert_eq!(info.video_url, "https://cdn.example.com/video.mp4");
        assert_eq!(info.metadata.title.as_deref(), Some("Test Video"));
        assert_eq!(info.metadata.duration_secs, Some(45));
        assert_eq!(info.metadata.file_size_bytes, 5242880);
        assert_eq!(
            info.metadata.cover_url.as_deref(),
            Some("https://cdn.example.com/cover.jpg")
        );
        assert_eq!(info.metadata.create_time, Some(1700000000));

        let author = info.metadata.author.as_ref().unwrap();
        assert_eq!(author.username.as_deref(), Some("testuser"));
        assert_eq!(author.nickname.as_deref(), Some("Test User"));

        assert_eq!(info.metadata.stats.like_count, Some(5000));
        assert_eq!(info.metadata.stats.comment_count, Some(200));
        assert_eq!(info.metadata.stats.share_count, Some(50));
        assert_eq!(info.metadata.stats.play_count, Some(100000));

        assert_eq!(info.metadata.music_title.as_deref(), Some("Song"));
        assert_eq!(info.metadata.music_author.as_deref(), Some("Artist"));
    }

    #[test]
    fn parse_success_minimal_fields() {
        let json = serde_json::json!({
            "code": 0,
            "msg": "success",
            "data": { "play": "https://cdn.example.com/video.mp4" }
        });

        let info = parse_api_response(&json.to_string()).unwrap();
        assert_eq!(info.video_url, "https://cdn.example.com/video.mp4");
        assert!(info.metadata.title.is_none());
        assert!(info.metadata.author.is_none());
        assert!(info.metadata.stats.like_count.is_none());
    }

    #[test]
    fn parse_api_error_code() {
        let json = serde_json::json!({ "code": -1, "msg": "Video not found" });
        let err = parse_api_response(&json.to_string()).unwrap_err();
        assert!(matches!(err, BotError::TikTokApi(msg) if msg == "Video not found"));
    }

    #[test]
    fn parse_missing_data_field() {
        let json = serde_json::json!({ "code": 0, "msg": "success" });
        let err = parse_api_response(&json.to_string()).unwrap_err();
        assert!(matches!(err, BotError::NoVideoFound));
    }

    #[test]
    fn parse_missing_play_url() {
        let json = serde_json::json!({
            "code": 0,
            "msg": "success",
            "data": { "title": "No play URL here" }
        });
        let err = parse_api_response(&json.to_string()).unwrap_err();
        assert!(matches!(err, BotError::NoVideoFound));
    }

    #[test]
    fn parse_invalid_json() {
        let err = parse_api_response("not valid json").unwrap_err();
        assert!(matches!(err, BotError::Json(_)));
    }

    #[tokio::test]
    async fn fetch_video_info_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "code": 0,
                    "msg": "success",
                    "data": {
                        "play": "https://cdn.example.com/video.mp4",
                        "title": "Test",
                        "duration": 10,
                        "size": 1024,
                        "digg_count": 999
                    }
                })),
            )
            .mount(&server)
            .await;

        let client = Client::new();
        let info =
            fetch_video_info(&client, &format!("{}/", server.uri()), "https://tiktok.com/v/123")
                .await
                .unwrap();

        assert_eq!(info.metadata.title.as_deref(), Some("Test"));
        assert_eq!(info.metadata.duration_secs, Some(10));
        assert_eq!(info.metadata.stats.like_count, Some(999));
    }

    #[tokio::test]
    async fn rejects_oversized_api_response() {
        let server = MockServer::start().await;

        // Return a body larger than MAX_API_RESPONSE_BYTES (1 MB)
        let oversized_body = "x".repeat(MAX_API_RESPONSE_BYTES + 1);
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(oversized_body))
            .mount(&server)
            .await;

        let client = Client::new();
        let result =
            fetch_video_info(&client, &format!("{}/", server.uri()), "https://tiktok.com/v/789")
                .await;

        assert!(matches!(result.unwrap_err(), BotError::TikTokApi(_)));
    }

    #[tokio::test]
    async fn fetch_video_info_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "code": -1,
                    "msg": "something went wrong"
                })),
            )
            .mount(&server)
            .await;

        let client = Client::new();
        let result =
            fetch_video_info(&client, &format!("{}/", server.uri()), "https://tiktok.com/v/456")
                .await;

        assert!(matches!(result.unwrap_err(), BotError::TikTokApi(_)));
    }
}
