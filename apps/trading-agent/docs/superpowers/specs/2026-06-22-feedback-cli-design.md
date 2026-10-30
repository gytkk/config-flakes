# Feedback CLI Design

## Status

- Date: 2026-06-22
- State: approved design for implementation planning
- Source plan:
  - `plans/CHAT_FEEDBACK_LOOP_PLAN.md`

## Goal

Add the first Rust CLI structure for reviewing and querying the trading
runtime's advisor, universe, decision, and risk audit history.

This slice introduces three top-level `trade-engine` commands:

- `trade-engine status`
- `trade-engine review`
- `trade-engine query --prompt "..."` with `-p` as a short alias

The commands are operator-facing review tools. They must not submit, cancel, or
amend broker orders, and they must not apply policy or config changes.

## Non-Goals

- Do not implement chat UI or Hermes integration.
- Do not support direct natural-language buy, sell, cancel, or amend commands.
- Do not mutate live-impacting policy/config from `query`.
- Do not add a separate Python package or move product ownership out of
  `trade-engine`.
- Do not require a new broker API endpoint.
- Do not add normalized feedback-policy tables in the first slice.

## Current Code Context

The repository already has the foundations needed for this first slice:

- `trade-engine/src/runner/args.rs` owns top-level CLI parsing.
- `trade-engine/src/runner.rs` dispatches `doctor`, `run`, `universe`, and
  `backtest` commands.
- `trade-engine/src/storage/mod.rs` owns Main DB access and already exposes
  audit counts, unresolved broker-command checks, panic-stop checks, and
  command token-usage summaries.
- `operator_actions` already stores append-only operator/audit events with
  structured `metadata_json`.
- Advisor and universe Codex integrations already use external shell wrappers
  under `trade-engine/scripts/`.
- `trade-engine/scripts/codex-advisor` and `trade-engine/scripts/codex-universe`
  already demonstrate the intended `codex exec --ephemeral --sandbox read-only
  --output-schema --output-last-message` pattern.

## Command Semantics

### `trade-engine status`

`status` is deterministic and read-only.

It opens the configured Main DB and prints a compact operational status:

- configured broker environment and account suffix,
- applied migration versions or migration status summary,
- major audit table counts,
- unresolved broker command presence,
- panic-stop active state,
- recent advisor/universe command token-usage summaries when available.

It does not invoke Codex.

### `trade-engine review`

`review` is deterministic and read-only.

It reads recent audit records and renders a human-readable summary of what the
system recently observed and proposed:

- recent universe discovery and universe snapshot events,
- recent tick advisory events,
- decision output/proposal counts and statuses,
- risk evaluation/block counts and reasons when available,
- domestic market-alignment details already recorded in tick output when
  present.

Default options:

```text
--since-hours 24
--limit 20
```

The command supports `--config`, following the existing CLI pattern.
It does not invoke Codex.

### `trade-engine query --prompt(-p) "..."`

`query` is the AI-assisted review command.

Rust owns context selection, redaction, schema validation, rendering, and audit
recording. Codex only receives a bounded read-only context and returns an
explanation, diagnosis, and review candidates.

The command flow is:

1. Open the configured Main DB.
2. Build the same bounded feedback context used by `review`.
3. Add the operator prompt from `--prompt` / `-p`.
4. Pass the JSON request to `trade-engine/scripts/codex-feedback`.
5. The wrapper invokes `codex exec` in read-only, ephemeral mode with a strict
   output schema.
6. Rust validates and renders the structured response.
7. Rust records the query attempt in `operator_actions`, including raw prompt,
   bounded context summary, raw output, parsed response, and error if any.

`query` must not apply policy/config changes. If Codex suggests an adjustment,
the result is only a review candidate for a later explicit proposal/approval
workflow.

## Codex Boundary

Add a wrapper script:

```text
trade-engine/scripts/codex-feedback
```

The wrapper follows the existing advisor/universe wrapper style:

- read request JSON from stdin,
- construct a task prompt that emphasizes review-only authority,
- invoke `codex exec` with:
  - `--cd` set to the repository root,
  - `--ephemeral`,
  - `--sandbox read-only`,
  - `--color never`,
  - `--output-schema trade-engine/schemas/feedback-answer.schema.json`,
  - `--output-last-message` to capture the final JSON,
- write only the final JSON answer to stdout.

The Codex prompt must state:

- Codex may summarize, diagnose, and suggest bounded policy candidates.
- Codex must not instruct broker writes.
- Codex must not ask for secrets, credentials, tokens, hashkeys, or full account
  keys.
- Codex must treat account suffixes as the only account identifiers.
- Codex must cite provided audit evidence instead of inventing unseen market
  facts.
- Codex must mark insufficient evidence explicitly.

## Data Contracts

Add a feedback query request shape owned by Rust. It contains:

- schema version,
- prompt,
- runtime/config scope summary,
- review window,
- status summary,
- recent universe events,
- recent advisor events,
- recent decision/risk summary,
- optional domestic market-alignment snippets,
- explicit redaction notes.

Add a response schema:

```text
trade-engine/schemas/feedback-answer.schema.json
```

Initial response fields:

- `summary`: short answer to the operator prompt,
- `findings`: evidence-backed observations,
- `risks`: safety or evidence limitations,
- `suggested_followups`: bounded next questions or review actions,
- `policy_candidates`: proposed but unapplied policy/config ideas,
- `evidence`: references to supplied audit records or context sections.

All fields must be bounded in count and string length in Rust validation.

## Storage And Audit

Use the existing Main DB and prefer the existing `operator_actions` table for
this slice.

Action types:

- `feedback_query_completed`
- `feedback_query_failed`

`status` and `review` do not record operator actions in this slice because they
remain deterministic local DB reads. `query` always records an attempt because
it sends audit context to an external agent boundary.

Metadata contains:

- schema version,
- review window,
- prompt for `query`,
- compact context summary,
- Codex provider/wrapper name,
- raw output when present,
- parsed answer when valid,
- error when failed.

## Safety Rules

- Full account keys, secrets, access tokens, hashkeys, and API keys must never
  be included in prompts, logs, fixtures, or audit metadata.
- `review`, `query`, and `status` must not call broker write methods.
- `query` must run Codex with a read-only sandbox.
- Ambiguous or missing evidence must be represented as uncertainty, not hidden
  confidence.
- Policy candidates returned by Codex remain unapplied text/JSON until a later
  explicit approval workflow exists.

## Error Handling

- Missing or invalid Main DB path returns a config/storage error.
- Missing Codex wrapper or non-zero wrapper exit records
  `feedback_query_failed` and returns a user-facing error.
- Invalid Codex JSON or schema validation failure records
  `feedback_query_failed`.
- Empty audit history must render a useful empty-state status rather than
  failing.
- Prompt text is required for `query`; empty or whitespace-only prompts fail
  during argument/config validation.

## Testing Strategy

Use TDD for implementation.

CLI parsing tests:

- `trade-engine status` parses.
- `trade-engine review --since-hours 12 --limit 5` parses.
- `trade-engine query -p "..."` parses.
- `trade-engine query --prompt "..."` parses.
- empty prompt is rejected by command validation or command execution.

Storage tests:

- feedback status summary reads existing audit counts and safety state.
- feedback review context returns recent operator actions and
  decision/risk records in stable order.
- feedback query audit records completed and failed attempts.

Provider tests:

- command provider sends bounded request JSON to a test script.
- valid structured output is accepted.
- invalid JSON, invalid schema, timeout, and non-zero exit are rejected with
  specific errors.

Runner tests:

- `status` renders without invoking Codex.
- `review` renders without invoking Codex.
- `query` invokes the configured/scripted feedback provider and records audit.

Implementation verification includes at least:

```sh
cargo fmt --check
cargo test --all-features feedback
cargo test --all-features runner
cargo test --all-features storage
```

Full required Rust verification remains the repository definition of done for
code changes:

```sh
cargo check --workspace --no-default-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

## Rollout

Implement the commands in this order:

1. `status`
2. `review`
3. `query`

This keeps the deterministic DB context stable before adding the Codex command
boundary. The later feedback-policy workflow can build on the stored query
audits without changing live trading behavior in this slice.
