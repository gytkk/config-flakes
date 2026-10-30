use std::fmt;
use std::path::PathBuf;

use tokio::time::Duration as TokioDuration;

use crate::core::BrokerError;
use crate::{AccountKey, BrokerEnvironment};

use super::credential::KisCredential;

const LIVE_BASE_URL: &str = "https://openapi.koreainvestment.com:9443";
const PAPER_BASE_URL: &str = "https://openapivts.koreainvestment.com:29443";
const DEFAULT_TOKEN_ISSUE_RETRIES: u8 = 1;
const DEFAULT_TOKEN_REFRESH_LOCK_STALE_AFTER: TokioDuration = TokioDuration::from_secs(30);
const DEFAULT_TOKEN_REFRESH_LOCK_ACQUIRE_TIMEOUT: TokioDuration = TokioDuration::from_secs(10);

#[derive(Clone, PartialEq, Eq)]
pub struct KisAuthConfig {
    pub environment: BrokerEnvironment,
    pub credential: KisCredential,
    pub base_url: String,
    pub account_key: Option<AccountKey>,
    pub cache_path: Option<PathBuf>,
    pub refresh_margin: time::Duration,
    pub max_issue_retries: u8,
    pub issue_retry_backoff: TokioDuration,
    pub token_lock_stale_after: TokioDuration,
    pub token_lock_acquire_timeout: TokioDuration,
}

impl fmt::Debug for KisAuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KisAuthConfig")
            .field("environment", &self.environment)
            .field("credential", &self.credential)
            .field("base_url", &self.base_url)
            .field("account_key", &self.account_key.as_ref().map(|_| "***"))
            .field("cache_path", &self.cache_path)
            .field("refresh_margin", &self.refresh_margin)
            .field("max_issue_retries", &self.max_issue_retries)
            .field("issue_retry_backoff", &self.issue_retry_backoff)
            .field("token_lock_stale_after", &self.token_lock_stale_after)
            .field(
                "token_lock_acquire_timeout",
                &self.token_lock_acquire_timeout,
            )
            .finish()
    }
}

impl KisAuthConfig {
    pub fn new(
        environment: BrokerEnvironment,
        credential: KisCredential,
    ) -> Result<Self, BrokerError> {
        let base_url = match environment {
            BrokerEnvironment::Paper => PAPER_BASE_URL,
            BrokerEnvironment::Live => LIVE_BASE_URL,
            BrokerEnvironment::Mock => {
                return Err(BrokerError::validation(
                    "mock environment does not issue KIS access tokens",
                ));
            }
        };

        Ok(Self {
            environment,
            credential,
            base_url: base_url.to_owned(),
            account_key: None,
            cache_path: None,
            refresh_margin: time::Duration::minutes(5),
            max_issue_retries: DEFAULT_TOKEN_ISSUE_RETRIES,
            issue_retry_backoff: TokioDuration::from_millis(100),
            token_lock_stale_after: DEFAULT_TOKEN_REFRESH_LOCK_STALE_AFTER,
            token_lock_acquire_timeout: DEFAULT_TOKEN_REFRESH_LOCK_ACQUIRE_TIMEOUT,
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_cache_path(mut self, cache_path: impl Into<PathBuf>) -> Self {
        self.cache_path = Some(cache_path.into());
        self
    }

    pub fn with_account_key(mut self, account_key: AccountKey) -> Self {
        self.account_key = Some(account_key);
        self
    }

    pub fn with_refresh_margin(mut self, refresh_margin: time::Duration) -> Self {
        self.refresh_margin = refresh_margin;
        self
    }

    pub fn with_issue_retry_policy(
        mut self,
        max_issue_retries: u8,
        issue_retry_backoff: TokioDuration,
    ) -> Self {
        self.max_issue_retries = max_issue_retries;
        self.issue_retry_backoff = issue_retry_backoff;
        self
    }

    pub fn with_token_lock_policy(
        mut self,
        stale_after: TokioDuration,
        acquire_timeout: TokioDuration,
    ) -> Result<Self, BrokerError> {
        if acquire_timeout.is_zero() {
            return Err(BrokerError::validation(
                "KIS token refresh lock acquire timeout must be positive",
            ));
        }
        self.token_lock_stale_after = stale_after;
        self.token_lock_acquire_timeout = acquire_timeout;
        Ok(self)
    }
}
