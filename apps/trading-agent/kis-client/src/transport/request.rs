use reqwest::Url;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

use super::audit::{redacted_body_summary, redacted_headers, redacted_query, redacted_url};
use super::endpoint::{KisEndpoint, KisHttpMethod};

#[derive(Debug, Clone, Copy, Default)]
pub struct KisRequestOptions<'a> {
    pub tr_cont: Option<&'a str>,
    pub hashkey: Option<&'a str>,
}

#[derive(Clone)]
pub struct KisPreparedRequest {
    pub(super) endpoint: KisEndpoint,
    pub(super) method: KisHttpMethod,
    pub(super) url: Url,
    pub(super) headers: BTreeMap<String, String>,
    pub(super) body: Option<Value>,
}

impl KisPreparedRequest {
    pub fn method(&self) -> KisHttpMethod {
        self.method
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    pub fn redacted_headers(&self) -> BTreeMap<String, String> {
        redacted_headers(&self.headers)
    }

    pub fn audit_record(&self) -> KisRequestAuditRecord {
        KisRequestAuditRecord {
            endpoint_name: self.endpoint.name,
            method: self.method,
            path: self.url.path().to_owned(),
            query: redacted_query(&self.url),
            headers: self.redacted_headers(),
            has_body: self.body.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KisRequestAuditRecord {
    pub endpoint_name: &'static str,
    pub method: KisHttpMethod,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub has_body: bool,
}

impl fmt::Debug for KisPreparedRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KisPreparedRequest")
            .field("endpoint", &self.endpoint)
            .field("method", &self.method)
            .field("url", &redacted_url(&self.url))
            .field("headers", &self.redacted_headers())
            .field(
                "body_redacted",
                &self.body.as_ref().map(redacted_body_summary),
            )
            .finish()
    }
}
