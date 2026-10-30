use rusqlite::{Connection, params};
use rust_decimal::Decimal;
use std::fs;
use tempfile::tempdir;
use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime, Time};

use trade_engine::config::{EngineConfig, RuntimeConfig, StrategyConfig};
use trade_engine::execution::{ExecutionEventType, ExecutionRequestStatus};
use trade_engine::gateway::testing::MockTradingGateway;
use trade_engine::gateway::types::{
    Fill, MarketScope, Position, Quantity, Quote, Symbol, TradingDay,
};
use trade_engine::runtime::{DryRunRunContext, DryRunRuntimeSettings, run_dry_run_tick};
use trade_engine::storage::{
    AuditCounts, BlockingMainDb, CompletedFeedbackQueryAudit, FailedFeedbackQueryAudit,
    FeedbackQueryAuditInput, MainDb, NewOhlcvBarRecord, NewTechnicalFeatureAuditComputation,
    NewTechnicalFeatureAuditRecord,
};
use trade_engine::technical_analysis::{
    DailyOhlcvBar, DailyTechnicalFeatures, OhlcvBarInterval, PriceAdjustmentPolicy,
    TechnicalFeatureParams,
};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, ClientOrderId, OrderReference, OrderSide, OrderState,
    OrderStatus, OrderStatusEvidence,
};

#[test]
fn main_db_applies_runtime_audit_migration() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");

    let db = MainDb::open(&db_path).expect("open db");

    assert_eq!(
        db.applied_migrations().expect("migrations"),
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
    );
    assert_eq!(db.audit_counts().expect("counts"), AuditCounts::default());
}

#[test]
fn feedback_status_summary_reads_db_health_and_counts() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");

    let db = MainDb::open(&db_path).expect("open db");
    let summary = db.feedback_status_summary().expect("feedback status");

    assert_eq!(
        summary.applied_migrations,
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
    );
    assert_eq!(summary.audit_counts, AuditCounts::default());
    assert!(!summary.unresolved_broker_commands);
    assert!(!summary.panic_stop_active);
    assert!(summary.command_token_usage.is_empty());
}

#[test]
fn feedback_review_context_reads_recent_operator_actions() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let db = MainDb::open(&db_path).expect("open db");
    let connection = Connection::open(&db_path).expect("inspect db");

    connection
        .execute(
            "INSERT INTO operator_actions (
                observed_at, operator_id, action_type, target_type, target_id, reason, metadata_json
             )
             VALUES
                ('2999-01-01T00:00:00Z', NULL, 'tick_advisory_completed', 'runtime_run', '1', NULL, '{\"status\":\"parsed\",\"output\":{\"summary\":\"advisor ok\"}}'),
                ('2999-01-01T00:05:00Z', NULL, 'universe_snapshot_refreshed', 'universe_snapshot', '2', NULL, '{\"summary\":\"universe ok\",\"candidate_count\":3}')",
            [],
        )
        .expect("insert actions");

    let context = db.feedback_review_context(24, 20).expect("review context");

    assert_eq!(context.window.since_hours, 24);
    assert_eq!(context.window.limit, 20);
    assert_eq!(context.operator_actions.len(), 2);
    assert_eq!(
        context.operator_actions[0].action_type,
        "universe_snapshot_refreshed"
    );
    assert_eq!(context.operator_actions[0].target_id.as_deref(), Some("2"));
    assert_eq!(
        context.operator_actions[1].action_type,
        "tick_advisory_completed"
    );
}

#[test]
fn record_feedback_query_audit_records_completed_and_failed_attempts() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");

    db.record_feedback_query_completed(&CompletedFeedbackQueryAudit {
        input: feedback_query_audit_input(),
        raw_output_json: "{\"summary\":\"ok\"}".to_owned(),
        parsed_output: serde_json::json!({"summary": "ok"}),
    })
    .expect("record completed");
    db.record_feedback_query_failed(&FailedFeedbackQueryAudit {
        input: feedback_query_audit_input(),
        error: "provider failed".to_owned(),
    })
    .expect("record failed");

    let connection = Connection::open(&db_path).expect("inspect db");
    let completed_count: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM operator_actions WHERE action_type = 'feedback_query_completed'",
            [],
            |row| row.get(0),
        )
        .expect("completed count");
    let failed_count: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM operator_actions WHERE action_type = 'feedback_query_failed'",
            [],
            |row| row.get(0),
        )
        .expect("failed count");

    assert_eq!(completed_count, 1);
    assert_eq!(failed_count, 1);
}

fn feedback_query_audit_input() -> FeedbackQueryAuditInput {
    FeedbackQueryAuditInput {
        schema_version: 1,
        since_hours: 24,
        limit: 20,
        prompt: "why?".to_owned(),
        context_summary: serde_json::json!({"operator_actions": 0}),
    }
}

#[test]
fn main_db_schema_contains_plan_v4_system_of_record_tables_and_links() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");

    let _db = MainDb::open(&db_path).expect("open db");
    let connection = Connection::open(&db_path).expect("inspect db");

    for table in [
        "decision_inputs",
        "execution_events",
        "fills",
        "operator_actions",
        "universe_snapshots",
        "ohlcv_bars",
        "technical_feature_audits",
        "broker_command_events",
        "panic_stop_state",
    ] {
        assert!(table_exists(&connection, table), "missing table {table}");
    }
    for table in [
        "strategy_specs",
        "strategy_versions",
        "backtest_runs",
        "backtest_metrics",
        "daily_reports",
    ] {
        assert!(
            !table_exists(&connection, table),
            "deferred table should not remain in final V4 schema: {table}"
        );
    }

    assert_column_exists(&connection, "schema_migrations", "checksum");
    for column in [
        "tick_sequence",
        "lease_owner",
        "lease_expires_at",
        "duration_ms",
        "status_detail_json",
    ] {
        assert_column_exists(&connection, "runtime_ticks", column);
    }
    for column in ["open_orders_json", "today_fills_json"] {
        assert_column_exists(&connection, "portfolio_snapshots", column);
    }
    assert_column_exists(&connection, "market_snapshots", "full_name");
    for column in [
        "observed_at",
        "expires_at",
        "environment",
        "account_suffix",
        "provider",
        "symbols_json",
        "output_json",
    ] {
        assert_column_exists(&connection, "universe_snapshots", column);
    }
    for column in [
        "observed_at",
        "source",
        "environment",
        "symbol",
        "market_scope",
        "exchange",
        "currency",
        "interval",
        "trading_day",
        "bar_start_at",
        "bar_end_at",
        "adjustment_policy",
        "data_quality_json",
        "open",
        "high",
        "low",
        "close",
        "volume",
    ] {
        assert_column_exists(&connection, "ohlcv_bars", column);
    }
    for column in [
        "observed_at",
        "source",
        "symbol",
        "interval",
        "ohlcv_bar_id",
        "feature_set",
        "features_json",
        "inputs_json",
    ] {
        assert_column_exists(&connection, "technical_feature_audits", column);
    }
    assert!(index_exists(
        &connection,
        "idx_ohlcv_bars_symbol_interval_end"
    ));
    assert!(index_exists(
        &connection,
        "idx_technical_feature_audits_symbol_interval"
    ));
    assert!(
        !column_exists(&connection, "strategy_signals", "strategy_version_id"),
        "deferred strategy version link should not remain in final V4 schema"
    );
    assert_column_exists(&connection, "decision_outputs", "decision_input_id");
    for column in [
        "execution_request_id",
        "risk_evaluation_id",
        "client_order_id",
        "broker_order_id",
        "broker_order_orgno",
        "submitted_at",
        "completed_at",
        "rejection_reason",
        "last_error",
    ] {
        assert_column_exists(&connection, "execution_requests", column);
    }
    assert_column_exists(
        &connection,
        "broker_command_journal",
        "execution_request_id",
    );
    for column in ["client_order_id", "event_type", "metadata_json"] {
        assert_column_exists(&connection, "broker_command_events", column);
    }
}

#[test]
fn main_db_records_ohlcv_bars_and_technical_feature_audits_append_only() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    let bar = daily_bar(1, 10, 11, 9, 10, 100);

    let bar_id = db
        .record_ohlcv_bar(&NewOhlcvBarRecord {
            observed_at: test_time(2),
            source: "unit-test".to_owned(),
            environment: BrokerEnvironment::Paper,
            market_scope: MarketScope::DomesticStock,
            interval: OhlcvBarInterval::Daily,
            bar_start_at: test_time(1),
            bar_end_at: test_time(2),
            adjustment_policy: PriceAdjustmentPolicy::Raw,
            data_quality_flags: vec!["complete".to_owned()],
            bar: bar.clone(),
        })
        .expect("record bar");

    let feature_id = db
        .record_technical_feature_audit(&NewTechnicalFeatureAuditRecord {
            observed_at: test_time(2),
            source: "unit-test".to_owned(),
            interval: OhlcvBarInterval::Daily,
            ohlcv_bar_id: Some(bar_id),
            feature_set: "daily-core-v1".to_owned(),
            features: DailyTechnicalFeatures {
                symbol: bar.symbol.clone(),
                trading_day: bar.trading_day,
                sma_short: Some(Decimal::new(10, 0)),
                sma_long: None,
                ema_short: Some(Decimal::new(10, 0)),
                ema_long: None,
                atr: Some(Decimal::new(2, 0)),
                rsi: Some(Decimal::new(50, 0)),
                roc: None,
                average_volume: Some(Decimal::new(100, 0)),
                volume_ratio: None,
                volume_breakout: Some(false),
            },
            input_bar_ids: vec![bar_id],
            params: technical_feature_params(),
        })
        .expect("record feature audit");

    let second_bar_id = db
        .record_ohlcv_bar(&NewOhlcvBarRecord {
            observed_at: test_time(3),
            source: "unit-test".to_owned(),
            environment: BrokerEnvironment::Paper,
            market_scope: MarketScope::DomesticStock,
            interval: OhlcvBarInterval::Daily,
            bar_start_at: test_time(2),
            bar_end_at: test_time(3),
            adjustment_policy: PriceAdjustmentPolicy::Raw,
            data_quality_flags: vec!["complete".to_owned()],
            bar: daily_bar(2, 11, 12, 10, 11, 110),
        })
        .expect("record second bar");

    assert_eq!(bar_id.as_i64(), 1);
    assert_eq!(feature_id.as_i64(), 1);
    assert_eq!(second_bar_id.as_i64(), 2);

    let connection = Connection::open(&db_path).expect("inspect db");
    let row = connection
        .query_row(
            "SELECT observed_at, source, environment, market_scope, exchange, currency,
                    interval, trading_day, bar_start_at, bar_end_at, adjustment_policy,
                    data_quality_json, open, high, low, close, volume
             FROM ohlcv_bars
             WHERE id = ?1",
            [bar_id.as_i64()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, String>(14)?,
                    row.get::<_, String>(15)?,
                    row.get::<_, String>(16)?,
                ))
            },
        )
        .expect("ohlcv row");
    assert_eq!(row.0, "2026-01-02T00:00:00Z");
    assert_eq!(row.1, "unit-test");
    assert_eq!(row.2, "paper");
    assert_eq!(row.3, "domestic_stock");
    assert_eq!(row.4, None);
    assert_eq!(row.5, None);
    assert_eq!(row.6, "daily");
    assert_eq!(row.7, "2026-01-01");
    assert_eq!(row.8, "2026-01-01T00:00:00Z");
    assert_eq!(row.9, "2026-01-02T00:00:00Z");
    assert_eq!(row.10, "raw");
    assert_eq!(row.11, r#"["complete"]"#);
    assert_eq!(row.12, "10");
    assert_eq!(row.13, "11");
    assert_eq!(row.14, "9");
    assert_eq!(row.15, "10");
    assert_eq!(row.16, "100");

    let (features_json, inputs_json): (String, String) = connection
        .query_row(
            "SELECT features_json, inputs_json
             FROM technical_feature_audits
             WHERE id = ?1",
            [feature_id.as_i64()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("feature audit row");
    let features: serde_json::Value = serde_json::from_str(&features_json).expect("features json");
    let inputs: serde_json::Value = serde_json::from_str(&inputs_json).expect("inputs json");
    assert_eq!(features["symbol"], "005930");
    assert_eq!(features["sma_short"], "10");
    assert_eq!(features["volume_breakout"], false);
    assert_eq!(inputs["input_bar_ids"], serde_json::json!([1]));
    assert_eq!(inputs["params"]["sma_short_window"], 2);
    assert_eq!(inputs["params"]["volume_breakout_ratio"], "5");

    let counts: (i64, i64) = connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM ohlcv_bars),
                (SELECT COUNT(*) FROM technical_feature_audits)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("counts");
    assert_eq!(counts, (2, 1));
}

#[test]
fn main_db_recent_ohlcv_bars_returns_chronological_output() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for (day, close) in [(1, 10), (2, 11), (3, 12)] {
        db.record_ohlcv_bar(&NewOhlcvBarRecord {
            observed_at: test_time(day + 1),
            source: "unit-test".to_owned(),
            environment: BrokerEnvironment::Paper,
            market_scope: MarketScope::DomesticStock,
            interval: OhlcvBarInterval::Daily,
            bar_start_at: test_time(day),
            bar_end_at: test_time(day + 1),
            adjustment_policy: PriceAdjustmentPolicy::Raw,
            data_quality_flags: Vec::new(),
            bar: daily_bar(day, close, close + 1, close - 1, close, 100),
        })
        .expect("record bar");
    }

    let bars = db
        .recent_ohlcv_bars(
            &Symbol::new("005930").expect("symbol"),
            OhlcvBarInterval::Daily,
            2,
        )
        .expect("recent bars");

    assert_eq!(
        bars,
        vec![
            daily_bar(2, 11, 12, 10, 11, 100),
            daily_bar(3, 12, 13, 11, 12, 100)
        ]
    );
}

#[test]
fn main_db_ohlcv_bars_for_symbol_between_returns_latest_daily_rows_chronologically() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for (day, close, observed_day, end_day) in
        [(1, 10, 1, 1), (2, 20, 2, 2), (2, 22, 3, 3), (3, 30, 4, 4)]
    {
        db.record_ohlcv_bar(&NewOhlcvBarRecord {
            observed_at: test_time(observed_day),
            source: "unit-test".to_owned(),
            environment: BrokerEnvironment::Paper,
            market_scope: MarketScope::DomesticStock,
            interval: OhlcvBarInterval::Daily,
            bar_start_at: test_time(day),
            bar_end_at: test_time(end_day),
            adjustment_policy: PriceAdjustmentPolicy::Raw,
            data_quality_flags: vec!["complete".to_owned()],
            bar: daily_bar(day, close, close + 1, close - 1, close, 100),
        })
        .expect("record bar");
    }
    let mut other_symbol = daily_bar(2, 99, 100, 98, 99, 100);
    other_symbol.symbol = Symbol::new("000660").expect("valid symbol");
    db.record_ohlcv_bar(&NewOhlcvBarRecord {
        observed_at: test_time(5),
        source: "unit-test".to_owned(),
        environment: BrokerEnvironment::Paper,
        market_scope: MarketScope::DomesticStock,
        interval: OhlcvBarInterval::Daily,
        bar_start_at: test_time(2),
        bar_end_at: test_time(5),
        adjustment_policy: PriceAdjustmentPolicy::Raw,
        data_quality_flags: vec!["complete".to_owned()],
        bar: other_symbol,
    })
    .expect("record other symbol bar");

    let bars = db
        .ohlcv_bars_for_symbol_between(
            &Symbol::new("005930").expect("valid symbol"),
            &MarketScope::DomesticStock,
            OhlcvBarInterval::Daily,
            TradingDay(Date::from_calendar_date(2026, Month::January, 1).expect("valid date")),
            TradingDay(Date::from_calendar_date(2026, Month::January, 2).expect("valid date")),
        )
        .expect("range bars");

    assert_eq!(
        bars,
        vec![
            daily_bar(1, 10, 11, 9, 10, 100),
            daily_bar(2, 22, 23, 21, 22, 100)
        ]
    );
}

#[test]
fn main_db_computes_and_records_latest_daily_technical_feature_audit_from_stored_bars() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for (day, close, volume) in [(1, 10, 10), (2, 20, 20), (3, 30, 30), (4, 40, 100)] {
        db.record_ohlcv_bar(&NewOhlcvBarRecord {
            observed_at: test_time(day + 1),
            source: "unit-test".to_owned(),
            environment: BrokerEnvironment::Paper,
            market_scope: MarketScope::DomesticStock,
            interval: OhlcvBarInterval::Daily,
            bar_start_at: test_time(day),
            bar_end_at: test_time(day + 1),
            adjustment_policy: PriceAdjustmentPolicy::Raw,
            data_quality_flags: Vec::new(),
            bar: daily_bar(day, close - 1, close + 1, close - 2, close, volume),
        })
        .expect("record bar");
    }

    let feature_id = db
        .compute_and_record_latest_daily_technical_features(&NewTechnicalFeatureAuditComputation {
            observed_at: test_time(5),
            source: "unit-test".to_owned(),
            symbol: Symbol::new("005930").expect("symbol"),
            interval: OhlcvBarInterval::Daily,
            feature_set: "daily-core-v1".to_owned(),
            params: technical_feature_params(),
            lookback_limit: 4,
        })
        .expect("compute feature audit")
        .expect("feature audit id");

    assert_eq!(feature_id.as_i64(), 1);

    let connection = Connection::open(&db_path).expect("inspect db");
    let (ohlcv_bar_id, features_json, inputs_json): (i64, String, String) = connection
        .query_row(
            "SELECT ohlcv_bar_id, features_json, inputs_json
             FROM technical_feature_audits
             WHERE id = ?1",
            [feature_id.as_i64()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("feature audit row");
    let features: serde_json::Value = serde_json::from_str(&features_json).expect("features json");
    let inputs: serde_json::Value = serde_json::from_str(&inputs_json).expect("inputs json");

    assert_eq!(ohlcv_bar_id, 4);
    assert_eq!(features["trading_day"], "2026-01-04");
    assert_eq!(features["sma_short"], "35");
    assert_eq!(features["sma_long"], "30");
    assert_eq!(features["volume_ratio"], "5");
    assert_eq!(features["volume_breakout"], true);
    assert_eq!(inputs["input_bar_ids"], serde_json::json!([1, 2, 3, 4]));
}

#[test]
fn main_db_reads_latest_technical_feature_audits_in_requested_symbol_order() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    let bar_id = db
        .record_ohlcv_bar(&NewOhlcvBarRecord {
            observed_at: test_time(2),
            source: "unit-test".to_owned(),
            environment: BrokerEnvironment::Paper,
            market_scope: MarketScope::DomesticStock,
            interval: OhlcvBarInterval::Daily,
            bar_start_at: test_time(1),
            bar_end_at: test_time(2),
            adjustment_policy: PriceAdjustmentPolicy::Raw,
            data_quality_flags: Vec::new(),
            bar: daily_bar(1, 10, 11, 9, 10, 100),
        })
        .expect("record bar");
    for (symbol, observed_day, feature_set, sma_short) in [
        ("005930", 3, "daily-core-v1", 10),
        ("000660", 4, "daily-core-v1", 20),
        ("005930", 5, "alternate-v1", 30),
        ("005930", 6, "daily-core-v1", 40),
    ] {
        db.record_technical_feature_audit(&NewTechnicalFeatureAuditRecord {
            observed_at: test_time(observed_day),
            source: "unit-test".to_owned(),
            interval: OhlcvBarInterval::Daily,
            ohlcv_bar_id: Some(bar_id),
            feature_set: feature_set.to_owned(),
            features: technical_features(symbol, observed_day, sma_short),
            input_bar_ids: vec![bar_id],
            params: technical_feature_params(),
        })
        .expect("record feature audit");
    }

    let audits = db
        .latest_technical_features_for_symbols(
            &[
                Symbol::new("000660").expect("symbol"),
                Symbol::new("111111").expect("symbol"),
                Symbol::new("005930").expect("symbol"),
            ],
            OhlcvBarInterval::Daily,
            "daily-core-v1",
        )
        .expect("latest feature audits");

    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].symbol.as_str(), "000660");
    assert_eq!(audits[0].features.sma_short, Some(Decimal::new(20, 0)));
    assert_eq!(audits[1].symbol.as_str(), "005930");
    assert_eq!(audits[1].features.sma_short, Some(Decimal::new(40, 0)));
    assert_eq!(audits[1].feature_set, "daily-core-v1");
    assert_eq!(audits[1].input_bar_ids, vec![bar_id]);
    assert_eq!(audits[1].params.volume_breakout_ratio, Decimal::new(5, 0));
}

#[test]
fn execution_status_and_event_enums_match_sqlite_constraints() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");

    let _db = MainDb::open(&db_path).expect("open db");
    let connection = Connection::open(&db_path).expect("inspect db");
    let status_trigger_sql = sqlite_object_sql(
        &connection,
        "trigger",
        "trg_execution_requests_status_insert",
    );
    let execution_events_sql = sqlite_object_sql(&connection, "table", "execution_events");

    for status in ExecutionRequestStatus::ALL {
        assert!(
            status_trigger_sql.contains(&format!("'{}'", status.as_str())),
            "missing execution status {} in trigger SQL: {status_trigger_sql}",
            status.as_str()
        );
    }
    for event_type in ExecutionEventType::ALL {
        assert!(
            execution_events_sql.contains(&format!("'{}'", event_type.as_str())),
            "missing execution event type {} in table SQL: {execution_events_sql}",
            event_type.as_str()
        );
    }
}

#[test]
fn config_loads_required_main_db_path() {
    let tempdir = tempdir().expect("tempdir");
    let config_path = tempdir.path().join("trade-engine.toml");
    fs::write(
        &config_path,
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = ".local/state/trade-engine/live-dry-run-main.sqlite3"

[kis]
environment = "live"
token_cache_path = ".local/state/trade-engine/kis-live-token.json"

[strategy]
id = "sma-crossover-v1"
short_window = 5
long_window = 20

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
enable_live_trading = false
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_order_count = 3
"#,
    )
    .expect("write config");

    let config = EngineConfig::load_from_path(&config_path).expect("load config");

    assert_eq!(
        config.storage.main_db_path.as_deref(),
        Some(std::path::Path::new(
            ".local/state/trade-engine/live-dry-run-main.sqlite3"
        ))
    );
}

#[tokio::test]
async fn dry_run_tick_persists_runtime_portfolio_market_and_decision_snapshots() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let open_order_reference = test_order_reference("open-snapshot");
    let fill_reference = test_order_reference("fill-snapshot");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: Symbol::new("005930").expect("valid symbol"),
            quantity: Quantity::new(10),
            average_price: Decimal::new(70_000, 0),
        })
        .with_open_order(OrderStatus {
            reference: open_order_reference,
            state: OrderState::Accepted,
            evidence: Some(OrderStatusEvidence {
                symbol: Symbol::new("005930").expect("valid symbol"),
                side: Some(OrderSide::Buy),
                ordered_quantity: Quantity::new(1),
                filled_quantity: Quantity::new(0),
                order_price: Some(Decimal::new(73_000, 0)),
                trading_day: Some("20260426".to_owned()),
                observed_at: None,
            }),
        })
        .with_fill(Fill {
            reference: fill_reference,
            quantity: Quantity::new(2),
            price: Decimal::new(72_500, 0),
            evidence: Some(OrderStatusEvidence {
                symbol: Symbol::new("005930").expect("valid symbol"),
                side: Some(OrderSide::Buy),
                ordered_quantity: Quantity::new(2),
                filled_quantity: Quantity::new(2),
                order_price: Some(Decimal::new(72_500, 0)),
                trading_day: Some("20260426".to_owned()),
                observed_at: None,
            }),
        })
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("삼성전자보통주".to_owned()),
            last_price: Decimal::new(73_000, 0),
        })
        .with_quote(Quote {
            symbol: Symbol::new("000660").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(180_000, 0),
        });

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[
            Symbol::new("005930").expect("valid symbol"),
            Symbol::new("000660").expect("valid symbol"),
        ],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
    )
    .await
    .expect("dry-run tick with audit");

    assert!(output.contains("Dry-run snapshot"));
    assert!(output.contains("  account_suffix: 5678"));
    assert_eq!(
        MainDb::open(&db_path)
            .expect("inspect db")
            .audit_counts()
            .expect("counts"),
        AuditCounts {
            runtime_runs: 1,
            runtime_ticks: 1,
            portfolio_snapshots: 1,
            market_snapshots: 2,
            strategy_signals: 2,
            decision_inputs: 2,
            decision_outputs: 2,
            decision_proposals: 2,
            risk_evaluations: 2,
            risk_blocks: 0,
            execution_requests: 0,
            execution_events: 0,
            broker_command_events: 0,
            fills: 0,
        }
    );
    let (open_orders_json, today_fills_json) = portfolio_snapshot_order_jsons(&db_path);
    assert!(open_orders_json.contains("\"symbol\":\"005930\""));
    assert!(open_orders_json.contains("\"state\":\"accepted\""));
    assert!(today_fills_json.contains("\"quantity\":2"));
    assert!(today_fills_json.contains("72500"));
    assert_eq!(
        market_snapshot_full_names(&db_path),
        vec![Some("삼성전자보통주".to_owned()), None]
    );
}

fn table_exists(connection: &Connection, table: &str) -> bool {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |_| Ok(()),
        )
        .is_ok()
}

fn index_exists(connection: &Connection, index: &str) -> bool {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1",
            [index],
            |_| Ok(()),
        )
        .is_ok()
}

fn assert_column_exists(connection: &Connection, table: &str, column: &str) {
    assert!(
        column_exists(connection, table, column),
        "missing column {table}.{column}"
    );
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> bool {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("table info");
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query columns")
        .collect::<Result<Vec<_>, _>>()
        .expect("columns");
    columns.iter().any(|stored| stored == column)
}

fn sqlite_object_sql(connection: &Connection, object_type: &str, name: &str) -> String {
    connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = ?1 AND name = ?2",
            [object_type, name],
            |row| row.get(0),
        )
        .expect("sqlite object SQL")
}

fn portfolio_snapshot_order_jsons(path: &std::path::Path) -> (String, String) {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT open_orders_json, today_fills_json
             FROM portfolio_snapshots
             ORDER BY id
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("portfolio snapshot order JSON")
}

#[test]
fn latest_position_average_price_before_reads_sold_out_cost_basis() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let db = MainDb::open(&db_path).expect("open db");
    let connection = Connection::open(&db_path).expect("inspect db");
    connection
        .execute(
            "INSERT INTO runtime_runs (mode, environment, account_suffix, started_at, status)
             VALUES ('submit_orders', 'live', '4372', '2026-06-22T00:59:00Z', 'started')",
            [],
        )
        .expect("insert run");
    let run_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO runtime_ticks (run_id, mode, started_at, status)
             VALUES (?1, 'submit_orders', '2026-06-22T00:59:00Z', 'started')",
            [run_id],
        )
        .expect("insert tick");
    let tick_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO portfolio_snapshots (
                run_id, tick_id, observed_at, account_suffix, cash, positions_json,
                open_orders_json, today_fills_json
             )
             VALUES (?1, ?2, '2026-06-22T01:01:56Z', '4372', '25.67',
                '[{\"symbol\":\"CMCSA\",\"quantity\":8,\"average_price\":\"25.6312\"}]',
                '[]', '[]')",
            params![run_id, tick_id],
        )
        .expect("insert portfolio snapshot");

    let average_price = db
        .latest_position_average_price_before(
            &Symbol::new("CMCSA").expect("symbol"),
            OffsetDateTime::parse("2026-06-22T01:02:14Z", &Rfc3339).expect("timestamp"),
        )
        .expect("average price");

    assert_eq!(average_price, Some(Decimal::new(25_6312, 4)));
}

fn market_snapshot_full_names(path: &std::path::Path) -> Vec<Option<String>> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT full_name FROM market_snapshots ORDER BY id")
        .expect("market snapshot full names");
    statement
        .query_map([], |row| row.get(0))
        .expect("query market snapshot full names")
        .collect::<Result<Vec<_>, _>>()
        .expect("market snapshot full names")
}

fn daily_bar(day: u8, open: i64, high: i64, low: i64, close: i64, volume: i64) -> DailyOhlcvBar {
    DailyOhlcvBar {
        symbol: Symbol::new("005930").expect("valid symbol"),
        trading_day: TradingDay(
            Date::from_calendar_date(2026, Month::January, day).expect("valid date"),
        ),
        open: Decimal::new(open, 0),
        high: Decimal::new(high, 0),
        low: Decimal::new(low, 0),
        close: Decimal::new(close, 0),
        volume: Decimal::new(volume, 0),
    }
}

fn technical_features(symbol: &str, day: u8, sma_short: i64) -> DailyTechnicalFeatures {
    DailyTechnicalFeatures {
        symbol: Symbol::new(symbol).expect("valid symbol"),
        trading_day: TradingDay(
            Date::from_calendar_date(2026, Month::January, day).expect("valid date"),
        ),
        sma_short: Some(Decimal::new(sma_short, 0)),
        sma_long: None,
        ema_short: None,
        ema_long: None,
        atr: None,
        rsi: None,
        roc: None,
        average_volume: None,
        volume_ratio: None,
        volume_breakout: None,
    }
}

fn test_time(day: u8) -> OffsetDateTime {
    Date::from_calendar_date(2026, Month::January, day)
        .expect("valid date")
        .with_time(Time::MIDNIGHT)
        .assume_utc()
}

fn technical_feature_params() -> TechnicalFeatureParams {
    TechnicalFeatureParams {
        sma_short_window: 2,
        sma_long_window: 3,
        ema_short_window: 2,
        ema_long_window: 3,
        atr_window: 3,
        rsi_window: 3,
        roc_window: 3,
        average_volume_window: 3,
        volume_breakout_ratio: Decimal::new(5, 0),
    }
}

fn test_order_reference(client_order_id: &str) -> OrderReference {
    OrderReference {
        client_order_id: ClientOrderId::new(client_order_id).expect("client order id"),
        broker_order_id: None,
        broker_order_orgno: None,
        exchange_code: None,
        account_key: AccountKey::new("12345678-01").expect("account key"),
        environment: BrokerEnvironment::Paper,
    }
}
