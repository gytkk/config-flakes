mod support;

use rust_decimal::Decimal;
use support::fake_toss_server::{FakeResponse, FakeTossServer};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, BrokerError, BrokerOrderId, CancelOrderRequest, ClientOrderId,
    CreateOrderRequest, OrderQuery, OrderReference, OrderSide, Quantity, Symbol,
};
use trading_agent_toss_client::auth::{TossAccessToken, TossAuthClient, TossCredential};
use trading_agent_toss_client::client::{
    TossAccountSeq, TossBrokerClient, TossCandleInterval, TossCandleRequest, TossMarketCountry,
};
use trading_agent_toss_client::transport::{TossTransport, TossTransportConfig};

#[test]
fn access_token_debug_redacts_token_material() {
    let token = TossAccessToken::new("debug-access-token").expect("access token");

    let debug = format!("{token:?}");

    assert!(!debug.contains("debug-access-token"));
    assert!(debug.contains("***"));
}

#[tokio::test]
async fn issued_token_debug_redacts_token_material() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"access_token":"issued-debug-access-token","token_type":"Bearer","expires_in":86400}"#,
    ));
    let auth = TossAuthClient::new(
        server.base_url(),
        TossCredential::new("client-id", "client-secret").expect("credential"),
    )
    .expect("auth client");

    let token = auth.issue_token().await.expect("token");
    let debug = format!("{token:?}");

    assert!(!debug.contains("issued-debug-access-token"));
    assert!(debug.contains("***"));
    assert!(debug.contains("Bearer"));
    assert!(debug.contains("expires_in_seconds"));
}

#[test]
fn transport_debug_redacts_authorization_bearer_value() {
    let credential = TossCredential::new("client-id", "client-secret").expect("credential");
    let access_token = TossAccessToken::new("transport-debug-access-token").expect("access token");
    let transport = TossTransport::new(
        TossTransportConfig::new(credential, access_token)
            .expect("transport config")
            .with_base_url("http://127.0.0.1:65535"),
    )
    .expect("transport");

    let debug = format!("{transport:?}");

    assert!(!debug.contains("transport-debug-access-token"));
    assert!(!debug.contains("Bearer transport-debug-access-token"));
    assert!(debug.contains("***"));
}

fn client_for(base_url: &str, market_country: TossMarketCountry) -> TossBrokerClient {
    let credential = TossCredential::new("client-id", "client-secret").expect("credential");
    let access_token = TossAccessToken::new("access-token").expect("access token");
    let transport = TossTransport::new(
        TossTransportConfig::new(credential, access_token)
            .expect("transport config")
            .with_base_url(base_url),
    )
    .expect("transport");
    TossBrokerClient::new(
        transport,
        TossAccountSeq::new(7).expect("account seq"),
        market_country,
    )
}

#[tokio::test]
async fn token_issuance_uses_oauth2_client_credentials_form() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"access_token":"issued-access-token","token_type":"Bearer","expires_in":86400}"#,
    ));
    let auth = TossAuthClient::new(
        server.base_url(),
        TossCredential::new("client-id", "client-secret").expect("credential"),
    )
    .expect("auth client");

    let token = auth.issue_token().await.expect("token");

    assert_eq!(token.access_token.as_str(), "issued-access-token");
    assert_eq!(token.token_type.as_str(), "Bearer");
    assert_eq!(token.expires_in_seconds, 86_400);
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/oauth2/token");
    assert!(
        requests[0]
            .headers
            .get("content-type")
            .is_some_and(|value| value.starts_with("application/x-www-form-urlencoded"))
    );
    assert!(!requests[0].headers.contains_key("authorization"));
    assert!(requests[0].body.contains("grant_type=client_credentials"));
    assert!(requests[0].body.contains("client_id=client-id"));
    assert!(requests[0].body.contains("client_secret=client-secret"));
}

#[tokio::test]
async fn account_snapshot_uses_account_header_and_maps_holdings_and_buying_power() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":{"totalPurchaseAmount":{"krw":"6500000","usd":null},"marketValue":{"amount":{"krw":"7200000","usd":null},"amountAfterCost":{"krw":"7050000","usd":null}},"profitLoss":{"amount":{"krw":"700000","usd":null},"amountAfterCost":{"krw":"550000","usd":null},"rate":"0.1077","rateAfterCost":"0.0846"},"dailyProfitLoss":{"amount":{"krw":"100000","usd":null},"rate":"0.0141"},"items":[{"symbol":"005930","name":"Samsung Electronics","marketCountry":"KR","currency":"KRW","quantity":"100","lastPrice":"72000","averagePurchasePrice":"65000","marketValue":{"purchaseAmount":"6500000","amount":"7200000","amountAfterCost":"7050000"},"profitLoss":{"amount":"700000","amountAfterCost":"550000","rate":"0.1077","rateAfterCost":"0.0846"},"dailyProfitLoss":{"amount":"100000","rate":"0.0141"},"cost":{"commission":"14400","tax":"135600"}}]}}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":{"currency":"KRW","cashBuyingPower":"5000000"}}"#,
    ));

    let snapshot = client_for(server.base_url(), TossMarketCountry::Kr)
        .get_account_snapshot()
        .await
        .expect("account snapshot");

    assert_eq!(snapshot.cash.cash, Decimal::new(5_000_000, 0));
    assert_eq!(snapshot.positions.len(), 1);
    assert_eq!(snapshot.positions[0].symbol.as_str(), "005930");
    assert_eq!(snapshot.positions[0].quantity.get(), 100);
    assert_eq!(snapshot.positions[0].average_price, Decimal::new(65_000, 0));
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/api/v1/holdings");
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("Bearer access-token")
    );
    assert_eq!(
        requests[0]
            .headers
            .get("x-tossinvest-account")
            .map(String::as_str),
        Some("7")
    );
    assert_eq!(requests[1].path, "/api/v1/buying-power?currency=KRW");
}

#[tokio::test]
async fn quote_uses_single_symbol_prices_request_and_maps_response() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":[{"symbol":"AAPL","timestamp":"2026-03-25T22:30:00.456+09:00","lastPrice":"185.70","currency":"USD"}]}"#,
    ));

    let quote = client_for(server.base_url(), TossMarketCountry::Us)
        .get_quote(&Symbol::new("AAPL").expect("symbol"))
        .await
        .expect("quote");

    assert_eq!(quote.symbol.as_str(), "AAPL");
    assert_eq!(quote.full_name, None);
    assert_eq!(quote.last_price, Decimal::new(18_570, 2));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/api/v1/prices?symbols=AAPL");
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("Bearer access-token")
    );
    assert!(!requests[0].headers.contains_key("x-tossinvest-account"));
}

#[tokio::test]
async fn stock_info_uses_symbols_query_and_maps_reference_fields() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":[{"symbol":"005930","name":"삼성전자","englishName":"SamsungElec","isinCode":"KR7005930003","market":"KOSPI","securityType":"STOCK","isCommonShare":true,"status":"ACTIVE","currency":"KRW","listDate":"1975-06-11","delistDate":null,"sharesOutstanding":"5919637922","leverageFactor":null,"koreanMarketDetail":{"liquidationTrading":false,"nxtSupported":true,"krxTradingSuspended":false,"nxtTradingSuspended":false}}]}"#,
    ));

    let stocks = client_for(server.base_url(), TossMarketCountry::Kr)
        .get_stock_info(&[Symbol::new("005930").expect("symbol")])
        .await
        .expect("stock info");

    assert_eq!(stocks.len(), 1);
    assert_eq!(stocks[0].symbol.as_str(), "005930");
    assert_eq!(stocks[0].name, "삼성전자");
    assert_eq!(stocks[0].english_name, "SamsungElec");
    assert_eq!(stocks[0].isin_code, "KR7005930003");
    assert_eq!(stocks[0].market, "KOSPI");
    assert_eq!(stocks[0].security_type, "STOCK");
    assert_eq!(stocks[0].currency, "KRW");
    assert_eq!(stocks[0].shares_outstanding, Decimal::new(5_919_637_922, 0));
    assert!(
        stocks[0]
            .korean_market_detail
            .as_ref()
            .expect("kr market detail")
            .nxt_supported
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].path, "/api/v1/stocks?symbols=005930");
    assert!(!requests[0].headers.contains_key("x-tossinvest-account"));
}

#[tokio::test]
async fn candles_use_symbol_interval_count_pagination_and_map_ohlcv() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":{"candles":[{"timestamp":"2026-03-25T09:00:00+09:00","openPrice":"71600","highPrice":"72300","lowPrice":"71500","closePrice":"72000","volume":"3521000","currency":"KRW"}],"nextBefore":"2026-03-25T09:00:00+09:00"}}"#,
    ));
    let request = TossCandleRequest::new(
        Symbol::new("005930").expect("symbol"),
        TossCandleInterval::OneDay,
    )
    .with_count(20)
    .with_before("2026-03-26T00:00:00+09:00")
    .with_adjusted(false);

    let page = client_for(server.base_url(), TossMarketCountry::Kr)
        .get_candles(request)
        .await
        .expect("candles");

    assert_eq!(page.candles.len(), 1);
    assert_eq!(page.candles[0].timestamp, "2026-03-25T09:00:00+09:00");
    assert_eq!(page.candles[0].open_price, Decimal::new(71_600, 0));
    assert_eq!(page.candles[0].high_price, Decimal::new(72_300, 0));
    assert_eq!(page.candles[0].low_price, Decimal::new(71_500, 0));
    assert_eq!(page.candles[0].close_price, Decimal::new(72_000, 0));
    assert_eq!(page.candles[0].volume, Decimal::new(3_521_000, 0));
    assert_eq!(
        page.next_before.as_deref(),
        Some("2026-03-25T09:00:00+09:00")
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].path,
        "/api/v1/candles?symbol=005930&interval=1d&count=20&before=2026-03-26T00%3A00%3A00%2B09%3A00&adjusted=false"
    );
}

#[tokio::test]
async fn recent_trades_use_symbol_query_and_map_trade_ticks() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":[{"price":"72000","volume":"120","timestamp":"2026-03-25T09:30:42.000+09:00","currency":"KRW"}]}"#,
    ));

    let trades = client_for(server.base_url(), TossMarketCountry::Kr)
        .get_recent_trades(&Symbol::new("005930").expect("symbol"), Some(10))
        .await
        .expect("trades");

    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].price, Decimal::new(72_000, 0));
    assert_eq!(trades[0].volume, Decimal::new(120, 0));
    assert_eq!(trades[0].timestamp, "2026-03-25T09:30:42.000+09:00");
    assert_eq!(trades[0].currency, "KRW");
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/api/v1/trades?symbol=005930&count=10");
}

#[tokio::test]
async fn orderbook_and_price_limits_use_symbol_query_and_map_market_details() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":{"timestamp":"2026-03-25T09:30:00.123+09:00","currency":"KRW","asks":[{"price":"72100","volume":"8500"}],"bids":[{"price":"72000","volume":"10000"}]}}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":{"timestamp":"2026-03-25T09:30:00.123+09:00","upperLimitPrice":"93000","lowerLimitPrice":"50400","currency":"KRW"}}"#,
    ));
    let client = client_for(server.base_url(), TossMarketCountry::Kr);

    let orderbook = client
        .get_orderbook(&Symbol::new("005930").expect("symbol"))
        .await
        .expect("orderbook");
    let price_limits = client
        .get_price_limits(&Symbol::new("005930").expect("symbol"))
        .await
        .expect("price limits");

    assert_eq!(orderbook.asks[0].price, Decimal::new(72_100, 0));
    assert_eq!(orderbook.asks[0].volume, Decimal::new(8_500, 0));
    assert_eq!(orderbook.bids[0].price, Decimal::new(72_000, 0));
    assert_eq!(orderbook.bids[0].volume, Decimal::new(10_000, 0));
    assert_eq!(
        price_limits.upper_limit_price,
        Some(Decimal::new(93_000, 0))
    );
    assert_eq!(
        price_limits.lower_limit_price,
        Some(Decimal::new(50_400, 0))
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, "/api/v1/orderbook?symbol=005930");
    assert_eq!(requests[1].path, "/api/v1/price-limits?symbol=005930");
}

#[tokio::test]
async fn stock_warnings_use_symbol_path_and_preserve_unknown_warning_codes() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":[{"warningType":"VI_STATIC","exchange":"KRX","startDate":"2026-03-26","endDate":"2026-03-27"},{"warningType":"NEW_WARNING_CODE","exchange":null,"startDate":null,"endDate":null}]}"#,
    ));

    let warnings = client_for(server.base_url(), TossMarketCountry::Kr)
        .get_stock_warnings(&Symbol::new("005930").expect("symbol"))
        .await
        .expect("warnings");

    assert_eq!(warnings.len(), 2);
    assert_eq!(warnings[0].warning_type, "VI_STATIC");
    assert_eq!(warnings[0].exchange.as_deref(), Some("KRX"));
    assert_eq!(warnings[0].start_date.as_deref(), Some("2026-03-26"));
    assert_eq!(warnings[0].end_date.as_deref(), Some("2026-03-27"));
    assert_eq!(warnings[1].warning_type, "NEW_WARNING_CODE");
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/api/v1/stocks/005930/warnings");
}

#[tokio::test]
async fn market_session_uses_country_calendar_and_maps_closed_days() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":{"today":{"date":"2026-07-03","dayMarket":null,"preMarket":null,"regularMarket":null,"afterMarket":null},"previousBusinessDay":{"date":"2026-07-02","dayMarket":{"startTime":"2026-07-02T09:00:00+09:00","endTime":"2026-07-02T16:50:00+09:00"},"preMarket":null,"regularMarket":null,"afterMarket":null},"nextBusinessDay":{"date":"2026-07-06","dayMarket":{"startTime":"2026-07-06T09:00:00+09:00","endTime":"2026-07-06T16:50:00+09:00"},"preMarket":null,"regularMarket":null,"afterMarket":null}}}"#,
    ));

    let session = client_for(server.base_url(), TossMarketCountry::Us)
        .get_market_session("20260703")
        .await
        .expect("market session");

    assert_eq!(session.trading_day, "20260703");
    assert!(!session.is_open);
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].path,
        "/api/v1/market-calendar/US?date=2026-07-03"
    );
}

#[tokio::test]
async fn list_open_orders_uses_open_status_and_maps_order_states() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":{"orders":[{"orderId":"order-1","symbol":"005930","side":"BUY","orderType":"LIMIT","timeInForce":"DAY","status":"PENDING","price":"70000","quantity":"10","orderAmount":null,"currency":"KRW","orderedAt":"2026-03-29T09:30:00+09:00","canceledAt":null,"execution":{"filledQuantity":"0","averageFilledPrice":null,"filledAmount":null,"commission":null,"tax":null,"filledAt":null,"settlementDate":null}},{"orderId":"order-2","symbol":"AAPL","side":"SELL","orderType":"LIMIT","timeInForce":"DAY","status":"PARTIAL_FILLED","price":"185.5","quantity":"5","orderAmount":null,"currency":"USD","orderedAt":"2026-03-29T10:00:00+09:00","canceledAt":null,"execution":{"filledQuantity":"2","averageFilledPrice":"185.25","filledAmount":"370.5","commission":"0.66","tax":"0","filledAt":"2026-03-29T10:00:05+09:00","settlementDate":null}}],"nextCursor":null,"hasNext":false}}"#,
    ));

    let orders = client_for(server.base_url(), TossMarketCountry::Kr)
        .list_open_orders()
        .await
        .expect("open orders");

    assert_eq!(orders.len(), 2);
    assert_eq!(
        orders[0]
            .reference
            .broker_order_id
            .as_ref()
            .expect("broker order id")
            .as_str(),
        "order-1"
    );
    assert_eq!(
        orders[0]
            .evidence
            .as_ref()
            .expect("order evidence")
            .symbol
            .as_str(),
        "005930"
    );
    assert_eq!(
        orders[1]
            .evidence
            .as_ref()
            .expect("order evidence")
            .filled_quantity
            .get(),
        2
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/api/v1/orders?status=OPEN");
    assert_eq!(
        requests[0]
            .headers
            .get("x-tossinvest-account")
            .map(String::as_str),
        Some("7")
    );
}

#[tokio::test]
async fn get_order_uses_order_path_and_account_header() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"result":{"orderId":"order-1","symbol":"005930","side":"BUY","orderType":"LIMIT","timeInForce":"DAY","status":"FILLED","price":"70000","quantity":"10","orderAmount":null,"currency":"KRW","orderedAt":"2026-03-29T09:30:00+09:00","canceledAt":null,"execution":{"filledQuantity":"10","averageFilledPrice":"70000","filledAmount":"700000","commission":"1400","tax":"0","filledAt":"2026-03-29T09:30:05+09:00","settlementDate":null}}}"#,
    ));
    let reference = OrderReference {
        client_order_id: ClientOrderId::new("client-order-1").expect("client order id"),
        broker_order_id: Some(BrokerOrderId::new("order-1").expect("broker order id")),
        broker_order_orgno: None,
        exchange_code: None,
        account_key: AccountKey::new("toss-7").expect("account key"),
        environment: BrokerEnvironment::Live,
    };

    let order = client_for(server.base_url(), TossMarketCountry::Kr)
        .get_order(OrderQuery { reference })
        .await
        .expect("order detail");

    assert_eq!(
        order
            .reference
            .broker_order_id
            .as_ref()
            .expect("broker order id")
            .as_str(),
        "order-1"
    );
    assert_eq!(
        order
            .evidence
            .as_ref()
            .expect("order evidence")
            .filled_quantity
            .get(),
        10
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/api/v1/orders/order-1");
    assert_eq!(
        requests[0]
            .headers
            .get("x-tossinvest-account")
            .map(String::as_str),
        Some("7")
    );
}

#[tokio::test]
async fn toss_error_response_preserves_code_message_and_request_id() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        401,
        r#"{"error":{"requestId":"req-1","code":"invalid-token","message":"token is invalid"}}"#,
    ));

    let error = client_for(server.base_url(), TossMarketCountry::Kr)
        .get_quote(&Symbol::new("005930").expect("symbol"))
        .await
        .expect_err("invalid token must fail");

    assert!(matches!(error, BrokerError::Auth { .. }));
    assert!(error.to_string().contains("invalid-token"));
    assert!(error.to_string().contains("token is invalid"));
    assert!(error.to_string().contains("req-1"));
}

#[tokio::test]
async fn oauth_error_response_maps_invalid_client_to_auth_error() {
    let server = FakeTossServer::start();
    server.push_response(FakeResponse::json(
        401,
        r#"{"error":"invalid_client","error_description":"Client authentication failed."}"#,
    ));
    let auth = TossAuthClient::new(
        server.base_url(),
        TossCredential::new("client-id", "client-secret").expect("credential"),
    )
    .expect("auth client");

    let error = auth.issue_token().await.expect_err("invalid client fails");

    assert!(matches!(error, BrokerError::Auth { .. }));
    assert!(error.to_string().contains("invalid_client"));
    assert!(error.to_string().contains("Client authentication failed."));
}

#[tokio::test]
async fn write_methods_fail_closed_without_http_requests() {
    let server = FakeTossServer::start();
    let client = client_for(server.base_url(), TossMarketCountry::Kr);
    let order = CreateOrderRequest::cash_limit(
        ClientOrderId::new("client-order-1").expect("client order id"),
        AccountKey::new("toss-7").expect("account key"),
        Symbol::new("005930").expect("symbol"),
        OrderSide::Buy,
        Quantity::new(1),
        Decimal::new(70_000, 0),
    )
    .expect("order");
    let reference = OrderReference {
        client_order_id: ClientOrderId::new("client-order-2").expect("client order id"),
        broker_order_id: None,
        broker_order_orgno: None,
        exchange_code: None,
        account_key: AccountKey::new("toss-7").expect("account key"),
        environment: BrokerEnvironment::Live,
    };

    let place = client
        .place_order(order)
        .await
        .expect_err("place is halted");
    let cancel = client
        .cancel_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("client-order-3").expect("client order id"),
            reference: reference.clone(),
            quantity: None,
        })
        .await
        .expect_err("cancel is halted");
    let order_query = client
        .get_order(OrderQuery { reference })
        .await
        .expect_err("order detail is not available without a Toss order id");

    assert!(matches!(place, BrokerError::SafetyHalt { .. }));
    assert!(matches!(cancel, BrokerError::SafetyHalt { .. }));
    assert!(matches!(order_query, BrokerError::Validation { .. }));
    assert!(server.requests().is_empty());
}
