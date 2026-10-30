use std::fmt;

use crate::core::BrokerError;

#[derive(Clone, PartialEq, Eq)]
pub struct HashKey(String);

impl HashKey {
    pub fn new(value: impl Into<String>) -> Result<Self, BrokerError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(BrokerError::validation("KIS hashkey must not be empty"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for HashKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HashKey(***)")
    }
}
