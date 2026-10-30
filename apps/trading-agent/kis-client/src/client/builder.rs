use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::core::journal::BrokerCommandJournal;
use crate::{AccountKey, BrokerEnvironment};
use time::OffsetDateTime;

use crate::auth::{KisAuthConfig, KisCredential, KisTokenProvider};
use crate::config::BrokerRuntimeConfig;
use crate::core::BrokerError;
use crate::safety::SingleWriterLock;
use crate::transport::{KisRateLimitConfig, KisTransport, KisTransportConfig};

use super::KisBrokerClient;

#[derive(Clone)]
pub struct KisClientBuilder {
    environment: BrokerEnvironment,
    credential: KisCredential,
    base_url: Option<String>,
    account_key: Option<AccountKey>,
    token_cache_path: Option<PathBuf>,
    runtime_config: Option<BrokerRuntimeConfig>,
    journal: Option<Arc<dyn BrokerCommandJournal>>,
    single_writer_lock: Option<Arc<SingleWriterLock>>,
    market_session_now: Option<OffsetDateTime>,
    rate_limits: Option<KisRateLimitConfig>,
    shared_rate_limit_path: Option<PathBuf>,
    retry_policy: Option<(u8, Duration)>,
}

impl KisClientBuilder {
    pub fn new(environment: BrokerEnvironment, credential: KisCredential) -> Self {
        Self {
            environment,
            credential,
            base_url: None,
            account_key: None,
            token_cache_path: None,
            runtime_config: None,
            journal: None,
            single_writer_lock: None,
            market_session_now: None,
            rate_limits: None,
            shared_rate_limit_path: None,
            retry_policy: None,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    pub fn with_account_key(mut self, account_key: AccountKey) -> Self {
        self.account_key = Some(account_key);
        self
    }

    pub fn with_token_cache_path(mut self, token_cache_path: impl Into<PathBuf>) -> Self {
        self.token_cache_path = Some(token_cache_path.into());
        self
    }

    pub fn with_runtime_config(mut self, runtime_config: BrokerRuntimeConfig) -> Self {
        self.runtime_config = Some(runtime_config);
        self
    }

    pub fn with_journal(mut self, journal: Arc<dyn BrokerCommandJournal>) -> Self {
        self.journal = Some(journal);
        self
    }

    pub fn with_single_writer_lock(mut self, single_writer_lock: Arc<SingleWriterLock>) -> Self {
        self.single_writer_lock = Some(single_writer_lock);
        self
    }

    pub fn with_market_session_now(mut self, now_utc: OffsetDateTime) -> Self {
        self.market_session_now = Some(now_utc);
        self
    }

    pub fn with_rate_limits(mut self, rate_limits: KisRateLimitConfig) -> Self {
        self.rate_limits = Some(rate_limits);
        self
    }

    pub fn with_shared_rate_limit_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.shared_rate_limit_path = Some(path.into());
        self
    }

    pub fn with_retry_policy(mut self, max_retries: u8, retry_backoff: Duration) -> Self {
        self.retry_policy = Some((max_retries, retry_backoff));
        self
    }

    pub async fn build(self) -> Result<KisBrokerClient, BrokerError> {
        let token_provider = KisTokenProvider::new(self.auth_config()?)?;
        let access_token = token_provider.access_token(false).await?;

        let mut transport_config =
            KisTransportConfig::new(self.environment, self.credential, access_token)?;
        if let Some(base_url) = self.base_url {
            transport_config = transport_config.with_base_url(base_url);
        }
        if let Some(rate_limits) = self.rate_limits {
            transport_config = transport_config.with_rate_limits(rate_limits);
        }
        if let Some(shared_rate_limit_path) = self.shared_rate_limit_path {
            transport_config = transport_config.with_shared_rate_limit_path(shared_rate_limit_path);
        }
        if let Some((max_retries, retry_backoff)) = self.retry_policy {
            transport_config = transport_config.with_retry_policy(max_retries, retry_backoff);
        }

        let transport =
            KisTransport::new_with_refreshing_token_provider(transport_config, &token_provider)?;
        let mut client = match self.account_key {
            Some(account_key) => KisBrokerClient::new_for_account(transport, account_key),
            None => KisBrokerClient::new(transport),
        };

        if let Some(runtime_config) = self.runtime_config {
            client = client.with_runtime_config(runtime_config);
        }
        if let Some(journal) = self.journal {
            client = client.with_journal(journal);
        }
        if let Some(single_writer_lock) = self.single_writer_lock {
            client = client.with_single_writer_lock(single_writer_lock);
        }
        if let Some(now_utc) = self.market_session_now {
            client = client.with_market_session_now(now_utc);
        }

        Ok(client)
    }

    fn auth_config(&self) -> Result<KisAuthConfig, BrokerError> {
        let mut auth_config = KisAuthConfig::new(self.environment, self.credential.clone())?;
        if let Some(base_url) = &self.base_url {
            auth_config = auth_config.with_base_url(base_url.clone());
        }
        if let Some(token_cache_path) = &self.token_cache_path {
            auth_config = auth_config.with_cache_path(token_cache_path.clone());
        }
        if let Some(account_key) = &self.account_key {
            auth_config = auth_config.with_account_key(account_key.clone());
        }

        Ok(auth_config)
    }
}
