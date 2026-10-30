mod support;

use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use trading_agent_kis_client::auth::{AccessToken, KisAuthConfig, KisCredential};
use trading_agent_kis_client::client::{KisBrokerClient, KisClientBuilder};
use trading_agent_kis_client::domain::{
    AccountKey, BrokerEnvironment, CurrencyCode, ExchangeCode, OverseasStock,
    OverseasStockBalanceRequest, OverseasStockOrderHistoryRequest, OverseasStockOrderRequest,
    OverseasStockOrderStatusFilter, Quantity, Symbol, TradingDay,
};
use trading_agent_kis_client::journal::BrokerCommandType;
use trading_agent_kis_client::safety::{SingleWriterLockConfig, SingleWriterLockScope};
use trading_agent_kis_client::transport::{KisRateLimitConfig, KisTransportConfig};
use trading_agent_kis_client::{BrokerError, ClientOrderId};

#[test]
fn primary_public_api_is_available_without_deep_broker_modules() {
    let credential = KisCredential::new("app-key", "app-secret").expect("credential");
    let access_token = AccessToken::new("access-token").expect("access token");
    let auth_config =
        KisAuthConfig::new(BrokerEnvironment::Paper, credential.clone()).expect("auth config");
    let transport_config =
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config");
    let lock_config = SingleWriterLockConfig {
        lock_dir: "state/locks".into(),
        environment: BrokerEnvironment::Paper,
        account_key: AccountKey::new("12345678-01").expect("account key"),
        scope: SingleWriterLockScope::DomesticStock,
    };

    assert_eq!(auth_config.environment, BrokerEnvironment::Paper);
    assert_eq!(transport_config.environment, BrokerEnvironment::Paper);
    assert_eq!(lock_config.environment, BrokerEnvironment::Paper);
    assert_eq!(lock_config.scope, SingleWriterLockScope::DomesticStock);
    assert_eq!(BrokerCommandType::Place.as_str(), "place");
    assert_eq!(Quantity::new(3).get(), 3);
    assert_eq!(Symbol::new("005930").expect("symbol").as_str(), "005930");
    let overseas_stock = OverseasStock::new(
        ExchangeCode::new("NAS").expect("exchange"),
        Symbol::new("AAPL").expect("symbol"),
    );
    assert_eq!(overseas_stock.exchange.as_str(), "NAS");
    let overseas_balance = OverseasStockBalanceRequest::new(
        ExchangeCode::new("NASD").expect("exchange"),
        CurrencyCode::new("USD").expect("currency"),
    );
    assert_eq!(overseas_balance.currency.as_str(), "USD");
    let overseas_order = OverseasStockOrderRequest::cash_limit(
        ClientOrderId::new("coid-public-overseas").expect("client order id"),
        AccountKey::new("12345678-01").expect("account key"),
        OverseasStock::new(
            ExchangeCode::new("NASD").expect("exchange"),
            Symbol::new("AAPL").expect("symbol"),
        ),
        trading_agent_kis_client::domain::OrderSide::Buy,
        Quantity::new(1),
        rust_decimal::Decimal::new(19825, 2),
    )
    .expect("overseas order");
    assert_eq!(overseas_order.instrument.symbol.as_str(), "AAPL");
    let mut overseas_history = OverseasStockOrderHistoryRequest::new(
        Some(ExchangeCode::new("NASD").expect("exchange")),
        TradingDay(time::macros::date!(2026 - 04 - 23)),
        TradingDay(time::macros::date!(2026 - 04 - 23)),
    )
    .expect("history request");
    overseas_history.status = OverseasStockOrderStatusFilter::Filled;
    assert_eq!(overseas_history.status.as_kis_code(), "01");
    assert!(matches!(
        BrokerError::DuplicateIntent {
            client_order_id: ClientOrderId::new("coid-public").expect("client order id"),
        },
        BrokerError::DuplicateIntent { .. }
    ));
    assert!(KisRateLimitConfig::default().auth_min_interval.is_zero());
    assert!(std::any::type_name::<KisBrokerClient>().contains("KisBrokerClient"));
}

#[tokio::test]
async fn builder_constructs_client_by_issuing_access_token() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("auth/token_success.json"),
    ));
    let credential = KisCredential::new("app-key", "app-secret").expect("credential");
    let client = KisClientBuilder::new(BrokerEnvironment::Paper, credential)
        .with_base_url(server.base_url())
        .with_account_key(AccountKey::new("12345678-01").expect("account key"))
        .build()
        .await
        .expect("client");

    assert!(std::any::type_name_of_val(&client).contains("KisBrokerClient"));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/oauth2/tokenP");
}
