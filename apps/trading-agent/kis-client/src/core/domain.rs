use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use time::{Date, OffsetDateTime};

use super::error::BrokerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrokerEnvironment {
    Mock,
    Paper,
    Live,
}

impl BrokerEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Paper => "paper",
            Self::Live => "live",
        }
    }

    pub fn parse(value: &str) -> Result<Self, BrokerError> {
        match value {
            "mock" => Ok(Self::Mock),
            "paper" => Ok(Self::Paper),
            "live" => Ok(Self::Live),
            other => Err(BrokerError::invariant(format!(
                "unknown broker environment: {other}"
            ))),
        }
    }
}

impl fmt::Display for BrokerEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

macro_rules! string_newtype {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, BrokerError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(BrokerError::validation(format!(
                        "{} must not be empty",
                        $label
                    )));
                }
                if value.chars().any(char::is_whitespace) {
                    return Err(BrokerError::validation(format!(
                        "{} must not contain whitespace",
                        $label
                    )));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

string_newtype!(AccountKey, "account key");
string_newtype!(BrokerOrderId, "broker order id");
string_newtype!(BrokerOrderOrgNo, "broker order organization number");
string_newtype!(ClientOrderId, "client order id");
string_newtype!(CurrencyCode, "currency code");
string_newtype!(ExchangeCode, "exchange code");
string_newtype!(RequestId, "request id");
string_newtype!(Symbol, "symbol");

impl AccountKey {
    pub fn ends_with_suffix(&self, suffix: &str) -> bool {
        if suffix.is_empty() {
            return false;
        }
        self.0.ends_with(suffix)
            || self
                .0
                .split_once('-')
                .map(|(account_number, _)| account_number)
                .is_some_and(|account_number| account_number.ends_with(suffix))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quantity(u64);

impl Quantity {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }

    pub fn parse(value: &str) -> Result<Self, BrokerError> {
        match value {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            other => Err(BrokerError::invariant(format!(
                "unknown order side: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
}

impl OrderType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Limit => "limit",
        }
    }

    pub fn parse(value: &str) -> Result<Self, BrokerError> {
        match value {
            "market" => Ok(Self::Market),
            "limit" => Ok(Self::Limit),
            other => Err(BrokerError::invariant(format!(
                "unknown order type: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsEquityOrderSession {
    Daytime,
    Premarket,
    Regular,
    Aftermarket,
}

impl UsEquityOrderSession {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daytime => "daytime",
            Self::Premarket => "premarket",
            Self::Regular => "regular",
            Self::Aftermarket => "aftermarket",
        }
    }

    pub fn parse(value: &str) -> Result<Self, BrokerError> {
        match value {
            "daytime" => Ok(Self::Daytime),
            "premarket" => Ok(Self::Premarket),
            "regular" => Ok(Self::Regular),
            "aftermarket" => Ok(Self::Aftermarket),
            other => Err(BrokerError::invariant(format!(
                "unknown US equity order session: {other}"
            ))),
        }
    }
}

impl fmt::Display for UsEquityOrderSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: Symbol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    pub last_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub symbol: Symbol,
    pub quantity: Quantity,
    pub average_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashBalance {
    pub cash: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSnapshot {
    pub cash: CashBalance,
    pub positions: Vec<Position>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyingPower {
    pub available_cash: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStock {
    pub exchange: ExchangeCode,
    pub symbol: Symbol,
}

impl OverseasStock {
    pub fn new(exchange: ExchangeCode, symbol: Symbol) -> Self {
        Self { exchange, symbol }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockQuote {
    pub instrument: OverseasStock,
    pub last_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockPosition {
    pub instrument: OverseasStock,
    pub currency: CurrencyCode,
    pub quantity: Quantity,
    pub average_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockBalanceSummary {
    pub currency: CurrencyCode,
    pub purchase_amount: Decimal,
    pub evaluation_amount: Decimal,
    pub profit_loss: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockAccountSnapshot {
    pub summary: OverseasStockBalanceSummary,
    pub positions: Vec<OverseasStockPosition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockBalanceRequest {
    pub exchange: ExchangeCode,
    pub currency: CurrencyCode,
}

impl OverseasStockBalanceRequest {
    pub fn new(exchange: ExchangeCode, currency: CurrencyCode) -> Self {
        Self { exchange, currency }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockBuyingPowerRequest {
    pub instrument: OverseasStock,
    pub order_price: Decimal,
}

impl OverseasStockBuyingPowerRequest {
    pub fn new(instrument: OverseasStock, order_price: Decimal) -> Result<Self, BrokerError> {
        if order_price <= Decimal::ZERO {
            return Err(BrokerError::validation(
                "overseas stock buying power order_price must be greater than zero",
            ));
        }

        Ok(Self {
            instrument,
            order_price,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockBuyingPower {
    pub currency: CurrencyCode,
    pub available_cash: Decimal,
    pub max_order_quantity: Option<Quantity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockOrderRequest {
    pub client_order_id: ClientOrderId,
    pub account_key: AccountKey,
    pub instrument: OverseasStock,
    pub side: OrderSide,
    pub quantity: Quantity,
    pub order_type: OrderType,
    pub limit_price: Option<Decimal>,
}

impl OverseasStockOrderRequest {
    pub fn cash_limit(
        client_order_id: ClientOrderId,
        account_key: AccountKey,
        instrument: OverseasStock,
        side: OrderSide,
        quantity: Quantity,
        limit_price: Decimal,
    ) -> Result<Self, BrokerError> {
        if quantity.is_zero() {
            return Err(BrokerError::validation(
                "overseas stock order quantity must be greater than zero",
            ));
        }
        if limit_price <= Decimal::ZERO {
            return Err(BrokerError::validation(
                "overseas stock limit price must be greater than zero",
            ));
        }

        Ok(Self {
            client_order_id,
            account_key,
            instrument,
            side,
            quantity,
            order_type: OrderType::Limit,
            limit_price: Some(limit_price),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverseasStockOrderSideFilter {
    All,
    Sell,
    Buy,
}

impl OverseasStockOrderSideFilter {
    pub fn as_kis_code(self) -> &'static str {
        match self {
            Self::All => "00",
            Self::Sell => "01",
            Self::Buy => "02",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverseasStockOrderStatusFilter {
    All,
    Filled,
    Open,
}

impl OverseasStockOrderStatusFilter {
    pub fn as_kis_code(self) -> &'static str {
        match self {
            Self::All => "00",
            Self::Filled => "01",
            Self::Open => "02",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverseasStockOrderSort {
    Ascending,
    Descending,
}

impl OverseasStockOrderSort {
    pub fn as_kis_code(self) -> &'static str {
        match self {
            Self::Ascending => "DS",
            Self::Descending => "AS",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverseasStockOrderHistoryRequest {
    pub exchange: Option<ExchangeCode>,
    pub symbol: Option<Symbol>,
    pub start_day: TradingDay,
    pub end_day: TradingDay,
    pub side: OverseasStockOrderSideFilter,
    pub status: OverseasStockOrderStatusFilter,
    pub sort: OverseasStockOrderSort,
}

impl OverseasStockOrderHistoryRequest {
    pub fn new(
        exchange: Option<ExchangeCode>,
        start_day: TradingDay,
        end_day: TradingDay,
    ) -> Result<Self, BrokerError> {
        if start_day.0 > end_day.0 {
            return Err(BrokerError::validation(
                "overseas stock order history start_day must be on or before end_day",
            ));
        }

        Ok(Self {
            exchange,
            symbol: None,
            start_day,
            end_day,
            side: OverseasStockOrderSideFilter::All,
            status: OverseasStockOrderStatusFilter::All,
            sort: OverseasStockOrderSort::Ascending,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketSession {
    pub trading_day: String,
    pub is_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub client_order_id: ClientOrderId,
    pub account_key: AccountKey,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub quantity: Quantity,
    pub order_type: OrderType,
    pub limit_price: Option<Decimal>,
}

impl CreateOrderRequest {
    pub fn cash_limit(
        client_order_id: ClientOrderId,
        account_key: AccountKey,
        symbol: Symbol,
        side: OrderSide,
        quantity: Quantity,
        limit_price: Decimal,
    ) -> Result<Self, BrokerError> {
        if quantity.is_zero() {
            return Err(BrokerError::validation(
                "order quantity must be greater than zero",
            ));
        }
        if limit_price <= Decimal::ZERO {
            return Err(BrokerError::validation(
                "limit price must be greater than zero",
            ));
        }

        Ok(Self {
            client_order_id,
            account_key,
            symbol,
            side,
            quantity,
            order_type: OrderType::Limit,
            limit_price: Some(limit_price),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderReference {
    pub client_order_id: ClientOrderId,
    pub broker_order_id: Option<BrokerOrderId>,
    pub broker_order_orgno: Option<BrokerOrderOrgNo>,
    pub exchange_code: Option<ExchangeCode>,
    pub account_key: AccountKey,
    pub environment: BrokerEnvironment,
}

impl OrderReference {
    pub fn is_complete_for_follow_up(&self) -> bool {
        self.broker_order_id.is_some() && self.broker_order_orgno.is_some()
    }

    pub fn with_broker_order_id(mut self, broker_order_id: &str) -> Result<Self, BrokerError> {
        self.broker_order_id = Some(BrokerOrderId::new(broker_order_id)?);
        Ok(self)
    }

    pub fn with_broker_order_orgno(
        mut self,
        broker_order_orgno: &str,
    ) -> Result<Self, BrokerError> {
        self.broker_order_orgno = Some(BrokerOrderOrgNo::new(broker_order_orgno)?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderSubmission {
    pub reference: OrderReference,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelOrderRequest {
    pub client_order_id: ClientOrderId,
    pub reference: OrderReference,
    pub quantity: Option<Quantity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelResult {
    pub reference: OrderReference,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmendOrderRequest {
    pub client_order_id: ClientOrderId,
    pub reference: OrderReference,
    pub quantity: Option<Quantity>,
    pub limit_price: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmendResult {
    pub reference: OrderReference,
    pub accepted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelableQuantity(Quantity);

impl CancelableQuantity {
    pub fn new(quantity: Quantity) -> Self {
        Self(quantity)
    }

    pub fn quantity(self) -> Quantity {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmendableQuantity(Quantity);

impl AmendableQuantity {
    pub fn new(quantity: Quantity) -> Self {
        Self(quantity)
    }

    pub fn quantity(self) -> Quantity {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviseCancelCandidate {
    pub reference: OrderReference,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub ordered_quantity: Quantity,
    pub filled_quantity: Quantity,
    pub order_price: Option<Decimal>,
    pub cancelable_quantity: CancelableQuantity,
    pub amendable_quantity: AmendableQuantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviseCancelCapability {
    pub candidates: Vec<ReviseCancelCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderQuery {
    pub reference: OrderReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderStatus {
    pub reference: OrderReference,
    pub state: OrderState,
    pub evidence: Option<OrderStatusEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderStatusEvidence {
    pub symbol: Symbol,
    pub side: Option<OrderSide>,
    pub ordered_quantity: Quantity,
    pub filled_quantity: Quantity,
    pub order_price: Option<Decimal>,
    pub trading_day: Option<String>,
    pub observed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderState {
    Accepted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fill {
    pub reference: OrderReference,
    pub quantity: Quantity,
    pub price: Decimal,
    pub evidence: Option<OrderStatusEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TradingDay(pub Date);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationResult {
    ConfirmedSubmitted,
    ConfirmedAmended,
    ConfirmedRejected,
    ConfirmedCancelled,
    ConfirmedFilled,
    StillUnknown,
    ManualInterventionRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingReconciliation {
    pub client_order_id: ClientOrderId,
    pub result: ReconciliationResult,
    pub matcher: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrokerHealth {
    pub environment: BrokerEnvironment,
    pub writable: bool,
}
