#![cfg(feature = "internal-test-support")]

#[path = "support/memory_journal.rs"]
mod memory_journal;
mod support;

use memory_journal::MemoryCommandJournal;
use rust_decimal::Decimal;
use serde_json::Value;
use std::sync::Arc;
use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use time::{OffsetDateTime, macros::datetime};
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::config::{BrokerRuntimeConfig, MarketSessionWriteGate};
use trading_agent_kis_client::domain::{
    AccountKey, AmendOrderRequest, BrokerEnvironment, BrokerOrderId, BrokerOrderOrgNo,
    CancelOrderRequest, ClientOrderId, CreateOrderRequest, ExchangeCode, OrderReference, OrderSide,
    OverseasStock, OverseasStockOrderRequest, Quantity, Symbol, UsEquityOrderSession,
};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::internal::protocol::{
    overseas_stock_order_endpoint, overseas_stock_rvsecncl_order_endpoint,
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

fn nasd_aapl() -> OverseasStock {
    OverseasStock::new(
        ExchangeCode::new("NASD").expect("exchange"),
        Symbol::new("AAPL").expect("symbol"),
    )
}

fn buy_order(client_order_id: &str) -> OverseasStockOrderRequest {
    OverseasStockOrderRequest::cash_limit(
        ClientOrderId::new(client_order_id).expect("client order id"),
        account_key(),
        nasd_aapl(),
        OrderSide::Buy,
        Quantity::new(10),
        Decimal::new(19825, 2),
    )
    .expect("order request")
}

fn accepted_reference() -> OrderReference {
    OrderReference {
        client_order_id: ClientOrderId::new("coid-overseas-parent").expect("client order id"),
        broker_order_id: Some(BrokerOrderId::new("0000116058").expect("broker order id")),
        broker_order_orgno: Some(BrokerOrderOrgNo::new("91252").expect("broker orgno")),
        exchange_code: Some(ExchangeCode::new("NASD").expect("exchange")),
        account_key: account_key(),
        environment: BrokerEnvironment::Paper,
    }
}

fn accepted_live_reference() -> OrderReference {
    OrderReference {
        environment: BrokerEnvironment::Live,
        ..accepted_reference()
    }
}

async fn journal() -> Arc<MemoryCommandJournal> {
    Arc::new(MemoryCommandJournal::new())
}

async fn journal_with_overseas_parent() -> Arc<MemoryCommandJournal> {
    let journal = journal().await;
    journal
        .reserve_command(
            BrokerCommandRecord::new_place_overseas_stock_order(
                BrokerEnvironment::Paper,
                buy_order("coid-overseas-parent"),
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    journal
}

async fn journal_with_live_overseas_parent() -> Arc<MemoryCommandJournal> {
    let journal = journal().await;
    journal
        .reserve_command(
            BrokerCommandRecord::new_place_overseas_stock_order(
                BrokerEnvironment::Live,
                buy_order("coid-overseas-parent"),
            )
            .expect("parent record"),
        )
        .await
        .expect("reserve parent");
    journal
}

fn live_runtime_config() -> BrokerRuntimeConfig {
    BrokerRuntimeConfig {
        read_only: false,
        enable_live_trading: true,
        confirmed_account_suffix: Some("01".to_owned()),
        allowed_live_accounts: vec![account_key().to_string()],
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: vec![UsEquityOrderSession::Regular],
    }
}

fn us_regular_session_now() -> OffsetDateTime {
    datetime!(2026-04-24 14:00:00 UTC)
}

fn us_daytime_session_now() -> OffsetDateTime {
    datetime!(2026-04-24 01:00:00 UTC)
}

fn us_premarket_session_now() -> OffsetDateTime {
    datetime!(2026-04-24 09:00:00 UTC)
}

fn us_after_regular_session_now() -> OffsetDateTime {
    datetime!(2026-04-24 21:00:00 UTC)
}

fn live_runtime_config_with_sessions(
    allowed_us_equity_order_sessions: Vec<UsEquityOrderSession>,
) -> BrokerRuntimeConfig {
    BrokerRuntimeConfig {
        allowed_us_equity_order_sessions,
        ..live_runtime_config()
    }
}

fn live_single_writer() -> Arc<SingleWriterLock> {
    let tempdir = tempfile::tempdir().expect("tempdir");
    Arc::new(
        SingleWriterLock::acquire(SingleWriterLockConfig {
            lock_dir: tempdir.keep(),
            environment: BrokerEnvironment::Live,
            account_key: account_key(),
            scope: SingleWriterLockScope::OverseasStock,
        })
        .expect("single writer lock"),
    )
}

#[test]
fn overseas_order_endpoint_contract_matches_official_kis_sample() {
    let paper_buy = overseas_stock_order_endpoint(
        BrokerEnvironment::Paper,
        OrderSide::Buy,
        &ExchangeCode::new("NASD").expect("exchange"),
    )
    .expect("paper buy endpoint");
    let live_sell = overseas_stock_order_endpoint(
        BrokerEnvironment::Live,
        OrderSide::Sell,
        &ExchangeCode::new("SEHK").expect("exchange"),
    )
    .expect("live sell endpoint");

    assert_eq!(paper_buy.path, "/uapi/overseas-stock/v1/trading/order");
    assert_eq!(paper_buy.tr_id, "VTTT1002U");
    assert_eq!(live_sell.tr_id, "TTTS1001U");

    let paper_us_sell = overseas_stock_order_endpoint(
        BrokerEnvironment::Paper,
        OrderSide::Sell,
        &ExchangeCode::new("NASD").expect("exchange"),
    )
    .expect("paper US sell endpoint");
    assert_eq!(paper_us_sell.tr_id, "VTTT1001U");
}

#[test]
fn overseas_daytime_order_endpoint_contract_matches_official_kis_guide() {
    let buy = trading_agent_kis_client::internal::protocol::overseas_stock_daytime_order_endpoint(
        BrokerEnvironment::Live,
        OrderSide::Buy,
    )
    .expect("live daytime buy endpoint");
    let sell = trading_agent_kis_client::internal::protocol::overseas_stock_daytime_order_endpoint(
        BrokerEnvironment::Live,
        OrderSide::Sell,
    )
    .expect("live daytime sell endpoint");
    let paper =
        trading_agent_kis_client::internal::protocol::overseas_stock_daytime_order_endpoint(
            BrokerEnvironment::Paper,
            OrderSide::Buy,
        )
        .expect_err("paper daytime is unsupported");

    assert_eq!(buy.path, "/uapi/overseas-stock/v1/trading/daytime-order");
    assert_eq!(buy.tr_id, "TTTS6036U");
    assert_eq!(sell.tr_id, "TTTS6037U");
    assert!(matches!(paper, BrokerError::Validation { .. }));
}

#[test]
fn overseas_rvsecncl_endpoint_contract_matches_official_kis_sample() {
    let paper =
        overseas_stock_rvsecncl_order_endpoint(BrokerEnvironment::Paper).expect("paper endpoint");
    let live =
        overseas_stock_rvsecncl_order_endpoint(BrokerEnvironment::Live).expect("live endpoint");

    assert_eq!(paper.path, "/uapi/overseas-stock/v1/trading/order-rvsecncl");
    assert_eq!(paper.tr_id, "VTTT1004U");
    assert_eq!(live.tr_id, "TTTT1004U");
}

#[tokio::test]
async fn place_overseas_order_reserves_journal_before_submit_and_records_ack() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let submission = client
        .place_overseas_stock_order(buy_order("coid-overseas-place-1"))
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
        "0000116058"
    );
    assert_eq!(
        submission
            .reference
            .exchange_code
            .as_ref()
            .unwrap()
            .as_str(),
        "NASD"
    );

    let stored = journal
        .get_command(&ClientOrderId::new("coid-overseas-place-1").expect("client id"))
        .await
        .expect("lookup")
        .expect("record exists");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Accepted);
    assert_eq!(stored.broker_order_id.as_deref(), Some("0000116058"));
    assert_eq!(stored.exchange_code.as_deref(), Some("NASD"));

    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/uapi/overseas-stock/v1/trading/order");
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("VTTT1002U")
    );
    assert!(!requests[0].headers.contains_key("hashkey"));
    let body: Value = serde_json::from_str(&requests[0].body).expect("request body");
    assert_eq!(body["CANO"], "12345678");
    assert_eq!(body["ACNT_PRDT_CD"], "01");
    assert_eq!(body["OVRS_EXCG_CD"], "NASD");
    assert_eq!(body["PDNO"], "AAPL");
    assert_eq!(body["ORD_QTY"], "10");
    assert_eq!(body["OVRS_ORD_UNPR"], "198.25");
    assert_eq!(body["SLL_TYPE"], "");
    assert_eq!(body["ORD_SVR_DVSN_CD"], "0");
    assert_eq!(body["ORD_DVSN"], "00");
}

#[tokio::test]
async fn pre_reserved_overseas_place_order_uses_existing_journal_command() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());
    let request = buy_order("coid-overseas-pre-reserved");
    let mut command = BrokerCommandRecord::new_place_order(
        BrokerEnvironment::Paper,
        CreateOrderRequest::cash_limit(
            request.client_order_id.clone(),
            request.account_key.clone(),
            request.instrument.symbol.clone(),
            request.side,
            request.quantity,
            request.limit_price.expect("limit price"),
        )
        .expect("generic order request"),
    )
    .expect("generic place command");
    command.exchange_code = Some(request.instrument.exchange.to_string());
    journal
        .reserve_command(command)
        .await
        .expect("reserve pre-existing command");

    let submission = client
        .place_overseas_stock_order(request)
        .await
        .expect("submission");

    assert!(submission.accepted);
    assert_eq!(server.requests().len(), 1);
    let stored = journal
        .get_command(&ClientOrderId::new("coid-overseas-pre-reserved").expect("client id"))
        .await
        .expect("lookup")
        .expect("record exists");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Accepted);
    assert_eq!(stored.exchange_code.as_deref(), Some("NASD"));
}

#[tokio::test]
async fn duplicate_overseas_client_order_id_is_blocked_before_fake_server_call() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal);

    let request = buy_order("coid-overseas-duplicate");
    client
        .place_overseas_stock_order(request.clone())
        .await
        .expect_err("no fake response means first submit becomes reconcile-needed");
    let error = client
        .place_overseas_stock_order(request)
        .await
        .expect_err("duplicate must fail before broker call");

    assert!(matches!(error, BrokerError::DuplicateIntent { .. }));
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn ambiguous_overseas_place_response_marks_reconcile_needed_without_retry() {
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
        .place_overseas_stock_order(buy_order("coid-overseas-ambiguous"))
        .await
        .expect_err("ambiguous outcome requires reconciliation");

    assert!(matches!(error, BrokerError::ReconciliationNeeded { .. }));
    assert_eq!(server.requests().len(), 1);

    let stored = journal
        .get_command(&ClientOrderId::new("coid-overseas-ambiguous").expect("client id"))
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
async fn overseas_order_rejection_is_terminal_and_not_reconcile_needed() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_rejected.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .place_overseas_stock_order(buy_order("coid-overseas-rejected"))
        .await
        .expect_err("broker rejection must surface");

    assert!(matches!(error, BrokerError::OrderRejected { .. }));
    let stored = journal
        .get_command(&ClientOrderId::new("coid-overseas-rejected").expect("client id"))
        .await
        .expect("lookup")
        .expect("record exists");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Rejected);
    assert!(!stored.reconcile_required);
}

#[tokio::test]
async fn read_only_guardrail_blocks_overseas_write_before_fake_server_call() {
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
        .place_overseas_stock_order(buy_order("coid-overseas-read-only"))
        .await
        .expect_err("read-only must block writes");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn live_overseas_place_order_allows_open_us_regular_market_and_submits() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_open.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(us_regular_session_now());

    let submission = client
        .place_overseas_stock_order(buy_order("coid-overseas-live-open"))
        .await
        .expect("live overseas order");

    assert!(submission.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-stock/v1/quotations/countries-holiday?TRAD_DT=20260424&CTX_AREA_NK=&CTX_AREA_FK="
    );
    assert_eq!(
        requests[0].headers.get("tr_id").map(String::as_str),
        Some("CTOS5011R")
    );
    assert_eq!(requests[1].path, "/uapi/overseas-stock/v1/trading/order");
}

#[tokio::test]
async fn live_overseas_place_order_blocks_closed_us_market_before_submit() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_closed.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(us_regular_session_now());

    let error = client
        .place_overseas_stock_order(buy_order("coid-overseas-live-closed"))
        .await
        .expect_err("closed overseas market must block live submit");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(error.to_string().contains("market session is closed"));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].path.contains("countries-holiday"));
}

#[tokio::test]
async fn live_overseas_place_order_blocks_after_regular_session_before_submit() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_open.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(us_after_regular_session_now());

    let error = client
        .place_overseas_stock_order(buy_order("coid-overseas-live-after-hours"))
        .await
        .expect_err("after-hours overseas market must block live submit");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(
        error
            .to_string()
            .contains("KIS US equity order session aftermarket is not enabled")
    );
    let requests = server.requests();
    assert!(requests.is_empty());
}

#[tokio::test]
async fn live_overseas_place_order_allows_configured_premarket_session() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_open.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config_with_sessions(vec![
        UsEquityOrderSession::Regular,
        UsEquityOrderSession::Premarket,
    ]))
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(us_premarket_session_now());

    let submission = client
        .place_overseas_stock_order(buy_order("coid-overseas-live-premarket"))
        .await
        .expect("live premarket overseas order");

    assert!(submission.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].path, "/uapi/overseas-stock/v1/trading/order");
    assert_eq!(
        requests[1].headers.get("tr_id").map(String::as_str),
        Some("TTTT1002U")
    );
}

#[tokio::test]
async fn live_overseas_place_order_allows_configured_aftermarket_session() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_open.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config_with_sessions(vec![
        UsEquityOrderSession::Regular,
        UsEquityOrderSession::Aftermarket,
    ]))
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(us_after_regular_session_now());

    let submission = client
        .place_overseas_stock_order(buy_order("coid-overseas-live-aftermarket"))
        .await
        .expect("live aftermarket overseas order");

    assert!(submission.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].path, "/uapi/overseas-stock/v1/trading/order");
}

#[tokio::test]
async fn live_overseas_place_order_allows_configured_daytime_session() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_open.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config_with_sessions(vec![
        UsEquityOrderSession::Regular,
        UsEquityOrderSession::Daytime,
    ]))
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(us_daytime_session_now());

    let submission = client
        .place_overseas_stock_order(buy_order("coid-overseas-live-daytime"))
        .await
        .expect("live daytime overseas order");

    assert!(submission.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-stock/v1/quotations/countries-holiday?TRAD_DT=20260424&CTX_AREA_NK=&CTX_AREA_FK="
    );
    assert_eq!(
        requests[1].path,
        "/uapi/overseas-stock/v1/trading/daytime-order"
    );
    assert_eq!(
        requests[1].headers.get("tr_id").map(String::as_str),
        Some("TTTS6036U")
    );
    let body: Value = serde_json::from_str(&requests[1].body).expect("request body");
    assert_eq!(body["ORD_DVSN"], "00");
    assert_eq!(body["OVRS_ORD_UNPR"], "198.25");
}

#[tokio::test]
async fn live_overseas_cancel_checks_market_session_before_nccs_preflight() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_closed.json"),
    ));
    let journal = journal_with_live_overseas_parent().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(us_regular_session_now());

    let error = client
        .cancel_overseas_stock_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("coid-overseas-live-cancel-closed")
                .expect("client id"),
            reference: accepted_live_reference(),
            quantity: None,
        })
        .await
        .expect_err("closed overseas market must block live cancel");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].path.contains("countries-holiday"));
}

#[tokio::test]
async fn live_overseas_amend_checks_market_session_before_nccs_preflight() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/countries_holiday_us_closed.json"),
    ));
    let journal = journal_with_live_overseas_parent().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal)
    .with_runtime_config(live_runtime_config())
    .with_single_writer_lock(live_single_writer())
    .with_market_session_now(us_regular_session_now());

    let error = client
        .amend_overseas_stock_order(AmendOrderRequest {
            client_order_id: ClientOrderId::new("coid-overseas-live-amend-closed")
                .expect("client id"),
            reference: accepted_live_reference(),
            quantity: Some(Quantity::new(5)),
            limit_price: Some(Decimal::new(19900, 2)),
        })
        .await
        .expect_err("closed overseas market must block live amend");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].path.contains("countries-holiday"));
}

#[tokio::test]
async fn cancel_overseas_order_uses_nccs_preflight_and_rvsecncl_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/nccs_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_rvsecncl_success.json"),
    ));
    let journal = journal_with_overseas_parent().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal);

    let result = client
        .cancel_overseas_stock_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("coid-overseas-cancel-1").expect("client id"),
            reference: accepted_reference(),
            quantity: None,
        })
        .await
        .expect("cancel result");

    assert!(result.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].path,
        "/uapi/overseas-stock/v1/trading/inquire-nccs?CANO=12345678&ACNT_PRDT_CD=01&OVRS_EXCG_CD=NASD&SORT_SQN=DS&CTX_AREA_FK200=&CTX_AREA_NK200="
    );
    let request = requests.last().expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.path,
        "/uapi/overseas-stock/v1/trading/order-rvsecncl"
    );
    assert_eq!(
        request.headers.get("tr_id").map(String::as_str),
        Some("VTTT1004U")
    );
    let body: Value = serde_json::from_str(&request.body).expect("request body");
    assert_eq!(body["OVRS_EXCG_CD"], "NASD");
    assert_eq!(body["PDNO"], "AAPL");
    assert_eq!(body["ORGN_ODNO"], "0000116058");
    assert_eq!(body["RVSE_CNCL_DVSN_CD"], "02");
    assert_eq!(body["ORD_QTY"], "8");
    assert_eq!(body["OVRS_ORD_UNPR"], "0");
}

#[tokio::test]
async fn amend_overseas_order_uses_nccs_preflight_and_rvsecncl_request_shape() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/nccs_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/order_rvsecncl_success.json"),
    ));
    let journal = journal_with_overseas_parent().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal);

    let result = client
        .amend_overseas_stock_order(AmendOrderRequest {
            client_order_id: ClientOrderId::new("coid-overseas-amend-1").expect("client id"),
            reference: accepted_reference(),
            quantity: Some(Quantity::new(5)),
            limit_price: Some(Decimal::new(19900, 2)),
        })
        .await
        .expect("amend result");

    assert!(result.accepted);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].path.contains("inquire-nccs"));
    let request = requests.last().expect("request");
    let body: Value = serde_json::from_str(&request.body).expect("request body");
    assert_eq!(body["RVSE_CNCL_DVSN_CD"], "01");
    assert_eq!(body["ORD_QTY"], "5");
    assert_eq!(body["OVRS_ORD_UNPR"], "199.00");
}

#[tokio::test]
async fn cancel_overseas_order_rejects_non_open_order_before_write_call() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/nccs_success.json"),
    ));
    let journal = journal_with_overseas_parent().await;
    let mut reference = accepted_reference();
    reference.broker_order_id = Some(BrokerOrderId::new("0000119999").expect("broker id"));
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let error = client
        .cancel_overseas_stock_order(CancelOrderRequest {
            client_order_id: ClientOrderId::new("coid-overseas-cancel-not-open")
                .expect("client id"),
            reference,
            quantity: None,
        })
        .await
        .expect_err("missing possible order must fail preflight");

    assert!(matches!(error, BrokerError::Validation { .. }));
    assert_eq!(server.requests().len(), 1);
    assert!(
        journal
            .get_command(&ClientOrderId::new("coid-overseas-cancel-not-open").expect("client id"))
            .await
            .expect("lookup")
            .is_none()
    );
}
