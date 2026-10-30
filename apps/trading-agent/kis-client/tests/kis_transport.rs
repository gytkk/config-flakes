mod support;

use serde_json::json;
use std::time::{Duration, Instant};
use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use trading_agent_kis_client::auth::{AccessToken, KisAuthConfig, KisCredential, KisTokenProvider};
use trading_agent_kis_client::domain::BrokerEnvironment;
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::transport::{
    EndpointGroup, KisEndpoint, KisHttpMethod, KisRateLimitConfig, KisTransport,
    KisTransportConfig, TransportFailureClass,
};

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

#[tokio::test]
async fn endpoint_group_rate_limit_waits_between_requests() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_rate_limits(KisRateLimitConfig {
                read_light_min_interval: Duration::from_millis(25),
                ..KisRateLimitConfig::default()
            }),
    )
    .expect("transport");
    let request = || {
        transport
            .prepare_json(
                quote_endpoint(),
                &[
                    ("FID_COND_MRKT_DIV_CODE", "J"),
                    ("FID_INPUT_ISCD", "005930"),
                ],
                None,
            )
            .expect("prepared request")
    };

    let started_at = Instant::now();
    transport
        .execute_json(request())
        .await
        .expect("first response");
    transport
        .execute_json(request())
        .await
        .expect("second response");

    assert!(started_at.elapsed() >= Duration::from_millis(20));
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn global_rate_limit_waits_between_different_endpoint_groups() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_rate_limits(KisRateLimitConfig {
                global_min_interval: Duration::from_millis(50),
                ..KisRateLimitConfig::default()
            }),
    )
    .expect("transport");

    let started_at = Instant::now();
    transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared light request"),
        )
        .await
        .expect("first response");
    transport
        .execute_json(
            transport
                .prepare_json(read_heavy_endpoint(), &[("CANO", "12345678")], None)
                .expect("prepared heavy request"),
        )
        .await
        .expect("second response");

    assert!(started_at.elapsed() >= Duration::from_millis(45));
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn shared_rate_limit_waits_across_transport_instances() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let state_dir = tempfile::tempdir().expect("temp shared limiter dir");
    let shared_state_path = state_dir.path().join("kis-rate-limit.sqlite3");
    let first = transport_with_shared_rate_limit(
        server.base_url(),
        &shared_state_path,
        Duration::from_millis(60),
    );
    let second = transport_with_shared_rate_limit(
        server.base_url(),
        &shared_state_path,
        Duration::from_millis(60),
    );

    let started_at = Instant::now();
    first
        .execute_json(
            first
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("first prepared request"),
        )
        .await
        .expect("first response");
    second
        .execute_json(
            second
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "000660"),
                    ],
                    None,
                )
                .expect("second prepared request"),
        )
        .await
        .expect("second response");

    assert!(started_at.elapsed() >= Duration::from_millis(50));
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn shared_rate_limit_serializes_concurrent_transport_instances() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let state_dir = tempfile::tempdir().expect("temp shared limiter dir");
    let shared_state_path = state_dir.path().join("kis-rate-limit.sqlite3");
    let first = transport_with_shared_rate_limit(
        server.base_url(),
        &shared_state_path,
        Duration::from_millis(80),
    );
    let second = transport_with_shared_rate_limit(
        server.base_url(),
        &shared_state_path,
        Duration::from_millis(80),
    );
    let first_request = first
        .prepare_json(
            quote_endpoint(),
            &[
                ("FID_COND_MRKT_DIV_CODE", "J"),
                ("FID_INPUT_ISCD", "005930"),
            ],
            None,
        )
        .expect("first prepared request");
    let second_request = second
        .prepare_json(
            quote_endpoint(),
            &[
                ("FID_COND_MRKT_DIV_CODE", "J"),
                ("FID_INPUT_ISCD", "000660"),
            ],
            None,
        )
        .expect("second prepared request");

    let started_at = Instant::now();
    let (first_result, second_result) = tokio::join!(
        first.execute_json(first_request),
        second.execute_json(second_request)
    );

    first_result.expect("first response");
    second_result.expect("second response");
    assert!(started_at.elapsed() >= Duration::from_millis(70));
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn shared_rate_limit_circuit_waits_across_transport_instances() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"rt_cd":"1","msg_cd":"EGW429","msg1":"rate limit exceeded"}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let state_dir = tempfile::tempdir().expect("temp shared limiter dir");
    let shared_state_path = state_dir.path().join("kis-rate-limit.sqlite3");
    let first = transport_with_shared_circuit(
        server.base_url(),
        &shared_state_path,
        Duration::from_millis(1_000),
    );
    let second = transport_with_shared_circuit(
        server.base_url(),
        &shared_state_path,
        Duration::from_millis(1_000),
    );

    let first_error = first
        .execute_json(
            first
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("first prepared request"),
        )
        .await
        .expect_err("first transport should observe throttle");
    let circuit_open_until_ms: i64 = rusqlite::Connection::open(&shared_state_path)
        .expect("open shared limiter db")
        .query_row(
            "SELECT circuit_open_until_ms
               FROM kis_rate_limit_state
              WHERE key LIKE '%:group:read_light'",
            [],
            |row| row.get(0),
        )
        .expect("shared circuit row");
    assert!(circuit_open_until_ms > 0);
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_millis();
    let expected_wait_ms = u128::try_from(circuit_open_until_ms)
        .expect("positive circuit timestamp")
        .saturating_sub(now_ms);
    assert!(
        expected_wait_ms >= 200,
        "shared circuit should remain open long enough to verify, remaining={expected_wait_ms}ms"
    );
    let started_at = Instant::now();
    second
        .execute_json(
            second
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "000660"),
                    ],
                    None,
                )
                .expect("second prepared request"),
        )
        .await
        .expect("second response after shared circuit wait");

    assert!(matches!(first_error, BrokerError::RateLimit { .. }));
    assert!(
        started_at.elapsed().as_millis().saturating_add(50) >= expected_wait_ms,
        "second transport should wait for shared circuit, elapsed={}ms expected={}ms",
        started_at.elapsed().as_millis(),
        expected_wait_ms
    );
    assert_eq!(server.requests().len(), 2);
}

fn quote_endpoint() -> KisEndpoint {
    KisEndpoint {
        name: "domestic_quote",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/quotations/inquire-price",
        tr_id: "FHKST01010100",
        group: EndpointGroup::ReadLight,
        requires_hashkey: false,
    }
}

fn transport_with_shared_rate_limit(
    base_url: &str,
    shared_state_path: &std::path::Path,
    global_min_interval: Duration,
) -> KisTransport {
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(base_url)
            .with_shared_rate_limit_path(shared_state_path)
            .with_rate_limits(KisRateLimitConfig {
                global_min_interval,
                ..KisRateLimitConfig::default()
            }),
    )
    .expect("transport")
}

fn transport_with_shared_circuit(
    base_url: &str,
    shared_state_path: &std::path::Path,
    throttle_circuit_open_for: Duration,
) -> KisTransport {
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(base_url)
            .with_shared_rate_limit_path(shared_state_path)
            .with_rate_limits(KisRateLimitConfig {
                throttle_circuit_open_for,
                ..KisRateLimitConfig::default()
            })
            .with_retry_policy(0, Duration::ZERO),
    )
    .expect("transport")
}

fn read_heavy_endpoint() -> KisEndpoint {
    KisEndpoint {
        name: "domestic_balance",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-balance",
        tr_id: "TTTC8434R",
        group: EndpointGroup::ReadHeavy,
        requires_hashkey: false,
    }
}

fn write_endpoint() -> KisEndpoint {
    KisEndpoint {
        name: "cash_buy_order",
        method: KisHttpMethod::Post,
        path: "/uapi/domestic-stock/v1/trading/order-cash",
        tr_id: "VTTC0011U",
        group: EndpointGroup::WriteOrder,
        requires_hashkey: false,
    }
}

#[test]
fn request_builder_adds_kis_headers_query_and_redacted_metadata() {
    let transport = transport_for("http://127.0.0.1:65535");
    let request = transport
        .prepare_json(
            quote_endpoint(),
            &[
                ("FID_COND_MRKT_DIV_CODE", "J"),
                ("FID_INPUT_ISCD", "005930"),
            ],
            None,
        )
        .expect("prepared request");

    assert_eq!(request.method(), KisHttpMethod::Get);
    assert_eq!(
        request.url().as_str(),
        "http://127.0.0.1:65535/uapi/domestic-stock/v1/quotations/inquire-price?FID_COND_MRKT_DIV_CODE=J&FID_INPUT_ISCD=005930"
    );
    assert_eq!(
        request.headers().get("tr_id").map(String::as_str),
        Some("FHKST01010100")
    );
    assert_eq!(
        request.headers().get("appkey").map(String::as_str),
        Some("paper-app-key")
    );
    assert_eq!(
        request.headers().get("authorization").map(String::as_str),
        Some("Bearer paper-access-token")
    );

    let redacted = request.redacted_headers();
    assert_eq!(redacted.get("appkey").map(String::as_str), Some("***"));
    assert_eq!(redacted.get("appsecret").map(String::as_str), Some("***"));
    assert_eq!(
        redacted.get("authorization").map(String::as_str),
        Some("***")
    );
    assert!(!format!("{redacted:?}").contains("paper-access-token"));
    assert!(!format!("{redacted:?}").contains("paper-app-secret"));
}

#[test]
fn request_builder_generates_unique_uuid_request_ids() {
    let transport = transport_for("http://127.0.0.1:65535");
    let first = transport
        .prepare_json(quote_endpoint(), &[("FID_INPUT_ISCD", "005930")], None)
        .expect("first prepared request");
    let second = transport
        .prepare_json(quote_endpoint(), &[("FID_INPUT_ISCD", "005930")], None)
        .expect("second prepared request");
    let first_id = first
        .headers()
        .get("x-request-id")
        .expect("first request id");
    let second_id = second
        .headers()
        .get("x-request-id")
        .expect("second request id");

    uuid::Uuid::parse_str(first_id).expect("first request id is uuid");
    uuid::Uuid::parse_str(second_id).expect("second request id is uuid");
    assert_ne!(first_id, second_id);
}

#[test]
fn transport_rejects_invalid_base_url_and_zero_timeouts() {
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let config = KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
        .expect("transport config");

    let invalid_url = KisTransport::new(config.clone().with_base_url("not a url"))
        .expect_err("invalid base URL should fail");
    assert!(invalid_url.to_string().contains("invalid base URL"));

    let invalid_timeout = config
        .try_with_timeouts(Duration::ZERO, Duration::from_secs(1))
        .expect_err("zero total timeout should fail");
    assert!(invalid_timeout.to_string().contains("timeout"));
}

#[test]
fn request_audit_record_redacts_credentials_and_account_query() {
    let transport = transport_for("http://127.0.0.1:65535");
    let request = transport
        .prepare_json(
            quote_endpoint(),
            &[
                ("CANO", "12345678"),
                ("ACNT_PRDT_CD", "01"),
                ("FID_INPUT_ISCD", "005930"),
            ],
            None,
        )
        .expect("prepared request");

    let audit = request.audit_record();

    assert_eq!(audit.endpoint_name, "domestic_quote");
    assert_eq!(audit.method, KisHttpMethod::Get);
    assert_eq!(
        audit.path,
        "/uapi/domestic-stock/v1/quotations/inquire-price"
    );
    assert_eq!(audit.query.get("CANO").map(String::as_str), Some("***"));
    assert_eq!(
        audit.query.get("ACNT_PRDT_CD").map(String::as_str),
        Some("***")
    );
    assert_eq!(
        audit.query.get("FID_INPUT_ISCD").map(String::as_str),
        Some("005930")
    );
    assert_eq!(audit.headers.get("appkey").map(String::as_str), Some("***"));
    assert_eq!(
        audit.headers.get("authorization").map(String::as_str),
        Some("***")
    );

    let rendered = format!("{audit:?}");
    assert!(!rendered.contains("paper-access-token"));
    assert!(!rendered.contains("paper-app-key"));
    assert!(!rendered.contains("paper-app-secret"));
    assert!(!rendered.contains("12345678"));
}

#[test]
fn request_debug_redacts_order_body_and_hashkey() {
    let transport = transport_for("http://127.0.0.1:65535");
    let request = transport
        .prepare_json_with_options(
            write_endpoint(),
            &[],
            Some(json!({
                "CANO": "12345678",
                "ACNT_PRDT_CD": "01",
                "PDNO": "005930",
                "ORD_QTY": "10",
                "ORD_UNPR": "70000"
            })),
            trading_agent_kis_client::transport::KisRequestOptions {
                tr_cont: None,
                hashkey: Some("raw-hashkey-value"),
            },
        )
        .expect("prepared request");

    let rendered = format!("{request:?}");

    assert!(!rendered.contains("12345678"));
    assert!(!rendered.contains("raw-hashkey-value"));
    assert!(!rendered.contains("ORD_QTY"));
    assert!(rendered.contains("body_redacted"));
}

#[test]
fn request_debug_redacts_account_query_in_url() {
    let transport = transport_for("http://127.0.0.1:65535");
    let request = transport
        .prepare_json(
            quote_endpoint(),
            &[
                ("CANO", "12345678"),
                ("ACNT_PRDT_CD", "01"),
                ("FID_INPUT_ISCD", "005930"),
            ],
            None,
        )
        .expect("prepared request");

    let rendered = format!("{request:?}");

    assert!(!rendered.contains("12345678"));
    assert!(!rendered.contains("ACNT_PRDT_CD=01"));
    assert!(rendered.contains("CANO=***"));
    assert!(rendered.contains("FID_INPUT_ISCD=005930"));
}

#[tokio::test]
async fn execute_json_sends_prepared_request_to_fake_server() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let transport = transport_for(server.base_url());

    let response = transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect("transport response");

    assert_eq!(response.status(), 200);
    assert_eq!(response.body()["rt_cd"], "0");
    let audit = response.audit_record();
    assert_eq!(audit.endpoint_name, "domestic_quote");
    assert_eq!(audit.status, 200);
    assert_eq!(audit.kis_rt_cd.as_deref(), Some("0"));
    assert_eq!(audit.kis_msg_cd.as_deref(), Some("MCA00000"));
    assert_eq!(
        audit.headers.get("content-type").map(String::as_str),
        Some("application/json")
    );
    assert!(audit.has_body);
    assert!(!format!("{audit:?}").contains("72300"));

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].path,
        "/uapi/domestic-stock/v1/quotations/inquire-price?FID_COND_MRKT_DIV_CODE=J&FID_INPUT_ISCD=005930"
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("FHKST01010100")
    );
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("Bearer paper-access-token")
    );
}

#[tokio::test]
async fn transport_can_be_built_from_token_provider() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"provider-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let provider = KisTokenProvider::new(
        KisAuthConfig::new(BrokerEnvironment::Paper, credential)
            .expect("auth config")
            .with_base_url(server.base_url()),
    )
    .expect("token provider");

    let transport = KisTransport::new_with_token_provider(&provider)
        .await
        .expect("transport");
    transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect("quote response");

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].path, "/oauth2/tokenP");
    assert_eq!(
        requests[1].headers.get("authorization").map(String::as_str),
        Some("Bearer provider-token")
    );
}

#[tokio::test]
async fn transport_refreshes_provider_token_before_request() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"expired-provider-token",
            "token_type":"Bearer",
            "expires_in":1,
            "access_token_token_expired":"2000-01-01 09:00:00"
        }"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"fresh-provider-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let provider = KisTokenProvider::new(
        KisAuthConfig::new(BrokerEnvironment::Paper, credential)
            .expect("auth config")
            .with_base_url(server.base_url()),
    )
    .expect("token provider");

    let transport = KisTransport::new_with_token_provider(&provider)
        .await
        .expect("transport");
    transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect("quote response");

    let requests = server.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].path, "/oauth2/tokenP");
    assert_eq!(requests[1].path, "/oauth2/tokenP");
    assert_eq!(
        requests[2].headers.get("authorization").map(String::as_str),
        Some("Bearer fresh-provider-token")
    );
}

#[tokio::test]
async fn transport_invalidates_provider_token_and_retries_after_auth_failure() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"initial-provider-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    server.push_response(FakeResponse::json(
        401,
        r#"{"rt_cd":"1","msg_cd":"401","msg1":"expired access token"}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"refreshed-provider-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let provider = KisTokenProvider::new(
        KisAuthConfig::new(BrokerEnvironment::Paper, credential)
            .expect("auth config")
            .with_base_url(server.base_url()),
    )
    .expect("token provider");

    let transport = KisTransport::new_with_token_provider(&provider)
        .await
        .expect("transport");
    transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect("quote response after token refresh");

    let requests = server.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].path, "/oauth2/tokenP");
    assert_eq!(
        requests[1].headers.get("authorization").map(String::as_str),
        Some("Bearer initial-provider-token")
    );
    assert_eq!(requests[2].path, "/oauth2/tokenP");
    assert_eq!(
        requests[3].headers.get("authorization").map(String::as_str),
        Some("Bearer refreshed-provider-token")
    );
}

#[tokio::test]
async fn hashkey_helper_posts_exact_order_payload_and_redacts_hash_debug() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(200, r#"{"HASH":"raw-hash-value"}"#));
    let transport = transport_for(server.base_url());
    let payload = json!({
        "CANO": "12345678",
        "ACNT_PRDT_CD": "01",
        "PDNO": "005930",
        "ORD_QTY": "10"
    });

    let hashkey = transport.issue_hashkey(&payload).await.expect("hashkey");

    assert_eq!(hashkey.as_str(), "raw-hash-value");
    assert!(!format!("{hashkey:?}").contains("raw-hash-value"));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/uapi/hashkey");
    let sent_body: serde_json::Value =
        serde_json::from_str(&requests[0].body).expect("request body json");
    assert_eq!(sent_body, payload);
}

#[tokio::test]
async fn execute_json_maps_kis_body_errors_by_endpoint_group() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"rt_cd":"1","msg_cd":"EGW401","msg1":"unauthorized"}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        r#"{"rt_cd":"1","msg_cd":"APBK0013","msg1":"주문가능금액을 초과하였습니다"}"#,
    ));
    let transport = transport_for(server.base_url());

    let auth = transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared read request"),
        )
        .await
        .expect_err("body-level auth error should fail");
    let rejected = transport
        .execute_json(
            transport
                .prepare_json(write_endpoint(), &[], Some(json!({"PDNO": "005930"})))
                .expect("prepared write request"),
        )
        .await
        .expect_err("body-level write rejection should fail");

    assert!(matches!(auth, BrokerError::Auth { .. }));
    assert!(matches!(rejected, BrokerError::OrderRejected { .. }));
}

#[tokio::test]
async fn execute_json_tracks_body_level_throttle_as_group_circuit() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{"rt_cd":"1","msg_cd":"EGW429","msg1":"rate limit exceeded"}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_rate_limits(KisRateLimitConfig {
                throttle_circuit_open_for: Duration::from_millis(25),
                ..KisRateLimitConfig::default()
            })
            .with_retry_policy(0, Duration::ZERO),
    )
    .expect("transport");
    let request = || {
        transport
            .prepare_json(
                quote_endpoint(),
                &[
                    ("FID_COND_MRKT_DIV_CODE", "J"),
                    ("FID_INPUT_ISCD", "005930"),
                ],
                None,
            )
            .expect("prepared request")
    };

    let first = transport
        .execute_json(request())
        .await
        .expect_err("body-level throttle should fail");
    let started_at = Instant::now();
    let second = transport
        .execute_json(request())
        .await
        .expect("circuit opens then allows next response");

    assert!(matches!(first, BrokerError::RateLimit { .. }));
    assert_eq!(second.status(), 200);
    assert!(started_at.elapsed() >= Duration::from_millis(20));
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn execute_json_retries_read_rate_limit_after_throttle_circuit() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        500,
        r#"{"rt_cd":"1","msg_cd":"EGW00201","msg1":"초당 거래건수를 초과하였습니다."}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_rate_limits(KisRateLimitConfig {
                throttle_circuit_open_for: Duration::from_millis(25),
                ..KisRateLimitConfig::default()
            })
            .with_retry_policy(1, Duration::ZERO),
    )
    .expect("transport");

    let started_at = Instant::now();
    let response = transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect("read rate limit should retry after circuit");

    assert_eq!(response.status(), 200);
    assert!(started_at.elapsed() >= Duration::from_millis(20));
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn execute_json_retries_temporary_read_5xx_once() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        503,
        r#"{"rt_cd":"1","msg_cd":"EGW001","msg1":"temporary"}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("market/quote_success.json"),
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_retry_policy(1, Duration::ZERO),
    )
    .expect("transport");

    let response = transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect("retried read response");

    assert_eq!(response.status(), 200);
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn execute_json_includes_kis_message_fields_in_5xx_errors() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        500,
        r#"{"rt_cd":"1","msg_cd":"EGW500","msg1":"upstream gateway failed"}"#,
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_retry_policy(0, Duration::ZERO),
    )
    .expect("transport");

    let error = transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect_err("5xx should fail");

    let rendered = error.to_string();
    assert!(matches!(error, BrokerError::Temporary { .. }));
    assert!(rendered.contains("domestic_quote"));
    assert!(rendered.contains("status 500"));
    assert!(rendered.contains("EGW500"));
    assert!(rendered.contains("upstream gateway failed"));
}

#[tokio::test]
async fn execute_json_maps_kis_throttle_code_to_rate_limit_even_with_5xx_status() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        500,
        r#"{"rt_cd":"1","msg_cd":"EGW00201","msg1":"초당 거래건수를 초과하였습니다."}"#,
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_retry_policy(0, Duration::ZERO),
    )
    .expect("transport");

    let error = transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect_err("KIS throttle response should fail");

    let rendered = error.to_string();
    assert!(matches!(error, BrokerError::RateLimit { .. }));
    assert!(rendered.contains("domestic_quote"));
    assert!(rendered.contains("status 500"));
    assert!(rendered.contains("EGW00201"));
    assert!(rendered.contains("초당 거래건수를 초과"));
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn execute_json_reports_endpoint_and_status_for_non_json_5xx_errors() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse {
        status: 500,
        headers: Default::default(),
        body: "<html>temporary gateway failure</html>".to_owned(),
    });
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_retry_policy(0, Duration::ZERO),
    )
    .expect("transport");

    let error = transport
        .execute_json(
            transport
                .prepare_json(
                    quote_endpoint(),
                    &[
                        ("FID_COND_MRKT_DIV_CODE", "J"),
                        ("FID_INPUT_ISCD", "005930"),
                    ],
                    None,
                )
                .expect("prepared request"),
        )
        .await
        .expect_err("non-JSON 5xx should fail");

    let rendered = error.to_string();
    assert!(matches!(error, BrokerError::Temporary { .. }));
    assert!(rendered.contains("domestic_quote"));
    assert!(rendered.contains("status 500"));
    assert!(rendered.contains("non-JSON response body"));
}

#[tokio::test]
async fn execute_json_maps_common_http_statuses_to_broker_errors() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        429,
        r#"{"rt_cd":"1","msg_cd":"EGW429","msg1":"rate limited"}"#,
    ));
    server.push_response(FakeResponse::json(
        401,
        r#"{"rt_cd":"1","msg_cd":"EGW401","msg1":"unauthorized"}"#,
    ));
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let access_token = AccessToken::new("paper-access-token").expect("access token");
    let transport = KisTransport::new(
        KisTransportConfig::new(BrokerEnvironment::Paper, credential, access_token)
            .expect("transport config")
            .with_base_url(server.base_url())
            .with_retry_policy(0, Duration::ZERO),
    )
    .expect("transport");
    let request = || {
        transport
            .prepare_json(
                quote_endpoint(),
                &[
                    ("FID_COND_MRKT_DIV_CODE", "J"),
                    ("FID_INPUT_ISCD", "005930"),
                ],
                None,
            )
            .expect("prepared request")
    };

    let rate_limit = transport
        .execute_json(request())
        .await
        .expect_err("429 should map to rate limit");
    let auth = transport
        .execute_json(request())
        .await
        .expect_err("401 should map to auth");

    assert!(matches!(rate_limit, BrokerError::RateLimit { .. }));
    assert!(matches!(auth, BrokerError::Auth { .. }));
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn retry_policy_allows_idempotent_reads_but_blocks_ambiguous_writes() {
    assert!(EndpointGroup::ReadLight.should_retry(TransportFailureClass::Http5xx));
    assert!(EndpointGroup::ReadHeavy.should_retry(TransportFailureClass::ReadTimeout));
    assert!(EndpointGroup::ReadLight.should_retry(TransportFailureClass::RateLimit));
    assert!(!EndpointGroup::WriteOrder.should_retry(TransportFailureClass::AmbiguousAfterWrite));
    assert!(!EndpointGroup::WriteOrder.should_retry(TransportFailureClass::Http5xx));
    assert!(!EndpointGroup::WriteOrder.should_retry(TransportFailureClass::RateLimit));
    assert!(!EndpointGroup::Auth.should_retry(TransportFailureClass::Validation));
}

#[test]
fn write_request_requires_hashkey_when_endpoint_declares_it() {
    let transport = transport_for("http://127.0.0.1:65535");
    let endpoint = KisEndpoint {
        name: "cash_buy_order",
        method: KisHttpMethod::Post,
        path: "/uapi/domestic-stock/v1/trading/order-cash",
        tr_id: "VTTC0802U",
        group: EndpointGroup::WriteOrder,
        requires_hashkey: true,
    };

    let error = transport
        .prepare_json(endpoint, &[], Some(json!({"ORD_QTY": "1"})))
        .expect_err("write order without hashkey should fail");

    assert!(error.to_string().contains("hashkey"));
}
