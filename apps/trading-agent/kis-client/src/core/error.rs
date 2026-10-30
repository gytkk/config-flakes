use super::domain::ClientOrderId;

#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    #[error("authentication failed: {message}")]
    Auth { message: String },
    #[error("permission denied: {message}")]
    Permission { message: String },
    #[error("rate limited: {message}")]
    RateLimit { message: String },
    #[error("validation failed: {message}")]
    Validation { message: String },
    #[error("temporary broker error: {message}")]
    Temporary { message: String },
    #[error("transport error: {message}")]
    Transport { message: String },
    #[error("market is closed: {message}")]
    MarketClosed { message: String },
    #[error("duplicate intent: {client_order_id}")]
    DuplicateIntent { client_order_id: ClientOrderId },
    #[error("order rejected: {message}")]
    OrderRejected { message: String },
    #[error("reconciliation is required: {client_order_id}")]
    ReconciliationNeeded { client_order_id: ClientOrderId },
    #[error("safety halt: {reason}")]
    SafetyHalt { reason: String },
    #[error("broker invariant violated: {message}")]
    Invariant { message: String },
    #[error("persistence error: {message}")]
    Persistence { message: String },
}

impl BrokerError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    pub fn invariant(message: impl Into<String>) -> Self {
        Self::Invariant {
            message: message.into(),
        }
    }

    pub fn persistence(message: impl Into<String>) -> Self {
        Self::Persistence {
            message: message.into(),
        }
    }

    pub fn safety_halt(reason: impl Into<String>) -> Self {
        Self::SafetyHalt {
            reason: reason.into(),
        }
    }
}
