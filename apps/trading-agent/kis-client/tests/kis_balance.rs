#![cfg(feature = "internal-test-support")]

mod support;

use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::domain::{AccountKey, BrokerEnvironment};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::mapping::{
    map_cash_balance_response, map_positions_response,
};
use trading_agent_kis_client::internal::protocol::domestic_stock_balance_endpoint;
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

#[test]
fn balance_endpoint_contract_matches_official_kis_sample() {
    let paper = domestic_stock_balance_endpoint(BrokerEnvironment::Paper).expect("paper endpoint");
    let live = domestic_stock_balance_endpoint(BrokerEnvironment::Live).expect("live endpoint");

    assert_eq!(
        paper.path,
        "/uapi/domestic-stock/v1/trading/inquire-balance"
    );
    assert_eq!(paper.tr_id, "VTTC8434R");
    assert_eq!(live.tr_id, "TTTC8434R");
}

#[test]
fn balance_mapper_filters_zero_quantity_positions() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("account/balance_success.json"))
            .expect("balance fixture");

    let positions = map_positions_response(&body).expect("positions");

    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].symbol.as_str(), "005930");
    assert_eq!(positions[0].quantity.get(), 10);
    assert_eq!(positions[0].average_price.to_string(), "70100");
}

#[test]
fn balance_mapper_reads_cash_from_summary_output() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("account/balance_success.json"))
            .expect("balance fixture");

    let cash = map_cash_balance_response(&body).expect("cash balance");

    assert_eq!(cash.cash.to_string(), "1200000");
}

#[test]
fn balance_mapper_rejects_invalid_quantity_field() {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture("account/balance_success.json"))
            .expect("balance fixture");
    body["output1"][0]["hldg_qty"] = serde_json::json!("not-a-number");

    let error = map_positions_response(&body).expect_err("invalid quantity should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(error.to_string().contains("hldg_qty"));
}

#[tokio::test]
async fn get_positions_uses_account_query_shape_and_maps_response() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("account/balance_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let positions = client.get_positions().await.expect("positions");

    assert_eq!(positions.len(), 1);
    assert_eq!(positions[0].symbol.as_str(), "005930");

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/domestic-stock/v1/trading/inquire-balance?CANO=12345678&ACNT_PRDT_CD=01&AFHR_FLPR_YN=N&OFL_YN=&INQR_DVSN=01&UNPR_DVSN=01&FUND_STTL_ICLD_YN=N&FNCG_AMT_AUTO_RDPT_YN=N&PRCS_DVSN=00&CTX_AREA_FK100=&CTX_AREA_NK100="
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("VTTC8434R")
    );
}

#[tokio::test]
async fn get_positions_follows_balance_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(200, load_fixture("account/balance_page1.json"))
            .with_header("tr_cont", "M"),
    );
    server.push_response(FakeResponse::json(
        200,
        load_fixture("account/balance_page2.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let positions = client.get_positions().await.expect("positions");

    assert_eq!(positions.len(), 2);
    assert_eq!(positions[0].symbol.as_str(), "005930");
    assert_eq!(positions[1].symbol.as_str(), "035720");

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].path,
        "/uapi/domestic-stock/v1/trading/inquire-balance?CANO=12345678&ACNT_PRDT_CD=01&AFHR_FLPR_YN=N&OFL_YN=&INQR_DVSN=01&UNPR_DVSN=01&FUND_STTL_ICLD_YN=N&FNCG_AMT_AUTO_RDPT_YN=N&PRCS_DVSN=00&CTX_AREA_FK100=FK1&CTX_AREA_NK100=NK1"
    );
    assert_eq!(
        requests[1].headers.get("tr_cont").map(String::as_str),
        Some("N")
    );
}

#[tokio::test]
async fn get_positions_rejects_repeated_balance_continuation_tokens() {
    let server = FakeKisServer::start();
    server.push_response(
        FakeResponse::json(200, load_fixture("account/balance_page1.json"))
            .with_header("tr_cont", "M"),
    );
    server.push_response(
        FakeResponse::json(200, load_fixture("account/balance_page1.json"))
            .with_header("tr_cont", "M"),
    );
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let error = client
        .get_positions()
        .await
        .expect_err("repeated continuation token must fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(error.to_string().contains("continuation token repeated"));
}

#[tokio::test]
async fn get_cash_balance_maps_summary_cash() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("account/balance_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let cash = client.get_cash_balance().await.expect("cash balance");

    assert_eq!(cash.cash.to_string(), "1200000");
}

#[tokio::test]
async fn get_account_snapshot_reads_balance_once_for_cash_and_positions() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("account/balance_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );

    let snapshot = client
        .get_account_snapshot()
        .await
        .expect("account snapshot");

    assert_eq!(snapshot.cash.cash.to_string(), "1200000");
    assert_eq!(snapshot.positions.len(), 1);
    assert_eq!(snapshot.positions[0].symbol.as_str(), "005930");
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn account_reads_require_kis_split_account_key() {
    let server = FakeKisServer::start();
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        AccountKey::new("1234567801").expect("account key"),
    );

    let error = client
        .get_positions()
        .await
        .expect_err("KIS account key must include product code separator");

    assert!(matches!(error, BrokerError::Validation { .. }));
    assert!(server.requests().is_empty());
}
