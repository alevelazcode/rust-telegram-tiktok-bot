use futures::StreamExt;
use reqwest::Client;
use tempfile::NamedTempFile;
use tokio::io::{AsyncWriteExt, BufWriter};

use crate::error::BotError;
use crate::security::temp_cleaner::get_temp_dir;
use crate::tiktok::models::DownloadedFile;

/// Maximum download size. Larger than Telegram's 50 MB limit because videos
/// exceeding that limit will be compressed with ffmpeg before sending.
const MAX_DOWNLOAD_SIZE: u64 = 200 * 1024 * 1024; // 200 MB
const BUF_WRITER_CAPACITY: usize = 64 * 1024; // 64 KB

// Re-export public types so existing imports from `downloader` keep working.
pub use crate::tiktok::api_client::fetch_video_info;
#[allow(unused_imports)]
pub use crate::tiktok::models::{
    AuthorInfo, DownloadProgress, VideoInfo, VideoMetadata, VideoStats,
};

/// Downloads a video from a URL to a temporary file with streaming.
/// Calls `on_progress` after each chunk with current download progress.
pub async fn download_to_file<F>(
    client: &Client,
    video_url: &str,
    on_progress: F,
) -> Result<DownloadedFile, BotError>
where
    F: Fn(DownloadProgress),
{
    let response = client.get(video_url).send().await?;

    let total_bytes = response.content_length();

    if let Some(content_length) = total_bytes {
        if content_length > MAX_DOWNLOAD_SIZE {
            return Err(BotError::FileTooLarge {
                size_mb: content_length as f64 / (1024.0 * 1024.0),
            });
        }
    }

    let mut stream = response.bytes_stream();

    let temp_file = NamedTempFile::new_in(get_temp_dir())?;
    let file_path = temp_file.path().to_path_buf();
    let async_file = tokio::fs::File::create(&file_path).await?;
    let mut writer = BufWriter::with_capacity(BUF_WRITER_CAPACITY, async_file);
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded += chunk.len() as u64;

        if downloaded > MAX_DOWNLOAD_SIZE {
            return Err(BotError::FileTooLarge {
                size_mb: downloaded as f64 / (1024.0 * 1024.0),
            });
        }

        writer.write_all(&chunk).await?;

        on_progress(DownloadProgress {
            downloaded_bytes: downloaded,
            total_bytes,
        });
    }

    writer.flush().await?;

    Ok(DownloadedFile {
        file_path,
        _temp_file: temp_file,
        actual_size: downloaded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn download_to_file_writes_content() {
        let server = MockServer::start().await;
        let video_bytes = vec![0xDE, 0xAD, 0xBE, 0xEF];

        Mock::given(method("GET"))
            .and(path("/video.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(video_bytes.clone()))
            .mount(&server)
            .await;

        let client = Client::new();
        let downloaded =
            download_to_file(&client, &format!("{}/video.mp4", server.uri()), |_| {})
                .await
                .unwrap();

        assert_eq!(downloaded.actual_size, 4);
        assert!(downloaded.file_path.exists());

        let content = std::fs::read(&downloaded.file_path).unwrap();
        assert_eq!(content, video_bytes);
    }
}
