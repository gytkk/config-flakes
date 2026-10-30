use serde::Deserialize;
use std::fmt;
use time::macros::format_description;
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

use crate::core::BrokerError;

const KST_OFFSET: UtcOffset = time::macros::offset!(+9);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct KisTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub access_token_token_expired: String,
}

impl KisTokenResponse {
    pub fn to_snapshot(self) -> Result<TokenSnapshot, BrokerError> {
        Ok(TokenSnapshot {
            access_token: AccessToken::new(self.access_token)?,
            token_type: self.token_type,
            expires_in_seconds: self.expires_in,
            expires_at_utc: parse_kis_expiry_as_utc(&self.access_token_token_expired)?,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(String);

impl AccessToken {
    pub fn new(value: impl Into<String>) -> Result<Self, BrokerError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(BrokerError::validation("access token must not be empty"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken(***)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSnapshot {
    pub access_token: AccessToken,
    pub token_type: String,
    pub expires_in_seconds: u64,
    pub expires_at_utc: OffsetDateTime,
}

impl TokenSnapshot {
    pub(super) fn is_usable_at(&self, now: OffsetDateTime, refresh_margin: time::Duration) -> bool {
        now.checked_add(refresh_margin)
            .is_some_and(|refresh_at| refresh_at < self.expires_at_utc)
    }
}

fn parse_kis_expiry_as_utc(value: &str) -> Result<OffsetDateTime, BrokerError> {
    let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let local_expiry = PrimitiveDateTime::parse(value, &format).map_err(|error| {
        BrokerError::validation(format!("invalid KIS token expiry timestamp: {error}"))
    })?;

    Ok(local_expiry
        .assume_offset(KST_OFFSET)
        .to_offset(UtcOffset::UTC))
}
