use std::path::Path;

use tempfile::NamedTempFile;
use tokio::process::Command;

use crate::error::BotError;
use crate::security::temp_cleaner::get_temp_dir;
use crate::tiktok::models::DownloadedFile;

/// Telegram's file size limit for uploads.
const TELEGRAM_FILE_SIZE_LIMIT: u64 = 50 * 1024 * 1024; // 50 MB

/// Target size for compressed output (3 MB safety margin for container overhead).
const COMPRESSION_TARGET_BYTES: u64 = 47 * 1024 * 1024; // 47 MB

/// Audio bitrate in kbps (stereo AAC, transparent quality for speech/music).
const AUDIO_BITRATE_KBPS: u64 = 128;

/// Minimum video bitrate below which quality is unacceptable.
const MIN_VIDEO_BITRATE_KBPS: u64 = 400;

/// Calculates the target video bitrate (kbps) to fit within the size limit.
fn calculate_video_bitrate_kbps(duration_secs: u32) -> u64 {
    let total_kbps = (COMPRESSION_TARGET_BYTES * 8) / (duration_secs as u64 * 1000);
    total_kbps.saturating_sub(AUDIO_BITRATE_KBPS)
}

/// Selects the ffmpeg scale filter based on available bitrate.
/// Lower bitrates get lower resolutions for better quality-per-bit.
fn select_scale_filter(video_bitrate_kbps: u64) -> &'static str {
    if video_bitrate_kbps > 2000 {
        // Keep original, cap width at 1280px
        "scale='min(iw,1280)':-2"
    } else if video_bitrate_kbps > 800 {
        "scale=-2:720"
    } else {
        "scale=-2:480"
    }
}

/// Compresses a video using ffmpeg to fit within Telegram's file size limit.
///
/// Uses one-pass H.264 encoding with calculated bitrate based on video duration.
/// The output is written to a new temporary file; the original is kept alive
/// by the caller until the compressed file is sent.
pub async fn compress_video(
    input: &Path,
    duration_secs: u32,
) -> Result<DownloadedFile, BotError> {
    let video_bitrate = calculate_video_bitrate_kbps(duration_secs);

    if video_bitrate < MIN_VIDEO_BITRATE_KBPS {
        return Err(BotError::CompressionFailed(format!(
            "Video too long ({} secs) for acceptable quality at target size",
            duration_secs
        )));
    }

    let scale_filter = select_scale_filter(video_bitrate);

    let output_file = NamedTempFile::new_in(get_temp_dir())?;
    let output_path = output_file.path().to_path_buf();

    // NamedTempFile creates an empty file. We need ffmpeg to write to a .mp4 path,
    // so we use a separate path with .mp4 extension in the same temp dir.
    let mp4_path = output_path.with_extension("mp4");

    tracing::info!(
        video_bitrate_kbps = video_bitrate,
        scale = scale_filter,
        duration_secs = duration_secs,
        "Compressing video with ffmpeg"
    );

    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            input.to_str().unwrap_or_default(),
            "-c:v",
            "libx264",
            "-profile:v",
            "main",
            "-preset",
            "fast",
            "-b:v",
            &format!("{}k", video_bitrate),
            "-pix_fmt",
            "yuv420p",
            "-vf",
            scale_filter,
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-ar",
            "44100",
            "-ac",
            "2",
            "-movflags",
            "+faststart",
        ])
        .arg(mp4_path.to_str().unwrap_or_default())
        .output()
        .await
        .map_err(|e| {
            BotError::CompressionFailed(format!("Failed to run ffmpeg: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(stderr = %stderr, "ffmpeg compression failed");
        return Err(BotError::CompressionFailed(
            "ffmpeg exited with an error".to_string(),
        ));
    }

    // Verify the compressed file fits within Telegram's limit
    let metadata = tokio::fs::metadata(&mp4_path).await?;
    let compressed_size = metadata.len();

    tracing::info!(
        original_mb = format!("{:.1}", 0.0).as_str(), // caller logs original size
        compressed_mb = format!("{:.1}", compressed_size as f64 / (1024.0 * 1024.0)).as_str(),
        "Compression complete"
    );

    if compressed_size > TELEGRAM_FILE_SIZE_LIMIT {
        // Clean up and fail
        let _ = tokio::fs::remove_file(&mp4_path).await;
        return Err(BotError::CompressionFailed(format!(
            "Compressed file still too large: {:.1} MB",
            compressed_size as f64 / (1024.0 * 1024.0)
        )));
    }

    // Move the .mp4 file to the NamedTempFile path so it's cleaned up on drop
    tokio::fs::rename(&mp4_path, &output_path).await?;

    Ok(DownloadedFile {
        file_path: output_path,
        _temp_file: output_file,
        actual_size: compressed_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_short_video() {
        // 15 second video: should have very high bitrate
        let bitrate = calculate_video_bitrate_kbps(15);
        assert!(bitrate > 10_000, "15s video should be >10000 kbps, got {}", bitrate);
    }

    #[test]
    fn bitrate_medium_video() {
        // 60 second video
        let bitrate = calculate_video_bitrate_kbps(60);
        assert!(bitrate > 4000 && bitrate < 7000, "60s video bitrate: {}", bitrate);
    }

    #[test]
    fn bitrate_long_video() {
        // 3 minute video
        let bitrate = calculate_video_bitrate_kbps(180);
        assert!(bitrate > 1000 && bitrate < 3000, "180s video bitrate: {}", bitrate);
    }

    #[test]
    fn bitrate_very_long_video() {
        // 15 minute video: bitrate drops below acceptable threshold
        let bitrate = calculate_video_bitrate_kbps(900);
        assert!(bitrate < MIN_VIDEO_BITRATE_KBPS, "900s should be below min: {}", bitrate);
    }

    #[test]
    fn scale_filter_high_bitrate() {
        assert_eq!(select_scale_filter(5000), "scale='min(iw,1280)':-2");
    }

    #[test]
    fn scale_filter_medium_bitrate() {
        assert_eq!(select_scale_filter(1500), "scale=-2:720");
    }

    #[test]
    fn scale_filter_low_bitrate() {
        assert_eq!(select_scale_filter(600), "scale=-2:480");
    }

    #[test]
    fn scale_filter_boundary_2000() {
        // Exactly 2000 should go to 720p
        assert_eq!(select_scale_filter(2000), "scale=-2:720");
    }

    #[test]
    fn scale_filter_boundary_800() {
        // Exactly 800 should go to 480p
        assert_eq!(select_scale_filter(800), "scale=-2:480");
    }
}
