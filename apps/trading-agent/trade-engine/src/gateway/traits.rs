use async_trait::async_trait;
use trading_agent_kis_client::BrokerError;

use super::types::{
    AccountSnapshot, AmendOrderRequest, AmendResult, BrokerHealth, BuyingPower, CancelOrderRequest,
    CancelResult, CashBalance, CreateOrderRequest, Fill, MarketSession, OrderStatus,
    OrderSubmission, Position, Quote, Symbol, TradingDay,
};

#[async_trait]
pub trait TradingGateway: Send + Sync {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError>;
    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError>;
    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError>;
    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError>;
    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError>;
    async fn get_market_session(&self, trading_day: &str) -> Result<MarketSession, BrokerError>;
    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError>;
    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError>;
    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError>;
    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError>;
    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError>;
    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError>;
}
