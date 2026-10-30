use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use tempfile::tempdir;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use trade_engine::config::RiskConfig;
use trade_engine::decision::DecisionOrderType;
use trade_engine::error::TradeEngineError;
use trade_engine::execution::{
    AllowAllFinalExecutionRiskGate, AmendExecutionRequest, CancelExecutionRequest,
    ExecutionRequestId, ExecutionWorkflowConfig, FinalExecutionRiskDecision,
    FinalExecutionRiskInputs, FinalFollowUpExecutionRiskGate, FollowUpExecutionOutcome,
    FollowUpExecutionRiskRequest, PolicyFinalExecutionRiskGate, submit_amend_execution_request,
    submit_cancel_execution_request, submit_next_execution_request,
    submit_next_execution_request_with_policy,
};
use trade_engine::gateway::testing::MockTradingGateway;
use trade_engine::gateway::traits::TradingGateway;
use trade_engine::gateway::types::{
    AccountSnapshot, AmendOrderRequest, AmendResult, BrokerHealth, BuyingPower, CancelOrderRequest,
    CancelResult as GatewayCancelResult, CashBalance, CurrencyCode, ExchangeCode, Fill,
    MarketScope, MarketSession, OrderStatus, Position, Quote, TradingDay,
};
use trade_engine::risk::{RiskEvaluationContext, RiskPolicy};
use trade_engine::storage::{
    MainDb, NewExecutionRequestRecord, SqliteBrokerCommandJournal, StoredExecutionRequestId,
};
use trading_agent_kis_client::journal::{
    BrokerCommandAck, BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus,
};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, BrokerError, CancelResult, ClientOrderId, CreateOrderRequest,
    OrderReference, OrderSide, OrderStatusEvidence, OrderSubmission, Quantity, RequestId, Symbol,
};

#[tokio::test]
async fn execution_request_storage_requires_approved_risk_and_links_journal_command() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let approved_ids = seed_decision_with_risk(&db_path, "approved");
    let blocked_ids = seed_decision_with_risk(&db_path, "blocked");

    let mut db = MainDb::open(&db_path).expect("reopen db");
    let blocked = execution_record("exec-blocked", blocked_ids);
    let error = db
        .record_execution_request(&blocked)
        .expect_err("blocked risk must not create an execution request");
    assert!(
        error
            .to_string()
            .contains("execution request requires an approved risk evaluation")
    );

    let approved = execution_record("exec-approved", approved_ids);
    let stored_execution = db
        .record_execution_request(&approved)
        .expect("record execution request");
    assert_eq!(
        execution_events(&db_path, stored_execution),
        vec!["execution_requested"]
    );

    let client_order_id = ClientOrderId::new("client-order-1").expect("client order id");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    journal
        .reserve_command(place_command(client_order_id.clone()))
        .await
        .expect("reserve command");

    db.link_execution_request_to_journal(stored_execution, &client_order_id)
        .expect("link execution request to journal");

    let connection = Connection::open(&db_path).expect("inspect db");
    let execution_client_order_id: String = connection
        .query_row(
            "SELECT client_order_id FROM execution_requests WHERE id = ?1",
            [stored_execution.as_i64()],
            |row| row.get(0),
        )
        .expect("execution client order id");
    assert_eq!(execution_client_order_id, client_order_id.as_str());
    let journal_execution_id: i64 = connection
        .query_row(
            "SELECT execution_request_id
             FROM broker_command_journal
             WHERE client_order_id = ?1",
            [client_order_id.as_str()],
            |row| row.get(0),
        )
        .expect("journal execution id");
    assert_eq!(journal_execution_id, stored_execution.as_i64());
    assert_eq!(
        execution_events(&db_path, stored_execution),
        vec!["execution_requested", "broker_command_reserved"]
    );
}

#[tokio::test]
async fn execution_request_storage_rejects_duplicate_request_for_same_risk_evaluation() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let approved_ids = seed_decision_with_risk(&db_path, "approved");

    let mut db = MainDb::open(&db_path).expect("reopen db");
    db.record_execution_request(&execution_record("exec-first", approved_ids))
        .expect("record first execution request");

    let error = db
        .record_execution_request(&execution_record("exec-second", approved_ids))
        .expect_err("risk evaluation must not produce duplicate execution requests");

    assert!(
        error
            .to_string()
            .contains("risk evaluation already has an execution request")
    );
}

#[tokio::test]
async fn execution_request_storage_rejects_mismatched_or_already_linked_journal_command() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let approved_ids = seed_decision_with_risk(&db_path, "approved");
    let second_approved_ids = seed_decision_with_risk(&db_path, "approved");

    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-match-check", approved_ids))
        .expect("record execution request");
    let second_execution = db
        .record_execution_request(&execution_record("exec-link-check", second_approved_ids))
        .expect("record second execution request");

    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let mismatched_client_order_id =
        ClientOrderId::new("client-order-mismatch").expect("client order id");
    journal
        .reserve_command(place_command_for(
            mismatched_client_order_id.clone(),
            "000660",
            OrderSide::Buy,
            1,
            Decimal::new(73_000, 0),
        ))
        .await
        .expect("reserve mismatched command");

    let error = db
        .link_execution_request_to_journal(execution, &mismatched_client_order_id)
        .expect_err("mismatched journal command must not link");
    assert!(
        error
            .to_string()
            .contains("broker command journal record does not match execution request")
    );
    assert_eq!(execution_client_order_id(&db_path, execution), None);
    assert_eq!(
        journal_execution_request_id(&db_path, &mismatched_client_order_id),
        None
    );

    let linked_client_order_id =
        ClientOrderId::new("client-order-linked").expect("client order id");
    journal
        .reserve_command(place_command(linked_client_order_id.clone()))
        .await
        .expect("reserve matching command");
    db.link_execution_request_to_journal(execution, &linked_client_order_id)
        .expect("link matching command");

    let error = db
        .link_execution_request_to_journal(second_execution, &linked_client_order_id)
        .expect_err("linked journal command must not be overwritten");
    assert!(
        error
            .to_string()
            .contains("broker command journal record is already linked")
    );
    assert_eq!(
        journal_execution_request_id(&db_path, &linked_client_order_id),
        Some(execution.as_i64())
    );
}

#[test]
fn execution_risk_state_helpers_read_duplicate_and_daily_count_from_db() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let first = seed_decision_with_risk(&db_path, "approved");
    let second = seed_decision_with_risk(&db_path, "approved");
    insert_execution_request_row(
        &db_path,
        ExecutionRequestSeed {
            ids: first,
            execution_request_id: "exec-risk-state-1",
            status: "requested",
            symbol: "005930",
            side: "buy",
            quantity: 1,
            order_type: "limit",
            limit_price: Some("73000"),
            created_at: "2026-04-26T00:00:10Z",
        },
    );
    insert_execution_request_row(
        &db_path,
        ExecutionRequestSeed {
            ids: second,
            execution_request_id: "exec-risk-state-2",
            status: "rejected",
            symbol: "000660",
            side: "buy",
            quantity: 1,
            order_type: "limit",
            limit_price: Some("180000"),
            created_at: "2026-04-26T00:01:10Z",
        },
    );

    let db = MainDb::open(&db_path).expect("reopen db");
    assert!(
        db.has_active_execution_intent(
            &Symbol::new("005930").expect("symbol"),
            OrderSide::Buy,
            DecisionOrderType::Limit,
            Some(Decimal::new(73_000, 0)),
        )
        .expect("duplicate intent lookup")
    );
    assert!(
        !db.has_active_execution_intent(
            &Symbol::new("005930").expect("symbol"),
            OrderSide::Buy,
            DecisionOrderType::Limit,
            Some(Decimal::new(74_000, 0)),
        )
        .expect("different price intent lookup")
    );
    assert_eq!(
        db.execution_request_count_since(parse_time("2026-04-26T00:00:00Z"))
            .expect("daily count"),
        2
    );
}

#[tokio::test]
async fn execution_risk_state_treats_submitting_journal_command_as_unresolved() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let client_order_id =
        ClientOrderId::new("exec-risk-submitting-unresolved").expect("client order id");

    journal
        .reserve_command(place_command(client_order_id.clone()))
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("req-risk-submitting-unresolved").expect("request id"),
        )
        .await
        .expect("mark submitting");

    let db = MainDb::open(&db_path).expect("reopen db");
    assert!(
        db.has_unresolved_broker_commands()
            .expect("unresolved journal lookup")
    );
}

#[test]
fn execution_result_storage_records_accepted_rejected_and_ambiguous_outcomes() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let accepted_ids = seed_decision_with_risk(&db_path, "approved");
    let rejected_ids = seed_decision_with_risk(&db_path, "approved");
    let ambiguous_ids = seed_decision_with_risk(&db_path, "approved");

    let mut db = MainDb::open(&db_path).expect("reopen db");
    let accepted = db
        .record_execution_request(&execution_record("exec-accepted", accepted_ids))
        .expect("record accepted execution");
    let rejected = db
        .record_execution_request(&execution_record("exec-rejected", rejected_ids))
        .expect("record rejected execution");
    let ambiguous = db
        .record_execution_request(&execution_record("exec-ambiguous", ambiguous_ids))
        .expect("record ambiguous execution");

    db.record_execution_order_accepted(
        accepted,
        &OrderSubmission {
            reference: order_reference("exec-accepted")
                .with_broker_order_id("broker-order-1")
                .expect("broker order id")
                .with_broker_order_orgno("001")
                .expect("broker order orgno"),
            accepted: true,
        },
    )
    .expect("record accepted order");
    db.record_execution_order_rejected(rejected, "broker rejected order")
        .expect("record rejected order");
    db.record_execution_reconciliation_needed(ambiguous, "submit", "broker response was ambiguous")
        .expect("record ambiguous order");

    assert_eq!(execution_status(&db_path, accepted), "accepted");
    assert_eq!(
        execution_broker_reference(&db_path, accepted),
        (Some("broker-order-1".to_owned()), Some("001".to_owned()))
    );
    assert_eq!(execution_status(&db_path, rejected), "rejected");
    assert_eq!(
        execution_rejection_reason(&db_path, rejected),
        Some("broker rejected order".to_owned())
    );
    assert_eq!(
        execution_status(&db_path, ambiguous),
        "reconciliation_needed"
    );
    assert_eq!(
        execution_events(&db_path, accepted),
        vec!["execution_requested", "order_accepted"]
    );
    assert_eq!(
        execution_events(&db_path, rejected),
        vec!["execution_requested", "order_rejected"]
    );
    assert_eq!(
        execution_events(&db_path, ambiguous),
        vec!["execution_requested", "reconciliation_needed"]
    );
}

#[tokio::test]
async fn execution_workflow_reserves_journal_links_submits_and_records_accepted_order() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-submit-accepted", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new().with_order_submission(OrderSubmission {
        reference: order_reference("exec-submit-accepted")
            .with_broker_order_id("broker-order-accepted")
            .expect("broker order id")
            .with_broker_order_orgno("001")
            .expect("broker order orgno"),
        accepted: true,
    });
    let journal_gateway = JournalAwareGateway::new(gateway.clone(), &journal);

    let submitted = submit_next_execution_request(
        &journal_gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("submit execution");

    assert_eq!(submitted, Some(execution));
    assert_eq!(gateway.call_counts().place_order, 1);
    assert_eq!(execution_status(&db_path, execution), "accepted");
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-submit-accepted").expect("client order id"),
        ),
        Some(BrokerCommandStatus::Accepted)
    );
    assert_eq!(
        journal_execution_request_id(
            &db_path,
            &ClientOrderId::new("exec-submit-accepted").expect("client order id"),
        ),
        Some(execution.as_i64())
    );
}

#[tokio::test]
async fn execution_workflow_reserves_overseas_submit_command_with_exchange_code() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-submit-overseas", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new().with_order_submission(OrderSubmission {
        reference: order_reference("exec-submit-overseas")
            .with_broker_order_id("broker-order-overseas")
            .expect("broker order id")
            .with_broker_order_orgno("001")
            .expect("broker order orgno"),
        accepted: true,
    });
    let journal_gateway = JournalAwareGateway::new(gateway.clone(), &journal);

    let submitted = submit_next_execution_request(
        &journal_gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &ExecutionWorkflowConfig {
            market_scope: MarketScope::OverseasStock {
                exchange: ExchangeCode::new("NASD").expect("exchange"),
                currency: CurrencyCode::new("USD").expect("currency"),
            },
            ..execution_workflow_config(false)
        },
    )
    .await
    .expect("submit execution");

    assert_eq!(submitted, Some(execution));
    assert_eq!(
        journal_exchange_code(
            &db_path,
            &ClientOrderId::new("exec-submit-overseas").expect("client order id"),
        ),
        Some("NASD".to_owned())
    );
    assert_eq!(execution_status(&db_path, execution), "accepted");
}

#[tokio::test]
async fn execution_workflow_marks_reconciliation_when_gateway_accepts_without_journal_ack() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-submit-missing-ack", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new().with_order_submission(OrderSubmission {
        reference: order_reference("exec-submit-missing-ack")
            .with_broker_order_id("broker-order-missing-ack")
            .expect("broker order id")
            .with_broker_order_orgno("001")
            .expect("broker order orgno"),
        accepted: true,
    });

    let submitted = submit_next_execution_request(
        &gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("submit execution");

    assert_eq!(submitted, Some(execution));
    assert_eq!(gateway.call_counts().place_order, 1);
    assert_eq!(
        execution_status(&db_path, execution),
        "reconciliation_needed"
    );
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "reconciliation_needed"
        ]
    );
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-submit-missing-ack").expect("client order id"),
        ),
        Some(BrokerCommandStatus::SubmittedAckUnknown)
    );
}

#[tokio::test]
async fn execution_workflow_policy_submit_allows_first_request_with_default_policy() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-policy-submit", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_order_submission(OrderSubmission {
            reference: order_reference("exec-policy-submit")
                .with_broker_order_id("broker-order-policy-submit")
                .expect("broker order id")
                .with_broker_order_orgno("001")
                .expect("broker order orgno"),
            accepted: true,
        });
    let journal_gateway = JournalAwareGateway::new(gateway.clone(), &journal);
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let allowed_symbols = [Symbol::new("005930").expect("symbol")];

    let submitted = submit_next_execution_request_with_policy(
        &journal_gateway,
        &mut db,
        &journal,
        &execution_workflow_config(false),
        final_risk_inputs(&risk, &allowed_symbols, &account_key),
    )
    .await
    .expect("policy submit execution");

    assert_eq!(submitted, Some(execution));
    assert_eq!(gateway.call_counts().place_order, 1);
    assert_eq!(execution_status(&db_path, execution), "accepted");
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-policy-submit").expect("client order id"),
        ),
        Some(BrokerCommandStatus::Accepted)
    );
}

#[tokio::test]
async fn execution_workflow_policy_submit_blocks_unresolved_journal_before_submit() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-policy-unresolved", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    seed_unresolved_journal_command(&journal).await;
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_order_submission(OrderSubmission {
            reference: order_reference("exec-policy-unresolved"),
            accepted: true,
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let allowed_symbols = [Symbol::new("005930").expect("symbol")];

    let submitted = submit_next_execution_request_with_policy(
        &gateway,
        &mut db,
        &journal,
        &execution_workflow_config(false),
        final_risk_inputs(&risk, &allowed_symbols, &account_key),
    )
    .await
    .expect("policy submit execution");

    let calls = gateway.call_counts();
    assert_eq!(submitted, Some(execution));
    assert_eq!(calls.buying_power, 0);
    assert_eq!(calls.account_snapshot, 0);
    assert_eq!(calls.market_session, 0);
    assert_eq!(calls.fills, 0);
    assert_eq!(calls.place_order, 0);
    assert_eq!(execution_status(&db_path, execution), "failed");
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-policy-unresolved").expect("client order id"),
        ),
        Some(BrokerCommandStatus::SafetyHalted)
    );
}

#[tokio::test]
async fn execution_workflow_policy_submit_blocks_panic_stop_before_gateway_reads() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    db.set_panic_stop(true, Some("operator-1"), "risk drill")
        .expect("activate panic stop");
    let execution = db
        .record_execution_request(&execution_record("exec-policy-panic-stop", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_order_submission(OrderSubmission {
            reference: order_reference("exec-policy-panic-stop"),
            accepted: true,
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let allowed_symbols = [Symbol::new("005930").expect("symbol")];

    let submitted = submit_next_execution_request_with_policy(
        &gateway,
        &mut db,
        &journal,
        &execution_workflow_config(false),
        final_risk_inputs(&risk, &allowed_symbols, &account_key),
    )
    .await
    .expect("policy submit execution");

    let calls = gateway.call_counts();
    assert_eq!(submitted, Some(execution));
    assert_eq!(calls.buying_power, 0);
    assert_eq!(calls.account_snapshot, 0);
    assert_eq!(calls.market_session, 0);
    assert_eq!(calls.fills, 0);
    assert_eq!(calls.place_order, 0);
    assert_eq!(execution_status(&db_path, execution), "failed");
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-policy-panic-stop").expect("client order id"),
        ),
        Some(BrokerCommandStatus::SafetyHalted)
    );
}

#[tokio::test]
async fn execution_workflow_policy_submit_blocks_daily_loss_before_submit() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-policy-daily-loss", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let symbol = Symbol::new("005930").expect("symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: symbol.clone(),
            quantity: Quantity::new(10),
            average_price: Decimal::new(80_000, 0),
        })
        .with_fill(Fill {
            reference: order_reference("loss-fill"),
            quantity: Quantity::new(2),
            price: Decimal::new(75_000, 0),
            evidence: Some(OrderStatusEvidence {
                symbol: symbol.clone(),
                side: Some(OrderSide::Sell),
                ordered_quantity: Quantity::new(2),
                filled_quantity: Quantity::new(2),
                order_price: Some(Decimal::new(75_000, 0)),
                trading_day: Some("20260426".to_owned()),
                observed_at: None,
            }),
        })
        .with_order_submission(OrderSubmission {
            reference: order_reference("exec-policy-daily-loss"),
            accepted: true,
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        max_daily_loss: Decimal::new(10_000, 0),
        ..RiskConfig::default()
    };
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let allowed_symbols = [symbol];

    let submitted = submit_next_execution_request_with_policy(
        &gateway,
        &mut db,
        &journal,
        &execution_workflow_config(false),
        final_risk_inputs(&risk, &allowed_symbols, &account_key),
    )
    .await
    .expect("policy submit execution");

    let calls = gateway.call_counts();
    assert_eq!(submitted, Some(execution));
    assert_eq!(calls.fills, 1);
    assert_eq!(calls.place_order, 0);
    assert_eq!(execution_status(&db_path, execution), "failed");
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-policy-daily-loss").expect("client order id"),
        ),
        Some(BrokerCommandStatus::SafetyHalted)
    );
}

#[tokio::test]
async fn execution_workflow_policy_submit_uses_stored_cost_basis_for_sold_out_fills() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk_for_order(&db_path, "approved", "sell", 1, "73000", "73000");
    seed_portfolio_snapshot_position(
        &db_path,
        ids.run_id,
        ids.tick_id,
        "2026-06-22T01:01:56Z",
        "111111",
        8,
        "80000",
    );
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record_for(
            "exec-policy-sold-out-cost-basis",
            ids,
            OrderSide::Sell,
            Quantity::new(1),
            Decimal::new(73_000, 0),
        ))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let request_symbol = Symbol::new("005930").expect("symbol");
    let sold_out_symbol = Symbol::new("111111").expect("symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: request_symbol.clone(),
            quantity: Quantity::new(10),
            average_price: Decimal::new(70_000, 0),
        })
        .with_fill(Fill {
            reference: order_reference("sold-out-fill"),
            quantity: Quantity::new(2),
            price: Decimal::new(75_000, 0),
            evidence: Some(OrderStatusEvidence {
                symbol: sold_out_symbol,
                side: Some(OrderSide::Sell),
                ordered_quantity: Quantity::new(2),
                filled_quantity: Quantity::new(2),
                order_price: Some(Decimal::new(75_000, 0)),
                trading_day: Some("20260622".to_owned()),
                observed_at: None,
            }),
        })
        .with_order_submission(OrderSubmission {
            reference: order_reference("exec-policy-sold-out-cost-basis")
                .with_broker_order_id("broker-order-sold-out-cost-basis")
                .expect("broker order id")
                .with_broker_order_orgno("001")
                .expect("broker order orgno"),
            accepted: true,
        });
    let journal_gateway = JournalAwareGateway::new(gateway.clone(), &journal);
    let risk = RiskConfig {
        enable_live_trading: true,
        max_daily_loss: Decimal::new(20_000, 0),
        ..RiskConfig::default()
    };
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let allowed_symbols = [request_symbol];

    let submitted = submit_next_execution_request_with_policy(
        &journal_gateway,
        &mut db,
        &journal,
        &execution_workflow_config(false),
        final_risk_inputs(&risk, &allowed_symbols, &account_key),
    )
    .await
    .expect("policy submit execution");

    let calls = gateway.call_counts();
    assert_eq!(submitted, Some(execution));
    assert_eq!(calls.fills, 1);
    assert_eq!(calls.place_order, 1);
    assert_eq!(execution_status(&db_path, execution), "accepted");
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-policy-sold-out-cost-basis").expect("client order id"),
        ),
        Some(BrokerCommandStatus::Accepted)
    );
}

#[tokio::test]
async fn execution_workflow_policy_submit_blocks_sell_above_current_position_before_submit() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids =
        seed_decision_with_risk_for_order(&db_path, "approved", "sell", 10, "73000", "730000");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record_for(
            "exec-policy-sell-over-position",
            ids,
            OrderSide::Sell,
            Quantity::new(10),
            Decimal::new(73_000, 0),
        ))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let symbol = Symbol::new("005930").expect("symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: symbol.clone(),
            quantity: Quantity::new(5),
            average_price: Decimal::new(70_000, 0),
        })
        .with_order_submission(OrderSubmission {
            reference: order_reference("exec-policy-sell-over-position"),
            accepted: true,
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let allowed_symbols = [symbol];

    let submitted = submit_next_execution_request_with_policy(
        &gateway,
        &mut db,
        &journal,
        &execution_workflow_config(false),
        final_risk_inputs(&risk, &allowed_symbols, &account_key),
    )
    .await
    .expect("policy submit execution");

    let calls = gateway.call_counts();
    assert_eq!(submitted, Some(execution));
    assert_eq!(calls.buying_power, 0);
    assert_eq!(calls.account_snapshot, 1);
    assert_eq!(calls.market_session, 1);
    assert_eq!(calls.fills, 1);
    assert_eq!(calls.place_order, 0);
    assert_eq!(execution_status(&db_path, execution), "failed");
    assert!(
        execution_last_error(&db_path, execution)
            .expect("last error")
            .contains("sell_quantity_exceeds_position")
    );
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-policy-sell-over-position").expect("client order id"),
        ),
        Some(BrokerCommandStatus::SafetyHalted)
    );
}

#[tokio::test]
async fn execution_workflow_policy_submit_read_only_fails_before_gateway_reads() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-policy-read-only", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new();
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };
    let account_key = AccountKey::new("12345678-01").expect("account key");
    let allowed_symbols = [Symbol::new("005930").expect("symbol")];

    let submitted = submit_next_execution_request_with_policy(
        &gateway,
        &mut db,
        &journal,
        &execution_workflow_config(true),
        final_risk_inputs(&risk, &allowed_symbols, &account_key),
    )
    .await
    .expect("policy submit read-only execution");

    let calls = gateway.call_counts();
    assert_eq!(submitted, Some(execution));
    assert_eq!(calls.buying_power, 0);
    assert_eq!(calls.account_snapshot, 0);
    assert_eq!(calls.market_session, 0);
    assert_eq!(calls.place_order, 0);
    assert_eq!(execution_status(&db_path, execution), "failed");
}

#[tokio::test]
async fn execution_workflow_read_only_fails_before_journal_or_submit() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-read-only", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new();

    let submitted = submit_next_execution_request(
        &gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(true),
    )
    .await
    .expect("read-only execution check");

    assert_eq!(submitted, Some(execution));
    assert_eq!(gateway.call_counts().place_order, 0);
    assert_eq!(execution_status(&db_path, execution), "failed");
    assert_eq!(
        execution_events(&db_path, execution),
        vec!["execution_requested", "execution_failed"]
    );
    assert_eq!(
        journal_execution_request_id(
            &db_path,
            &ClientOrderId::new("exec-read-only").expect("client order id"),
        ),
        None
    );
}

#[tokio::test]
async fn execution_workflow_final_risk_halts_reserved_command_before_submit() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-final-risk-blocked", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new();
    let final_risk = PolicyFinalExecutionRiskGate::new(
        RiskPolicy::new(
            false,
            false,
            [Symbol::new("005930").expect("symbol")],
            Decimal::new(1_000_000, 0),
        ),
        RiskEvaluationContext::default(),
    );

    let submitted = submit_next_execution_request(
        &gateway,
        &mut db,
        &journal,
        &final_risk,
        &execution_workflow_config(false),
    )
    .await
    .expect("final risk block");

    assert_eq!(submitted, Some(execution));
    assert_eq!(gateway.call_counts().place_order, 0);
    assert_eq!(execution_status(&db_path, execution), "failed");
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "execution_failed"
        ]
    );
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-final-risk-blocked").expect("client order id"),
        ),
        Some(BrokerCommandStatus::SafetyHalted)
    );
}

#[tokio::test]
async fn execution_workflow_records_rejected_submission_without_retry() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-submit-rejected", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new().with_order_submission(OrderSubmission {
        reference: order_reference("exec-submit-rejected"),
        accepted: false,
    });
    let journal_gateway = JournalAwareGateway::new(gateway.clone(), &journal);

    let submitted = submit_next_execution_request(
        &journal_gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("submit execution");
    let second = submit_next_execution_request(
        &gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("no retry after rejected submission");

    assert_eq!(submitted, Some(execution));
    assert_eq!(second, None);
    assert_eq!(gateway.call_counts().place_order, 1);
    assert_eq!(execution_status(&db_path, execution), "rejected");
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-submit-rejected").expect("client order id"),
        ),
        Some(BrokerCommandStatus::Rejected)
    );
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "order_rejected"
        ]
    );
}

#[tokio::test]
async fn execution_workflow_marks_ambiguous_submit_for_reconciliation_and_does_not_retry() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-ambiguous-submit", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new().with_order_error(BrokerError::Temporary {
        message: "timeout after submit".to_owned(),
    });

    let submitted = submit_next_execution_request(
        &gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("ambiguous submit is recorded");
    let second = submit_next_execution_request(
        &gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("no blind retry");

    assert_eq!(submitted, Some(execution));
    assert_eq!(second, None);
    assert_eq!(gateway.call_counts().place_order, 1);
    assert_eq!(
        execution_status(&db_path, execution),
        "reconciliation_needed"
    );
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-ambiguous-submit").expect("client order id"),
        ),
        Some(BrokerCommandStatus::SubmittedAckUnknown)
    );
}

#[tokio::test]
async fn execution_workflow_marks_pre_submit_safety_halt_failed_without_reconciliation() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let ids = seed_decision_with_risk(&db_path, "approved");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let execution = db
        .record_execution_request(&execution_record("exec-pre-submit-safety-halt", ids))
        .expect("record execution request");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = MockTradingGateway::new().with_order_error(BrokerError::safety_halt(
        "confirmed account suffix does not match configured account",
    ));

    let submitted = submit_next_execution_request(
        &gateway,
        &mut db,
        &journal,
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("pre-submit safety halt is terminal");

    assert_eq!(submitted, Some(execution));
    assert_eq!(gateway.call_counts().place_order, 1);
    assert_eq!(execution_status(&db_path, execution), "failed");
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "execution_failed"
        ]
    );
    assert_eq!(
        journal_status(
            &db_path,
            &ClientOrderId::new("exec-pre-submit-safety-halt").expect("client order id"),
        ),
        Some(BrokerCommandStatus::SafetyHalted)
    );
}

#[tokio::test]
async fn cancel_execution_workflow_reserves_child_command_and_records_cancelled_order() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let execution = seed_accepted_execution_for_follow_up(&db_path).await;
    let cancel_client_order_id = ClientOrderId::new("exec-cancel-accepted").expect("client id");
    let gateway = MockTradingGateway::new().with_cancel_result(CancelResult {
        reference: accepted_follow_up_reference("exec-follow-up-parent"),
        accepted: true,
    });
    let journal_gateway = JournalAwareGateway::new(gateway.clone(), &journal);

    submit_cancel_execution_request(
        &journal_gateway,
        &mut db,
        &journal,
        CancelExecutionRequest {
            parent_execution_request_id: execution,
            client_order_id: cancel_client_order_id.clone(),
            quantity: Some(Quantity::new(1)),
        },
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("submit cancel");

    assert_eq!(gateway.call_counts().cancel_order, 1);
    assert_eq!(execution_status(&db_path, execution), "cancelled");
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "order_accepted",
            "broker_command_reserved",
            "order_cancelled"
        ]
    );
    assert_eq!(
        journal_execution_request_id(&db_path, &cancel_client_order_id),
        Some(execution.as_i64())
    );
    assert_eq!(
        journal_status(&db_path, &cancel_client_order_id),
        Some(BrokerCommandStatus::Cancelled)
    );
}

#[tokio::test]
async fn cancel_execution_workflow_reconciles_accepted_gateway_result_without_child_terminal() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let execution = seed_accepted_execution_for_follow_up(&db_path).await;
    let cancel_client_order_id =
        ClientOrderId::new("exec-cancel-missing-terminal").expect("cancel client order id");
    let gateway = MockTradingGateway::new().with_cancel_result(CancelResult {
        reference: accepted_follow_up_reference("exec-follow-up-parent"),
        accepted: true,
    });

    let outcome = submit_cancel_execution_request(
        &gateway,
        &mut db,
        &journal,
        CancelExecutionRequest {
            parent_execution_request_id: execution,
            client_order_id: cancel_client_order_id.clone(),
            quantity: Some(Quantity::new(1)),
        },
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("submit cancel");

    assert_eq!(outcome, FollowUpExecutionOutcome::ReconciliationNeeded);
    assert_eq!(gateway.call_counts().cancel_order, 1);
    assert_eq!(
        execution_status(&db_path, execution),
        "reconciliation_needed"
    );
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "order_accepted",
            "broker_command_reserved",
            "reconciliation_needed"
        ]
    );
    assert_eq!(
        journal_status(&db_path, &cancel_client_order_id),
        Some(BrokerCommandStatus::SubmittedAckUnknown)
    );
}

#[tokio::test]
async fn cancel_execution_workflow_final_risk_halts_child_command_before_submit() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let execution = seed_accepted_execution_for_follow_up(&db_path).await;
    let cancel_client_order_id = ClientOrderId::new("exec-cancel-risk-blocked").expect("client id");
    let gateway = MockTradingGateway::new();

    let outcome = submit_cancel_execution_request(
        &gateway,
        &mut db,
        &journal,
        CancelExecutionRequest {
            parent_execution_request_id: execution,
            client_order_id: cancel_client_order_id.clone(),
            quantity: Some(Quantity::new(1)),
        },
        &BlockAllFollowUpRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("blocked cancel");

    assert_eq!(outcome, FollowUpExecutionOutcome::Failed);
    assert_eq!(gateway.call_counts().cancel_order, 0);
    assert_eq!(execution_status(&db_path, execution), "accepted");
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "order_accepted",
            "broker_command_reserved",
            "execution_failed"
        ]
    );
    assert_eq!(
        journal_status(&db_path, &cancel_client_order_id),
        Some(BrokerCommandStatus::SafetyHalted)
    );
}

#[tokio::test]
async fn cancel_execution_workflow_marks_pre_submit_gateway_error_failed() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let execution = seed_accepted_execution_for_follow_up(&db_path).await;
    let cancel_client_order_id =
        ClientOrderId::new("exec-cancel-preflight-failed").expect("client id");
    let gateway = MockTradingGateway::new().with_cancel_error(BrokerError::validation(
        "quantity exceeds possible cancel quantity",
    ));

    let outcome = submit_cancel_execution_request(
        &gateway,
        &mut db,
        &journal,
        CancelExecutionRequest {
            parent_execution_request_id: execution,
            client_order_id: cancel_client_order_id.clone(),
            quantity: Some(Quantity::new(999)),
        },
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("pre-submit cancel failure");

    assert_eq!(outcome, FollowUpExecutionOutcome::Failed);
    assert_eq!(gateway.call_counts().cancel_order, 1);
    assert_eq!(execution_status(&db_path, execution), "accepted");
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "order_accepted",
            "broker_command_reserved",
            "execution_failed"
        ]
    );
    assert_eq!(
        journal_status(&db_path, &cancel_client_order_id),
        Some(BrokerCommandStatus::SafetyHalted)
    );
}

#[tokio::test]
async fn amend_execution_workflow_records_ambiguous_follow_up_for_reconciliation() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let execution = seed_accepted_execution_for_follow_up(&db_path).await;
    let amend_client_order_id = ClientOrderId::new("exec-amend-ambiguous").expect("client id");
    let gateway = MockTradingGateway::new().with_amend_error(BrokerError::ReconciliationNeeded {
        client_order_id: amend_client_order_id.clone(),
    });

    submit_amend_execution_request(
        &gateway,
        &mut db,
        &journal,
        AmendExecutionRequest {
            parent_execution_request_id: execution,
            client_order_id: amend_client_order_id.clone(),
            quantity: Some(Quantity::new(1)),
            limit_price: Some(Decimal::new(72_000, 0)),
        },
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("submit amend");

    assert_eq!(gateway.call_counts().amend_order, 1);
    assert_eq!(
        execution_status(&db_path, execution),
        "reconciliation_needed"
    );
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "order_accepted",
            "broker_command_reserved",
            "reconciliation_needed"
        ]
    );
    assert_eq!(
        journal_execution_request_id(&db_path, &amend_client_order_id),
        Some(execution.as_i64())
    );
    assert_eq!(
        journal_status(&db_path, &amend_client_order_id),
        Some(BrokerCommandStatus::SubmittedAckUnknown)
    );
}

#[tokio::test]
async fn amend_execution_workflow_reconciles_accepted_gateway_result_without_child_terminal() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let mut db = MainDb::open(&db_path).expect("reopen db");
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let execution = seed_accepted_execution_for_follow_up(&db_path).await;
    let amend_client_order_id =
        ClientOrderId::new("exec-amend-missing-terminal").expect("amend client id");
    let gateway = MockTradingGateway::new().with_amend_result(AmendResult {
        reference: accepted_follow_up_reference("exec-follow-up-parent"),
        accepted: true,
    });

    let outcome = submit_amend_execution_request(
        &gateway,
        &mut db,
        &journal,
        AmendExecutionRequest {
            parent_execution_request_id: execution,
            client_order_id: amend_client_order_id.clone(),
            quantity: Some(Quantity::new(1)),
            limit_price: Some(Decimal::new(72_000, 0)),
        },
        &AllowAllFinalExecutionRiskGate,
        &execution_workflow_config(false),
    )
    .await
    .expect("submit amend");

    assert_eq!(outcome, FollowUpExecutionOutcome::ReconciliationNeeded);
    assert_eq!(gateway.call_counts().amend_order, 1);
    assert_eq!(
        execution_status(&db_path, execution),
        "reconciliation_needed"
    );
    assert_eq!(
        execution_events(&db_path, execution),
        vec![
            "execution_requested",
            "broker_command_reserved",
            "order_accepted",
            "broker_command_reserved",
            "reconciliation_needed"
        ]
    );
    assert_eq!(
        journal_status(&db_path, &amend_client_order_id),
        Some(BrokerCommandStatus::SubmittedAckUnknown)
    );
}

#[derive(Debug, Clone, Copy)]
struct BlockAllFollowUpRiskGate;

impl FinalFollowUpExecutionRiskGate for BlockAllFollowUpRiskGate {
    fn evaluate(
        &self,
        request: &FollowUpExecutionRiskRequest,
    ) -> Result<FinalExecutionRiskDecision, TradeEngineError> {
        Ok(FinalExecutionRiskDecision::Blocked {
            reason: format!(
                "test blocked {} follow-up {}",
                request.command_type.as_str(),
                request.client_order_id.as_str()
            ),
        })
    }
}

struct JournalAwareGateway<'a> {
    inner: MockTradingGateway,
    journal: &'a SqliteBrokerCommandJournal,
}

impl<'a> JournalAwareGateway<'a> {
    fn new(inner: MockTradingGateway, journal: &'a SqliteBrokerCommandJournal) -> Self {
        Self { inner, journal }
    }
}

#[async_trait]
impl TradingGateway for JournalAwareGateway<'_> {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        self.inner.ping().await
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        self.inner.get_account_snapshot().await
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        self.inner.get_cash_balance().await
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        self.inner.get_positions().await
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        self.inner.get_quote(symbol).await
    }

    async fn get_market_session(&self, trading_day: &str) -> Result<MarketSession, BrokerError> {
        self.inner.get_market_session(trading_day).await
    }

    async fn get_buying_power(&self, symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        self.inner.get_buying_power(symbol).await
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        self.inner.list_open_orders().await
    }

    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        self.inner.list_fills(day).await
    }

    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError> {
        let client_order_id = order.client_order_id.clone();
        let request_id = RequestId::new(format!("journal-aware-{client_order_id}"))?;
        self.journal
            .mark_submitting(&client_order_id, &request_id)
            .await?;
        let submission = self.inner.place_order(order).await?;
        if submission.accepted {
            self.journal
                .mark_ack(
                    &client_order_id,
                    BrokerCommandAck {
                        broker_order_id: submission
                            .reference
                            .broker_order_id
                            .as_ref()
                            .map(ToString::to_string),
                        broker_order_orgno: submission
                            .reference
                            .broker_order_orgno
                            .as_ref()
                            .map(ToString::to_string),
                        raw_ack_code: Some("test-accepted".to_owned()),
                    },
                )
                .await?;
        } else {
            self.journal
                .mark_terminal(&client_order_id, BrokerCommandStatus::Rejected)
                .await?;
        }
        Ok(submission)
    }

    async fn cancel_order(
        &self,
        request: CancelOrderRequest,
    ) -> Result<GatewayCancelResult, BrokerError> {
        let client_order_id = request.client_order_id.clone();
        let request_id = RequestId::new(format!("journal-aware-{client_order_id}"))?;
        self.journal
            .mark_submitting(&client_order_id, &request_id)
            .await?;
        let result = self.inner.cancel_order(request).await?;
        if result.accepted {
            self.journal
                .mark_terminal(&client_order_id, BrokerCommandStatus::Cancelled)
                .await?;
        } else {
            self.journal
                .mark_terminal(&client_order_id, BrokerCommandStatus::Rejected)
                .await?;
        }
        Ok(result)
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        let client_order_id = request.client_order_id.clone();
        let request_id = RequestId::new(format!("journal-aware-{client_order_id}"))?;
        self.journal
            .mark_submitting(&client_order_id, &request_id)
            .await?;
        let result = self.inner.amend_order(request).await?;
        if result.accepted {
            self.journal
                .mark_terminal(&client_order_id, BrokerCommandStatus::Accepted)
                .await?;
        } else {
            self.journal
                .mark_terminal(&client_order_id, BrokerCommandStatus::Rejected)
                .await?;
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy)]
struct SeededDecisionIds {
    run_id: i64,
    tick_id: i64,
    decision_proposal_id: i64,
    risk_evaluation_id: i64,
}

fn seed_decision_with_risk(path: &std::path::Path, risk_status: &str) -> SeededDecisionIds {
    seed_decision_with_risk_for_order(path, risk_status, "buy", 1, "73000", "73000")
}

fn seed_decision_with_risk_for_order(
    path: &std::path::Path,
    risk_status: &str,
    decision_type: &str,
    quantity: i64,
    limit_price: &str,
    order_notional: &str,
) -> SeededDecisionIds {
    let connection = Connection::open(path).expect("open seed db");
    connection
        .execute(
            "INSERT INTO runtime_runs (mode, environment, account_suffix, started_at, completed_at, status, error)
             VALUES ('dry_run', 'paper', '1234', '2026-04-26T00:00:00Z', NULL, 'started', NULL)",
            [],
        )
        .expect("insert run");
    let run_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO runtime_ticks (run_id, mode, started_at, completed_at, status, error)
             VALUES (?1, 'dry_run', '2026-04-26T00:00:00Z', NULL, 'started', NULL)",
            [run_id],
        )
        .expect("insert tick");
    let tick_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO decision_outputs (run_id, tick_id, observed_at, provider, raw_output_json, status, error)
             VALUES (?1, ?2, '2026-04-26T00:00:01Z', 'test-provider', '{}', 'parsed', NULL)",
            params![run_id, tick_id],
        )
        .expect("insert decision output");
    let decision_output_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO decision_proposals (
                run_id,
                tick_id,
                decision_output_id,
                observed_at,
                decision_type,
                symbol,
                quantity,
                order_type,
                limit_price,
                confidence,
                time_horizon,
                reason,
                risk_notes_json,
                status,
                rejection_reason
            )
            VALUES (?1, ?2, ?3, '2026-04-26T00:00:01Z', ?4, '005930', ?5, 'limit', ?6, '0.8', 'intraday', 'test', '[]', 'valid', NULL)",
            params![
                run_id,
                tick_id,
                decision_output_id,
                decision_type,
                quantity,
                limit_price
            ],
        )
        .expect("insert proposal");
    let decision_proposal_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO risk_evaluations (
                run_id,
                tick_id,
                decision_proposal_id,
                observed_at,
                status,
                order_notional,
                reasons_json
            )
            VALUES (?1, ?2, ?3, '2026-04-26T00:00:02Z', ?4, ?5, '[]')",
            params![
                run_id,
                tick_id,
                decision_proposal_id,
                risk_status,
                order_notional
            ],
        )
        .expect("insert risk");
    SeededDecisionIds {
        run_id,
        tick_id,
        decision_proposal_id,
        risk_evaluation_id: connection.last_insert_rowid(),
    }
}

fn seed_portfolio_snapshot_position(
    path: &std::path::Path,
    run_id: i64,
    tick_id: i64,
    observed_at: &str,
    symbol: &str,
    quantity: i64,
    average_price: &str,
) {
    let connection = Connection::open(path).expect("open seed db");
    let positions_json = format!(
        r#"[{{"symbol":"{symbol}","quantity":{quantity},"average_price":"{average_price}"}}]"#
    );
    connection
        .execute(
            "INSERT INTO portfolio_snapshots (
                run_id, tick_id, observed_at, account_suffix, cash, positions_json,
                open_orders_json, today_fills_json
             )
             VALUES (?1, ?2, ?3, '1234', '0', ?4, '[]', '[]')",
            params![run_id, tick_id, observed_at, positions_json],
        )
        .expect("insert portfolio snapshot");
}

#[derive(Debug, Clone, Copy)]
struct ExecutionRequestSeed<'a> {
    ids: SeededDecisionIds,
    execution_request_id: &'a str,
    status: &'a str,
    symbol: &'a str,
    side: &'a str,
    quantity: i64,
    order_type: &'a str,
    limit_price: Option<&'a str>,
    created_at: &'a str,
}

fn insert_execution_request_row(path: &std::path::Path, seed: ExecutionRequestSeed<'_>) {
    let connection = Connection::open(path).expect("open seed db");
    connection
        .execute(
            "INSERT INTO execution_requests (
                run_id,
                tick_id,
                decision_proposal_id,
                created_at,
                status,
                symbol,
                side,
                quantity,
                order_type,
                limit_price,
                order_notional,
                execution_request_id,
                risk_evaluation_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                seed.ids.run_id,
                seed.ids.tick_id,
                seed.ids.decision_proposal_id,
                seed.created_at,
                seed.status,
                seed.symbol,
                seed.side,
                seed.quantity,
                seed.order_type,
                seed.limit_price,
                seed.limit_price,
                seed.execution_request_id,
                seed.ids.risk_evaluation_id,
            ],
        )
        .expect("insert execution request row");
}

fn parse_time(value: &str) -> OffsetDateTime {
    OffsetDateTime::parse(value, &Rfc3339).expect("valid timestamp")
}

fn execution_record(id: &str, ids: SeededDecisionIds) -> NewExecutionRequestRecord {
    execution_record_for(
        id,
        ids,
        OrderSide::Buy,
        Quantity::new(1),
        Decimal::new(73_000, 0),
    )
}

fn execution_record_for(
    id: &str,
    ids: SeededDecisionIds,
    side: OrderSide,
    quantity: Quantity,
    limit_price: Decimal,
) -> NewExecutionRequestRecord {
    NewExecutionRequestRecord {
        execution_request_id: ExecutionRequestId::new(id).expect("execution request id"),
        run_id: ids.run_id,
        tick_id: ids.tick_id,
        decision_proposal_id: ids.decision_proposal_id,
        risk_evaluation_id: ids.risk_evaluation_id,
        symbol: Symbol::new("005930").expect("symbol"),
        side,
        quantity,
        order_type: DecisionOrderType::Limit,
        limit_price: Some(limit_price),
        order_notional: Some(Decimal::from(quantity.get()) * limit_price),
    }
}

fn order_reference(client_order_id: &str) -> OrderReference {
    OrderReference {
        client_order_id: ClientOrderId::new(client_order_id).expect("client order id"),
        broker_order_id: None,
        broker_order_orgno: None,
        exchange_code: None,
        account_key: AccountKey::new("12345678-01").expect("account key"),
        environment: BrokerEnvironment::Paper,
    }
}

fn accepted_follow_up_reference(client_order_id: &str) -> OrderReference {
    order_reference(client_order_id)
        .with_broker_order_id("broker-order-follow-up")
        .expect("broker order id")
        .with_broker_order_orgno("001")
        .expect("broker order orgno")
}

async fn seed_accepted_execution_for_follow_up(path: &std::path::Path) -> StoredExecutionRequestId {
    let ids = seed_decision_with_risk(path, "approved");
    let mut db = MainDb::open(path).expect("open seed db");
    let execution = db
        .record_execution_request(&execution_record("exec-follow-up-parent", ids))
        .expect("record execution request");
    let parent_client_order_id =
        ClientOrderId::new("exec-follow-up-parent").expect("parent client id");
    let journal = SqliteBrokerCommandJournal::open(path).expect("open journal");
    journal
        .reserve_command(place_command(parent_client_order_id.clone()))
        .await
        .expect("reserve parent command");
    db.link_execution_request_to_journal(execution, &parent_client_order_id)
        .expect("link parent journal");
    db.record_execution_order_accepted(
        execution,
        &OrderSubmission {
            reference: accepted_follow_up_reference("exec-follow-up-parent"),
            accepted: true,
        },
    )
    .expect("record accepted parent");
    execution
}

fn execution_workflow_config(read_only: bool) -> ExecutionWorkflowConfig {
    ExecutionWorkflowConfig {
        read_only,
        account_key: AccountKey::new("12345678-01").expect("account key"),
        environment: BrokerEnvironment::Paper,
        market_scope: MarketScope::DomesticStock,
    }
}

fn final_risk_inputs<'a>(
    risk: &'a RiskConfig,
    allowed_symbols: &'a [Symbol],
    _account_key: &'a AccountKey,
) -> FinalExecutionRiskInputs<'a> {
    FinalExecutionRiskInputs {
        risk,
        read_only: false,
        market_scope: trade_engine::gateway::types::MarketScope::DomesticStock,
        allowed_symbols,
        now: OffsetDateTime::now_utc(),
    }
}

async fn seed_unresolved_journal_command(journal: &SqliteBrokerCommandJournal) {
    let client_order_id = ClientOrderId::new("policy-submit-unresolved").expect("client order id");
    journal
        .reserve_command(place_command(client_order_id.clone()))
        .await
        .expect("reserve unresolved command");
    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("policy-submit-unresolved-request").expect("request id"),
        )
        .await
        .expect("mark unresolved command submitting");
    journal
        .mark_reconcile_needed(&client_order_id, "policy submit test")
        .await
        .expect("mark unresolved command");
}

fn execution_status(path: &std::path::Path, id: StoredExecutionRequestId) -> String {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT status FROM execution_requests WHERE id = ?1",
            [id.as_i64()],
            |row| row.get(0),
        )
        .expect("execution status")
}

fn execution_broker_reference(
    path: &std::path::Path,
    id: StoredExecutionRequestId,
) -> (Option<String>, Option<String>) {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT broker_order_id, broker_order_orgno FROM execution_requests WHERE id = ?1",
            [id.as_i64()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("execution broker reference")
}

fn execution_rejection_reason(
    path: &std::path::Path,
    id: StoredExecutionRequestId,
) -> Option<String> {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT rejection_reason FROM execution_requests WHERE id = ?1",
            [id.as_i64()],
            |row| row.get(0),
        )
        .expect("execution rejection reason")
}

fn execution_last_error(path: &std::path::Path, id: StoredExecutionRequestId) -> Option<String> {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT last_error FROM execution_requests WHERE id = ?1",
            [id.as_i64()],
            |row| row.get(0),
        )
        .expect("execution last error")
}

fn place_command(client_order_id: ClientOrderId) -> BrokerCommandRecord {
    place_command_for(
        client_order_id,
        "005930",
        OrderSide::Buy,
        1,
        Decimal::new(73_000, 0),
    )
}

fn place_command_for(
    client_order_id: ClientOrderId,
    symbol: &str,
    side: OrderSide,
    quantity: u64,
    limit_price: Decimal,
) -> BrokerCommandRecord {
    BrokerCommandRecord::new_place_order(
        BrokerEnvironment::Paper,
        CreateOrderRequest::cash_limit(
            client_order_id,
            AccountKey::new("paper-account").expect("account key"),
            Symbol::new(symbol).expect("symbol"),
            side,
            Quantity::new(quantity),
            limit_price,
        )
        .expect("order request"),
    )
    .expect("broker command")
}

fn execution_events(path: &std::path::Path, id: StoredExecutionRequestId) -> Vec<String> {
    let connection = Connection::open(path).expect("open inspect db");
    let mut statement = connection
        .prepare(
            "SELECT event_type
             FROM execution_events
             WHERE execution_request_id = ?1
             ORDER BY id",
        )
        .expect("prepare execution events");
    statement
        .query_map([id.as_i64()], |row| row.get::<_, String>(0))
        .expect("query execution events")
        .collect::<Result<Vec<_>, _>>()
        .expect("execution events")
}

fn execution_client_order_id(
    path: &std::path::Path,
    id: StoredExecutionRequestId,
) -> Option<String> {
    let connection = Connection::open(path).expect("open inspect db");
    connection
        .query_row(
            "SELECT client_order_id FROM execution_requests WHERE id = ?1",
            [id.as_i64()],
            |row| row.get(0),
        )
        .expect("execution client order id")
}

fn journal_status(
    path: &std::path::Path,
    client_order_id: &ClientOrderId,
) -> Option<BrokerCommandStatus> {
    let connection = Connection::open(path).expect("open inspect db");
    connection
        .query_row(
            "SELECT last_known_state
             FROM broker_command_journal
             WHERE client_order_id = ?1",
            [client_order_id.as_str()],
            |row| {
                let status = row.get::<_, String>(0)?;
                BrokerCommandStatus::parse(status.as_str()).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            },
        )
        .optional()
        .expect("journal status")
}

fn journal_execution_request_id(
    path: &std::path::Path,
    client_order_id: &ClientOrderId,
) -> Option<i64> {
    let connection = Connection::open(path).expect("open inspect db");
    connection
        .query_row(
            "SELECT execution_request_id
             FROM broker_command_journal
             WHERE client_order_id = ?1",
            [client_order_id.as_str()],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .map(Option::flatten)
        .expect("journal execution request id")
}

fn journal_exchange_code(
    path: &std::path::Path,
    client_order_id: &ClientOrderId,
) -> Option<String> {
    let connection = Connection::open(path).expect("open inspect db");
    connection
        .query_row(
            "SELECT exchange_code
             FROM broker_command_journal
             WHERE client_order_id = ?1",
            [client_order_id.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(Option::flatten)
        .expect("journal exchange code")
}
