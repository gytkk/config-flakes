#![cfg(feature = "internal-test-support")]

mod support;

use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use time::macros::date;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::domain::{
    AccountKey, BrokerEnvironment, ExchangeCode, Fill, OrderSide, OrderState, OrderStatus,
    OverseasStockOrderHistoryRequest, OverseasStockOrderStatusFilter, TradingDay,
};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::mapping::{
    map_overseas_stock_open_orders_response, map_overseas_stock_order_fills_response,
    map_overseas_stock_order_history_response,
};
use trading_agent_kis_client::internal::protocol::{
    overseas_stock_open_orders_endpoint, overseas_stock_order_history_endpoint,
};
use trading_agent_kis_client::transport::{KisTransport, KisTransportConfig};

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

fn nasd() -> ExchangeCode {
    ExchangeCode::new("NASD").expect("exchange")
}

fn history_request() -> OverseasStockOrderHistoryRequest {
    OverseasStockOrderHistoryRequest::new(
        Some(nasd()),
        TradingDay(date!(2026 - 04 - 23)),
        TradingDay(date!(2026 - 04 - 23)),
    )
    .expect("history request")
}

fn nccs_fixture_with_tokens(fixture: &str, fk200: &str, nk200: &str) -> String {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture(fixture)).expect("nccs fixture");
    body["ctx_area_fk200"] = serde_json::json!(fk200);
    body["ctx_area_nk200"] = serde_json::json!(nk200);
    serde_json::to_string(&body).expect("serialize fixture")
}

fn ccnl_fixture_with_tokens(fixture: &str, fk200: &str, nk200: &str) -> String {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture(fixture)).expect("ccnl fixture");
    body["ctx_area_fk200"] = serde_json::json!(fk200);
    body["ctx_area_nk200"] = serde_json::json!(nk200);
    serde_json::to_string(&body).expect("serialize fixture")
}

#[test]
fn overseas_open_orders_endpoint_contract_matches_official_kis_sample() {
    let paper =
        overseas_stock_open_orders_endpoint(BrokerEnvironment::Paper).expect("paper endpoint");
    let live = overseas_stock_open_orders_endpoint(BrokerEnvironment::Live).expect("live endpoint");

    assert_eq!(paper.path, "/uapi/overseas-stock/v1/trading/inquire-nccs");
    assert_eq!(paper.tr_id, "TTTS3018R");
    assert_eq!(live.tr_id, "TTTS3018R");
}

#[test]
fn overseas_order_history_endpoint_contract_matches_official_kis_sample() {
    let paper =
        overseas_stock_order_history_endpoint(BrokerEnvironment::Paper).expect("paper endpoint");
    let live =
        overseas_stock_order_history_endpoint(BrokerEnvironment::Live).expect("live endpoint");

    assert_eq!(paper.path, "/uapi/overseas-stock/v1/trading/inquire-ccnl");
    assert_eq!(paper.tr_id, "VTTS3035R");
    assert_eq!(live.tr_id, "TTTS3035R");
}

#[test]
fn overseas_open_orders_mapper_builds_candidates_and_possible_quantities() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/nccs_success.json")).expect("fixture");

    let page =
        map_overseas_stock_open_orders_response(&body, &account_key(), BrokerEnvironment::Paper)
            .expect("open orders");

    assert_eq!(page.candidates.len(), 1);
    let candidate = &page.candidates[0];
    assert_eq!(
        candidate
            .reference
            .broker_order_id
            .as_ref()
            .unwrap()
            .as_str(),
        "0000116058"
    );
    assert_eq!(
        candidate.reference.exchange_code.as_ref().unwrap().as_str(),
        "NASD"
    );
    assert_eq!(candidate.symbol.as_str(), "AAPL");
    assert_eq!(candidate.side, OrderSide::Buy);
    assert_eq!(candidate.ordered_quantity.get(), 10);
    assert_eq!(candidate.filled_quantity.get(), 2);
    assert_eq!(candidate.cancelable_quantity.quantity().get(), 8);
    assert_eq!(candidate.amendable_quantity.quantity().get(), 8);
}

#[test]
fn overseas_order_history_mapper_builds_order_references_states_and_fills() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("overseas/ccnl_success.json")).expect("fixture");

    let orders: Vec<OrderStatus> =
        map_overseas_stock_order_history_response(&body, &account_key(), BrokerEnvironment::Paper)
            .expect("orders")
            .orders;
    let fills: Vec<Fill> =
        map_overseas_stock_order_fills_response(&body, &account_key(), BrokerEnvironment::Paper)
            .expect("fills");

    assert_eq!(orders.len(), 2);
    assert_eq!(orders[0].state, OrderState::PartiallyFilled);
    assert_eq!(orders[1].state, OrderState::Filled);
    let first = orders[0].evidence.as_ref().expect("evidence");
    assert_eq!(first.symbol.as_str(), "AAPL");
    assert_eq!(first.side, Some(OrderSide::Buy));
    assert_eq!(first.ordered_quantity.get(), 10);
    assert_eq!(first.filled_quantity.get(), 2);
    assert_eq!(first.order_price.expect("price").to_string(), "198.25");

    assert_eq!(fills.len(), 2);
    assert_eq!(
        fills[1]
            .reference
            .broker_order_id
            .as_ref()
            .unwrap()
            .as_str(),
        "0000116061"
    );
    assert_eq!(fills[1].quantity.get(), 3);
    assert_eq!(fills[1].price.to_string(), "949.50");
}

#[tokio::test]
async fn list_overseas_open_orders_uses_official_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/nccs_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let orders = client
        .list_overseas_stock_open_orders(&nasd())
        .await
        .expect("open orders");

    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].state, OrderState::PartiallyFilled);
    assert_eq!(
        orders[0]
            .evidence
            .as_ref()
            .expect("evidence")
            .trading_day
            .as_deref(),
        Some("20260423")
    );
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-stock/v1/trading/inquire-nccs?CANO=12345678&ACNT_PRDT_CD=01&OVRS_EXCG_CD=NASD&SORT_SQN=DS&CTX_AREA_FK200=&CTX_AREA_NK200="
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("TTTS3018R")
    );
}

#[tokio::test]
async fn list_overseas_open_orders_follows_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(
            200,
            nccs_fixture_with_tokens("overseas/nccs_page1.json", "nccs-fk-1", "nccs-nk-1"),
        )
        .with_header("tr_cont", "M"),
    );
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/nccs_page2.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let orders = client
        .list_overseas_stock_open_orders(&nasd())
        .await
        .expect("open orders");

    assert_eq!(orders.len(), 2);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].path,
        "/uapi/overseas-stock/v1/trading/inquire-nccs?CANO=12345678&ACNT_PRDT_CD=01&OVRS_EXCG_CD=NASD&SORT_SQN=DS&CTX_AREA_FK200=nccs-fk-1&CTX_AREA_NK200=nccs-nk-1"
    );
    assert_eq!(
        requests[1].headers.get("tr_cont").map(String::as_str),
        Some("N")
    );
}

#[tokio::test]
async fn list_overseas_orders_uses_official_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/ccnl_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let orders = client
        .list_overseas_stock_orders(&history_request())
        .await
        .expect("orders");

    assert_eq!(orders.len(), 2);
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-stock/v1/trading/inquire-ccnl?CANO=12345678&ACNT_PRDT_CD=01&PDNO=&ORD_STRT_DT=20260423&ORD_END_DT=20260423&SLL_BUY_DVSN=00&CCLD_NCCS_DVSN=00&OVRS_EXCG_CD=NASD&SORT_SQN=DS&ORD_DT=&ORD_GNO_BRNO=&ODNO=&CTX_AREA_NK200=&CTX_AREA_FK200="
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("VTTS3035R")
    );
}

#[tokio::test]
async fn list_overseas_fills_filters_history_to_filled_rows() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/ccnl_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );
    let mut request = history_request();
    request.status = OverseasStockOrderStatusFilter::Filled;

    let fills = client
        .list_overseas_stock_fills(&request)
        .await
        .expect("fills");

    assert_eq!(fills.len(), 2);
}

#[tokio::test]
async fn list_overseas_orders_follows_history_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(
            200,
            ccnl_fixture_with_tokens("overseas/ccnl_page1.json", "ccnl-fk-1", "ccnl-nk-1"),
        )
        .with_header("tr_cont", "M"),
    );
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/ccnl_page2.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let orders = client
        .list_overseas_stock_orders(&history_request())
        .await
        .expect("orders");

    assert_eq!(orders.len(), 2);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].path,
        "/uapi/overseas-stock/v1/trading/inquire-ccnl?CANO=12345678&ACNT_PRDT_CD=01&PDNO=&ORD_STRT_DT=20260423&ORD_END_DT=20260423&SLL_BUY_DVSN=00&CCLD_NCCS_DVSN=00&OVRS_EXCG_CD=NASD&SORT_SQN=DS&ORD_DT=&ORD_GNO_BRNO=&ODNO=&CTX_AREA_NK200=ccnl-nk-1&CTX_AREA_FK200=ccnl-fk-1"
    );
    assert_eq!(
        requests[1].headers.get("tr_cont").map(String::as_str),
        Some("N")
    );
}

#[tokio::test]
async fn list_overseas_orders_rejects_repeated_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(
            200,
            ccnl_fixture_with_tokens("overseas/ccnl_page1.json", "repeat-fk", "repeat-nk"),
        )
        .with_header("tr_cont", "M"),
    );
    server.push_response(
        FakeResponse::json(
            200,
            ccnl_fixture_with_tokens("overseas/ccnl_page2.json", "repeat-fk", "repeat-nk"),
        )
        .with_header("tr_cont", "M"),
    );
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let error = client
        .list_overseas_stock_orders(&history_request())
        .await
        .expect_err("repeated continuation token should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(error.to_string().contains("continuation token repeated"));
}
