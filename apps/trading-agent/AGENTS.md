# AGENTS.md

## Project Context

This repository implements the full `trading-agent` product described in
`plans/PLAN_V4.md`.

The product is Rust-first and combines deterministic broker/data/execution
code, an agent decision layer that emits structured proposals, risk/policy gates
with final authority, auditable storage, and a minimal `run`/`doctor` runtime
surface. Do not introduce a Python MVP package unless `plans/PLAN_V4.md` is
updated first.

## Source of Truth

Use this order when project documents conflict:

1. `AGENTS.md`
2. `plans/PLAN_V4.md`
3. `plans/KIS_CLIENT_IMPLEMENTATION_PLAN.md`
4. `plans/PLAN_V3.md`, `plans/PLAN_V2.md`, and `plans/INITIAL_PLAN.md`
   as historical context only
5. `README.md`

The KIS client is one broker adapter component. Do not reshape the whole
product around it unless `plans/PLAN_V4.md` changes first.

## Repository Structure

- `plans/`: product and component plans. `PLAN_V4.md` is current; older plans
  are historical unless explicitly referenced.
- `kis-client/`: Rust KIS broker client crate. Owns KIS auth, protocol,
  transport, mapping, safety, reconciliation helpers, and broker-agnostic core
  traits/domain/error/journal types. Do not add product DB ownership, SQLx
  migrations, or an operator CLI here.
- `trade-engine/`: Rust product runtime crate. Owns config, gateway adapters,
  decision, risk, execution, reconciliation, storage, doctor, runtime, and
  `run`/`doctor` CLI behavior. Runtime code should depend on
  `TradingGateway`, not concrete KIS transport details.
- `trade-engine/migrations/`: main product database migrations. Add new
  migrations instead of editing applied ones, register them in
  `trade-engine/src/storage/mod.rs::MIGRATIONS`, and test schema changes.
- `trade-engine/schemas/`: product schemas for structured boundaries.
- `trade-engine/scripts/`: project scripts; keep them deterministic and
  non-interactive.
- `configs/`: configuration examples or local config templates. Never commit
  secrets, tokens, account files, local DBs, or runtime state.
- `README.md`: user-facing setup, commands, and behavior. Update it when
  user-facing behavior or project structure changes.

## Workflow Rules

- Check whether an available Codex skill fits before starting work. Use a skill
  only when it directly applies.
- Use `rg` for searching.
- For simple, low-risk single-file changes, edit directly and verify with the
  smallest relevant check.
- For cross-module, schema, broker API, execution, risk, reconciliation, or
  live-safety changes, present a short plan and get approval before editing
  unless implementation was explicitly requested.
- Ask before changing behavior when account scope, live trading impact, market
  assumptions, risk policy, or recovery behavior is ambiguous.
- Keep changes small and commit each logical change locally. Prefer
  Conventional Commits in imperative mood. Do not push unless requested.
- Do not use destructive git commands or revert user changes unless explicitly
  approved.

## Rust and Product Design Rules

- Use TDD: write or update the failing test first, then implement the minimum
  code needed to pass.
- Prefer simple, idiomatic Rust with typed domain values over unchecked strings,
  booleans, or sentinel values.
- All broker and gateway I/O should be async.
- Public `kis-client` APIs should return `Result<T, BrokerError>`.
- Product runtime APIs should return `Result<T, TradeEngineError>` when product
  context is needed.
- Preserve specific error context. Do not collapse broker, storage, policy,
  validation, timeout, and reconciliation failures into generic strings.
- Avoid panics, `unwrap`, and `expect` in production paths.
- Use `rust_decimal::Decimal` for money and prices. Do not use `f64` for money,
  prices, or quantities.
- Keep async code cancellation-aware and avoid holding locks across `.await`
  unless the call path is designed for it.
- Keep comments sparse and useful.

## Architecture and Trading Safety

- Keep agent reasoning separate from deterministic execution. The agent may
  propose structured decisions, but it must not submit orders.
- Risk and policy gates must sit between decisions and execution and have final
  authority.
- Execution results, risk blocks, broker responses, fills, portfolio snapshots,
  and reconciliation outcomes must be auditable.
- Prefer append-only, reconstructable state transitions. Do not make in-memory
  state the source of truth for execution, risk, fills, balances, positions, or
  reconciliation.
- Treat ambiguous broker outcomes as first-class states. Persist enough context
  to reconcile them and avoid blind retries for actions that may have reached a
  broker.
- Keep observability useful but redacted. Secrets such as app secrets, access
  tokens, hashkeys, and API keys must not appear in `Debug`, logs, fixtures,
  prompts, Discord output, or audit output.

## Strategy Research Prompts

When asked to improve, add, or tune trading strategy behavior, especially for
short-term bull-market or "maximize profit" requests:

- Reframe the task as a testable strategy research problem, not a return-only
  optimization problem.
- Do not assume the market regime from the user's wording. Define data-driven
  regime conditions first, such as trend, breadth, volatility, liquidity, and
  freshness requirements.
- Compare candidate strategies before implementation when the request is broad:
  momentum continuation, breakout with volatility filter, and pullback in an
  uptrend are reasonable starting baselines.
- Specify entry, exit, stop, trailing stop, position sizing, concentration,
  cooldown, and no-trade rules before changing execution behavior.
- Require fee, slippage, spread, partial-fill, stale-data, and lookahead-bias
  handling in backtests or paper-trading validation.
- Evaluate strategies with risk-adjusted metrics, not raw return alone:
  fee/slippage-adjusted return, max drawdown, Sharpe or Sortino, turnover, hit
  rate, average win/loss, exposure, and out-of-sample performance.
- Keep strategy research and backtests separate from live broker writes. New or
  tuned strategies must start in backtest or paper-only paths unless the user
  explicitly approves a live-safety design change.
- The LLM/advisor may emit structured proposals and evidence, but it must not
  bypass deterministic validation, risk/policy gates, durable audit, execution
  workflow, or live-write invariants.
- Treat "increase upside capture" as acceptable only when paired with explicit
  drawdown, notional, leverage, turnover, and halt conditions.
- Before implementation, present a short plan for cross-module strategy,
  decision, risk, execution, storage, or live-safety changes.

## Live Write Invariants

Live trading must remain blocked unless all relevant safety conditions are true:

- `live` writes are explicitly enabled.
- `read_only=true` is not set.
- account suffix confirmation matches.
- live account allowlists are honored.
- the command is durable before the broker write.
- a single-writer lock is held for the same `(environment, account)`.
- unresolved journal records are reconciled before new live writes.
- after a broker write may have reached KIS, recovery happens through
  inquiry/reconciliation rather than blind retry.

When in doubt, prefer duplicate-order prevention over convenience.

## KIS API Workflow

For every KIS REST endpoint:

1. Verify current official KIS documentation or official sample behavior, and
   record the source name/URL plus access date when endpoint behavior is not
   obvious from code.
2. Add executable contract tests for method, path, headers, `tr_id`, request
   shape, response mapping, and error mapping.
3. Add pagination tests when `tr_cont` or `CTX_AREA_*` tokens are involved.
4. Add journal, safety, ambiguous-outcome, and reconciliation tests before any
   order write endpoint.
5. Implement only after the tests fail for the expected reason.

Do not treat blog posts or old examples as authoritative when exact endpoint
behavior matters.

## Verification

Required Rust verification before committing code changes:

```sh
cargo check --workspace --no-default-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

For docs-only changes, verify the rendered intent with `git diff` and run only
the project checks that apply to the changed document. If SQLx query macros are
introduced later, also run:

```sh
cargo sqlx prepare --check
```

## Documentation

Update `README.md` when user-facing behavior, commands, setup, or repository
structure changes. Do not create one-off temporary documentation files.
