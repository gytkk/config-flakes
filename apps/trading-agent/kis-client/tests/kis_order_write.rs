#![cfg(feature = "internal-test-support")]

#[path = "support/memory_journal.rs"]
mod memory_journal;
mod support;

use memory_journal::MemoryCommandJournal;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::sync::Arc;
use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use time::OffsetDateTime;
use time::macros::datetime;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::config::{BrokerRuntimeConfig, MarketSessionWriteGate};
use trading_agent_kis_client::domain::{
    AccountKey, AmendOrderRequest, BrokerEnvironment, BrokerOrderId, BrokerOrderOrgNo,
    CancelOrderRequest, ClientOrderId, CreateOrderRequest, ExchangeCode, OrderReference, OrderSide,
    Quantity, RequestId, Symbol,
};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::protocol::{
    domestic_stock_cash_order_endpoint, domestic_stock_rvsecncl_order_endpoint,
};
use trading_agent_kis_client::journal::{
    BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus,
};
use trading_agent_kis_client::safety::{
    SingleWriterLock, SingleWriterLockConfig, SingleWriterLockScope,
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

fn buy_order(client_order_id: &str) -> CreateOrderRequest {
    CreateOrderRequest::cash_limit(
        ClientOrderId::new(client_order_id).expect("client order id"),
        account_key(),
        Symbol::new("005930").expect("symbol"),
        OrderSide::Buy,
        Quantity::new(10),
        Decimal::new(70_000, 0),
    )
    .expect("order request")
}

fn accepted_reference() -> OrderReference {
    reference_with_broker_order_id("0000001234")
}

fn duplicate_possible_fixture() -> String {
    let mut body: serde_json::Value =
        serde_json::from_str(&load_fixture("orders/psbl_rvsecncl_success.json"))
            .expect("rvsecncl possible fixture");
    let first = body["output"][0].clone();
    body["output"]
        .as_array_mut()
        .expect("output array")
        .push(first);
    serde_json::to_string(&body).expect("serialize duplicate possible fixture")
}

fn reference_with_broker_order_id(broker_order_id: &str) -> OrderReference {
    OrderReference {
        client_order_id: ClientOrderId::new("coid-parent").expect("client order id"),
        broker_order_id: Some(BrokerOrderId::new(broker_order_id).expect("broker order id")),
        broker_order_orgno: Some(BrokerOrderOrgNo::new("001").expect("broker orgno")),
        exchange_code: Some(ExchangeCode::new("KRX").expect("exchange code")),
        account_key: account_key(),
        environment: BrokerEnvironment::Paper,
    }
}

async fn journal() -> Arc<MemoryCommandJournal> {
    Arc::new(MemoryCommandJournal::new())
}

fn live_runtime_config() -> BrokerRuntimeConfig {
    BrokerRuntimeConfig {
        read_only: false,
        enable_live_trading: true,
        confirmed_account_suffix: Some("01".to_owned()),
        allowed_live_accounts: vec![account_key().to_string()],
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: BrokerRuntimeConfig::regular_us_equity_order_sessions(),
    }
}

fn live_runtime_config_with_market_gate(
    market_session_write_gate: MarketSessionWriteGate,
) -> BrokerRuntimeConfig {
    BrokerRuntimeConfig {
        market_session_write_gate,
        ..live_runtime_config()
    }
}

fn kst_day_at(now_utc: OffsetDateTime) -> String {
    let day = now_utc.to_offset(time::macros::offset!(+9)).date();
    format!(
        "{:04}{:02}{:02}",
        day.year(),
        u8::from(day.month()),
        day.day()
    )
}

fn regular_session_now() -> OffsetDateTime {
    datetime!(2026-04-24 01:00:00 UTC)
}

fn after_regular_session_now() -> OffsetDateTime {
    datetime!(2026-04-24 07:00:00 UTC)
}

fn holiday_response(trading_day: &str, is_open: bool) -> String {
    json!({
        "rt_cd": "0",
        "msg_cd": "MCA00000",
        "msg1": "ok",
        "ctx_area_fk": "",
        "ctx_area_nk": "",
        "output": [
            {
                "bass_dt": trading_day,
                "opnd_yn": if is_open { "Y" } else { "N" }
            }
        ]
    })
    .to_string()
}

fn live_single_writer() -> Arc<SingleWriterLock> {
    let tempdir = tempfile::tempdir().expect("tempdir");
    Arc::new(
        SingleWriterLock::acquire(SingleWriterLockConfig {
            lock_dir: tempdir.keep(),
            environment: BrokerEnvironment::Live,
            account_key: account_key(),
            scope: SingleWriterLockScope::DomesticStock,
        })
        .expect("single writer lock"),
    )
}

#[test]
fn cash_order_endpoint_contract_matches_official_kis_sample() {
    let paper_buy = domestic_stock_cash_order_endpoint(BrokerEnvironment::Paper, OrderSide::Buy)
        .expect("paper buy endpoint");
    let live_sell = domestic_stock_cash_order_endpoint(BrokerEnvironment::Live, OrderSide::Sell)
        .expect("live sell endpoint");

    assert_eq!(paper_buy.path, "/uapi/domestic-stock/v1/trading/order-cash");
    assert_eq!(paper_buy.tr_id, "VTTC0012U");
    assert_eq!(live_sell.tr_id, "TTTC0011U");
}

#[tokio::test]
async fn place_order_reserves_journal_before_submit_and_records_ack() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/order_cash_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let submission = client
        .place_order(buy_order("coid-place-1"))
        .await
        .expect("submission");

    assert!(submission.accepted);
    assert_eq!(
        submission
            .reference
            .broker_order_id
            .as_ref()
            .unwrap()
            .as_str(),
        "0000001234"
    );

    let stored = journal
        .get_command(&ClientOrderId::new("coid-place-1").expect("client id"))
        .await
        .expect("lookup")
        .expect("record exists");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Accepted);
    assert_eq!(stored.broker_order_id.as_deref(), Some("0000001234"));

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(
        requests[0].path,
        "/uapi/domestic-stock/v1/trading/order-cash"
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("VTTC0012U")
    );
    let body: Value = serde_json::from_str(&requests[0].body).expect("request body");
    assert_eq!(body["CANO"], "12345678");
    assert_eq!(body["ACNT_PRDT_CD"], "01");
    assert_eq!(body["PDNO"], "005930");
    assert_eq!(body["ORD_DVSN"], "00");
    assert_eq!(body["ORD_QTY"], "10");
    assert_eq!(body["ORD_UNPR"], "70000");
    assert_eq!(body["EXCG_ID_DVSN_CD"], "KRX");
}

#[tokio::test]
async fn duplicate_client_order_id_is_blocked_before_fake_server_call() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal);

    let request = buy_order("coid-duplicate");
    client
        .place_order(request.clone())
        .await
        .expect_err("no fake response means first submit becomes reconcile-needed");
    let error = client
        .place_order(request)
        .await
        .expect_err("duplicate must fail before broker call");

    assert!(matches!(error, BrokerError::DuplicateIntent { .. }));
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn pre_reserved_place_order_submits_existing_command_without_duplicate() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/order_cash_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());
    let request = buy_order("coid-pre-reserved");
    let command = BrokerCommandRecord::new_place_order(BrokerEnvironment::Paper, request.clone())
        .expect("broker command");
    journal
        .reserve_command(command)
        .await
        .expect("pre-reserve command");

    let submission = client
        .place_order(request)
        .await
        .expect("pre-reserved submission");

    assert!(submission.accepted);
    assert_eq!(server.requests().len(), 1);
    let stored = journal
        .get_command(&ClientOrderId::new("coid-pre-reserved").expect("client id"))
        .await
        .expect("lookup")
        .expect("stored command");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Accepted);
}

#[tokio::test]
async fn ambiguous_place_order_response_marks_reconcile_needed_without_retry() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        500,
        r#"{"rt_cd":"1","msg_cd":"EGW001","msg1":"temporary"}"#,
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .place_order(buy_order("coid-ambiguous"))
        .await
        .expect_err("ambiguous outcome requires reconciliation");

    assert!(matches!(error, BrokerError::ReconciliationNeeded { .. }));
    assert_eq!(server.requests().len(), 1);

    let stored = journal
        .get_command(&ClientOrderId::new("coid-ambiguous").expect("client id"))
        .await
        .expect("lookup")
        .expect("record exists");
    assert_eq!(
        stored.last_known_state,
        BrokerCommandStatus::SubmittedAckUnknown
    );
    assert!(stored.reconcile_required);
}

#[tokio::test]
async fn order_rejection_is_terminal_and_not_reconcile_needed() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/order_rejected.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .place_order(buy_order("coid-rejected"))
        .await
        .expect_err("broker rejection must surface");

    assert!(matches!(error, BrokerError::OrderRejected { .. }));
    let stored = journal
        .get_command(&ClientOrderId::new("coid-rejected").expect("client id"))
        .await
        .expect("lookup")
        .expect("record exists");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Rejected);
    assert!(!stored.reconcile_required);
}

#[tokio::test]
async fn read_only_guardrail_blocks_write_before_fake_server_call() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(BrokerRuntimeConfig {
        read_only: true,
        enable_live_trading: false,
        confirmed_account_suffix: None,
        allowed_live_accounts: vec![],
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: BrokerRuntimeConfig::regular_us_equity_order_sessions(),
    });

    let error = client
        .place_order(buy_order("coid-read-only"))
        .await
        .expect_err("read-only must block writes");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn live_place_order_blocks_closed_market_before_order_post() {
    let server = FakeKisServer::start();
    let now = regular_session_now();
    server.push_response(FakeResponse::json(
        200,
        holiday_response(&kst_day_at(now), false),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(now);

    let error = client
        .place_order(buy_order("coid-live-closed-market"))
        .await
        .expect_err("closed market must block live order");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].path.contains("chk-holiday"));
}

#[tokio::test]
async fn live_place_order_blocks_unknown_market_before_order_post() {
    let server = FakeKisServer::start();
    let now = regular_session_now();
    server.push_response(FakeResponse::json(
        500,
        r#"{"rt_cd":"1","msg_cd":"EGW001","msg1":"temporary"}"#,
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(now);

    let error = client
        .place_order(buy_order("coid-live-unknown-market"))
        .await
        .expect_err("unknown market session must block live order");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request.path.contains("chk-holiday"))
    );
}

#[tokio::test]
async fn live_place_order_allows_open_market_and_submits() {
    let server = FakeKisServer::start();
    let now = regular_session_now();
    server.push_response(FakeResponse::json(
        200,
        holiday_response(&kst_day_at(now), true),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/order_cash_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(now);

    let submission = client
        .place_order(buy_order("coid-live-open-market"))
        .await
        .expect("live order");

    assert!(submission.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].path.contains("chk-holiday"));
    assert_eq!(
        requests[1].path,
        "/uapi/domestic-stock/v1/trading/order-cash"
    );
}

#[tokio::test]
async fn live_place_order_blocks_after_regular_session_before_order_post() {
    let server = FakeKisServer::start();
    let now = after_regular_session_now();
    server.push_response(FakeResponse::json(
        200,
        holiday_response(&kst_day_at(now), true),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(now);

    let error = client
        .place_order(buy_order("coid-live-after-regular-session"))
        .await
        .expect_err("regular-session gate must block after close");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].path.contains("chk-holiday"));
}

#[tokio::test]
async fn live_write_without_runtime_config_stops_before_fake_server_call() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal);

    let error = client
        .place_order(buy_order("coid-live-no-config"))
        .await
        .expect_err("live write without runtime config must halt");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn live_write_with_mismatched_single_writer_lock_stops_before_fake_server_call() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let tempdir = tempfile::tempdir().expect("tempdir");
    let single_writer = Arc::new(
        SingleWriterLock::acquire(SingleWriterLockConfig {
            lock_dir: tempdir.path().to_path_buf(),
            environment: BrokerEnvironment::Live,
            account_key: AccountKey::new("87654321-01").expect("other account key"),
            scope: SingleWriterLockScope::DomesticStock,
        })
        .expect("single writer lock"),
    );
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config_with_market_gate(
        MarketSessionWriteGate::DisabledForTesting,
    ))
    .with_single_writer_lock(single_writer);

    let error = client
        .place_order(buy_order("coid-live-wrong-lock"))
        .await
        .expect_err("mismatched single-writer lock must halt live write");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn live_write_with_mismatched_single_writer_market_scope_stops_before_fake_server_call() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let tempdir = tempfile::tempdir().expect("tempdir");
    let single_writer = Arc::new(
        SingleWriterLock::acquire(SingleWriterLockConfig {
            lock_dir: tempdir.path().to_path_buf(),
            environment: BrokerEnvironment::Live,
            account_key: account_key(),
            scope: SingleWriterLockScope::OverseasStock,
        })
        .expect("single writer lock"),
    );
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config_with_market_gate(
        MarketSessionWriteGate::DisabledForTesting,
    ))
    .with_single_writer_lock(single_writer);

    let error = client
        .place_order(buy_order("coid-live-wrong-lock-scope"))
        .await
        .expect_err("mismatched single-writer lock scope must halt live write");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn unresolved_live_journal_blocks_write_before_fake_server_call() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let tempdir = tempfile::tempdir().expect("tempdir");
    let single_writer = Arc::new(
        SingleWriterLock::acquire(SingleWriterLockConfig {
            lock_dir: tempdir.path().to_path_buf(),
            environment: BrokerEnvironment::Live,
            account_key: account_key(),
            scope: SingleWriterLockScope::DomesticStock,
        })
        .expect("single writer lock"),
    );
    let unresolved_order = buy_order("coid-live-unresolved-existing");
    journal
        .reserve_command(
            trading_agent_kis_client::journal::BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Live,
                unresolved_order,
            )
            .expect("unresolved command"),
        )
        .await
        .expect("reserve unresolved");
    journal
        .mark_submitting(
            &ClientOrderId::new("coid-live-unresolved-existing").expect("client id"),
            &RequestId::new("req-live-unresolved-existing").expect("request id"),
        )
        .await
        .expect("mark submitting unresolved");
    journal
        .mark_reconcile_needed(
            &ClientOrderId::new("coid-live-unresolved-existing").expect("client id"),
            "startup recovery pending",
        )
        .await
        .expect("mark unresolved");

    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(single_writer);

    let error = client
        .place_order(buy_order("coid-live-blocked-new"))
        .await
        .expect_err("unresolved journal must block live write");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(server.requests().is_empty());
}

#[test]
fn rvsecncl_endpoint_contract_matches_official_kis_sample() {
    let endpoint = domestic_stock_rvsecncl_order_endpoint(BrokerEnvironment::Paper)
        .expect("paper rvsecncl endpoint");

    assert_eq!(
        endpoint.path,
        "/uapi/domestic-stock/v1/trading/order-rvsecncl"
    );
    assert_eq!(endpoint.tr_id, "VTTC0013U");
}

#[tokio::test]
async fn cancel_order_rejects_incomplete_order_reference_before_fake_server_call() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let mut reference = accepted_reference();
    reference.broker_order_orgno = None;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal);

    let error = client
        .cancel_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("coid-cancel").expect("client id"),
            reference,
            quantity: None,
        })
        .await
        .expect_err("incomplete reference must fail");

    assert!(matches!(error, BrokerError::Validation { .. }));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn cancel_order_uses_rvsecncl_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/order_rvsecncl_success.json"),
    ));
    let journal = journal().await;
    let parent = buy_order("coid-parent");
    journal
        .reserve_command(
            trading_agent_kis_client::journal::BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Paper,
                parent,
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal);

    let result = client
        .cancel_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("coid-cancel-1").expect("client id"),
            reference: accepted_reference(),
            quantity: None,
        })
        .await
        .expect("cancel result");

    assert!(result.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].path.contains("inquire-psbl-rvsecncl"));
    let request = requests.last().expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.path,
        "/uapi/domestic-stock/v1/trading/order-rvsecncl"
    );
    let body: Value = serde_json::from_str(&request.body).expect("request body");
    assert_eq!(body["KRX_FWDG_ORD_ORGNO"], "001");
    assert_eq!(body["ORGN_ODNO"], "0000001234");
    assert_eq!(body["RVSE_CNCL_DVSN_CD"], "02");
    assert_eq!(body["QTY_ALL_ORD_YN"], "Y");
}

#[tokio::test]
async fn cancel_order_uses_pre_reserved_follow_up_command() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/order_rvsecncl_success.json"),
    ));
    let journal = journal().await;
    let parent =
        BrokerCommandRecord::new_place_order(BrokerEnvironment::Paper, buy_order("coid-parent"))
            .expect("parent record");
    journal
        .reserve_command(parent.clone())
        .await
        .expect("reserve parent");
    let request = CancelOrderRequest {
        client_order_id: ClientOrderId::new("coid-cancel-pre-reserved").expect("client id"),
        reference: accepted_reference(),
        quantity: Some(Quantity::new(3)),
    };
    journal
        .reserve_command(BrokerCommandRecord::new_cancel_order(&parent, &request).expect("cancel"))
        .await
        .expect("pre-reserve cancel");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let result = client
        .cancel_order(request.clone())
        .await
        .expect("cancel result");

    assert!(result.accepted);
    assert_eq!(server.requests().len(), 2);
    let command = journal
        .get_command(&request.client_order_id)
        .await
        .expect("lookup")
        .expect("cancel command");
    assert_eq!(command.last_known_state, BrokerCommandStatus::Cancelled);
}

#[tokio::test]
async fn cancel_order_rejects_non_open_order_before_write_call() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    let journal = journal().await;
    let parent = buy_order("coid-parent");
    journal
        .reserve_command(
            trading_agent_kis_client::journal::BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Paper,
                parent,
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .cancel_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("coid-cancel-not-open").expect("client id"),
            reference: reference_with_broker_order_id("0000005678"),
            quantity: None,
        })
        .await
        .expect_err("missing possible order must fail preflight");

    assert!(matches!(error, BrokerError::Validation { .. }));
    assert_eq!(server.requests().len(), 1);
    assert!(server.requests()[0].path.contains("inquire-psbl-rvsecncl"));
    assert!(
        journal
            .get_command(&ClientOrderId::new("coid-cancel-not-open").expect("client id"))
            .await
            .expect("lookup")
            .is_none()
    );
}

#[tokio::test]
async fn cancel_order_rejects_quantity_above_possible_before_write_call() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    let journal = journal().await;
    let parent = buy_order("coid-parent");
    journal
        .reserve_command(
            trading_agent_kis_client::journal::BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Paper,
                parent,
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .cancel_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("coid-cancel-too-large").expect("client id"),
            reference: accepted_reference(),
            quantity: Some(Quantity::new(9)),
        })
        .await
        .expect_err("quantity above KIS possible quantity must fail");

    assert!(matches!(error, BrokerError::Validation { .. }));
    assert_eq!(server.requests().len(), 1);
    assert!(server.requests()[0].path.contains("inquire-psbl-rvsecncl"));
    assert!(
        journal
            .get_command(&ClientOrderId::new("coid-cancel-too-large").expect("client id"))
            .await
            .expect("lookup")
            .is_none()
    );
}

#[tokio::test]
async fn duplicate_possible_cancel_rows_block_write_call() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(200, duplicate_possible_fixture()));
    let journal = journal().await;
    let parent = buy_order("coid-parent");
    journal
        .reserve_command(
            trading_agent_kis_client::journal::BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Paper,
                parent,
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .cancel_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("coid-cancel-duplicate").expect("client id"),
            reference: accepted_reference(),
            quantity: Some(Quantity::new(1)),
        })
        .await
        .expect_err("duplicate possible rows must fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    assert_eq!(server.requests().len(), 1);
    assert!(
        journal
            .get_command(&ClientOrderId::new("coid-cancel-duplicate").expect("client id"))
            .await
            .expect("lookup")
            .is_none()
    );
}

#[tokio::test]
async fn amend_order_uses_rvsecncl_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/order_rvsecncl_success.json"),
    ));
    let journal = journal().await;
    let parent = buy_order("coid-parent");
    journal
        .reserve_command(
            trading_agent_kis_client::journal::BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Paper,
                parent,
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal);

    let result = client
        .amend_order(AmendOrderRequest {
            client_order_id: ClientOrderId::new("coid-amend-1").expect("client id"),
            reference: accepted_reference(),
            quantity: Some(Quantity::new(5)),
            limit_price: Some(Decimal::new(69_000, 0)),
        })
        .await
        .expect("amend result");

    assert!(result.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].path.contains("inquire-psbl-rvsecncl"));
    let request = requests.last().expect("request");
    assert_eq!(request.method, "POST");
    let body: Value = serde_json::from_str(&request.body).expect("request body");
    assert_eq!(body["RVSE_CNCL_DVSN_CD"], "01");
    assert_eq!(body["ORD_QTY"], "5");
    assert_eq!(body["ORD_UNPR"], "69000");
    assert_eq!(body["QTY_ALL_ORD_YN"], "N");
}

#[tokio::test]
async fn amend_order_uses_pre_reserved_follow_up_command() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/order_rvsecncl_success.json"),
    ));
    let journal = journal().await;
    let parent =
        BrokerCommandRecord::new_place_order(BrokerEnvironment::Paper, buy_order("coid-parent"))
            .expect("parent record");
    journal
        .reserve_command(parent.clone())
        .await
        .expect("reserve parent");
    let request = AmendOrderRequest {
        client_order_id: ClientOrderId::new("coid-amend-pre-reserved").expect("client id"),
        reference: accepted_reference(),
        quantity: Some(Quantity::new(5)),
        limit_price: Some(Decimal::new(69_000, 0)),
    };
    journal
        .reserve_command(BrokerCommandRecord::new_amend_order(&parent, &request).expect("amend"))
        .await
        .expect("pre-reserve amend");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let result = client
        .amend_order(request.clone())
        .await
        .expect("amend result");

    assert!(result.accepted);
    assert_eq!(server.requests().len(), 2);
    let command = journal
        .get_command(&request.client_order_id)
        .await
        .expect("lookup")
        .expect("amend command");
    assert_eq!(command.last_known_state, BrokerCommandStatus::Accepted);
}

#[tokio::test]
async fn amend_order_rejects_missing_open_order_before_write_call() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    let journal = journal().await;
    let parent = buy_order("coid-parent");
    journal
        .reserve_command(
            trading_agent_kis_client::journal::BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Paper,
                parent,
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .amend_order(AmendOrderRequest {
            client_order_id: ClientOrderId::new("coid-amend-missing").expect("client id"),
            reference: reference_with_broker_order_id("9999999999"),
            quantity: Some(Quantity::new(5)),
            limit_price: Some(Decimal::new(69_000, 0)),
        })
        .await
        .expect_err("missing possible order must fail preflight");

    assert!(matches!(error, BrokerError::Validation { .. }));
    assert_eq!(server.requests().len(), 1);
    assert!(server.requests()[0].path.contains("inquire-psbl-rvsecncl"));
    assert!(
        journal
            .get_command(&ClientOrderId::new("coid-amend-missing").expect("client id"))
            .await
            .expect("lookup")
            .is_none()
    );
}

#[tokio::test]
async fn amend_order_rejects_quantity_above_possible_before_write_call() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/psbl_rvsecncl_success.json"),
    ));
    let journal = journal().await;
    let parent = buy_order("coid-parent");
    journal
        .reserve_command(
            trading_agent_kis_client::journal::BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Paper,
                parent,
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .amend_order(AmendOrderRequest {
            client_order_id: ClientOrderId::new("coid-amend-too-large").expect("client id"),
            reference: accepted_reference(),
            quantity: Some(Quantity::new(9)),
            limit_price: Some(Decimal::new(69_000, 0)),
        })
        .await
        .expect_err("quantity above KIS possible quantity must fail");

    assert!(matches!(error, BrokerError::Validation { .. }));
    assert_eq!(server.requests().len(), 1);
    assert!(server.requests()[0].path.contains("inquire-psbl-rvsecncl"));
    assert!(
        journal
            .get_command(&ClientOrderId::new("coid-amend-too-large").expect("client id"))
            .await
            .expect("lookup")
            .is_none()
    );
}
