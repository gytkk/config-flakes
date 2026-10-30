#![cfg(feature = "internal-test-support")]

mod support;

use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::domain::{BrokerEnvironment, Symbol};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::mapping::map_quote_response;
use trading_agent_kis_client::internal::protocol::DOMESTIC_STOCK_QUOTE_ENDPOINT;
use trading_agent_kis_client::transport::{KisTransport, KisTransportConfig};

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
fn quote_endpoint_contract_matches_official_kis_sample() {
    assert_eq!(
        DOMESTIC_STOCK_QUOTE_ENDPOINT.path,
        "/uapi/domestic-stock/v1/quotations/inquire-price"
    );
    assert_eq!(DOMESTIC_STOCK_QUOTE_ENDPOINT.tr_id, "FHKST01010100");
}

#[test]
fn quote_mapper_normalizes_kis_numeric_strings_to_domain_quote() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("market/quote_success.json")).expect("quote fixture");

    let quote = map_quote_response(&body).expect("mapped quote");

    assert_eq!(quote.symbol.as_str(), "005930");
    assert_eq!(quote.full_name, None);
    assert_eq!(quote.last_price.to_string(), "72300");
}

#[test]
fn quote_mapper_preserves_kis_error_context() {
    let body: serde_json::Value =
        serde_json::from_str(&load_fixture("market/quote_error.json")).expect("error fixture");

    let error = map_quote_response(&body).expect_err("KIS error body must fail");

    assert!(matches!(error, BrokerError::Transport { .. }));
    assert!(error.to_string().contains("KIS00001"));
    assert!(error.to_string().contains("invalid input symbol"));
}

#[tokio::test]
async fn get_quote_uses_official_kis_request_shape_and_maps_response() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/search_stock_info_success.json"),
    ));
    let client = KisBrokerClient::new(transport_for(server.base_url()));
    let symbol = Symbol::new("005930").expect("symbol");

    let quote = client.get_quote(&symbol).await.expect("quote response");

    assert_eq!(quote.symbol, symbol);
    assert_eq!(quote.full_name.as_deref(), Some("삼성전자보통주"));
    assert_eq!(quote.last_price.to_string(), "72300");

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/domestic-stock/v1/quotations/inquire-price?FID_COND_MRKT_DIV_CODE=J&FID_INPUT_ISCD=005930"
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("FHKST01010100")
    );
    assert!(!requests[0].headers.contains_key("hashkey"));
    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].path,
        "/uapi/domestic-stock/v1/quotations/search-stock-info?PRDT_TYPE_CD=300&PDNO=005930"
    );
    assert_eq!(
        requests[1].headers.get("tr_id").map(String::as_str),
        Some("VTPF1002R")
    );
    assert!(!requests[1].headers.contains_key("hashkey"));
}
