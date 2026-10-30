use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use tokio::time::{Duration as TokioDuration, Instant, sleep};

use crate::core::BrokerError;

const TOKEN_REFRESH_LOCK_RETRY_SLEEP: TokioDuration = TokioDuration::from_millis(25);

pub(super) fn token_refresh_lock_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kis-token-cache");
    path.with_file_name(format!("{file_name}.lock"))
}

pub(super) struct TokenRefreshLockGuard {
    path: PathBuf,
}

impl TokenRefreshLockGuard {
    pub(super) async fn acquire(
        path: &Path,
        stale_after: TokioDuration,
        acquire_timeout: TokioDuration,
    ) -> Result<Self, BrokerError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                BrokerError::persistence(format!(
                    "failed to create KIS token lock directory: {error}"
                ))
            })?;
        }

        let started_at = Instant::now();
        loop {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                options.mode(0o600);
            }
            match options.open(path) {
                Ok(mut file) => {
                    file.write_all(std::process::id().to_string().as_bytes())
                        .map_err(|error| {
                            BrokerError::persistence(format!(
                                "failed to write KIS token refresh lock: {error}"
                            ))
                        })?;
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if token_refresh_lock_is_stale(path, stale_after)? {
                        match fs::remove_file(path) {
                            Ok(()) => continue,
                            Err(error) if error.kind() == ErrorKind::NotFound => continue,
                            Err(error) => {
                                return Err(BrokerError::persistence(format!(
                                    "failed to remove stale KIS token refresh lock: {error}"
                                )));
                            }
                        }
                    }

                    let elapsed = started_at.elapsed();
                    if elapsed >= acquire_timeout {
                        return Err(BrokerError::persistence(format!(
                            "timed out acquiring KIS token refresh lock at {}",
                            path.display()
                        )));
                    }
                    sleep(std::cmp::min(
                        TOKEN_REFRESH_LOCK_RETRY_SLEEP,
                        acquire_timeout - elapsed,
                    ))
                    .await;
                }
                Err(error) => {
                    return Err(BrokerError::persistence(format!(
                        "failed to acquire KIS token refresh lock: {error}"
                    )));
                }
            }
        }
    }
}

fn token_refresh_lock_is_stale(
    path: &Path,
    stale_after: TokioDuration,
) -> Result<bool, BrokerError> {
    if stale_after.is_zero() {
        return Ok(true);
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(BrokerError::persistence(format!(
                "failed to inspect KIS token refresh lock: {error}"
            )));
        }
    };
    let modified_at = metadata.modified().map_err(|error| {
        BrokerError::persistence(format!(
            "failed to read KIS token refresh lock modified time: {error}"
        ))
    })?;

    match modified_at.elapsed() {
        Ok(age) => Ok(age >= stale_after),
        Err(_) => Ok(false),
    }
}

impl Drop for TokenRefreshLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
