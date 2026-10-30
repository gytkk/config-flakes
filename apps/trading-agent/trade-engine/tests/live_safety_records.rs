use rusqlite::Connection;
use rust_decimal::Decimal;
use tempfile::tempdir;
use trade_engine::storage::{MainDb, SqliteBrokerCommandJournal};
use trading_agent_kis_client::journal::{BrokerCommandJournal, BrokerCommandRecord};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, ClientOrderId, CreateOrderRequest, OrderSide, Quantity,
    RequestId, Symbol,
};

#[tokio::test]
async fn panic_stop_clear_does_not_clear_unresolved_journal_records() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    MainDb::open(&db_path).expect("open db");
    seed_unresolved_command(&db_path).await;
    let mut db = MainDb::open(&db_path).expect("reopen db");

    db.set_panic_stop(true, Some("operator-1"), "risk drill")
        .expect("activate panic stop");
    assert!(db.panic_stop_active().expect("panic stop active"));
    db.set_panic_stop(false, Some("operator-1"), "drill complete")
        .expect("clear panic stop");

    assert!(!db.panic_stop_active().expect("panic stop cleared"));
    assert_eq!(operator_action_count(&db_path, "panic_stop_activated"), 1);
    assert_eq!(operator_action_count(&db_path, "panic_stop_cleared"), 1);
    let journal = SqliteBrokerCommandJournal::open(&db_path).expect("open journal");
    assert_eq!(
        journal.list_unreconciled().await.expect("unresolved").len(),
        1
    );
}

async fn seed_unresolved_command(path: &std::path::Path) {
    let journal = SqliteBrokerCommandJournal::open(path).expect("open journal");
    let client_order_id = ClientOrderId::new("coid-live-safety-unresolved").expect("client id");
    journal
        .reserve_command(place_command(client_order_id.clone()))
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("req-live-safety-unresolved").expect("request id"),
        )
        .await
        .expect("mark submitting");
    journal
        .mark_reconcile_needed(&client_order_id, "live safety test")
        .await
        .expect("mark reconcile needed");
}

fn place_command(client_order_id: ClientOrderId) -> BrokerCommandRecord {
    BrokerCommandRecord::new_place_order(
        BrokerEnvironment::Live,
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
    .expect("broker command")
}

fn operator_action_count(path: &std::path::Path, action_type: &str) -> u64 {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT COUNT(*) FROM operator_actions WHERE action_type = ?1",
            [action_type],
            |row| row.get(0),
        )
        .expect("operator action count")
}
