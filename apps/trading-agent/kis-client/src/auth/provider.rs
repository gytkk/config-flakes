use std::fs;
use std::io::ErrorKind;
use std::sync::Arc;

use time::OffsetDateTime;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::BrokerEnvironment;
use crate::core::BrokerError;

use super::cache::{read_file_cache, write_file_cache};
use super::config::KisAuthConfig;
use super::credential::KisCredential;
use super::lock::{TokenRefreshLockGuard, token_refresh_lock_path};
use super::request::TokenIssueRequest;
use super::token::{AccessToken, KisTokenResponse, TokenSnapshot};

#[derive(Debug, Clone)]
pub struct KisTokenProvider {
    client: reqwest::Client,
    config: KisAuthConfig,
    cached: Arc<Mutex<Option<TokenSnapshot>>>,
}

impl KisTokenProvider {
    pub fn new(config: KisAuthConfig) -> Result<Self, BrokerError> {
        if config.environment == BrokerEnvironment::Live && config.cache_path.is_none() {
            return Err(BrokerError::safety_halt(
                "live token provider requires file cache",
            ));
        }

        Ok(Self {
            client: reqwest::Client::new(),
            config,
            cached: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn access_token(&self, force_refresh: bool) -> Result<AccessToken, BrokerError> {
        let mut cached = self.cached.lock().await;
        let now = OffsetDateTime::now_utc();

        if !force_refresh
            && let Some(snapshot) = cached.as_ref()
            && snapshot.is_usable_at(now, self.config.refresh_margin)
        {
            return Ok(snapshot.access_token.clone());
        }

        if !force_refresh
            && let Some(snapshot) = read_file_cache(&self.config)?
            && snapshot.is_usable_at(now, self.config.refresh_margin)
        {
            let access_token = snapshot.access_token.clone();
            *cached = Some(snapshot);
            return Ok(access_token);
        }

        let snapshot = self.refresh_snapshot_with_file_lock(now).await?;
        let access_token = snapshot.access_token.clone();
        *cached = Some(snapshot);
        Ok(access_token)
    }

    pub fn environment(&self) -> BrokerEnvironment {
        self.config.environment
    }

    pub fn credential(&self) -> &KisCredential {
        &self.config.credential
    }

    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    pub async fn invalidate(&self, _reason: &str) -> Result<(), BrokerError> {
        *self.cached.lock().await = None;
        if let Some(cache_path) = &self.config.cache_path {
            match fs::remove_file(cache_path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(BrokerError::persistence(format!(
                        "failed to remove KIS token cache: {error}"
                    )));
                }
            }
        }
        Ok(())
    }

    pub async fn snapshot(&self) -> Result<Option<TokenSnapshot>, BrokerError> {
        if let Some(snapshot) = self.cached.lock().await.clone() {
            return Ok(Some(snapshot));
        }
        read_file_cache(&self.config)
    }

    async fn refresh_snapshot_with_file_lock(
        &self,
        now: OffsetDateTime,
    ) -> Result<TokenSnapshot, BrokerError> {
        let Some(cache_path) = &self.config.cache_path else {
            return self.issue_token().await;
        };

        let lock_path = token_refresh_lock_path(cache_path);
        let _guard = TokenRefreshLockGuard::acquire(
            &lock_path,
            self.config.token_lock_stale_after,
            self.config.token_lock_acquire_timeout,
        )
        .await?;
        if let Some(snapshot) = read_file_cache(&self.config)?
            && snapshot.is_usable_at(now, self.config.refresh_margin)
        {
            return Ok(snapshot);
        }

        let snapshot = self.issue_token().await?;
        write_file_cache(&self.config, &snapshot)?;
        Ok(snapshot)
    }

    async fn issue_token(&self) -> Result<TokenSnapshot, BrokerError> {
        let mut attempt = 0;
        loop {
            match self.issue_token_once().await {
                Ok(snapshot) => return Ok(snapshot),
                Err(error)
                    if attempt < self.config.max_issue_retries
                        && is_retryable_token_issue_error(&error) =>
                {
                    attempt += 1;
                    if !self.config.issue_retry_backoff.is_zero() {
                        sleep(self.config.issue_retry_backoff).await;
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn issue_token_once(&self) -> Result<TokenSnapshot, BrokerError> {
        let request = TokenIssueRequest::build(&self.config)?;
        let mut builder = self.client.post(request.url());
        for (name, value) in request.headers() {
            builder = builder.header(name.as_str(), value.as_str());
        }

        let response = builder
            .json(request.body())
            .send()
            .await
            .map_err(auth_transport_error)?;
        let status = response.status().as_u16();
        let body_text = response.text().await.map_err(auth_transport_error)?;

        if status == 401 || status == 403 {
            return Err(BrokerError::Auth {
                message: "KIS token issue rejected credentials".to_owned(),
            });
        }
        if status == 429 {
            return Err(BrokerError::RateLimit {
                message: "KIS token issue was rate limited".to_owned(),
            });
        }
        if (500..=599).contains(&status) {
            return Err(BrokerError::Temporary {
                message: format!("temporary KIS token issue failure: status {status}"),
            });
        }
        if !(200..=299).contains(&status) {
            return Err(BrokerError::Transport {
                message: format!("unexpected KIS token issue status: {status}"),
            });
        }

        let response: KisTokenResponse =
            serde_json::from_str(&body_text).map_err(|error| BrokerError::Transport {
                message: format!("KIS token response body is not valid JSON: {error}"),
            })?;
        response.to_snapshot()
    }
}

fn is_retryable_token_issue_error(error: &BrokerError) -> bool {
    matches!(
        error,
        BrokerError::Temporary { .. } | BrokerError::RateLimit { .. }
    )
}

fn auth_transport_error(error: reqwest::Error) -> BrokerError {
    if error.is_timeout() {
        BrokerError::Temporary {
            message: format!("KIS token issue timeout: {error}"),
        }
    } else if error.is_connect() {
        BrokerError::Temporary {
            message: format!("KIS token issue connection failed: {error}"),
        }
    } else {
        BrokerError::Transport {
            message: error.to_string(),
        }
    }
}
