# Domestic Account Operation Improvement Plan

## Status

- Date: 2026-06-22
- State: planning only
- Priority: immediate planning target before implementing domestic-account
  behavior changes
- Related:
  - `plans/IMPROVEMENT_PLAN_TECHNICAL_ANALYSIS.md`
  - `plans/CHAT_FEEDBACK_LOOP_PLAN.md`

## Why This Plan Exists

The overseas account behavior appears acceptable for now, but the domestic
account is buying too many stocks that are weakly aligned with the current
KOSPI-led move. In strong domestic rallies, especially when leadership is
concentrated in sectors such as semiconductors, the current process appears too
willing to buy names that are disconnected from index direction, sector
leadership, or relative-strength context.

This is not primarily a "pick better stocks" problem. It is a portfolio and
decision-pipeline alignment problem:

- the system can surface candidates without enough domestic market-regime
  awareness,
- counter-trend or lagging names may survive too deep into the pipeline,
- there is not yet a clear domestic-specific penalty for buying against strong
  market leadership,
- the system lacks an explicit distinction between exploratory contrarian buys
  and core trend-aligned buys.

## Goal

Improve domestic-account behavior without introducing abrupt discretionary
overrides or direct stock-picking instructions.

The system should:

1. detect when the domestic market is in a strong, concentrated, trend-led
   regime,
2. measure how aligned each candidate is with that regime,
3. penalize but not necessarily ban weakly aligned or counter-trend buys,
4. reduce size or require stronger evidence for contrarian entries,
5. preserve auditability and deterministic risk authority.

## Non-Goals

- Do not introduce an LLM-only rule that directly commands specific buys.
- Do not hard-rotate the portfolio mechanically into one sector without risk
  controls.
- Do not replace existing execution/risk authority with chat or prompt logic.
- Do not assume that all divergence from the index is bad; some divergence may
  still be valid as controlled exploration.

## Current Repository Reality

The current codebase already has useful foundations:

- market scope boundaries for domestic vs overseas,
- strategy/risk/execution separation,
- universe signals such as `relative_strength` and `volume_breakout`,
- deterministic execution and audit boundaries,
- a technical-analysis improvement plan that already identifies data and feature
  gaps.

But for this specific domestic-account problem, the main gaps are:

1. No explicit domestic market-regime layer
   There is no first-class feature set for KOSPI trend strength, breadth,
   sector leadership concentration, or benchmark-relative direction.

2. No candidate-to-regime alignment score
   Candidate evaluation does not yet produce a deterministic score for "how
   compatible is this idea with the current domestic regime?"

3. No soft counter-trend penalty framework
   The system lacks a clear mechanism to reduce size, raise thresholds, or
   delay entry when a candidate fights the dominant market move.

4. No domestic-account-specific operating profile
   Domestic behavior likely needs account- and market-specific constraints
   rather than global strategy changes.

## Design Principles

1. Prefer soft constraints before hard bans.
   Counter-trend ideas should usually be throttled, down-weighted, or held to a
   higher evidentiary bar before they are fully prohibited.

2. Keep domestic logic deterministic and auditable.
   Regime classification, sector leadership, and candidate penalties should be
   recorded as structured evidence, not hidden inside advisor prose.

3. Separate market context from stock-specific conviction.
   A stock can look attractive on its own and still be poorly timed in the
   current domestic market context. Both dimensions should be visible.

4. Distinguish exploratory entries from core entries.
   If a contrarian domestic buy survives, it should often enter as a smaller,
   explicitly exploratory position.

5. Validate behavior before live influence.
   Changes should start as shadow scoring or dry-run gating before affecting
   live domestic sizing or eligibility.

## Proposed Operating Model

### 1. Domestic market regime layer

Add a domestic-only regime model that classifies each run or tick using bounded,
deterministic features such as:

- KOSPI short/medium trend direction
- KOSDAQ short/medium trend direction
- market breadth proxy
- sector leadership concentration
- semiconductor and other leading-sector relative strength
- volatility or dispersion regime

Possible early regime labels:

- `trend_up_broad`
- `trend_up_narrow_leadership`
- `range_mixed`
- `risk_off_downtrend`

The first useful version does not need to be perfect. It only needs to be
stable enough to distinguish "strong domestic trend led by a narrow set of
leaders" from "mixed/noisy tape".

### 2. Candidate alignment scoring

For each domestic candidate, compute a deterministic alignment score using
features such as:

- relative strength versus KOSPI
- relative strength versus its sector
- whether the symbol belongs to a leading or lagging sector
- whether the candidate confirms the current market regime or fights it
- whether the idea is breakout/continuation versus mean-reversion/catch-up
- whether liquidity and volume support the thesis

This score should not decide alone. It should become a visible input to risk
and decision policy.

### 3. Soft penalty and throttling layer

Instead of directly blocking all non-aligned candidates, add policy outcomes
like:

- raise minimum confidence threshold for non-aligned buys,
- reduce target position size for counter-trend candidates,
- cap the number of exploratory counter-trend entries per day or per run,
- require multiple supporting evidence classes before a contrarian buy is
  allowed,
- prefer hold/watchlist outcomes over immediate buy when alignment is weak.

This is the core mechanism for improving behavior without abrupt manual
directional overrides.

### 4. Advisor and universe role clarification

Universe and advisor components may still surface contrarian names, but the
domestic account should no longer treat them the same as trend-aligned names.

Operationally:

- universe can continue broad discovery,
- advisor can continue explaining contrarian upside,
- deterministic domestic policy should decide whether the idea becomes:
  - core buy candidate,
  - exploratory small-size candidate,
  - watch-only candidate,
  - rejected candidate.

### 5. Domestic-account-specific policy profile

This work should likely result in domestic-only policy/config knobs rather than
global behavior changes for every market.

Examples:

- counter-trend penalty multiplier
- leading-sector alignment boost
- exploratory position max notional
- non-aligned candidate daily cap
- stronger evidence threshold during narrow-leadership rallies

## Phased Plan

### Phase 1: Audit and diagnosis

Objective: prove where the current domestic flow is going wrong.

- Review recent domestic buys and near-buys.
- Classify them by:
  - market regime at decision time,
  - sector alignment,
  - relative-strength posture,
  - advisor rationale type,
  - eventual short-horizon outcome.
- Produce a simple failure taxonomy:
  - bought laggard during leader-led rally,
  - bought counter-trend mean reversion too early,
  - bought weak sector without enough confirming evidence,
  - ignored stronger aligned alternatives.

Deliverable:

- a small evidence summary showing the dominant failure modes in domestic buys.

### Phase 2: Regime and alignment feature design

Objective: define the minimum deterministic features needed to represent the
problem.

- Define domestic regime inputs and labels.
- Define candidate alignment features and score semantics.
- Decide freshness requirements and fallback behavior for missing inputs.
- Record where each feature comes from and whether current data sources support
  it.

Deliverable:

- a feature glossary and data-availability checklist for domestic regime and
  alignment scoring.

### Phase 3: Shadow scoring

Objective: compute domestic regime and alignment without changing execution.

- Add scoring in dry-run or audit-only mode.
- Compare:
  - what the system currently buys,
  - what it would down-rank,
  - what it would classify as exploratory only.
- Measure how often the new layer catches obviously weak domestic buys without
  discarding useful opportunities.

Deliverable:

- shadow-run report showing candidate score distributions and likely policy
  effects.

### Phase 4: Soft policy integration

Objective: let the new signals influence domestic decision policy in bounded
ways.

- Add throttles, confidence thresholds, or size reductions.
- Keep full audit records for:
  - regime label,
  - alignment score,
  - applied penalties,
  - final decision path.
- Start with dry-run or paper-only influence.

Deliverable:

- domestic-only policy integration with explicit audit visibility.

### Phase 5: Validation and promotion

Objective: confirm that the change improves domestic behavior rather than merely
  following recent winners.

- Compare before/after on:
  - hit rate,
  - benchmark-relative return,
  - drawdown,
  - turnover,
  - concentration,
  - missed-upside cases,
  - number of low-alignment buys avoided.
- Review for overfitting to one recent KOSPI regime.

Deliverable:

- promotion decision for whether the policy can influence live domestic trading.

## What Likely Needs to Change in the Product

This plan suggests changes in or around:

- market data and technical feature storage,
- domestic benchmark or sector context inputs,
- decision/risk policy for candidate eligibility and sizing,
- audit output for regime/alignment/penalty evidence,
- config surface for domestic-only policy knobs,
- reporting or review tools for domestic decision analysis.

It does not imply changing the `kis-client` product boundary.

## Suggested First Implementation Slice

The smallest useful slice is probably:

1. define a domestic regime summary,
2. compute a simple candidate alignment score,
3. record both in audit output,
4. apply one conservative soft penalty:
   reduce size or raise threshold for low-alignment domestic buys.

That is enough to test whether the main issue is actually poor market
alignment, without prematurely building a large indicator framework.

## Open Questions

1. What is the first acceptable benchmark set for domestic alignment:
   KOSPI only, KOSPI plus KOSDAQ, or benchmark plus sector indices?
2. How should sector leadership be sourced and updated?
3. Which contrarian setups should remain intentionally allowed as exploratory
   positions?
4. Should the first penalty act on ranking, threshold, size, or all three?
5. What is the minimum validation window needed before live domestic impact is
   acceptable?

## Recommendation

Treat this workstream as a domestic policy and feature-layer improvement, not a
wholesale strategy rewrite.

The practical sequence should be:

1. diagnose recent domestic misalignment,
2. define regime and candidate-alignment features,
3. run shadow scoring,
4. introduce one or two conservative soft penalties,
5. validate before any live promotion.
