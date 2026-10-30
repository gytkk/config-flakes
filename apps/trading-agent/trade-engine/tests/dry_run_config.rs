use std::path::{Path, PathBuf};

use rust_decimal::Decimal;
use trade_engine::config::{
    AdvisorProviderKind, BrokerProviderKind, DecisionProviderKind, DomesticMarketAlignmentConfig,
    DomesticMarketAlignmentMode, EngineConfig, KisEnvironment, MarketKind,
    UniverseDiscoveryProviderKind,
};
use trading_agent_kis_client::UsEquityOrderSession;

#[test]
fn dry_run_config_accepts_live_read_only_mode() {
    let config = EngineConfig {
        read_only: true,
        watchlist: vec!["005930".to_owned()],
        broker: Default::default(),
        market: Default::default(),
        kis: Default::default(),
        toss: Default::default(),
        storage: trade_engine::config::StorageConfig {
            main_db_path: Some(PathBuf::from("main.db")),
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
    };

    config
        .validate_read_only_smoke()
        .expect("live dry-run is allowed for smoke reads");
    assert_eq!(config.kis.environment, KisEnvironment::Live);
}

#[test]
fn dry_run_config_fails_closed_without_read_only() {
    let config = EngineConfig {
        read_only: false,
        watchlist: vec!["005930".to_owned()],
        broker: Default::default(),
        market: Default::default(),
        kis: Default::default(),
        toss: Default::default(),
        storage: Default::default(),
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
    };

    let error = config
        .validate_read_only_smoke()
        .expect_err("read_only=false must fail closed");

    assert!(error.to_string().contains("read_only=true"));
}

#[test]
fn backtest_config_accepts_read_only_stored_data_mode() {
    let config = EngineConfig {
        read_only: true,
        watchlist: vec!["005930".to_owned()],
        broker: Default::default(),
        market: Default::default(),
        kis: Default::default(),
        toss: Default::default(),
        storage: trade_engine::config::StorageConfig {
            main_db_path: Some(PathBuf::from("main.db")),
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
    };

    config
        .validate_backtest()
        .expect("backtest uses read-only stored data");
}

#[test]
fn backtest_config_rejects_live_write_mode() {
    let config = EngineConfig {
        read_only: false,
        watchlist: vec!["005930".to_owned()],
        broker: Default::default(),
        market: Default::default(),
        kis: Default::default(),
        toss: Default::default(),
        storage: trade_engine::config::StorageConfig {
            main_db_path: Some(PathBuf::from("main.db")),
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
    };

    let error = config
        .validate_backtest()
        .expect_err("backtest must not use live write mode");

    assert!(error.to_string().contains("read_only=true"));
}

#[test]
fn domestic_market_alignment_defaults_to_disabled_audit_only() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[strategy]
id = "hold-only"

[risk]
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_loss = "1000000"
max_daily_order_count = 3
enable_live_trading = false
"#,
    )
    .expect("parse config without domestic alignment section");

    assert!(!config.domestic_market_alignment.enabled);
    assert_eq!(
        config.domestic_market_alignment.mode,
        DomesticMarketAlignmentMode::AuditOnly
    );
    assert_eq!(config.domestic_market_alignment.benchmark_symbol, None);
    assert_eq!(
        config.domestic_market_alignment.feature_set,
        "daily-core-v1"
    );
    assert_eq!(
        config
            .domestic_market_alignment
            .relative_strength_lookback_days,
        20
    );
}

#[test]
fn domestic_market_alignment_accepts_advisor_context_config() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[strategy]
id = "hold-only"

[domestic_market_alignment]
enabled = true
mode = "advisor_context"
benchmark_symbol = "KOSPI"
feature_set = "daily-core-v1"
relative_strength_lookback_days = 20

[risk]
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_loss = "1000000"
max_daily_order_count = 3
enable_live_trading = false
"#,
    )
    .expect("parse domestic alignment config");

    assert!(config.domestic_market_alignment.enabled);
    assert_eq!(
        config.domestic_market_alignment.mode,
        DomesticMarketAlignmentMode::AdvisorContext
    );
    assert_eq!(
        config
            .domestic_market_alignment
            .benchmark_symbol()
            .expect("benchmark symbol")
            .expect("configured benchmark")
            .as_str(),
        "KOSPI"
    );
    config
        .validate_read_only_smoke()
        .expect("domestic alignment config validates");
}

#[test]
fn domestic_market_alignment_rejects_empty_feature_set() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[strategy]
id = "hold-only"

[domestic_market_alignment]
enabled = true
mode = "advisor_context"
feature_set = ""

[risk]
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_loss = "1000000"
max_daily_order_count = 3
enable_live_trading = false
"#,
    )
    .expect("parse config with invalid domestic alignment feature set");

    let error = config
        .validate_read_only_smoke()
        .expect_err("empty feature set must fail validation");
    assert!(
        error
            .to_string()
            .contains("domestic_market_alignment.feature_set")
    );
}

#[test]
fn domestic_market_alignment_rejects_zero_relative_strength_lookback_days() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[strategy]
id = "hold-only"

[domestic_market_alignment]
enabled = true
mode = "advisor_context"
relative_strength_lookback_days = 0

[risk]
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_loss = "1000000"
max_daily_order_count = 3
enable_live_trading = false
"#,
    )
    .expect("parse config with invalid domestic alignment lookback");

    let error = config
        .validate_read_only_smoke()
        .expect_err("zero lookback must fail validation");
    assert!(
        error
            .to_string()
            .contains("domestic_market_alignment.relative_strength_lookback_days")
    );
}

#[test]
fn domestic_market_alignment_sends_advisor_context_only_when_enabled_for_advisor_context() {
    let disabled_advisor_context = DomesticMarketAlignmentConfig {
        enabled: false,
        mode: DomesticMarketAlignmentMode::AdvisorContext,
        ..Default::default()
    };
    let enabled_audit_only = DomesticMarketAlignmentConfig {
        enabled: true,
        mode: DomesticMarketAlignmentMode::AuditOnly,
        ..Default::default()
    };
    let enabled_advisor_context = DomesticMarketAlignmentConfig {
        enabled: true,
        mode: DomesticMarketAlignmentMode::AdvisorContext,
        ..Default::default()
    };

    assert!(!disabled_advisor_context.sends_advisor_context());
    assert!(!enabled_audit_only.sends_advisor_context());
    assert!(enabled_advisor_context.sends_advisor_context());
}

#[test]
fn live_dry_run_config_is_read_only_and_uses_a_main_db() {
    let config_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../configs/trade-engine.live-dry-run.toml");
    let config = EngineConfig::load_from_path(config_path).expect("load live dry-run config");

    assert!(config.read_only);
    assert_eq!(config.kis.environment, KisEnvironment::Live);
    assert_eq!(config.market.kind, MarketKind::DomesticStock);
    assert!(config.storage.main_db_path.is_some());
    assert_eq!(config.strategy.id, "sma-crossover-v1");
    assert_eq!(
        config.decision.provider,
        DecisionProviderKind::Deterministic
    );
    assert_eq!(config.advisor.provider, AdvisorProviderKind::Command);
    assert_eq!(config.advisor.command.as_deref(), Some(Path::new("sh")));
    assert_eq!(config.runtime.market_tick_seconds, 30);
    assert_eq!(config.runtime.max_tick_runtime_seconds, 240);
    assert_eq!(config.advisor.timeout_seconds, 180);
    assert_eq!(config.advisor.min_command_interval_seconds, 600);
    assert_eq!(config.advisor.price_change_threshold_bps, 75);
    assert!(config.sizing.enabled);
    assert_eq!(config.sizing.target_account_fraction, Decimal::new(10, 2));
    assert!(config.exit_policy.enabled);
    assert_eq!(config.exit_policy.stop_loss_pct, Decimal::new(5, 2));
    assert_eq!(config.exit_policy.take_profit_pct, Decimal::new(10, 2));
    assert_eq!(
        config.universe_discovery.provider,
        UniverseDiscoveryProviderKind::Command
    );
    assert_eq!(
        config.universe_discovery.command.as_deref(),
        Some(Path::new("sh"))
    );
    assert!(config.universe_discovery.auto_refresh);
    assert!(!config.risk.enable_live_trading);
    config
        .validate_read_only_smoke()
        .expect("live dry-run uses read-only KIS access");
}

#[test]
fn config_accepts_toss_read_only_provider_without_kis_table() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[broker]
provider = "toss"

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[toss]
client_id_env = "TOSS_CLIENT_ID"
client_secret_env = "TOSS_CLIENT_SECRET"
account_seq = 7

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_loss = "1000000"
max_daily_order_count = 3
enable_live_trading = false
"#,
    )
    .expect("parse toss config without KIS table");

    assert_eq!(config.broker.provider, BrokerProviderKind::Toss);
    assert_eq!(
        config
            .toss
            .account_seq()
            .expect("configured account seq")
            .as_u64(),
        7
    );
    config
        .validate_read_only_smoke()
        .expect("toss read-only config");
}

#[test]
fn toss_dry_run_example_config_is_read_only() {
    let config_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../configs/trade-engine.toss-dry-run.toml");
    let config = EngineConfig::load_from_path(config_path).expect("load Toss dry-run config");

    assert!(config.read_only);
    assert_eq!(config.broker.provider, BrokerProviderKind::Toss);
    assert_eq!(config.market.kind, MarketKind::DomesticStock);
    assert!(config.storage.main_db_path.is_some());
    assert_eq!(config.toss.client_id_env, "TOSS_CLIENT_ID");
    assert_eq!(config.toss.client_secret_env, "TOSS_CLIENT_SECRET");
    assert_eq!(config.toss.account_seq_env, "TOSS_ACCOUNT_SEQ");
    assert!(!config.risk.enable_live_trading);
    config
        .validate_read_only_smoke()
        .expect("Toss dry-run config uses read-only access");
}

#[test]
fn config_rejects_toss_order_submission_runtime() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = false
watchlist = ["005930"]

[broker]
provider = "toss"

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[toss]
client_id_env = "TOSS_CLIENT_ID"
client_secret_env = "TOSS_CLIENT_SECRET"
account_seq = 7

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_loss = "1000000"
max_daily_order_count = 3
enable_live_trading = true
"#,
    )
    .expect("parse toss write config");

    let error = config
        .validate_write_runtime_safety()
        .expect_err("toss write mode must remain disabled");
    assert!(error.to_string().contains("Toss"));
    assert!(error.to_string().contains("read-only"));
}

#[test]
fn config_rejects_secret_material_in_toss_table() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[broker]
provider = "toss"

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[toss]
client_id_env = "TOSS_CLIENT_ID"
client_secret = "do-not-put-secret-material-here"
account_seq = 7

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_loss = "1000000"
max_daily_order_count = 3
enable_live_trading = false
"#,
    )
    .expect_err("literal Toss secret must be rejected");

    assert!(error.to_string().contains("client_secret"));
}

#[test]
fn config_rejects_deprecated_command_decision_provider() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "command"
command = "sh"
args = ["scripts/decision-provider"]

[risk]
enable_live_trading = false
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_order_count = 3
"#,
    )
    .expect("parse deprecated command decision config")
    .validate_read_only_smoke()
    .expect_err("command decision provider is no longer a runtime boundary");

    assert!(error.to_string().contains("decision.provider=command"));
    assert!(error.to_string().contains("advisor.provider=command"));
}

#[test]
fn overseas_dry_run_config_selects_overseas_market_job() {
    let config_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../configs/trade-engine.overseas-dry-run.toml");
    let config = EngineConfig::load_from_path(config_path).expect("load overseas dry-run config");

    assert!(config.read_only);
    assert_eq!(config.market.kind, MarketKind::OverseasStock);
    assert_eq!(
        config.market.scope().expect("overseas scope").kind(),
        "overseas_stock"
    );
    assert_eq!(
        config
            .market
            .allowed_us_equity_order_sessions()
            .expect("order sessions"),
        vec![
            UsEquityOrderSession::Regular,
            UsEquityOrderSession::Premarket,
            UsEquityOrderSession::Aftermarket,
            UsEquityOrderSession::Daytime,
        ]
    );
    assert_eq!(config.advisor.provider, AdvisorProviderKind::Command);
    assert_eq!(config.runtime.max_tick_runtime_seconds, 240);
    assert_eq!(config.advisor.timeout_seconds, 180);
    assert!(config.sizing.enabled);
    assert_eq!(config.sizing.target_account_fraction, Decimal::new(10, 2));
    assert!(config.exit_policy.enabled);
    assert_eq!(config.exit_policy.stop_loss_pct, Decimal::new(5, 2));
    assert_eq!(config.exit_policy.take_profit_pct, Decimal::new(10, 2));
    assert_eq!(
        config.universe_discovery.provider,
        UniverseDiscoveryProviderKind::Command
    );
    assert_eq!(
        config.universe_discovery.command.as_deref(),
        Some(Path::new("sh"))
    );
    assert!(config.universe_discovery.auto_refresh);
    assert!(!config.risk.enable_live_trading);
    config
        .validate_read_only_smoke()
        .expect("overseas dry-run uses read-only KIS access");
}

#[test]
fn overseas_stock_config_requires_exchange_and_currency() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["AAPL"]

[market]
kind = "overseas_stock"
exchange_code = "NASD"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
enable_live_trading = false
max_order_notional = "1000"
max_position_notional = "5000"
max_daily_order_count = 3
"#,
    )
    .expect("parse overseas config missing currency")
    .validate_read_only_smoke()
    .expect_err("overseas stock market requires currency");

    assert!(error.to_string().contains("market.currency"));
}

#[test]
fn overseas_stock_config_defaults_to_regular_order_session() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["AAPL"]

[market]
kind = "overseas_stock"
exchange_code = "NASD"
currency = "USD"

[kis]
environment = "live"
token_cache_path = ".local/state/token.json"

[storage]
main_db_path = "main.db"

[strategy]
id = "hold-only"

[risk]
enable_live_trading = false
max_order_notional = "1000"
max_position_notional = "5000"
max_daily_order_count = 3
"#,
    )
    .expect("config");

    assert_eq!(
        config
            .market
            .allowed_us_equity_order_sessions()
            .expect("order sessions"),
        vec![UsEquityOrderSession::Regular]
    );
}

#[test]
fn overseas_stock_config_accepts_explicit_order_sessions() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = false
watchlist = ["AAPL"]

[market]
kind = "overseas_stock"
exchange_code = "NASD"
currency = "USD"
allowed_order_sessions = ["regular", "premarket", "aftermarket", "daytime"]

[kis]
environment = "live"
token_cache_path = ".local/state/token.json"

[storage]
main_db_path = "main.db"

[strategy]
id = "hold-only"

[risk]
enable_live_trading = true
max_order_notional = "1000"
max_position_notional = "5000"
max_daily_order_count = 3
"#,
    )
    .expect("config");

    assert_eq!(
        config
            .market
            .allowed_us_equity_order_sessions()
            .expect("order sessions"),
        vec![
            UsEquityOrderSession::Regular,
            UsEquityOrderSession::Premarket,
            UsEquityOrderSession::Aftermarket,
            UsEquityOrderSession::Daytime,
        ]
    );
}

#[test]
fn overseas_stock_config_builds_market_scope_without_cash_probe_symbol() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["AAPL", "MSFT"]

[market]
kind = "overseas_stock"
exchange_code = "NASD"
currency = "USD"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
enable_live_trading = false
max_order_notional = "1000"
max_position_notional = "5000"
max_daily_order_count = 3
"#,
    )
    .expect("parse overseas config");

    config
        .validate_read_only_smoke()
        .expect("overseas read-only smoke config");
    assert_eq!(config.market.kind, MarketKind::OverseasStock);
    assert_eq!(
        config.market.scope().expect("scope").kind(),
        "overseas_stock"
    );
}

#[test]
fn config_rejects_watchlist_symbols_outside_market_scope() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "overseas_stock"
exchange_code = "NASD"
currency = "USD"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
enable_live_trading = false
max_order_notional = "1000"
max_position_notional = "5000"
max_daily_order_count = 3
"#,
    )
    .expect("parse overseas config with domestic watchlist")
    .validate_read_only_smoke()
    .expect_err("watchlist must match configured market scope");

    assert!(error.to_string().contains("watchlist"));
    assert!(error.to_string().contains("overseas_stock"));

    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["AAPL"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

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
    .expect("parse domestic config with overseas watchlist")
    .validate_read_only_smoke()
    .expect_err("watchlist must match configured market scope");

    assert!(error.to_string().contains("watchlist"));
    assert!(error.to_string().contains("domestic_stock"));
}

#[test]
fn overseas_stock_config_rejects_cash_probe_symbol_key() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["AAPL"]

[market]
kind = "overseas_stock"
exchange_code = "NASD"
currency = "USD"
cash_probe_symbol = "AAPL"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
enable_live_trading = false
max_order_notional = "1000"
max_position_notional = "5000"
max_daily_order_count = 3
"#,
    )
    .expect_err("cash probe symbol is no longer a config key");

    assert!(error.to_string().contains("cash_probe_symbol"));
}

#[test]
fn live_submit_orders_config_is_write_enabled_without_account_literals() {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../configs/trade-engine.live-submit-orders.toml");
    let config = EngineConfig::load_from_path(config_path).expect("load live submit-orders config");

    assert!(!config.read_only);
    assert_eq!(config.kis.environment, KisEnvironment::Live);
    assert_eq!(config.market.kind, MarketKind::DomesticStock);
    assert!(config.storage.main_db_path.is_some());
    assert_eq!(config.strategy.id, "hold-only");
    assert_eq!(
        config.decision.provider,
        DecisionProviderKind::Deterministic
    );
    assert_eq!(config.advisor.provider, AdvisorProviderKind::Command);
    assert_eq!(config.advisor.command.as_deref(), Some(Path::new("sh")));
    assert_eq!(config.runtime.market_tick_seconds, 30);
    assert_eq!(config.runtime.max_tick_runtime_seconds, 240);
    assert_eq!(config.advisor.timeout_seconds, 180);
    assert_eq!(config.advisor.recent_price_count, 16);
    assert_eq!(config.advisor.candidate_limit, 10);
    assert_eq!(config.advisor.min_command_interval_seconds, 900);
    assert_eq!(config.advisor.price_change_threshold_bps, 100);
    assert!(config.sizing.enabled);
    assert_eq!(config.sizing.target_account_fraction, Decimal::new(40, 2));
    assert!(config.exit_policy.enabled);
    assert_eq!(config.exit_policy.stop_loss_pct, Decimal::new(8, 2));
    assert_eq!(config.exit_policy.take_profit_pct, Decimal::new(15, 2));
    assert_eq!(
        config.universe_discovery.provider,
        UniverseDiscoveryProviderKind::Command
    );
    assert_eq!(
        config.universe_discovery.command.as_deref(),
        Some(Path::new("sh"))
    );
    assert!(config.universe_discovery.auto_refresh);
    assert_eq!(
        config.decision_source_id().expect("decision source id"),
        "advisor-command"
    );
    assert!(config.risk.enable_live_trading);
    config
        .validate_write_runtime_safety()
        .expect("live submit-orders config uses env-scoped live guards");
}

#[test]
fn overseas_submit_orders_config_selects_overseas_write_job() {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../configs/trade-engine.overseas-submit-orders.toml");
    let config =
        EngineConfig::load_from_path(config_path).expect("load overseas submit-orders config");

    assert!(!config.read_only);
    assert_eq!(config.kis.environment, KisEnvironment::Live);
    assert_eq!(config.market.kind, MarketKind::OverseasStock);
    assert_eq!(config.market.exchange_code.as_deref(), Some("NASD"));
    assert_eq!(config.market.currency.as_deref(), Some("USD"));
    assert_eq!(
        config.market.scope().expect("overseas scope").kind(),
        "overseas_stock"
    );
    assert_eq!(
        config
            .market
            .allowed_us_equity_order_sessions()
            .expect("order sessions"),
        vec![
            UsEquityOrderSession::Regular,
            UsEquityOrderSession::Premarket,
            UsEquityOrderSession::Aftermarket,
            UsEquityOrderSession::Daytime,
        ]
    );
    assert_eq!(config.strategy.id, "hold-only");
    assert_eq!(config.advisor.provider, AdvisorProviderKind::Command);
    assert_eq!(config.runtime.market_tick_seconds, 30);
    assert_eq!(config.runtime.max_tick_runtime_seconds, 240);
    assert_eq!(config.advisor.timeout_seconds, 180);
    assert_eq!(config.advisor.recent_price_count, 16);
    assert_eq!(config.advisor.candidate_limit, 10);
    assert_eq!(config.advisor.min_command_interval_seconds, 900);
    assert_eq!(config.advisor.price_change_threshold_bps, 100);
    assert!(config.sizing.enabled);
    assert_eq!(config.sizing.target_account_fraction, Decimal::new(40, 2));
    assert!(config.exit_policy.enabled);
    assert_eq!(config.exit_policy.stop_loss_pct, Decimal::new(8, 2));
    assert_eq!(config.exit_policy.take_profit_pct, Decimal::new(15, 2));
    assert_eq!(
        config.universe_discovery.provider,
        UniverseDiscoveryProviderKind::Command
    );
    assert!(config.universe_discovery.auto_refresh);
    assert_eq!(
        config.decision_source_id().expect("decision source id"),
        "advisor-command"
    );
    assert!(config.risk.enable_live_trading);
    assert_eq!(config.risk.max_order_notional, Decimal::new(3000, 0));
    config
        .validate_write_runtime_safety()
        .expect("overseas submit-orders config uses env-scoped live guards");
}

#[test]
fn config_fills_nested_defaults_from_partial_tables() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"
app_key_env = "CUSTOM_KIS_APP_KEY"

[runtime]
market_tick_seconds = 45

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
enable_live_trading = false
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_loss = "750000"
max_daily_order_count = 3
"#,
    )
    .expect("partial nested config uses defaults");

    assert_eq!(config.kis.environment, KisEnvironment::Paper);
    assert_eq!(config.kis.app_key_env, "CUSTOM_KIS_APP_KEY");
    assert_eq!(config.kis.app_secret_env, "KIS_APP_SECRET");
    assert_eq!(config.kis.account_key_env, "KIS_ACCOUNT_KEY");
    assert_eq!(config.kis.account_key, None);
    assert_eq!(config.runtime.market_tick_seconds, 45);
    assert_eq!(config.runtime.off_hours_market_tick_seconds, 900);
    assert_eq!(config.runtime.max_tick_runtime_seconds, 10);
    assert!(config.runtime.skip_if_previous_tick_running);
    assert_eq!(config.runtime.market_session_ttl_seconds, 300);
    assert_eq!(config.advisor.provider, AdvisorProviderKind::Disabled);
    assert_eq!(config.advisor.timeout_seconds, 120);
    assert_eq!(
        config.universe_discovery.provider,
        UniverseDiscoveryProviderKind::Disabled
    );
    assert_eq!(config.universe_discovery.max_symbols, 20);
    assert!(!config.sizing.enabled);
    assert_eq!(config.sizing.target_account_fraction, Decimal::new(10, 2));
    assert!(!config.exit_policy.enabled);
    assert_eq!(config.exit_policy.stop_loss_pct, Decimal::new(5, 2));
    assert_eq!(config.exit_policy.take_profit_pct, Decimal::new(10, 2));
    assert_eq!(config.risk.max_daily_loss, Decimal::new(750_000, 0));
}

#[test]
fn config_accepts_direct_kis_account_key() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"
account_key = "12345678-01"
account_key_env = "__TRADE_ENGINE_TEST_MISSING_ACCOUNT_KEY_DIRECT__"

[strategy]
id = "hold-only"

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
    .expect("parse config with direct KIS account key");

    assert_eq!(config.kis.account_key.as_deref(), Some("12345678-01"));
    assert_eq!(
        config
            .kis
            .account_key()
            .expect("direct account key should resolve")
            .as_str(),
        "12345678-01"
    );
    config
        .validate_read_only_smoke()
        .expect("direct KIS account key should be valid config");
}

#[test]
fn config_validates_direct_kis_account_key() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"
account_key = "12345678 01"

[strategy]
id = "hold-only"

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
    .expect("parse config with invalid direct KIS account key");

    let error = config
        .validate_read_only_smoke()
        .expect_err("direct KIS account key should be validated");

    assert!(error.to_string().contains("kis.account_key"));
    assert!(error.to_string().contains("must not contain whitespace"));
}

#[test]
fn config_requires_explicit_market_kind() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

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
    .expect_err("market table must be explicit");

    assert!(error.to_string().contains("market"));

    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

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
    .expect_err("market.kind must be explicit");

    assert!(error.to_string().contains("kind"));
}

#[test]
fn advisor_command_config_validates_command_boundary() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[runtime]
max_tick_runtime_seconds = 180

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[advisor]
provider = "command"
command = "sh"
args = ["trade-engine/scripts/codex-advisor"]

[risk]
enable_live_trading = false
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_order_count = 3
"#,
    )
    .expect("parse advisor command config");

    config
        .validate_read_only_smoke()
        .expect("advisor command config");
    assert_eq!(config.advisor.provider, AdvisorProviderKind::Command);
    assert_eq!(config.advisor.timeout_seconds, 120);
    assert_eq!(config.advisor.max_stderr_bytes, 4096);
    assert_eq!(config.advisor.max_stdout_bytes, 262_144);
    assert_eq!(config.advisor.recent_price_count, 20);
    assert_eq!(config.advisor.candidate_limit, 8);
    assert_eq!(config.advisor.min_command_interval_seconds, 600);
    assert_eq!(config.advisor.price_change_threshold_bps, 75);
}

#[test]
fn universe_discovery_command_config_validates_command_boundary() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[universe_discovery]
provider = "command"
command = "sh"
args = ["trade-engine/scripts/codex-universe"]

[risk]
enable_live_trading = false
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_order_count = 3
"#,
    )
    .expect("parse universe discovery command config");

    config
        .validate_read_only_smoke()
        .expect("universe discovery command config");
    assert_eq!(
        config.universe_discovery.provider,
        UniverseDiscoveryProviderKind::Command
    );
    assert_eq!(config.universe_discovery.max_symbols, 20);
    assert_eq!(config.universe_discovery.timeout_seconds, 120);
    assert_eq!(config.universe_discovery.max_stderr_bytes, 4096);
    assert_eq!(config.universe_discovery.max_stdout_bytes, 262_144);
    assert_eq!(config.universe_discovery.refresh_seconds, 10_800);
    assert!(config.universe_discovery.auto_refresh);
}

#[test]
fn config_rejects_advisor_timeout_without_tick_runtime_buffer() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[runtime]
max_tick_runtime_seconds = 120

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

[advisor]
provider = "command"
command = "sh"
args = ["trade-engine/scripts/codex-advisor"]
timeout_seconds = 120

[risk]
enable_live_trading = false
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_order_count = 3
"#,
    )
    .expect("parse advisor timeout config");

    let error = config
        .validate_read_only_smoke()
        .expect_err("advisor timeout requires tick runtime buffer");

    assert!(
        error
            .to_string()
            .contains("runtime.max_tick_runtime_seconds")
    );
    assert!(error.to_string().contains("advisor.timeout_seconds"));
}

#[test]
fn config_rejects_legacy_top_level_environment_field() {
    let error = toml::from_str::<EngineConfig>(
        r#"
environment = "paper"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

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
    .expect_err("environment is no longer part of trade-engine config");

    assert!(error.to_string().contains("environment"));
}

#[test]
fn config_requires_explicit_kis_environment() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"

[strategy]
id = "hold-only"

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
    .expect_err("KIS environment must be explicit");

    assert!(error.to_string().contains("environment"));
}

#[test]
fn config_rejects_secret_material_in_kis_table() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = true
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "paper"
token_cache_path = ".local/state/trade-engine/kis-paper-token.json"
app_secret = "do-not-put-secret-values-here"

[strategy]
id = "hold-only"

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
    .expect_err("secret values are not config fields");

    assert!(error.to_string().contains("app_secret"));
}

#[test]
fn live_write_safety_fails_closed_without_enable_flag_and_guards() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = false
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "live"
token_cache_path = ".local/state/trade-engine/kis-live-token.json"

[strategy]
id = "hold-only"

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
    .expect("parse live write-shaped config");

    let error = config
        .validate_live_write_safety()
        .expect_err("read_only=false alone must not enable live writes");

    assert!(error.to_string().contains("risk.enable_live_trading"));
}

#[test]
fn live_write_safety_does_not_require_toml_account_confirmation() {
    let config = toml::from_str::<EngineConfig>(
        r#"
read_only = false
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "live"
token_cache_path = ".local/state/trade-engine/kis-live-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
enable_live_trading = true
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_order_count = 3
"#,
    )
    .expect("parse live safety config without account literals");

    config
        .validate_live_write_safety()
        .expect("live safety no longer requires account literals in TOML");
}

#[test]
fn config_rejects_deprecated_live_account_confirmation_literal() {
    let error = toml::from_str::<EngineConfig>(
        r#"
read_only = false
watchlist = ["005930"]

[market]
kind = "domestic_stock"

[storage]
main_db_path = "main.db"

[kis]
environment = "live"
token_cache_path = ".local/state/trade-engine/kis-live-token.json"

[strategy]
id = "hold-only"

[decision]
provider = "deterministic"
timeout_seconds = 15
max_stderr_bytes = 4096

[risk]
enable_live_trading = true
max_order_notional = "1000000"
max_position_notional = "5000000"
max_daily_order_count = 3

[live]
confirmed_account_key = "12345678-01"
"#,
    )
    .expect_err("account literals must not be accepted in TOML");

    assert!(error.to_string().contains("confirmed_account_key"));
}

#[test]
fn account_redaction_uses_only_kis_account_number_suffix() {
    assert_eq!(
        EngineConfig::account_suffix_from_account_key("12345678-01"),
        "5678"
    );
    assert_eq!(
        EngineConfig::account_suffix_from_account_key("001234-01"),
        "1234"
    );
}
