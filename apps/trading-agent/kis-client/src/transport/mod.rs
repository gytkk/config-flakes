use reqwest::Url;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::time::sleep;
use uuid::Uuid;

mod audit;
mod config;
mod endpoint;
mod error;
mod hashkey;
mod rate_limit;
mod request;
mod response;

pub use config::{KisRateLimitConfig, KisTransportConfig};
pub use endpoint::{EndpointGroup, KisEndpoint, KisHttpMethod, TransportFailureClass};
pub use error::KisErrorClassifier;
pub use hashkey::HashKey;
pub use request::{KisPreparedRequest, KisRequestAuditRecord, KisRequestOptions};
pub use response::{KisRawResponse, KisResponseAuditRecord};

use crate::BrokerEnvironment;
use crate::auth::KisTokenProvider;
use crate::core::BrokerError;
use config::{parse_url, validate_timeouts};
use endpoint::HASHKEY_ENDPOINT;
use error::{jittered_backoff, non_json_response_error, reqwest_error, transport_failure_class};
use rate_limit::{KisRateLimiter, SharedRateLimitConfig};

#[derive(Debug, Clone)]
pub struct KisTransport {
    client: reqwest::Client,
    config: KisTransportConfig,
    base_url: Url,
    limiter: Arc<KisRateLimiter>,
    token_provider: Option<Arc<KisTokenProvider>>,
}

impl KisTransport {
    pub fn new(config: KisTransportConfig) -> Result<Self, BrokerError> {
        Self::new_with_optional_token_provider(config, None)
    }

    pub fn new_with_refreshing_token_provider(
        config: KisTransportConfig,
        provider: &KisTokenProvider,
    ) -> Result<Self, BrokerError> {
        Self::new_with_optional_token_provider(config, Some(Arc::new(provider.clone())))
    }

    fn new_with_optional_token_provider(
        config: KisTransportConfig,
        token_provider: Option<Arc<KisTokenProvider>>,
    ) -> Result<Self, BrokerError> {
        validate_timeouts(config.timeout, config.connect_timeout)?;
        let base_url = parse_url(&config.base_url)?;
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .build()
            .map_err(|error| BrokerError::Transport {
                message: format!("failed to build KIS HTTP client: {error}"),
            })?;

        let shared_rate_limit = config.shared_rate_limit_path.as_ref().map(|path| {
            SharedRateLimitConfig::new(
                path.clone(),
                config.environment,
                config.credential.app_key(),
            )
        });

        Ok(Self {
            client,
            limiter: Arc::new(KisRateLimiter::new(config.rate_limits, shared_rate_limit)),
            config,
            base_url,
            token_provider,
        })
    }

    pub async fn new_with_token_provider(provider: &KisTokenProvider) -> Result<Self, BrokerError> {
        let access_token = provider.access_token(false).await?;
        let config = KisTransportConfig::new(
            provider.environment(),
            provider.credential().clone(),
            access_token,
        )?
        .with_base_url(provider.base_url());
        Self::new_with_refreshing_token_provider(config, provider)
    }

    pub fn environment(&self) -> BrokerEnvironment {
        self.config.environment
    }

    pub fn prepare_json(
        &self,
        endpoint: KisEndpoint,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<KisPreparedRequest, BrokerError> {
        self.prepare_json_with_options(endpoint, query, body, KisRequestOptions::default())
    }

    pub fn prepare_json_with_options(
        &self,
        endpoint: KisEndpoint,
        query: &[(&str, &str)],
        body: Option<Value>,
        options: KisRequestOptions<'_>,
    ) -> Result<KisPreparedRequest, BrokerError> {
        if endpoint.requires_hashkey && options.hashkey.is_none() {
            return Err(BrokerError::validation(format!(
                "endpoint {} requires hashkey before request preparation",
                endpoint.name
            )));
        }

        let mut url = self.base_url.join(endpoint.path).map_err(|error| {
            BrokerError::validation(format!(
                "invalid KIS endpoint path {}: {error}",
                endpoint.path
            ))
        })?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (name, value) in query {
                pairs.append_pair(name, value);
            }
        }

        let headers = self.base_headers(endpoint, options);
        Ok(KisPreparedRequest {
            endpoint,
            method: endpoint.method,
            url,
            headers,
            body,
        })
    }

    pub async fn execute_json(
        &self,
        request: KisPreparedRequest,
    ) -> Result<KisRawResponse, BrokerError> {
        let mut attempt = 0;
        let mut retried_after_auth_refresh = false;
        loop {
            self.limiter.wait(request.endpoint.group).await?;
            match self.execute_json_once(&request).await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    if !retried_after_auth_refresh
                        && matches!(error, BrokerError::Auth { .. })
                        && let Some(provider) = &self.token_provider
                    {
                        provider
                            .invalidate("KIS request authentication failed")
                            .await?;
                        retried_after_auth_refresh = true;
                        continue;
                    }
                    if matches!(error, BrokerError::RateLimit { .. }) {
                        self.limiter.record_throttle(request.endpoint.group).await;
                    }
                    if attempt >= self.config.max_retries
                        || !request
                            .endpoint
                            .group
                            .should_retry(transport_failure_class(&error))
                    {
                        return Err(error);
                    }
                    attempt += 1;
                    let retry_backoff = jittered_backoff(self.config.retry_backoff);
                    if !retry_backoff.is_zero() {
                        sleep(retry_backoff).await;
                    }
                }
            }
        }
    }

    pub async fn issue_hashkey(&self, body: &Value) -> Result<HashKey, BrokerError> {
        let response = self
            .execute_json(self.prepare_json(HASHKEY_ENDPOINT, &[], Some(body.clone()))?)
            .await?;
        let hash = response
            .body()
            .get("HASH")
            .and_then(Value::as_str)
            .ok_or_else(|| BrokerError::invariant("KIS hashkey response is missing HASH"))?;
        HashKey::new(hash)
    }

    async fn execute_json_once(
        &self,
        request: &KisPreparedRequest,
    ) -> Result<KisRawResponse, BrokerError> {
        tracing::debug!(audit = ?request.audit_record(), "sending KIS request");
        let mut builder = self
            .client
            .request(request.method.as_reqwest_method(), request.url.clone());
        let headers = self.request_headers(request).await?;
        for (name, value) in &headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(body) = &request.body {
            builder = builder.json(body);
        }

        let response = builder.send().await.map_err(reqwest_error)?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect::<BTreeMap<_, _>>();
        let body_text = response.text().await.map_err(reqwest_error)?;
        let body = serde_json::from_str(&body_text)
            .map_err(|error| non_json_response_error(request.endpoint, status, error))?;

        if let Some(error) = KisErrorClassifier::classify(request.endpoint, status, &headers, &body)
        {
            return Err(error);
        }

        let response = KisRawResponse {
            endpoint: request.endpoint,
            status,
            headers,
            body,
        };
        tracing::debug!(audit = ?response.audit_record(), "received KIS response");
        Ok(response)
    }

    async fn request_headers(
        &self,
        request: &KisPreparedRequest,
    ) -> Result<BTreeMap<String, String>, BrokerError> {
        let mut headers = request.headers.clone();
        if let Some(provider) = &self.token_provider {
            let access_token = provider.access_token(false).await?;
            headers.insert(
                "authorization".to_owned(),
                format!("Bearer {}", access_token.as_str()),
            );
        }
        Ok(headers)
    }

    fn base_headers(
        &self,
        endpoint: KisEndpoint,
        options: KisRequestOptions<'_>,
    ) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_owned(), "application/json".to_owned());
        headers.insert(
            "authorization".to_owned(),
            format!("Bearer {}", self.config.access_token.as_str()),
        );
        headers.insert(
            "appkey".to_owned(),
            self.config.credential.app_key().to_owned(),
        );
        headers.insert(
            "appsecret".to_owned(),
            self.config.credential.app_secret().to_owned(),
        );
        headers.insert("tr_id".to_owned(), endpoint.tr_id.to_owned());
        headers.insert("custtype".to_owned(), "P".to_owned());
        headers.insert("x-request-id".to_owned(), Uuid::new_v4().to_string());
        if let Some(tr_cont) = options.tr_cont {
            headers.insert("tr_cont".to_owned(), tr_cont.to_owned());
        }
        if let Some(hashkey) = options.hashkey {
            headers.insert("hashkey".to_owned(), hashkey.to_owned());
        }
        headers
    }
}
