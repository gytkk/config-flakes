#![cfg(feature = "internal-test-support")]

#[path = "support/memory_journal.rs"]
mod memory_journal;
mod support;

use memory_journal::MemoryCommandJournal;
use rust_decimal::Decimal;
use std::sync::Arc;
use support::fake_kis_server::{FakeKisServer, FakeResponse};
use support::fixtures::load_fixture;
use time::macros::datetime;
use trading_agent_kis_client::BrokerClient;
use trading_agent_kis_client::auth::{AccessToken, KisCredential};
use trading_agent_kis_client::client::KisBrokerClient;
use trading_agent_kis_client::domain::{
    AccountKey, AmendOrderRequest, BrokerEnvironment, BrokerOrderId, BrokerOrderOrgNo,
    CancelOrderRequest, ClientOrderId, CreateOrderRequest, ExchangeCode, Fill, OrderReference,
    OrderSide, OrderState, OrderStatus, OrderStatusEvidence, OverseasStock,
    OverseasStockOrderRequest, Quantity, RequestId, Symbol,
};
use trading_agent_kis_client::internal::reconcile::reconcile_command_snapshot;
use trading_agent_kis_client::journal::{
    BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus,
};
use trading_agent_kis_client::transport::{KisTransport, KisTransportConfig};

fn account_key() -> AccountKey {
    AccountKey::new("12345678-01").expect("account key")
}

fn command(client_order_id: &str) -> BrokerCommandRecord {
    command_for(BrokerEnvironment::Paper, client_order_id)
}

fn command_for(environment: BrokerEnvironment, client_order_id: &str) -> BrokerCommandRecord {
    let request = CreateOrderRequest::cash_limit(
        ClientOrderId::new(client_order_id).expect("client order id"),
        account_key(),
        Symbol::new("005930").expect("symbol"),
        OrderSide::Buy,
        Quantity::new(10),
        Decimal::new(70_000, 0),
    )
    .expect("order request");
    BrokerCommandRecord::new_place_order(environment, request).expect("command")
}

fn overseas_command(client_order_id: &str) -> BrokerCommandRecord {
    let request = OverseasStockOrderRequest::cash_limit(
        ClientOrderId::new(client_order_id).expect("client order id"),
        account_key(),
        OverseasStock::new(
            ExchangeCode::new("NASD").expect("exchange"),
            Symbol::new("AAPL").expect("symbol"),
        ),
        OrderSide::Buy,
        Quantity::new(10),
        Decimal::new(19825, 2),
    )
    .expect("overseas order request");
    BrokerCommandRecord::new_place_overseas_stock_order(BrokerEnvironment::Paper, request)
        .expect("overseas command")
}

fn reference(client_order_id: &str, broker_order_id: Option<&str>) -> OrderReference {
    OrderReference {
        client_order_id: ClientOrderId::new(client_order_id).expect("client order id"),
        broker_order_id: broker_order_id
            .map(BrokerOrderId::new)
            .transpose()
            .expect("broker order id"),
        broker_order_orgno: Some(BrokerOrderOrgNo::new("001").expect("broker orgno")),
        exchange_code: None,
        account_key: account_key(),
        environment: BrokerEnvironment::Paper,
    }
}

fn complete_reference() -> OrderReference {
    reference("coid-parent", Some("0000001234"))
}

fn status(reference: OrderReference, state: OrderState) -> OrderStatus {
    OrderStatus {
        reference,
        state,
        evidence: None,
    }
}

fn status_with_evidence(
    reference: OrderReference,
    state: OrderState,
    evidence: OrderStatusEvidence,
) -> OrderStatus {
    OrderStatus {
        reference,
        state,
        evidence: Some(evidence),
    }
}

fn matching_evidence() -> OrderStatusEvidence {
    OrderStatusEvidence {
        symbol: Symbol::new("005930").expect("symbol"),
        side: Some(OrderSide::Buy),
        ordered_quantity: Quantity::new(10),
        filled_quantity: Quantity::new(0),
        order_price: Some(Decimal::new(70_000, 0)),
        trading_day: Some("20260424".to_owned()),
        observed_at: Some(datetime!(2026-04-24 01:01:00 UTC)),
    }
}

#[test]
fn reconcile_confirms_terminal_order_by_broker_native_id() {
    let mut command = command("coid-reconcile-1");
    command.broker_order_id = Some("0000001234".to_owned());
    command.broker_order_orgno = Some("001".to_owned());
    let orders = vec![status(
        reference("kis-0000001234", Some("0000001234")),
        OrderState::Filled,
    )];

    let result = reconcile_command_snapshot(&command, &orders, &[]);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedFilled
    );
}

#[test]
fn reconcile_uses_client_order_id_when_broker_id_is_missing() {
    let command = command("coid-reconcile-2");
    let orders = vec![status(
        reference("coid-reconcile-2", None),
        OrderState::Accepted,
    )];

    let result = reconcile_command_snapshot(&command, &orders, &[]);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedSubmitted
    );
}

#[test]
fn reconcile_confirms_fill_from_fill_snapshot() {
    let mut command = command("coid-reconcile-3");
    command.broker_order_id = Some("0000005678".to_owned());
    command.broker_order_orgno = Some("001".to_owned());
    let fills = vec![Fill {
        reference: reference("kis-0000005678", Some("0000005678")),
        quantity: Quantity::new(3),
        price: Decimal::new(55_000, 0),
        evidence: None,
    }];

    let result = reconcile_command_snapshot(&command, &[], &fills);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedFilled
    );
}

#[test]
fn reconcile_prefers_matching_fill_over_open_order_snapshot() {
    let mut command = command("coid-reconcile-fill-priority");
    command.broker_order_id = Some("0000005678".to_owned());
    command.broker_order_orgno = Some("001".to_owned());
    let orders = vec![status(
        reference("kis-0000005678", Some("0000005678")),
        OrderState::Accepted,
    )];
    let fills = vec![Fill {
        reference: reference("kis-0000005678", Some("0000005678")),
        quantity: Quantity::new(3),
        price: Decimal::new(55_000, 0),
        evidence: None,
    }];

    let result = reconcile_command_snapshot(&command, &orders, &fills);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedFilled
    );
}

#[test]
fn reconcile_returns_still_unknown_when_no_unique_match_exists() {
    let command = command("coid-reconcile-4");

    let result = reconcile_command_snapshot(&command, &[], &[]);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::StillUnknown
    );
}

#[test]
fn reconcile_requires_broker_orgno_account_and_environment_for_exact_match() {
    let mut command = command("coid-reconcile-scope");
    command.broker_order_id = Some("0000001234".to_owned());
    command.broker_order_orgno = Some("001".to_owned());
    let wrong_orgno = OrderReference {
        broker_order_orgno: Some(BrokerOrderOrgNo::new("002").expect("broker orgno")),
        ..reference("kis-0000001234", Some("0000001234"))
    };
    let wrong_environment = OrderReference {
        environment: BrokerEnvironment::Live,
        ..reference("kis-0000001234-live", Some("0000001234"))
    };
    let wrong_account = OrderReference {
        account_key: AccountKey::new("87654321-01").expect("account key"),
        ..reference("kis-0000001234-other-account", Some("0000001234"))
    };
    let orders = vec![
        status(wrong_orgno, OrderState::Filled),
        status(wrong_environment, OrderState::Filled),
        status(wrong_account, OrderState::Filled),
    ];

    let result = reconcile_command_snapshot(&command, &orders, &[]);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::StillUnknown
    );
}

#[test]
fn reconcile_matches_missing_broker_native_id_by_order_evidence_window() {
    let mut command = command("coid-reconcile-evidence");
    command.trading_day = Some("20260424".to_owned());
    command.broker_observation_window_start = Some(datetime!(2026-04-24 01:00:00 UTC));
    command.broker_observation_window_end = Some(datetime!(2026-04-24 01:05:00 UTC));
    let orders = vec![status_with_evidence(
        reference("kis-0000009999", Some("0000009999")),
        OrderState::Accepted,
        matching_evidence(),
    )];

    let result = reconcile_command_snapshot(&command, &orders, &[]);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedSubmitted
    );
}

#[test]
fn reconcile_returns_manual_intervention_for_duplicate_evidence_matches() {
    let mut command = command("coid-reconcile-duplicate-evidence");
    command.trading_day = Some("20260424".to_owned());
    command.broker_observation_window_start = Some(datetime!(2026-04-24 01:00:00 UTC));
    command.broker_observation_window_end = Some(datetime!(2026-04-24 01:05:00 UTC));
    let orders = vec![
        status_with_evidence(
            reference("kis-0000009998", Some("0000009998")),
            OrderState::Accepted,
            matching_evidence(),
        ),
        status_with_evidence(
            reference("kis-0000009999", Some("0000009999")),
            OrderState::Accepted,
            matching_evidence(),
        ),
    ];

    let result = reconcile_command_snapshot(&command, &orders, &[]);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ManualInterventionRequired
    );
}

#[test]
fn reconcile_confirms_fill_from_symbol_side_quantity_day_window_evidence() {
    let mut command = command("coid-reconcile-fill-evidence");
    command.trading_day = Some("20260424".to_owned());
    command.broker_observation_window_start = Some(datetime!(2026-04-24 01:00:00 UTC));
    command.broker_observation_window_end = Some(datetime!(2026-04-24 01:05:00 UTC));
    let fills = vec![Fill {
        reference: reference("kis-0000009999", Some("0000009999")),
        quantity: Quantity::new(4),
        price: Decimal::new(70_000, 0),
        evidence: Some(matching_evidence()),
    }];

    let result = reconcile_command_snapshot(&command, &[], &fills);

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedFilled
    );
}

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

async fn journal() -> Arc<MemoryCommandJournal> {
    Arc::new(MemoryCommandJournal::new())
}

async fn mark_submitting(
    journal: &MemoryCommandJournal,
    client_order_id: &ClientOrderId,
    request_id: &str,
) {
    journal
        .mark_submitting(
            client_order_id,
            &RequestId::new(request_id).expect("request id"),
        )
        .await
        .expect("mark submitting");
}

#[tokio::test]
async fn public_reconcile_order_marks_filled_command_reconciled() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let client_order_id = ClientOrderId::new("coid-reconcile-public-1").expect("client order id");
    let mut record = command(client_order_id.as_str());
    record.broker_order_id = Some("0000005678".to_owned());
    record.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(record)
        .await
        .expect("reserve command");
    mark_submitting(journal.as_ref(), &client_order_id, "req-reconcile-public-1").await;
    journal
        .mark_reconcile_needed(&client_order_id, "socket timeout after write")
        .await
        .expect("mark reconcile needed");

    let result = BrokerClient::reconcile_order(&client, &client_order_id, 0)
        .await
        .expect("reconcile result");

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedFilled
    );

    let stored = journal
        .get_command(&client_order_id)
        .await
        .expect("lookup")
        .expect("command exists");
    assert_eq!(server.requests().len(), 2);
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Filled);
    assert!(!stored.reconcile_required);
    assert!(stored.last_reconciled_at.is_some());
    assert!(stored.last_reconcile_reason.is_none());
}

#[tokio::test]
async fn public_reconcile_order_uses_overseas_history_for_exchange_scoped_command() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/ccnl_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("overseas/ccnl_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let client_order_id = ClientOrderId::new("coid-reconcile-overseas").expect("client order id");
    let mut record = overseas_command(client_order_id.as_str());
    record.broker_order_id = Some("0000116058".to_owned());
    record.broker_order_orgno = Some("91252".to_owned());
    record.trading_day = Some("20260423".to_owned());
    journal
        .reserve_command(record)
        .await
        .expect("reserve overseas command");
    mark_submitting(journal.as_ref(), &client_order_id, "req-reconcile-overseas").await;
    journal
        .mark_reconcile_needed(&client_order_id, "socket timeout after overseas write")
        .await
        .expect("mark overseas reconcile needed");

    let result = BrokerClient::reconcile_order(&client, &client_order_id, 0)
        .await
        .expect("overseas reconcile result");

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedFilled
    );

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| {
        request
            .path
            .starts_with("/uapi/overseas-stock/v1/trading/inquire-ccnl?")
    }));
    assert!(
        requests
            .iter()
            .all(|request| request.path.contains("OVRS_EXCG_CD=NASD"))
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.path.contains("inquire-daily-ccld"))
    );
}

#[tokio::test]
async fn public_reconcile_order_marks_still_unknown_as_reconcile_failed() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let client_order_id = ClientOrderId::new("coid-reconcile-public-2").expect("client order id");
    let mut record = command(client_order_id.as_str());
    record.broker_order_id = Some("9999999999".to_owned());
    record.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(record)
        .await
        .expect("reserve command");
    mark_submitting(journal.as_ref(), &client_order_id, "req-reconcile-public-2").await;
    journal
        .mark_reconcile_needed(&client_order_id, "socket timeout after write")
        .await
        .expect("mark reconcile needed");

    let result = BrokerClient::reconcile_order(&client, &client_order_id, 0)
        .await
        .expect("reconcile result");

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::StillUnknown
    );

    let stored = journal
        .get_command(&client_order_id)
        .await
        .expect("lookup")
        .expect("command exists");
    assert_eq!(server.requests().len(), 2);
    assert_eq!(
        stored.last_known_state,
        BrokerCommandStatus::ReconcileFailed
    );
    assert!(stored.reconcile_required);
    assert!(stored.last_reconciled_at.is_some());
    assert_eq!(
        stored.last_reconcile_reason.as_deref(),
        Some("reconciliation snapshot did not find a unique matching broker state")
    );
}

#[tokio::test]
async fn reconcile_pending_processes_all_unresolved_journal_records() {
    let server = FakeKisServer::start();
    for _ in 0..4 {
        server.push_response(FakeResponse::json(
            200,
            load_fixture("orders/daily_ccld_success.json"),
        ));
    }
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let accepted_id = ClientOrderId::new("coid-pending-accepted").expect("client order id");
    let mut accepted = command(accepted_id.as_str());
    accepted.broker_order_id = Some("0000001234".to_owned());
    accepted.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(accepted)
        .await
        .expect("reserve accepted command");
    mark_submitting(journal.as_ref(), &accepted_id, "req-pending-accepted").await;
    journal
        .mark_reconcile_needed(&accepted_id, "startup recovery")
        .await
        .expect("mark accepted reconcile needed");

    let filled_id = ClientOrderId::new("coid-pending-filled").expect("client order id");
    let mut filled = command(filled_id.as_str());
    filled.broker_order_id = Some("0000005678".to_owned());
    filled.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(filled)
        .await
        .expect("reserve filled command");
    mark_submitting(journal.as_ref(), &filled_id, "req-pending-filled").await;
    journal
        .mark_reconcile_needed(&filled_id, "startup recovery")
        .await
        .expect("mark filled reconcile needed");

    let results = client
        .reconcile_pending(0)
        .await
        .expect("pending reconciliation");

    assert_eq!(server.requests().len(), 4);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].client_order_id, accepted_id);
    assert_eq!(
        results[0].result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedSubmitted
    );
    assert_eq!(results[1].client_order_id, filled_id);
    assert_eq!(
        results[1].result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedFilled
    );
    assert!(
        journal
            .list_unreconciled()
            .await
            .expect("list unreconciled")
            .is_empty()
    );
}

#[tokio::test]
async fn live_startup_recovery_halts_when_unresolved_records_remain() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let client_order_id = ClientOrderId::new("coid-live-startup-unknown").expect("client order id");
    let mut record = command_for(BrokerEnvironment::Live, client_order_id.as_str());
    record.broker_order_id = Some("9999999999".to_owned());
    record.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(record)
        .await
        .expect("reserve live command");
    mark_submitting(
        journal.as_ref(),
        &client_order_id,
        "req-live-startup-unknown",
    )
    .await;
    journal
        .mark_reconcile_needed(&client_order_id, "startup recovery")
        .await
        .expect("mark live reconcile needed");

    let error = client
        .recover_startup(0)
        .await
        .expect_err("live startup must halt with unresolved records");

    assert!(matches!(
        error,
        trading_agent_kis_client::error::BrokerError::SafetyHalt { .. }
    ));
    assert_eq!(server.requests().len(), 2);
    assert_eq!(
        journal
            .list_unreconciled()
            .await
            .expect("list unreconciled")
            .len(),
        1
    );
}

#[tokio::test]
async fn live_startup_recovery_succeeds_when_all_records_reconcile() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Live, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let client_order_id = ClientOrderId::new("coid-live-startup-filled").expect("client order id");
    let mut record = command_for(BrokerEnvironment::Live, client_order_id.as_str());
    record.broker_order_id = Some("0000005678".to_owned());
    record.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(record)
        .await
        .expect("reserve live command");
    mark_submitting(
        journal.as_ref(),
        &client_order_id,
        "req-live-startup-filled",
    )
    .await;
    journal
        .mark_reconcile_needed(&client_order_id, "startup recovery")
        .await
        .expect("mark live reconcile needed");

    let results = client
        .recover_startup(0)
        .await
        .expect("startup recovery succeeds");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].client_order_id, client_order_id);
    assert_eq!(
        results[0].result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedFilled
    );
    assert!(
        journal
            .list_unreconciled()
            .await
            .expect("list unresolved")
            .is_empty()
    );
}

#[tokio::test]
async fn reconcile_order_lookback_advances_by_business_days() {
    let server = FakeKisServer::start();
    for _ in 0..4 {
        server.push_response(FakeResponse::json(
            200,
            load_fixture("orders/daily_ccld_success.json"),
        ));
    }
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let client_order_id =
        ClientOrderId::new("coid-reconcile-business-days").expect("client order id");
    let mut command = command(client_order_id.as_str());
    command.requested_at = datetime!(2026-04-17 23:55:00 UTC);
    command.trading_day = Some("20260417".to_owned());
    command.broker_order_id = Some("0000005678".to_owned());
    command.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    mark_submitting(journal.as_ref(), &client_order_id, "req-business-days").await;
    journal
        .mark_reconcile_needed(&client_order_id, "socket timeout after write")
        .await
        .expect("mark reconcile needed");

    BrokerClient::reconcile_order(&client, &client_order_id, 1)
        .await
        .expect("reconcile result");

    let request_paths = server
        .requests()
        .into_iter()
        .map(|request| request.path)
        .collect::<Vec<_>>();
    assert_eq!(request_paths.len(), 4);
    assert!(
        request_paths
            .iter()
            .any(|path| path.contains("INQR_STRT_DT=20260417"))
    );
    assert!(
        request_paths
            .iter()
            .any(|path| path.contains("INQR_STRT_DT=20260420"))
    );
    assert!(
        !request_paths
            .iter()
            .any(|path| path.contains("INQR_STRT_DT=20260418"))
    );
    assert!(
        !request_paths
            .iter()
            .any(|path| path.contains("INQR_STRT_DT=20260419"))
    );
}

#[tokio::test]
async fn reconcile_order_uses_persisted_trading_day_for_query_window() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let client_order_id =
        ClientOrderId::new("coid-reconcile-trading-day").expect("client order id");
    let mut command = command(client_order_id.as_str());
    command.requested_at = datetime!(2026-04-17 23:55:00 UTC);
    command.trading_day = Some("20260420".to_owned());
    command.broker_order_id = Some("0000005678".to_owned());
    command.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    mark_submitting(journal.as_ref(), &client_order_id, "req-trading-day").await;
    journal
        .mark_reconcile_needed(&client_order_id, "socket timeout after write")
        .await
        .expect("mark reconcile needed");

    BrokerClient::reconcile_order(&client, &client_order_id, 0)
        .await
        .expect("reconcile result");

    let request_paths = server
        .requests()
        .into_iter()
        .map(|request| request.path)
        .collect::<Vec<_>>();
    assert_eq!(request_paths.len(), 2);
    assert!(
        request_paths
            .iter()
            .all(|path| path.contains("INQR_STRT_DT=20260420"))
    );
}

#[tokio::test]
async fn reconcile_order_rejects_future_trading_day_before_query() {
    let server = FakeKisServer::start();
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());

    let client_order_id =
        ClientOrderId::new("coid-reconcile-future-trading-day").expect("client order id");
    let mut command = command(client_order_id.as_str());
    command.trading_day = Some("20991231".to_owned());
    command.broker_order_id = Some("0000005678".to_owned());
    command.broker_order_orgno = Some("001".to_owned());
    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    mark_submitting(journal.as_ref(), &client_order_id, "req-future-trading-day").await;
    journal
        .mark_reconcile_needed(&client_order_id, "socket timeout after write")
        .await
        .expect("mark reconcile needed");

    let error = BrokerClient::reconcile_order(&client, &client_order_id, 0)
        .await
        .expect_err("future trading day must fail before broker query");

    assert!(matches!(
        error,
        trading_agent_kis_client::error::BrokerError::Validation { .. }
    ));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn public_reconcile_order_recovers_ambiguous_cancel_command() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_cancelled.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_cancelled.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());
    let parent = command("coid-parent");
    journal
        .reserve_command(parent.clone())
        .await
        .expect("reserve parent");
    let cancel_id = ClientOrderId::new("coid-cancel-recover").expect("client order id");
    let cancel = BrokerCommandRecord::new_cancel_order(
        &parent,
        &CancelOrderRequest {
            client_order_id: cancel_id.clone(),
            reference: complete_reference(),
            quantity: None,
        },
    )
    .expect("cancel command");
    journal
        .reserve_command(cancel)
        .await
        .expect("reserve cancel");
    mark_submitting(journal.as_ref(), &cancel_id, "req-cancel-recover").await;
    journal
        .mark_reconcile_needed(&cancel_id, "cancel socket timeout")
        .await
        .expect("mark cancel reconcile needed");

    let result = BrokerClient::reconcile_order(&client, &cancel_id, 0)
        .await
        .expect("cancel reconcile result");

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedCancelled
    );
    let stored = journal
        .get_command(&cancel_id)
        .await
        .expect("lookup")
        .expect("cancel command exists");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Cancelled);
    assert!(!stored.reconcile_required);
}

#[tokio::test]
async fn public_reconcile_order_recovers_ambiguous_amend_command() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    server.push_response(FakeResponse::json(
        200,
        load_fixture("orders/daily_ccld_success.json"),
    ));
    let journal = journal().await;
    let client = KisBrokerClient::new_for_account(
        transport_for(BrokerEnvironment::Paper, server.base_url()),
        account_key(),
    )
    .with_journal(journal.clone());
    let parent = command("coid-parent");
    journal
        .reserve_command(parent.clone())
        .await
        .expect("reserve parent");
    let amend_id = ClientOrderId::new("coid-amend-recover").expect("client order id");
    let amend = BrokerCommandRecord::new_amend_order(
        &parent,
        &AmendOrderRequest {
            client_order_id: amend_id.clone(),
            reference: complete_reference(),
            quantity: Some(Quantity::new(5)),
            limit_price: Some(Decimal::new(69_000, 0)),
        },
    )
    .expect("amend command");
    journal.reserve_command(amend).await.expect("reserve amend");
    mark_submitting(journal.as_ref(), &amend_id, "req-amend-recover").await;
    journal
        .mark_reconcile_needed(&amend_id, "amend socket timeout")
        .await
        .expect("mark amend reconcile needed");

    let result = BrokerClient::reconcile_order(&client, &amend_id, 0)
        .await
        .expect("amend reconcile result");

    assert_eq!(
        result,
        trading_agent_kis_client::domain::ReconciliationResult::ConfirmedAmended
    );
    let stored = journal
        .get_command(&amend_id)
        .await
        .expect("lookup")
        .expect("amend command exists");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::Accepted);
    assert!(!stored.reconcile_required);
}
