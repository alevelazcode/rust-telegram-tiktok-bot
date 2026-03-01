use std::path::PathBuf;
use tokio::fs;

const TEMP_DIR_NAME: &str = "tiktok_bot_tmp";
const TEMP_FILE_MAX_AGE_SECS: u64 = 300; // 5 minutes

/// Returns the bot's dedicated temp directory, creating it if needed.
/// Sets restrictive permissions (0o700) so only the bot's user can access downloaded files.
pub fn get_temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(TEMP_DIR_NAME);
    std::fs::create_dir_all(&dir).expect("Failed to create temp directory");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }

    dir
}

/// Spawns a background task that periodically cleans stale temp files
/// to prevent disk exhaustion from orphaned downloads.
pub fn spawn_temp_cleaner() {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_stale_temp_files().await;
        }
    });
}

async fn cleanup_stale_temp_files() {
    let temp_dir = get_temp_dir();
    let mut entries = match fs::read_dir(&temp_dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    let cutoff =
        std::time::SystemTime::now() - std::time::Duration::from_secs(TEMP_FILE_MAX_AGE_SECS);

    while let Ok(Some(entry)) = entries.next_entry().await {
        if let Ok(metadata) = entry.metadata().await {
            let modified = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if modified < cutoff {
                let _ = fs::remove_file(entry.path()).await;
                tracing::info!(path = %entry.path().display(), "Cleaned up stale temp file");
            }
        }
    }
}
