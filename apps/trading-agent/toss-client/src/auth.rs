use std::fmt;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use trading_agent_kis_client::BrokerError;

use crate::transport::{classify_oauth_error_response, parse_base_url, reqwest_error};

#[derive(Clone, PartialEq, Eq)]
pub struct TossCredential {
    client_id: String,
    client_secret: String,
}

impl TossCredential {
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Result<Self, BrokerError> {
        let client_id = client_id.into();
        let client_secret = client_secret.into();
        validate_secret_part("Toss client_id", &client_id)?;
        validate_secret_part("Toss client_secret", &client_secret)?;
        Ok(Self {
            client_id,
            client_secret,
        })
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn client_secret(&self) -> &str {
        &self.client_secret
    }
}

impl fmt::Debug for TossCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TossCredential")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TossAccessToken(String);

impl TossAccessToken {
    pub fn new(value: impl Into<String>) -> Result<Self, BrokerError> {
        let value = value.into();
        validate_secret_part("Toss access token", &value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for TossAccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TossAccessToken(***)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossTokenType(String);

impl TossTokenType {
    fn new(value: impl Into<String>) -> Result<Self, BrokerError> {
        let value = value.into();
        if value != "Bearer" {
            return Err(BrokerError::invariant(format!(
                "Toss OAuth2 token_type must be Bearer, got {value}"
            )));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossIssuedToken {
    pub access_token: TossAccessToken,
    pub token_type: TossTokenType,
    pub expires_in_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct TossAuthClient {
    client: reqwest::Client,
    base_url: Url,
    credential: TossCredential,
}

impl TossAuthClient {
    pub fn new(base_url: impl AsRef<str>, credential: TossCredential) -> Result<Self, BrokerError> {
        let client =
            reqwest::Client::builder()
                .build()
                .map_err(|error| BrokerError::Transport {
                    message: format!("failed to build Toss auth HTTP client: {error}"),
                })?;
        Ok(Self {
            client,
            base_url: parse_base_url(base_url.as_ref())?,
            credential,
        })
    }

    pub async fn issue_token(&self) -> Result<TossIssuedToken, BrokerError> {
        let url = self.base_url.join("/oauth2/token").map_err(|error| {
            BrokerError::validation(format!("invalid Toss token endpoint path: {error}"))
        })?;
        let request = OAuth2TokenRequest {
            grant_type: "client_credentials",
            client_id: self.credential.client_id(),
            client_secret: self.credential.client_secret(),
        };
        let response = self
            .client
            .post(url)
            .form(&request)
            .send()
            .await
            .map_err(reqwest_error)?;
        let status = response.status().as_u16();
        let body_text = response.text().await.map_err(reqwest_error)?;
        if status >= 400 {
            return Err(classify_oauth_error_response(status, &body_text));
        }
        let body: OAuth2TokenResponse =
            serde_json::from_str(&body_text).map_err(|error| BrokerError::Transport {
                message: format!("Toss OAuth2 token response was not valid JSON: {error}"),
            })?;
        Ok(TossIssuedToken {
            access_token: TossAccessToken::new(body.access_token)?,
            token_type: TossTokenType::new(body.token_type)?,
            expires_in_seconds: body.expires_in,
        })
    }
}

#[derive(Debug, Serialize)]
struct OAuth2TokenRequest<'a> {
    grant_type: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
}

#[derive(Debug, Deserialize)]
struct OAuth2TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: i64,
}

fn validate_secret_part(name: &str, value: &str) -> Result<(), BrokerError> {
    if value.trim().is_empty() {
        return Err(BrokerError::validation(format!("{name} must not be empty")));
    }
    if value.chars().any(char::is_whitespace) {
        return Err(BrokerError::validation(format!(
            "{name} must not contain whitespace"
        )));
    }
    Ok(())
}
