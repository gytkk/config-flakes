use serde_json::Value;
use std::collections::BTreeMap;

use super::audit::redacted_headers;
use super::endpoint::KisEndpoint;

#[derive(Debug, Clone)]
pub struct KisRawResponse {
    pub(super) endpoint: KisEndpoint,
    pub(super) status: u16,
    pub(super) headers: BTreeMap<String, String>,
    pub(super) body: Value,
}

impl KisRawResponse {
    pub fn endpoint(&self) -> KisEndpoint {
        self.endpoint
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn headers(&self) -> &BTreeMap<String, String> {
        &self.headers
    }

    pub fn tr_cont(&self) -> Option<&str> {
        self.headers.get("tr_cont").map(String::as_str)
    }

    pub fn body(&self) -> &Value {
        &self.body
    }

    pub fn audit_record(&self) -> KisResponseAuditRecord {
        KisResponseAuditRecord {
            endpoint_name: self.endpoint.name,
            status: self.status,
            headers: redacted_headers(&self.headers),
            has_body: !self.body.is_null(),
            kis_rt_cd: self
                .body
                .get("rt_cd")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            kis_msg_cd: self
                .body
                .get("msg_cd")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KisResponseAuditRecord {
    pub endpoint_name: &'static str,
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub has_body: bool,
    pub kis_rt_cd: Option<String>,
    pub kis_msg_cd: Option<String>,
}
