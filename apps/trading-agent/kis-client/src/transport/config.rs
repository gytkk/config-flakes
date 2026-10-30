use reqwest::Url;
use std::path::PathBuf;
use std::time::Duration;

use crate::BrokerEnvironment;
use crate::auth::{AccessToken, KisCredential};
use crate::core::BrokerError;

use super::endpoint::EndpointGroup;

const LIVE_BASE_URL: &str = "https://openapi.koreainvestment.com:9443";
const PAPER_BASE_URL: &str = "https://openapivts.koreainvestment.com:29443";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct KisTransportConfig {
    pub environment: BrokerEnvironment,
    pub credential: KisCredential,
    pub access_token: AccessToken,
    pub base_url: String,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub rate_limits: KisRateLimitConfig,
    pub shared_rate_limit_path: Option<PathBuf>,
    pub max_retries: u8,
    pub retry_backoff: Duration,
}

impl KisTransportConfig {
    pub fn new(
        environment: BrokerEnvironment,
        credential: KisCredential,
        access_token: AccessToken,
    ) -> Result<Self, BrokerError> {
        let base_url = match environment {
            BrokerEnvironment::Paper => PAPER_BASE_URL,
            BrokerEnvironment::Live => LIVE_BASE_URL,
            BrokerEnvironment::Mock => {
                return Err(BrokerError::validation(
                    "mock environment does not use KIS HTTP transport",
                ));
            }
        };

        Ok(Self {
            environment,
            credential,
            access_token,
            base_url: base_url.to_owned(),
            timeout: DEFAULT_TIMEOUT,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            rate_limits: KisRateLimitConfig::default(),
            shared_rate_limit_path: None,
            max_retries: 1,
            retry_backoff: Duration::from_millis(100),
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_rate_limits(mut self, rate_limits: KisRateLimitConfig) -> Self {
        self.rate_limits = rate_limits;
        self
    }

    pub fn with_shared_rate_limit_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.shared_rate_limit_path = Some(path.into());
        self
    }

    pub fn with_retry_policy(mut self, max_retries: u8, retry_backoff: Duration) -> Self {
        self.max_retries = max_retries;
        self.retry_backoff = retry_backoff;
        self
    }

    pub fn try_with_timeouts(
        mut self,
        timeout: Duration,
        connect_timeout: Duration,
    ) -> Result<Self, BrokerError> {
        validate_timeouts(timeout, connect_timeout)?;
        self.timeout = timeout;
        self.connect_timeout = connect_timeout;
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KisRateLimitConfig {
    pub global_min_interval: Duration,
    pub auth_min_interval: Duration,
    pub read_light_min_interval: Duration,
    pub read_heavy_min_interval: Duration,
    pub write_order_min_interval: Duration,
    pub throttle_circuit_open_for: Duration,
}

impl KisRateLimitConfig {
    pub(super) fn min_interval(self, group: EndpointGroup) -> Duration {
        match group {
            EndpointGroup::Auth => self.auth_min_interval,
            EndpointGroup::ReadLight => self.read_light_min_interval,
            EndpointGroup::ReadHeavy => self.read_heavy_min_interval,
            EndpointGroup::WriteOrder => self.write_order_min_interval,
        }
    }
}

pub(super) fn parse_url(value: &str) -> Result<Url, BrokerError> {
    Url::parse(value).map_err(|error| BrokerError::validation(format!("invalid base URL: {error}")))
}

pub(super) fn validate_timeouts(
    timeout: Duration,
    connect_timeout: Duration,
) -> Result<(), BrokerError> {
    if timeout.is_zero() {
        return Err(BrokerError::validation("KIS HTTP timeout must be positive"));
    }
    if connect_timeout.is_zero() {
        return Err(BrokerError::validation(
            "KIS HTTP connect timeout must be positive",
        ));
    }
    Ok(())
}
