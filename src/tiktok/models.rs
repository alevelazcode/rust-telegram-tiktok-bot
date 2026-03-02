use std::path::PathBuf;
use tempfile::NamedTempFile;

/// Author information from a TikTok video.
#[derive(Debug)]
pub struct AuthorInfo {
    pub username: Option<String>,
    pub nickname: Option<String>,
    #[allow(dead_code)]
    pub avatar_url: Option<String>,
}

/// Engagement statistics from a TikTok video.
#[derive(Debug)]
pub struct VideoStats {
    pub play_count: Option<u64>,
    pub like_count: Option<u64>,
    pub comment_count: Option<u64>,
    pub share_count: Option<u64>,
    #[allow(dead_code)]
    pub download_count: Option<u64>,
    #[allow(dead_code)]
    pub collect_count: Option<u64>,
}

/// Rich metadata about a TikTok video.
#[derive(Debug)]
pub struct VideoMetadata {
    pub title: Option<String>,
    pub duration_secs: Option<u32>,
    pub file_size_bytes: u64,
    #[allow(dead_code)]
    pub cover_url: Option<String>,
    #[allow(dead_code)]
    pub create_time: Option<i64>,
    pub author: Option<AuthorInfo>,
    pub stats: VideoStats,
    pub music_title: Option<String>,
    pub music_author: Option<String>,
}

/// Resolved video info: URL to download + metadata.
#[derive(Debug)]
pub struct VideoInfo {
    pub video_url: String,
    pub metadata: VideoMetadata,
}

/// A downloaded file on disk, held by a NamedTempFile for automatic cleanup.
pub struct DownloadedFile {
    pub file_path: PathBuf,
    pub _temp_file: NamedTempFile,
    pub actual_size: u64,
}

/// Progress data emitted during streaming download.
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

impl DownloadProgress {
    pub fn percentage(&self) -> Option<u8> {
        self.total_bytes.map(|total| {
            if total == 0 {
                100
            } else {
                ((self.downloaded_bytes * 100) / total).min(100) as u8
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_percentage_calculation() {
        let p = DownloadProgress {
            downloaded_bytes: 50,
            total_bytes: Some(100),
        };
        assert_eq!(p.percentage(), Some(50));

        let p = DownloadProgress {
            downloaded_bytes: 100,
            total_bytes: Some(100),
        };
        assert_eq!(p.percentage(), Some(100));

        let p = DownloadProgress {
            downloaded_bytes: 0,
            total_bytes: Some(100),
        };
        assert_eq!(p.percentage(), Some(0));

        let p = DownloadProgress {
            downloaded_bytes: 50,
            total_bytes: None,
        };
        assert_eq!(p.percentage(), None);

        let p = DownloadProgress {
            downloaded_bytes: 0,
            total_bytes: Some(0),
        };
        assert_eq!(p.percentage(), Some(100));
    }
}
