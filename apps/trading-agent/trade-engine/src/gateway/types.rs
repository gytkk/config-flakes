pub use trading_agent_kis_client::domain::{
    AccountSnapshot, AmendOrderRequest, AmendResult, BrokerHealth, BuyingPower, CancelOrderRequest,
    CancelResult, CashBalance, CreateOrderRequest, CurrencyCode, ExchangeCode, Fill, MarketSession,
    OrderStatus, OrderSubmission, Position, Quantity, Quote, Symbol, TradingDay,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketScope {
    DomesticStock,
    OverseasStock {
        exchange: ExchangeCode,
        currency: CurrencyCode,
    },
}

impl MarketScope {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::DomesticStock => "domestic_stock",
            Self::OverseasStock { .. } => "overseas_stock",
        }
    }
}
