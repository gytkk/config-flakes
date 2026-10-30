#![cfg(feature = "internal-test-support")]

mod support;

use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use time::macros::date;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::domain::{
    AccountKey, BrokerEnvironment, Fill, OrderSide, OrderState, OrderStatus, TradingDay,
};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::mapping::{
    map_daily_fills_response, map_daily_orders_response, map_revise_cancel_possible_response,
};
use trading_agent_kis_client::internal::protocol::{
    KisDailyCcldPeriod, domestic_stock_daily_ccld_endpoint,
    domestic_stock_revise_cancel_possible_endpoint,
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

fn daily_ccld_fixture_with_tokens(fixture: &str, fk100: &str, nk100: &str) -> String {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture(fixture)).expect("daily ccld fixture");
    body["ctx_area_fk100"] = serde_json::json!(fk100);
    body["ctx_area_nk100"] = serde_json::json!(nk100);
    serde_json::to_string(&body).expect("serialize fixture")
}

fn revise_cancel_possible_fixture_with_tokens(fixture: &str, fk100: &str, nk100: &str) -> String {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture(fixture)).expect("rvsecncl possible fixture");
    body["ctx_area_fk100"] = serde_json::json!(fk100);
    body["ctx_area_nk100"] = serde_json::json!(nk100);
    serde_json::to_string(&body).expect("serialize fixture")
}

#[test]
fn daily_order_endpoint_contract_matches_official_kis_sample() {
    let paper = domestic_stock_daily_ccld_endpoint(
        BrokerEnvironment::Paper,
        KisDailyCcldPeriod::WithinThreeMonths,
    )
    .expect("paper endpoint");
    let live = domestic_stock_daily_ccld_endpoint(
        BrokerEnvironment::Live,
        KisDailyCcldPeriod::WithinThreeMonths,
    )
    .expect("live endpoint");

    assert_eq!(
        paper.path,
        "/uapi/domestic-stock/v1/trading/inquire-daily-ccld"
    );
    assert_eq!(paper.tr_id, "VTTC0081R");
    assert_eq!(live.tr_id, "TTTC0081R");
}

#[test]
fn revise_cancel_possible_endpoint_contract_matches_official_kis_sample() {
    let paper = domestic_stock_revise_cancel_possible_endpoint(BrokerEnvironment::Paper)
        .expect("paper endpoint");
    let live = domestic_stock_revise_cancel_possible_endpoint(BrokerEnvironment::Live)
        .expect("live endpoint");

    assert_eq!(
        paper.path,
        "/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl"
    );
    assert_eq!(paper.tr_id, "VTTC0084R");
    assert_eq!(live.tr_id, "TTTC0084R");
}

#[test]
fn revise_cancel_possible_mapper_builds_candidates_and_possible_quantities() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("orders/psbl_rvsecncl_success.json"))
            .expect("rvsecncl possible fixture");

    let page = map_revise_cancel_possible_response(&body, &account_key(), BrokerEnvironment::Paper)
        .expect("possible page");

    assert_eq!(page.candidates.len(), 1);
    let candidate = &page.candidates[0];
    assert_eq!(
        candidate
            .reference
            .broker_order_id
            .as_ref()
            .unwrap()
            .as_str(),
        "0000001234"
    );
    assert_eq!(candidate.symbol.as_str(), "005930");
    assert_eq!(candidate.ordered_quantity.get(), 10);
    assert_eq!(candidate.filled_quantity.get(), 2);
    assert_eq!(candidate.cancelable_quantity.quantity().get(), 8);
    assert_eq!(candidate.amendable_quantity.quantity().get(), 8);
}

#[test]
fn daily_order_mapper_builds_order_references_and_states() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("orders/daily_ccld_success.json"))
            .expect("daily ccld fixture");

    let orders: Vec<OrderStatus> =
        map_daily_orders_response(&body, &account_key(), BrokerEnvironment::Paper).expect("orders");

    assert_eq!(orders.len(), 2);
    assert_eq!(
        orders[0]
            .reference
            .broker_order_id
            .as_ref()
            .unwrap()
            .as_str(),
        "0000001234"
    );
    assert_eq!(orders[0].state, OrderState::Accepted);
    let first_evidence = orders[0].evidence.as_ref().expect("order evidence");
    assert_eq!(first_evidence.symbol.as_str(), "005930");
    assert_eq!(first_evidence.side, Some(OrderSide::Buy));
    assert_eq!(first_evidence.ordered_quantity.get(), 10);
    assert_eq!(first_evidence.filled_quantity.get(), 0);
    assert_eq!(
        first_evidence.order_price.expect("price").to_string(),
        "70000"
    );
    assert_eq!(first_evidence.trading_day.as_deref(), Some("20260423"));
    assert!(first_evidence.observed_at.is_some());
    assert_eq!(orders[1].state, OrderState::Filled);
}

#[test]
fn daily_fill_mapper_keeps_only_filled_quantity_rows() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("orders/daily_ccld_success.json"))
            .expect("daily ccld fixture");

    let fills: Vec<Fill> =
        map_daily_fills_response(&body, &account_key(), BrokerEnvironment::Paper).expect("fills");

    assert_eq!(fills.len(), 1);
    assert_eq!(
        fills[0]
            .reference
            .broker_order_id
            .as_ref()
            .unwrap()
            .as_str(),
        "0000005678"
    );
    assert_eq!(fills[0].quantity.get(), 3);
    assert_eq!(fills[0].price.to_string(), "55000");
    let fill_evidence = fills[0].evidence.as_ref().expect("fill evidence");
    assert_eq!(fill_evidence.symbol.as_str(), "035720");
    assert_eq!(fill_evidence.side, Some(OrderSide::Buy));
    assert_eq!(fill_evidence.ordered_quantity.get(), 3);
    assert_eq!(fill_evidence.filled_quantity.get(), 3);
}

#[test]
fn daily_order_mapper_rejects_missing_broker_order_id() {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture("orders/daily_ccld_success.json"))
            .expect("daily ccld fixture");
    body["output1"][0]["odno"] = serde_json::json!("");

    let error = map_daily_orders_response(&body, &account_key(), BrokerEnvironment::Paper)
        .expect_err("missing order id should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(error.to_string().contains("odno"));
}

#[test]
fn daily_order_mapper_maps_cancelled_rejected_and_partial_states() {
    let body = serde_json::json!({
        "rt_cd": "0",
        "msg_cd": "MCA00000",
        "msg1": "정상처리 되었습니다.",
        "ctx_area_fk100": "",
        "ctx_area_nk100": "",
        "output1": [
            {
                "pdno": "005930",
                "odno": "0000001111",
                "ord_gno_brno": "001",
                "excg_id_dvsn_cd": "KRX",
                "ord_qty": "10",
                "tot_ccld_qty": "4",
                "avg_prvs": "70000",
                "cncl_yn": "N",
                "rjct_qty": "0"
            },
            {
                "pdno": "005930",
                "odno": "0000002222",
                "ord_gno_brno": "001",
                "excg_id_dvsn_cd": "KRX",
                "ord_qty": "10",
                "tot_ccld_qty": "0",
                "avg_prvs": "0",
                "cncl_yn": "Y",
                "rjct_qty": "0"
            },
            {
                "pdno": "005930",
                "odno": "0000003333",
                "ord_gno_brno": "001",
                "excg_id_dvsn_cd": "KRX",
                "ord_qty": "10",
                "tot_ccld_qty": "0",
                "avg_prvs": "0",
                "cncl_yn": "N",
                "rjct_qty": "10"
            }
        ],
        "output2": {
            "tot_ord_qty": "30",
            "tot_ccld_qty": "4"
        }
    });

    let orders =
        map_daily_orders_response(&body, &account_key(), BrokerEnvironment::Paper).expect("orders");

    assert_eq!(orders[0].state, OrderState::PartiallyFilled);
    assert_eq!(orders[1].state, OrderState::Cancelled);
    assert_eq!(orders[2].state, OrderState::Rejected);
}

#[tokio::test]
async fn list_orders_uses_official_daily_ccld_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let orders = client
        .list_orders(TradingDay(date!(2026 - 04 - 23)))
        .await
        .expect("orders");

    assert_eq!(orders.len(), 2);

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/domestic-stock/v1/trading/inquire-daily-ccld?CANO=12345678&ACNT_PRDT_CD=01&INQR_STRT_DT=20260423&INQR_END_DT=20260423&SLL_BUY_DVSN_CD=00&PDNO=&CCLD_DVSN=00&INQR_DVSN=00&INQR_DVSN_3=00&ORD_GNO_BRNO=&ODNO=&INQR_DVSN_1=&CTX_AREA_FK100=&CTX_AREA_NK100=&EXCG_ID_DVSN_CD=KRX"
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("VTTC0081R")
    );
}

#[tokio::test]
async fn list_orders_follows_daily_ccld_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(
            200,
            daily_ccld_fixture_with_tokens("orders/daily_ccld_success.json", "FK1", "NK1"),
        )
        .with_header("tr_cont", "M"),
    );
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_cancelled.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let orders = client
        .list_orders(TradingDay(date!(2026 - 04 - 23)))
        .await
        .expect("orders");

    assert_eq!(orders.len(), 3);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].path,
        "/uapi/domestic-stock/v1/trading/inquire-daily-ccld?CANO=12345678&ACNT_PRDT_CD=01&INQR_STRT_DT=20260423&INQR_END_DT=20260423&SLL_BUY_DVSN_CD=00&PDNO=&CCLD_DVSN=00&INQR_DVSN=00&INQR_DVSN_3=00&ORD_GNO_BRNO=&ODNO=&INQR_DVSN_1=&CTX_AREA_FK100=FK1&CTX_AREA_NK100=NK1&EXCG_ID_DVSN_CD=KRX"
    );
    assert_eq!(
        requests[1].headers.get("tr_cont").map(String::as_str),
        Some("N")
    );
}

#[tokio::test]
async fn list_orders_rejects_repeated_daily_ccld_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(
            200,
            daily_ccld_fixture_with_tokens("orders/daily_ccld_success.json", "FK1", "NK1"),
        )
        .with_header("tr_cont", "M"),
    );
    server.push_response(
        FakeResponse::json(
            200,
            daily_ccld_fixture_with_tokens("orders/daily_ccld_success.json", "FK1", "NK1"),
        )
        .with_header("tr_cont", "M"),
    );
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let error = client
        .list_orders(TradingDay(date!(2026 - 04 - 23)))
        .await
        .expect_err("repeated continuation token should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(error.to_string().contains("continuation token repeated"));
}

#[tokio::test]
async fn get_revise_cancel_capability_uses_official_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let capability = client
        .get_revise_cancel_capability()
        .await
        .expect("revise/cancel capability");

    assert_eq!(capability.candidates.len(), 1);

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl?CANO=12345678&ACNT_PRDT_CD=01&INQR_DVSN_1=0&INQR_DVSN_2=0&CTX_AREA_FK100=&CTX_AREA_NK100="
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("VTTC0084R")
    );
}

#[tokio::test]
async fn get_revise_cancel_capability_follows_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(
            200,
            revise_cancel_possible_fixture_with_tokens(
                "orders/psbl_rvsecncl_success.json",
                "FK1",
                "NK1",
            ),
        )
        .with_header("tr_cont", "M"),
    );
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_page2.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let capability = client
        .get_revise_cancel_capability()
        .await
        .expect("revise/cancel capability");

    assert_eq!(capability.candidates.len(), 2);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].path,
        "/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl?CANO=12345678&ACNT_PRDT_CD=01&INQR_DVSN_1=0&INQR_DVSN_2=0&CTX_AREA_FK100=FK1&CTX_AREA_NK100=NK1"
    );
    assert_eq!(
        requests[1].headers.get("tr_cont").map(String::as_str),
        Some("N")
    );
}

#[tokio::test]
async fn get_revise_cancel_capability_rejects_repeated_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(
            200,
            revise_cancel_possible_fixture_with_tokens(
                "orders/psbl_rvsecncl_success.json",
                "FK1",
                "NK1",
            ),
        )
        .with_header("tr_cont", "M"),
    );
    server.push_response(
        FakeResponse::json(
            200,
            revise_cancel_possible_fixture_with_tokens(
                "orders/psbl_rvsecncl_page2.json",
                "FK1",
                "NK1",
            ),
        )
        .with_header("tr_cont", "M"),
    );
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let error = client
        .get_revise_cancel_capability()
        .await
        .expect_err("repeated continuation token should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(error.to_string().contains("continuation token repeated"));
}

#[tokio::test]
async fn get_revise_cancel_capability_rejects_duplicate_rows_across_pages() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(
            200,
            revise_cancel_possible_fixture_with_tokens(
                "orders/psbl_rvsecncl_success.json",
                "FK1",
                "NK1",
            ),
        )
        .with_header("tr_cont", "M"),
    );
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let error = client
        .get_revise_cancel_capability()
        .await
        .expect_err("duplicate native rows should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(
        error
            .to_string()
            .contains("duplicate revise/cancel candidate")
    );
}
