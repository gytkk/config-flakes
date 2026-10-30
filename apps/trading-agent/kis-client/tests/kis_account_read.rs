#![cfg(feature = "internal-test-support")]

mod support;

use std::time::Duration;

use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::domain::{AccountKey, BrokerEnvironment, Symbol};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::mapping::map_buying_power_response;
use trading_agent_kis_client::internal::protocol::domestic_stock_buying_power_endpoint;
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

fn retrying_transport_for(environment: BrokerEnvironment, base_url: &str) -> KisTransport {
    let credential = KisCredential::new("app-key", "app-secret").expect("credential");
    let access_token = AccessToken::new("access-token").expect("access token");
    KisTransport::new(
        KisTransportConfig::new(environment, credential, access_token)
            .expect("transport config")
            .with_base_url(base_url)
            .with_retry_policy(1, Duration::ZERO),
    )
    .expect("transport")
}

fn account_key() -> AccountKey {
    AccountKey::new("12345678-01").expect("account key")
}

#[test]
fn buying_power_endpoint_contract_matches_official_kis_sample() {
    let paper =
        domestic_stock_buying_power_endpoint(BrokerEnvironment::Paper).expect("paper endpoint");
    let live =
        domestic_stock_buying_power_endpoint(BrokerEnvironment::Live).expect("live endpoint");

    assert_eq!(
        paper.path,
        "/uapi/domestic-stock/v1/trading/inquire-psbl-order"
    );
    assert_eq!(paper.tr_id, "VTTC8908R");
    assert_eq!(live.tr_id, "TTTC8908R");
}

#[test]
fn buying_power_mapper_prefers_non_receivable_cash_amount() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("account/buying_power_success.json"))
            .expect("buying power fixture");

    let buying_power = map_buying_power_response(&body).expect("buying power");

    assert_eq!(buying_power.available_cash.to_string(), "900000");
}

#[test]
fn buying_power_mapper_rejects_missing_cash_fields() {
    let body = serde_json::json!({
        "rt_cd": "0",
        "msg_cd": "0",
        "msg1": "ok",
        "output": {
            "nrcvb_buy_amt": "",
            "ord_psbl_cash": "",
            "max_buy_amt": ""
        }
    });

    let error = map_buying_power_response(&body).expect_err("missing cash fields should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(error.to_string().contains("nrcvb_buy_amt"));
}

#[tokio::test]
async fn get_buying_power_uses_official_kis_request_shape_and_maps_response() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("account/buying_power_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );
    let symbol = Symbol::new("005930").expect("symbol");

    let buying_power = client
        .get_buying_power(Some(&symbol))
        .await
        .expect("buying power");

    assert_eq!(buying_power.available_cash.to_string(), "900000");

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/domestic-stock/v1/trading/inquire-psbl-order?CANO=12345678&ACNT_PRDT_CD=01&PDNO=005930&ORD_UNPR=0&ORD_DVSN=01&CMA_EVLU_AMT_ICLD_YN=N&OVRS_ICLD_YN=N"
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("VTTC8908R")
    );
}

#[tokio::test]
async fn get_buying_power_retries_kis_retry_prompt_read_error() {
    let server = FakeKisServer::start();
    // Sources checked 2026-06-10:
    // - Official endpoint sample: https://github.com/koreainvestment/open-trading-api/blob/main/examples_llm/domestic_stock/inquire_psbl_order/inquire_psbl_order.py
    // - Official error-code page: https://apiportal.koreainvestment.com/faq-error-code
    server.push_response(FakeResponse::json(
        200,
        r#"{"rt_cd":"1","msg_cd":"APBK1350","msg1":"조회 오류입니다. 다시 조회 하세요."}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("account/buying_power_success.json"),
    ));
    let client = KisBrokerClient::new_for_account(
        retrying_transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    );
    let symbol = Symbol::new("005930").expect("symbol");

    let buying_power = client
        .get_buying_power(Some(&symbol))
        .await
        .expect("buying power should retry APBK1350 read prompt once");

    assert_eq!(buying_power.available_cash.to_string(), "900000");
    assert_eq!(server.requests().len(), 2);
}
