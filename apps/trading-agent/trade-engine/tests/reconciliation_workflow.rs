use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use rust_decimal::Decimal;
use tempfile::tempdir;
use trade_engine::reconciliation::{
    ReconciliationGateway, ReconciliationWorkflowConfig, reconcile_pending_execution_requests,
};
use trade_engine::storage::{MainDb, SqliteBrokerCommandJournal};
use trading_agent_kis_client::journal::{
    BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus, BrokerCommandType,
};
use trading_agent_kis_client::{
    AccountKey, AmendOrderRequest, BrokerEnvironment, BrokerError, BrokerOrderId, BrokerOrderOrgNo,
    CancelOrderRequest, ClientOrderId, CreateOrderRequest, OrderReference, OrderSide, Quantity,
    ReconciliationResult, RequestId, Symbol,
};

#[tokio::test]
async fn reconciliation_workflow_records_confirmed_submission_and_clears_journal() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let client_order_id = ClientOrderId::new("exec-reconcile-submitted").expect("client order id");
    let execution_id = seed_reconciliation_case(&db_path, &client_order_id).await;
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway =
        JournalUpdatingReconciler::new(&db_path, ReconciliationResult::ConfirmedSubmitted);
    let mut db = MainDb::open(&db_path).expect("reopen db");

    let outcomes = reconcile_pending_execution_requests(
        &gateway,
        &mut db,
        &journal,
        ReconciliationWorkflowConfig { lookback_days: 1 },
    )
    .await
    .expect("reconcile pending");

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].client_order_id, client_order_id);
    assert_eq!(
        outcomes[0].execution_request_id.map(|id| id.as_i64()),
        Some(execution_id)
    );
    assert_eq!(execution_status(&db_path, execution_id), "accepted");
    assert_eq!(
        execution_events(&db_path, execution_id),
        vec!["reconciliation_needed", "reconciliation_succeeded"]
    );
    assert_eq!(
        journal_status(&db_path, &client_order_id),
        Some("accepted".to_owned())
    );
    assert!(
        journal
            .list_unreconciled()
            .await
            .expect("unresolved")
            .is_empty()
    );
}

#[tokio::test]
async fn reconciliation_workflow_records_confirmed_fill_and_clears_journal() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let client_order_id = ClientOrderId::new("exec-reconcile-filled").expect("client order id");
    let execution_id = seed_reconciliation_case(&db_path, &client_order_id).await;
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = JournalUpdatingReconciler::new(&db_path, ReconciliationResult::ConfirmedFilled);
    let mut db = MainDb::open(&db_path).expect("reopen db");

    reconcile_pending_execution_requests(
        &gateway,
        &mut db,
        &journal,
        ReconciliationWorkflowConfig { lookback_days: 1 },
    )
    .await
    .expect("reconcile pending");

    assert_eq!(execution_status(&db_path, execution_id), "filled");
    assert_eq!(
        execution_events(&db_path, execution_id),
        vec!["reconciliation_needed", "order_filled"]
    );
    assert_eq!(
        journal_status(&db_path, &client_order_id),
        Some("filled".to_owned())
    );
    assert!(
        journal
            .list_unreconciled()
            .await
            .expect("unresolved")
            .is_empty()
    );
}

#[tokio::test]
async fn reconciliation_workflow_records_still_unknown_and_keeps_journal_unresolved() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let client_order_id = ClientOrderId::new("exec-reconcile-unknown").expect("client order id");
    let execution_id = seed_reconciliation_case(&db_path, &client_order_id).await;
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = JournalUpdatingReconciler::new(&db_path, ReconciliationResult::StillUnknown);
    let mut db = MainDb::open(&db_path).expect("reopen db");

    reconcile_pending_execution_requests(
        &gateway,
        &mut db,
        &journal,
        ReconciliationWorkflowConfig { lookback_days: 1 },
    )
    .await
    .expect("reconcile pending");

    assert_eq!(
        execution_status(&db_path, execution_id),
        "reconciliation_needed"
    );
    assert_eq!(
        execution_events(&db_path, execution_id),
        vec!["reconciliation_needed", "reconciliation_still_unknown"]
    );
    assert_eq!(
        journal_status(&db_path, &client_order_id),
        Some("reconcile_failed".to_owned())
    );
    assert_eq!(
        journal.list_unreconciled().await.expect("unresolved").len(),
        1
    );
}

#[tokio::test]
async fn reconciliation_workflow_recovers_submitting_command_as_confirmed_submission() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let client_order_id =
        ClientOrderId::new("exec-reconcile-submitting-confirmed").expect("client order id");
    seed_submitting_command(&db_path, &client_order_id).await;
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway =
        JournalUpdatingReconciler::new(&db_path, ReconciliationResult::ConfirmedSubmitted);
    let mut db = MainDb::open(&db_path).expect("reopen db");

    let outcomes = reconcile_pending_execution_requests(
        &gateway,
        &mut db,
        &journal,
        ReconciliationWorkflowConfig { lookback_days: 1 },
    )
    .await
    .expect("reconcile submitting command");

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].client_order_id, client_order_id);
    assert_eq!(outcomes[0].execution_request_id, None);
    assert_eq!(
        journal_status(&db_path, &client_order_id),
        Some("accepted".to_owned())
    );
    assert!(
        journal
            .list_unreconciled()
            .await
            .expect("unresolved")
            .is_empty()
    );
}

#[tokio::test]
async fn reconciliation_workflow_recovers_submitting_command_as_still_unknown() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let client_order_id =
        ClientOrderId::new("exec-reconcile-submitting-unknown").expect("client order id");
    seed_submitting_command(&db_path, &client_order_id).await;
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = JournalUpdatingReconciler::new(&db_path, ReconciliationResult::StillUnknown);
    let mut db = MainDb::open(&db_path).expect("reopen db");

    let outcomes = reconcile_pending_execution_requests(
        &gateway,
        &mut db,
        &journal,
        ReconciliationWorkflowConfig { lookback_days: 1 },
    )
    .await
    .expect("reconcile submitting command");

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].client_order_id, client_order_id);
    assert_eq!(outcomes[0].execution_request_id, None);
    assert_eq!(
        journal_status(&db_path, &client_order_id),
        Some("reconcile_failed".to_owned())
    );
    assert_eq!(
        journal.list_unreconciled().await.expect("unresolved").len(),
        1
    );
}

#[tokio::test]
async fn reconciliation_workflow_maps_ambiguous_cancel_child_to_parent_execution() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let child_client_order_id =
        ClientOrderId::new("exec-reconcile-cancel-child").expect("client order id");
    let execution_id = seed_follow_up_reconciliation_case(
        &db_path,
        &child_client_order_id,
        BrokerCommandType::Cancel,
    )
    .await;
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway =
        JournalUpdatingReconciler::new(&db_path, ReconciliationResult::ConfirmedCancelled);
    let mut db = MainDb::open(&db_path).expect("reopen db");

    let outcomes = reconcile_pending_execution_requests(
        &gateway,
        &mut db,
        &journal,
        ReconciliationWorkflowConfig { lookback_days: 1 },
    )
    .await
    .expect("reconcile cancel child");

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].client_order_id, child_client_order_id);
    assert_eq!(
        outcomes[0].execution_request_id.map(|id| id.as_i64()),
        Some(execution_id)
    );
    assert_eq!(execution_status(&db_path, execution_id), "cancelled");
    assert_eq!(
        execution_events(&db_path, execution_id),
        vec!["order_accepted", "reconciliation_needed", "order_cancelled"]
    );
}

#[tokio::test]
async fn reconciliation_workflow_maps_ambiguous_amend_child_to_parent_execution() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    let child_client_order_id =
        ClientOrderId::new("exec-reconcile-amend-child").expect("client order id");
    let execution_id = seed_follow_up_reconciliation_case(
        &db_path,
        &child_client_order_id,
        BrokerCommandType::Amend,
    )
    .await;
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    let gateway = JournalUpdatingReconciler::new(&db_path, ReconciliationResult::ConfirmedAmended);
    let mut db = MainDb::open(&db_path).expect("reopen db");

    let outcomes = reconcile_pending_execution_requests(
        &gateway,
        &mut db,
        &journal,
        ReconciliationWorkflowConfig { lookback_days: 1 },
    )
    .await
    .expect("reconcile amend child");

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].client_order_id, child_client_order_id);
    assert_eq!(
        outcomes[0].execution_request_id.map(|id| id.as_i64()),
        Some(execution_id)
    );
    assert_eq!(execution_status(&db_path, execution_id), "accepted");
    assert_eq!(
        execution_events(&db_path, execution_id),
        vec![
            "order_accepted",
            "reconciliation_needed",
            "reconciliation_succeeded"
        ]
    );
}

struct JournalUpdatingReconciler {
    journal: SqliteBrokerCommandJournal,
    result: ReconciliationResult,
}

impl JournalUpdatingReconciler {
    fn new(path: &std::path::Path, result: ReconciliationResult) -> Self {
        Self {
            journal: SqliteBrokerCommandJournal::open(path).expect("open reconciler journal"),
            result,
        }
    }
}

#[async_trait]
impl ReconciliationGateway for JournalUpdatingReconciler {
    async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        _lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError> {
        match self.result {
            ReconciliationResult::ConfirmedSubmitted | ReconciliationResult::ConfirmedAmended => {
                self.journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Accepted)
                    .await?;
            }
            ReconciliationResult::ConfirmedRejected => {
                self.journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Rejected)
                    .await?;
            }
            ReconciliationResult::ConfirmedCancelled => {
                self.journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Cancelled)
                    .await?;
            }
            ReconciliationResult::ConfirmedFilled => {
                self.journal
                    .mark_reconciled(client_order_id, BrokerCommandStatus::Filled)
                    .await?;
            }
            ReconciliationResult::StillUnknown => {
                self.journal
                    .mark_reconcile_failed(client_order_id, "still unknown")
                    .await?;
            }
            ReconciliationResult::ManualInterventionRequired => {
                self.journal
                    .mark_reconcile_failed(client_order_id, "manual intervention required")
                    .await?;
            }
        }
        Ok(self.result.clone())
    }
}

async fn seed_submitting_command(path: &std::path::Path, client_order_id: &ClientOrderId) {
    let journal = SqliteBrokerCommandJournal::open(path).expect("open journal");
    journal
        .reserve_command(place_command(client_order_id.clone()))
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            client_order_id,
            &RequestId::new(format!("req-{client_order_id}")).expect("request id"),
        )
        .await
        .expect("mark submitting");
}

async fn seed_reconciliation_case(path: &std::path::Path, client_order_id: &ClientOrderId) -> i64 {
    let journal = SqliteBrokerCommandJournal::open(path).expect("open journal");
    journal
        .reserve_command(place_command(client_order_id.clone()))
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            client_order_id,
            &RequestId::new(format!("req-{client_order_id}")).expect("request id"),
        )
        .await
        .expect("mark submitting");
    journal
        .mark_reconcile_needed(client_order_id, "ambiguous submit")
        .await
        .expect("mark reconcile needed");

    let connection = Connection::open(path).expect("open db");
    let observed_at = "2026-04-26T00:00:00Z";
    connection
        .execute(
            "INSERT INTO runtime_runs (
                mode, environment, account_suffix, started_at, status
            )
            VALUES ('dry_run', 'paper', '5678', ?1, 'started')",
            [observed_at],
        )
        .expect("insert run");
    let run_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO runtime_ticks (
                run_id, mode, started_at, status
            )
            VALUES (?1, 'dry_run', ?2, 'started')",
            params![run_id, observed_at],
        )
        .expect("insert tick");
    let tick_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO decision_outputs (
                run_id, tick_id, observed_at, provider, raw_output_json, status
            )
            VALUES (?1, ?2, ?3, 'test', '{}', 'parsed')",
            params![run_id, tick_id, observed_at],
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
                reason,
                risk_notes_json,
                status
            )
            VALUES (?1, ?2, ?3, ?4, 'buy', '005930', 1, 'limit', '73000', '1', 'test', '[]', 'valid')",
            params![run_id, tick_id, decision_output_id, observed_at],
        )
        .expect("insert decision proposal");
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
            VALUES (?1, ?2, ?3, ?4, 'approved', '73000', '[]')",
            params![run_id, tick_id, decision_proposal_id, observed_at],
        )
        .expect("insert risk evaluation");
    let risk_evaluation_id = connection.last_insert_rowid();
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
                risk_evaluation_id,
                client_order_id,
                submitted_at,
                last_error
            )
            VALUES (?1, ?2, ?3, ?4, 'reconciliation_needed', '005930', 'buy', 1, 'limit', '73000', '73000', ?5, ?6, ?7, ?4, 'ambiguous submit')",
            params![
                run_id,
                tick_id,
                decision_proposal_id,
                observed_at,
                format!("execution-{client_order_id}"),
                risk_evaluation_id,
                client_order_id.as_str()
            ],
        )
        .expect("insert execution request");
    let execution_id = connection.last_insert_rowid();
    connection
        .execute(
            "UPDATE broker_command_journal
             SET execution_request_id = ?2
             WHERE client_order_id = ?1",
            params![client_order_id.as_str(), execution_id],
        )
        .expect("link journal");
    connection
        .execute(
            "INSERT INTO execution_events (
                execution_request_id,
                event_type,
                observed_at,
                client_order_id,
                metadata_json
            )
            VALUES (?1, 'reconciliation_needed', ?2, ?3, '{}')",
            params![execution_id, observed_at, client_order_id.as_str()],
        )
        .expect("insert reconciliation-needed event");
    execution_id
}

async fn seed_follow_up_reconciliation_case(
    path: &std::path::Path,
    child_client_order_id: &ClientOrderId,
    command_type: BrokerCommandType,
) -> i64 {
    let journal = SqliteBrokerCommandJournal::open(path).expect("open journal");
    let parent_client_order_id =
        ClientOrderId::new(format!("parent-{child_client_order_id}")).expect("parent client id");
    let parent = place_command(parent_client_order_id.clone());
    journal
        .reserve_command(parent.clone())
        .await
        .expect("reserve parent command");
    let reference = accepted_reference(parent_client_order_id.clone());
    let child = match command_type {
        BrokerCommandType::Cancel => BrokerCommandRecord::new_cancel_order(
            &parent,
            &CancelOrderRequest {
                client_order_id: child_client_order_id.clone(),
                reference,
                quantity: Some(Quantity::new(1)),
            },
        )
        .expect("cancel command"),
        BrokerCommandType::Amend => BrokerCommandRecord::new_amend_order(
            &parent,
            &AmendOrderRequest {
                client_order_id: child_client_order_id.clone(),
                reference,
                quantity: Some(Quantity::new(1)),
                limit_price: Some(Decimal::new(72_000, 0)),
            },
        )
        .expect("amend command"),
        BrokerCommandType::Place => unreachable!("follow-up seed requires cancel or amend"),
    };
    journal
        .reserve_command(child)
        .await
        .expect("reserve child command");
    journal
        .mark_submitting(
            child_client_order_id,
            &RequestId::new(format!("req-{child_client_order_id}")).expect("request id"),
        )
        .await
        .expect("mark child submitting");
    journal
        .mark_reconcile_needed(child_client_order_id, "ambiguous follow-up")
        .await
        .expect("mark child reconcile needed");

    let execution_id = insert_accepted_execution_row(path, &parent_client_order_id);
    let connection = Connection::open(path).expect("open db");
    for client_order_id in [&parent_client_order_id, child_client_order_id] {
        connection
            .execute(
                "UPDATE broker_command_journal
                 SET execution_request_id = ?2
                 WHERE client_order_id = ?1",
                params![client_order_id.as_str(), execution_id],
            )
            .expect("link journal");
    }
    connection
        .execute(
            "INSERT INTO execution_events (
                execution_request_id,
                event_type,
                observed_at,
                client_order_id,
                metadata_json
            )
            VALUES
                (?1, 'order_accepted', '2026-04-26T00:00:00Z', ?2, '{}'),
                (?1, 'reconciliation_needed', '2026-04-26T00:00:01Z', ?3, '{}')",
            params![
                execution_id,
                parent_client_order_id.as_str(),
                child_client_order_id.as_str()
            ],
        )
        .expect("insert follow-up events");
    execution_id
}

fn insert_accepted_execution_row(path: &std::path::Path, client_order_id: &ClientOrderId) -> i64 {
    let connection = Connection::open(path).expect("open db");
    let observed_at = "2026-04-26T00:00:00Z";
    connection
        .execute(
            "INSERT INTO runtime_runs (
                mode, environment, account_suffix, started_at, status
            )
            VALUES ('dry_run', 'paper', '5678', ?1, 'started')",
            [observed_at],
        )
        .expect("insert run");
    let run_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO runtime_ticks (
                run_id, mode, started_at, status
            )
            VALUES (?1, 'dry_run', ?2, 'started')",
            params![run_id, observed_at],
        )
        .expect("insert tick");
    let tick_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO decision_outputs (
                run_id, tick_id, observed_at, provider, raw_output_json, status
            )
            VALUES (?1, ?2, ?3, 'test', '{}', 'parsed')",
            params![run_id, tick_id, observed_at],
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
                reason,
                risk_notes_json,
                status
            )
            VALUES (?1, ?2, ?3, ?4, 'buy', '005930', 1, 'limit', '73000', '1', 'test', '[]', 'valid')",
            params![run_id, tick_id, decision_output_id, observed_at],
        )
        .expect("insert decision proposal");
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
            VALUES (?1, ?2, ?3, ?4, 'approved', '73000', '[]')",
            params![run_id, tick_id, decision_proposal_id, observed_at],
        )
        .expect("insert risk evaluation");
    let risk_evaluation_id = connection.last_insert_rowid();
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
                risk_evaluation_id,
                client_order_id,
                broker_order_id,
                broker_order_orgno,
                submitted_at,
                completed_at
            )
            VALUES (?1, ?2, ?3, ?4, 'accepted', '005930', 'buy', 1, 'limit', '73000', '73000', ?5, ?6, ?7, 'broker-order-follow-up', '001', ?4, ?4)",
            params![
                run_id,
                tick_id,
                decision_proposal_id,
                observed_at,
                format!("execution-{client_order_id}"),
                risk_evaluation_id,
                client_order_id.as_str()
            ],
        )
        .expect("insert accepted execution request");
    connection.last_insert_rowid()
}

fn place_command(client_order_id: ClientOrderId) -> BrokerCommandRecord {
    BrokerCommandRecord::new_place_order(
        BrokerEnvironment::Paper,
        CreateOrderRequest::cash_limit(
            client_order_id,
            AccountKey::new("12345678-01").expect("account key"),
            Symbol::new("005930").expect("symbol"),
            OrderSide::Buy,
            Quantity::new(1),
            Decimal::new(73_000, 0),
        )
        .expect("order request"),
    )
    .expect("place command")
}

fn accepted_reference(client_order_id: ClientOrderId) -> OrderReference {
    OrderReference {
        client_order_id,
        broker_order_id: Some(BrokerOrderId::new("broker-order-follow-up").expect("broker id")),
        broker_order_orgno: Some(BrokerOrderOrgNo::new("001").expect("broker orgno")),
        exchange_code: None,
        account_key: AccountKey::new("12345678-01").expect("account key"),
        environment: BrokerEnvironment::Paper,
    }
}

fn execution_status(path: &std::path::Path, id: i64) -> String {
    let connection = Connection::open(path).expect("open inspect db");
    connection
        .query_row(
            "SELECT status FROM execution_requests WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .expect("execution status")
}

fn execution_events(path: &std::path::Path, id: i64) -> Vec<String> {
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
        .query_map([id], |row| row.get::<_, String>(0))
        .expect("query execution events")
        .collect::<Result<Vec<_>, _>>()
        .expect("execution events")
}

fn journal_status(path: &std::path::Path, client_order_id: &ClientOrderId) -> Option<String> {
    let connection = Connection::open(path).expect("open inspect db");
    connection
        .query_row(
            "SELECT last_known_state
             FROM broker_command_journal
             WHERE client_order_id = ?1",
            [client_order_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .expect("journal status")
}
