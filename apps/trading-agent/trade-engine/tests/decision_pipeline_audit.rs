use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;
use rust_decimal::Decimal;
use serde_json::Value;
use tempfile::tempdir;

use trade_engine::config::{
    AdvisorConfig, AdvisorProviderKind, DecisionConfig, ExitPolicyConfig, RiskConfig,
    RuntimeConfig, SizingConfig, StrategyConfig,
};
use trade_engine::decision::DecisionOutputStatus;
use trade_engine::gateway::testing::MockTradingGateway;
use trade_engine::gateway::types::{
    AccountSnapshot, CashBalance, Fill, MarketScope, Position, Quantity, Quote, Symbol,
};
use trade_engine::runtime::{
    DryRunAdvisoryAudit, DryRunRunContext, DryRunRuntimeSettings, DryRunSnapshot, DryRunTickOutput,
    run_dry_run_tick,
};
use trade_engine::storage::{AuditCounts, BlockingMainDb, MainDb, SqliteBrokerCommandJournal};
use trading_agent_kis_client::journal::{BrokerCommandJournal, BrokerCommandRecord};
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, ClientOrderId, CreateOrderRequest, CurrencyCode, ExchangeCode,
    OrderReference, OrderSide, OrderState, OrderStatus, OrderStatusEvidence, OrderType, RequestId,
};

#[tokio::test]
async fn dry_run_tick_persists_snapshot_signal_decision_and_risk_audit() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: Symbol::new("005930").expect("valid symbol"),
            quantity: Quantity::new(10),
            average_price: Decimal::new(70_000, 0),
        })
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
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
    assert!(output.contains(
        "  - 005930 quantity=10 average_price=70000 current_price=73000 unrealized_return_pct=4.29"
    ));
    assert!(output.contains(
        "  - 005930 strategy=advisor-deterministic_sma signal=hold strength=0 decision=hold confidence=0 risk=approved"
    ));
    let decisions_index = output.find("Decisions").expect("decisions section");
    let positions_index = output.rfind("Positions").expect("positions section");
    assert!(
        positions_index > decisions_index,
        "positions should be rendered after decisions"
    );
    assert!(
        output[positions_index..].trim_end().ends_with(
            "  - 005930 quantity=10 average_price=70000 current_price=73000 unrealized_return_pct=4.29"
        ),
        "positions should be the final rendered section"
    );
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
    let input_contexts = decision_input_contexts(&db_path);
    assert_eq!(input_contexts.len(), 2);
    assert_eq!(input_contexts[0]["source"], "dry_run");
    assert_eq!(input_contexts[0]["provider"], "deterministic_sma");
    let request = &input_contexts[0]["decision_request"];
    assert_eq!(request["runtime_mode"], "dry_run");
    assert_eq!(request["environment"], "paper");
    assert_eq!(request["account_suffix"], "5678");
    assert_eq!(request["cash"], "1000000");
    assert_eq!(request["positions"][0]["symbol"], "005930");
    assert_eq!(request["positions"][0]["average_price"], "70000");
    assert_eq!(request["quote"]["symbol"], "005930");
    assert_eq!(request["quote"]["last_price"], "73000");
    assert_eq!(
        request["strategy_signal"]["strategy_id"],
        "advisor-deterministic_sma"
    );

    let policy_snapshots = risk_policy_snapshots(&db_path);
    assert_eq!(policy_snapshots.len(), 2);
    assert_eq!(policy_snapshots[0]["policy_version"], "risk_policy_v1");
    assert_eq!(policy_snapshots[0]["read_only"], false);
    assert_eq!(policy_snapshots[0]["live_trading_enabled"], false);
    assert_eq!(
        policy_snapshots[0]["allowed_symbols"],
        serde_json::json!(["000660", "005930"])
    );
    assert_eq!(policy_snapshots[0]["max_order_notional"], "1000000");
    assert_eq!(policy_snapshots[0]["max_position_notional"], "5000000");
    assert_eq!(policy_snapshots[0]["max_daily_loss"], "1000000");
    assert_eq!(policy_snapshots[0]["max_daily_order_count"], 3);
    assert!(policy_snapshots[0].get("order_cooldown_seconds").is_none());
}

#[tokio::test]
async fn dry_run_tick_collects_position_quotes_outside_watchlist_for_position_returns() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let position_symbol = Symbol::new("005930").expect("valid symbol");
    let watchlist_symbol = Symbol::new("000660").expect("valid symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: position_symbol.clone(),
            quantity: Quantity::new(10),
            average_price: Decimal::new(70_000, 0),
        })
        .with_quote(Quote {
            symbol: position_symbol.clone(),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        })
        .with_quote(Quote {
            symbol: watchlist_symbol.clone(),
            full_name: None,
            last_price: Decimal::new(180_000, 0),
        });

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[watchlist_symbol],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
    )
    .await
    .expect("dry-run tick with position quote");

    assert!(output.contains(
        "  - 005930 quantity=10 average_price=70000 current_price=73000 unrealized_return_pct=4.29"
    ));
    assert!(output.contains(
        "  - 000660 strategy=advisor-deterministic_sma signal=hold strength=0 decision=hold confidence=0 risk=approved"
    ));
    assert!(!output.contains("  - 005930 strategy=advisor-deterministic_sma"));
    assert_eq!(gateway.call_counts().quotes, 2);
    assert_eq!(
        MainDb::open(&db_path)
            .expect("inspect db")
            .audit_counts()
            .expect("counts")
            .market_snapshots,
        2
    );
}

#[tokio::test]
async fn dry_run_tick_uses_stored_market_snapshots_for_sma_buy_signal() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for price in [Decimal::new(100, 0); 15]
        .into_iter()
        .chain([Decimal::new(90, 0); 5])
    {
        record_price(&mut db, price);
    }
    drop(db);
    let store = BlockingMainDb::open(&db_path).expect("open db");

    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(200, 0),
        });

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default()),
    )
    .await
    .expect("dry-run tick with audit");

    assert!(output.contains("  - 005930 strategy=advisor-deterministic_sma signal=buy"));
    assert!(output.contains("decision=buy"));
    assert!(output.contains("risk=blocked"));
}

#[tokio::test]
async fn dry_run_tick_uses_runtime_risk_settings() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for price in [Decimal::new(100, 0); 15]
        .into_iter()
        .chain([Decimal::new(90, 0); 5])
    {
        record_price(&mut db, price);
    }
    drop(db);
    let store = BlockingMainDb::open(&db_path).expect("open db");

    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(200, 0),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::with_risk(
            RuntimeConfig::default(),
            StrategyConfig::default(),
            DecisionConfig::default(),
            risk,
            false,
        ),
    )
    .await
    .expect("dry-run tick with risk config");

    assert!(output.contains("  - 005930 strategy=advisor-deterministic_sma signal=buy"));
    assert!(output.contains("risk=approved"));
    assert_eq!(
        execution_request_rows(&db_path),
        vec![(
            "requested".to_owned(),
            "005930".to_owned(),
            "buy".to_owned(),
            1,
            "limit".to_owned(),
            Some("200".to_owned())
        )]
    );
    let counts = MainDb::open(&db_path)
        .expect("inspect db")
        .audit_counts()
        .expect("counts");
    assert_eq!(counts.execution_requests, 1);
    assert_eq!(counts.execution_events, 1);
}

#[tokio::test]
async fn dry_run_tick_sizes_buy_proposal_from_account_fraction() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        r#"cat > /dev/null
printf '%s\n' '{"summary":"advisor buy","observations":["quote present"],"risks":[],"next_actions":["risk gate"],"rationale":"candidate entry","evidence":[{"source":"test","detail":"quote present"}],"proposals":[{"decision_type":"buy","symbol":"005930","quantity":1,"order_type":"limit","limit_price":"100","confidence":"0.62","time_horizon":"intraday","reason":"advisor one-share buy","risk_notes":[]}]}'
"#,
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000, 0))
        .with_buying_power(Decimal::new(1_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(100, 0),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        max_order_notional: Decimal::new(1_000, 0),
        max_position_notional: Decimal::new(1_000, 0),
        ..RiskConfig::default()
    };
    let settings = DryRunRuntimeSettings::with_risk(
        RuntimeConfig::default(),
        StrategyConfig {
            id: "hold-only".to_owned(),
            short_window: None,
            long_window: None,
        },
        DecisionConfig::default(),
        risk,
        false,
    )
    .with_advisor(command_advisor_config(
        "/bin/sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(1),
    ))
    .with_sizing(SizingConfig {
        enabled: true,
        target_account_fraction: Decimal::new(30, 2),
    });

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        settings,
    )
    .await
    .expect("dry-run tick with sizing");

    assert!(output.contains("decision=buy"));
    assert_eq!(
        execution_request_rows(&db_path),
        vec![(
            "requested".to_owned(),
            "005930".to_owned(),
            "buy".to_owned(),
            3,
            "limit".to_owned(),
            Some("100".to_owned())
        )]
    );
}

#[tokio::test]
async fn overseas_limit_proposals_are_stored_with_kis_valid_cent_prices() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        r#"cat > /dev/null
printf '%s\n' '{"summary":"advisor orders","observations":["quotes present"],"risks":[],"next_actions":["risk gate"],"rationale":"approved order prices","evidence":[{"source":"test","detail":"sub-cent overseas prices"}],"proposals":[{"decision_type":"sell","symbol":"NAVN","quantity":1,"order_type":"limit","limit_price":"21.8650","confidence":"0.99","time_horizon":"approved_rebalance","reason":"approved NAVN sell","risk_notes":[]},{"decision_type":"sell","symbol":"SMCI","quantity":1,"order_type":"limit","limit_price":"35.4001","confidence":"0.99","time_horizon":"approved_rebalance","reason":"approved SMCI sell","risk_notes":[]},{"decision_type":"buy","symbol":"AAPL","quantity":1,"order_type":"limit","limit_price":"301.9101","confidence":"0.99","time_horizon":"approved_rebalance","reason":"approved AAPL buy","risk_notes":[]}]}'
"#,
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let navn = Symbol::new("NAVN").expect("symbol");
    let smci = Symbol::new("SMCI").expect("symbol");
    let aapl = Symbol::new("AAPL").expect("symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000, 0))
        .with_buying_power(Decimal::new(1_000, 0))
        .with_position(Position {
            symbol: navn.clone(),
            quantity: Quantity::new(1),
            average_price: Decimal::new(23_8000, 4),
        })
        .with_position(Position {
            symbol: smci.clone(),
            quantity: Quantity::new(1),
            average_price: Decimal::new(34_6500, 4),
        })
        .with_quote(Quote {
            symbol: navn.clone(),
            full_name: None,
            last_price: Decimal::new(21_8650, 4),
        })
        .with_quote(Quote {
            symbol: smci.clone(),
            full_name: None,
            last_price: Decimal::new(35_4001, 4),
        })
        .with_quote(Quote {
            symbol: aapl.clone(),
            full_name: None,
            last_price: Decimal::new(301_9101, 4),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        max_order_notional: Decimal::new(3_000, 0),
        max_position_notional: Decimal::new(5_000, 0),
        ..RiskConfig::default()
    };
    let settings = DryRunRuntimeSettings::with_risk(
        RuntimeConfig::default(),
        StrategyConfig {
            id: "hold-only".to_owned(),
            short_window: None,
            long_window: None,
        },
        DecisionConfig::default(),
        risk,
        false,
    )
    .with_advisor(command_advisor_config(
        "/bin/sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(1),
    ));
    let run_context = DryRunRunContext::new("5678", BrokerEnvironment::Paper).with_market_scope(
        MarketScope::OverseasStock {
            exchange: ExchangeCode::new("NASD").expect("exchange"),
            currency: CurrencyCode::new("USD").expect("currency"),
        },
    );

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &run_context,
        &[navn, smci, aapl],
        settings,
    )
    .await
    .expect("overseas dry-run tick");

    assert!(output.contains("NAVN strategy=advisor-command signal=sell"));
    assert_eq!(
        execution_request_rows(&db_path),
        vec![
            (
                "requested".to_owned(),
                "NAVN".to_owned(),
                "sell".to_owned(),
                1,
                "limit".to_owned(),
                Some("21.86".to_owned())
            ),
            (
                "requested".to_owned(),
                "SMCI".to_owned(),
                "sell".to_owned(),
                1,
                "limit".to_owned(),
                Some("35.40".to_owned())
            ),
            (
                "requested".to_owned(),
                "AAPL".to_owned(),
                "buy".to_owned(),
                1,
                "limit".to_owned(),
                Some("301.92".to_owned())
            )
        ]
    );
}

#[tokio::test]
async fn dry_run_tick_adds_stop_loss_exit_proposal_for_open_position() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        r#"cat > /dev/null
printf '%s\n' '{"summary":"advisor buy","observations":["quote present"],"risks":[],"next_actions":["risk gate"],"rationale":"candidate entry","evidence":[{"source":"test","detail":"quote present"}],"proposals":[{"decision_type":"buy","symbol":"005930","quantity":1,"order_type":"limit","limit_price":"94","confidence":"0.62","time_horizon":"intraday","reason":"advisor buy despite drawdown","risk_notes":[]}]}'
"#,
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: symbol.clone(),
            quantity: Quantity::new(5),
            average_price: Decimal::new(100, 0),
        })
        .with_quote(Quote {
            symbol: symbol.clone(),
            full_name: None,
            last_price: Decimal::new(94, 0),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };
    let settings = DryRunRuntimeSettings::with_risk(
        RuntimeConfig::default(),
        StrategyConfig {
            id: "hold-only".to_owned(),
            short_window: None,
            long_window: None,
        },
        DecisionConfig::default(),
        risk,
        false,
    )
    .with_advisor(command_advisor_config(
        "/bin/sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(1),
    ))
    .with_exit_policy(ExitPolicyConfig {
        enabled: true,
        stop_loss_pct: Decimal::new(5, 2),
        take_profit_pct: Decimal::new(10, 2),
    });

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[symbol],
        settings,
    )
    .await
    .expect("dry-run tick with stop loss exit");

    assert!(output.contains("decision=sell"));
    assert_eq!(
        execution_request_rows(&db_path),
        vec![(
            "requested".to_owned(),
            "005930".to_owned(),
            "sell".to_owned(),
            5,
            "limit".to_owned(),
            Some("94".to_owned())
        )]
    );
}

#[tokio::test]
async fn dry_run_tick_blocks_sell_when_quantity_exceeds_position() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let script_path = tempdir.path().join("advisor.sh");
    fs::write(
        &script_path,
        r#"cat > /dev/null
printf '%s\n' '{"summary":"advisor sell","observations":["quote present"],"risks":[],"next_actions":["risk gate"],"rationale":"exit candidate","evidence":[{"source":"test","detail":"quote present"}],"proposals":[{"decision_type":"sell","symbol":"005930","quantity":10,"order_type":"limit","limit_price":"94","confidence":"0.62","time_horizon":"intraday","reason":"advisor sell above position","risk_notes":[]}]}'
"#,
    )
    .expect("write advisor script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let symbol = Symbol::new("005930").expect("valid symbol");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: symbol.clone(),
            quantity: Quantity::new(5),
            average_price: Decimal::new(100, 0),
        })
        .with_quote(Quote {
            symbol: symbol.clone(),
            full_name: None,
            last_price: Decimal::new(94, 0),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };
    let settings = DryRunRuntimeSettings::with_risk(
        RuntimeConfig::default(),
        StrategyConfig {
            id: "hold-only".to_owned(),
            short_window: None,
            long_window: None,
        },
        DecisionConfig::default(),
        risk,
        false,
    )
    .with_advisor(command_advisor_config(
        "/bin/sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(1),
    ));

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[symbol],
        settings,
    )
    .await
    .expect("dry-run tick with sell quantity risk");

    assert!(output.contains("decision=sell"));
    assert!(output.contains("risk=blocked"));
    assert_eq!(
        risk_block_reasons(&db_path),
        vec!["sell_quantity_exceeds_position"]
    );
    assert!(execution_request_rows(&db_path).is_empty());
}

#[tokio::test]
async fn dry_run_tick_blocks_buy_when_buying_power_is_insufficient() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for price in [Decimal::new(100, 0); 15]
        .into_iter()
        .chain([Decimal::new(90, 0); 5])
    {
        record_price(&mut db, price);
    }
    drop(db);
    let store = BlockingMainDb::open(&db_path).expect("open db");

    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_buying_power(Decimal::new(100, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(200, 0),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::with_risk(
            RuntimeConfig::default(),
            StrategyConfig::default(),
            DecisionConfig::default(),
            risk,
            false,
        ),
    )
    .await
    .expect("dry-run tick with buying power risk");

    assert!(output.contains("  - 005930 strategy=advisor-deterministic_sma signal=buy"));
    assert!(output.contains("risk=blocked"));
    assert_eq!(gateway.call_counts().buying_power, 1);
    assert_eq!(
        risk_block_reasons(&db_path),
        vec!["insufficient_buying_power"]
    );
}

#[tokio::test]
async fn dry_run_tick_blocks_buy_when_daily_loss_limit_is_reached() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for price in [Decimal::new(100, 0); 15]
        .into_iter()
        .chain([Decimal::new(90, 0); 5])
    {
        record_price(&mut db, price);
    }
    drop(db);
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let symbol = Symbol::new("005930").expect("valid symbol");

    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_position(Position {
            symbol: symbol.clone(),
            quantity: Quantity::new(10),
            average_price: Decimal::new(80_000, 0),
        })
        .with_fill(Fill {
            reference: test_order_reference("loss-fill"),
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
        .with_quote(Quote {
            symbol: symbol.clone(),
            full_name: None,
            last_price: Decimal::new(200, 0),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        max_daily_loss: Decimal::new(10_000, 0),
        ..RiskConfig::default()
    };

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[symbol],
        DryRunRuntimeSettings::with_risk(
            RuntimeConfig::default(),
            StrategyConfig::default(),
            DecisionConfig::default(),
            risk,
            false,
        ),
    )
    .await
    .expect("dry-run tick with daily loss risk");

    assert!(output.contains("  - 005930 strategy=advisor-deterministic_sma signal=buy"));
    assert!(output.contains("risk=blocked"));
    assert_eq!(gateway.call_counts().fills, 1);
    assert_eq!(
        risk_block_reasons(&db_path),
        vec!["max_daily_loss_exceeded"]
    );
    assert!(execution_request_rows(&db_path).is_empty());
}

#[tokio::test]
async fn dry_run_tick_blocks_when_unresolved_broker_command_exists() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for price in [Decimal::new(100, 0); 15]
        .into_iter()
        .chain([Decimal::new(90, 0); 5])
    {
        record_price(&mut db, price);
    }
    drop(db);
    seed_unresolved_journal_command(&db_path).await;
    let store = BlockingMainDb::open(&db_path).expect("open db");

    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(200, 0),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::with_risk(
            RuntimeConfig::default(),
            StrategyConfig::default(),
            DecisionConfig::default(),
            risk,
            false,
        ),
    )
    .await
    .expect("dry-run tick with journal risk");

    assert!(output.contains("  - 005930 strategy=advisor-deterministic_sma signal=buy"));
    assert!(output.contains("risk=blocked"));
    assert_eq!(
        risk_block_reasons(&db_path),
        vec!["unresolved_journal_commands"]
    );
}

#[tokio::test]
async fn dry_run_tick_blocks_when_matching_open_order_exists() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");
    for price in [Decimal::new(100, 0); 15]
        .into_iter()
        .chain([Decimal::new(90, 0); 5])
    {
        record_price(&mut db, price);
    }
    drop(db);
    let store = BlockingMainDb::open(&db_path).expect("open db");

    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_open_order(OrderStatus {
            reference: test_order_reference("open-order"),
            state: OrderState::Accepted,
            evidence: Some(OrderStatusEvidence {
                symbol: Symbol::new("005930").expect("valid symbol"),
                side: Some(OrderSide::Buy),
                ordered_quantity: Quantity::new(1),
                filled_quantity: Quantity::new(0),
                order_price: None,
                trading_day: Some("20260426".to_owned()),
                observed_at: None,
            }),
        })
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(200, 0),
        });
    let risk = RiskConfig {
        enable_live_trading: true,
        ..RiskConfig::default()
    };

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::with_risk(
            RuntimeConfig::default(),
            StrategyConfig::default(),
            DecisionConfig::default(),
            risk,
            false,
        ),
    )
    .await
    .expect("dry-run tick with open order risk");

    assert!(output.contains("  - 005930 strategy=advisor-deterministic_sma signal=buy"));
    assert!(output.contains("risk=blocked"));
    assert_eq!(gateway.call_counts().open_orders, 1);
    assert_eq!(risk_block_reasons(&db_path), vec!["duplicate_order_intent"]);
    assert!(execution_request_rows(&db_path).is_empty());
}

#[tokio::test]
async fn dry_run_tick_uses_hold_only_strategy_from_config() {
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
    let strategy = StrategyConfig {
        id: "hold-only".to_owned(),
        short_window: None,
        long_window: None,
    };

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), strategy),
    )
    .await
    .expect("dry-run tick with hold-only strategy");

    assert!(
        output.contains("  - 005930 strategy=advisor-deterministic_sma signal=hold strength=0")
    );
    assert!(output.contains("decision=hold confidence=0"));
    let raw_outputs = decision_output_jsons(&db_path);
    assert_eq!(raw_outputs.len(), 1);
    assert!(raw_outputs[0].contains("\"reason\":\"strategy hold-only hold signal\""));
    assert!(!raw_outputs[0].contains("sma crossover v1"));
}

#[tokio::test]
async fn dry_run_tick_records_invalid_command_provider_output_without_risk() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let script_path = tempdir.path().join("provider.sh");
    fs::write(
        &script_path,
        r#"cat > /dev/null
printf '%s\n' '{not-json'
"#,
    )
    .expect("write provider script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
            .with_advisor(command_advisor_config(
                "/bin/sh",
                vec![script_path.to_string_lossy().into_owned()],
                Duration::from_secs(1),
            )),
    )
    .await
    .expect("invalid provider output is audited without order flow");

    assert!(output.contains(
        "  - 005930 strategy=advisor-command signal=hold strength=0 status=rejected provider=command"
    ));
    assert_eq!(runtime_tick_statuses(&db_path), vec!["decision_rejected"]);
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
            decision_proposals: 0,
            risk_evaluations: 0,
            risk_blocks: 0,
            execution_requests: 0,
            execution_events: 0,
            broker_command_events: 0,
            fills: 0,
        }
    );
    let outputs = decision_output_rows(&db_path);
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].0, "rejected");
    assert!(outputs[0].1.contains("{not-json"));
    assert!(
        outputs[0]
            .2
            .contains("failed to parse advisor provider output")
    );
}

#[tokio::test]
async fn dry_run_tick_records_failed_command_provider_exit_without_risk() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let script_path = tempdir.path().join("provider.sh");
    fs::write(
        &script_path,
        r#"cat > /dev/null
printf '%s' '{"decision_type":"hold"}'
printf '%s' 'provider failed' >&2
exit 7
"#,
    )
    .expect("write provider script");
    let store = BlockingMainDb::open(&db_path).expect("open db");
    let gateway = MockTradingGateway::new()
        .with_cash(Decimal::new(1_000_000, 0))
        .with_quote(Quote {
            symbol: Symbol::new("005930").expect("valid symbol"),
            full_name: None,
            last_price: Decimal::new(73_000, 0),
        });
    let mut advisor = command_advisor_config(
        "/bin/sh",
        vec![script_path.to_string_lossy().into_owned()],
        Duration::from_secs(1),
    );
    advisor.max_stderr_bytes = 8;

    let output = run_dry_run_tick(
        &gateway,
        &store,
        &DryRunRunContext::new("5678", BrokerEnvironment::Paper),
        &[Symbol::new("005930").expect("valid symbol")],
        DryRunRuntimeSettings::new(RuntimeConfig::default(), StrategyConfig::default())
            .with_advisor(advisor),
    )
    .await
    .expect("failed provider exit is audited without order flow");

    assert!(output.contains(
        "  - 005930 strategy=advisor-command signal=hold strength=0 status=failed provider=command"
    ));
    assert_eq!(runtime_tick_statuses(&db_path), vec!["decision_failed"]);
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
            decision_proposals: 0,
            risk_evaluations: 0,
            risk_blocks: 0,
            execution_requests: 0,
            execution_events: 0,
            broker_command_events: 0,
            fills: 0,
        }
    );
    let outputs = decision_output_rows(&db_path);
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].0, "failed");
    assert!(outputs[0].1.contains("\"decision_type\":\"hold\""));
    assert!(outputs[0].2.contains("exit status: 7"));
    assert!(outputs[0].2.contains("<truncated>"));
}

fn record_price(db: &mut MainDb, price: Decimal) {
    db.record_standalone_dry_run_tick(&DryRunTickOutput {
        snapshot: DryRunSnapshot {
            account_suffix: "5678".to_owned(),
            account: AccountSnapshot {
                cash: CashBalance {
                    cash: Decimal::new(1_000_000, 0),
                },
                positions: Vec::new(),
            },
            open_orders: Vec::new(),
            today_fills: Vec::new(),
            quotes: vec![Quote {
                symbol: Symbol::new("005930").expect("valid symbol"),
                full_name: None,
                last_price: price,
            }],
            position_quotes: Vec::new(),
        },
        technical_analysis: Vec::new(),
        domestic_market_alignment: None,
        universe_discovery: None,
        advisory: DryRunAdvisoryAudit {
            run_id: None,
            tick_sequence: 0,
            provider: "test".to_owned(),
            status: DecisionOutputStatus::Parsed,
            output: None,
            error: None,
        },
        decisions: Vec::new(),
        decision_failures: Vec::new(),
    })
    .expect("record price");
}

fn decision_output_jsons(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT raw_output_json FROM decision_outputs ORDER BY id")
        .expect("prepare decision outputs");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query decision outputs")
        .collect::<Result<Vec<_>, _>>()
        .expect("decision outputs")
}

fn decision_output_rows(path: &Path) -> Vec<(String, String, String)> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT status, raw_output_json, error FROM decision_outputs ORDER BY id")
        .expect("prepare decision outputs");
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("query decision outputs")
        .collect::<Result<Vec<_>, _>>()
        .expect("decision outputs")
}

fn decision_input_contexts(path: &Path) -> Vec<Value> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT context_json FROM decision_inputs ORDER BY id")
        .expect("prepare decision inputs");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query decision inputs")
        .map(|row| {
            let value = row.expect("decision input context");
            serde_json::from_str::<Value>(&value).expect("decision input context json")
        })
        .collect()
}

fn risk_policy_snapshots(path: &Path) -> Vec<Value> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT policy_snapshot_json FROM risk_evaluations ORDER BY id")
        .expect("prepare risk policy snapshots");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query risk policy snapshots")
        .map(|row| {
            let value = row.expect("risk policy snapshot");
            serde_json::from_str::<Value>(&value).expect("risk policy snapshot json")
        })
        .collect()
}

fn runtime_tick_statuses(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT status FROM runtime_ticks ORDER BY id")
        .expect("prepare runtime tick statuses");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query runtime tick statuses")
        .collect::<Result<Vec<_>, _>>()
        .expect("runtime tick statuses")
}

fn risk_block_reasons(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare("SELECT reason FROM risk_blocks ORDER BY id")
        .expect("prepare risk block reasons");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query risk block reasons")
        .collect::<Result<Vec<_>, _>>()
        .expect("risk block reasons")
}

fn execution_request_rows(
    path: &Path,
) -> Vec<(String, String, String, i64, String, Option<String>)> {
    let connection = Connection::open(path).expect("open db");
    let mut statement = connection
        .prepare(
            "SELECT status, symbol, side, quantity, order_type, limit_price
             FROM execution_requests
             ORDER BY id",
        )
        .expect("prepare execution requests");
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .expect("query execution requests")
        .collect::<Result<Vec<_>, _>>()
        .expect("execution requests")
}

async fn seed_unresolved_journal_command(path: &Path) {
    let journal = SqliteBrokerCommandJournal::open(path).expect("open journal");
    let client_order_id = ClientOrderId::new("unresolved-command").expect("client order id");
    journal
        .reserve_command(
            BrokerCommandRecord::new_place_order(
                BrokerEnvironment::Paper,
                CreateOrderRequest {
                    client_order_id: client_order_id.clone(),
                    account_key: AccountKey::new("12345678-01").expect("account key"),
                    symbol: Symbol::new("005930").expect("symbol"),
                    side: OrderSide::Buy,
                    quantity: Quantity::new(1),
                    order_type: OrderType::Limit,
                    limit_price: Some(Decimal::new(73_000, 0)),
                },
            )
            .expect("broker command"),
        )
        .await
        .expect("reserve command");
    journal
        .mark_submitting(
            &client_order_id,
            &RequestId::new("unresolved-request").expect("request id"),
        )
        .await
        .expect("mark submitting");
    journal
        .mark_reconcile_needed(&client_order_id, "ack timeout")
        .await
        .expect("mark unresolved");
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

fn command_advisor_config(
    command: impl Into<PathBuf>,
    args: Vec<String>,
    timeout: Duration,
) -> AdvisorConfig {
    AdvisorConfig {
        provider: AdvisorProviderKind::Command,
        command: Some(command.into()),
        args,
        timeout_seconds: timeout.as_secs(),
        max_stderr_bytes: 4096,
        max_stdout_bytes: 262_144,
        ..AdvisorConfig::default()
    }
}
