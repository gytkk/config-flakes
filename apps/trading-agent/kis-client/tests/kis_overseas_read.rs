#![cfg(feature = "internal-test-support")]

mod support;

use rust_decimal::Decimal;
use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::domain::{
    AccountKey, BrokerEnvironment, CurrencyCode, ExchangeCode, OverseasStock,
    OverseasStockBalanceRequest, OverseasStockBuyingPowerRequest, Symbol,
};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::mapping::{
    map_overseas_stock_balance_page_response, map_overseas_stock_buying_power_response,
    map_overseas_stock_market_session_page_response, map_overseas_stock_quote_response,
};
use trading_agent_kis_client::internal::protocol::{
    OVERSEAS_STOCK_COUNTRIES_HOLIDAY_ENDPOINT, OVERSEAS_STOCK_PRICE_ENDPOINT,
    overseas_stock_balance_endpoint, overseas_stock_buying_power_endpoint,
};
use trading_agent_kis_client::transport::{EndpointGroup, KisTransport, KisTransportConfig};

fn transport_for(environment: BrokerEnvironment, base_url: &str) -> KisTransport {
    let credential = KisCredential::new("app-key", "app-secret").expect("credential");
    let access_token = AccessToken::new("access-token").expect("access token");
    KisTransport::new(
        KisTransportConfig::new(environment, credential, access_token)
            .expect("transport config")
            .with_base_url(base_url),
    )
    .expect("transport")
}

fn account_key() -> AccountKey {
    AccountKey::new("12345678-01").expect("account key")
}

fn nasdaq_aapl() -> OverseasStock {
    OverseasStock::new(
        ExchangeCode::new("NAS").expect("exchange"),
        Symbol::new("AAPL").expect("symbol"),
    )
}

fn nasd_usd_balance_request() -> OverseasStockBalanceRequest {
    OverseasStockBalanceRequest::new(
        ExchangeCode::new("NASD").expect("exchange"),
        CurrencyCode::new("USD").expect("currency"),
    )
}

#[test]
fn overseas_quote_endpoint_contract_matches_official_kis_sample() {
    assert_eq!(
        OVERSEAS_STOCK_PRICE_ENDPOINT.path,
        "/uapi/overseas-price/v1/quotations/price"
    );
    assert_eq!(OVERSEAS_STOCK_PRICE_ENDPOINT.tr_id, "HHDFS00000300");
}

#[test]
fn overseas_balance_endpoint_contract_matches_official_kis_sample() {
    let paper = overseas_stock_balance_endpoint(BrokerEnvironment::Paper).expect("paper endpoint");
    let live = overseas_stock_balance_endpoint(BrokerEnvironment::Live).expect("live endpoint");

    assert_eq!(
        paper.path,
        "/uapi/overseas-stock/v1/trading/inquire-balance"
    );
    assert_eq!(paper.tr_id, "VTTS3012R");
    assert_eq!(live.tr_id, "TTTS3012R");
}

#[test]
fn overseas_buying_power_endpoint_contract_matches_official_kis_sample() {
    let paper =
        overseas_stock_buying_power_endpoint(BrokerEnvironment::Paper).expect("paper endpoint");
    let live =
        overseas_stock_buying_power_endpoint(BrokerEnvironment::Live).expect("live endpoint");

    assert_eq!(
        paper.path,
        "/uapi/overseas-stock/v1/trading/inquire-psamount"
    );
    assert_eq!(paper.tr_id, "VTTS3007R");
    assert_eq!(live.tr_id, "TTTS3007R");
}

#[test]
fn overseas_countries_holiday_endpoint_contract_matches_official_kis_sample() {
    // Official KIS sample: examples_llm/overseas_stock/countries_holiday/countries_holiday.py
    // https://github.com/koreainvestment/open-trading-api/blob/main/examples_llm/overseas_stock/countries_holiday/countries_holiday.py
    // Accessed 2026-04-27.
    assert_eq!(
        OVERSEAS_STOCK_COUNTRIES_HOLIDAY_ENDPOINT.path,
        "/uapi/overseas-stock/v1/quotations/countries-holiday"
    );
    assert_eq!(OVERSEAS_STOCK_COUNTRIES_HOLIDAY_ENDPOINT.tr_id, "CTOS5011R");
    assert_eq!(
        OVERSEAS_STOCK_COUNTRIES_HOLIDAY_ENDPOINT.group,
        EndpointGroup::ReadHeavy
    );
}

#[test]
fn overseas_quote_mapper_normalizes_last_price() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/quote_success.json")).expect("fixture");
    let instrument = nasdaq_aapl();

    let quote = map_overseas_stock_quote_response(&body, &instrument).expect("quote");

    assert_eq!(quote.instrument, instrument);
    assert_eq!(quote.last_price.to_string(), "198.25");
}

#[test]
fn overseas_market_session_mapper_opens_matching_us_market_row() {
    // KIS live countries-holiday response observed 2026-04-27 uses numeric
    // tr_mket_cd values for US markets: 01 NASD, 02 NYSE, 05 AMEX.
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/countries_holiday_us_open.json"))
            .expect("fixture");

    for exchange_code in ["NASD", "NYSE", "AMEX"] {
        let exchange = ExchangeCode::new(exchange_code).expect("exchange");
        let page = map_overseas_stock_market_session_page_response(&body, &exchange, "20260424")
            .expect("market session page");

        assert!(page.is_open, "{exchange_code} should be open");
        assert_eq!(page.trading_day, "20260424");
    }
}

#[test]
fn overseas_market_session_mapper_closes_when_market_row_is_absent() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/countries_holiday_us_closed.json"))
            .expect("fixture");
    let exchange = ExchangeCode::new("NASD").expect("exchange");

    let page = map_overseas_stock_market_session_page_response(&body, &exchange, "20260424")
        .expect("market session page");

    assert!(!page.is_open);
    assert_eq!(page.trading_day, "20260424");
}

#[test]
fn overseas_quote_mapper_preserves_kis_error_context() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/quote_error.json")).expect("fixture");

    let error = map_overseas_stock_quote_response(&body, &nasdaq_aapl())
        .expect_err("KIS error body must fail");

    assert!(matches!(error, BrokerError::Transport { .. }));
    assert!(error.to_string().contains("KIS30001"));
    assert!(error.to_string().contains("invalid overseas symbol"));
}

#[tokio::test]
async fn get_overseas_stock_market_session_uses_official_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_open.json"),
    ));
    let client = KisBrokerClient::new(transport_for(BrokerEnvironment::Paper, server.base_url()));
    let exchange = ExchangeCode::new("NASD").expect("exchange");

    let session = client
        .get_overseas_stock_market_session(&exchange, "20260424")
        .await
        .expect("market session");

    assert!(session.is_open);
    assert_eq!(session.trading_day, "20260424");

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-stock/v1/quotations/countries-holiday?TRAD_DT=20260424&CTX_AREA_NK=&CTX_AREA_FK="
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("CTOS5011R")
    );
}

#[test]
fn overseas_balance_mapper_filters_zero_positions() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/balance_success.json")).expect("fixture");
    let request = nasd_usd_balance_request();

    let page = map_overseas_stock_balance_page_response(&body, &request).expect("balance");

    assert_eq!(page.positions.len(), 1);
    assert_eq!(page.positions[0].instrument.symbol.as_str(), "AAPL");
    assert_eq!(page.positions[0].instrument.exchange.as_str(), "NASD");
    assert_eq!(page.positions[0].currency.as_str(), "USD");
    assert_eq!(page.positions[0].quantity.get(), 3);
    assert_eq!(page.positions[0].average_price.to_string(), "175.50");
    assert_eq!(page.summary.currency.as_str(), "USD");
    assert_eq!(page.summary.purchase_amount.to_string(), "526.50");
    assert_eq!(page.summary.evaluation_amount.to_string(), "594.75");
    assert_eq!(page.summary.profit_loss.to_string(), "68.25");
}

#[test]
fn overseas_balance_mapper_defaults_empty_summary_amounts_to_zero() {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/balance_success.json")).expect("fixture");
    body["output2"]["frcr_pchs_amt1"] = serde_json::Value::String(String::new());
    body["output2"]["ovrs_stck_evlu_amt"] = serde_json::Value::String(String::new());
    body["output2"]["ovrs_tot_pfls"] = serde_json::Value::String(String::new());
    let request = nasd_usd_balance_request();

    let page = map_overseas_stock_balance_page_response(&body, &request)
        .expect("balance summary with empty amounts");

    assert_eq!(page.summary.purchase_amount, Decimal::ZERO);
    assert_eq!(page.summary.evaluation_amount, Decimal::ZERO);
    assert_eq!(page.summary.profit_loss, Decimal::ZERO);
}

#[test]
fn overseas_buying_power_mapper_prefers_overseas_order_amount() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/buying_power_success.json")).expect("fixture");

    let buying_power = map_overseas_stock_buying_power_response(&body).expect("buying power");

    assert_eq!(buying_power.currency.as_str(), "USD");
    assert_eq!(buying_power.available_cash.to_string(), "950.00");
    assert_eq!(buying_power.max_order_quantity.expect("quantity").get(), 4);
}

#[tokio::test]
async fn get_overseas_stock_quote_uses_official_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/quote_success.json"),
    ));
    let client = KisBrokerClient::new(transport_for(BrokerEnvironment::Paper, server.base_url()));
    let instrument = nasdaq_aapl();

    let quote = client
        .get_overseas_stock_quote(&instrument)
        .await
        .expect("quote");

    assert_eq!(quote.instrument, instrument);
    assert_eq!(quote.last_price.to_string(), "198.25");

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-price/v1/quotations/price?AUTH=&EXCD=NAS&SYMB=AAPL"
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("HHDFS00000300")
    );
    assert!(!requests[0].headers.contains_key("hashkey"));
}

#[tokio::test]
async fn get_overseas_stock_account_snapshot_paginates_balance() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(200, load_fixture("overseas/balance_page1.json"))
            .with_header("tr_cont", "M"),
    );
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/balance_page2.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );
    let request = nasd_usd_balance_request();

    let snapshot = client
        .get_overseas_stock_account_snapshot(&request)
        .await
        .expect("snapshot");

    assert_eq!(snapshot.positions.len(), 2);
    assert_eq!(snapshot.summary.currency.as_str(), "USD");
    assert_eq!(snapshot.summary.evaluation_amount.to_string(), "594.75");

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-stock/v1/trading/inquire-balance?CANO=12345678&ACNT_PRDT_CD=01&OVRS_EXCG_CD=NASD&TR_CRCY_CD=USD&CTX_AREA_FK200=&CTX_AREA_NK200="
    );
    assert_eq!(
        requests[1].path,
        "/uapi/overseas-stock/v1/trading/inquire-balance?CANO=12345678&ACNT_PRDT_CD=01&OVRS_EXCG_CD=NASD&TR_CRCY_CD=USD&CTX_AREA_FK200=page-1-fk&CTX_AREA_NK200=page-1-nk"
    );
    assert_eq!(
        requests[1].headers.get("tr_cont").map(String::as_str),
        Some("N")
    );
}

#[tokio::test]
async fn get_overseas_stock_buying_power_uses_official_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/buying_power_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );
    let request = OverseasStockBuyingPowerRequest::new(
        OverseasStock::new(
            ExchangeCode::new("NASD").expect("exchange"),
            Symbol::new("AAPL").expect("symbol"),
        ),
        Decimal::new(19825, 2),
    )
    .expect("request");

    let buying_power = client
        .get_overseas_stock_buying_power(&request)
        .await
        .expect("buying power");

    assert_eq!(buying_power.available_cash.to_string(), "950.00");

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-stock/v1/trading/inquire-psamount?CANO=12345678&ACNT_PRDT_CD=01&OVRS_EXCG_CD=NASD&OVRS_ORD_UNPR=198.25&ITEM_CD=AAPL"
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("VTTS3007R")
    );
}
