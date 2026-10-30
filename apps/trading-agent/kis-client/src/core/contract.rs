use async_trait::async_trait;

use super::domain::{
    AmendOrderRequest, AmendResult, BrokerHealth, BuyingPower, CancelOrderRequest, CancelResult,
    CashBalance, ClientOrderId, CreateOrderRequest, Fill, OrderQuery, OrderStatus, OrderSubmission,
    Position, Quote, ReconciliationResult, Symbol, TradingDay,
};
use super::error::BrokerError;

#[async_trait]
pub trait BrokerClient: Send + Sync {
    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError>;
    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError>;
    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError>;
    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError>;
    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError>;
    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError>;
    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError>;
    async fn get_order(&self, query: OrderQuery) -> Result<OrderStatus, BrokerError>;
    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError>;
    async fn list_orders(&self, day: TradingDay) -> Result<Vec<OrderStatus>, BrokerError>;
    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError>;
    async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError>;
    async fn ping(&self) -> Result<BrokerHealth, BrokerError>;
}
