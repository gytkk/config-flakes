use async_trait::async_trait;
use trading_agent_kis_client::{
    AccountSnapshot, AmendOrderRequest, AmendResult, BrokerError, BrokerHealth, BuyingPower,
    CancelOrderRequest, CancelResult, CashBalance, CreateOrderRequest, Fill, MarketSession,
    OrderStatus, OrderSubmission, Position, Quote, Symbol, TradingDay,
};
use trading_agent_toss_client::{
    TossAuthClient, TossBrokerClient, TossMarketCountry, TossTransport, TossTransportConfig,
};

use crate::config::EngineConfig;
use crate::error::TradeEngineError;
use crate::gateway::traits::TradingGateway;
use crate::gateway::types::MarketScope;

pub struct TossGateway {
    client: TossBrokerClient,
}

impl TossGateway {
    pub fn new(client: TossBrokerClient) -> Self {
        Self { client }
    }
}

pub struct TossGatewayBundle {
    pub gateway: TossGateway,
    pub account_suffix: String,
}

pub async fn build_read_only_toss_gateway(
    config: &EngineConfig,
) -> Result<TossGatewayBundle, TradeEngineError> {
    config.validate_read_only_smoke()?;

    let credential = config.toss.credentials_from_env()?;
    let account_seq = config.toss.account_seq()?;
    let market_country = toss_market_country(config)?;
    let issued_token = TossAuthClient::new(&config.toss.base_url, credential.clone())?
        .issue_token()
        .await?;
    let transport = TossTransport::new(
        TossTransportConfig::new(credential, issued_token.access_token)?
            .with_base_url(config.toss.base_url.clone()),
    )?;
    let client = TossBrokerClient::new(transport, account_seq, market_country);

    Ok(TossGatewayBundle {
        gateway: TossGateway::new(client),
        account_suffix: account_seq.as_u64().to_string(),
    })
}

fn toss_market_country(config: &EngineConfig) -> Result<TossMarketCountry, TradeEngineError> {
    match config.market.scope()? {
        MarketScope::DomesticStock => Ok(TossMarketCountry::Kr),
        MarketScope::OverseasStock { currency, .. } if currency.as_str() == "USD" => {
            Ok(TossMarketCountry::Us)
        }
        MarketScope::OverseasStock { currency, .. } => Err(TradeEngineError::config(format!(
            "Toss overseas market support currently requires currency=USD, got {}",
            currency.as_str()
        ))),
    }
}

#[async_trait]
impl TradingGateway for TossGateway {
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
