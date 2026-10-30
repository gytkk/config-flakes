use std::path::PathBuf;

use rusqlite::Connection;
use rust_decimal::Decimal;
use tempfile::tempdir;
use trade_engine::config::{EngineConfig, KisConfig, KisEnvironment, StorageConfig};
use trade_engine::doctor::{StartupDoctorContext, run_read_only_startup_doctor};
use trade_engine::gateway::testing::MockTradingGateway;
use trade_engine::gateway::types::{MarketSession, Quote, Symbol};
use trade_engine::storage::SqliteBrokerCommandJournal;
use trading_agent_kis_client::journal::{BrokerCommandJournal, BrokerCommandRecord};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, ClientOrderId, CreateOrderRequest, OrderSide, Quantity,
    RequestId,
};

#[tokio::test]
async fn read_only_startup_doctor_checks_broker_snapshots_quotes_and_market_session() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let config = test_config(db_path.clone());
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: symbol.clone(),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        })
        .with_market_session(MarketSession {
            trading_day: "20260426".to_owned(),
            is_open: true,
        });

    let report = run_read_only_startup_doctor(
        &config,
        &gateway,
        StartupDoctorContext {
            account_suffix: "5678",
            watchlist: &[symbol],
        },
    )
    .await
    .expect("startup doctor passes");

    assert_eq!(report.account_suffix, "5678");
    assert_eq!(report.watchlist_count, 1);
    assert_eq!(report.quote_count, 1);
    assert_eq!(report.position_count, 0);
    assert_eq!(report.unresolved_command_count, 0);
    assert!(!report.broker_writable);
    assert_eq!(report.broker_environment.as_str(), "paper");
    assert_eq!(report.market_session.trading_day, "20260426");

    let output = report.render();
    assert!(output.contains("Startup Doctor"));
    assert!(output.contains("  status: passed"));
    assert!(output.contains(
        "  purpose: verify config, local state, broker read access, and market data before runtime start"
    ));
    assert!(output.contains("  mode: dry_run"));
    assert!(output.contains("Preflight checks"));
    assert!(output.contains("  configuration: passed (read_only=true, watchlist=1)"));
    assert!(output.contains("  local_state: passed (unresolved_commands=0)"));
    assert!(output.contains("  broker_connection: passed (environment=paper, writable=false)"));
    assert!(output.contains("  account_access: passed (account_suffix=5678, positions=0)"));
    assert!(output.contains("  market_data: passed (quotes=1)"));
    assert!(output.contains("  market_session: passed (trading_day=20260426, open=true)"));
    assert!(!output.contains("12345678"));

    let calls = gateway.call_counts();
    assert_eq!(calls.ping, 1);
    assert_eq!(calls.account_snapshot, 1);
    assert_eq!(calls.quotes, 1);
    assert_eq!(calls.market_session, 1);
    assert_eq!(
        startup_doctor_audits(&db_path),
        vec![(
            "startup_doctor_passed".to_owned(),
            None,
            "passed".to_owned()
        )]
    );
}

#[tokio::test]
async fn read_only_startup_doctor_fails_when_quote_collection_fails() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let config = test_config(db_path.clone());
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = MockTradingGateway::new().with_cash(Decimal::new(1_000_000, 0));

    let error = run_read_only_startup_doctor(
        &config,
        &gateway,
        StartupDoctorContext {
            account_suffix: "5678",
            watchlist: &[symbol],
        },
    )
    .await
    .expect_err("missing quote fails startup doctor");

    assert!(error.to_string().contains("gateway stage quote failed"));
    let audits = startup_doctor_audits(&db_path);
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].0, "startup_doctor_failed");
    assert!(
        audits[0]
            .1
            .as_deref()
            .is_some_and(|reason| { reason.contains("gateway stage quote failed") })
    );
    assert_eq!(audits[0].2, "failed");
}

#[tokio::test]
async fn read_only_startup_doctor_blocks_unresolved_broker_commands_before_gateway_reads() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let config = test_config(db_path.clone());
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: symbol.clone(),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });
    seed_unresolved_command(&db_path).await;

    let error = run_read_only_startup_doctor(
        &config,
        &gateway,
        StartupDoctorContext {
            account_suffix: "5678",
            watchlist: &[symbol],
        },
    )
    .await
    .expect_err("unresolved journal blocks startup doctor");

    assert!(error.to_string().contains("unresolved broker command"));
    let calls = gateway.call_counts();
    assert_eq!(calls.ping, 0);
    assert_eq!(calls.account_snapshot, 0);
    assert_eq!(calls.quotes, 0);
    assert_eq!(calls.market_session, 0);
    let audits = startup_doctor_audits(&db_path);
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].0, "startup_doctor_failed");
    assert!(
        audits[0]
            .1
            .as_deref()
            .is_some_and(|reason| { reason.contains("unresolved broker command") })
    );
    assert_eq!(audits[0].2, "failed");
}

fn test_config(main_db_path: PathBuf) -> EngineConfig {
    EngineConfig {
        read_only: true,
        watchlist: vec!["005930".to_owned()],
        broker: Default::default(),
        market: Default::default(),
        kis: KisConfig {
            environment: KisEnvironment::Paper,
            token_cache_path: PathBuf::from(".local/state/trade-engine/kis-paper-token.json"),
            ..Default::default()
        },
        toss: Default::default(),
        storage: StorageConfig {
            main_db_path: Some(main_db_path),
        },
        runtime: Default::default(),
        strategy: Default::default(),
        decision: Default::default(),
        advisor: Default::default(),
        universe_discovery: Default::default(),
        domestic_market_alignment: Default::default(),
        sizing: Default::default(),
        exit_policy: Default::default(),
        risk: Default::default(),
        live: Default::default(),
    }
}

async fn seed_unresolved_command(path: &std::path::Path) {
    let journal = SqliteBrokerCommandJournal::open(path).expect("open journal");
    let client_order_id = ClientOrderId::new("coid-startup-unresolved").expect("client order id");
    journal
        .reserve_command(place_command(client_order_id.clone()))
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("req-startup-unresolved").expect("request id"),
        )
        .await
        .expect("mark submitting");
    journal
        .mark_reconcile_needed(&client_order_id, "startup doctor test")
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
    .expect("place command")
}

fn startup_doctor_audits(path: &std::path::Path) -> Vec<(String, Option<String>, String)> {
    let connection = Connection::open(path).expect("open inspect db");
    let mut statement = connection
        .prepare(
            "SELECT action_type, reason, json_extract(metadata_json, '$.status')
             FROM operator_actions
             WHERE target_type = 'startup_doctor'
             ORDER BY id",
        )
        .expect("prepare startup doctor audits");
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("query startup doctor audits")
        .collect::<Result<Vec<_>, _>>()
        .expect("startup doctor audits")
}
