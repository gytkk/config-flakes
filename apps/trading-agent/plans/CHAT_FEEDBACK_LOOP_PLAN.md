# Chat and Feedback Loop Plan

> **For Hermes:** This is a direction-setting plan only. Do not implement until the domestic-account operation improvement work is prioritized and the required policy boundaries are confirmed.

## Status

- Date: 2026-06-22
- State: recorded only, not scheduled for implementation
- Priority: after domestic-account operation improvement workstream
- Primary purpose: preserve the product direction for future design and review

**Goal:** Build a chat-driven feedback loop that lets the user review universe/advisor decisions, express portfolio-direction preferences in natural language, and convert approved feedback into auditable policy/config changes.

**Architecture:** Hermes or another chat surface should act as the operator interface, not the trading authority. The Rust `trade-engine` remains the system of record and deterministic executor: advisor/universe events are stored as structured audit records, chat summarizes and asks for feedback, and only explicit policy/config artifacts are consumed by future advisor/risk runs.

**Tech Stack:** Rust `trade-engine`, SQLite Main DB, existing advisor command boundary, optional Hermes chat integration, TOML/YAML/JSON policy config, append-only audit tables.

---

## Scope

This plan covers the second requested workstream: a general interaction layer over universe/advisor history. It intentionally does **not** implement direct stock-specific buy/sell instructions, automatic live policy mutation, or LLM-controlled execution.

## Product Direction

The chat loop should answer three operator questions:

1. **What did the system think?** Show recent universe candidates, advisor proposals, rejected ideas, risk/policy outcomes, and evidence.
2. **What went wrong or right?** Attribute decisions against account, market, sector, relative strength, and later outcomes.
3. **What should change next?** Let the user approve bounded policy adjustments such as weights, caps, throttles, review reminders, or account-specific preferences.

The output of chat feedback should be a durable, reviewable policy artifact, not hidden prompt drift.

## Core Principles

- LLM/chat never submits orders and never bypasses risk/policy.
- Feedback is stored as structured operator intent with scope, strength, expiry, and audit trail.
- Preference changes are proposed first, then applied only through explicit config/policy records.
- All advisor/universe context shown to the user must be traceable to Main DB records.
- Account-specific preferences are allowed; live trading impact requires conservative defaults and clear review points.

## Data Model Direction

Add or extend audit records so each universe/advisor decision can be reconstructed:

```json
{
  "run_id": "...",
  "tick_id": "...",
  "account_scope": "kr-live|kr-paper|us-live|us-paper",
  "market_scope": "KR|US",
  "symbol": "005930",
  "event_type": "universe_candidate|advisor_proposal|risk_rejection|operator_feedback|policy_change",
  "advisor_reason": "...",
  "evidence": ["..."],
  "scores": {
    "advisor": 0.72,
    "market_alignment": 0.81,
    "relative_strength": 0.77
  },
  "decision": "candidate_buy|hold|reject|throttle",
  "outcome_window": "5d",
  "benchmark_return": null,
  "symbol_return": null
}
```

Likely tables or logical entities:

- `advisor_events`: normalized advisor/universe/risk events.
- `operator_feedback`: raw user feedback plus parsed structured intent.
- `policy_change_proposals`: proposed config changes requiring approval.
- `policy_change_audits`: applied/rejected/expired policy changes.
- `decision_outcomes`: delayed attribution records such as 1D/5D/20D relative performance.

## Feedback Intent Shape

Natural language feedback should compile into a bounded policy preference:

```json
{
  "scope": "KR account",
  "intent": "prefer_market_leading_sectors_during_strong_uptrends",
  "strength": 0.7,
  "effective_from": "next_run",
  "expires_after": "30d",
  "requires_confirmation": true,
  "generated_config_patch": {
    "kr_account_policy.leading_sector_boost": 0.15,
    "kr_account_policy.anti_index_buy_penalty": 0.30
  }
}
```

Keep the structured shape small at first. Prefer reusable knobs over free-form prompt memories.

## Chat Interaction Direction

Recommended loop:

1. Summarize recent advisor/universe behavior for a selected account and period.
2. Highlight repeated patterns, especially policy-relevant patterns rather than individual-stock drama.
3. Offer 2-4 bounded adjustment candidates.
4. Let the user choose, edit, or reject them in natural language.
5. Store raw feedback and parsed intent.
6. Generate a config/policy proposal.
7. Apply only after explicit approval or in dry-run-only mode.
8. Report the later outcome and whether the preference should be kept, softened, or expired.

Example prompt to user:

```text
최근 국내 계좌 advisor 후보 18개 중 11개가 KOSPI 20D 상대강도 하위권이었습니다.
주된 원인은 저평가/mean-reversion 점수가 시장 주도성보다 강하게 작동한 것입니다.

정책 후보:
1. 강한 상승장에서는 주도 섹터 가중치 +15%
2. KOSPI 대비 20D 상대강도 하위 40% 신규매수 금액 50% 축소
3. 비주도 섹터 신규매수는 탐색 포지션으로 제한
```

## Integration Boundaries

- `trade-engine` owns DB schema, policy artifact validation, and audit records.
- Advisor command provider may receive current approved policy preferences as input context.
- Risk/policy layer must enforce hard constraints deterministically.
- Hermes can be the chat UI and summarizer, but should not be required for core runtime safety.
- Reports should be reproducible from DB without relying on chat transcript history.

## Phased Development Direction

### Phase 1: Read-only review surface

- Add query/report command for recent advisor/universe decisions.
- Summarize by account, market, sector, decision type, and benchmark-relative outcome.
- No policy mutation.

### Phase 2: Feedback capture

- Store user feedback with raw text, scope, timestamp, and linked decision windows.
- Parse into structured intent, but mark as proposed/unapplied.
- Add tests for safe parsing, ambiguous scope handling, and expiry defaults.

### Phase 3: Policy proposal generation

- Convert structured intents into small config patches.
- Require explicit approval before applying to live-impacting config.
- Record all generated, accepted, rejected, and expired proposals.

### Phase 4: Advisor/risk consumption

- Pass approved preferences into advisor context.
- Add deterministic risk/policy checks for hard caps and throttles.
- Verify that rejected/expired preferences are ignored.

### Phase 5: Outcome review loop

- Compute delayed attribution for decisions affected by feedback.
- Ask whether to keep, weaken, strengthen, or expire the preference.
- Prefer periodic review over permanent hidden behavior changes.

## Open Questions

- Should Hermes own only the UI, or should a dedicated `trade-engine feedback` CLI expose the full loop?
- What is the first supported feedback scope: account, market, sector, strategy, or all of them?
- Which preferences are safe for dry-run auto-apply, and which always require approval?
- How should policy artifacts be stored: TOML patch files, DB rows, or both?
- What minimum attribution windows are useful: 1D/5D/20D, or market-session based?

## Non-goals

- Direct natural-language trading commands.
- LLM-owned portfolio allocation.
- Hidden prompt-only memory that changes future decisions without audit.
- Auto-applying live-impacting policy changes without explicit approval.
