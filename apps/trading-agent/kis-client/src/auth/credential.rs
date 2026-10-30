use std::fmt;

use crate::core::BrokerError;

#[derive(Clone, PartialEq, Eq)]
pub struct KisCredential {
    app_key: String,
    app_secret: String,
}

impl KisCredential {
    pub fn new(
        app_key: impl Into<String>,
        app_secret: impl Into<String>,
    ) -> Result<Self, BrokerError> {
        let app_key = app_key.into();
        let app_secret = app_secret.into();

        if app_key.trim().is_empty() {
            return Err(BrokerError::validation("KIS app key must not be empty"));
        }
        if app_secret.trim().is_empty() {
            return Err(BrokerError::validation("KIS app secret must not be empty"));
        }

        Ok(Self {
            app_key,
            app_secret,
        })
    }

    pub fn app_key(&self) -> &str {
        &self.app_key
    }

    pub fn app_secret(&self) -> &str {
        &self.app_secret
    }
}

impl fmt::Debug for KisCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KisCredential")
            .field("app_key", &"***")
            .field("app_secret", &"***")
            .finish()
    }
}
