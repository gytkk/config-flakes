# Domestic Market Alignment First Slice Design

## Status

- Date: 2026-06-22
- State: approved design for implementation planning
- Source plans:
  - `plans/DOMESTIC_MARKET_ALIGNMENT_PLAN.md`
  - `plans/DOMESTIC_ACCOUNT_OPERATION_IMPROVEMENT_PLAN.md`

## Goal

Start implementing domestic-account market alignment without changing order
sizing, risk blocking, or execution behavior.

The first slice computes a deterministic domestic market-alignment context and,
when configured, includes it in the advisor request as structured evidence. It
is an audit and advisor-context feature only.

## Non-Goals

- Do not change live order submission behavior.
- Do not add sizing throttles, risk blocks, or execution caps.
- Do not introduce direct stock-specific instructions.
- Do not require a new broker API endpoint for the first slice.
- Do not reshape `kis-client` or move product runtime ownership into it.

## Current Code Context

The repository already has the boundaries needed for a safe first slice:

- `MarketScope::DomesticStock` separates domestic and overseas runtime scope.
- `trade-engine/src/runtime/mod.rs` builds `TickObservation` and advisor request
  JSON.
- `trade-engine/src/advisor.rs` defines the structured advisor request schema.
- `trade-engine/src/technical_analysis.rs` already computes daily features such
  as moving averages, rate of change, and volume breakout.
- `trade-engine/src/storage/mod.rs` already persists OHLCV bars and technical
  feature audits.
- Runtime tests already inspect advisor request JSON.

## Architecture

Add a focused `trade-engine/src/market_alignment.rs` module. The module owns
domestic market-alignment domain types and pure calculation logic.

Runtime integration remains shallow:

1. `collect_tick_observation` gathers the latest technical feature evidence
   already available for the quoted symbols.
2. If domestic market alignment is enabled and the market scope is domestic,
   runtime computes a `DomesticMarketAlignmentContext`.
3. If mode is `advisor_context`, runtime includes that context in
   `AdvisorMarketTickObservation`.
4. If data is missing or the market is overseas, the system fails soft:
   behavior is unchanged and no orders are blocked.

No execution, broker write, or risk-policy path consumes the new context in
this slice.

## Configuration

Add a new config section:

```toml
[domestic_market_alignment]
enabled = false
mode = "audit_only" # audit_only | advisor_context
benchmark_symbol = "KOSPI"
feature_set = "daily-core-v1"
```

Default behavior must preserve the current product behavior:

- `enabled = false`
- `mode = "audit_only"`
- `feature_set = "daily-core-v1"`
- `benchmark_symbol = null`

When `enabled = true` and `mode = "advisor_context"`, the advisor request may
include `current_state.current_tick.domestic_market_alignment`.

## Data Model

Use existing technical feature audits for the first slice instead of adding a
new migration.

The context contains:

- `status`: `computed` or `unknown_insufficient_data`
- `mode`: configured alignment mode
- `benchmark_symbol`: optional configured benchmark symbol
- `benchmark_regime`: `strong_uptrend`, `uptrend`, `range`, `downtrend`, or
  `unknown_insufficient_data`
- `candidates`: one entry per candidate with available technical features
- `notes`: short machine-readable notes for missing data or fallback decisions

Candidate entries contain:

- `symbol`
- `relative_strength_lookback_days`
- `symbol_roc`
- `benchmark_roc`
- `alignment_label`: `aligned`, `weak`, `anti_index`, or `unknown`
- `volume_breakout`
- `notes`

All decimal values must serialize as strings, matching existing advisor and
technical-analysis JSON style.

## Initial Rules

The first rules are intentionally simple and testable:

- If no benchmark symbol is configured or benchmark features are missing,
  benchmark regime is `unknown_insufficient_data`.
- If benchmark ROC is at least `5.00`, regime is `strong_uptrend`.
- If benchmark ROC is at least `2.00`, regime is `uptrend`.
- If benchmark ROC is less than or equal to `-2.00`, regime is `downtrend`.
- Otherwise, regime is `range`.

Candidate labels:

- `unknown` when candidate ROC is missing or benchmark ROC is missing.
- `anti_index` when benchmark ROC is positive and candidate ROC is negative.
- `aligned` when candidate ROC is greater than or equal to benchmark ROC.
- `weak` otherwise.

These rules are not meant to be final trading logic. They create auditable
evidence for comparing current domestic decisions against market alignment.

## Advisor Request Contract

Increment `ADVISOR_REQUEST_SCHEMA_VERSION` because command advisors receive a
new optional structured field.

The new field is optional:

```json
{
  "current_state": {
    "current_tick": {
      "domestic_market_alignment": {
        "status": "computed",
        "mode": "advisor_context",
        "benchmark_symbol": "KOSPI",
        "benchmark_regime": "strong_uptrend",
        "candidates": [
          {
            "symbol": "005930",
            "relative_strength_lookback_days": 20,
            "symbol_roc": "8.00",
            "benchmark_roc": "5.50",
            "alignment_label": "aligned",
            "volume_breakout": true,
            "notes": []
          }
        ],
        "notes": []
      }
    }
  }
}
```

When disabled, overseas, or not in `advisor_context` mode, this field is absent.

## Error Handling

- Missing benchmark features produce `unknown_insufficient_data`, not an error.
- Missing candidate features omit that candidate or mark it `unknown`,
  depending on what data is available.
- Invalid config values fail during config validation.
- Alignment computation must not panic on empty inputs.

## Testing Strategy

Use TDD for implementation.

Unit tests:

- benchmark regime classification from synthetic technical features.
- candidate labels for aligned, weak, anti-index, and unknown cases.
- fail-soft behavior when benchmark data is missing.
- serialization uses snake_case enums and string decimals.

Config tests:

- defaults keep alignment disabled.
- valid `advisor_context` config parses.
- unknown mode fails through serde/config validation.

Runtime tests:

- command advisor request includes domestic alignment only when enabled and
  mode is `advisor_context`.
- disabled config preserves the existing request shape.
- overseas market does not include domestic alignment.

## Rollout

This slice is safe to run in dry-run first. It provides structured evidence for
the later sizing-throttle and policy work without affecting live writes.

Later slices can add:

- dedicated alignment audit tables if the existing advisor audit is not enough,
- sector classification and leadership scoring,
- deterministic sizing throttles,
- risk-enforced caps for clear safety cases.
