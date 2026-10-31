use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rusqlite::Connection;
use rust_decimal::Decimal;
use tempfile::tempdir;
use time::{Date, Month, Time};

use trade_engine::config::{
    AdvisorConfig, AdvisorProviderKind, DomesticMarketAlignmentConfig, DomesticMarketAlignmentMode,
    RuntimeConfig, StrategyConfig, UniverseDiscoveryConfig, UniverseDiscoveryProviderKind,
};
use trade_engine::gateway::testing::MockTradingGateway;
use trade_engine::gateway::traits::TradingGateway;
use trade_engine::gateway::types::{
    AccountSnapshot, AmendOrderRequest, AmendResult, BrokerHealth, BuyingPower, CancelOrderRequest,
    CancelResult, CashBalance, CreateOrderRequest, CurrencyCode, ExchangeCode, Fill, MarketScope,
    MarketSession, OrderStatus, OrderSubmission, Position, Quote, Symbol, TradingDay,
};
use trade_engine::runtime::{
    DryRunRunContext, DryRunRuntimeSettings, RuntimeLoopControl, RuntimeLoopOutputOptions,
    RuntimeMarketHours, RuntimeOutputSink, RuntimeShutdownToken, UniverseRefreshContext,
    refresh_universe_once, run_dry_run_loop, run_dry_run_loop_with_output,
    run_dry_run_loop_with_shutdown, run_dry_run_tick,
};
use trade_engine::storage::{
    AuditCounts, BlockingMainDb, MainDb, NewOhlcvBarRecord, NewTechnicalFeatureAuditComputation,
};
use trade_engine::technical_analysis::{
    DailyOhlcvBar, OhlcvBarInterval, PriceAdjustmentPolicy, TechnicalFeatureParams,
};
use trade_engine::universe::{
    UniverseDiscoveryBucket, UniverseDiscoveryCandidate, UniverseDiscoveryEvidence,
    UniverseDiscoveryOutput, UniverseDiscoveryRiskFlag, UniverseDiscoverySignal,
};
use trading_agent_kis_client::{BrokerEnvironment, BrokerError};

#[test]
fn runtime_config_uses_requested_tick_defaults() {
    let config = RuntimeConfig::default();

    assert_eq!(config.market_tick_seconds, 30);
    assert_eq!(config.off_hours_market_tick_seconds, 900);
    assert_eq!(config.max_tick_runtime_seconds, 10);
    assert!(config.skip_if_previous_tick_running);
    assert_eq!(config.market_session_ttl_seconds, 300);
}

#[tokio::test]
async fn dry_run_loop_runs_market_tick_and_disabled_advisor_once() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("Dry-run loop"));
    assert!(output.contains("  status: started"));
    assert!(output.contains("  market_interval_seconds: 30"));
    assert!(output.contains("Market tick #1\n  status: started"));
    assert!(output.contains("Tick advisory #1\n  status: parsed\n  provider: deterministic_sma"));
    assert!(output.contains("  reason: max_market_ticks"));
    assert_eq!(
        MainDb::open(&db_path)
            .expect("inspect db")
            .audit_counts()
            .expect("counts"),
        AuditCounts {
            runtime_runs: 1,
            runtime_ticks: 1,
            portfolio_snapshots: 1,
            market_snapshots: 1,
            strategy_signals: 1,
            decision_inputs: 1,
            decision_outputs: 1,
            decision_proposals: 1,
            risk_evaluations: 1,
            risk_blocks: 0,
            execution_requests: 0,
            execution_events: 0,
            broker_command_events: 0,
            fills: 0,
        }
    );
    assert_eq!(runtime_run_environments(&db_path), vec!["paper"]);
}

#[tokio::test]
async fn dry_run_loop_streams_output_chunks_as_they_are_rendered() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });
    let mut output_sink = CollectingOutputSink::default();

    let output = run_dry_run_loop_with_output(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
        RuntimeLoopOutputOptions {
            shutdown: RuntimeShutdownToken::new(),
            output_sink: &mut output_sink,
        },
    )
    .await
    .expect("streamed dry-run loop");

    assert_eq!(output_sink.chunks.concat(), output);
    assert!(
        output_sink
            .chunks
            .first()
            .expect("first streamed chunk")
            .contains("Dry-run loop\n  status: started")
    );
    assert!(
        output_sink
            .chunks
            .iter()
            .any(|chunk| chunk.contains("Market tick #1\n  status: started"))
    );
    assert!(
        output_sink
            .chunks
            .iter()
            .any(|chunk| chunk.contains("Tick advisory #1"))
    );
    assert!(
        output_sink
            .chunks
            .last()
            .expect("last streamed chunk")
            .contains("reason: max_market_ticks")
    );
}

#[tokio::test]
async fn streaming_loop_does_not_collect_output() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });
    let mut output_sink = CollectingOutputSink::default();

    let output = trade_engine::runtime::stream_dry_run_loop_with_output(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
        RuntimeLoopOutputOptions {
            shutdown: RuntimeShutdownToken::new(),
            output_sink: &mut output_sink,
        },
    )
    .await
    .expect("streamed dry-run loop");

    assert!(output.is_empty());
    assert!(
        output_sink
            .chunks
            .first()
            .expect("first streamed chunk")
            .contains("Dry-run loop\n  status: started")
    );
    assert!(
        output_sink
            .chunks
            .iter()
            .any(|chunk| chunk.contains("Market tick #1\n  status: started"))
    );
    assert!(
        output_sink
            .chunks
            .iter()
            .any(|chunk| chunk.contains("Tick advisory #1"))
    );
    assert!(
        output_sink
            .chunks
            .last()
            .expect("last streamed chunk")
            .contains("reason: max_market_ticks")
    );
}

#[tokio::test]
async fn dry_run_loop_runs_command_advisor_and_records_audit() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("advisor-request.json");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"advisor reviewed latest tick","observations":["one quote"],"risks":["read-only"],"next_actions":["no action"],"rationale":"latest KIS API quote supports a hold proposal","evidence":[{{"source":"kis_api","detail":"quote for 005930 was present"}}],"proposals":[{{"decision_type":"hold","symbol":"005930","quantity":null,"order_type":null,"limit_price":null,"confidence":"0.50","time_horizon":null,"reason":"advisor hold","risk_notes":[]}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write advisor script");
    let mut db = MainDb::open(&db_path).expect("open db");
    seed_daily_feature_audit(&mut db);
    drop(db);
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("삼성전자보통주".to_owned()),
            last_price: Decimal::new(73_000, 0),
        });
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_advisor(AdvisorConfig {
            provider: AdvisorProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            timeout_seconds: 5,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
            ..AdvisorConfig::default()
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("Tick advisory #1\n  status: parsed\n  provider: command"));
    assert!(
        output.contains(
            "  audit: operator_actions action=tick_advisory_completed target=runtime_run:"
        )
    );
    assert!(output.contains("  summary: advisor reviewed latest tick"));
    assert!(output.contains("  risks: read-only"));
    assert!(output.contains("  next_actions: no action"));
    assert!(output.contains("Technical analysis"));
    assert!(
        output.contains(
            "  - 005930 interval=daily feature_set=daily-core-v1 sma_short=73500 volume_breakout=true"
        ),
        "output should surface audited technical evidence: {output}"
    );
    let request_json = fs::read_to_string(request_path).expect("advisor request");
    let request: serde_json::Value =
        serde_json::from_str(&request_json).expect("advisor request json");
    assert_eq!(request["schema_version"], 8);
    assert_eq!(request["runtime"]["mode"], "dry_run");
    assert_eq!(request["runtime"]["account_suffix"], "5678");
    let latest_tick = &request["current_state"]["current_tick"];
    assert_eq!(latest_tick["stage"], "TickObservation");
    assert_eq!(latest_tick["market_session_open"], true);
    assert_eq!(latest_tick["kis_api"]["account"]["cash"], "1000000");
    assert_eq!(latest_tick["kis_api"]["quotes"][0]["symbol"], "005930");
    assert_eq!(
        latest_tick["kis_api"]["quotes"][0]["full_name"],
        "삼성전자보통주"
    );
    assert_eq!(
        latest_tick["kis_api"]["quotes"][0]["recent_prices"][0],
        "73000"
    );
    assert_eq!(latest_tick["technical_analysis"][0]["symbol"], "005930");
    assert_eq!(latest_tick["technical_analysis"][0]["interval"], "daily");
    assert_eq!(
        latest_tick["technical_analysis"][0]["feature_set"],
        "daily-core-v1"
    );
    assert_eq!(
        latest_tick["technical_analysis"][0]["features"]["sma_short"],
        "73500"
    );
    assert_eq!(
        latest_tick["technical_analysis"][0]["features"]["volume_breakout"],
        true
    );
    assert!(!request_json.contains("12345678-01"));
    assert_eq!(
        advisor_action_types(&db_path),
        vec!["tick_advisory_completed"]
    );
    let metadata: serde_json::Value =
        serde_json::from_str(&advisor_metadata(&db_path)).expect("advisor metadata json");
    assert_eq!(
        metadata["output"]["summary"],
        "advisor reviewed latest tick"
    );
    assert_eq!(
        metadata["output"]["rationale"],
        "latest KIS API quote supports a hold proposal"
    );
    assert_eq!(metadata["output"]["evidence"][0]["source"], "kis_api");
    assert_eq!(metadata["output"]["proposals"][0]["decision_type"], "hold");
}

#[tokio::test]
async fn command_advisor_receives_domestic_market_alignment_context_when_enabled() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("advisor-request.json");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"advisor reviewed alignment","observations":["alignment present"],"risks":[],"next_actions":["hold"],"rationale":"alignment context was available","evidence":[{{"source":"domestic_market_alignment","detail":"context present"}}],"proposals":[{{"decision_type":"hold","symbol":"005930","quantity":null,"order_type":null,"limit_price":null,"confidence":"0.50","time_horizon":null,"reason":"advisor hold","risk_notes":[]}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write advisor script");

    let mut db = MainDb::open(&db_path).expect("open db");
    seed_daily_feature_audit_for_symbol(&mut db, "KOSPI", [1000, 1010, 1030, 1060]);
    seed_daily_feature_audit_for_symbol(&mut db, "005930", [1000, 1020, 1040, 1100]);
    drop(db);

    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("Samsung Electronics".to_owned()),
            last_price: Decimal::new(110, 0),
        });
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_domestic_market_alignment(DomesticMarketAlignmentConfig {
            enabled: true,
            mode: DomesticMarketAlignmentMode::AdvisorContext,
            benchmark_symbol: Some("KOSPI".to_owned()),
            feature_set: "daily-core-v1".to_owned(),
            relative_strength_lookback_days: 20,
        })
        .with_advisor(AdvisorConfig {
            provider: AdvisorProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            timeout_seconds: 5,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
            ..AdvisorConfig::default()
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("Domestic market alignment"));
    assert!(output.contains("  benchmark_regime: strong_uptrend"));
    assert!(output.contains("005930 label=aligned"));

    let request_json = fs::read_to_string(request_path).expect("advisor request");
    let request: serde_json::Value =
        serde_json::from_str(&request_json).expect("advisor request json");
    assert_eq!(request["schema_version"], 8);
    let alignment = &request["current_state"]["current_tick"]["domestic_market_alignment"];
    assert_eq!(alignment["status"], "computed");
    assert_eq!(alignment["mode"], "advisor_context");
    assert_eq!(alignment["benchmark_symbol"], "KOSPI");
    assert_eq!(alignment["benchmark_regime"], "strong_uptrend");
    assert_eq!(alignment["candidates"][0]["symbol"], "005930");
    assert_eq!(alignment["candidates"][0]["alignment_label"], "aligned");
    assert_eq!(alignment["candidates"][0]["benchmark_roc"], "6.00");
    assert_eq!(alignment["candidates"][0]["symbol_roc"], "10.00");

    let detail: serde_json::Value =
        serde_json::from_str(&first_tick_detail_json(&db_path)).expect("tick detail json");
    assert_eq!(
        detail["domestic_market_alignment"]["benchmark_regime"],
        "strong_uptrend"
    );
    assert_eq!(
        detail["domestic_market_alignment"]["candidates"][0]["alignment_label"],
        "aligned"
    );
}

#[tokio::test]
async fn dry_run_loop_throttles_command_advisor_between_fast_ticks() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("advisor-request.json");
    let call_log_path = tempdir.path().join("advisor-calls.log");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        format!(
            r#"printf 'called\n' >> "{}"
while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"advisor reviewed latest tick","observations":["one quote"],"risks":[],"next_actions":["continue"],"rationale":"first tick command advisor call","evidence":[{{"source":"runtime","detail":"initial command advisor invocation"}}],"proposals":[{{"decision_type":"hold","symbol":"005930","quantity":null,"order_type":null,"limit_price":null,"confidence":"0.50","time_horizon":null,"reason":"advisor hold","risk_notes":[]}}]}}'
"#,
            call_log_path.display(),
            request_path.display()
        ),
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("Samsung Electronics".to_owned()),
            last_price: Decimal::new(73_000, 0),
        });
    let runtime = RuntimeConfig {
        market_tick_seconds: 1,
        ..RuntimeConfig::default()
    };
    let settings = DryRunRuntimeSettings::new(
        runtime,
        StrategyConfig {
            id: "hold-only".to_owned(),
            short_window: None,
            long_window: None,
        },
    )
    .with_advisor(AdvisorConfig {
        provider: AdvisorProviderKind::Command,
        command: Some(PathBuf::from("sh")),
        args: vec![script_path.to_string_lossy().into_owned()],
        timeout_seconds: 5,
        max_stderr_bytes: 4096,
        max_stdout_bytes: 262_144,
        min_command_interval_seconds: 300,
        price_change_threshold_bps: 50,
        ..AdvisorConfig::default()
    });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(2),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("  market_interval_seconds: 1"));
    assert!(output.contains("Tick advisory #1\n  status: parsed\n  provider: command"));
    assert!(output.contains("Tick advisory #2\n  status: parsed\n  provider: advisor_policy_skip"));
    assert!(
        output.contains("  summary: advisor command skipped by invocation policy"),
        "second tick should record an explicit policy skip advisory"
    );
    assert_eq!(
        fs::read_to_string(call_log_path)
            .expect("advisor call log")
            .lines()
            .count(),
        1
    );
    assert_eq!(gateway.call_counts().quotes, 2);
    assert_eq!(
        advisor_action_types(&db_path),
        vec!["tick_advisory_completed", "tick_advisory_completed"]
    );
}

#[tokio::test]
async fn command_advisor_receives_recent_prices_with_hold_only_strategy() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("advisor-request.json");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"advisor reviewed recent prices","observations":["history present"],"risks":[],"next_actions":["continue"],"rationale":"recent price history is present","evidence":[{{"source":"runtime","detail":"recent_prices include prior tick"}}],"proposals":[{{"decision_type":"hold","symbol":"005930","quantity":null,"order_type":null,"limit_price":null,"confidence":"0.50","time_horizon":null,"reason":"history hold","risk_notes":[]}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("Samsung Electronics".to_owned()),
            last_price: Decimal::new(73_000, 0),
        });
    let runtime = RuntimeConfig {
        market_tick_seconds: 2,
        ..RuntimeConfig::default()
    };
    let settings = DryRunRuntimeSettings::new(
        runtime,
        StrategyConfig {
            id: "hold-only".to_owned(),
            short_window: None,
            long_window: None,
        },
    )
    .with_advisor(AdvisorConfig {
        provider: AdvisorProviderKind::Command,
        command: Some(PathBuf::from("sh")),
        args: vec![script_path.to_string_lossy().into_owned()],
        timeout_seconds: 5,
        max_stderr_bytes: 4096,
        max_stdout_bytes: 262_144,
        min_command_interval_seconds: 1,
        ..AdvisorConfig::default()
    });

    run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(2),
        },
    )
    .await
    .expect("dry-run loop");

    let request_json = fs::read_to_string(request_path).expect("advisor request");
    let request: serde_json::Value =
        serde_json::from_str(&request_json).expect("advisor request json");
    assert_eq!(
        request["current_state"]["current_tick"]["kis_api"]["quotes"][0]["recent_prices"],
        serde_json::json!(["73000", "73000"])
    );
}

#[tokio::test]
async fn dry_run_loop_uses_discovered_universe_for_quotes_and_advisor_decisions() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let advisor_request_path = tempdir.path().join("advisor-request.json");
    let advisor_script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &advisor_script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"advisor reviewed expanded universe","observations":["two quotes"],"risks":["read-only"],"next_actions":["risk gate candidate"],"rationale":"discovered quote is available in the tick input","evidence":[{{"source":"kis_api","detail":"quote for 000660 was present"}}],"proposals":[{{"decision_type":"buy","symbol":"000660","quantity":1,"order_type":"limit","limit_price":"120000","confidence":"0.62","time_horizon":"intraday","reason":"advisor candidate buy","risk_notes":[]}}]}}'
"#,
            advisor_request_path.display()
        ),
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    MainDb::open(&db_path)
        .expect("open db for universe snapshot")
        .record_universe_snapshot(
            BrokerEnvironment::Paper,
            "5678",
            "command",
            &UniverseDiscoveryOutput {
                summary: "added memory candidate".to_owned(),
                candidates: vec![UniverseDiscoveryCandidate {
                    symbol: Symbol::new("000660").expect("valid symbol"),
                    bucket: Some(UniverseDiscoveryBucket::ShortTermMomentum),
                    target_support: None,
                    signals: vec![
                        UniverseDiscoverySignal::VolumeBreakout,
                        UniverseDiscoverySignal::RelativeStrength,
                    ],
                    evidence: vec![UniverseDiscoveryEvidence {
                        metric: "20d volume percentile".to_owned(),
                        value: "top quartile".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: Some("universe snapshot".to_owned()),
                    }],
                    risk_flags: vec![UniverseDiscoveryRiskFlag::UnverifiedSource],
                    reason: "same-sector candidate for this tick".to_owned(),
                    confidence: Decimal::new(62, 2),
                }],
            },
            None,
            Duration::from_secs(1800),
        )
        .expect("record universe snapshot");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("Samsung Electronics".to_owned()),
            last_price: Decimal::new(73_000, 0),
        })
        .with_quote(Quote {
            symbol: Symbol::new("000660").expect("valid symbol"),
            full_name: Some("SK hynix".to_owned()),
            last_price: Decimal::new(120_000, 0),
        });
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_universe_discovery(UniverseDiscoveryConfig {
            provider: UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec!["unused-universe-command.sh".to_owned()],
            max_symbols: 20,
            timeout_seconds: 5,
            refresh_seconds: 1800,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        })
        .with_advisor(AdvisorConfig {
            provider: AdvisorProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![advisor_script_path.to_string_lossy().into_owned()],
            timeout_seconds: 5,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
            ..AdvisorConfig::default()
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("Universe discovery #1\n  status: parsed\n  provider: cache"));
    assert!(output.contains("  audit: universe_snapshots id="));
    assert!(output.contains("  user_watchlist: 1"));
    assert!(output.contains("  discovered: 1"));
    assert!(output.contains("  effective_candidates: 2"));
    assert!(output.contains("  quotes: 2"));
    assert!(output.contains("- 000660"));
    assert!(output.contains("decision=buy"));
    assert_eq!(gateway.call_counts().quotes, 2);
    let advisor_request_json = fs::read_to_string(advisor_request_path).expect("advisor request");
    let advisor_request: serde_json::Value =
        serde_json::from_str(&advisor_request_json).expect("advisor request json");
    assert_eq!(advisor_request["schema_version"], 8);
    assert!(advisor_request["runtime"].get("watchlist").is_none());
    assert_eq!(
        advisor_request["runtime"]["market"],
        serde_json::json!({
            "kind": "domestic_stock"
        })
    );
    assert_eq!(advisor_request["runtime"]["user_watchlist"][0], "005930");
    assert_eq!(
        advisor_request["runtime"]["discovered_universe"][0],
        "000660"
    );
    assert_eq!(
        advisor_request["runtime"]["effective_candidates"],
        serde_json::json!(["005930", "000660"])
    );
    assert!(
        advisor_request["runtime"]["universe_discovery"]
            .get("discovered_symbols")
            .is_none()
    );
    assert!(
        advisor_request["runtime"]["universe_discovery"]
            .get("user_watchlist")
            .is_none()
    );
    assert!(
        advisor_request["runtime"]["universe_discovery"]
            .get("effective_candidates")
            .is_none()
    );
    assert_eq!(
        advisor_request["runtime"]["universe_discovery"]["discovered_candidates"][0]["bucket"],
        "short_term_momentum"
    );
    assert_eq!(
        advisor_request["runtime"]["universe_discovery"]["discovered_candidates"][0]["signals"],
        serde_json::json!(["volume_breakout", "relative_strength"])
    );
    assert_eq!(
        advisor_request["runtime"]["universe_discovery"]["discovered_candidates"][0]["evidence"][0]
            ["metric"],
        "20d volume percentile"
    );
    assert_eq!(
        advisor_request["runtime"]["universe_discovery"]["discovered_candidates"][0]["risk_flags"],
        serde_json::json!(["unverified_source"])
    );
    let universe_source = &advisor_request["runtime"]["universe_discovery"]["source"];
    assert_eq!(universe_source["provider"], "cache");
    assert_eq!(universe_source["status"], "parsed");
    assert_eq!(universe_source["snapshot_provider"], "command");
    assert_eq!(universe_source["summary"], "added memory candidate");
    assert!(universe_source["snapshot_id"].is_number());
    assert!(universe_source["observed_at"].is_string());
    assert!(universe_source["expires_at"].is_string());
    assert_eq!(
        advisor_request["current_state"]["current_tick"]["kis_api"]["quotes"][1]["symbol"],
        "000660"
    );
    assert_eq!(
        advisor_action_types(&db_path),
        vec!["tick_advisory_completed"]
    );
}

#[tokio::test]
async fn command_advisor_receives_prefiltered_candidate_context() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let advisor_request_path = tempdir.path().join("advisor-request.json");
    let advisor_script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &advisor_script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"advisor reviewed filtered candidates","observations":["filtered quote set"],"risks":[],"next_actions":["continue"],"rationale":"prefilter kept strongest candidate","evidence":[{{"source":"runtime","detail":"candidate limit applied"}}],"proposals":[{{"decision_type":"hold","symbol":"005930","quantity":null,"order_type":null,"limit_price":null,"confidence":"0.50","time_horizon":null,"reason":"filtered hold","risk_notes":[]}}]}}'
"#,
            advisor_request_path.display()
        ),
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    MainDb::open(&db_path)
        .expect("open db for universe snapshot")
        .record_universe_snapshot(
            BrokerEnvironment::Paper,
            "5678",
            "command",
            &UniverseDiscoveryOutput {
                summary: "ranked candidates".to_owned(),
                candidates: vec![
                    test_universe_candidate("000660", Decimal::new(50, 2), Vec::new()),
                    test_universe_candidate("035420", Decimal::new(90, 2), Vec::new()),
                    test_universe_candidate(
                        "068270",
                        Decimal::new(80, 2),
                        vec![UniverseDiscoveryRiskFlag::UnverifiedSource],
                    ),
                ],
            },
            None,
            Duration::from_secs(1800),
        )
        .expect("record universe snapshot");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("Samsung Electronics".to_owned()),
            last_price: Decimal::new(73_000, 0),
        })
        .with_quote(Quote {
            symbol: Symbol::new("000660").expect("valid symbol"),
            full_name: Some("SK hynix".to_owned()),
            last_price: Decimal::new(120_000, 0),
        })
        .with_quote(Quote {
            symbol: Symbol::new("035420").expect("valid symbol"),
            full_name: Some("NAVER".to_owned()),
            last_price: Decimal::new(180_000, 0),
        })
        .with_quote(Quote {
            symbol: Symbol::new("068270").expect("valid symbol"),
            full_name: Some("Celltrion".to_owned()),
            last_price: Decimal::new(170_000, 0),
        });
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_universe_discovery(UniverseDiscoveryConfig {
            provider: UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec!["unused-universe-command.sh".to_owned()],
            max_symbols: 20,
            timeout_seconds: 5,
            refresh_seconds: 1800,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        })
        .with_advisor(AdvisorConfig {
            provider: AdvisorProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![advisor_script_path.to_string_lossy().into_owned()],
            timeout_seconds: 5,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
            candidate_limit: 2,
            ..AdvisorConfig::default()
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("  effective_candidates: 4"));
    assert!(output.contains("  quotes: 4"));
    assert_eq!(gateway.call_counts().quotes, 4);
    let advisor_request_json = fs::read_to_string(advisor_request_path).expect("advisor request");
    let advisor_request: serde_json::Value =
        serde_json::from_str(&advisor_request_json).expect("advisor request json");
    assert_eq!(
        advisor_request["runtime"]["effective_candidates"],
        serde_json::json!(["005930", "035420"])
    );
    assert_eq!(
        advisor_request["runtime"]["discovered_universe"],
        serde_json::json!(["035420"])
    );
    assert_eq!(
        advisor_request["runtime"]["universe_discovery"]["discovered_candidates"][0]["symbol"],
        "035420"
    );
    assert_eq!(
        advisor_request["current_state"]["current_tick"]["kis_api"]["quote_count"],
        2
    );
    assert_eq!(
        advisor_request["current_state"]["current_tick"]["kis_api"]["quotes"]
            .as_array()
            .expect("quotes")
            .iter()
            .map(|quote| quote["symbol"].as_str().expect("symbol"))
            .collect::<Vec<_>>(),
        vec!["005930", "035420"]
    );
}

#[tokio::test]
async fn dry_run_loop_skips_unquoted_discovered_universe_candidate() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    MainDb::open(&db_path)
        .expect("open db for universe snapshot")
        .record_universe_snapshot(
            BrokerEnvironment::Paper,
            "5678",
            "command",
            &UniverseDiscoveryOutput {
                summary: "added overseas candidates".to_owned(),
                candidates: vec![
                    UniverseDiscoveryCandidate {
                        symbol: Symbol::new("XOM").expect("valid symbol"),
                        bucket: Some(UniverseDiscoveryBucket::WatchlistPeer),
                        target_support: None,
                        signals: vec![UniverseDiscoverySignal::WatchlistPeer],
                        evidence: vec![UniverseDiscoveryEvidence {
                            metric: "listing".to_owned(),
                            value: "not quoteable through configured exchange".to_owned(),
                            as_of: Some("latest available tick".to_owned()),
                            source: Some("universe snapshot".to_owned()),
                        }],
                        risk_flags: vec![UniverseDiscoveryRiskFlag::DataUnavailable],
                        reason: "candidate should be skipped when quote is unavailable".to_owned(),
                        confidence: Decimal::new(50, 2),
                    },
                    UniverseDiscoveryCandidate {
                        symbol: Symbol::new("NVDA").expect("valid symbol"),
                        bucket: Some(UniverseDiscoveryBucket::Liquidity),
                        target_support: None,
                        signals: vec![UniverseDiscoverySignal::TradingValueLeader],
                        evidence: vec![UniverseDiscoveryEvidence {
                            metric: "liquidity".to_owned(),
                            value: "quoteable NASDAQ candidate".to_owned(),
                            as_of: Some("latest available tick".to_owned()),
                            source: Some("universe snapshot".to_owned()),
                        }],
                        risk_flags: Vec::new(),
                        reason: "candidate remains because quote is available".to_owned(),
                        confidence: Decimal::new(72, 2),
                    },
                ],
            },
            None,
            Duration::from_secs(1800),
        )
        .expect("record universe snapshot");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("AAPL").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(26711, 2),
        })
        .with_quote(Quote {
            symbol: Symbol::new("NVDA").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(20892, 2),
        });
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_universe_discovery(UniverseDiscoveryConfig {
            provider: UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("unused-universe-command.sh")),
            args: Vec::new(),
            max_symbols: 20,
            timeout_seconds: 5,
            refresh_seconds: 1800,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper).with_market_scope(
            MarketScope::OverseasStock {
                exchange: ExchangeCode::new("NASD").expect("exchange"),
                currency: CurrencyCode::new("USD").expect("currency"),
            },
        ),
        &[Symbol::new("AAPL").expect("valid symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("Universe discovery #1\n  status: parsed\n  provider: cache"));
    assert!(output.contains("  discovered: 1"));
    assert!(output.contains("  effective_candidates: 2"));
    assert!(output.contains("  quotes: 2"));
    assert!(output.contains("- NVDA"));
    assert!(!output.contains("- XOM"));
    assert_eq!(gateway.call_counts().quotes, 3);
}

#[tokio::test]
async fn dry_run_loop_falls_back_to_watchlist_when_universe_discovery_fails() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let advisor_request_path = tempdir.path().join("advisor-request.json");
    let universe_script_path = tempdir.path().join("universe.sh");
    let advisor_script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &universe_script_path,
        r#"printf '%s\n' 'provider failed' >&2
exit 2
"#,
    )
    .expect("write failing universe script");
    fs::write(
        &advisor_script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"advisor reviewed watchlist fallback","observations":["one quote"],"risks":["discovery failed"],"next_actions":["continue watchlist"],"rationale":"fallback still has watchlist quote","evidence":[{{"source":"kis_api","detail":"watchlist quote was present"}}],"proposals":[{{"decision_type":"hold","symbol":"005930","quantity":null,"order_type":null,"limit_price":null,"confidence":"0.50","time_horizon":null,"reason":"fallback hold","risk_notes":[]}}]}}'
"#,
            advisor_request_path.display()
        ),
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("Samsung Electronics".to_owned()),
            last_price: Decimal::new(73_000, 0),
        });
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_universe_discovery(UniverseDiscoveryConfig {
            provider: UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![universe_script_path.to_string_lossy().into_owned()],
            max_symbols: 20,
            timeout_seconds: 5,
            refresh_seconds: 1800,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        })
        .with_advisor(AdvisorConfig {
            provider: AdvisorProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![advisor_script_path.to_string_lossy().into_owned()],
            timeout_seconds: 5,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
            ..AdvisorConfig::default()
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop falls back to watchlist");

    assert!(output.contains("Universe discovery #1\n  status: failed\n  provider: cache"));
    assert!(output.contains("no valid cached universe snapshot; using user watchlist"));
    assert!(output.contains("  discovered: 0"));
    assert!(output.contains("  effective_candidates: 1"));
    assert!(output.contains("  quotes: 1"));
    assert!(output.contains("Tick advisory #1\n  status: parsed\n  provider: command"));
    assert_eq!(gateway.call_counts().quotes, 1);
    let advisor_request_json = fs::read_to_string(advisor_request_path).expect("advisor request");
    let advisor_request: serde_json::Value =
        serde_json::from_str(&advisor_request_json).expect("advisor request json");
    assert_eq!(advisor_request["runtime"]["user_watchlist"][0], "005930");
    assert_eq!(
        advisor_request["runtime"]["discovered_universe"],
        serde_json::json!([])
    );
    assert_eq!(
        advisor_request["runtime"]["effective_candidates"],
        serde_json::json!(["005930"])
    );
    assert_eq!(
        advisor_request["runtime"]["universe_discovery"]["source"]["provider"],
        "cache"
    );
    assert_eq!(
        advisor_request["runtime"]["universe_discovery"]["source"]["status"],
        "failed"
    );
    assert_eq!(
        advisor_request["runtime"]["universe_discovery"]["source"]["error"],
        "no valid cached universe snapshot; using user watchlist"
    );
    assert_eq!(
        advisor_action_types(&db_path),
        vec!["tick_advisory_completed"]
    );
}

#[tokio::test]
async fn universe_refresh_command_provider_stores_snapshot_for_runtime_cache() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("universe-request.json");
    let script_path = tempdir.path().join("universe.sh");
    fs::write(
        &script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"refresh added candidate","candidates":[{{"symbol":"000660","bucket":"liquidity","signals":["trading_value_leader"],"evidence":[{{"metric":"trading value rank","value":"large-cap liquidity bucket","as_of":"latest available tick","source":"runtime discovery context"}}],"risk_flags":[],"reason":"cached candidate","confidence":"0.62"}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write universe script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_universe_discovery(UniverseDiscoveryConfig {
            provider: UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            max_symbols: 20,
            timeout_seconds: 5,
            refresh_seconds: 1800,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });

    let output = refresh_universe_once(
        &store,
        &settings,
        &UniverseRefreshContext {
            environment: BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: MarketScope::DomesticStock,
        },
        &[Symbol::new("005930").expect("valid symbol")],
    )
    .await
    .expect("refresh universe");

    assert_eq!(output.status.as_str(), "parsed");
    assert!(output.snapshot_id.is_some());
    assert!(
        output
            .render()
            .contains("Universe refresh\n  status: parsed")
    );
    assert!(
        output
            .render()
            .contains("  - 000660 bucket=liquidity confidence=0.62 reason=cached candidate")
    );
    let request_json = fs::read_to_string(request_path).expect("universe request");
    let request: serde_json::Value =
        serde_json::from_str(&request_json).expect("universe request json");
    assert_eq!(request["runtime"]["mode"], "universe_refresh");
    let snapshot = MainDb::open(&db_path)
        .expect("open db for latest universe")
        .latest_universe_snapshot(BrokerEnvironment::Paper, "5678")
        .expect("latest universe")
        .expect("stored universe snapshot");
    assert_eq!(
        snapshot.symbols,
        vec![Symbol::new("000660").expect("valid symbol")]
    );
}

#[tokio::test]
async fn overseas_universe_refresh_records_only_overseas_candidates() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("universe-request.json");
    let script_path = tempdir.path().join("universe.sh");
    fs::write(
        &script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"mixed market candidates","candidates":[{{"symbol":"AAPL","bucket":"liquidity","signals":["trading_value_leader"],"evidence":[{{"metric":"liquidity","value":"mega-cap US listing","as_of":"latest available tick","source":"runtime discovery context"}}],"risk_flags":[],"reason":"US large-cap candidate","confidence":"0.64"}},{{"symbol":"005930","bucket":"liquidity","signals":["trading_value_leader"],"evidence":[{{"metric":"market","value":"KRX domestic listing","as_of":"latest available tick","source":"runtime discovery context"}}],"risk_flags":[],"reason":"wrong market candidate","confidence":"0.60"}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write universe script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_universe_discovery(UniverseDiscoveryConfig {
            provider: UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            max_symbols: 20,
            timeout_seconds: 5,
            refresh_seconds: 1800,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });

    let output = refresh_universe_once(
        &store,
        &settings,
        &UniverseRefreshContext {
            environment: BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: MarketScope::OverseasStock {
                exchange: ExchangeCode::new("NASD").expect("exchange"),
                currency: CurrencyCode::new("USD").expect("currency"),
            },
        },
        &[Symbol::new("MSFT").expect("valid symbol")],
    )
    .await
    .expect("refresh universe");

    assert_eq!(output.status.as_str(), "parsed");
    assert_eq!(
        output
            .output
            .as_ref()
            .expect("output")
            .candidates
            .iter()
            .map(|candidate| candidate.symbol.as_str())
            .collect::<Vec<_>>(),
        vec!["AAPL"]
    );
    let request_json = fs::read_to_string(request_path).expect("universe request");
    let request: serde_json::Value =
        serde_json::from_str(&request_json).expect("universe request json");
    assert_eq!(request["runtime"]["market"]["kind"], "overseas_stock");
    assert_eq!(request["runtime"]["market"]["exchange_code"], "NASD");
    assert_eq!(request["runtime"]["market"]["currency"], "USD");
    let snapshot = MainDb::open(&db_path)
        .expect("open db for latest universe")
        .latest_universe_snapshot(BrokerEnvironment::Paper, "5678")
        .expect("latest universe")
        .expect("stored universe snapshot");
    assert_eq!(snapshot.symbols, vec![Symbol::new("AAPL").expect("symbol")]);
}

#[tokio::test]
async fn dry_run_loop_records_multiple_market_ticks_under_one_runtime_run() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });
    let runtime = RuntimeConfig {
        market_tick_seconds: 1,
        ..RuntimeConfig::default()
    };

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Live),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(runtime, StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(2),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("Market tick #2\n  status: completed"));
    assert_eq!(
        MainDb::open(&db_path)
            .expect("inspect db")
            .audit_counts()
            .expect("counts"),
        AuditCounts {
            runtime_runs: 1,
            runtime_ticks: 2,
            portfolio_snapshots: 2,
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
    assert_eq!(runtime_run_statuses(&db_path), vec!["completed"]);
    assert_eq!(runtime_run_environments(&db_path), vec!["live"]);
    assert_eq!(
        runtime_tick_statuses(&db_path),
        vec!["completed", "completed"]
    );
    assert_eq!(runtime_tick_sequences(&db_path), vec![0, 1]);
}

#[tokio::test]
async fn dry_run_loop_records_snapshot_failed_tick_when_quote_collection_fails() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new().with_cash(Decimal::new(1_000_000, 0));

    let error = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect_err("missing quote should fail the tick");

    assert!(error.to_string().contains("quote"));
    assert_eq!(
        MainDb::open(&db_path)
            .expect("inspect db")
            .audit_counts()
            .expect("counts"),
        AuditCounts {
            runtime_runs: 1,
            runtime_ticks: 1,
            portfolio_snapshots: 0,
            market_snapshots: 0,
            strategy_signals: 0,
            decision_inputs: 0,
            decision_outputs: 0,
            decision_proposals: 0,
            risk_evaluations: 0,
            risk_blocks: 0,
            execution_requests: 0,
            execution_events: 0,
            broker_command_events: 0,
            fills: 0,
        }
    );
    assert_eq!(runtime_run_statuses(&db_path), vec!["failed"]);
    assert_eq!(runtime_tick_statuses(&db_path), vec!["snapshot_failed"]);

    let detail = first_tick_detail_json(&db_path);
    assert!(detail.contains("\"stage\":\"TickObservation.quote\""));
    assert!(detail.contains("\"symbol\":\"005930\""));
}

#[tokio::test]
async fn dry_run_loop_retries_next_tick_after_rate_limited_account_snapshot() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = AccountSnapshotRateLimitOnceGateway {
        attempts: AtomicUsize::new(0),
        inner: MockTradingGateway::new()
            .with_cash(Decimal::new(1_000_000, 0))
            .with_quote(Quote {
                symbol: symbol.clone(),
                full_name: None,
                last_price: Decimal::new(73_000, 0),
            }),
    };
    let runtime = RuntimeConfig {
        market_tick_seconds: 1,
        ..RuntimeConfig::default()
    };

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[symbol],
        DryRunRuntimeSettings::new(runtime, StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(2),
        },
    )
    .await
    .expect("rate-limited account snapshot should retry on the next tick");

    assert!(output.contains("Market tick #1\n  status: snapshot_failed"));
    assert!(output.contains("  stage: TickObservation.account_snapshot"));
    assert!(output.contains("rate limited: KIS rate limit for domestic_stock_balance"));
    assert!(output.contains("Market tick #2\n  status: completed"));
    assert_eq!(runtime_run_statuses(&db_path), vec!["completed"]);
    assert_eq!(
        runtime_tick_statuses(&db_path),
        vec!["snapshot_failed", "completed"]
    );
    assert_eq!(runtime_tick_sequences(&db_path), vec![0, 1]);
    assert_eq!(gateway.attempts.load(Ordering::SeqCst), 2);

    let detail = first_tick_detail_json(&db_path);
    assert!(detail.contains("\"stage\":\"TickObservation.account_snapshot\""));
    assert!(detail.contains("rate limited"));
}

#[tokio::test]
async fn dry_run_loop_continues_next_tick_after_temporary_buying_power_read() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let advisor_script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &advisor_script_path,
        r#"printf '%s\n' '{"summary":"advisor buy","observations":["quote ok"],"risks":["read-only"],"next_actions":["risk gate"],"rationale":"exercise buying power retry path","evidence":[{"source":"test","detail":"buy proposal"}],"proposals":[{"decision_type":"buy","symbol":"005930","quantity":1,"order_type":"limit","limit_price":"73000","confidence":"0.62","time_horizon":"intraday","reason":"test buy","risk_notes":[]}]}'
"#,
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = BuyingPowerTemporaryOnceGateway {
        attempts: AtomicUsize::new(0),
        inner: MockTradingGateway::new()
            .with_cash(Decimal::new(1_000_000, 0))
            .with_quote(Quote {
                symbol: symbol.clone(),
                full_name: None,
                last_price: Decimal::new(73_000, 0),
            }),
    };
    let runtime = RuntimeConfig {
        market_tick_seconds: 1,
        ..RuntimeConfig::default()
    };
    let settings = DryRunRuntimeSettings::new(runtime, StrategyConfig::default()).with_advisor(
        AdvisorConfig {
            provider: AdvisorProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![advisor_script_path.to_string_lossy().into_owned()],
            timeout_seconds: 5,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
            min_command_interval_seconds: 1,
            ..AdvisorConfig::default()
        },
    );

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[symbol],
        settings,
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(2),
        },
    )
    .await
    .expect("temporary buying power read should continue on the next tick");

    assert!(output.contains("Market tick #1\n  status: snapshot_failed"));
    assert!(output.contains("  stage: buying_power"));
    assert!(output.contains("APBK1350"));
    assert!(output.contains("Market tick #2\n  status: completed"));
    assert_eq!(runtime_run_statuses(&db_path), vec!["completed"]);
    assert_eq!(
        runtime_tick_statuses(&db_path),
        vec!["snapshot_failed", "completed"]
    );
    assert!(gateway.attempts.load(Ordering::SeqCst) >= 1);

    let detail = first_tick_detail_json(&db_path);
    assert!(detail.contains("\"stage\":\"buying_power\""));
    assert!(detail.contains("APBK1350"));
}

#[tokio::test]
async fn dry_run_loop_inserts_started_tick_before_gateway_snapshot() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = StartedTickAssertingGateway {
        db_path: db_path.clone(),
        inner: MockTradingGateway::new()
            .with_cash(Decimal::new(1_000_000, 0))
            .with_quote(Quote {
                symbol: symbol.clone(),
                full_name: None,
                last_price: Decimal::new(73_000, 0),
            }),
    };

    run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[symbol],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop");

    assert_eq!(runtime_tick_statuses(&db_path), vec!["completed"]);
}

#[tokio::test]
async fn dry_run_loop_records_failed_tick_when_tick_times_out() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let runtime = RuntimeConfig {
        max_tick_runtime_seconds: 1,
        ..RuntimeConfig::default()
    };
    let error = run_dry_run_loop(
        &SlowAccountGateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(runtime, StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect_err("slow gateway should time out the tick");

    assert!(error.to_string().contains("timeout"));
    assert_eq!(runtime_run_statuses(&db_path), vec!["failed"]);
    assert_eq!(runtime_tick_statuses(&db_path), vec!["failed"]);
    let detail = first_tick_detail_json(&db_path);
    assert!(detail.contains("\"stage\":\"timeout\""));
}

#[tokio::test]
async fn dry_run_loop_records_skip_when_previous_tick_lease_is_active() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let run_context = DryRunRunContext::new("5678", BrokerEnvironment::Paper);
    let mut db = MainDb::open(&db_path).expect("open db");
    let active_run_id = db
        .start_dry_run_run(&run_context)
        .expect("start active run");
    db.start_dry_run_tick(active_run_id, 0, Duration::from_secs(30))
        .expect("start active tick lease");
    drop(db);

    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new();
    let output = run_dry_run_loop(
        &gateway,
        &store,
        &run_context,
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("previous running tick is skipped");

    assert!(output.contains("Market tick #1\n  status: skipped_previous_tick_running"));
    assert_eq!(
        runtime_tick_statuses(&db_path),
        vec!["started", "skipped_previous_tick_running"]
    );
    let calls = gateway.call_counts();
    assert_eq!(calls.account_snapshot, 0);
    assert_eq!(calls.quotes, 0);
}

#[tokio::test]
async fn dry_run_loop_collects_snapshot_when_market_is_closed() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("삼성전자보통주".to_owned()),
            last_price: Decimal::new(73_000, 0),
        })
        .with_market_session(MarketSession {
            trading_day: "20260426".to_owned(),
            is_open: false,
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("market closed tick still collects dry-run data");

    assert!(output.contains("Dry-run snapshot"));
    assert!(output.contains("  account_suffix: 5678"));
    assert!(output.contains("full_name=삼성전자보통주"));
    assert!(output.contains("last_price=73000"));
    assert!(output.contains("Market tick #1\n  status: completed"));
    assert_eq!(runtime_tick_statuses(&db_path), vec!["completed"]);
    let calls = gateway.call_counts();
    assert_eq!(calls.market_session, 1);
    assert_eq!(calls.account_snapshot, 1);
    assert_eq!(calls.open_orders, 1);
    assert_eq!(calls.fills, 1);
    assert_eq!(calls.quotes, 1);
}

#[tokio::test]
async fn dry_run_once_skips_command_advisor_after_market_closed_snapshot() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let request_path = tempdir.path().join("advisor-request.json");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        format!(
            r#"while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done > "{}"
printf '%s\n' '{{"summary":"closed market dry-run reviewed","observations":["snapshot present"],"risks":["market closed"],"next_actions":["review audit"],"rationale":"closed market snapshot was available","evidence":[{{"source":"kis_api","detail":"quote captured"}}],"proposals":[{{"decision_type":"hold","symbol":"005930","quantity":null,"order_type":null,"limit_price":null,"confidence":"0.50","time_horizon":null,"reason":"market closed hold","risk_notes":["market closed"]}}]}}'
"#,
            request_path.display()
        ),
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: Some("삼성전자보통주".to_owned()),
            last_price: Decimal::new(73_000, 0),
        })
        .with_market_session(MarketSession {
            trading_day: "20260426".to_owned(),
            is_open: false,
        });
    let settings = DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
        .with_advisor(AdvisorConfig {
            provider: AdvisorProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            timeout_seconds: 5,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
            ..AdvisorConfig::default()
        });

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
    )
    .await
    .expect("dry-run once with closed market advisor skip");

    assert!(output.contains("Dry-run snapshot"));
    assert!(output.contains("Tick advisory #1\n  status: parsed\n  provider: market_closed"));
    assert!(output.contains("  summary: market is closed; advisor provider was not invoked"));
    assert!(
        !request_path.exists(),
        "closed-market tick must not invoke command advisor"
    );
    assert_eq!(
        advisor_action_types(&db_path),
        vec!["tick_advisory_completed"]
    );
}

#[tokio::test]
async fn dry_run_loop_reuses_market_session_cache_within_ttl() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
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
    let runtime = RuntimeConfig {
        market_tick_seconds: 1,
        market_session_ttl_seconds: 300,
        ..RuntimeConfig::default()
    };

    run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[symbol],
        DryRunRuntimeSettings::new(runtime, StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: Some(2),
        },
    )
    .await
    .expect("dry-run loop");

    assert_eq!(gateway.call_counts().market_session, 1);
    assert_eq!(
        runtime_tick_statuses(&db_path),
        vec!["completed", "completed"]
    );
}

#[tokio::test]
async fn dry_run_loop_records_interrupted_run_when_shutdown_is_requested() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let shutdown = RuntimeShutdownToken::new();
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = ShutdownAfterQuoteGateway {
        shutdown: shutdown.clone(),
        inner: MockTradingGateway::new()
            .with_cash(Decimal::new(1_000_000, 0))
            .with_quote(Quote {
                symbol: symbol.clone(),
                full_name: None,
                last_price: Decimal::new(73_000, 0),
            })
            .with_market_session(MarketSession {
                trading_day: "20260426".to_owned(),
                is_open: true,
            }),
    };

    let output = run_dry_run_loop_with_shutdown(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[symbol],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::Regular,
            max_market_ticks: None,
        },
        shutdown,
    )
    .await
    .expect("shutdown stops the loop cleanly");

    assert!(output.contains("Dry-run loop\n  status: stopped\n  reason: shutdown_signal"));
    assert_eq!(runtime_run_statuses(&db_path), vec!["interrupted"]);
    assert_eq!(runtime_tick_statuses(&db_path), vec!["completed"]);
}

#[tokio::test]
async fn dry_run_loop_stops_while_waiting_for_next_tick() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let shutdown = RuntimeShutdownToken::new();
    let shutdown_for_task = shutdown.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        shutdown_for_task.request_shutdown();
    });
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: symbol.clone(),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });
    let settings = DryRunRuntimeSettings::new(
        RuntimeConfig {
            market_tick_seconds: 3600,
            ..RuntimeConfig::default()
        },
        StrategyConfig::default(),
    );

    let output = tokio::time::timeout(
        Duration::from_secs(2),
        run_dry_run_loop_with_shutdown(
            &gateway,
            &store,
            &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
            &[symbol],
            settings,
            RuntimeLoopControl {
                market_hours: RuntimeMarketHours::Regular,
                max_market_ticks: None,
            },
            shutdown,
        ),
    )
    .await
    .expect("shutdown should interrupt next-tick sleep")
    .expect("shutdown stops the loop cleanly");

    assert!(output.contains("Dry-run loop\n  status: stopped\n  reason: shutdown_signal"));
    assert_eq!(runtime_run_statuses(&db_path), vec!["interrupted"]);
    assert_eq!(runtime_tick_statuses(&db_path), vec!["completed"]);
}

#[tokio::test]
async fn dry_run_loop_uses_off_hours_tick_intervals() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });

    let output = run_dry_run_loop(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
        RuntimeLoopControl {
            market_hours: RuntimeMarketHours::OffHours,
            max_market_ticks: Some(1),
        },
    )
    .await
    .expect("dry-run loop");

    assert!(output.contains("  market_interval_seconds: 900"));
}

fn runtime_run_statuses(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT status FROM runtime_runs ORDER BY id")
        .expect("prepare statuses");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query statuses")
        .collect::<Result<Vec<_>, _>>()
        .expect("statuses")
}

fn runtime_run_environments(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT environment FROM runtime_runs ORDER BY id")
        .expect("prepare environments");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query environments")
        .collect::<Result<Vec<_>, _>>()
        .expect("environments")
}

fn advisor_action_types(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare(
            "SELECT action_type
             FROM operator_actions
             WHERE target_type = 'runtime_run'
             ORDER BY id",
        )
        .expect("prepare advisor action types");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query advisor action types")
        .collect::<Result<Vec<_>, _>>()
        .expect("advisor action types")
}

fn test_universe_candidate(
    symbol: &str,
    confidence: Decimal,
    risk_flags: Vec<UniverseDiscoveryRiskFlag>,
) -> UniverseDiscoveryCandidate {
    UniverseDiscoveryCandidate {
        symbol: Symbol::new(symbol).expect("valid symbol"),
        bucket: Some(UniverseDiscoveryBucket::ShortTermMomentum),
        target_support: None,
        signals: vec![UniverseDiscoverySignal::RelativeStrength],
        evidence: vec![UniverseDiscoveryEvidence {
            metric: "relative strength".to_owned(),
            value: "ranked fixture".to_owned(),
            as_of: Some("latest available tick".to_owned()),
            source: Some("test universe snapshot".to_owned()),
        }],
        risk_flags,
        reason: format!("{symbol} ranked fixture"),
        confidence,
    }
}

fn seed_daily_feature_audit(db: &mut MainDb) {
    for (day, close, volume) in [
        (1, 71_000, 10),
        (2, 72_000, 20),
        (3, 73_000, 30),
        (4, 74_000, 100),
    ] {
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
            bar: DailyOhlcvBar {
                symbol: Symbol::new("005930").expect("valid symbol"),
                trading_day: TradingDay(
                    Date::from_calendar_date(2026, Month::January, day).expect("valid date"),
                ),
                open: Decimal::new(close - 100, 0),
                high: Decimal::new(close + 100, 0),
                low: Decimal::new(close - 200, 0),
                close: Decimal::new(close, 0),
                volume: Decimal::new(volume, 0),
            },
        })
        .expect("record bar");
    }
    db.compute_and_record_latest_daily_technical_features(&NewTechnicalFeatureAuditComputation {
        observed_at: test_time(5),
        source: "unit-test".to_owned(),
        symbol: Symbol::new("005930").expect("valid symbol"),
        interval: OhlcvBarInterval::Daily,
        feature_set: "daily-core-v1".to_owned(),
        params: TechnicalFeatureParams {
            sma_short_window: 2,
            sma_long_window: 3,
            ema_short_window: 2,
            ema_long_window: 3,
            atr_window: 3,
            rsi_window: 3,
            roc_window: 3,
            average_volume_window: 3,
            volume_breakout_ratio: Decimal::new(5, 0),
        },
        lookback_limit: 4,
    })
    .expect("compute feature audit")
    .expect("feature audit id");
}

fn seed_daily_feature_audit_for_symbol(db: &mut MainDb, symbol: &str, closes: [i64; 4]) {
    for (index, close) in closes.into_iter().enumerate() {
        let day = u8::try_from(index + 1).expect("day fits u8");
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
            bar: DailyOhlcvBar {
                symbol: Symbol::new(symbol).expect("valid symbol"),
                trading_day: TradingDay(
                    Date::from_calendar_date(2026, Month::January, day).expect("valid date"),
                ),
                open: Decimal::new(close - 100, 0),
                high: Decimal::new(close + 100, 0),
                low: Decimal::new(close - 200, 0),
                close: Decimal::new(close, 0),
                volume: Decimal::new(i64::from(day) * 10, 0),
            },
        })
        .expect("record bar");
    }
    db.compute_and_record_latest_daily_technical_features(&NewTechnicalFeatureAuditComputation {
        observed_at: test_time(5),
        source: "unit-test".to_owned(),
        symbol: Symbol::new(symbol).expect("valid symbol"),
        interval: OhlcvBarInterval::Daily,
        feature_set: "daily-core-v1".to_owned(),
        params: TechnicalFeatureParams {
            sma_short_window: 2,
            sma_long_window: 3,
            ema_short_window: 2,
            ema_long_window: 3,
            atr_window: 3,
            rsi_window: 3,
            roc_window: 3,
            average_volume_window: 3,
            volume_breakout_ratio: Decimal::new(5, 0),
        },
        lookback_limit: 4,
    })
    .expect("compute daily features")
    .expect("feature audit id");
}

fn test_time(day: u8) -> time::OffsetDateTime {
    Date::from_calendar_date(2026, Month::January, day)
        .expect("valid date")
        .with_time(Time::MIDNIGHT)
        .assume_utc()
}

fn advisor_metadata(path: &Path) -> String {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT metadata_json
             FROM operator_actions
             WHERE target_type = 'runtime_run'
             ORDER BY id
             LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("advisor metadata")
}

#[derive(Default)]
struct CollectingOutputSink {
    chunks: Vec<String>,
}

impl RuntimeOutputSink for CollectingOutputSink {
    fn write(&mut self, chunk: &str) -> Result<(), trade_engine::error::TradeEngineError> {
        self.chunks.push(chunk.to_owned());
        Ok(())
    }
}

fn runtime_tick_statuses(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT status FROM runtime_ticks ORDER BY id")
        .expect("prepare statuses");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query statuses")
        .collect::<Result<Vec<_>, _>>()
        .expect("statuses")
}

fn runtime_tick_sequences(path: &Path) -> Vec<i64> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT tick_sequence FROM runtime_ticks ORDER BY id")
        .expect("prepare sequences");
    statement
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("query sequences")
        .collect::<Result<Vec<_>, _>>()
        .expect("sequences")
}

fn first_tick_detail_json(path: &Path) -> String {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT status_detail_json FROM runtime_ticks ORDER BY id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("tick detail")
}

struct StartedTickAssertingGateway {
    db_path: PathBuf,
    inner: MockTradingGateway,
}

struct AccountSnapshotRateLimitOnceGateway {
    attempts: AtomicUsize,
    inner: MockTradingGateway,
}

#[async_trait]
impl TradingGateway for AccountSnapshotRateLimitOnceGateway {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        self.inner.ping().await
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(BrokerError::RateLimit {
                message: "KIS rate limit for domestic_stock_balance".to_owned(),
            });
        }

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
        self.inner.place_order(order).await
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        self.inner.cancel_order(request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        self.inner.amend_order(request).await
    }
}

struct BuyingPowerTemporaryOnceGateway {
    attempts: AtomicUsize,
    inner: MockTradingGateway,
}

#[async_trait]
impl TradingGateway for BuyingPowerTemporaryOnceGateway {
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
        if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(BrokerError::Temporary {
                message: "KIS returned error APBK1350: 조회 오류입니다. 다시 조회 하세요."
                    .to_owned(),
            });
        }

        self.inner.get_buying_power(symbol).await
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        self.inner.list_open_orders().await
    }

    async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        self.inner.list_fills(day).await
    }

    async fn place_order(&self, order: CreateOrderRequest) -> Result<OrderSubmission, BrokerError> {
        self.inner.place_order(order).await
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        self.inner.cancel_order(request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        self.inner.amend_order(request).await
    }
}

#[async_trait]
impl TradingGateway for StartedTickAssertingGateway {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        self.inner.ping().await
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        assert_eq!(last_runtime_tick_status(&self.db_path), "started");
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
        self.inner.place_order(order).await
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        self.inner.cancel_order(request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        self.inner.amend_order(request).await
    }
}

struct SlowAccountGateway;

#[async_trait]
impl TradingGateway for SlowAccountGateway {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        Ok(BrokerHealth {
            environment: BrokerEnvironment::Paper,
            writable: false,
        })
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(AccountSnapshot {
            cash: CashBalance {
                cash: Decimal::ZERO,
            },
            positions: Vec::new(),
        })
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        Ok(CashBalance {
            cash: Decimal::ZERO,
        })
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        Ok(Vec::new())
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        Ok(Quote {
            symbol: symbol.clone(),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        })
    }

    async fn get_market_session(&self, _trading_day: &str) -> Result<MarketSession, BrokerError> {
        Ok(MarketSession {
            trading_day: "19700101".to_owned(),
            is_open: true,
        })
    }

    async fn get_buying_power(&self, _symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        Ok(BuyingPower {
            available_cash: Decimal::ZERO,
        })
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        Ok(Vec::new())
    }

    async fn list_fills(&self, _day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        Ok(Vec::new())
    }

    async fn place_order(
        &self,
        _order: CreateOrderRequest,
    ) -> Result<OrderSubmission, BrokerError> {
        Err(BrokerError::validation(
            "dry-run timeout test gateway does not support order writes",
        ))
    }

    async fn cancel_order(
        &self,
        _request: CancelOrderRequest,
    ) -> Result<CancelResult, BrokerError> {
        Err(BrokerError::validation(
            "dry-run timeout test gateway does not support order writes",
        ))
    }

    async fn amend_order(&self, _request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        Err(BrokerError::validation(
            "dry-run timeout test gateway does not support order writes",
        ))
    }
}

struct ShutdownAfterQuoteGateway {
    shutdown: RuntimeShutdownToken,
    inner: MockTradingGateway,
}

#[async_trait]
impl TradingGateway for ShutdownAfterQuoteGateway {
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
        let quote = self.inner.get_quote(symbol).await?;
        self.shutdown.request_shutdown();
        Ok(quote)
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
        self.inner.place_order(order).await
    }

    async fn cancel_order(&self, request: CancelOrderRequest) -> Result<CancelResult, BrokerError> {
        self.inner.cancel_order(request).await
    }

    async fn amend_order(&self, request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        self.inner.amend_order(request).await
    }
}

fn last_runtime_tick_status(path: &Path) -> String {
    let connection = Connection::open(path).expect("open db");
    connection
        .query_row(
            "SELECT status FROM runtime_ticks ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("started tick row")
}
