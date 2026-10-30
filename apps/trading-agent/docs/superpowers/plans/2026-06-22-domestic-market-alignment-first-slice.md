# Domestic Market Alignment First Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first domestic market-alignment slice that computes auditable alignment context and optionally passes it to the advisor request without changing execution, risk blocks, or order sizing.

**Architecture:** Add a pure `market_alignment` module for deterministic domestic regime and candidate scoring. Add a disabled-by-default config section, carry the config into runtime settings, compute alignment during tick observation for domestic markets, include it in advisor JSON only in `advisor_context` mode, and persist a summary in existing runtime tick detail JSON.

**Tech Stack:** Rust 2024, `trade-engine`, serde JSON/TOML, rusqlite-backed Main DB, existing dry-run runtime tests.

---

## File Structure

- Create: `trade-engine/src/market_alignment.rs`
  - Owns alignment enums, serializable context structs, and pure calculation logic.
- Modify: `trade-engine/src/lib.rs`
  - Exposes `market_alignment`.
- Modify: `trade-engine/src/config/mod.rs`
  - Adds `DomesticMarketAlignmentConfig` and `DomesticMarketAlignmentMode`.
- Modify: `trade-engine/src/runtime/mod.rs`
  - Adds runtime settings plumbing, computes alignment during observation, renders it, and passes it to advisor request JSON when enabled.
- Modify: `trade-engine/src/storage/mod.rs`
  - Persists completed tick status detail JSON with optional domestic alignment context.
- Modify: `trade-engine/src/advisor.rs`
  - Adds optional advisor request field and bumps schema version.
- Modify: `trade-engine/src/runner.rs`
  - Carries config into runtime settings.
- Modify: `trade-engine/tests/dry_run_config.rs`
  - Verifies config defaults, valid parsing, invalid parsing, and direct initializers.
- Modify: `trade-engine/tests/runtime_loop.rs`
  - Verifies request JSON, CLI rendering, and runtime tick detail persistence.
- Modify: `trade-engine/tests/decision_pipeline_audit.rs`, `trade-engine/tests/startup_doctor.rs`, `trade-engine/src/runner.rs` tests
  - Adds `domestic_market_alignment: Default::default()` to direct `EngineConfig` initializers.
- Modify: `README.md`
  - Documents the new disabled-by-default config section.

## Execution Setup

- [ ] **Step 1: Use an isolated workspace**

Run the `superpowers:using-git-worktrees` skill before implementation. If already in a linked worktree, stay there. If not, ask whether to create one, unless the user already gave a preference.

- [ ] **Step 2: Verify baseline**

Run:

```sh
cargo check --workspace --no-default-features
```

Expected: command succeeds before task work starts. If it fails, stop and report the baseline failure before editing.

---

### Task 1: Pure Domestic Market Alignment Module

**Files:**
- Create: `trade-engine/src/market_alignment.rs`
- Modify: `trade-engine/src/lib.rs`

- [ ] **Step 1: Write failing unit tests**

Add `pub mod market_alignment;` to `trade-engine/src/lib.rs`.

Create `trade-engine/src/market_alignment.rs` with only this test module and imports. Do not add production types or functions yet.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal::Decimal;
    use time::{Date, Month};

    use crate::gateway::types::{Symbol, TradingDay};
    use crate::technical_analysis::DailyTechnicalFeatures;

    fn symbol(value: &str) -> Symbol {
        Symbol::new(value).expect("valid symbol")
    }

    fn features(value: &str, roc: Option<Decimal>, volume_breakout: Option<bool>) -> DailyTechnicalFeatures {
        DailyTechnicalFeatures {
            symbol: symbol(value),
            trading_day: TradingDay(Date::from_calendar_date(2026, Month::June, 22).expect("valid date")),
            sma_short: None,
            sma_long: None,
            ema_short: None,
            ema_long: None,
            atr: None,
            rsi: None,
            roc,
            average_volume: None,
            volume_ratio: None,
            volume_breakout,
        }
    }

    #[test]
    fn classifies_benchmark_regime_from_roc() {
        let benchmark = features("KOSPI", Some(Decimal::new(550, 2)), None);

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AdvisorContext,
            Some(symbol("KOSPI")),
            Some(&benchmark),
            &[],
            20,
        );

        assert_eq!(context.status, DomesticMarketAlignmentStatus::Computed);
        assert_eq!(context.benchmark_regime, DomesticMarketRegime::StrongUptrend);
        assert!(context.notes.is_empty());
    }

    #[test]
    fn labels_candidates_against_benchmark_roc() {
        let benchmark = features("KOSPI", Some(Decimal::new(550, 2)), None);
        let candidates = vec![
            features("005930", Some(Decimal::new(800, 2)), Some(true)),
            features("000660", Some(Decimal::new(100, 2)), Some(false)),
            features("035720", Some(Decimal::new(-100, 2)), None),
            features("068270", None, None),
        ];

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AdvisorContext,
            Some(symbol("KOSPI")),
            Some(&benchmark),
            &candidates,
            20,
        );

        let labels = context
            .candidates
            .iter()
            .map(|candidate| candidate.alignment_label)
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                DomesticCandidateAlignmentLabel::Aligned,
                DomesticCandidateAlignmentLabel::Weak,
                DomesticCandidateAlignmentLabel::AntiIndex,
                DomesticCandidateAlignmentLabel::Unknown,
            ]
        );
        assert_eq!(context.candidates[0].symbol.as_str(), "005930");
        assert_eq!(context.candidates[0].symbol_roc, Some(Decimal::new(800, 2)));
        assert_eq!(context.candidates[0].benchmark_roc, Some(Decimal::new(550, 2)));
        assert_eq!(context.candidates[0].volume_breakout, Some(true));
    }

    #[test]
    fn fails_soft_when_benchmark_features_are_missing() {
        let candidates = vec![features("005930", Some(Decimal::new(800, 2)), Some(true))];

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AuditOnly,
            Some(symbol("KOSPI")),
            None,
            &candidates,
            20,
        );

        assert_eq!(context.status, DomesticMarketAlignmentStatus::UnknownInsufficientData);
        assert_eq!(context.benchmark_regime, DomesticMarketRegime::UnknownInsufficientData);
        assert_eq!(context.candidates[0].alignment_label, DomesticCandidateAlignmentLabel::Unknown);
        assert_eq!(context.notes, vec!["benchmark_features_missing"]);
    }

    #[test]
    fn serializes_snake_case_and_decimal_strings() {
        let benchmark = features("KOSPI", Some(Decimal::new(550, 2)), None);
        let candidates = vec![features("005930", Some(Decimal::new(800, 2)), Some(true))];

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AdvisorContext,
            Some(symbol("KOSPI")),
            Some(&benchmark),
            &candidates,
            20,
        );
        let json = serde_json::to_value(&context).expect("alignment json");

        assert_eq!(json["status"], "computed");
        assert_eq!(json["mode"], "advisor_context");
        assert_eq!(json["benchmark_regime"], "strong_uptrend");
        assert_eq!(json["candidates"][0]["symbol"], "005930");
        assert_eq!(json["candidates"][0]["symbol_roc"], "8.00");
        assert_eq!(json["candidates"][0]["benchmark_roc"], "5.50");
        assert_eq!(json["candidates"][0]["alignment_label"], "aligned");
    }
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```sh
cargo test -p trade-engine market_alignment --all-features
```

Expected: fails to compile because `compute_domestic_market_alignment`, `DomesticMarketAlignmentMode`, `DomesticMarketAlignmentStatus`, `DomesticMarketRegime`, and `DomesticCandidateAlignmentLabel` are not defined.

- [ ] **Step 3: Implement the minimal module**

Add production code above the test module in `trade-engine/src/market_alignment.rs`:

```rust
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::gateway::types::Symbol;
use crate::technical_analysis::DailyTechnicalFeatures;

pub const DEFAULT_DOMESTIC_ALIGNMENT_FEATURE_SET: &str = "daily-core-v1";
pub const DEFAULT_RELATIVE_STRENGTH_LOOKBACK_DAYS: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomesticMarketAlignmentMode {
    AuditOnly,
    AdvisorContext,
}

impl Default for DomesticMarketAlignmentMode {
    fn default() -> Self {
        Self::AuditOnly
    }
}

impl DomesticMarketAlignmentMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuditOnly => "audit_only",
            Self::AdvisorContext => "advisor_context",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomesticMarketAlignmentStatus {
    Computed,
    UnknownInsufficientData,
}

impl DomesticMarketAlignmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Computed => "computed",
            Self::UnknownInsufficientData => "unknown_insufficient_data",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomesticMarketRegime {
    StrongUptrend,
    Uptrend,
    Range,
    Downtrend,
    UnknownInsufficientData,
}

impl DomesticMarketRegime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StrongUptrend => "strong_uptrend",
            Self::Uptrend => "uptrend",
            Self::Range => "range",
            Self::Downtrend => "downtrend",
            Self::UnknownInsufficientData => "unknown_insufficient_data",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomesticCandidateAlignmentLabel {
    Aligned,
    Weak,
    AntiIndex,
    Unknown,
}

impl DomesticCandidateAlignmentLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::Weak => "weak",
            Self::AntiIndex => "anti_index",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomesticMarketAlignmentContext {
    pub status: DomesticMarketAlignmentStatus,
    pub mode: DomesticMarketAlignmentMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_symbol: Option<Symbol>,
    pub benchmark_regime: DomesticMarketRegime,
    pub candidates: Vec<DomesticCandidateAlignment>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomesticCandidateAlignment {
    pub symbol: Symbol,
    pub relative_strength_lookback_days: u32,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub symbol_roc: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub benchmark_roc: Option<Decimal>,
    pub alignment_label: DomesticCandidateAlignmentLabel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_breakout: Option<bool>,
    pub notes: Vec<String>,
}

pub fn compute_domestic_market_alignment(
    mode: DomesticMarketAlignmentMode,
    benchmark_symbol: Option<Symbol>,
    benchmark_features: Option<&DailyTechnicalFeatures>,
    candidate_features: &[DailyTechnicalFeatures],
    relative_strength_lookback_days: u32,
) -> DomesticMarketAlignmentContext {
    let benchmark_roc = benchmark_features.and_then(|features| features.roc);
    let mut notes = Vec::new();
    if benchmark_symbol.is_none() {
        notes.push("benchmark_symbol_not_configured".to_owned());
    }
    if benchmark_roc.is_none() {
        notes.push("benchmark_features_missing".to_owned());
    }

    let status = if benchmark_roc.is_some() {
        DomesticMarketAlignmentStatus::Computed
    } else {
        DomesticMarketAlignmentStatus::UnknownInsufficientData
    };
    let benchmark_regime = classify_regime(benchmark_roc);
    let candidates = candidate_features
        .iter()
        .map(|features| candidate_alignment(features, benchmark_roc, relative_strength_lookback_days))
        .collect();

    DomesticMarketAlignmentContext {
        status,
        mode,
        benchmark_symbol,
        benchmark_regime,
        candidates,
        notes,
    }
}

fn classify_regime(benchmark_roc: Option<Decimal>) -> DomesticMarketRegime {
    let Some(roc) = benchmark_roc else {
        return DomesticMarketRegime::UnknownInsufficientData;
    };
    if roc >= Decimal::new(500, 2) {
        DomesticMarketRegime::StrongUptrend
    } else if roc >= Decimal::new(200, 2) {
        DomesticMarketRegime::Uptrend
    } else if roc <= Decimal::new(-200, 2) {
        DomesticMarketRegime::Downtrend
    } else {
        DomesticMarketRegime::Range
    }
}

fn candidate_alignment(
    features: &DailyTechnicalFeatures,
    benchmark_roc: Option<Decimal>,
    relative_strength_lookback_days: u32,
) -> DomesticCandidateAlignment {
    let symbol_roc = features.roc;
    let alignment_label = match (symbol_roc, benchmark_roc) {
        (Some(symbol), Some(benchmark)) if benchmark > Decimal::ZERO && symbol < Decimal::ZERO => {
            DomesticCandidateAlignmentLabel::AntiIndex
        }
        (Some(symbol), Some(benchmark)) if symbol >= benchmark => {
            DomesticCandidateAlignmentLabel::Aligned
        }
        (Some(_), Some(_)) => DomesticCandidateAlignmentLabel::Weak,
        _ => DomesticCandidateAlignmentLabel::Unknown,
    };
    let notes = if symbol_roc.is_none() {
        vec!["candidate_roc_missing".to_owned()]
    } else if benchmark_roc.is_none() {
        vec!["benchmark_roc_missing".to_owned()]
    } else {
        Vec::new()
    };

    DomesticCandidateAlignment {
        symbol: features.symbol.clone(),
        relative_strength_lookback_days,
        symbol_roc,
        benchmark_roc,
        alignment_label,
        volume_breakout: features.volume_breakout,
        notes,
    }
}
```

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```sh
cargo test -p trade-engine market_alignment --all-features
```

Expected: `4 passed`.

- [ ] **Step 5: Format and commit**

Run:

```sh
cargo fmt --check
```

If formatting fails, run `cargo fmt`, then rerun `cargo fmt --check`.

Commit:

```sh
git add trade-engine/src/lib.rs trade-engine/src/market_alignment.rs
git commit -m "feat: add domestic market alignment scoring"
```

---

### Task 2: Config and Runtime Settings Plumbing

**Files:**
- Modify: `trade-engine/src/config/mod.rs`
- Modify: `trade-engine/src/runtime/mod.rs`
- Modify: `trade-engine/src/runner.rs`
- Modify: `trade-engine/tests/dry_run_config.rs`
- Modify: `trade-engine/tests/startup_doctor.rs`
- Modify: `trade-engine/src/runner.rs` test helpers

- [ ] **Step 1: Write failing config tests**

In `trade-engine/tests/dry_run_config.rs`, extend the config imports:

```rust
use trade_engine::config::{
    AdvisorProviderKind, BrokerProviderKind, DecisionProviderKind, DomesticMarketAlignmentMode,
    EngineConfig, KisEnvironment, MarketKind, UniverseDiscoveryProviderKind,
};
```

Add these tests near the existing config parsing tests:

```rust
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
    assert_eq!(config.domestic_market_alignment.feature_set, "daily-core-v1");
    assert_eq!(config.domestic_market_alignment.relative_strength_lookback_days, 20);
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
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```sh
cargo test -p trade-engine --test dry_run_config domestic_market_alignment --all-features
```

Expected: fails because `DomesticMarketAlignmentMode`, `domestic_market_alignment`, and `benchmark_symbol()` do not exist.

- [ ] **Step 3: Add config types and validation**

In `trade-engine/src/config/mod.rs`, add imports:

```rust
pub use crate::market_alignment::DomesticMarketAlignmentMode;

use crate::market_alignment::{
    DEFAULT_DOMESTIC_ALIGNMENT_FEATURE_SET, DEFAULT_RELATIVE_STRENGTH_LOOKBACK_DAYS,
};
```

Add this field to `EngineConfig` after `universe_discovery`:

```rust
#[serde(default)]
pub domestic_market_alignment: DomesticMarketAlignmentConfig,
```

Call validation in `validate_common` after `self.universe_discovery.validate()?`:

```rust
self.domestic_market_alignment.validate()?;
```

Add this config struct near `UniverseDiscoveryConfig`:

```rust
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DomesticMarketAlignmentConfig {
    pub enabled: bool,
    pub mode: DomesticMarketAlignmentMode,
    pub benchmark_symbol: Option<String>,
    pub feature_set: String,
    pub relative_strength_lookback_days: u32,
}

impl Default for DomesticMarketAlignmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: DomesticMarketAlignmentMode::AuditOnly,
            benchmark_symbol: None,
            feature_set: DEFAULT_DOMESTIC_ALIGNMENT_FEATURE_SET.to_owned(),
            relative_strength_lookback_days: DEFAULT_RELATIVE_STRENGTH_LOOKBACK_DAYS,
        }
    }
}

impl DomesticMarketAlignmentConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        if self.feature_set.trim().is_empty() {
            return Err(TradeEngineError::config(
                "domestic_market_alignment.feature_set must not be empty",
            ));
        }
        if self.relative_strength_lookback_days == 0 {
            return Err(TradeEngineError::config(
                "domestic_market_alignment.relative_strength_lookback_days must be greater than zero",
            ));
        }
        self.benchmark_symbol()?;
        Ok(())
    }

    pub fn benchmark_symbol(&self) -> Result<Option<Symbol>, TradeEngineError> {
        self.benchmark_symbol
            .as_deref()
            .map(|value| {
                Symbol::new(value.to_owned()).map_err(|source| {
                    TradeEngineError::invalid_broker_value(
                        "domestic_market_alignment.benchmark_symbol",
                        source,
                    )
                })
            })
            .transpose()
    }

    pub fn sends_advisor_context(&self) -> bool {
        self.enabled && self.mode == DomesticMarketAlignmentMode::AdvisorContext
    }
}
```

- [ ] **Step 4: Add runtime settings field**

In `trade-engine/src/runtime/mod.rs`, extend imports:

```rust
use crate::config::{
    AdvisorConfig, AdvisorProviderKind, DecisionConfig, DomesticMarketAlignmentConfig,
    ExitPolicyConfig, RiskConfig, RuntimeConfig, SizingConfig, StrategyConfig,
    UniverseDiscoveryConfig, UniverseDiscoveryProviderKind, decision_source_id,
};
```

Add the field to `DryRunRuntimeSettings` after `universe_discovery`:

```rust
pub domestic_market_alignment: DomesticMarketAlignmentConfig,
```

Set it to `DomesticMarketAlignmentConfig::default()` in `new`, `with_decision`, and `with_risk`.

Add a builder:

```rust
pub fn with_domestic_market_alignment(
    mut self,
    domestic_market_alignment: DomesticMarketAlignmentConfig,
) -> Self {
    self.domestic_market_alignment = domestic_market_alignment;
    self
}
```

Call validation in `DryRunRuntimeSettings::validate` after universe discovery:

```rust
self.domestic_market_alignment.validate()?;
```

- [ ] **Step 5: Carry config from runner**

In each `DryRunRuntimeSettings::with_risk(...).with_*` chain in `trade-engine/src/runner.rs`, add:

```rust
.with_domestic_market_alignment(config.domestic_market_alignment.clone())
```

Use `.clone()` consistently because surrounding chains already clone configs in submit-order paths.

- [ ] **Step 6: Fix direct `EngineConfig` initializers**

Run:

```sh
rg -n "EngineConfig \\{" trade-engine/src trade-engine/tests
```

For every direct initializer, add:

```rust
domestic_market_alignment: Default::default(),
```

Place it after `universe_discovery: Default::default(),` or after the explicit `universe_discovery` field.

- [ ] **Step 7: Run tests and verify GREEN**

Run:

```sh
cargo test -p trade-engine --test dry_run_config domestic_market_alignment --all-features
cargo check -p trade-engine --all-features
```

Expected: config tests pass and `cargo check` succeeds.

- [ ] **Step 8: Format and commit**

Run:

```sh
cargo fmt --check
```

If formatting fails, run `cargo fmt`, then rerun `cargo fmt --check`.

Commit:

```sh
git add trade-engine/src/config/mod.rs trade-engine/src/runtime/mod.rs trade-engine/src/runner.rs trade-engine/tests/dry_run_config.rs trade-engine/tests/startup_doctor.rs trade-engine/tests/decision_pipeline_audit.rs
git add trade-engine/tests/runtime_loop.rs trade-engine/tests/runtime_write_loop.rs trade-engine/tests/storage_audit.rs
git commit -m "feat: add domestic market alignment config"
```

If one of the listed test files was not modified, omit it from `git add`.

---

### Task 3: Advisor Context and Runtime Observation Integration

**Files:**
- Modify: `trade-engine/src/advisor.rs`
- Modify: `trade-engine/src/runtime/mod.rs`
- Modify: `trade-engine/tests/runtime_loop.rs`

- [ ] **Step 1: Write failing runtime request test**

In `trade-engine/tests/runtime_loop.rs`, extend config imports:

```rust
use trade_engine::config::{
    AdvisorConfig, AdvisorProviderKind, DomesticMarketAlignmentConfig,
    DomesticMarketAlignmentMode, RuntimeConfig, StrategyConfig, UniverseDiscoveryConfig,
    UniverseDiscoveryProviderKind,
};
```

Add a helper below `seed_daily_feature_audit`:

```rust
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
```

Add this new async test near `dry_run_loop_runs_command_advisor_and_records_audit`:

```rust
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
    seed_daily_feature_audit_for_symbol(&mut db, "KOSPI", [100, 101, 103, 106]);
    seed_daily_feature_audit_for_symbol(&mut db, "005930", [100, 102, 104, 110]);
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
}
```

Update the existing `dry_run_loop_runs_command_advisor_and_records_audit` schema assertion from `7` to `8` after implementation. Do not change it before the RED run for the new test.

- [ ] **Step 2: Run test and verify RED**

Run:

```sh
cargo test -p trade-engine --test runtime_loop command_advisor_receives_domestic_market_alignment_context_when_enabled --all-features
```

Expected: fails because `with_domestic_market_alignment` may exist from Task 2 but runtime does not compute/render/pass `domestic_market_alignment`, and schema version is still `7`.

- [ ] **Step 3: Add advisor request field**

In `trade-engine/src/advisor.rs`, import the context:

```rust
use crate::market_alignment::DomesticMarketAlignmentContext;
```

Bump:

```rust
pub const ADVISOR_REQUEST_SCHEMA_VERSION: u32 = 8;
```

Add this field to `AdvisorMarketTickObservation` after `technical_analysis`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub domestic_market_alignment: Option<DomesticMarketAlignmentContext>,
```

- [ ] **Step 4: Add runtime observation field and filtering**

In `trade-engine/src/runtime/mod.rs`, import:

```rust
use crate::market_alignment::{
    DomesticMarketAlignmentContext, compute_domestic_market_alignment,
};
```

Add this field to `TickObservation` and `DryRunTickOutput`:

```rust
domestic_market_alignment: Option<DomesticMarketAlignmentContext>,
```

For `DryRunTickOutput`, make it public:

```rust
pub domestic_market_alignment: Option<DomesticMarketAlignmentContext>,
```

Update all `DryRunTickOutput` constructors in `build_dry_run_tick_output` to set:

```rust
domestic_market_alignment: observation.domestic_market_alignment,
```

When a constructor still needs later use of `observation`, clone the option first:

```rust
let domestic_market_alignment = observation.domestic_market_alignment.clone();
```

Update `observation_for_symbols` so it filters candidates inside the alignment context:

```rust
let domestic_market_alignment = observation
    .domestic_market_alignment
    .clone()
    .map(|mut context| {
        context
            .candidates
            .retain(|candidate| selected_symbols.contains(&candidate.symbol));
        context
    });
```

Return the field in `TickObservation`.

- [ ] **Step 5: Compute alignment during observation collection**

Change `collect_tick_observation` signature to accept config:

```rust
domestic_market_alignment_config: &DomesticMarketAlignmentConfig,
```

Update the call from `run_dry_run_market_tick`:

```rust
&context.settings.domestic_market_alignment,
```

After loading `technical_analysis`, call a new helper:

```rust
let domestic_market_alignment = collect_domestic_market_alignment(
    store,
    market_scope,
    &snapshot.quotes,
    domestic_market_alignment_config,
)
.await?;
```

Add this helper near `collect_tick_observation`:

```rust
async fn collect_domestic_market_alignment<S>(
    store: &S,
    market_scope: &MarketScope,
    quotes: &[Quote],
    config: &DomesticMarketAlignmentConfig,
) -> Result<Option<DomesticMarketAlignmentContext>, DryRunTickFailure>
where
    S: DryRunAuditStore,
{
    if !config.enabled || !matches!(market_scope, MarketScope::DomesticStock) {
        return Ok(None);
    }

    let candidate_symbols = quotes
        .iter()
        .map(|quote| quote.symbol.clone())
        .collect::<Vec<_>>();
    let candidate_audits = store
        .latest_technical_features_for_symbols(
            &candidate_symbols,
            OhlcvBarInterval::Daily,
            config.feature_set.clone(),
        )
        .await
        .map_err(|error| {
            DryRunTickFailure::failed(
                "TickObservation.domestic_market_alignment.candidates",
                None,
                error.to_string(),
            )
        })?;
    let candidate_features = candidate_audits
        .into_iter()
        .map(|audit| audit.features)
        .collect::<Vec<_>>();
    let benchmark_symbol = config.benchmark_symbol().map_err(|error| {
        DryRunTickFailure::failed(
            "TickObservation.domestic_market_alignment.config",
            None,
            error.to_string(),
        )
    })?;
    let benchmark_features = if let Some(symbol) = benchmark_symbol.as_ref() {
        store
            .latest_technical_features_for_symbols(
                std::slice::from_ref(symbol),
                OhlcvBarInterval::Daily,
                config.feature_set.clone(),
            )
            .await
            .map_err(|error| {
                DryRunTickFailure::failed(
                    "TickObservation.domestic_market_alignment.benchmark",
                    None,
                    error.to_string(),
                )
            })?
            .into_iter()
            .next()
            .map(|audit| audit.features)
    } else {
        None
    };

    Ok(Some(compute_domestic_market_alignment(
        config.mode,
        benchmark_symbol,
        benchmark_features.as_ref(),
        &candidate_features,
        config.relative_strength_lookback_days,
    )))
}
```

- [ ] **Step 6: Pass context only in advisor context mode**

Change `advisor_market_tick_observation` signature:

```rust
fn advisor_market_tick_observation(
    observation: &TickObservation,
    market_session_open: bool,
    include_domestic_market_alignment: bool,
) -> AdvisorMarketTickObservation
```

Set the field:

```rust
domestic_market_alignment: include_domestic_market_alignment
    .then(|| observation.domestic_market_alignment.clone())
    .flatten(),
```

Add `domestic_market_alignment: &'a DomesticMarketAlignmentConfig` to `TickAdvisoryContext` and update the call in `run_tick_advisory`:

```rust
advisor_market_tick_observation(
    context.observation,
    context.market_session_open,
    context.domestic_market_alignment.sends_advisor_context(),
)
```

Pass `&context.settings.domestic_market_alignment` when constructing `TickAdvisoryContext`.

- [ ] **Step 7: Render alignment in dry-run output**

In `render_dry_run_tick`, after the technical-analysis section and before the blank line, add:

```rust
if let Some(alignment) = &run.domestic_market_alignment {
    output
        .blank()
        .heading("Domestic market alignment")
        .field("status", alignment.status.as_str())
        .field("mode", alignment.mode.as_str())
        .field(
            "benchmark_symbol",
            alignment
                .benchmark_symbol
                .as_ref()
                .map(Symbol::as_str)
                .unwrap_or("none"),
        )
        .field("benchmark_regime", alignment.benchmark_regime.as_str());
    for candidate in &alignment.candidates {
        output.item(
            &candidate.symbol,
            &[
                OutputField::new("label", candidate.alignment_label.as_str()),
                OutputField::new("symbol_roc", optional_decimal_display(candidate.symbol_roc)),
                OutputField::new(
                    "benchmark_roc",
                    optional_decimal_display(candidate.benchmark_roc),
                ),
            ],
        );
    }
}
```

- [ ] **Step 8: Update existing constructors and schema assertions**

Run:

```sh
rg -n "AdvisorMarketTickObservation \\{|DryRunTickOutput \\{|schema_version\"\\], 7" trade-engine/src trade-engine/tests
```

Update every initializer with the new optional field:

```rust
domestic_market_alignment: None,
```

Update schema assertions for advisor request JSON from `7` to `8`.

- [ ] **Step 9: Run tests and verify GREEN**

Run:

```sh
cargo test -p trade-engine --test runtime_loop command_advisor_receives_domestic_market_alignment_context_when_enabled --all-features
cargo test -p trade-engine --test runtime_loop dry_run_loop_runs_command_advisor_and_records_audit --all-features
```

Expected: both tests pass.

- [ ] **Step 10: Format and commit**

Run:

```sh
cargo fmt --check
```

If formatting fails, run `cargo fmt`, then rerun `cargo fmt --check`.

Commit:

```sh
git add trade-engine/src/advisor.rs trade-engine/src/runtime/mod.rs trade-engine/tests/runtime_loop.rs
git commit -m "feat: include domestic alignment in advisor context"
```

---

### Task 4: Audit Persistence, Docs, and Verification

**Files:**
- Modify: `trade-engine/src/storage/mod.rs`
- Modify: `trade-engine/tests/runtime_loop.rs`
- Modify: `README.md`

- [ ] **Step 1: Write failing persistence assertion**

In the new runtime-loop test from Task 3, after reading the advisor request, add:

```rust
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
```

- [ ] **Step 2: Run test and verify RED**

Run:

```sh
cargo test -p trade-engine --test runtime_loop command_advisor_receives_domestic_market_alignment_context_when_enabled --all-features
```

Expected: fails because completed tick `status_detail_json` is still `{}` or does not contain `domestic_market_alignment`.

- [ ] **Step 3: Persist completed tick detail JSON**

In `trade-engine/src/storage/mod.rs`, change `complete_dry_run_tick` to build detail JSON before `update_tick`:

```rust
let status_detail_json = completed_tick_status_detail(output)?;
update_tick_with_detail(
    &transaction,
    tick_id,
    output.terminal_status(),
    None,
    &completed_at,
    &status_detail_json,
)?;
```

Add this helper near `complete_dry_run_tick`:

```rust
fn completed_tick_status_detail(output: &DryRunTickOutput) -> Result<String, TradeEngineError> {
    let mut detail = serde_json::Map::new();
    detail.insert(
        "status".to_owned(),
        serde_json::Value::String(output.terminal_status().as_str().to_owned()),
    );
    if let Some(alignment) = output.domestic_market_alignment.as_ref() {
        detail.insert(
            "domestic_market_alignment".to_owned(),
            serde_json::to_value(alignment).map_err(|error| {
                TradeEngineError::storage(format!("failed to encode domestic alignment: {error}"))
            })?,
        );
    }
    serde_json::to_string(&detail)
        .map_err(|error| TradeEngineError::storage(format!("failed to encode tick detail: {error}")))
}
```

- [ ] **Step 4: Run persistence test and verify GREEN**

Run:

```sh
cargo test -p trade-engine --test runtime_loop command_advisor_receives_domestic_market_alignment_context_when_enabled --all-features
```

Expected: test passes and the stored tick detail contains the alignment context.

- [ ] **Step 5: Document the config**

In `README.md`, add a short subsection near the existing dry-run/config documentation:

````markdown
### Domestic Market Alignment

Domestic market alignment is disabled by default. When enabled, the runtime can
compute domestic benchmark-relative context from stored daily technical feature
audits. In `advisor_context` mode, the context is included in the structured
advisor request and recorded in runtime tick detail JSON. It does not change
order sizing, risk blocks, or execution behavior.

```toml
[domestic_market_alignment]
enabled = true
mode = "advisor_context" # audit_only | advisor_context
benchmark_symbol = "KOSPI"
feature_set = "daily-core-v1"
relative_strength_lookback_days = 20
```
````

- [ ] **Step 6: Run focused test suite**

Run:

```sh
cargo test -p trade-engine market_alignment --all-features
cargo test -p trade-engine --test dry_run_config domestic_market_alignment --all-features
cargo test -p trade-engine --test runtime_loop command_advisor_receives_domestic_market_alignment_context_when_enabled --all-features
cargo test -p trade-engine --test runtime_loop dry_run_loop_runs_command_advisor_and_records_audit --all-features
```

Expected: all focused tests pass.

- [ ] **Step 7: Run required project verification**

Run the repository-required Rust checks:

```sh
cargo check --workspace --no-default-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

Expected: all commands pass. If a command fails due to a pre-existing unrelated issue, capture the exact failure and stop before committing the final task unless the user explicitly approves preserving the failure.

- [ ] **Step 8: Inspect final diff**

Run:

```sh
git diff --stat
git diff -- trade-engine/src/market_alignment.rs trade-engine/src/config/mod.rs trade-engine/src/runtime/mod.rs trade-engine/src/advisor.rs trade-engine/src/storage/mod.rs trade-engine/src/runner.rs trade-engine/tests/dry_run_config.rs trade-engine/tests/runtime_loop.rs README.md
```

Expected: diff contains only domestic market-alignment first-slice changes.

- [ ] **Step 9: Commit**

Commit:

```sh
git add trade-engine/src/storage/mod.rs trade-engine/tests/runtime_loop.rs README.md
git commit -m "feat: audit domestic alignment in runtime ticks"
```

## Self-Review

- Spec coverage:
  - Pure deterministic context: Task 1.
  - Disabled-by-default config: Task 2.
  - Advisor context only in `advisor_context` mode: Task 3.
  - Audit visibility without new migration: Task 4 via `runtime_ticks.status_detail_json`.
  - No execution/risk/sizing behavior change: no task touches risk decisions or order submission logic except carrying read-only context.
- Placeholder scan:
  - No incomplete markers or unspecified error-handling steps.
- Type consistency:
  - Config uses `DomesticMarketAlignmentMode`.
  - Runtime and advisor use `DomesticMarketAlignmentContext`.
  - Tests assert schema version `8` after the optional advisor field is added.
