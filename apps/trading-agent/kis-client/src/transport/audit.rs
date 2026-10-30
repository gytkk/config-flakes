use reqwest::Url;
use serde_json::Value;
use std::collections::BTreeMap;

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "appkey" | "appsecret" | "hashkey"
    )
}

fn is_sensitive_query_param(name: &str) -> bool {
    matches!(name.to_ascii_uppercase().as_str(), "CANO" | "ACNT_PRDT_CD")
}

pub(super) fn redacted_headers(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            if is_sensitive_header(name) {
                (name.clone(), "***".to_owned())
            } else {
                (name.clone(), value.clone())
            }
        })
        .collect()
}

pub(super) fn redacted_query(url: &Url) -> BTreeMap<String, String> {
    url.query_pairs()
        .map(|(name, value)| {
            let value = if is_sensitive_query_param(&name) {
                "***".to_owned()
            } else {
                value.into_owned()
            };
            (name.into_owned(), value)
        })
        .collect()
}

pub(super) fn redacted_url(url: &Url) -> String {
    let mut redacted = url.clone();
    redacted.set_query(None);
    if url.query().is_some() {
        let mut pairs = redacted.query_pairs_mut();
        for (name, value) in url.query_pairs() {
            let value = if is_sensitive_query_param(&name) {
                "***".to_owned()
            } else {
                value.into_owned()
            };
            pairs.append_pair(&name, &value);
        }
    }
    redacted.to_string()
}

pub(super) fn redacted_body_summary(body: &Value) -> String {
    match body {
        Value::Object(fields) => format!("<redacted object: {} fields>", fields.len()),
        Value::Null => "<empty>".to_owned(),
        _ => "<redacted>".to_owned(),
    }
}
