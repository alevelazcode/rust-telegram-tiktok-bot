use crate::tiktok::downloader::VideoMetadata;

fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn build_caption(metadata: &VideoMetadata, file_size: u64) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(8);

    // Title
    if let Some(ref title) = metadata.title {
        lines.push(title.clone());
    }

    // Author
    if let Some(ref author) = metadata.author {
        let display = match (&author.nickname, &author.username) {
            (Some(nick), Some(user)) => format!("\u{1f464} @{} ({})", user, nick),
            (None, Some(user)) => format!("\u{1f464} @{}", user),
            (Some(nick), None) => format!("\u{1f464} {}", nick),
            (None, None) => String::new(),
        };
        if !display.is_empty() {
            lines.push(display);
        }
    }

    // Stats — each on its own line for readability
    if let Some(plays) = metadata.stats.play_count {
        lines.push(format!("\u{25b6}\u{fe0f} {} views", format_count(plays)));
    }
    if let Some(likes) = metadata.stats.like_count {
        lines.push(format!("\u{2764}\u{fe0f} {} likes", format_count(likes)));
    }
    if let Some(comments) = metadata.stats.comment_count {
        lines.push(format!("\u{1f4ac} {} comments", format_count(comments)));
    }
    if let Some(shares) = metadata.stats.share_count {
        lines.push(format!("\u{1f504} {} shares", format_count(shares)));
    }

    // Duration
    if let Some(duration) = metadata.duration_secs {
        let mins = duration / 60;
        let secs = duration % 60;
        lines.push(format!("\u{23f1}\u{fe0f} {}:{:02}", mins, secs));
    }

    // File size
    if file_size > 0 {
        let size_mb = file_size as f64 / (1024.0 * 1024.0);
        lines.push(format!("\u{1f4be} {:.1} MB", size_mb));
    }

    // Music
    if let Some(ref music) = metadata.music_title {
        let music_str = match metadata.music_author {
            Some(ref author) => format!("\u{1f3b5} {} - {}", author, music),
            None => format!("\u{1f3b5} {}", music),
        };
        lines.push(music_str);
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiktok::downloader::{AuthorInfo, VideoStats};

    fn empty_stats() -> VideoStats {
        VideoStats {
            play_count: None,
            like_count: None,
            comment_count: None,
            share_count: None,
            download_count: None,
            collect_count: None,
        }
    }

    fn minimal_metadata() -> VideoMetadata {
        VideoMetadata {
            title: None,
            duration_secs: None,
            file_size_bytes: 0,
            cover_url: None,
            create_time: None,
            author: None,
            stats: empty_stats(),
            music_title: None,
            music_author: None,
        }
    }

    #[test]
    fn all_fields_present() {
        let m = VideoMetadata {
            title: Some("Cool Video".into()),
            duration_secs: Some(125),
            file_size_bytes: 0,
            cover_url: None,
            create_time: None,
            author: Some(AuthorInfo {
                username: Some("user123".into()),
                nickname: Some("Cool User".into()),
                avatar_url: None,
            }),
            stats: VideoStats {
                play_count: Some(1_500_000),
                like_count: Some(50_000),
                comment_count: Some(1_200),
                share_count: Some(300),
                download_count: None,
                collect_count: None,
            },
            music_title: Some("Song".into()),
            music_author: Some("Artist".into()),
        };
        let caption = build_caption(&m, 5 * 1024 * 1024);

        assert!(caption.contains("Cool Video"));
        assert!(caption.contains("@user123 (Cool User)"));
        assert!(caption.contains("1.5M views"));
        assert!(caption.contains("50.0K likes"));
        assert!(caption.contains("1.2K comments"));
        assert!(caption.contains("300 shares"));
        assert!(caption.contains("2:05"));
        assert!(caption.contains("5.0 MB"));
        assert!(caption.contains("Artist - Song"));

        // Each field is on its own line
        let line_count = caption.lines().count();
        assert_eq!(line_count, 9);
    }

    #[test]
    fn empty_metadata_returns_empty_string() {
        assert!(build_caption(&minimal_metadata(), 0).is_empty());
    }

    #[test]
    fn title_only() {
        let mut m = minimal_metadata();
        m.title = Some("Just a title".into());
        assert_eq!(build_caption(&m, 0), "Just a title");
    }

    #[test]
    fn duration_formats_correctly() {
        let mut m = minimal_metadata();
        m.duration_secs = Some(5);
        assert!(build_caption(&m, 0).contains("0:05"));

        m.duration_secs = Some(60);
        assert!(build_caption(&m, 0).contains("1:00"));

        m.duration_secs = Some(3661);
        assert!(build_caption(&m, 0).contains("61:01"));
    }

    #[test]
    fn author_username_only() {
        let mut m = minimal_metadata();
        m.author = Some(AuthorInfo {
            username: Some("user".into()),
            nickname: None,
            avatar_url: None,
        });
        assert!(build_caption(&m, 0).contains("@user"));
    }

    #[test]
    fn stats_format_counts() {
        let mut m = minimal_metadata();
        m.stats.like_count = Some(500);
        assert!(build_caption(&m, 0).contains("500 likes"));

        m.stats.like_count = Some(1_500);
        assert!(build_caption(&m, 0).contains("1.5K likes"));

        m.stats.like_count = Some(2_500_000);
        assert!(build_caption(&m, 0).contains("2.5M likes"));
    }

    #[test]
    fn format_count_values() {
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_000), "1.0K");
        assert_eq!(format_count(1_500), "1.5K");
        assert_eq!(format_count(1_000_000), "1.0M");
        assert_eq!(format_count(2_500_000), "2.5M");
    }

    #[test]
    fn music_without_author() {
        let mut m = minimal_metadata();
        m.music_title = Some("Song".into());
        assert!(build_caption(&m, 0).contains("Song"));
    }

    #[test]
    fn music_with_author() {
        let mut m = minimal_metadata();
        m.music_title = Some("Song".into());
        m.music_author = Some("Artist".into());
        assert!(build_caption(&m, 0).contains("Artist - Song"));
    }
}
