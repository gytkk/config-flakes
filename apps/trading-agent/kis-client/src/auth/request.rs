use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

use crate::core::BrokerError;

use super::config::KisAuthConfig;

const TOKEN_PATH: &str = "/oauth2/tokenP";

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct TokenIssueRequestBody {
    pub grant_type: &'static str,
    #[serde(rename = "appkey")]
    pub app_key: String,
    #[serde(rename = "appsecret")]
    pub app_secret: String,
}

impl fmt::Debug for TokenIssueRequestBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenIssueRequestBody")
            .field("grant_type", &self.grant_type)
            .field("app_key", &"***")
            .field("app_secret", &"***")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenIssueRequest {
    method: &'static str,
    url: String,
    headers: BTreeMap<String, String>,
    body: TokenIssueRequestBody,
}

impl TokenIssueRequest {
    pub fn build(config: &KisAuthConfig) -> Result<Self, BrokerError> {
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_owned(), "application/json".to_owned());

        Ok(Self {
            method: "POST",
            url: format!("{}{}", config.base_url, TOKEN_PATH),
            headers,
            body: TokenIssueRequestBody {
                grant_type: "client_credentials",
                app_key: config.credential.app_key().to_owned(),
                app_secret: config.credential.app_secret().to_owned(),
            },
        })
    }

    pub fn method(&self) -> &str {
        self.method
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    pub fn body(&self) -> &TokenIssueRequestBody {
        &self.body
    }
}
