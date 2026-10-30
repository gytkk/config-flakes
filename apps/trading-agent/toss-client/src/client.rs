use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Value;
use trading_agent_kis_client::{
    AccountSnapshot, AmendOrderRequest, AmendResult, BrokerClient, BrokerError, BrokerHealth,
    BuyingPower, CancelOrderRequest, CancelResult, CashBalance, ClientOrderId, CreateOrderRequest,
    Fill, MarketSession, OrderQuery, OrderStatus, OrderSubmission, Position, Quote,
    ReconciliationResult, Symbol, TradingDay,
};

use crate::mapping::{decimal_from_str, order_status_from_raw, position_from_holding};
use crate::transport::{TossEndpoint, TossHttpMethod, TossTransport};

const ACCOUNTS_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "accounts",
    method: TossHttpMethod::Get,
    path: "/api/v1/accounts",
};
const HOLDINGS_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "holdings",
    method: TossHttpMethod::Get,
    path: "/api/v1/holdings",
};
const PRICES_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "prices",
    method: TossHttpMethod::Get,
    path: "/api/v1/prices",
};
const STOCKS_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "stocks",
    method: TossHttpMethod::Get,
    path: "/api/v1/stocks",
};
const STOCK_WARNINGS_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "stock_warnings",
    method: TossHttpMethod::Get,
    path: "/api/v1/stocks",
};
const CANDLES_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "candles",
    method: TossHttpMethod::Get,
    path: "/api/v1/candles",
};
const TRADES_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "trades",
    method: TossHttpMethod::Get,
    path: "/api/v1/trades",
};
const ORDERBOOK_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "orderbook",
    method: TossHttpMethod::Get,
    path: "/api/v1/orderbook",
};
const PRICE_LIMITS_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "price_limits",
    method: TossHttpMethod::Get,
    path: "/api/v1/price-limits",
};
const KR_MARKET_CALENDAR_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "kr_market_calendar",
    method: TossHttpMethod::Get,
    path: "/api/v1/market-calendar/KR",
};
const US_MARKET_CALENDAR_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "us_market_calendar",
    method: TossHttpMethod::Get,
    path: "/api/v1/market-calendar/US",
};
const BUYING_POWER_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "buying_power",
    method: TossHttpMethod::Get,
    path: "/api/v1/buying-power",
};
const ORDERS_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "orders",
    method: TossHttpMethod::Get,
    path: "/api/v1/orders",
};
const ORDER_ENDPOINT: TossEndpoint = TossEndpoint {
    name: "order",
    method: TossHttpMethod::Get,
    path: "/api/v1/orders",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TossAccountSeq(u64);

impl TossAccountSeq {
    pub fn new(value: u64) -> Result<Self, BrokerError> {
        if value == 0 {
            return Err(BrokerError::validation(
                "Toss account_seq must be greater than zero",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossMarketCountry {
    Kr,
    Us,
}

impl TossMarketCountry {
    pub fn currency(self) -> &'static str {
        match self {
            Self::Kr => "KRW",
            Self::Us => "USD",
        }
    }

    fn calendar_endpoint(self) -> TossEndpoint {
        match self {
            Self::Kr => KR_MARKET_CALENDAR_ENDPOINT,
            Self::Us => US_MARKET_CALENDAR_ENDPOINT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TossAccount {
    #[serde(rename = "accountNo")]
    pub account_no: String,
    #[serde(rename = "accountSeq")]
    pub account_seq: u64,
    #[serde(rename = "accountType")]
    pub account_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossCandleInterval {
    OneMinute,
    OneDay,
}

impl TossCandleInterval {
    fn as_str(self) -> &'static str {
        match self {
            Self::OneMinute => "1m",
            Self::OneDay => "1d",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossCandleRequest {
    pub symbol: Symbol,
    pub interval: TossCandleInterval,
    pub count: Option<u16>,
    pub before: Option<String>,
    pub adjusted: Option<bool>,
}

impl TossCandleRequest {
    pub fn new(symbol: Symbol, interval: TossCandleInterval) -> Self {
        Self {
            symbol,
            interval,
            count: None,
            before: None,
            adjusted: None,
        }
    }

    pub fn with_count(mut self, count: u16) -> Self {
        self.count = Some(count);
        self
    }

    pub fn with_before(mut self, before: impl Into<String>) -> Self {
        self.before = Some(before.into());
        self
    }

    pub fn with_adjusted(mut self, adjusted: bool) -> Self {
        self.adjusted = Some(adjusted);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossStockInfo {
    pub symbol: Symbol,
    pub name: String,
    pub english_name: String,
    pub isin_code: String,
    pub market: String,
    pub security_type: String,
    pub is_common_share: bool,
    pub status: String,
    pub currency: String,
    pub list_date: Option<String>,
    pub delist_date: Option<String>,
    pub shares_outstanding: Decimal,
    pub leverage_factor: Option<Decimal>,
    pub korean_market_detail: Option<TossKrMarketDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossKrMarketDetail {
    pub liquidation_trading: bool,
    pub nxt_supported: bool,
    pub krx_trading_suspended: bool,
    pub nxt_trading_suspended: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossCandlePage {
    pub candles: Vec<TossCandle>,
    pub next_before: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossCandle {
    pub timestamp: String,
    pub open_price: Decimal,
    pub high_price: Decimal,
    pub low_price: Decimal,
    pub close_price: Decimal,
    pub volume: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossTrade {
    pub price: Decimal,
    pub volume: Decimal,
    pub timestamp: String,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossOrderbook {
    pub timestamp: Option<String>,
    pub currency: String,
    pub asks: Vec<TossOrderbookEntry>,
    pub bids: Vec<TossOrderbookEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossOrderbookEntry {
    pub price: Decimal,
    pub volume: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TossPriceLimits {
    pub timestamp: String,
    pub upper_limit_price: Option<Decimal>,
    pub lower_limit_price: Option<Decimal>,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TossStockWarning {
    #[serde(rename = "warningType")]
    pub warning_type: String,
    pub exchange: Option<String>,
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TossBrokerClient {
    transport: TossTransport,
    account_seq: TossAccountSeq,
    market_country: TossMarketCountry,
}

impl TossBrokerClient {
    pub fn new(
        transport: TossTransport,
        account_seq: TossAccountSeq,
        market_country: TossMarketCountry,
    ) -> Self {
        Self {
            transport,
            account_seq,
            market_country,
        }
    }

    pub async fn get_accounts(&self) -> Result<Vec<TossAccount>, BrokerError> {
        self.transport.get(ACCOUNTS_ENDPOINT, &[], None).await
    }

    pub async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        let positions = self.get_positions().await?;
        let cash = self.get_cash_balance().await?;
        Ok(AccountSnapshot { cash, positions })
    }

    pub async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        let holdings = self.get_holdings(None).await?;
        holdings
            .items
            .into_iter()
            .map(position_from_holding)
            .collect()
    }

    pub async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        Ok(CashBalance {
            cash: self.get_buying_power(None).await?.available_cash,
        })
    }

    pub async fn get_buying_power(
        &self,
        _symbol: Option<&Symbol>,
    ) -> Result<BuyingPower, BrokerError> {
        let result: RawBuyingPower = self
            .transport
            .get(
                BUYING_POWER_ENDPOINT,
                &[("currency", self.market_country.currency().to_owned())],
                Some(self.account_seq),
            )
            .await?;
        Ok(BuyingPower {
            available_cash: decimal_from_str(
                "buyingPower.cashBuyingPower",
                &result.cash_buying_power,
            )?,
        })
    }

    pub async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        let prices: Vec<RawPrice> = self
            .transport
            .get(
                PRICES_ENDPOINT,
                &[("symbols", symbol.as_str().to_owned())],
                None,
            )
            .await?;
        let price = prices
            .into_iter()
            .find(|price| price.symbol == symbol.as_str())
            .ok_or_else(|| {
                BrokerError::invariant(format!(
                    "Toss prices response did not include requested symbol {}",
                    symbol.as_str()
                ))
            })?;
        Ok(Quote {
            symbol: Symbol::new(price.symbol)?,
            full_name: None,
            last_price: decimal_from_str("prices[].lastPrice", &price.last_price)?,
        })
    }

    pub async fn get_stock_info(
        &self,
        symbols: &[Symbol],
    ) -> Result<Vec<TossStockInfo>, BrokerError> {
        validate_symbol_batch("Toss stock info", symbols, 200)?;
        let raw: Vec<RawStockInfo> = self
            .transport
            .get(
                STOCKS_ENDPOINT,
                &[("symbols", joined_symbols(symbols))],
                None,
            )
            .await?;
        raw.into_iter().map(stock_info_from_raw).collect()
    }

    pub async fn get_stock_warnings(
        &self,
        symbol: &Symbol,
    ) -> Result<Vec<TossStockWarning>, BrokerError> {
        validate_path_symbol("Toss stock warning symbol", symbol)?;
        let path = format!("/api/v1/stocks/{}/warnings", symbol.as_str());
        self.transport
            .get_path(STOCK_WARNINGS_ENDPOINT, &path, &[], None)
            .await
    }

    pub async fn get_candles(
        &self,
        request: TossCandleRequest,
    ) -> Result<TossCandlePage, BrokerError> {
        if let Some(count) = request.count {
            validate_count("Toss candle count", count, 1, 200)?;
        }
        let mut query = vec![
            ("symbol", request.symbol.as_str().to_owned()),
            ("interval", request.interval.as_str().to_owned()),
        ];
        if let Some(count) = request.count {
            query.push(("count", count.to_string()));
        }
        if let Some(before) = request.before {
            query.push(("before", before));
        }
        if let Some(adjusted) = request.adjusted {
            query.push(("adjusted", adjusted.to_string()));
        }
        let raw: RawCandlePage = self.transport.get(CANDLES_ENDPOINT, &query, None).await?;
        candle_page_from_raw(raw)
    }

    pub async fn get_recent_trades(
        &self,
        symbol: &Symbol,
        count: Option<u16>,
    ) -> Result<Vec<TossTrade>, BrokerError> {
        let mut query = vec![("symbol", symbol.as_str().to_owned())];
        if let Some(count) = count {
            validate_count("Toss recent trade count", count, 1, 50)?;
            query.push(("count", count.to_string()));
        }
        let raw: Vec<RawTrade> = self.transport.get(TRADES_ENDPOINT, &query, None).await?;
        raw.into_iter().map(trade_from_raw).collect()
    }

    pub async fn get_orderbook(&self, symbol: &Symbol) -> Result<TossOrderbook, BrokerError> {
        let raw: RawOrderbook = self
            .transport
            .get(
                ORDERBOOK_ENDPOINT,
                &[("symbol", symbol.as_str().to_owned())],
                None,
            )
            .await?;
        orderbook_from_raw(raw)
    }

    pub async fn get_price_limits(&self, symbol: &Symbol) -> Result<TossPriceLimits, BrokerError> {
        let raw: RawPriceLimits = self
            .transport
            .get(
                PRICE_LIMITS_ENDPOINT,
                &[("symbol", symbol.as_str().to_owned())],
                None,
            )
            .await?;
        price_limits_from_raw(raw)
    }

    pub async fn get_market_session(
        &self,
        trading_day: &str,
    ) -> Result<MarketSession, BrokerError> {
        let api_day = api_date_from_trading_day(trading_day)?;
        let endpoint = self.market_country.calendar_endpoint();
        let calendar: RawMarketCalendar = self
            .transport
            .get(endpoint, &[("date", api_day)], None)
            .await?;
        let is_open = match self.market_country {
            TossMarketCountry::Kr => calendar
                .today
                .integrated
                .as_ref()
                .is_some_and(has_non_null_session),
            TossMarketCountry::Us => {
                calendar.today.day_market.is_some()
                    || calendar.today.pre_market.is_some()
                    || calendar.today.regular_market.is_some()
                    || calendar.today.after_market.is_some()
            }
        };
        Ok(MarketSession {
            trading_day: trading_day.to_owned(),
            is_open,
        })
    }

    pub async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        let page: RawOrderPage = self
            .transport
            .get(
                ORDERS_ENDPOINT,
                &[("status", "OPEN".to_owned())],
                Some(self.account_seq),
            )
            .await?;
        page.orders
            .into_iter()
            .map(|order| order_status_from_raw(self.account_seq, order))
            .collect()
    }

    pub async fn list_fills(&self, _day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        Ok(Vec::new())
    }

    pub async fn get_order(&self, query: OrderQuery) -> Result<OrderStatus, BrokerError> {
        let order_id =
            query.reference.broker_order_id.as_ref().ok_or_else(|| {
                BrokerError::validation("Toss order query requires broker_order_id")
            })?;
        if order_id.as_str().contains('/') || order_id.as_str().chars().any(char::is_whitespace) {
            return Err(BrokerError::validation(
                "Toss broker_order_id must not contain slash or whitespace",
            ));
        }
        let path = format!("/api/v1/orders/{}", order_id.as_str());
        let order: RawOrder = self
            .transport
            .get_path(ORDER_ENDPOINT, &path, &[], Some(self.account_seq))
            .await?;
        order_status_from_raw(self.account_seq, order)
    }

    pub async fn place_order(
        &self,
        _order: CreateOrderRequest,
    ) -> Result<OrderSubmission, BrokerError> {
        Err(toss_write_safety_halt("create order"))
    }

    pub async fn cancel_order(
        &self,
        _request: CancelOrderRequest,
    ) -> Result<CancelResult, BrokerError> {
        Err(toss_write_safety_halt("cancel order"))
    }

    pub async fn amend_order(
        &self,
        _request: AmendOrderRequest,
    ) -> Result<AmendResult, BrokerError> {
        Err(toss_write_safety_halt("modify order"))
    }

    pub async fn reconcile_order(
        &self,
        _client_order_id: &ClientOrderId,
        _lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError> {
        Err(BrokerError::safety_halt(
            "Toss Open API reconciliation is not enabled in this read-only increment",
        ))
    }

    pub async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        Ok(BrokerHealth {
            environment: trading_agent_kis_client::BrokerEnvironment::Live,
            writable: false,
        })
    }

    async fn get_holdings(
        &self,
        symbol: Option<&Symbol>,
    ) -> Result<RawHoldingsOverview, BrokerError> {
        let query = symbol
            .map(|symbol| vec![("symbol", symbol.as_str().to_owned())])
            .unwrap_or_default();
        self.transport
            .get(HOLDINGS_ENDPOINT, &query, Some(self.account_seq))
            .await
    }
}

#[async_trait]
impl BrokerClient for TossBrokerClient {
    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        TossBrokerClient::get_quote(self, symbol).await
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        TossBrokerClient::get_positions(self).await
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        TossBrokerClient::get_cash_balance(self).await
    }

    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        TossBrokerClient::get_buying_power(self, symbol).await
    }

    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError> {
        TossBrokerClient::place_order(self, order).await
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        TossBrokerClient::cancel_order(self, request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        TossBrokerClient::amend_order(self, request).await
    }

    async fn get_order(&self, query: OrderQuery) -> Result<OrderStatus, BrokerError> {
        TossBrokerClient::get_order(self, query).await
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        TossBrokerClient::list_open_orders(self).await
    }

    async fn list_orders(&self, _day: TradingDay) -> Result<Vec<OrderStatus>, BrokerError> {
        TossBrokerClient::list_open_orders(self).await
    }

    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        TossBrokerClient::list_fills(self, day).await
    }

    async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError> {
        TossBrokerClient::reconcile_order(self, client_order_id, lookback_days).await
    }

    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        TossBrokerClient::ping(self).await
    }
}

pub(crate) fn synthetic_client_order_id(order_id: &str) -> Result<ClientOrderId, BrokerError> {
    let sanitized = order_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    ClientOrderId::new(format!("toss-{sanitized}"))
}

fn toss_write_safety_halt(operation: &str) -> BrokerError {
    BrokerError::safety_halt(format!(
        "Toss Open API {operation} is disabled in the safe read-only increment"
    ))
}

fn validate_symbol_batch(
    label: &'static str,
    symbols: &[Symbol],
    max: usize,
) -> Result<(), BrokerError> {
    if symbols.is_empty() {
        return Err(BrokerError::validation(format!(
            "{label} requires at least one symbol"
        )));
    }
    if symbols.len() > max {
        return Err(BrokerError::validation(format!(
            "{label} supports at most {max} symbols"
        )));
    }
    Ok(())
}

fn validate_count(label: &'static str, count: u16, min: u16, max: u16) -> Result<(), BrokerError> {
    if !(min..=max).contains(&count) {
        return Err(BrokerError::validation(format!(
            "{label} must be between {min} and {max}"
        )));
    }
    Ok(())
}

fn validate_path_symbol(label: &'static str, symbol: &Symbol) -> Result<(), BrokerError> {
    if symbol.as_str().contains('/') || symbol.as_str().chars().any(char::is_whitespace) {
        return Err(BrokerError::validation(format!(
            "{label} must not contain slash or whitespace"
        )));
    }
    Ok(())
}

fn joined_symbols(symbols: &[Symbol]) -> String {
    symbols
        .iter()
        .map(Symbol::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

fn stock_info_from_raw(raw: RawStockInfo) -> Result<TossStockInfo, BrokerError> {
    Ok(TossStockInfo {
        symbol: Symbol::new(raw.symbol)?,
        name: raw.name,
        english_name: raw.english_name,
        isin_code: raw.isin_code,
        market: raw.market,
        security_type: raw.security_type,
        is_common_share: raw.is_common_share,
        status: raw.status,
        currency: raw.currency,
        list_date: raw.list_date,
        delist_date: raw.delist_date,
        shares_outstanding: decimal_from_str(
            "stocks[].sharesOutstanding",
            &raw.shares_outstanding,
        )?,
        leverage_factor: optional_decimal_from_str("stocks[].leverageFactor", raw.leverage_factor)?,
        korean_market_detail: raw.korean_market_detail.map(kr_market_detail_from_raw),
    })
}

fn kr_market_detail_from_raw(raw: RawKrMarketDetail) -> TossKrMarketDetail {
    TossKrMarketDetail {
        liquidation_trading: raw.liquidation_trading,
        nxt_supported: raw.nxt_supported,
        krx_trading_suspended: raw.krx_trading_suspended,
        nxt_trading_suspended: raw.nxt_trading_suspended,
    }
}

fn candle_page_from_raw(raw: RawCandlePage) -> Result<TossCandlePage, BrokerError> {
    Ok(TossCandlePage {
        candles: raw
            .candles
            .into_iter()
            .map(candle_from_raw)
            .collect::<Result<Vec<_>, _>>()?,
        next_before: raw.next_before,
    })
}

fn candle_from_raw(raw: RawCandle) -> Result<TossCandle, BrokerError> {
    Ok(TossCandle {
        timestamp: raw.timestamp,
        open_price: decimal_from_str("candles[].openPrice", &raw.open_price)?,
        high_price: decimal_from_str("candles[].highPrice", &raw.high_price)?,
        low_price: decimal_from_str("candles[].lowPrice", &raw.low_price)?,
        close_price: decimal_from_str("candles[].closePrice", &raw.close_price)?,
        volume: decimal_from_str("candles[].volume", &raw.volume)?,
        currency: raw.currency,
    })
}

fn trade_from_raw(raw: RawTrade) -> Result<TossTrade, BrokerError> {
    Ok(TossTrade {
        price: decimal_from_str("trades[].price", &raw.price)?,
        volume: decimal_from_str("trades[].volume", &raw.volume)?,
        timestamp: raw.timestamp,
        currency: raw.currency,
    })
}

fn orderbook_from_raw(raw: RawOrderbook) -> Result<TossOrderbook, BrokerError> {
    Ok(TossOrderbook {
        timestamp: raw.timestamp,
        currency: raw.currency,
        asks: raw
            .asks
            .into_iter()
            .map(orderbook_entry_from_raw)
            .collect::<Result<Vec<_>, _>>()?,
        bids: raw
            .bids
            .into_iter()
            .map(orderbook_entry_from_raw)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn orderbook_entry_from_raw(raw: RawOrderbookEntry) -> Result<TossOrderbookEntry, BrokerError> {
    Ok(TossOrderbookEntry {
        price: decimal_from_str("orderbook.price", &raw.price)?,
        volume: decimal_from_str("orderbook.volume", &raw.volume)?,
    })
}

fn price_limits_from_raw(raw: RawPriceLimits) -> Result<TossPriceLimits, BrokerError> {
    Ok(TossPriceLimits {
        timestamp: raw.timestamp,
        upper_limit_price: optional_decimal_from_str(
            "priceLimits.upperLimitPrice",
            raw.upper_limit_price,
        )?,
        lower_limit_price: optional_decimal_from_str(
            "priceLimits.lowerLimitPrice",
            raw.lower_limit_price,
        )?,
        currency: raw.currency,
    })
}

fn optional_decimal_from_str(
    field: &'static str,
    value: Option<String>,
) -> Result<Option<Decimal>, BrokerError> {
    value
        .as_deref()
        .map(|value| decimal_from_str(field, value))
        .transpose()
}

fn api_date_from_trading_day(trading_day: &str) -> Result<String, BrokerError> {
    if trading_day.len() != 8
        || !trading_day
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(BrokerError::validation(
            "Toss market calendar trading_day must be formatted as YYYYMMDD",
        ));
    }
    Ok(format!(
        "{}-{}-{}",
        &trading_day[0..4],
        &trading_day[4..6],
        &trading_day[6..8]
    ))
}

fn has_non_null_session(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.values().any(|value| !value.is_null()))
}

#[derive(Debug, Deserialize)]
struct RawHoldingsOverview {
    items: Vec<RawHoldingItem>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawHoldingItem {
    pub symbol: String,
    pub quantity: String,
    #[serde(rename = "averagePurchasePrice")]
    pub average_purchase_price: String,
}

#[derive(Debug, Deserialize)]
struct RawBuyingPower {
    #[serde(rename = "cashBuyingPower")]
    cash_buying_power: String,
}

#[derive(Debug, Deserialize)]
struct RawPrice {
    symbol: String,
    #[serde(rename = "lastPrice")]
    last_price: String,
}

#[derive(Debug, Deserialize)]
struct RawStockInfo {
    symbol: String,
    name: String,
    #[serde(rename = "englishName")]
    english_name: String,
    #[serde(rename = "isinCode")]
    isin_code: String,
    market: String,
    #[serde(rename = "securityType")]
    security_type: String,
    #[serde(rename = "isCommonShare")]
    is_common_share: bool,
    status: String,
    currency: String,
    #[serde(rename = "listDate")]
    list_date: Option<String>,
    #[serde(rename = "delistDate")]
    delist_date: Option<String>,
    #[serde(rename = "sharesOutstanding")]
    shares_outstanding: String,
    #[serde(rename = "leverageFactor")]
    leverage_factor: Option<String>,
    #[serde(rename = "koreanMarketDetail")]
    korean_market_detail: Option<RawKrMarketDetail>,
}

#[derive(Debug, Deserialize)]
struct RawKrMarketDetail {
    #[serde(rename = "liquidationTrading")]
    liquidation_trading: bool,
    #[serde(rename = "nxtSupported")]
    nxt_supported: bool,
    #[serde(rename = "krxTradingSuspended")]
    krx_trading_suspended: bool,
    #[serde(rename = "nxtTradingSuspended")]
    nxt_trading_suspended: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawCandlePage {
    candles: Vec<RawCandle>,
    #[serde(rename = "nextBefore")]
    next_before: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCandle {
    timestamp: String,
    #[serde(rename = "openPrice")]
    open_price: String,
    #[serde(rename = "highPrice")]
    high_price: String,
    #[serde(rename = "lowPrice")]
    low_price: String,
    #[serde(rename = "closePrice")]
    close_price: String,
    volume: String,
    currency: String,
}

#[derive(Debug, Deserialize)]
struct RawTrade {
    price: String,
    volume: String,
    timestamp: String,
    currency: String,
}

#[derive(Debug, Deserialize)]
struct RawOrderbook {
    timestamp: Option<String>,
    currency: String,
    asks: Vec<RawOrderbookEntry>,
    bids: Vec<RawOrderbookEntry>,
}

#[derive(Debug, Deserialize)]
struct RawOrderbookEntry {
    price: String,
    volume: String,
}

#[derive(Debug, Deserialize)]
struct RawPriceLimits {
    timestamp: String,
    #[serde(rename = "upperLimitPrice")]
    upper_limit_price: Option<String>,
    #[serde(rename = "lowerLimitPrice")]
    lower_limit_price: Option<String>,
    currency: String,
}

#[derive(Debug, Deserialize)]
struct RawMarketCalendar {
    today: RawMarketDay,
}

#[derive(Debug, Deserialize)]
struct RawMarketDay {
    #[serde(default)]
    integrated: Option<Value>,
    #[serde(default, rename = "dayMarket")]
    day_market: Option<Value>,
    #[serde(default, rename = "preMarket")]
    pre_market: Option<Value>,
    #[serde(default, rename = "regularMarket")]
    regular_market: Option<Value>,
    #[serde(default, rename = "afterMarket")]
    after_market: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RawOrderPage {
    orders: Vec<RawOrder>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawOrder {
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub symbol: String,
    pub side: String,
    pub status: String,
    pub price: Option<String>,
    pub quantity: String,
    #[serde(rename = "orderedAt")]
    pub ordered_at: String,
    pub execution: RawOrderExecution,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawOrderExecution {
    #[serde(rename = "filledQuantity")]
    pub filled_quantity: String,
}
