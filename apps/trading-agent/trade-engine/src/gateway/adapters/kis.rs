use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use trading_agent_kis_client::config::{BrokerRuntimeConfig, MarketSessionWriteGate};
use trading_agent_kis_client::domain::BrokerHealth;
use trading_agent_kis_client::safety::{
    SingleWriterLock, SingleWriterLockConfig, SingleWriterLockScope,
};
use trading_agent_kis_client::transport::KisRateLimitConfig;
use trading_agent_kis_client::{
    BrokerEnvironment, BrokerError, ClientOrderId, CurrencyCode, ExchangeCode, KisBrokerClient,
    KisClientBuilder, OrderType, OverseasStock, OverseasStockBalanceRequest,
    OverseasStockBuyingPowerRequest, OverseasStockOrderHistoryRequest, OverseasStockOrderRequest,
    ReconciliationResult, UsEquityOrderSession,
};

use crate::config::{EngineConfig, MarketKind};
use crate::error::TradeEngineError;
use crate::gateway::traits::TradingGateway;
use crate::gateway::types::{
    AccountSnapshot, AmendOrderRequest, AmendResult, BuyingPower, CancelOrderRequest, CancelResult,
    CashBalance, CreateOrderRequest, Fill, MarketScope, MarketSession, OrderStatus,
    OrderSubmission, Position, Quote, Symbol, TradingDay,
};
use crate::order_price::normalize_overseas_stock_limit_price;
use crate::reconciliation::ReconciliationGateway;
use crate::storage::SqliteBrokerCommandJournal;

pub enum KisGateway {
    DomesticStock(KisDomesticStockGateway),
    OverseasStock(KisOverseasStockGateway),
}

pub struct KisDomesticStockGateway {
    client: KisBrokerClient,
}

pub struct KisOverseasStockGateway {
    client: KisBrokerClient,
    environment: BrokerEnvironment,
    exchange: ExchangeCode,
    currency: CurrencyCode,
    allowed_us_equity_order_sessions: Vec<UsEquityOrderSession>,
    cash_probe_symbol: Symbol,
}

const READ_ONLY_KIS_REQUEST_INTERVAL: Duration = Duration::from_millis(50);
const READ_HEAVY_KIS_REQUEST_INTERVAL: Duration = Duration::from_millis(1_100);
const READ_ONLY_KIS_THROTTLE_PAUSE: Duration = Duration::from_millis(1_100);
const READ_ONLY_KIS_RETRY_BACKOFF: Duration = Duration::from_millis(200);
const WRITE_KIS_REQUEST_INTERVAL: Duration = Duration::from_millis(100);
const WRITE_KIS_RETRY_BACKOFF: Duration = Duration::from_millis(200);
const DEFAULT_OVERSEAS_CASH_PROBE_SYMBOL: &str = "AAPL";

impl KisGateway {
    pub fn domestic_stock(client: KisBrokerClient) -> Self {
        Self::DomesticStock(KisDomesticStockGateway::new(client))
    }

    pub fn overseas_stock(
        client: KisBrokerClient,
        environment: BrokerEnvironment,
        exchange: ExchangeCode,
        currency: CurrencyCode,
        allowed_us_equity_order_sessions: Vec<UsEquityOrderSession>,
    ) -> Self {
        Self::OverseasStock(KisOverseasStockGateway::new(
            client,
            environment,
            exchange,
            currency,
            allowed_us_equity_order_sessions,
        ))
    }
}

impl KisDomesticStockGateway {
    pub fn new(client: KisBrokerClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl TradingGateway for KisDomesticStockGateway {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        self.client.ping().await
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        self.client.get_account_snapshot().await
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        self.client.get_cash_balance().await
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        self.client.get_positions().await
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        self.client.get_quote(symbol).await
    }

    async fn get_market_session(&self, trading_day: &str) -> Result<MarketSession, BrokerError> {
        self.client.get_market_session(trading_day).await
    }

    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        self.client.get_buying_power(symbol).await
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        self.client.list_open_orders().await
    }

    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        self.client.list_fills(day).await
    }

    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError> {
        self.client.place_order(order).await
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        self.client.cancel_order(request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        self.client.amend_order(request).await
    }
}

#[async_trait]
impl TradingGateway for KisOverseasStockGateway {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        Ok(BrokerHealth {
            environment: self.environment,
            writable: false,
        })
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        let snapshot = self
            .client
            .get_overseas_stock_account_snapshot(&self.balance_request())
            .await?;
        let positions = snapshot
            .positions
            .into_iter()
            .map(|position| Position {
                symbol: position.instrument.symbol,
                quantity: position.quantity,
                average_price: position.average_price,
            })
            .collect();
        Ok(AccountSnapshot {
            cash: self.get_cash_balance().await?,
            positions,
        })
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        let buying_power = self.get_buying_power(Some(&self.cash_probe_symbol)).await?;
        Ok(CashBalance {
            cash: buying_power.available_cash,
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        Ok(self.get_account_snapshot().await?.positions)
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        let quote = self
            .client
            .get_overseas_stock_quote(&self.quote_instrument(symbol)?)
            .await?;
        Ok(Quote {
            symbol: quote.instrument.symbol,
            full_name: None,
            last_price: quote.last_price,
        })
    }

    async fn get_market_session(&self, trading_day: &str) -> Result<MarketSession, BrokerError> {
        validate_trading_day(trading_day)?;
        if self.environment == BrokerEnvironment::Live {
            let is_open = self
                .client
                .is_overseas_stock_order_session_open(
                    &self.exchange,
                    &self.allowed_us_equity_order_sessions,
                )
                .await?;
            return Ok(MarketSession {
                trading_day: trading_day.to_owned(),
                is_open,
            });
        }
        Ok(MarketSession {
            trading_day: trading_day.to_owned(),
            is_open: false,
        })
    }

    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        let symbol = symbol.ok_or_else(|| {
            BrokerError::validation("KIS overseas buying power lookup requires a symbol")
        })?;
        let quote = self
            .client
            .get_overseas_stock_quote(&self.quote_instrument(symbol)?)
            .await?;
        let request =
            OverseasStockBuyingPowerRequest::new(self.instrument(symbol), quote.last_price)?;
        let buying_power = self
            .client
            .get_overseas_stock_buying_power(&request)
            .await?;
        Ok(BuyingPower {
            available_cash: buying_power.available_cash,
        })
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        self.client
            .list_overseas_stock_open_orders(&self.exchange)
            .await
    }

    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        let request = OverseasStockOrderHistoryRequest::new(Some(self.exchange.clone()), day, day)?;
        self.client.list_overseas_stock_fills(&request).await
    }

    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError> {
        self.client
            .place_overseas_stock_order(self.overseas_order_request(order)?)
            .await
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        self.client.cancel_overseas_stock_order(request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        self.client.amend_overseas_stock_order(request).await
    }
}

impl KisOverseasStockGateway {
    pub fn new(
        client: KisBrokerClient,
        environment: BrokerEnvironment,
        exchange: ExchangeCode,
        currency: CurrencyCode,
        allowed_us_equity_order_sessions: Vec<UsEquityOrderSession>,
    ) -> Self {
        Self {
            client,
            environment,
            exchange,
            currency,
            allowed_us_equity_order_sessions,
            cash_probe_symbol: default_overseas_cash_probe_symbol(),
        }
    }

    fn balance_request(&self) -> OverseasStockBalanceRequest {
        OverseasStockBalanceRequest::new(self.exchange.clone(), self.currency.clone())
    }

    fn instrument(&self, symbol: &Symbol) -> OverseasStock {
        OverseasStock::new(self.exchange.clone(), symbol.clone())
    }

    fn quote_instrument(&self, symbol: &Symbol) -> Result<OverseasStock, BrokerError> {
        Ok(OverseasStock::new(
            overseas_quote_exchange(&self.exchange)?,
            symbol.clone(),
        ))
    }

    fn overseas_order_request(
        &self,
        order: CreateOrderRequest,
    ) -> Result<OverseasStockOrderRequest, BrokerError> {
        let limit_price = match (order.order_type, order.limit_price) {
            (OrderType::Limit, Some(price)) => price,
            (OrderType::Limit, None) => {
                return Err(BrokerError::validation(
                    "overseas stock limit order requires price",
                ));
            }
            (OrderType::Market, _) => {
                return Err(BrokerError::validation(
                    "trade-engine overseas stock orders currently support limit orders only",
                ));
            }
        };

        let limit_price = normalize_overseas_stock_limit_price(order.side, limit_price);

        OverseasStockOrderRequest::cash_limit(
            order.client_order_id,
            order.account_key,
            self.instrument(&order.symbol),
            order.side,
            order.quantity,
            limit_price,
        )
    }
}

fn overseas_quote_exchange(exchange: &ExchangeCode) -> Result<ExchangeCode, BrokerError> {
    // KIS uses separate exchange code families for overseas quote and trading APIs.
    match exchange.as_str() {
        "NASD" => ExchangeCode::new("NAS"),
        "NYSE" => ExchangeCode::new("NYS"),
        "AMEX" => ExchangeCode::new("AMS"),
        value => ExchangeCode::new(value),
    }
}

#[async_trait]
impl TradingGateway for KisGateway {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.ping().await,
            Self::OverseasStock(gateway) => gateway.ping().await,
        }
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.get_account_snapshot().await,
            Self::OverseasStock(gateway) => gateway.get_account_snapshot().await,
        }
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.get_cash_balance().await,
            Self::OverseasStock(gateway) => gateway.get_cash_balance().await,
        }
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.get_positions().await,
            Self::OverseasStock(gateway) => gateway.get_positions().await,
        }
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.get_quote(symbol).await,
            Self::OverseasStock(gateway) => gateway.get_quote(symbol).await,
        }
    }

    async fn get_market_session(&self, trading_day: &str) -> Result<MarketSession, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.get_market_session(trading_day).await,
            Self::OverseasStock(gateway) => gateway.get_market_session(trading_day).await,
        }
    }

    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.get_buying_power(symbol).await,
            Self::OverseasStock(gateway) => gateway.get_buying_power(symbol).await,
        }
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.list_open_orders().await,
            Self::OverseasStock(gateway) => gateway.list_open_orders().await,
        }
    }

    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.list_fills(day).await,
            Self::OverseasStock(gateway) => gateway.list_fills(day).await,
        }
    }

    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.place_order(order).await,
            Self::OverseasStock(gateway) => gateway.place_order(order).await,
        }
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.cancel_order(request).await,
            Self::OverseasStock(gateway) => gateway.cancel_order(request).await,
        }
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => gateway.amend_order(request).await,
            Self::OverseasStock(gateway) => gateway.amend_order(request).await,
        }
    }
}

#[async_trait]
impl ReconciliationGateway for KisGateway {
    async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError> {
        match self {
            Self::DomesticStock(gateway) => {
                gateway
                    .client
                    .reconcile_order(client_order_id, lookback_days)
                    .await
            }
            Self::OverseasStock(gateway) => {
                gateway
                    .client
                    .reconcile_order(client_order_id, lookback_days)
                    .await
            }
        }
    }
}

pub struct KisGatewayBundle {
    pub gateway: KisGateway,
    pub account_key: trading_agent_kis_client::AccountKey,
    pub account_suffix: String,
}

pub async fn build_read_only_kis_gateway(
    config: &EngineConfig,
) -> Result<KisGatewayBundle, TradeEngineError> {
    config.validate_read_only_smoke()?;

    let credential = config.kis.credentials_from_env()?;
    let account_key = config.kis.account_key()?;
    let account_suffix = EngineConfig::account_suffix_from_account_key(account_key.as_str());
    let runtime_config = BrokerRuntimeConfig {
        read_only: true,
        enable_live_trading: false,
        confirmed_account_suffix: None,
        allowed_live_accounts: Vec::new(),
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: config.market.allowed_us_equity_order_sessions()?,
    };

    let environment = config.kis.environment.as_broker_environment();
    let builder = KisClientBuilder::new(environment, credential)
        .with_account_key(account_key.clone())
        .with_token_cache_path(config.kis.token_cache_path.clone())
        .with_runtime_config(runtime_config)
        .with_shared_rate_limit_path(shared_rate_limit_path(config))
        .with_rate_limits(read_only_kis_rate_limits())
        .with_retry_policy(3, READ_ONLY_KIS_RETRY_BACKOFF);

    let client = builder.build().await?;
    let gateway = gateway_for_market(config, client, environment)?;
    Ok(KisGatewayBundle {
        gateway,
        account_key,
        account_suffix,
    })
}

pub async fn build_write_enabled_kis_gateway(
    config: &EngineConfig,
) -> Result<KisGatewayBundle, TradeEngineError> {
    config.validate_write_runtime_safety()?;

    let credential = config.kis.credentials_from_env()?;
    let account_key = config.kis.account_key()?;
    let account_suffix = EngineConfig::account_suffix_from_account_key(account_key.as_str());
    let main_db_path = config.main_db_path()?;
    let journal = Arc::new(SqliteBrokerCommandJournal::open(main_db_path)?);
    let environment = config.kis.environment.as_broker_environment();
    let single_writer_lock = if environment == trading_agent_kis_client::BrokerEnvironment::Live {
        Some(Arc::new(SingleWriterLock::acquire(
            SingleWriterLockConfig {
                lock_dir: single_writer_lock_dir(config),
                environment,
                account_key: account_key.clone(),
                scope: single_writer_lock_scope(&config.market.scope()?),
            },
        )?))
    } else {
        None
    };
    let runtime_config = BrokerRuntimeConfig {
        read_only: false,
        enable_live_trading: config.risk.enable_live_trading,
        confirmed_account_suffix: Some(account_suffix.clone()),
        allowed_live_accounts: vec![account_key.as_str().to_owned()],
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: config.market.allowed_us_equity_order_sessions()?,
    };

    let mut builder = KisClientBuilder::new(environment, credential)
        .with_account_key(account_key.clone())
        .with_token_cache_path(config.kis.token_cache_path.clone())
        .with_runtime_config(runtime_config)
        .with_journal(journal)
        .with_shared_rate_limit_path(shared_rate_limit_path(config))
        .with_rate_limits(write_enabled_kis_rate_limits())
        .with_retry_policy(1, WRITE_KIS_RETRY_BACKOFF);
    if let Some(single_writer_lock) = single_writer_lock {
        builder = builder.with_single_writer_lock(single_writer_lock);
    }

    let client = builder.build().await?;
    let gateway = gateway_for_market(config, client, environment)?;
    Ok(KisGatewayBundle {
        gateway,
        account_key,
        account_suffix,
    })
}

fn single_writer_lock_dir(config: &EngineConfig) -> PathBuf {
    config
        .main_db_path()
        .ok()
        .and_then(|path| path.parent())
        .map(|parent| parent.join("locks"))
        .unwrap_or_else(|| PathBuf::from(".local/state/trade-engine/locks"))
}

fn shared_rate_limit_path(config: &EngineConfig) -> PathBuf {
    single_writer_lock_dir(config).join("kis-rate-limit.sqlite3")
}

fn read_only_kis_rate_limits() -> KisRateLimitConfig {
    KisRateLimitConfig {
        global_min_interval: READ_ONLY_KIS_REQUEST_INTERVAL,
        read_heavy_min_interval: READ_HEAVY_KIS_REQUEST_INTERVAL,
        throttle_circuit_open_for: READ_ONLY_KIS_THROTTLE_PAUSE,
        ..KisRateLimitConfig::default()
    }
}

fn write_enabled_kis_rate_limits() -> KisRateLimitConfig {
    KisRateLimitConfig {
        global_min_interval: READ_ONLY_KIS_REQUEST_INTERVAL,
        read_heavy_min_interval: READ_HEAVY_KIS_REQUEST_INTERVAL,
        write_order_min_interval: WRITE_KIS_REQUEST_INTERVAL,
        throttle_circuit_open_for: READ_ONLY_KIS_THROTTLE_PAUSE,
        ..KisRateLimitConfig::default()
    }
}

fn single_writer_lock_scope(market_scope: &MarketScope) -> SingleWriterLockScope {
    match market_scope {
        MarketScope::DomesticStock => SingleWriterLockScope::DomesticStock,
        MarketScope::OverseasStock { .. } => SingleWriterLockScope::OverseasStock,
    }
}

fn gateway_for_market(
    config: &EngineConfig,
    client: KisBrokerClient,
    environment: BrokerEnvironment,
) -> Result<KisGateway, TradeEngineError> {
    match config.market.kind {
        MarketKind::DomesticStock => Ok(KisGateway::domestic_stock(client)),
        MarketKind::OverseasStock => match config.market.scope()? {
            MarketScope::OverseasStock { exchange, currency } => Ok(KisGateway::overseas_stock(
                client,
                environment,
                exchange,
                currency,
                config.market.allowed_us_equity_order_sessions()?,
            )),
            MarketScope::DomesticStock => Err(TradeEngineError::config(
                "overseas_stock market resolved to domestic scope",
            )),
        },
    }
}

fn validate_trading_day(trading_day: &str) -> Result<(), BrokerError> {
    if trading_day.len() != 8
        || !trading_day
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(BrokerError::validation(
            "trading_day must be formatted as YYYYMMDD",
        ));
    }
    Ok(())
}

fn default_overseas_cash_probe_symbol() -> Symbol {
    Symbol::new(DEFAULT_OVERSEAS_CASH_PROBE_SYMBOL)
        .expect("default overseas cash probe symbol is a valid KIS symbol")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rust_decimal::Decimal;
    use trading_agent_kis_client::auth::{AccessToken, KisCredential};
    use trading_agent_kis_client::transport::{KisTransport, KisTransportConfig};
    use trading_agent_kis_client::{
        AccountKey, ClientOrderId, OrderSide, OrderType, Quantity, Symbol,
    };

    use super::*;

    fn overseas_gateway() -> KisOverseasStockGateway {
        let credential = KisCredential::new("app-key", "app-secret").expect("credential");
        let access_token = AccessToken::new("access-token").expect("access token");
        let transport = KisTransport::new(
            KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
                .expect("transport config"),
        )
        .expect("transport");
        let client = KisBrokerClient::new_for_account(
            transport,
            AccountKey::new("12345678-01").expect("account key"),
        );
        KisOverseasStockGateway::new(
            client,
            BrokerEnvironment::Paper,
            ExchangeCode::new("NASD").expect("exchange"),
            CurrencyCode::new("USD").expect("currency"),
            BrokerRuntimeConfig::regular_us_equity_order_sessions(),
        )
    }

    #[tokio::test]
    async fn overseas_gateway_uses_fail_closed_market_session() {
        let session = overseas_gateway()
            .get_market_session("20260424")
            .await
            .expect("market session");

        assert_eq!(session.trading_day, "20260424");
        assert!(!session.is_open);
    }

    #[test]
    fn overseas_gateway_defaults_cash_probe_symbol_to_aapl() {
        let gateway = overseas_gateway();

        assert_eq!(gateway.cash_probe_symbol.as_str(), "AAPL");
    }

    #[test]
    fn kis_gateway_rate_limits_space_ledger_reads_more_conservatively() {
        let read_only = read_only_kis_rate_limits();
        assert_eq!(
            read_only.global_min_interval,
            READ_ONLY_KIS_REQUEST_INTERVAL
        );
        assert_eq!(
            read_only.read_heavy_min_interval,
            Duration::from_millis(1_100)
        );
        assert_eq!(
            read_only.throttle_circuit_open_for,
            READ_ONLY_KIS_THROTTLE_PAUSE
        );

        let write_enabled = write_enabled_kis_rate_limits();
        assert_eq!(
            write_enabled.global_min_interval,
            READ_ONLY_KIS_REQUEST_INTERVAL
        );
        assert_eq!(
            write_enabled.read_heavy_min_interval,
            Duration::from_millis(1_100)
        );
        assert_eq!(
            write_enabled.write_order_min_interval,
            WRITE_KIS_REQUEST_INTERVAL
        );
        assert_eq!(
            write_enabled.throttle_circuit_open_for,
            READ_ONLY_KIS_THROTTLE_PAUSE
        );
    }

    #[test]
    fn overseas_gateway_maps_limit_orders_to_exchange_scoped_kis_requests() {
        let order = CreateOrderRequest::cash_limit(
            ClientOrderId::new("overseas-limit-order").expect("client order id"),
            AccountKey::new("12345678-01").expect("account key"),
            Symbol::new("AAPL").expect("symbol"),
            OrderSide::Buy,
            Quantity::new(1),
            Decimal::new(19825, 2),
        )
        .expect("order");

        let overseas_order = overseas_gateway()
            .overseas_order_request(order)
            .expect("overseas order request");

        assert_eq!(overseas_order.instrument.exchange.as_str(), "NASD");
        assert_eq!(overseas_order.instrument.symbol.as_str(), "AAPL");
        assert_eq!(overseas_order.account_key.as_str(), "12345678-01");
        assert_eq!(overseas_order.order_type, OrderType::Limit);
        assert_eq!(overseas_order.limit_price, Some(Decimal::new(19825, 2)));
    }

    #[test]
    fn overseas_gateway_normalizes_sub_cent_us_limit_prices() {
        let sell_order = CreateOrderRequest::cash_limit(
            ClientOrderId::new("overseas-limit-sell").expect("client order id"),
            AccountKey::new("12345678-01").expect("account key"),
            Symbol::new("SMCI").expect("symbol"),
            OrderSide::Sell,
            Quantity::new(1),
            Decimal::new(35_4001, 4),
        )
        .expect("order");
        let buy_order = CreateOrderRequest::cash_limit(
            ClientOrderId::new("overseas-limit-buy").expect("client order id"),
            AccountKey::new("12345678-01").expect("account key"),
            Symbol::new("NAVN").expect("symbol"),
            OrderSide::Buy,
            Quantity::new(1),
            Decimal::new(21_8650, 4),
        )
        .expect("order");

        let sell = overseas_gateway()
            .overseas_order_request(sell_order)
            .expect("sell order request");
        let buy = overseas_gateway()
            .overseas_order_request(buy_order)
            .expect("buy order request");

        assert_eq!(sell.limit_price, Some(Decimal::new(35_40, 2)));
        assert_eq!(buy.limit_price, Some(Decimal::new(21_87, 2)));
    }

    #[test]
    fn overseas_gateway_maps_trading_exchange_to_quote_exchange() {
        let cases = [("NASD", "NAS"), ("NYSE", "NYS"), ("AMEX", "AMS")];

        for (trading_exchange, quote_exchange) in cases {
            let exchange = ExchangeCode::new(trading_exchange).expect("exchange");
            let quote_exchange_code = overseas_quote_exchange(&exchange).expect("quote exchange");

            assert_eq!(quote_exchange_code.as_str(), quote_exchange);
        }
    }

    #[test]
    fn overseas_gateway_rejects_non_limit_order_mapping_before_submit() {
        let order = CreateOrderRequest {
            client_order_id: ClientOrderId::new("overseas-market-order").expect("client order id"),
            account_key: AccountKey::new("12345678-01").expect("account key"),
            symbol: Symbol::new("AAPL").expect("symbol"),
            side: OrderSide::Buy,
            quantity: Quantity::new(1),
            order_type: OrderType::Market,
            limit_price: None,
        };

        let error = overseas_gateway()
            .overseas_order_request(order)
            .expect_err("market order is not supported for KIS overseas stock order");

        assert!(
            error
                .to_string()
                .contains("overseas stock orders currently support limit orders only")
        );
    }
}
