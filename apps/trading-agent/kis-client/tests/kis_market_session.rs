#![cfg(feature = "internal-test-support")]

mod support;

use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::domain::BrokerEnvironment;
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::mapping::map_market_session_response;
use trading_agent_kis_client::internal::protocol::DOMESTIC_STOCK_HOLIDAY_ENDPOINT;
use trading_agent_kis_client::transport::{EndpointGroup, KisTransport, KisTransportConfig};

fn transport_for(base_url: &str) -> KisTransport {
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(base_url),
    )
    .expect("transport")
}

#[test]
fn holiday_endpoint_contract_matches_official_kis_sample() {
    assert_eq!(
        DOMESTIC_STOCK_HOLIDAY_ENDPOINT.path,
        "/uapi/domestic-stock/v1/quotations/chk-holiday"
    );
    assert_eq!(DOMESTIC_STOCK_HOLIDAY_ENDPOINT.tr_id, "CTCA0903R");
    assert_eq!(
        DOMESTIC_STOCK_HOLIDAY_ENDPOINT.group,
        EndpointGroup::ReadHeavy
    );
}

#[test]
fn market_session_mapper_uses_open_day_flag() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("market/holiday_open.json")).expect("holiday fixture");

    let session = map_market_session_response(&body).expect("market session");

    assert_eq!(session.trading_day, "20260423");
    assert!(session.is_open);
}

#[test]
fn market_session_mapper_rejects_missing_open_day_flag() {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture("market/holiday_open.json")).expect("holiday fixture");
    body["output"][0]["opnd_yn"] = serde_json::json!("");

    let error = map_market_session_response(&body).expect_err("missing open flag should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert!(error.to_string().contains("opnd_yn"));
}

#[tokio::test]
async fn get_market_session_uses_holiday_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/holiday_open.json"),
    ));
    let client = KisBrokerClient::new(transport_for(server.base_url()));

    let session = client
        .get_market_session("20260423")
        .await
        .expect("market session");

    assert!(session.is_open);

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/domestic-stock/v1/quotations/chk-holiday?BASS_DT=20260423&CTX_AREA_FK=&CTX_AREA_NK="
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("CTCA0903R")
    );
}

#[tokio::test]
async fn get_market_session_retries_kis_mci_postprocessing_error() {
    let server = FakeKisServer::start();
    // Sources checked 2026-06-16:
    // - Official endpoint sample: https://github.com/koreainvestment/open-trading-api/blob/main/examples_llm/domestic_stock/chk_holiday/chk_holiday.py
    // - Official error-code page: https://apiportal.koreainvestment.com/faq-error-code
    server.push_response(FakeResponse::json(
        200,
        r#"{"rt_cd":"1","msg_cd":"OPSQ0008","msg1":"호출 후처리(MCI전송) 오류 입니다."}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/holiday_open.json"),
    ));
    let client = KisBrokerClient::new(transport_for(server.base_url()));

    let session = client
        .get_market_session("20260423")
        .await
        .expect("market session should retry OPSQ0008 transient read error");

    assert!(session.is_open);
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn get_market_session_retries_kis_resource_postprocessing_error() {
    let server = FakeKisServer::start();
    // Sources checked 2026-06-16:
    // - Official endpoint sample: https://github.com/koreainvestment/open-trading-api/blob/main/examples_llm/domestic_stock/chk_holiday/chk_holiday.py
    // - Official error-code page: https://apiportal.koreainvestment.com/faq-error-code
    server.push_response(FakeResponse::json(
        200,
        r#"{"rt_cd":"1","msg_cd":"OPSQ0010","msg1":"호출 결과처리(리소스 부족) 오류 입니다."}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/holiday_open.json"),
    ));
    let client = KisBrokerClient::new(transport_for(server.base_url()));

    let session = client
        .get_market_session("20260423")
        .await
        .expect("market session should retry OPSQ0010 transient read error");

    assert!(session.is_open);
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn ping_uses_lightweight_quote_request_and_returns_health() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let client = KisBrokerClient::new(transport_for(server.base_url()));

    let health = client.ping().await.expect("broker health");

    assert_eq!(health.environment, BrokerEnvironment::Paper);
    assert!(!health.writable);
    assert_eq!(server.requests().len(), 1);
}
