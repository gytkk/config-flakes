use std::sync::Arc;

use time::OffsetDateTime;

mod account;
mod builder;
mod guardrails;
mod helpers;
mod market;
mod orders;
mod overseas;
mod recovery;

pub use builder::KisClientBuilder;

use crate::config::BrokerRuntimeConfig;
use crate::core::BrokerError;
use crate::core::journal::{BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus};
use crate::safety::SingleWriterLock;
use crate::transport::KisTransport;
use crate::{
    AccountKey, AmendOrderRequest, AmendResult, BrokerClient, BrokerHealth, BuyingPower,
    CancelOrderRequest, CancelResult, CashBalance, ClientOrderId, CreateOrderRequest, Fill,
    OrderQuery, OrderStatus, OrderSubmission, Position, Quote, ReconciliationResult, Symbol,
    TradingDay,
};

#[derive(Clone)]
pub struct KisBrokerClient {
    transport: KisTransport,
    account_key: Option<AccountKey>,
    runtime_config: Option<BrokerRuntimeConfig>,
    journal: Option<Arc<dyn BrokerCommandJournal>>,
    single_writer_lock: Option<Arc<SingleWriterLock>>,
    market_session_now: Option<OffsetDateTime>,
}

impl KisBrokerClient {
    pub fn new(transport: KisTransport) -> Self {
        Self {
            transport,
            account_key: None,
            runtime_config: None,
            journal: None,
            single_writer_lock: None,
            market_session_now: None,
        }
    }

    pub fn new_for_account(transport: KisTransport, account_key: AccountKey) -> Self {
        Self {
            transport,
            account_key: Some(account_key),
            runtime_config: None,
            journal: None,
            single_writer_lock: None,
            market_session_now: None,
        }
    }

    pub fn with_runtime_config(mut self, runtime_config: BrokerRuntimeConfig) -> Self {
        self.runtime_config = Some(runtime_config);
        self
    }

    pub fn with_journal(mut self, journal: Arc<dyn BrokerCommandJournal>) -> Self {
        self.journal = Some(journal);
        self
    }

    pub fn with_single_writer_lock(mut self, single_writer_lock: Arc<SingleWriterLock>) -> Self {
        self.single_writer_lock = Some(single_writer_lock);
        self
    }

    pub fn with_market_session_now(mut self, now_utc: OffsetDateTime) -> Self {
        self.market_session_now = Some(now_utc);
        self
    }

    fn account_key(&self) -> Result<&AccountKey, BrokerError> {
        self.account_key.as_ref().ok_or_else(|| {
            BrokerError::validation("KIS account read requires configured account_key")
        })
    }

    fn journal(&self) -> Result<Arc<dyn BrokerCommandJournal>, BrokerError> {
        self.journal
            .clone()
            .ok_or_else(|| BrokerError::safety_halt("broker writes require command journal"))
    }
}

pub(super) async fn reserve_or_use_pre_reserved_command(
    journal: &dyn BrokerCommandJournal,
    command: BrokerCommandRecord,
) -> Result<(), BrokerError> {
    match journal.reserve_command(command.clone()).await {
        Ok(()) => Ok(()),
        Err(BrokerError::DuplicateIntent { client_order_id })
            if client_order_id == command.client_order_id =>
        {
            let existing = journal
                .get_command(&command.client_order_id)
                .await?
                .ok_or_else(|| BrokerError::invariant("duplicate journal command was not found"))?;
            if pre_reserved_command_matches(&existing, &command) {
                Ok(())
            } else {
                Err(BrokerError::DuplicateIntent {
                    client_order_id: command.client_order_id,
                })
            }
        }
        Err(error) => Err(error),
    }
}

fn pre_reserved_command_matches(
    existing: &BrokerCommandRecord,
    command: &BrokerCommandRecord,
) -> bool {
    existing.last_known_state == BrokerCommandStatus::CommandRecorded
        && existing.parent_client_order_id == command.parent_client_order_id
        && existing.command_type == command.command_type
        && existing.environment == command.environment
        && existing.account_key == command.account_key
        && existing.symbol == command.symbol
        && existing.side == command.side
        && existing.quantity == command.quantity
        && existing.limit_price == command.limit_price
        && existing.order_type == command.order_type
        && existing.target_broker_order_id == command.target_broker_order_id
        && existing.target_broker_order_orgno == command.target_broker_order_orgno
        && existing.cancel_or_amend_quantity == command.cancel_or_amend_quantity
        && existing.amend_price == command.amend_price
        && existing.exchange_code == command.exchange_code
        && existing.request_id.is_none()
        && existing.transport_send_started_at.is_none()
        && existing.raw_ack_code.is_none()
        && !existing.reconcile_required
}

#[async_trait::async_trait]
impl BrokerClient for KisBrokerClient {
    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        KisBrokerClient::get_quote(self, symbol).await
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        KisBrokerClient::get_positions(self).await
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        KisBrokerClient::get_cash_balance(self).await
    }

    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        KisBrokerClient::get_buying_power(self, symbol).await
    }

    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError> {
        KisBrokerClient::place_order(self, order).await
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        KisBrokerClient::cancel_order(self, request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        KisBrokerClient::amend_order(self, request).await
    }

    async fn get_order(&self, query: OrderQuery) -> Result<OrderStatus, BrokerError> {
        KisBrokerClient::get_order(self, query).await
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        KisBrokerClient::list_open_orders(self).await
    }

    async fn list_orders(&self, day: TradingDay) -> Result<Vec<OrderStatus>, BrokerError> {
        KisBrokerClient::list_orders(self, day).await
    }

    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        KisBrokerClient::list_fills(self, day).await
    }

    async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError> {
        KisBrokerClient::reconcile_order(self, client_order_id, lookback_days).await
    }

    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        KisBrokerClient::ping(self).await
    }
}
