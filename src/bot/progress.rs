use crate::tiktok::downloader::{DownloadProgress, VideoInfo};

/// Builds the initial progress message shown before download starts.
pub fn build_initial_message(info: &VideoInfo) -> String {
    let mut parts = vec!["\u{1f3ac} Downloading TikTok video...".to_string()];

    if let Some(ref author) = info.metadata.author {
        if let Some(ref username) = author.username {
            parts.push(format!("\u{1f464} @{}", username));
        }
    }

    if info.metadata.file_size_bytes > 0 {
        let size_mb = info.metadata.file_size_bytes as f64 / (1024.0 * 1024.0);
        parts.push(format!("\u{1f4be} Size: {:.1} MB", size_mb));
    }

    if let Some(duration) = info.metadata.duration_secs {
        let mins = duration / 60;
        let secs = duration % 60;
        parts.push(format!("\u{23f1}\u{fe0f} Duration: {}:{:02}", mins, secs));
    }

    parts.push("\n\u{23f3} Starting download...".to_string());

    parts.join("\n")
}

/// Builds the live progress text during download.
pub fn build_download_text(info: &VideoInfo, progress: &DownloadProgress) -> String {
    let mut parts = vec!["\u{1f3ac} Downloading TikTok video...".to_string()];

    if let Some(ref author) = info.metadata.author {
        if let Some(ref username) = author.username {
            parts.push(format!("\u{1f464} @{}", username));
        }
    }

    let downloaded_mb = progress.downloaded_bytes as f64 / (1024.0 * 1024.0);

    match (progress.percentage(), progress.total_bytes) {
        (Some(pct), Some(total)) => {
            let total_mb = total as f64 / (1024.0 * 1024.0);
            let bar = build_progress_bar(pct);
            parts.push(format!("\n{} {}%", bar, pct));
            parts.push(format!("\u{1f4be} {:.1} / {:.1} MB", downloaded_mb, total_mb));
        }
        _ => {
            parts.push(format!("\n\u{1f4be} Downloaded: {:.1} MB", downloaded_mb));
        }
    }

    parts.join("\n")
}

fn build_progress_bar(percentage: u8) -> String {
    let filled = (percentage as usize) / 10;
    let empty = 10 - filled;
    let bar: String = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(empty);
    format!("[{}]", bar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiktok::downloader::{AuthorInfo, VideoMetadata, VideoStats};

    fn test_info(size: u64, duration: Option<u32>, username: Option<&str>) -> VideoInfo {
        VideoInfo {
            video_url: "https://example.com/video.mp4".to_string(),
            metadata: VideoMetadata {
                title: Some("Test".into()),
                duration_secs: duration,
                file_size_bytes: size,
                cover_url: None,
                create_time: None,
                author: username.map(|u| AuthorInfo {
                    username: Some(u.to_string()),
                    nickname: None,
                    avatar_url: None,
                }),
                stats: VideoStats {
                    play_count: None,
                    like_count: None,
                    comment_count: None,
                    share_count: None,
                    download_count: None,
                    collect_count: None,
                },
                music_title: None,
                music_author: None,
            },
        }
    }

    #[test]
    fn initial_message_contains_size_and_duration() {
        let info = test_info(5 * 1024 * 1024, Some(30), Some("user1"));
        let msg = build_initial_message(&info);
        assert!(msg.contains("5.0 MB"));
        assert!(msg.contains("0:30"));
        assert!(msg.contains("@user1"));
        assert!(msg.contains("Starting download"));
    }

    #[test]
    fn initial_message_without_optional_fields() {
        let info = test_info(0, None, None);
        let msg = build_initial_message(&info);
        assert!(msg.contains("Downloading TikTok video"));
        assert!(!msg.contains("MB"));
        assert!(!msg.contains("Duration"));
    }

    #[test]
    fn download_text_with_known_total() {
        let info = test_info(10 * 1024 * 1024, None, None);
        let progress = DownloadProgress {
            downloaded_bytes: 5 * 1024 * 1024,
            total_bytes: Some(10 * 1024 * 1024),
        };
        let text = build_download_text(&info, &progress);
        assert!(text.contains("50%"));
        assert!(text.contains("5.0 / 10.0 MB"));
    }

    #[test]
    fn download_text_without_total() {
        let info = test_info(0, None, None);
        let progress = DownloadProgress {
            downloaded_bytes: 3 * 1024 * 1024,
            total_bytes: None,
        };
        let text = build_download_text(&info, &progress);
        assert!(text.contains("Downloaded: 3.0 MB"));
    }

    #[test]
    fn progress_bar_rendering() {
        assert_eq!(build_progress_bar(0), "[\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}]");
        assert_eq!(build_progress_bar(50), "[\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}]");
        assert_eq!(build_progress_bar(100), "[\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}]");
    }
}
