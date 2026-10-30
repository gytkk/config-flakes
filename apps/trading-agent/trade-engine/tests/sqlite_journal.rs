use std::path::{Path, PathBuf};

use rusqlite::Connection;
use rust_decimal::Decimal;
use tempfile::{TempDir, tempdir};
use trade_engine::storage::SqliteBrokerCommandJournal;
use trading_agent_kis_client::journal::{
    BrokerCommandAck, BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus,
};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, BrokerError, BrokerOrderId, BrokerOrderOrgNo,
    CancelOrderRequest, ClientOrderId, CreateOrderRequest, OrderReference, OrderSide, Quantity,
    RequestId, Symbol,
};

#[tokio::test]
async fn sqlite_journal_rejects_duplicate_client_order_id() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let command = place_command("order-1");

    journal
        .reserve_command(command.clone())
        .await
        .expect("reserve command");
    let error = journal
        .reserve_command(command)
        .await
        .expect_err("duplicate should fail");

    assert!(matches!(error, BrokerError::DuplicateIntent { .. }));
}

#[tokio::test]
async fn sqlite_journal_clone_shares_connection_boundary() {
    let fixture = JournalFixture::new();
    let journal = fixture.journal.clone();
    let command = place_command("order-clone-boundary");
    let client_order_id = command.client_order_id.clone();

    journal
        .reserve_command(command)
        .await
        .expect("reserve command through cloned journal");

    let stored = fixture
        .journal
        .get_command(&client_order_id)
        .await
        .expect("load command through original journal")
        .expect("stored command");

    assert_eq!(stored.client_order_id, client_order_id);
}

#[tokio::test]
async fn sqlite_journal_requires_parent_for_child_commands() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let parent = place_command("parent-1");
    let child = BrokerCommandRecord::new_cancel_order(
        &parent,
        &CancelOrderRequest {
            client_order_id: ClientOrderId::new("cancel-1").expect("client order id"),
            reference: OrderReference {
                client_order_id: parent.client_order_id.clone(),
                broker_order_id: Some(BrokerOrderId::new("broker-1").expect("broker id")),
                broker_order_orgno: Some(BrokerOrderOrgNo::new("org-1").expect("org no")),
                exchange_code: None,
                account_key: parent.account_key.clone(),
                environment: parent.environment,
            },
            quantity: Some(Quantity::new(1)),
        },
    )
    .expect("cancel command");

    let error = journal
        .reserve_command(child)
        .await
        .expect_err("missing parent should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
}

#[tokio::test]
async fn sqlite_journal_preserves_record_when_state_transition_is_invalid() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let command = place_command("order-2");
    let client_order_id = command.client_order_id.clone();

    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    let error = journal
        .mark_ack(
            &client_order_id,
            BrokerCommandAck {
                broker_order_id: Some("broker-2".to_owned()),
                broker_order_orgno: Some("org-2".to_owned()),
                raw_ack_code: Some("0".to_owned()),
            },
        )
        .await
        .expect_err("ack before submitting should fail");

    assert!(matches!(error, BrokerError::Invariant { .. }));
    let stored = journal
        .get_command(&client_order_id)
        .await
        .expect("load command")
        .expect("stored command");
    assert_eq!(
        stored.last_known_state,
        BrokerCommandStatus::CommandRecorded
    );
    assert_eq!(stored.broker_order_id, None);
}

#[tokio::test]
async fn sqlite_journal_lists_and_clears_unreconciled_commands() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let command = place_command("order-3");
    let client_order_id = command.client_order_id.clone();

    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("request-3").expect("request id"),
        )
        .await
        .expect("mark submitting");
    journal
        .mark_reconcile_needed(&client_order_id, "transport timeout")
        .await
        .expect("mark reconcile needed");

    let unreconciled = journal
        .list_unreconciled()
        .await
        .expect("list unreconciled");
    assert_eq!(unreconciled.len(), 1);
    assert_eq!(unreconciled[0].client_order_id, client_order_id);

    journal
        .mark_reconciled(&client_order_id, BrokerCommandStatus::Accepted)
        .await
        .expect("mark reconciled");
    assert!(
        journal
            .list_unreconciled()
            .await
            .expect("list unreconciled")
            .is_empty()
    );
}

#[tokio::test]
async fn sqlite_journal_treats_submitting_command_as_unreconciled() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let command = place_command("order-submitting-unresolved");
    let client_order_id = command.client_order_id.clone();

    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("request-submitting-unresolved").expect("request id"),
        )
        .await
        .expect("mark submitting");

    let unreconciled = journal
        .list_unreconciled()
        .await
        .expect("list unreconciled");

    assert_eq!(unreconciled.len(), 1);
    assert_eq!(unreconciled[0].client_order_id, client_order_id);
    assert_eq!(
        unreconciled[0].last_known_state,
        BrokerCommandStatus::Submitting
    );
}

#[tokio::test]
async fn sqlite_journal_marks_pre_reserved_submit_as_reconciliation_needed() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let command = place_command("order-pre-reserved-reconcile");
    let client_order_id = command.client_order_id.clone();

    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    journal
        .mark_reconcile_needed(&client_order_id, "gateway returned without journal ack")
        .await
        .expect("mark reconcile needed");

    let stored = journal
        .get_command(&client_order_id)
        .await
        .expect("load command")
        .expect("stored command");
    assert_eq!(
        stored.last_known_state,
        BrokerCommandStatus::SubmittedAckUnknown
    );
    assert!(stored.reconcile_required);
    assert_eq!(
        journal_events(&fixture.db_path, client_order_id.as_str()),
        vec!["command_recorded", "submitted_ack_unknown"]
    );
}

#[tokio::test]
async fn sqlite_journal_records_command_lifecycle_events() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let command = place_command("order-4");
    let client_order_id = command.client_order_id.clone();

    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("request-4").expect("request id"),
        )
        .await
        .expect("mark submitting");
    journal
        .mark_ack(
            &client_order_id,
            BrokerCommandAck {
                broker_order_id: Some("broker-4".to_owned()),
                broker_order_orgno: Some("org-4".to_owned()),
                raw_ack_code: Some("0".to_owned()),
            },
        )
        .await
        .expect("mark ack");

    assert_eq!(
        journal_events(&fixture.db_path, client_order_id.as_str()),
        vec!["command_recorded", "submitting", "accepted"]
    );
}

#[tokio::test]
async fn sqlite_journal_records_pre_submit_safety_halt() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let command = place_command("order-safety-halt");
    let client_order_id = command.client_order_id.clone();

    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    journal
        .mark_safety_halted(&client_order_id, "final risk blocked")
        .await
        .expect("mark safety halted");

    let stored = journal
        .get_command(&client_order_id)
        .await
        .expect("load command")
        .expect("stored command");
    assert_eq!(stored.last_known_state, BrokerCommandStatus::SafetyHalted);
    assert_eq!(
        journal_events(&fixture.db_path, client_order_id.as_str()),
        vec!["command_recorded", "safety_halted"]
    );
}

#[tokio::test]
async fn sqlite_journal_rolls_back_command_update_when_event_append_fails() {
    let fixture = JournalFixture::new();
    let journal = &fixture.journal;
    let command = place_command("order-5");
    let client_order_id = command.client_order_id.clone();

    journal
        .reserve_command(command)
        .await
        .expect("reserve command");
    install_submitting_event_failure_trigger(&fixture.db_path);

    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("request-5").expect("request id"),
        )
        .await
        .expect_err("event append failure should fail transition");

    let stored = journal
        .get_command(&client_order_id)
        .await
        .expect("load command")
        .expect("stored command");
    assert_eq!(
        stored.last_known_state,
        BrokerCommandStatus::CommandRecorded
    );
    assert_eq!(
        journal_events(&fixture.db_path, client_order_id.as_str()),
        vec!["command_recorded"]
    );
}

struct JournalFixture {
    _tempdir: TempDir,
    db_path: PathBuf,
    journal: SqliteBrokerCommandJournal,
}

impl JournalFixture {
    fn new() -> Self {
        let tempdir = tempdir().expect("tempdir");
        let path = tempdir.path().join("main.db");
        let journal = SqliteBrokerCommandJournal::open(&path).expect("open journal");
        Self {
            _tempdir: tempdir,
            db_path: path,
            journal,
        }
    }
}

fn place_command(client_order_id: &str) -> BrokerCommandRecord {
    BrokerCommandRecord::new_place_order(
        BrokerEnvironment::Paper,
        CreateOrderRequest::cash_limit(
            ClientOrderId::new(client_order_id).expect("client order id"),
            AccountKey::new("paper-account").expect("account key"),
            Symbol::new("005930").expect("symbol"),
            OrderSide::Buy,
            Quantity::new(10),
            Decimal::new(73_000, 0),
        )
        .expect("order request"),
    )
    .expect("broker command")
}

fn journal_events(path: &Path, client_order_id: &str) -> Vec<String> {
    let connection = Connection::open(path).expect("open journal db");
    let mut statement = connection
        .prepare(
            "SELECT event_type
             FROM broker_command_events
             WHERE client_order_id = ?1
             ORDER BY id",
        )
        .expect("prepare events");
    statement
        .query_map([client_order_id], |row| row.get::<_, String>(0))
        .expect("query events")
        .collect::<Result<Vec<_>, _>>()
        .expect("events")
}

fn install_submitting_event_failure_trigger(path: &Path) {
    let connection = Connection::open(path).expect("open journal db");
    connection
        .execute_batch(
            "CREATE TRIGGER fail_broker_command_event_submitting
             BEFORE INSERT ON broker_command_events
             WHEN NEW.event_type = 'submitting'
             BEGIN
                 SELECT RAISE(ABORT, 'forced broker command event failure');
             END;",
        )
        .expect("install trigger");
}
