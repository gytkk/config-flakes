use time::{OffsetDateTime, macros::offset};

use crate::core::BrokerError;
use crate::core::journal::BrokerCommandStatus;
use crate::reconcile::{ReconciliationReport, reconcile_command_snapshot_report};
use crate::{
    BrokerEnvironment, ClientOrderId, ExchangeCode, OverseasStockOrderHistoryRequest,
    PendingReconciliation, ReconciliationResult, TradingDay,
};

use super::KisBrokerClient;
use super::helpers::{add_business_days, format_kis_date, next_business_day, parse_kis_date};

impl KisBrokerClient {
    pub async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError> {
        Ok(self
            .reconcile_order_report(client_order_id, lookback_days)
            .await?
            .result)
    }

    async fn reconcile_order_report(
        &self,
        client_order_id: &ClientOrderId,
        lookback_days: u16,
    ) -> Result<ReconciliationReport, BrokerError> {
        let journal = self.journal()?;
        let command = journal.get_command(client_order_id).await?.ok_or_else(|| {
            BrokerError::validation("reconcile order requires an existing journal command")
        })?;

        if command.environment != self.transport.environment() {
            return Err(BrokerError::validation(
                "journal command environment does not match KIS transport",
            ));
        }

        if let Some(configured_account) = &self.account_key
            && configured_account != &command.account_key
        {
            return Err(BrokerError::validation(
                "configured account does not match journal command account",
            ));
        }

        let today = OffsetDateTime::now_utc().to_offset(offset!(+9)).date();
        let start_day = command
            .trading_day
            .as_deref()
            .map(parse_kis_date)
            .transpose()?
            .unwrap_or_else(|| command.requested_at.to_offset(offset!(+9)).date());
        if start_day > today {
            return Err(BrokerError::validation(format!(
                "journal command trading day {} is in the future relative to current KST day {}",
                format_kis_date(start_day),
                format_kis_date(today)
            )));
        }
        let end_day = std::cmp::min(today, add_business_days(start_day, lookback_days)?);
        let overseas_exchange = command
            .exchange_code
            .as_ref()
            .map(|exchange_code| ExchangeCode::new(exchange_code.clone()))
            .transpose()?;

        let mut day = start_day;
        let mut orders = Vec::new();
        let mut fills = Vec::new();
        loop {
            if let Some(exchange) = overseas_exchange.as_ref() {
                let request = OverseasStockOrderHistoryRequest::new(
                    Some(exchange.clone()),
                    TradingDay(day),
                    TradingDay(day),
                )?;
                orders.extend(self.list_overseas_stock_orders(&request).await?);
                fills.extend(self.list_overseas_stock_fills(&request).await?);
            } else {
                orders.extend(self.list_orders(TradingDay(day)).await?);
                fills.extend(self.list_fills(TradingDay(day)).await?);
            }
            if day == end_day {
                break;
            }
            day = next_business_day(day)?;
        }

        let report = reconcile_command_snapshot_report(&command, &orders, &fills);
        self.persist_reconciliation_result(client_order_id, &report.result)
            .await?;
        Ok(report)
    }

    pub async fn reconcile_pending(
        &self,
        lookback_days: u16,
    ) -> Result<Vec<PendingReconciliation>, BrokerError> {
        let journal = self.journal()?;
        let commands = journal.list_unreconciled().await?;
        let mut results = Vec::with_capacity(commands.len());

        for command in commands {
            let report = self
                .reconcile_order_report(&command.client_order_id, lookback_days)
                .await?;
            results.push(PendingReconciliation {
                client_order_id: command.client_order_id,
                result: report.result,
                matcher: report.matcher.as_str().to_owned(),
            });
        }

        Ok(results)
    }

    pub async fn recover_startup(
        &self,
        lookback_days: u16,
    ) -> Result<Vec<PendingReconciliation>, BrokerError> {
        let results = self.reconcile_pending(lookback_days).await?;

        if self.transport.environment() == BrokerEnvironment::Live
            && !self.journal()?.list_unreconciled().await?.is_empty()
        {
            return Err(BrokerError::safety_halt(
                "unresolved journal records remain after startup reconciliation",
            ));
        }

        Ok(results)
    }

    async fn persist_reconciliation_result(
        &self,
        client_order_id: &ClientOrderId,
        result: &ReconciliationResult,
    ) -> Result<(), BrokerError> {
        let journal = self.journal()?;
        match result {
            ReconciliationResult::ConfirmedSubmitted | ReconciliationResult::ConfirmedAmended => {
                journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Accepted)
                    .await
            }
            ReconciliationResult::ConfirmedRejected => {
                journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Rejected)
                    .await
            }
            ReconciliationResult::ConfirmedCancelled => {
                journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Cancelled)
                    .await
            }
            ReconciliationResult::ConfirmedFilled => {
                journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Filled)
                    .await
            }
            ReconciliationResult::StillUnknown => {
                journal
                    .mark_reconcile_failed(
                        client_order_id,
                        "reconciliation snapshot did not find a unique matching broker state",
                    )
                    .await
            }
            ReconciliationResult::ManualInterventionRequired => {
                journal
                    .mark_reconcile_failed(
                        client_order_id,
                        "reconciliation found multiple matching broker states",
                    )
                    .await
            }
        }
    }
}
