use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;
use uuid::Uuid;

use crate::core::BrokerError;

use super::endpoint::{EndpointGroup, KisEndpoint, TransportFailureClass};

pub(super) fn transport_failure_class(error: &BrokerError) -> TransportFailureClass {
    match error {
        BrokerError::Validation { .. } => TransportFailureClass::Validation,
        BrokerError::Temporary { .. } => TransportFailureClass::Http5xx,
        BrokerError::RateLimit { .. } => TransportFailureClass::RateLimit,
        _ => TransportFailureClass::Validation,
    }
}

pub struct KisErrorClassifier;

impl KisErrorClassifier {
    pub fn classify(
        endpoint: KisEndpoint,
        status: u16,
        _headers: &BTreeMap<String, String>,
        body: &Value,
    ) -> Option<BrokerError> {
        let msg_cd = body.get("msg_cd").and_then(Value::as_str).unwrap_or("");
        let msg1 = body.get("msg1").and_then(Value::as_str).unwrap_or("");
        if is_throttle_error(msg_cd, msg1) {
            return Some(BrokerError::RateLimit {
                message: kis_error_message(endpoint, status, body, "KIS rate limit"),
            });
        }

        if (500..=599).contains(&status) {
            return Some(BrokerError::Temporary {
                message: kis_error_message(endpoint, status, body, "temporary KIS HTTP error"),
            });
        }
        if status == 429 {
            return Some(BrokerError::RateLimit {
                message: kis_error_message(endpoint, status, body, "KIS HTTP rate limit"),
            });
        }
        if status == 401 {
            return Some(BrokerError::Auth {
                message: kis_error_message(endpoint, status, body, "KIS authentication failed"),
            });
        }
        if status == 403 {
            return Some(BrokerError::Permission {
                message: kis_error_message(endpoint, status, body, "KIS permission denied"),
            });
        }
        if !(200..=299).contains(&status) {
            return Some(BrokerError::Transport {
                message: kis_error_message(endpoint, status, body, "unexpected KIS HTTP status"),
            });
        }

        let rt_cd = body.get("rt_cd").and_then(Value::as_str).unwrap_or("");
        if rt_cd.trim().is_empty() || rt_cd == "0" {
            return None;
        }

        let message = format!("KIS returned error {msg_cd}: {msg1}");
        if is_auth_error(msg_cd, msg1) {
            return Some(BrokerError::Auth { message });
        }
        if is_permission_error(msg_cd, msg1) {
            return Some(BrokerError::Permission { message });
        }
        if is_market_closed_error(msg1) {
            return Some(BrokerError::MarketClosed { message });
        }
        if is_transient_read_error(endpoint.group, msg_cd, msg1) {
            return Some(BrokerError::Temporary { message });
        }
        if endpoint.group == EndpointGroup::WriteOrder {
            return Some(BrokerError::OrderRejected { message });
        }

        Some(BrokerError::Transport { message })
    }
}

pub(super) fn non_json_response_error(
    endpoint: KisEndpoint,
    status: u16,
    error: serde_json::Error,
) -> BrokerError {
    if (500..=599).contains(&status) {
        return BrokerError::Temporary {
            message: format!(
                "temporary KIS HTTP error for {}: status {status}, non-JSON response body: {error}",
                endpoint.name
            ),
        };
    }

    BrokerError::Transport {
        message: format!(
            "KIS response body is not valid JSON for {}: status {status}: {error}",
            endpoint.name
        ),
    }
}

fn kis_error_message(endpoint: KisEndpoint, status: u16, body: &Value, fallback: &str) -> String {
    let msg_cd = body.get("msg_cd").and_then(Value::as_str).unwrap_or("");
    let msg1 = body.get("msg1").and_then(Value::as_str).unwrap_or("");
    if msg_cd.is_empty() && msg1.is_empty() {
        format!("{fallback} for {}: status {status}", endpoint.name)
    } else {
        format!(
            "{fallback} for {}: status {status}, {msg_cd}: {msg1}",
            endpoint.name
        )
    }
}

fn is_throttle_error(msg_cd: &str, msg1: &str) -> bool {
    let msg_cd = msg_cd.to_ascii_uppercase();
    let msg1 = msg1.to_ascii_lowercase();
    msg_cd.contains("429")
        || msg_cd == "EGW00201"
        || msg_cd.contains("THROTTLE")
        || msg1.contains("rate limit")
        || msg1.contains("too many")
        || msg1.contains("throttle")
        || (msg1.contains("거래건수") && msg1.contains("초과"))
        || (msg1.contains("호출") && msg1.contains("초과"))
}

fn is_auth_error(msg_cd: &str, msg1: &str) -> bool {
    let msg_cd = msg_cd.to_ascii_uppercase();
    let msg1 = msg1.to_ascii_lowercase();
    msg_cd.contains("401")
        || msg1.contains("unauthorized")
        || msg1.contains("authentication")
        || msg1.contains("인증")
}

fn is_permission_error(msg_cd: &str, msg1: &str) -> bool {
    let msg_cd = msg_cd.to_ascii_uppercase();
    let msg1 = msg1.to_ascii_lowercase();
    msg_cd.contains("403") || msg1.contains("permission") || msg1.contains("권한")
}

fn is_market_closed_error(msg1: &str) -> bool {
    let msg1 = msg1.to_ascii_lowercase();
    msg1.contains("market closed")
        || msg1.contains("market is closed")
        || msg1.contains("장종료")
        || msg1.contains("장 종료")
}

fn is_transient_read_error(group: EndpointGroup, msg_cd: &str, msg1: &str) -> bool {
    if !matches!(group, EndpointGroup::ReadLight | EndpointGroup::ReadHeavy) {
        return false;
    }

    let msg_cd = msg_cd.trim().to_ascii_uppercase();
    msg_cd == "APBK1350"
        || msg_cd == "OPSQ0008"
        || msg_cd == "OPSQ0009"
        || msg_cd == "OPSQ0010"
        || msg_cd == "OPSQ0011"
        || msg1.contains("다시 조회")
        || msg1.contains("호출 후처리")
        || msg1.contains("호출 결과처리")
        || msg1.contains("리소스 부족")
        || msg1.contains("MCI전송")
        || msg1.contains("MCI수신")
}

pub(super) fn jittered_backoff(base: Duration) -> Duration {
    if base.is_zero() {
        return Duration::ZERO;
    }

    let base_nanos = base.as_nanos().min(u128::from(u64::MAX)) as u64;
    let jitter_bound = (base_nanos / 4).max(1);
    let jitter = (Uuid::new_v4().as_u128() % u128::from(jitter_bound)) as u64;
    base.saturating_add(Duration::from_nanos(jitter))
}

pub(super) fn reqwest_error(error: reqwest::Error) -> BrokerError {
    if error.is_timeout() {
        BrokerError::Temporary {
            message: format!("KIS HTTP timeout: {error}"),
        }
    } else if error.is_connect() {
        BrokerError::Temporary {
            message: format!("KIS HTTP connection failed: {error}"),
        }
    } else {
        BrokerError::Transport {
            message: error.to_string(),
        }
    }
}
