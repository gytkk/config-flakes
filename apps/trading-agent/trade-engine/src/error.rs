use std::path::PathBuf;

use thiserror::Error;
use trading_agent_kis_client::BrokerError;

#[derive(Debug, Error)]
pub enum TradeEngineError {
    #[error("config error: {message}")]
    Config { message: String },
    #[error("environment variable {name} is required")]
    MissingEnv { name: String },
    #[error("failed to read config {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse config {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("invalid {field}: {source}")]
    InvalidBrokerValue {
        field: &'static str,
        source: BrokerError,
    },
    #[error("gateway stage {stage} failed: {source}")]
    Gateway {
        stage: &'static str,
        source: BrokerError,
    },
    #[error("runtime error: {message}")]
    Runtime { message: String },
    #[error("broker setup failed: {0}")]
    Broker(#[from] BrokerError),
    #[error("storage error: {message}")]
    Storage { message: String },
}

impl TradeEngineError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    pub fn missing_env(name: impl Into<String>) -> Self {
        Self::MissingEnv { name: name.into() }
    }

    pub fn invalid_broker_value(field: &'static str, source: BrokerError) -> Self {
        Self::InvalidBrokerValue { field, source }
    }

    pub fn gateway(stage: &'static str, source: BrokerError) -> Self {
        Self::Gateway { stage, source }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self::Runtime {
            message: message.into(),
        }
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
        }
    }
}
