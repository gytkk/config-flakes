# KIS Client Improvement Plan

## Purpose

This document turns the KIS client review findings into an implementation plan
for making `kis-client` a more correct, official-sample-aligned, and
live-safe API client.

It does not replace `plans/KIS_CLIENT_IMPLEMENTATION_PLAN.md`. It narrows the
next improvement pass to the gaps found after comparing the current Rust client
with the official `koreainvestment/open-trading-api` sample repository.

## Verified Reference Snapshot

Official source checked:

- Repository: <https://github.com/koreainvestment/open-trading-api>
- Snapshot: `33e0e1e65cd1c8c8b639531483ec0b327087bab1`
- Snapshot date: 2026-03-18

Primary official sample files used:

- `examples_llm/kis_auth.py`
- `examples_llm/domestic_stock/inquire_price/inquire_price.py`
- `examples_llm/domestic_stock/chk_holiday/chk_holiday.py`
- `examples_llm/domestic_stock/inquire_balance/inquire_balance.py`
- `examples_llm/domestic_stock/inquire_psbl_order/inquire_psbl_order.py`
- `examples_llm/domestic_stock/order_cash/order_cash.py`
- `examples_llm/domestic_stock/order_rvsecncl/order_rvsecncl.py`
- `examples_llm/domestic_stock/inquire_psbl_rvsecncl/inquire_psbl_rvsecncl.py`
- `examples_llm/domestic_stock/inquire_daily_ccld/inquire_daily_ccld.py`

Important observations from the official samples:

- Common request wrapper sets auth headers, `tr_id`, `custtype`, and
  `tr_cont`; paper trading transforms many `T...` TR IDs to `V...`.
- The hashkey helper exists at `/uapi/hashkey`, but the current sample wrapper
  comments out automatic hashkey attachment for POST requests.
- Cash order uses `/uapi/domestic-stock/v1/trading/order-cash` with
  `TTTC0011U`/`TTTC0012U` and `VTTC0011U`/`VTTC0012U`.
- Revise/cancel uses `/uapi/domestic-stock/v1/trading/order-rvsecncl` with
  `TTTC0013U` and `VTTC0013U`.
- The official revise/cancel sample explicitly says to call
  `inquire-psbl-rvsecncl` before revise/cancel and check possible quantity.
- Revise/cancel-possible inquiry uses
  `/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl`, `TTTC0084R`
  transformed to paper by the common wrapper, `CTX_AREA_FK100`,
  `CTX_AREA_NK100`, and `tr_cont` pagination.
- Balance and daily order/fill inquiries use `tr_cont` plus `CTX_AREA_*`
  continuation tokens and recursively fetch pages while `tr_cont` is `M` or
  `F`.

## Current Gap Summary

The current Rust client is well covered by local fake-server tests and already
passes:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

However, the current implementation should be treated as a tested foundation,
not as complete live-ready behavior. The main gaps are:

- Revise/cancel preflight checks open order state, but does not implement the
  official revise/cancel-possible inquiry or possible quantity check.
- Write paths do not gate on market session or holiday state.
- Reconciliation only matches broker id, target broker id, or client id. It does
  not use broker organization number, exchange code, symbol, side, quantity, or
  observation windows.
- Journal schema contains some planned fields, but the Rust record model and
  transitions do not persist all of them.
- Error mapping is too coarse for KIS body errors and throttle messages.
- Rate limiting is interval-based only; no jittered backoff, throttle tracking,
  or circuit breaker exists.
- Token cache is environment/app-key scoped, not account scoped, and has no
  inter-process refresh lock.
- API contract tests exist, but they do not yet encode enough official sample
  metadata to detect drift across all required endpoints.

## Improvement Principles

- Official samples and KIS portal behavior are the contract. Blog posts and old
  snippets are hints only.
- Every endpoint improvement starts with executable contract tests for method,
  path, headers, TR ID, request body/query, response mapping, error mapping, and
  pagination where applicable.
- Existing safety invariants stay stricter than convenience. If a write may have
  reached KIS, record reconciliation-needed and do not retry blindly.
- `kis-client/src/core/journal.rs` must remain broker-agnostic. KIS-specific raw
  payloads, TR IDs, and response field names stay under
  `kis-client/src/protocol/`, `kis-client/src/mapping/`,
  `kis-client/src/transport.rs`, and `kis-client/src/reconcile.rs`.
- Rust APIs must keep using typed wrappers and `rust_decimal::Decimal`; no
  money, price, or quantity should use `f64`.

## Phase 1. Official Spec Contract Baseline

Goal: make official sample behavior visible in tests before changing runtime
logic.

Tasks:

- Add an `ApiSpecCase` test helper with:
  - official source path and checked commit,
  - method,
  - path,
  - environment-specific TR IDs or paper TR transformation rule,
  - endpoint group,
  - hashkey policy,
  - required headers,
  - required query/body fields,
  - pagination token fields,
  - success and error fixture names.
- Add contract cases for:
  - auth token issue,
  - optional hashkey issue,
  - quote,
  - holiday/session,
  - balance,
  - buying power,
  - cash order,
  - revise/cancel order,
  - revise/cancel-possible inquiry,
  - daily order/fill inquiry.
- Encode the official common header behavior from `kis_auth.py`, including
  `custtype`, `tr_cont`, and redaction expectations for app key, app secret,
  token, account number, and hashkey.
- Decide per endpoint whether hashkey is required, optional, or disabled by
  current official sample behavior. The client must support hashkey generation
  even if cash order remains configured as no-hashkey for the current official
  sample.

Acceptance:

- Tests fail first against any missing contract metadata.
- Every implemented endpoint has a test that names the official sample file used
  as its reference.

## Phase 2. Transport, Auth, and Hashkey Hardening

Goal: make request construction and retry behavior match KIS semantics more
closely.

Tasks:

- Add a `KisTokenProvider` integration path to `KisTransport` instead of
  requiring callers to inject a static access token for normal operation.
- Scope token cache by `environment + account + appkey fingerprint`.
- Add an inter-process token refresh lock so multiple processes do not stampede
  KIS token issue.
- Add bounded token issue retry/backoff for temporary failures and rate limits.
- Implement `/uapi/hashkey` as a typed helper:
  - request body must be exactly the order payload,
  - response maps `HASH`,
  - secret and request body audit output must be redacted or summarized.
- Preserve transport failure classes:
  - connect failure,
  - request timeout before socket write,
  - timeout or reset after possible write,
  - HTTP 5xx,
  - KIS body error,
  - throttle.
- For `EndpointGroup::WriteOrder`, classify any uncertain post-send failure as
  ambiguous and return it to the write orchestrator for journal marking.
- Add throttle detection from both HTTP 429 and KIS `msg_cd`/`msg1` bodies.
- Add jittered backoff and temporary circuit state for repeated throttles.

Acceptance:

- Read endpoints retry only idempotent temporary failures.
- Write endpoints never retry after possible socket write.
- Auth refresh is coalesced across tasks and guarded across processes.
- Debug/audit output does not contain app secrets, access tokens, account
  numbers, raw hashkeys, or raw order bodies with account fields.

## Phase 3. Complete Inquiry Coverage

Goal: cover the read paths required for safe writes and recovery.

Tasks:

- Add `inquire-psbl-rvsecncl` endpoint metadata and tests:
  - path `/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl`,
  - live TR `TTTC0084R`,
  - paper TR behavior verified from the official wrapper transformation,
  - `INQR_DVSN_1`, `INQR_DVSN_2`, `CTX_AREA_FK100`, `CTX_AREA_NK100`,
  - `tr_cont` pagination.
- Add domain models:
  - `ReviseCancelCandidate`,
  - `ReviseCancelCapability`,
  - `CancelableQuantity`,
  - `AmendableQuantity`.
- Split current daily order/fill logic from open-order capability logic. Daily
  order inquiry can support reconciliation, but revise/cancel writes must use
  possible-quantity data.
- Add page aggregation rules:
  - repeated continuation tokens return `BrokerError::Invariant`,
  - repeated order rows across pages either deduplicate by full native key or
    fail explicitly,
  - partial page failures do not return partial results.
- Add mapping for unknown raw status codes into `Unknown(String)` instead of
  collapsing to accepted/rejected when the raw meaning is not known.

Acceptance:

- Cancel/amend preflight can prove a target order is currently eligible and has
  sufficient possible quantity.
- Inquiry tests include multi-page success, repeated token, duplicate row, and
  unknown status cases.

## Phase 4. Write Gate Rework

Goal: enforce the full write preflight sequence from the original plan and the
official revise/cancel guidance.

Required write order:

1. Validate runtime config and account.
2. Reserve command in the journal.
3. Acquire or verify single-writer lock for live.
4. Check unresolved journal records for live.
5. Check read-only state.
6. Check market session/holiday gate for new writes.
7. For place order, optionally check buying power or configured policy hook.
8. For cancel/amend, check `inquire-psbl-rvsecncl` possible quantity.
9. Mark `Submitting` with request id and send-start timestamp.
10. Submit to KIS.
11. Persist ack, rejection, or reconciliation-needed.

Tasks:

- Move journal reserve before validations that do not require network, while
  preserving "no broker submit without durable command".
- Add `MarketWriteGate`:
  - can be disabled only in explicit test/runtime config,
  - uses KST trading day,
  - treats existing reconciliation as allowed even when market is closed,
  - blocks new live writes when session is closed or holiday status is unknown.
- Add cancel/amend quantity validation:
  - full cancel uses possible quantity from KIS,
  - partial cancel cannot exceed possible quantity,
  - amend quantity cannot exceed possible amend/cancel quantity,
  - missing or duplicate possible-order rows block the write.
- Persist `transport_send_started_at` before sending.
- On mapper errors after a 2xx write response, decide whether the response could
  represent an accepted order. If yes, mark reconciliation-needed instead of
  returning a plain transport error.

Acceptance:

- Tests prove fake server is not called when any preflight fails.
- Tests prove command is durable before any fake server write call.
- Ambiguous post-send mapper/transport failures leave durable
  `SubmittedAckUnknown` state.

## Phase 5. Journal Schema and State Transitions

Goal: make durable state complete enough for startup recovery and audit.

Tasks:

- Add a migration after `0001_command_journal.sql`; do not edit existing
  migration history.
- Add model fields for:
  - `decision_id`,
  - `transport_send_started_at`,
  - `cancel_or_amend_quantity`,
  - `amend_price`,
  - `trading_day`,
  - `broker_observation_window_start`,
  - `broker_observation_window_end`,
  - optional sanitized last request/response metadata id.
- Ensure state transitions are done inside SQLite transactions, especially
  reserve plus parent integrity checks and all post-submit state changes.
- Add transition guards:
  - `CommandRecorded -> Submitting`,
  - `Submitting -> Accepted | Rejected | SubmittedAckUnknown`,
  - `SubmittedAckUnknown -> Accepted | Filled | Cancelled | Rejected | ReconcileFailed`,
  - terminal states cannot be overwritten except by explicit operator recovery.
- Add journal queries for:
  - unresolved live-blocking commands,
  - commands by parent,
  - commands by broker native id,
  - commands in observation window.

Acceptance:

- Crash-recovery tests can reconstruct pending work from the database alone.
- Invalid state transitions fail before mutating the database.

## Phase 6. Reconciliation Upgrade

Goal: recover ambiguous outcomes using all available broker evidence without
creating duplicate orders.

Tasks:

- Implement a reconciliation matcher with ordered strategies:
  - exact broker order id + organization number + account + environment,
  - target broker order id for cancel/amend,
  - client order id where available,
  - symbol + side + quantity + trading day + observation window,
  - fill evidence,
  - balance/cash evidence as supporting evidence only.
- Store and use observation windows:
  - command requested time,
  - send-start time,
  - broker response time if available,
  - KST trading day.
- Use multiple sources of truth:
  - daily order inquiry,
  - daily fills,
  - revise/cancel-possible inquiry,
  - account balance/cash when needed.
- Return `ManualInterventionRequired` when more than one plausible match
  exists.
- Keep `StillUnknown` live-blocking until operator action or later reconcile
  succeeds.
- Return structured reconciliation evidence that product operator surfaces can
  render, including which matcher succeeded or why manual intervention is
  required.

Acceptance:

- Tests cover ambiguous place, cancel, amend, duplicate plausible matches,
  next-business-day recovery, and missing broker-native id fallback.
- Startup recovery keeps live writes blocked until no unresolved live-blocking
  records remain.

## Phase 7. Error Mapping and Observability

Goal: make KIS failures actionable without leaking secrets.

Tasks:

- Add a `KisErrorClassifier` that uses HTTP status, `rt_cd`, `msg_cd`, `msg1`,
  endpoint group, and headers.
- Map known classes to:
  - `Auth`,
  - `Permission`,
  - `RateLimit`,
  - `Validation`,
  - `Temporary`,
  - `MarketClosed`,
  - `OrderRejected`,
  - `ReconciliationNeeded`,
  - `SafetyHalt`,
  - `Invariant`.
- Preserve raw `msg_cd` in sanitized metadata for audit/debug.
- Add structured tracing fields:
  - request id,
  - endpoint name,
  - endpoint group,
  - TR ID,
  - environment,
  - account suffix only,
  - latency,
  - retry count,
  - rate-limit sleep,
  - KIS result code,
  - command id for writes.
- Redact:
  - app key,
  - app secret,
  - access token,
  - account number,
  - hashkey,
  - webhook URL,
  - raw order payload account fields.

Acceptance:

- Contract tests cover at least one body-level KIS error per endpoint group.
- Logs and debug output do not contain secrets in negative tests.

## Phase 8. Product Operator Recovery

Goal: make live-blocking states visible and recoverable by an operator.

Tasks:

- Keep `kis-client` library-only. It should expose typed health, safety,
  journal, and reconciliation results; product operator commands belong in
  `agent`.
- Product `doctor` should report:
  - environment,
  - account suffix,
  - read-only state,
  - live enabled state,
  - single-writer state,
  - product-level panic stop state,
  - unresolved command count,
  - market session state,
  - token cache health without token value.
- Product `resume-live` must not clear unresolved journal records. It can clear
  product-level panic stop only; journal recovery remains explicit.

Acceptance:

- Product operator tests verify status and output for ok, halted, unresolved,
  and product-level panic stop states.
- Operator output never prints full account number or secrets.

## Phase 9. Paper and Live Certification

Goal: separate fake-server completion from real KIS readiness.

Fake-server completion is not live certification. The following must stay
unchecked until manually verified with real credentials and an approved account:

- paper auth token issue and reuse,
- paper quote, balance, buying power,
- paper cash buy/sell with tiny controlled order,
- paper order inquiry and fill inquiry,
- paper cancel/amend and possible-quantity preflight,
- ambiguous write recovery drill against a simulated transport failure,
- read-only live quote/balance smoke,
- product-level live panic stop and resume drill,
- live startup recovery drill with a seeded unresolved journal record.

Acceptance:

- Add a `paper-certification` test/runbook section to README only when commands
  are stable and safe.
- No live write command is documented as enabled until operator checklist items
  are complete.

## Immediate Implementation Order

1. Add failing contract tests for `inquire-psbl-rvsecncl`.
2. Implement the endpoint metadata, request builder, mapper, pagination, and
   fixtures.
3. Replace cancel/amend open-order-only preflight with possible-quantity
   preflight.
4. Add market-session write gate tests and implementation.
5. Add journal migration/model fields for send-start and observation windows.
6. Upgrade reconciliation matching and tests.
7. Harden error classification and rate-limit behavior.
8. Integrate token provider and hashkey helper.
9. Expand product operator recovery visibility.
10. Run full Rust verification after each logical change and commit each change
    separately.

## Definition of Done

The improvement pass is complete when:

- all implemented endpoints have official-sample-backed contract tests,
- cancel/amend cannot submit without KIS possible-quantity confirmation,
- new writes are blocked when market session state is closed or unknown,
- ambiguous write failures always produce durable reconciliation-needed state,
- reconciliation can recover through broker id, target id, client id, and
  symbol/side/quantity/time-window matching,
- live startup blocks until unresolved commands are cleared or reconciled,
- token cache is account-scoped and refresh is inter-process safe,
- rate-limit handling includes body-level throttle detection and circuit state,
- product operator surfaces expose enough state for an operator to understand
  and recover a halt,
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  and `cargo test --all-features` pass.
