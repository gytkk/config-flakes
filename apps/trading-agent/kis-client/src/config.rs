use crate::core::BrokerError;
use crate::{AccountKey, BrokerEnvironment, UsEquityOrderSession};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerRuntimeConfig {
    pub read_only: bool,
    pub enable_live_trading: bool,
    pub confirmed_account_suffix: Option<String>,
    pub allowed_live_accounts: Vec<String>,
    pub market_session_write_gate: MarketSessionWriteGate,
    pub allowed_us_equity_order_sessions: Vec<UsEquityOrderSession>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketSessionWriteGate {
    Enforced,
    DisabledForTesting,
}

impl MarketSessionWriteGate {
    pub fn is_enforced(self) -> bool {
        self == Self::Enforced
    }
}

impl BrokerRuntimeConfig {
    pub fn regular_us_equity_order_sessions() -> Vec<UsEquityOrderSession> {
        vec![UsEquityOrderSession::Regular]
    }

    pub fn validate_write_enabled(
        &self,
        environment: BrokerEnvironment,
        account_key: &AccountKey,
    ) -> Result<(), BrokerError> {
        if self.read_only {
            return Err(BrokerError::safety_halt(
                "read-only mode blocks broker writes",
            ));
        }

        if environment != BrokerEnvironment::Live {
            return Ok(());
        }

        if !self.enable_live_trading {
            return Err(BrokerError::safety_halt(
                "live trading requires enable_live_trading=true",
            ));
        }

        if !self.allowed_live_accounts.is_empty()
            && !self
                .allowed_live_accounts
                .iter()
                .any(|account| account == account_key.as_str())
        {
            return Err(BrokerError::safety_halt(
                "live account is not present in allowed_live_accounts",
            ));
        }

        let Some(confirmed_suffix) = &self.confirmed_account_suffix else {
            return Err(BrokerError::safety_halt(
                "live trading requires confirmed_account_suffix",
            ));
        };

        if !account_key.ends_with_suffix(confirmed_suffix) {
            return Err(BrokerError::safety_halt(
                "confirmed account suffix does not match configured account",
            ));
        }

        Ok(())
    }
}
