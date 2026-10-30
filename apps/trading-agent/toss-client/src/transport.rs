use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::{Method, Url};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use trading_agent_kis_client::BrokerError;

use crate::auth::{TossAccessToken, TossCredential};
use crate::client::TossAccountSeq;

const DEFAULT_TOSS_BASE_URL: &str = "https://openapi.tossinvest.com";

#[derive(Debug, Clone)]
pub struct TossTransportConfig {
    pub credential: TossCredential,
    pub access_token: TossAccessToken,
    pub base_url: String,
    pub timeout: Duration,
    pub connect_timeout: Duration,
}

impl TossTransportConfig {
    pub fn new(
        credential: TossCredential,
        access_token: TossAccessToken,
    ) -> Result<Self, BrokerError> {
        Ok(Self {
            credential,
            access_token,
            base_url: DEFAULT_TOSS_BASE_URL.to_owned(),
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(5),
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

#[derive(Debug, Clone)]
pub struct TossTransport {
    client: reqwest::Client,
    base_url: Url,
    access_token: TossAccessToken,
}

impl TossTransport {
    pub fn new(config: TossTransportConfig) -> Result<Self, BrokerError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .connect_timeout(config.connect_timeout)
            .build()
            .map_err(|error| BrokerError::Transport {
                message: format!("failed to build Toss HTTP client: {error}"),
            })?;
        Ok(Self {
            client,
            base_url: parse_base_url(&config.base_url)?,
            access_token: config.access_token,
        })
    }

    pub async fn get<T>(
        &self,
        endpoint: TossEndpoint,
        query: &[(&str, String)],
        account_seq: Option<TossAccountSeq>,
    ) -> Result<T, BrokerError>
    where
        T: DeserializeOwned,
    {
        self.get_path(endpoint, endpoint.path, query, account_seq)
            .await
    }

    pub async fn get_path<T>(
        &self,
        endpoint: TossEndpoint,
        path: &str,
        query: &[(&str, String)],
        account_seq: Option<TossAccountSeq>,
    ) -> Result<T, BrokerError>
    where
        T: DeserializeOwned,
    {
        let mut url = self.base_url.join(path).map_err(|error| {
            BrokerError::validation(format!("invalid Toss endpoint path {}: {error}", path))
        })?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (name, value) in query {
                pairs.append_pair(name, value);
            }
        }
        let mut builder = self
            .client
            .request(endpoint.method.as_reqwest_method(), url);
        builder = builder.header(
            "authorization",
            format!("Bearer {}", self.access_token.as_str()),
        );
        if let Some(account_seq) = account_seq {
            builder = builder.header("x-tossinvest-account", account_seq.as_u64().to_string());
        }
        let response = builder.send().await.map_err(reqwest_error)?;
        let status = response.status().as_u16();
        let headers = response_headers(response.headers());
        let body_text = response.text().await.map_err(reqwest_error)?;
        if status >= 400 {
            return Err(classify_api_error_response(
                endpoint, status, &headers, &body_text,
            ));
        }
        let envelope: ApiEnvelope<T> =
            serde_json::from_str(&body_text).map_err(|error| BrokerError::Transport {
                message: format!(
                    "Toss endpoint {} response was not valid JSON: {error}",
                    endpoint.name
                ),
            })?;
        Ok(envelope.result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TossEndpoint {
    pub name: &'static str,
    pub method: TossHttpMethod,
    pub path: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossHttpMethod {
    Get,
}

impl TossHttpMethod {
    fn as_reqwest_method(self) -> Method {
        match self {
            Self::Get => Method::GET,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    result: T,
}

#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(rename = "requestId")]
    request_id: String,
    code: String,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct OAuth2Error {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

pub fn parse_base_url(value: &str) -> Result<Url, BrokerError> {
    let url = Url::parse(value).map_err(|error| {
        BrokerError::validation(format!("invalid Toss base URL {value:?}: {error}"))
    })?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        scheme => Err(BrokerError::validation(format!(
            "Toss base URL must use http or https, got {scheme}"
        ))),
    }
}

pub fn reqwest_error(error: reqwest::Error) -> BrokerError {
    if error.is_timeout() {
        return BrokerError::Temporary {
            message: format!("Toss request timed out: {error}"),
        };
    }
    BrokerError::Transport {
        message: format!("Toss transport error: {error}"),
    }
}

pub fn classify_oauth_error_response(status: u16, body_text: &str) -> BrokerError {
    let parsed = serde_json::from_str::<OAuth2Error>(body_text).ok();
    let message = parsed
        .as_ref()
        .map(|error| {
            let description = error.error_description.as_deref().unwrap_or("");
            format!("{} {}", error.error, description).trim().to_owned()
        })
        .unwrap_or_else(|| format!("status {status}: non-JSON OAuth2 error response"));
    match status {
        400 => BrokerError::validation(format!("Toss OAuth2 error: {message}")),
        401 | 403 => BrokerError::Auth {
            message: format!("Toss OAuth2 error: {message}"),
        },
        429 => BrokerError::RateLimit {
            message: format!("Toss OAuth2 error: {message}"),
        },
        500..=599 => BrokerError::Temporary {
            message: format!("Toss OAuth2 error: {message}"),
        },
        _ => BrokerError::Transport {
            message: format!("Toss OAuth2 error: {message}"),
        },
    }
}

fn classify_api_error_response(
    endpoint: TossEndpoint,
    status: u16,
    headers: &BTreeMap<String, String>,
    body_text: &str,
) -> BrokerError {
    let parsed = serde_json::from_str::<ApiErrorEnvelope>(body_text).ok();
    let request_id = parsed
        .as_ref()
        .map(|envelope| envelope.error.request_id.as_str())
        .or_else(|| headers.get("x-request-id").map(String::as_str))
        .unwrap_or("unknown");
    let detail = parsed
        .as_ref()
        .map(|envelope| {
            let data = envelope
                .error
                .data
                .as_ref()
                .map(|data| format!(" data={data}"))
                .unwrap_or_default();
            format!(
                "{}: {} request_id={}{}",
                envelope.error.code, envelope.error.message, request_id, data
            )
        })
        .unwrap_or_else(|| {
            format!("status {status}: non-JSON error response request_id={request_id}")
        });
    let message = format!("Toss endpoint {} failed: {detail}", endpoint.name);
    match status {
        400 | 404 | 422 => BrokerError::validation(message),
        401 => BrokerError::Auth { message },
        403 => BrokerError::Permission { message },
        409 => BrokerError::Temporary { message },
        429 => BrokerError::RateLimit { message },
        500..=599 => BrokerError::Temporary { message },
        _ => BrokerError::Transport { message },
    }
}

fn response_headers(headers: &reqwest::header::HeaderMap) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_owned()))
        })
        .collect()
}
