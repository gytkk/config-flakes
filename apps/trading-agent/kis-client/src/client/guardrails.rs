use time::{OffsetDateTime, macros::offset};

use crate::core::BrokerError;
use crate::core::journal::BrokerCommandRecord;
use crate::safety::SingleWriterLockScope;
use crate::{
    AccountKey, BrokerEnvironment, ClientOrderId, ExchangeCode, OrderReference, Quantity,
    UsEquityOrderSession,
};

use super::KisBrokerClient;
use super::helpers::{
    FollowUpOperation, current_kst_trading_day_at, is_krx_regular_session_time,
    revise_cancel_candidate_matches, us_equity_order_session_at,
    us_equity_order_session_trading_day_at,
};

impl KisBrokerClient {
    pub(super) async fn validate_write_access(
        &self,
        account_key: &AccountKey,
        lock_scope: SingleWriterLockScope,
    ) -> Result<(), BrokerError> {
        if let Some(config) = &self.runtime_config {
            config.validate_write_enabled(self.transport.environment(), account_key)?;
        }

        if let Some(configured_account) = &self.account_key
            && configured_account != account_key
        {
            return Err(BrokerError::validation(
                "configured account does not match write request account",
            ));
        }

        if self.transport.environment() == BrokerEnvironment::Live {
            if self.runtime_config.is_none() {
                return Err(BrokerError::safety_halt(
                    "live writes require runtime guardrail config",
                ));
            }
            let Some(single_writer_lock) = &self.single_writer_lock else {
                return Err(BrokerError::safety_halt(
                    "live writes require an acquired single-writer lock",
                ));
            };
            if single_writer_lock.environment() != self.transport.environment()
                || single_writer_lock.account_key() != account_key
                || single_writer_lock.scope() != lock_scope
            {
                return Err(BrokerError::safety_halt(
                    "single-writer lock does not match live write environment, account, and market scope",
                ));
            }
            let journal = self.journal()?;
            if !journal.list_unreconciled().await?.is_empty() {
                return Err(BrokerError::safety_halt(
                    "unresolved journal records block live writes",
                ));
            }
        }

        Ok(())
    }

    pub(super) async fn ensure_follow_up_write_preflight(
        &self,
        parent: &BrokerCommandRecord,
        reference: &OrderReference,
        operation: FollowUpOperation,
        requested_quantity: Option<Quantity>,
    ) -> Result<(), BrokerError> {
        let operation_name = operation.as_str();
        if reference.environment != self.transport.environment() {
            return Err(BrokerError::validation(format!(
                "{operation_name} order reference environment does not match KIS transport",
            )));
        }

        let capability = self.get_revise_cancel_capability().await?;
        let mut matches = capability
            .candidates
            .into_iter()
            .filter(|candidate| revise_cancel_candidate_matches(reference, candidate));
        let Some(candidate) = matches.next() else {
            return Err(BrokerError::validation(format!(
                "{operation_name} order requires a matching KIS possible order before broker write",
            )));
        };
        if matches.next().is_some() {
            return Err(BrokerError::invariant(format!(
                "{operation_name} order preflight found duplicate matching KIS possible orders",
            )));
        }

        if candidate.symbol != parent.symbol || candidate.side != parent.side {
            return Err(BrokerError::validation(format!(
                "{operation_name} order possible row does not match parent symbol and side",
            )));
        }

        let possible_quantity = match operation {
            FollowUpOperation::Cancel => candidate.cancelable_quantity.quantity(),
            FollowUpOperation::Amend => candidate.amendable_quantity.quantity(),
        };
        if possible_quantity.is_zero() {
            return Err(BrokerError::validation(format!(
                "{operation_name} order requires positive KIS possible quantity",
            )));
        }

        if let Some(quantity) = requested_quantity {
            if quantity.is_zero() {
                return Err(BrokerError::validation(format!(
                    "{operation_name} order quantity must be greater than zero",
                )));
            }
            if quantity.get() > possible_quantity.get() {
                return Err(BrokerError::validation(format!(
                    "{operation_name} order quantity exceeds KIS possible quantity",
                )));
            }
        }

        Ok(())
    }

    pub(super) async fn ensure_overseas_follow_up_write_preflight(
        &self,
        parent: &BrokerCommandRecord,
        reference: &OrderReference,
        operation: FollowUpOperation,
        requested_quantity: Option<Quantity>,
    ) -> Result<crate::ReviseCancelCandidate, BrokerError> {
        let operation_name = operation.as_str();
        if reference.environment != self.transport.environment() {
            return Err(BrokerError::validation(format!(
                "{operation_name} overseas order reference environment does not match KIS transport",
            )));
        }

        let exchange = if let Some(exchange) = &reference.exchange_code {
            exchange.clone()
        } else {
            parent
                .exchange_code
                .as_ref()
                .map(crate::ExchangeCode::new)
                .transpose()?
                .ok_or_else(|| {
                    BrokerError::validation("overseas follow-up preflight requires exchange code")
                })?
        };

        let capability = self
            .get_overseas_stock_revise_cancel_capability(&exchange)
            .await?;
        let mut matches = capability
            .candidates
            .into_iter()
            .filter(|candidate| revise_cancel_candidate_matches(reference, candidate));
        let Some(candidate) = matches.next() else {
            return Err(BrokerError::validation(format!(
                "{operation_name} overseas order requires a matching KIS open order before broker write",
            )));
        };
        if matches.next().is_some() {
            return Err(BrokerError::invariant(format!(
                "{operation_name} overseas order preflight found duplicate matching KIS open orders",
            )));
        }

        if candidate.symbol != parent.symbol || candidate.side != parent.side {
            return Err(BrokerError::validation(format!(
                "{operation_name} overseas order open row does not match parent symbol and side",
            )));
        }

        let possible_quantity = match operation {
            FollowUpOperation::Cancel => candidate.cancelable_quantity.quantity(),
            FollowUpOperation::Amend => candidate.amendable_quantity.quantity(),
        };
        if possible_quantity.is_zero() {
            return Err(BrokerError::validation(format!(
                "{operation_name} overseas order requires positive KIS open quantity",
            )));
        }

        if let Some(quantity) = requested_quantity {
            if quantity.is_zero() {
                return Err(BrokerError::validation(format!(
                    "{operation_name} overseas order quantity must be greater than zero",
                )));
            }
            if quantity.get() > possible_quantity.get() {
                return Err(BrokerError::validation(format!(
                    "{operation_name} overseas order quantity exceeds KIS open quantity",
                )));
            }
        }

        Ok(candidate)
    }

    pub(super) async fn ensure_market_write_gate(&self) -> Result<(), BrokerError> {
        if self.transport.environment() != BrokerEnvironment::Live {
            return Ok(());
        }

        let Some(config) = &self.runtime_config else {
            return Err(BrokerError::safety_halt(
                "live writes require runtime guardrail config",
            ));
        };
        if !config.market_session_write_gate.is_enforced() {
            return Ok(());
        }

        let now_utc = self
            .market_session_now
            .unwrap_or_else(OffsetDateTime::now_utc);
        let trading_day = current_kst_trading_day_at(now_utc);
        let session = self
            .get_market_session(&trading_day)
            .await
            .map_err(|error| {
                BrokerError::safety_halt(format!(
                    "market session state is unknown for {trading_day}: {error}"
                ))
            })?;

        if session.trading_day != trading_day {
            return Err(BrokerError::safety_halt(format!(
                "market session response day {} does not match requested trading day {trading_day}",
                session.trading_day
            )));
        }
        if !session.is_open {
            return Err(BrokerError::safety_halt(format!(
                "market session is closed for trading day {trading_day}",
            )));
        }
        if !is_krx_regular_session_time(now_utc) {
            return Err(BrokerError::safety_halt(format!(
                "regular market session is closed for KST time {}",
                now_utc.to_offset(offset!(+9)).time()
            )));
        }

        Ok(())
    }

    pub(super) async fn ensure_overseas_market_write_gate(
        &self,
        exchange: &ExchangeCode,
    ) -> Result<UsEquityOrderSession, BrokerError> {
        if self.transport.environment() != BrokerEnvironment::Live {
            return Ok(UsEquityOrderSession::Regular);
        }

        let Some(config) = &self.runtime_config else {
            return Err(BrokerError::safety_halt(
                "live writes require runtime guardrail config",
            ));
        };
        if !config.market_session_write_gate.is_enforced() {
            return Ok(UsEquityOrderSession::Regular);
        }

        let now_utc = self
            .market_session_now
            .unwrap_or_else(OffsetDateTime::now_utc);
        let Some(order_session) = us_equity_order_session_at(now_utc) else {
            return Err(BrokerError::safety_halt(format!(
                "KIS US equity order session is closed for UTC time {}",
                now_utc.time()
            )));
        };
        if !config
            .allowed_us_equity_order_sessions
            .contains(&order_session)
        {
            return Err(BrokerError::safety_halt(format!(
                "KIS US equity order session {order_session} is not enabled for live writes",
            )));
        }

        let trading_day = us_equity_order_session_trading_day_at(now_utc, order_session);
        let session = self
            .get_overseas_stock_market_session(exchange, &trading_day)
            .await
            .map_err(|error| {
                BrokerError::safety_halt(format!(
                    "overseas market session state is unknown for {trading_day}: {error}"
                ))
            })?;

        if session.trading_day != trading_day {
            return Err(BrokerError::safety_halt(format!(
                "overseas market session response day {} does not match requested trading day {trading_day}",
                session.trading_day
            )));
        }
        if !session.is_open {
            return Err(BrokerError::safety_halt(format!(
                "overseas market session is closed for trading day {trading_day}",
            )));
        }

        Ok(order_session)
    }

    pub(super) async fn mark_ambiguous_write(
        &self,
        client_order_id: &ClientOrderId,
        error: BrokerError,
    ) -> Result<(), BrokerError> {
        let reason = error.to_string();
        self.journal()?
            .mark_reconcile_needed(client_order_id, &reason)
            .await
    }
}
