# Domestic Market Alignment Improvement Plan

> **For Hermes:** Use this as the planning baseline for workstream 1: improving Korean domestic-stock account operation. Implement incrementally and keep all live-impacting behavior behind deterministic policy/config and audit records.

**Goal:** Improve domestic-stock account behavior so new buys do not systematically drift away from KOSPI/KOSDAQ and leading-sector momentum during strong domestic market regimes, without issuing direct stock-specific commands or making abrupt discretionary portfolio turns.

**Architecture:** Add a domestic market/sector alignment layer around the existing universe/advisor/risk pipeline. The layer should compute auditable market-regime, sector-leadership, relative-strength, and portfolio-exposure features; feed them into universe/advisor context; and enforce only bounded deterministic throttles/caps through risk/policy.

**Tech Stack:** Rust `trade-engine`, SQLite Main DB migrations, existing `technical_analysis`, `universe`, `advisor`, `risk`, `runtime`, `config`, KIS/Toss read-only market data where available.

---

## Problem Statement

The overseas account appears acceptable, but the Korean domestic account has recently bought many stocks unrelated to the parts of the Korean market that are rising sharply, such as semiconductor-led KOSPI strength. The desired fix is not “buy this specific stock” or “force-sell everything else.” The desired fix is a systematic bias correction:

- recognize domestic market regime,
- identify leading sectors/themes,
- penalize candidates that are persistently anti-index or outside leadership during strong uptrends,
- reduce allocation rather than hard-block in ambiguous cases,
- keep all changes explainable and auditable.

## Core Principles

- Do not let an LLM directly change positions or submit orders.
- Prefer overlay scoring, throttling, and caps over hard symbol instructions.
- Domestic-specific behavior must be opt-in through config/policy and audit-visible.
- Existing advisor/universe signals should not be thrown away; they should be re-ranked or sized with market-alignment context.
- Risk/policy remains the final deterministic authority.
- Start read-only/dry-run first, compare against current behavior, then consider live-impacting use.

## High-Level Design

Add four layers:

1. **Market Regime Layer** — classifies domestic market state from KOSPI/KOSDAQ and breadth-like inputs.
2. **Sector Leadership Layer** — maps symbols to sector buckets and identifies current leading/lagging sectors.
3. **Candidate Alignment Layer** — scores each universe/advisor candidate for index alignment, sector leadership, relative strength, liquidity, and portfolio fit.
4. **Policy Throttle Layer** — converts alignment results into allocation multipliers, caps, and risk block reasons where appropriate.

This should change behavior gradually:

```text
advisor_score
+ market_alignment_score
+ sector_leadership_score
+ relative_strength_score
- anti_index_penalty
- low_liquidity_penalty
- concentration_penalty
=> adjusted_candidate_score + sizing_multiplier + policy_notes
```

## Phase 0: Baseline Audit Before Changing Behavior

Objective: prove the suspected issue with recent data before implementing behavior changes.

Tasks:

- Add or build a read-only report over recent domestic decisions.
- For each candidate/proposal/buy, compute:
  - KOSPI/KOSDAQ return over 1D/5D/20D if data exists,
  - symbol return over the same windows,
  - relative strength vs benchmark,
  - sector bucket,
  - whether the sector was leading/neutral/lagging,
  - buy size and later relative outcome.
- Summarize repeated error classes:
  - strong-index day but low-relative-strength buy,
  - strong semiconductor/sector leadership ignored,
  - low-liquidity or low-beta name selected in momentum regime,
  - excessive non-leading-sector new buys,
  - portfolio exposure moving away from market leadership.

Deliverable:

- A dry-run/operator report, not a trading behavior change.

Likely code areas:

- `trade-engine/src/storage/mod.rs`
- `trade-engine/src/runtime/mod.rs`
- `trade-engine/src/advisor.rs`
- possible new module: `trade-engine/src/market_alignment.rs`

## Phase 1: Domestic Market Regime Features

Objective: add deterministic domestic market context that advisor/universe can see.

Initial regime inputs:

- KOSPI index return: 1D/5D/20D/60D
- KOSDAQ index return: 1D/5D/20D/60D
- index trend state: above/below 20D and 60D moving averages
- index volatility or ATR proxy
- optional breadth proxy if data is available later
- optional foreigner/institution flow if data source becomes available

Initial regime labels:

```text
strong_uptrend
uptrend
range
downtrend
panic_or_high_volatility
unknown_insufficient_data
```

Rules should be boring and testable, for example:

```text
strong_uptrend if KOSPI 20D > +5%, close > 20D SMA, and close > 60D SMA
uptrend        if KOSPI 20D > +2% and close > 20D SMA
range          if neither uptrend nor downtrend
```

Data concern:

- Treat KOSPI/KOSDAQ as first-class benchmark symbols or market-index records.
- If index data is missing, fail soft to `unknown_insufficient_data` and do not penalize candidates.

Likely code areas:

- New: `trade-engine/src/market_alignment.rs`
- Extend: `trade-engine/src/technical_analysis.rs`
- Extend DB migrations for index/market feature audit if current OHLCV storage cannot represent index bars cleanly.

## Phase 2: Sector Classification and Leadership

Objective: make “semiconductor-led rally” visible to the system without hardcoding individual stock instructions.

Minimum viable sector model:

- Add a simple symbol-to-sector map for Korean stocks.
- Use broad sector buckets first:
  - semiconductor
  - battery_ev
  - financial
  - shipbuilding
  - defense
  - power_grid/electrical_equipment
  - platform/internet
  - bio_healthcare
  - consumer
  - industrial
  - unknown
- Store source and freshness for sector classification.

Leadership scoring:

- Compute sector return and relative return vs KOSPI over 5D/20D.
- Rank sectors by return, volume/turnover proxy, and breadth if available.
- Label sectors:
  - leading
  - improving
  - neutral
  - lagging
  - unknown

Avoid overfitting:

- Do not create tiny theme buckets too early.
- Do not encode “always buy semiconductor.” Encode “prefer leading sectors in strong uptrends.”

Likely code areas:

- New: `trade-engine/src/market_alignment.rs`
- New config section: `[domestic_market_alignment]`
- New data/config artifact for symbol-sector mapping if no broker API field exists.

## Phase 3: Candidate Alignment Scoring

Objective: adjust ranking/sizing before execution, without making the advisor brittle.

Candidate-level features:

- `benchmark_relative_return_5d/20d`
- `sector_relative_return_5d/20d`
- `is_leading_sector`
- `is_lagging_sector`
- `anti_index_flag`: index up strongly while symbol is flat/down
- `liquidity_flag`: low volume/turnover or stale price
- `portfolio_exposure_delta`: whether buying increases/decreases exposure to leadership

Scoring direction:

```text
market_alignment_score = weighted_sum(...)
sizing_multiplier = clamp(0.2, 1.5, f(score, regime))
```

Example policy behavior:

```text
if regime == strong_uptrend:
    boost leading sectors modestly
    throttle anti-index candidates
    cap new non-leading-sector exposure
elif regime == downtrend:
    reduce momentum-following boosts
    tighten position/notional caps
else:
    keep advisor score mostly unchanged
```

The advisor request should include a compact market-alignment context, for example:

```json
{
  "domestic_market_alignment": {
    "regime": "strong_uptrend",
    "benchmark": "KOSPI",
    "leading_sectors": ["semiconductor", "power_grid", "financial"],
    "candidate_alignment": [
      {
        "symbol": "005930",
        "sector": "semiconductor",
        "relative_strength_20d": "0.08",
        "sector_state": "leading",
        "alignment_label": "aligned"
      }
    ]
  }
}
```

Likely code areas:

- `trade-engine/src/advisor.rs`: schema version bump and request context extension.
- `trade-engine/src/universe.rs`: optional request/output evidence extension.
- `trade-engine/src/runtime/mod.rs`: compute and pass context.
- DB audit for computed features and final adjusted score.

## Phase 4: Deterministic Policy Throttles

Objective: make the final behavior safe and bounded.

Initial config knobs:

```toml
[domestic_market_alignment]
enabled = true
mode = "audit_only" # audit_only | advisor_context | sizing_throttle | risk_enforced
benchmark = "KOSPI"
leading_sector_boost_bps = 1500
anti_index_penalty_bps = 3000
min_sizing_multiplier = "0.20"
max_sizing_multiplier = "1.25"
max_non_leading_new_buy_fraction = "0.20"
relative_strength_lookback_days = 20
strong_uptrend_min_benchmark_return = "0.05"
```

Rollout modes:

1. `audit_only`: compute and store, do not affect advisor/risk.
2. `advisor_context`: pass features to advisor only, no deterministic throttle.
3. `sizing_throttle`: adjust proposal notional/quantity multiplier deterministically.
4. `risk_enforced`: block or cap proposals that violate hard policy.

Recommended first live-impacting behavior:

- Do **not** hard-block all non-leading sectors.
- Instead, cap new-buy notional for anti-index/non-leading candidates during strong uptrends.
- Require clear audit reason on every throttle.

Possible risk block/throttle reasons:

```text
DomesticAntiIndexCandidateThrottled
DomesticNonLeadingSectorExposureCap
DomesticMarketRegimeUnknownNoAdjustment
DomesticSectorUnknownNoAdjustment
```

Likely code areas:

- `trade-engine/src/config/mod.rs`
- `trade-engine/src/risk/mod.rs`
- `trade-engine/src/runtime/mod.rs`
- `trade-engine/src/storage/mod.rs`

## Phase 5: Portfolio Exposure Feedback

Objective: avoid optimizing only one order at a time.

Add account-level checks:

- current sector exposure by notional,
- new-buy sector exposure by day/week,
- leading vs non-leading exposure ratio,
- benchmark-relative portfolio behavior,
- repeated underperformance from specific error classes.

Policy examples:

```text
In strong_uptrend, at most 20% of new domestic buy notional can go to lagging/unknown sectors unless the advisor marks a clear defensive/mean-reversion thesis and risk allows exploration size only.
```

This layer should inform both chat feedback later and deterministic risk/sizing.

## Testing Strategy

Unit tests:

- regime classification from synthetic KOSPI/KOSDAQ bars,
- sector leadership ranking,
- candidate alignment scoring,
- unknown/missing data fail-soft behavior,
- sizing multiplier clamp behavior,
- config parsing and deny-unknown-fields behavior,
- risk throttle/block reason serialization.

Integration tests:

- domestic dry-run with `audit_only` produces identical proposals but extra audit records,
- `advisor_context` includes compact alignment JSON,
- `sizing_throttle` reduces anti-index candidate notional in strong uptrend,
- overseas configs are unaffected,
- unknown sector/index data does not create false blocks.

Verification commands when tools are available:

```sh
cargo fmt --check
cargo test --all-features market_alignment
cargo test --all-features domestic
cargo test --all-features
```

## Rollout Recommendation

1. Start with **Phase 0 + Phase 1 in audit-only mode**.
2. Run against recent domestic dry-run/live audit history and inspect whether it catches the observed issue.
3. Add sector leadership only after the benchmark-relative signal is trustworthy.
4. Enable advisor context before deterministic throttling.
5. Enable sizing throttle with conservative caps.
6. Keep risk-enforced hard blocks for much later, only for clear safety cases.

## Non-goals

- Force-buying semiconductor or any named stock.
- Automatically liquidating existing non-leading holdings.
- Replacing advisor/universe with a single momentum strategy.
- Letting chat or LLM mutate live policy without deterministic config/audit.
- Overfitting to one recent Korean market rally.

## Open Questions

- What data source should provide reliable KOSPI/KOSDAQ index OHLCV?
- Do we have a trustworthy sector classification source, or should we start with a checked-in manual mapping for major symbols?
- Should the first benchmark be only KOSPI, or KOSPI for KOSPI-listed names and KOSDAQ for KOSDAQ-listed names?
- Should current holdings be actively rebalanced later, or should the first version affect only new buys?
- How should “exploration position” size be defined: fraction of normal order, fraction of account, or hard notional cap?
