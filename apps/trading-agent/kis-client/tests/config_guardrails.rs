use trading_agent_kis_client::config::{BrokerRuntimeConfig, MarketSessionWriteGate};
use trading_agent_kis_client::domain::{AccountKey, BrokerEnvironment};
use trading_agent_kis_client::error::BrokerError;

#[test]
fn live_writes_require_explicit_enable_flag() {
    let config = BrokerRuntimeConfig {
        read_only: false,
        enable_live_trading: false,
        confirmed_account_suffix: Some("78-01".to_owned()),
        allowed_live_accounts: vec!["12345678-01".to_owned()],
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: BrokerRuntimeConfig::regular_us_equity_order_sessions(),
    };

    let error = config
        .validate_write_enabled(
            BrokerEnvironment::Live,
            &AccountKey::new("12345678-01").expect("valid account key"),
        )
        .expect_err("live writes should be blocked without enable flag");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
}

#[test]
fn read_only_blocks_all_writes() {
    let config = BrokerRuntimeConfig {
        read_only: true,
        enable_live_trading: false,
        confirmed_account_suffix: None,
        allowed_live_accounts: Vec::new(),
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: BrokerRuntimeConfig::regular_us_equity_order_sessions(),
    };

    let error = config
        .validate_write_enabled(
            BrokerEnvironment::Paper,
            &AccountKey::new("12345678-01").expect("valid account key"),
        )
        .expect_err("read only mode should block writes");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
}

#[test]
fn live_account_suffix_must_match_operator_confirmation() {
    let config = BrokerRuntimeConfig {
        read_only: false,
        enable_live_trading: true,
        confirmed_account_suffix: Some("00-00".to_owned()),
        allowed_live_accounts: vec!["12345678-01".to_owned()],
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: BrokerRuntimeConfig::regular_us_equity_order_sessions(),
    };

    let error = config
        .validate_write_enabled(
            BrokerEnvironment::Live,
            &AccountKey::new("12345678-01").expect("valid account key"),
        )
        .expect_err("suffix mismatch should block live writes");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
}

#[test]
fn live_account_suffix_matches_kis_account_number_before_product_code() {
    let config = BrokerRuntimeConfig {
        read_only: false,
        enable_live_trading: true,
        confirmed_account_suffix: Some("5678".to_owned()),
        allowed_live_accounts: vec!["12345678-01".to_owned()],
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: BrokerRuntimeConfig::regular_us_equity_order_sessions(),
    };

    config
        .validate_write_enabled(
            BrokerEnvironment::Live,
            &AccountKey::new("12345678-01").expect("valid account key"),
        )
        .expect("account-number suffix matches before KIS product code");
}

#[test]
fn live_account_must_be_present_in_allowlist() {
    let config = BrokerRuntimeConfig {
        read_only: false,
        enable_live_trading: true,
        confirmed_account_suffix: Some("78-01".to_owned()),
        allowed_live_accounts: vec!["87654321-01".to_owned()],
        market_session_write_gate: MarketSessionWriteGate::Enforced,
        allowed_us_equity_order_sessions: BrokerRuntimeConfig::regular_us_equity_order_sessions(),
    };

    let error = config
        .validate_write_enabled(
            BrokerEnvironment::Live,
            &AccountKey::new("12345678-01").expect("valid account key"),
        )
        .expect_err("non-allowlisted live account should be blocked");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));
    assert!(error.to_string().contains("allowed_live_accounts"));
}
