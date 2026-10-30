# Feedback CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `trade-engine status`, `trade-engine review`, and `trade-engine query --prompt/-p` as safe review-only operator commands.

**Architecture:** Add a focused `feedback` module for status/review/query domain types, rendering, validation, and command-provider interaction. Extend `storage` with bounded read-only feedback queries and query audit recording, then wire the commands through the existing `runner` CLI dispatch. `query` uses an external `trade-engine/scripts/codex-feedback` wrapper that invokes `codex exec` in read-only mode.

**Tech Stack:** Rust 2024, clap, tokio process, rusqlite, serde/serde_json, existing `StructuredOutput`, SQLite Main DB, Codex CLI wrapper scripts.

---

## File Structure

- Create `trade-engine/src/feedback.rs`
  - Own feedback request/response domain types.
  - Own validation limits for prompt and Codex response fields.
  - Own deterministic renderers for `status`, `review`, and `query`.
  - Own `CommandFeedbackProvider` that shells out to `trade-engine/scripts/codex-feedback`.
- Modify `trade-engine/src/lib.rs`
  - Export `feedback`.
- Modify `trade-engine/src/runner/args.rs`
  - Add `StatusArgs`, `ReviewArgs`, `QueryArgs`.
  - Add top-level `RunnerCommand::{Status, Review, Query}`.
  - Route `config_path()` for new commands.
- Modify `trade-engine/src/runner.rs`
  - Dispatch new commands.
  - Open Main DB and call feedback module functions.
  - Keep `status` and `review` deterministic and local.
  - Ensure `query` calls the feedback provider and records query audit.
- Modify `trade-engine/src/storage/mod.rs`
  - Add feedback status/review query structs.
  - Add `feedback_status_summary`.
  - Add `feedback_review_context`.
  - Add `record_feedback_query_completed`.
  - Add `record_feedback_query_failed`.
  - Add async `BlockingMainDb` wrappers for those methods.
- Create `trade-engine/schemas/feedback-output.schema.json`
  - Strict OpenAI-compatible schema for the Codex final answer.
  - Use `-output.schema.json` suffix so existing schema contract test covers it.
- Create `trade-engine/scripts/codex-feedback`
  - Read JSON request from stdin.
  - Build a review-only prompt.
  - Call `codex exec --ephemeral --sandbox read-only --output-schema`.
  - Print only final JSON.
- Modify `README.md`
  - Document the three new commands and the read-only/Codex boundary.

## Task 1: CLI Argument Skeleton

**Files:**
- Modify: `trade-engine/src/runner/args.rs`
- Modify: `trade-engine/src/runner.rs`

- [ ] **Step 1: Write failing CLI parse tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `trade-engine/src/runner.rs` near the other `runner_args_parse_*` tests:

```rust
#[test]
fn runner_args_parse_status_command() {
    use clap::Parser as _;

    let args = RunnerArgs::try_parse_from([
        "trade-engine",
        "status",
        "--config",
        "custom.toml",
    ])
    .expect("parse status command");

    assert_eq!(args.config_path(), Path::new("custom.toml"));
    let RunnerCommand::Status(_) = args.command else {
        panic!("expected status command");
    };
}

#[test]
fn runner_args_parse_review_command() {
    use clap::Parser as _;

    let args = RunnerArgs::try_parse_from([
        "trade-engine",
        "review",
        "--since-hours",
        "12",
        "--limit",
        "5",
        "--config",
        "custom.toml",
    ])
    .expect("parse review command");

    assert_eq!(args.config_path(), Path::new("custom.toml"));
    let RunnerCommand::Review(review_args) = args.command else {
        panic!("expected review command");
    };
    assert_eq!(review_args.since_hours, 12);
    assert_eq!(review_args.limit, 5);
}

#[test]
fn runner_args_parse_query_command_short_prompt() {
    use clap::Parser as _;

    let args = RunnerArgs::try_parse_from([
        "trade-engine",
        "query",
        "-p",
        "why were recent buys weak?",
        "--since-hours",
        "6",
        "--limit",
        "3",
        "--config",
        "custom.toml",
    ])
    .expect("parse query command");

    assert_eq!(args.config_path(), Path::new("custom.toml"));
    let RunnerCommand::Query(query_args) = args.command else {
        panic!("expected query command");
    };
    assert_eq!(query_args.prompt, "why were recent buys weak?");
    assert_eq!(query_args.since_hours, 6);
    assert_eq!(query_args.limit, 3);
}

#[test]
fn runner_args_parse_query_command_long_prompt() {
    use clap::Parser as _;

    let args = RunnerArgs::try_parse_from([
        "trade-engine",
        "query",
        "--prompt",
        "summarize advisor behavior",
    ])
    .expect("parse query command");

    let RunnerCommand::Query(query_args) = args.command else {
        panic!("expected query command");
    };
    assert_eq!(query_args.prompt, "summarize advisor behavior");
    assert_eq!(query_args.since_hours, 24);
    assert_eq!(query_args.limit, 20);
}
```

- [ ] **Step 2: Run the focused parser tests and verify failure**

Run:

```sh
cargo test --all-features runner_args_parse_status_command runner_args_parse_review_command runner_args_parse_query_command_short_prompt runner_args_parse_query_command_long_prompt
```

Expected: compile failure because `RunnerCommand::Status`, `Review`, and `Query` do not exist.

- [ ] **Step 3: Add argument structs and variants**

In `trade-engine/src/runner/args.rs`, extend `RunnerCommand` and add args:

```rust
#[derive(Debug, Subcommand)]
pub(super) enum RunnerCommand {
    Backtest(BacktestArgs),
    Doctor(DoctorArgs),
    Query(QueryArgs),
    Review(ReviewArgs),
    Run(RunArgs),
    Status(StatusArgs),
    Universe(UniverseArgs),
}

#[derive(Debug, Args)]
pub(super) struct StatusArgs {
    #[arg(long, default_value = "configs/trade-engine.live-dry-run.toml")]
    pub(super) config: PathBuf,
}

#[derive(Debug, Args)]
pub(super) struct ReviewArgs {
    #[arg(long, default_value = "configs/trade-engine.live-dry-run.toml")]
    pub(super) config: PathBuf,
    #[arg(long, default_value_t = 24)]
    pub(super) since_hours: u64,
    #[arg(long, default_value_t = 20)]
    pub(super) limit: usize,
}

#[derive(Debug, Args)]
pub(super) struct QueryArgs {
    #[arg(long, short = 'p')]
    pub(super) prompt: String,
    #[arg(long, default_value = "configs/trade-engine.live-dry-run.toml")]
    pub(super) config: PathBuf,
    #[arg(long, default_value_t = 24)]
    pub(super) since_hours: u64,
    #[arg(long, default_value_t = 20)]
    pub(super) limit: usize,
}
```

Update `config_path()`:

```rust
pub fn config_path(&self) -> &Path {
    match &self.command {
        RunnerCommand::Backtest(args) => &args.config,
        RunnerCommand::Doctor(args) => &args.config,
        RunnerCommand::Query(args) => &args.config,
        RunnerCommand::Review(args) => &args.config,
        RunnerCommand::Run(args) => &args.config,
        RunnerCommand::Status(args) => &args.config,
        RunnerCommand::Universe(args) => &args.config,
    }
}
```

Update the `use args::{...}` import in `trade-engine/src/runner.rs`:

```rust
use args::{
    BacktestArgs, QueryArgs, ReviewArgs, RunArgs, RunnerCommand, StatusArgs, UniverseArgs,
};
```

Add temporary dispatch arms that return explicit runtime errors:

```rust
RunnerCommand::Query(args) => query_command(config, args).await,
RunnerCommand::Review(args) => review_command(config, args).await,
RunnerCommand::Status(args) => status_command(config, args).await,
```

Add temporary functions:

```rust
async fn status_command(
    _config: EngineConfig,
    _args: StatusArgs,
) -> Result<String, TradeEngineError> {
    Err(TradeEngineError::runtime(
        "status command is not implemented yet",
    ))
}

async fn review_command(
    _config: EngineConfig,
    _args: ReviewArgs,
) -> Result<String, TradeEngineError> {
    Err(TradeEngineError::runtime(
        "review command is not implemented yet",
    ))
}

async fn query_command(
    _config: EngineConfig,
    _args: QueryArgs,
) -> Result<String, TradeEngineError> {
    Err(TradeEngineError::runtime(
        "query command is not implemented yet",
    ))
}
```

- [ ] **Step 4: Run focused parser tests and verify pass**

Run:

```sh
cargo test --all-features runner_args_parse_status_command runner_args_parse_review_command runner_args_parse_query_command_short_prompt runner_args_parse_query_command_long_prompt
```

Expected: parser tests pass.

- [ ] **Step 5: Commit CLI skeleton**

Run:

```sh
git add trade-engine/src/runner.rs trade-engine/src/runner/args.rs
git commit -m "feat: add feedback CLI argument skeleton"
```

## Task 2: Feedback Status Command

**Files:**
- Create: `trade-engine/src/feedback.rs`
- Modify: `trade-engine/src/lib.rs`
- Modify: `trade-engine/src/storage/mod.rs`
- Modify: `trade-engine/src/runner.rs`
- Test: `trade-engine/tests/storage_audit.rs`
- Test: `trade-engine/src/runner.rs`

- [ ] **Step 1: Write failing storage status test**

Add this test to `trade-engine/tests/storage_audit.rs`:

```rust
#[test]
fn feedback_status_summary_reads_db_health_and_counts() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");

    let db = MainDb::open(&db_path).expect("open db");
    let summary = db.feedback_status_summary().expect("feedback status");

    assert_eq!(summary.applied_migrations, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    assert_eq!(summary.audit_counts, AuditCounts::default());
    assert!(!summary.unresolved_broker_commands);
    assert!(!summary.panic_stop_active);
    assert!(summary.command_token_usage.is_empty());
}
```

- [ ] **Step 2: Run failing storage status test**

Run:

```sh
cargo test --all-features feedback_status_summary_reads_db_health_and_counts
```

Expected: compile failure because `feedback_status_summary` does not exist.

- [ ] **Step 3: Implement storage status summary**

In `trade-engine/src/storage/mod.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackStatusSummary {
    pub applied_migrations: Vec<i64>,
    pub audit_counts: AuditCounts,
    pub unresolved_broker_commands: bool,
    pub panic_stop_active: bool,
    pub command_token_usage: Vec<CommandTokenUsageSummary>,
}
```

Add a synchronous method on `MainDb`:

```rust
pub fn feedback_status_summary(&self) -> Result<FeedbackStatusSummary, TradeEngineError> {
    Ok(FeedbackStatusSummary {
        applied_migrations: self.applied_migrations()?,
        audit_counts: self.audit_counts()?,
        unresolved_broker_commands: self.has_unresolved_broker_commands()?,
        panic_stop_active: self.panic_stop_active()?,
        command_token_usage: self.command_token_usage_summaries()?,
    })
}
```

Add an async wrapper on `BlockingMainDb`:

```rust
pub async fn feedback_status_summary(&self) -> Result<FeedbackStatusSummary, TradeEngineError> {
    self.with_db(move |db| db.feedback_status_summary()).await
}
```

- [ ] **Step 4: Create feedback status renderer**

Create `trade-engine/src/feedback.rs`:

```rust
use crate::storage::{CommandTokenUsageSource, FeedbackStatusSummary};
use crate::stdout::{OutputField, StructuredOutput};

pub fn render_status(summary: &FeedbackStatusSummary, environment: &str, account_suffix: &str) -> String {
    let mut output = StructuredOutput::new();
    output
        .heading("Status")
        .field("environment", environment)
        .field("account_suffix", account_suffix)
        .field("migrations", migration_summary(&summary.applied_migrations))
        .field("unresolved_broker_commands", summary.unresolved_broker_commands)
        .field("panic_stop_active", summary.panic_stop_active);

    output
        .heading("Audit counts")
        .field("runtime_runs", summary.audit_counts.runtime_runs)
        .field("runtime_ticks", summary.audit_counts.runtime_ticks)
        .field("decision_outputs", summary.audit_counts.decision_outputs)
        .field("decision_proposals", summary.audit_counts.decision_proposals)
        .field("risk_evaluations", summary.audit_counts.risk_evaluations)
        .field("risk_blocks", summary.audit_counts.risk_blocks)
        .field("execution_requests", summary.audit_counts.execution_requests)
        .field("fills", summary.audit_counts.fills);

    output.heading("Command token usage");
    if summary.command_token_usage.is_empty() {
        output.empty_item();
    } else {
        for usage in &summary.command_token_usage {
            output.item(
                command_usage_source_label(usage.source),
                &[
                    OutputField::new("invocations", usage.recorded_invocations),
                    OutputField::new("input", usage.input_tokens),
                    OutputField::new("output", usage.output_tokens),
                    OutputField::new("reasoning", usage.reasoning_output_tokens),
                    OutputField::new("total", usage.total_tokens),
                ],
            );
        }
    }
    output.blank();
    output.finish()
}

fn migration_summary(versions: &[i64]) -> String {
    match (versions.first(), versions.last()) {
        (Some(first), Some(last)) if first == last => first.to_string(),
        (Some(first), Some(last)) => format!("{first}..{last} ({} applied)", versions.len()),
        _ => "none".to_owned(),
    }
}

fn command_usage_source_label(source: CommandTokenUsageSource) -> &'static str {
    match source {
        CommandTokenUsageSource::Advisor => "advisor",
        CommandTokenUsageSource::UniverseDiscovery => "universe_discovery",
    }
}
```

Modify `trade-engine/src/lib.rs`:

```rust
pub mod feedback;
```

- [ ] **Step 5: Replace status command stub**

In `trade-engine/src/runner.rs`, implement:

```rust
async fn status_command(
    config: EngineConfig,
    _args: StatusArgs,
) -> Result<String, TradeEngineError> {
    let account_suffix = broker_account_suffix(&config)?;
    let environment = config.broker_environment();
    let db = BlockingMainDb::open(config.main_db_path()?)?;
    let summary = db.feedback_status_summary().await?;
    Ok(crate::feedback::render_status(
        &summary,
        environment.as_str(),
        &account_suffix,
    ))
}
```

- [ ] **Step 6: Write runner status render test**

Add this runner unit test inside `trade-engine/src/runner.rs`:

```rust
#[tokio::test]
async fn status_command_renders_feedback_status() {
    use clap::Parser as _;

    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let config = EngineConfig {
        read_only: true,
        watchlist: vec!["005930".to_owned()],
        broker: Default::default(),
        market: Default::default(),
        kis: Default::default(),
        toss: Default::default(),
        storage: crate::config::StorageConfig {
            main_db_path: Some(db_path),
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
    let args = RunnerArgs::try_parse_from(["trade-engine", "status"]).expect("parse status");

    let output = run(config, args).await.expect("status output");

    assert!(output.contains("Status"));
    assert!(output.contains("migrations: 1..12 (12 applied)"));
    assert!(output.contains("unresolved_broker_commands: false"));
    assert!(output.contains("panic_stop_active: false"));
}
```

- [ ] **Step 7: Run status tests**

Run:

```sh
cargo test --all-features feedback_status_summary_reads_db_health_and_counts runner_args_parse_status_command
```

Expected: pass.

- [ ] **Step 8: Commit status command**

Run:

```sh
git add trade-engine/src/feedback.rs trade-engine/src/lib.rs trade-engine/src/storage/mod.rs trade-engine/src/runner.rs trade-engine/tests/storage_audit.rs
git commit -m "feat: add feedback status command"
```

## Task 3: Feedback Review Context And Command

**Files:**
- Modify: `trade-engine/src/storage/mod.rs`
- Modify: `trade-engine/src/feedback.rs`
- Modify: `trade-engine/src/runner.rs`
- Test: `trade-engine/tests/storage_audit.rs`
- Test: `trade-engine/src/runner.rs`

- [ ] **Step 1: Write failing storage review test**

Add a test in `trade-engine/tests/storage_audit.rs` that inserts two `operator_actions` rows and asserts stable newest-first review context:

```rust
#[test]
fn feedback_review_context_reads_recent_operator_actions() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let db = MainDb::open(&db_path).expect("open db");
    let connection = Connection::open(&db_path).expect("inspect db");

    connection
        .execute(
            "INSERT INTO operator_actions (
                observed_at, operator_id, action_type, target_type, target_id, reason, metadata_json
             )
             VALUES
                ('2026-06-22T00:00:00Z', NULL, 'tick_advisory_completed', 'runtime_run', '1', NULL, '{\"status\":\"parsed\",\"output\":{\"summary\":\"advisor ok\"}}'),
                ('2026-06-22T00:05:00Z', NULL, 'universe_snapshot_refreshed', 'universe_snapshot', '2', NULL, '{\"summary\":\"universe ok\",\"candidate_count\":3}')",
            [],
        )
        .expect("insert actions");

    let context = db.feedback_review_context(24, 20).expect("review context");

    assert_eq!(context.window.since_hours, 24);
    assert_eq!(context.window.limit, 20);
    assert_eq!(context.operator_actions.len(), 2);
    assert_eq!(context.operator_actions[0].action_type, "universe_snapshot_refreshed");
    assert_eq!(context.operator_actions[0].target_id.as_deref(), Some("2"));
    assert_eq!(context.operator_actions[1].action_type, "tick_advisory_completed");
}
```

- [ ] **Step 2: Run failing storage review test**

Run:

```sh
cargo test --all-features feedback_review_context_reads_recent_operator_actions
```

Expected: compile failure because `feedback_review_context` does not exist.

- [ ] **Step 3: Implement review context storage types**

In `trade-engine/src/storage/mod.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackReviewWindow {
    pub since_hours: u64,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackOperatorAction {
    pub id: i64,
    pub observed_at: String,
    pub action_type: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub reason: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackReviewContext {
    pub window: FeedbackReviewWindow,
    pub status: FeedbackStatusSummary,
    pub operator_actions: Vec<FeedbackOperatorAction>,
}
```

Add a method:

```rust
pub fn feedback_review_context(
    &self,
    since_hours: u64,
    limit: usize,
) -> Result<FeedbackReviewContext, TradeEngineError> {
    let observed_after = (OffsetDateTime::now_utc()
        - time::Duration::hours(i64::try_from(since_hours).unwrap_or(i64::MAX)))
    .format(&Rfc3339)
    .map_err(|error| TradeEngineError::storage(format!("failed to format feedback window: {error}")))?;
    let capped_limit = limit.clamp(1, 100);
    let mut statement = self
        .connection
        .prepare(
            "SELECT id, observed_at, action_type, target_type, target_id, reason, metadata_json
             FROM operator_actions
             WHERE observed_at >= ?1
             ORDER BY observed_at DESC, id DESC
             LIMIT ?2",
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![observed_after, capped_limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(sqlite_error)?;
    let mut operator_actions = Vec::new();
    for row in rows {
        let (id, observed_at, action_type, target_type, target_id, reason, metadata_json) =
            row.map_err(sqlite_error)?;
        let metadata = serde_json::from_str::<serde_json::Value>(&metadata_json).map_err(|error| {
            TradeEngineError::storage(format!(
                "invalid operator action metadata JSON for feedback review action {id}: {error}"
            ))
        })?;
        operator_actions.push(FeedbackOperatorAction {
            id,
            observed_at,
            action_type,
            target_type,
            target_id,
            reason,
            metadata,
        });
    }
    Ok(FeedbackReviewContext {
        window: FeedbackReviewWindow {
            since_hours,
            limit: capped_limit,
        },
        status: self.feedback_status_summary()?,
        operator_actions,
    })
}
```

Add async wrapper on `BlockingMainDb`:

```rust
pub async fn feedback_review_context(
    &self,
    since_hours: u64,
    limit: usize,
) -> Result<FeedbackReviewContext, TradeEngineError> {
    self.with_db(move |db| db.feedback_review_context(since_hours, limit))
        .await
}
```

- [ ] **Step 4: Add review renderer**

In `trade-engine/src/feedback.rs`, add:

```rust
use crate::storage::FeedbackReviewContext;

pub fn render_review(context: &FeedbackReviewContext) -> String {
    let mut output = StructuredOutput::new();
    output
        .heading("Review")
        .field("since_hours", context.window.since_hours)
        .field("limit", context.window.limit)
        .field("operator_actions", context.operator_actions.len())
        .field("decision_outputs", context.status.audit_counts.decision_outputs)
        .field("decision_proposals", context.status.audit_counts.decision_proposals)
        .field("risk_evaluations", context.status.audit_counts.risk_evaluations)
        .field("risk_blocks", context.status.audit_counts.risk_blocks);

    output.heading("Recent audit events");
    if context.operator_actions.is_empty() {
        output.empty_item();
    } else {
        for action in &context.operator_actions {
            output.item(
                format!("#{} {}", action.id, action.action_type),
                &[
                    OutputField::new("observed_at", &action.observed_at),
                    OutputField::new(
                        "target",
                        action
                            .target_type
                            .as_deref()
                            .zip(action.target_id.as_deref())
                            .map(|(target_type, target_id)| format!("{target_type}:{target_id}"))
                            .unwrap_or_else(|| "none".to_owned()),
                    ),
                    OutputField::new("summary", metadata_summary(&action.metadata)),
                ],
            );
        }
    }
    output.blank();
    output.finish()
}

fn metadata_summary(metadata: &serde_json::Value) -> String {
    metadata
        .pointer("/output/summary")
        .or_else(|| metadata.get("summary"))
        .and_then(|value| value.as_str())
        .map(truncate_feedback_text)
        .unwrap_or_else(|| "metadata recorded".to_owned())
}

fn truncate_feedback_text(value: &str) -> String {
    const MAX_CHARS: usize = 160;
    if value.chars().count() <= MAX_CHARS {
        return value.to_owned();
    }
    let mut truncated = value.chars().take(MAX_CHARS).collect::<String>();
    truncated.push_str("...");
    truncated
}
```

- [ ] **Step 5: Replace review command stub**

In `trade-engine/src/runner.rs`:

```rust
async fn review_command(
    config: EngineConfig,
    args: ReviewArgs,
) -> Result<String, TradeEngineError> {
    let db = BlockingMainDb::open(config.main_db_path()?)?;
    let context = db
        .feedback_review_context(args.since_hours, args.limit)
        .await?;
    Ok(crate::feedback::render_review(&context))
}
```

- [ ] **Step 6: Run review tests**

Run:

```sh
cargo test --all-features feedback_review_context_reads_recent_operator_actions runner_args_parse_review_command
```

Expected: pass.

- [ ] **Step 7: Commit review command**

Run:

```sh
git add trade-engine/src/feedback.rs trade-engine/src/storage/mod.rs trade-engine/src/runner.rs trade-engine/tests/storage_audit.rs
git commit -m "feat: add feedback review command"
```

## Task 4: Feedback Query Schema, Provider, And Validation

**Files:**
- Create: `trade-engine/schemas/feedback-output.schema.json`
- Create: `trade-engine/scripts/codex-feedback`
- Modify: `trade-engine/src/feedback.rs`
- Test: `trade-engine/tests/output_schema_contract.rs`
- Test: new `trade-engine/tests/feedback_provider.rs`

- [ ] **Step 1: Write failing schema contract expectation**

Add this test to `trade-engine/tests/output_schema_contract.rs`:

```rust
#[test]
fn feedback_output_schema_exists() {
    let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("feedback-output.schema.json");

    assert!(schema_path.exists(), "missing feedback output schema");
}
```

- [ ] **Step 2: Run failing schema test**

Run:

```sh
cargo test --all-features feedback_output_schema_exists
```

Expected: failure because the schema file does not exist.

- [ ] **Step 3: Add feedback output schema**

Create `trade-engine/schemas/feedback-output.schema.json` with a strict object schema:

```json
{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "summary": { "type": "string" },
    "findings": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "title": { "type": "string" },
          "detail": { "type": "string" },
          "evidence_refs": {
            "type": "array",
            "items": { "type": "string" }
          }
        },
        "required": ["title", "detail", "evidence_refs"]
      }
    },
    "risks": {
      "type": "array",
      "items": { "type": "string" }
    },
    "suggested_followups": {
      "type": "array",
      "items": { "type": "string" }
    },
    "policy_candidates": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "scope": { "type": "string" },
          "proposal": { "type": "string" },
          "requires_approval": { "type": "boolean" },
          "evidence_refs": {
            "type": "array",
            "items": { "type": "string" }
          }
        },
        "required": ["scope", "proposal", "requires_approval", "evidence_refs"]
      }
    },
    "evidence": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "ref": { "type": "string" },
          "description": { "type": "string" }
        },
        "required": ["ref", "description"]
      }
    }
  },
  "required": [
    "summary",
    "findings",
    "risks",
    "suggested_followups",
    "policy_candidates",
    "evidence"
  ]
}
```

- [ ] **Step 4: Add feedback response types and validation**

In `trade-engine/src/feedback.rs`, add serde types:

```rust
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEEDBACK_REQUEST_SCHEMA_VERSION: u32 = 1;
const MAX_FEEDBACK_PROMPT_CHARS: usize = 2_000;
const MAX_FEEDBACK_SUMMARY_CHARS: usize = 2_000;
const MAX_FEEDBACK_ITEMS: usize = 16;
const MAX_FEEDBACK_ITEM_CHARS: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedbackQueryRequest {
    pub schema_version: u32,
    pub prompt: String,
    pub context: FeedbackQueryContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedbackQueryContext {
    pub review: serde_json::Value,
    pub redaction_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackOutput {
    pub summary: String,
    pub findings: Vec<FeedbackFinding>,
    pub risks: Vec<String>,
    pub suggested_followups: Vec<String>,
    pub policy_candidates: Vec<FeedbackPolicyCandidate>,
    pub evidence: Vec<FeedbackEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackFinding {
    pub title: String,
    pub detail: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackPolicyCandidate {
    pub scope: String,
    pub proposal: String,
    pub requires_approval: bool,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackEvidence {
    pub ref_: String,
    pub description: String,
}
```

Use `#[serde(rename = "ref")]` on `ref_` in actual code:

```rust
#[serde(rename = "ref")]
pub ref_: String,
```

Add `validate()` with specific limits:

```rust
impl FeedbackOutput {
    pub fn validate(self) -> Result<ValidatedFeedbackOutput, FeedbackValidationError> {
        validate_feedback_text("summary", &self.summary, MAX_FEEDBACK_SUMMARY_CHARS)?;
        validate_feedback_items("findings", self.findings.len())?;
        validate_feedback_items("risks", self.risks.len())?;
        validate_feedback_items("suggested_followups", self.suggested_followups.len())?;
        validate_feedback_items("policy_candidates", self.policy_candidates.len())?;
        validate_feedback_items("evidence", self.evidence.len())?;

        for finding in &self.findings {
            validate_feedback_text("findings.title", &finding.title, MAX_FEEDBACK_ITEM_CHARS)?;
            validate_feedback_text("findings.detail", &finding.detail, MAX_FEEDBACK_ITEM_CHARS)?;
            validate_feedback_items("findings.evidence_refs", finding.evidence_refs.len())?;
            for evidence_ref in &finding.evidence_refs {
                validate_feedback_text(
                    "findings.evidence_refs",
                    evidence_ref,
                    MAX_FEEDBACK_ITEM_CHARS,
                )?;
            }
        }
        for risk in &self.risks {
            validate_feedback_text("risks", risk, MAX_FEEDBACK_ITEM_CHARS)?;
        }
        for followup in &self.suggested_followups {
            validate_feedback_text("suggested_followups", followup, MAX_FEEDBACK_ITEM_CHARS)?;
        }
        for candidate in &self.policy_candidates {
            validate_feedback_text(
                "policy_candidates.scope",
                &candidate.scope,
                MAX_FEEDBACK_ITEM_CHARS,
            )?;
            validate_feedback_text(
                "policy_candidates.proposal",
                &candidate.proposal,
                MAX_FEEDBACK_ITEM_CHARS,
            )?;
            validate_feedback_items(
                "policy_candidates.evidence_refs",
                candidate.evidence_refs.len(),
            )?;
        }
        for evidence in &self.evidence {
            validate_feedback_text("evidence.ref", &evidence.ref_, MAX_FEEDBACK_ITEM_CHARS)?;
            validate_feedback_text(
                "evidence.description",
                &evidence.description,
                MAX_FEEDBACK_ITEM_CHARS,
            )?;
        }
        Ok(ValidatedFeedbackOutput { output: self })
    }
}
```

Add error type:

```rust
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FeedbackValidationError {
    #[error("feedback {field} must be {max_chars} characters or fewer")]
    TextTooLong {
        field: &'static str,
        max_chars: usize,
    },
    #[error("feedback {field} must contain {max_items} items or fewer")]
    TooManyItems {
        field: &'static str,
        max_items: usize,
    },
}
```

- [ ] **Step 5: Add command provider**

In `trade-engine/src/feedback.rs`, add:

```rust
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::timeout;

#[async_trait]
pub trait FeedbackProvider: Send + Sync {
    async fn query(
        &self,
        request: &FeedbackQueryRequest,
    ) -> Result<FeedbackProviderResponse, FeedbackProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackProviderResponse {
    pub raw_output_json: String,
    pub output: ValidatedFeedbackOutput,
}

#[derive(Debug, Clone)]
pub struct CommandFeedbackProvider {
    command: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
}
```

Implement `FeedbackProvider for CommandFeedbackProvider` with this concrete sequence:

1. Encode `FeedbackQueryRequest` to a JSON string with `serde_json::to_string`.
2. Spawn `self.command` with `self.args`, piped stdin, piped stdout, and piped stderr.
3. Write the JSON string to child stdin and close stdin.
4. Read stdout and stderr with byte caps `self.max_stdout_bytes` and `self.max_stderr_bytes`.
5. Wrap the child wait in `tokio::time::timeout(self.timeout, ...)`.
6. If the command exits non-zero, return `FeedbackProviderError::CommandFailed`.
7. Parse stdout as `FeedbackOutput`.
8. Call `FeedbackOutput::validate`.
9. Return `FeedbackProviderResponse { raw_output_json, output }`.

Add a `codex_default` constructor with default command `sh` and args
`["trade-engine/scripts/codex-feedback"]`:

```rust
impl CommandFeedbackProvider {
    pub fn codex_default() -> Self {
        Self {
            command: PathBuf::from("sh"),
            args: vec!["trade-engine/scripts/codex-feedback".to_owned()],
            timeout: Duration::from_secs(120),
            max_stdout_bytes: 262_144,
            max_stderr_bytes: 16_384,
        }
    }
}
```

- [ ] **Step 6: Add provider tests**

Create `trade-engine/tests/feedback_provider.rs` with:

```rust
use std::fs;

use tempfile::tempdir;
use trade_engine::feedback::{
    CommandFeedbackProvider, FeedbackProvider, FeedbackQueryContext, FeedbackQueryRequest,
    FEEDBACK_REQUEST_SCHEMA_VERSION,
};

#[tokio::test]
async fn command_feedback_provider_sends_request_and_parses_stdout() {
    let tempdir = tempdir().expect("tempdir");
    let request_path = tempdir.path().join("feedback-request.json");
    let script_path = tempdir.path().join("feedback.sh");
    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\ncat > {}\nprintf '%s\\n' '{{\"summary\":\"reviewed\",\"findings\":[],\"risks\":[],\"suggested_followups\":[],\"policy_candidates\":[],\"evidence\":[]}}'\n",
            request_path.display()
        ),
    )
    .expect("write feedback script");

    let provider = CommandFeedbackProvider::new_for_test(
        "sh",
        vec![script_path.to_string_lossy().to_string()],
    );
    let response = provider
        .query(&FeedbackQueryRequest {
            schema_version: FEEDBACK_REQUEST_SCHEMA_VERSION,
            prompt: "summarize".to_owned(),
            context: FeedbackQueryContext {
                review: serde_json::json!({"events": []}),
                redaction_notes: vec!["account_suffix_only".to_owned()],
            },
        })
        .await
        .expect("feedback output");

    assert_eq!(response.output.output().summary, "reviewed");
    let request_json = fs::read_to_string(request_path).expect("request json");
    let request_value: serde_json::Value =
        serde_json::from_str(&request_json).expect("request json value");
    assert_eq!(request_value["schema_version"], 1);
    assert_eq!(request_value["prompt"], "summarize");
}
```

Expose `new_for_test` only under `#[cfg(test)]` or as a crate-public constructor if integration tests need it:

```rust
pub fn new_for_test(command: impl Into<PathBuf>, args: Vec<String>) -> Self {
    Self {
        command: command.into(),
        args,
        timeout: Duration::from_secs(5),
        max_stdout_bytes: 262_144,
        max_stderr_bytes: 16_384,
    }
}
```

- [ ] **Step 7: Add wrapper script**

Create `trade-engine/scripts/codex-feedback` following the existing shell wrapper style:

```sh
#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
TRADE_ENGINE_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
REPO_DIR=$(CDPATH= cd -- "$TRADE_ENGINE_DIR/.." && pwd)
SCHEMA_PATH="$TRADE_ENGINE_DIR/schemas/feedback-output.schema.json"

REQUEST_FILE=$(mktemp)
PROMPT_FILE=$(mktemp)
OUTPUT_FILE=$(mktemp)
cleanup() {
  rm -f "$REQUEST_FILE" "$PROMPT_FILE" "$OUTPUT_FILE"
}
trap cleanup EXIT

cat > "$REQUEST_FILE"

cat > "$PROMPT_FILE" <<'EOF'
You are a read-only trading runtime review assistant.

Rules:
- Summarize and diagnose only the supplied audit context.
- Do not submit, cancel, amend, or recommend blind broker writes.
- Do not ask for secrets, credentials, tokens, hashkeys, API keys, or full account keys.
- Treat account suffixes as the only account identifiers.
- Cite supplied evidence_refs for concrete claims.
- Mark missing or insufficient evidence explicitly in risks.
- Policy candidates are unapplied review ideas only and require explicit approval later.
- Return only JSON matching the provided schema.

Feedback request JSON follows:
EOF
cat "$REQUEST_FILE" >> "$PROMPT_FILE"

codex exec \
  --cd "$REPO_DIR" \
  --ephemeral \
  --sandbox read-only \
  --color never \
  --output-schema "$SCHEMA_PATH" \
  --output-last-message "$OUTPUT_FILE" \
  - < "$PROMPT_FILE" >/dev/null

cat "$OUTPUT_FILE"
```

- [ ] **Step 8: Run provider/schema tests**

Run:

```sh
cargo test --all-features feedback_output_schema_exists codex_output_schemas_are_openai_strict_compatible command_feedback_provider_sends_request_and_parses_stdout
```

Expected: pass.

- [ ] **Step 9: Commit query provider foundation**

Run:

```sh
git add trade-engine/src/feedback.rs trade-engine/schemas/feedback-output.schema.json trade-engine/scripts/codex-feedback trade-engine/tests/output_schema_contract.rs trade-engine/tests/feedback_provider.rs
git commit -m "feat: add feedback query provider"
```

## Task 5: Feedback Query Command And Audit

**Files:**
- Modify: `trade-engine/src/storage/mod.rs`
- Modify: `trade-engine/src/feedback.rs`
- Modify: `trade-engine/src/runner.rs`
- Test: `trade-engine/tests/storage_audit.rs`
- Test: `trade-engine/src/runner.rs`

- [ ] **Step 1: Write failing audit test**

Add to `trade-engine/tests/storage_audit.rs`:

```rust
#[test]
fn record_feedback_query_audit_records_completed_and_failed_attempts() {
    let tempdir = tempdir().expect("tempdir");
    let db_path = tempdir.path().join("main.db");
    let mut db = MainDb::open(&db_path).expect("open db");

    db.record_feedback_query_completed(
        1,
        24,
        20,
        "why?",
        &serde_json::json!({"operator_actions": 0}),
        "{\"summary\":\"ok\"}",
        &serde_json::json!({"summary": "ok"}),
    )
    .expect("record completed");
    db.record_feedback_query_failed(
        1,
        24,
        20,
        "why?",
        &serde_json::json!({"operator_actions": 0}),
        "provider failed",
    )
    .expect("record failed");

    let connection = Connection::open(&db_path).expect("inspect db");
    let completed_count: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM operator_actions WHERE action_type = 'feedback_query_completed'",
            [],
            |row| row.get(0),
        )
        .expect("completed count");
    let failed_count: u64 = connection
        .query_row(
            "SELECT COUNT(*) FROM operator_actions WHERE action_type = 'feedback_query_failed'",
            [],
            |row| row.get(0),
        )
        .expect("failed count");

    assert_eq!(completed_count, 1);
    assert_eq!(failed_count, 1);
}
```

- [ ] **Step 2: Run failing audit test**

Run:

```sh
cargo test --all-features record_feedback_query_audit_records_completed_and_failed_attempts
```

Expected: compile failure because audit methods do not exist.

- [ ] **Step 3: Implement feedback query audit storage**

In `trade-engine/src/storage/mod.rs`, add methods:

```rust
pub fn record_feedback_query_completed(
    &mut self,
    schema_version: u32,
    since_hours: u64,
    limit: usize,
    prompt: &str,
    context_summary: &serde_json::Value,
    raw_output_json: &str,
    parsed_output: &serde_json::Value,
) -> Result<(), TradeEngineError> {
    self.record_feedback_query_action(
        "feedback_query_completed",
        schema_version,
        since_hours,
        limit,
        prompt,
        context_summary,
        Some(raw_output_json),
        Some(parsed_output),
        None,
    )
}

pub fn record_feedback_query_failed(
    &mut self,
    schema_version: u32,
    since_hours: u64,
    limit: usize,
    prompt: &str,
    context_summary: &serde_json::Value,
    error: &str,
) -> Result<(), TradeEngineError> {
    self.record_feedback_query_action(
        "feedback_query_failed",
        schema_version,
        since_hours,
        limit,
        prompt,
        context_summary,
        None,
        None,
        Some(error),
    )
}
```

Implement the private helper with `validate_json` on metadata:

```rust
fn record_feedback_query_action(
    &mut self,
    action_type: &str,
    schema_version: u32,
    since_hours: u64,
    limit: usize,
    prompt: &str,
    context_summary: &serde_json::Value,
    raw_output_json: Option<&str>,
    parsed_output: Option<&serde_json::Value>,
    error: Option<&str>,
) -> Result<(), TradeEngineError> {
    let observed_at = timestamp()?;
    let metadata_json = serde_json::json!({
        "schema_version": schema_version,
        "review_window": {
            "since_hours": since_hours,
            "limit": limit
        },
        "prompt": prompt,
        "context_summary": context_summary,
        "provider": "codex-feedback",
        "raw_output_json": raw_output_json,
        "parsed_output": parsed_output,
        "error": error
    })
    .to_string();
    validate_json("feedback query metadata", &metadata_json)?;
    self.connection
        .execute(
            "INSERT INTO operator_actions (
                observed_at,
                operator_id,
                action_type,
                target_type,
                target_id,
                reason,
                metadata_json
            )
            VALUES (?1, NULL, ?2, 'feedback_query', NULL, ?3, ?4)",
            params![observed_at, action_type, error, metadata_json],
        )
        .map(|_| ())
        .map_err(sqlite_error)
}
```

Add async wrappers for completed and failed query audit records on `BlockingMainDb`.

- [ ] **Step 4: Build query request and renderer**

In `trade-engine/src/feedback.rs`, add:

```rust
pub fn build_query_request(
    prompt: String,
    context: &FeedbackReviewContext,
) -> Result<FeedbackQueryRequest, FeedbackValidationError> {
    validate_feedback_text("prompt", &prompt, MAX_FEEDBACK_PROMPT_CHARS)?;
    Ok(FeedbackQueryRequest {
        schema_version: FEEDBACK_REQUEST_SCHEMA_VERSION,
        prompt,
        context: FeedbackQueryContext {
            review: review_context_json(context),
            redaction_notes: vec![
                "account_suffix_only".to_owned(),
                "no_secrets_or_full_account_keys".to_owned(),
            ],
        },
    })
}

pub fn validate_query_prompt(prompt: &str) -> Result<(), TradeEngineError> {
    if prompt.trim().is_empty() {
        return Err(TradeEngineError::config(
            "query --prompt must not be empty",
        ));
    }
    Ok(())
}

pub fn render_query(output: &ValidatedFeedbackOutput) -> String {
    let answer = output.output();
    let mut block = StructuredOutput::new();
    block.heading("Query answer").field("summary", &answer.summary);
    block.heading("Findings");
    if answer.findings.is_empty() {
        block.empty_item();
    } else {
        for finding in &answer.findings {
            block.item(
                &finding.title,
                &[OutputField::new("detail", truncate_feedback_text(&finding.detail))],
            );
        }
    }
    block.heading("Risks");
    if answer.risks.is_empty() {
        block.empty_item();
    } else {
        for risk in &answer.risks {
            block.item(truncate_feedback_text(risk), &[]);
        }
    }
    block.heading("Policy candidates");
    if answer.policy_candidates.is_empty() {
        block.empty_item();
    } else {
        for candidate in &answer.policy_candidates {
            block.item(
                &candidate.scope,
                &[
                    OutputField::new("requires_approval", candidate.requires_approval),
                    OutputField::new("proposal", truncate_feedback_text(&candidate.proposal)),
                ],
            );
        }
    }
    block.blank();
    block.finish()
}
```

Add `ValidatedFeedbackOutput::output(&self) -> &FeedbackOutput`.

- [ ] **Step 5: Replace query command stub**

In `trade-engine/src/runner.rs`:

```rust
async fn query_command(
    config: EngineConfig,
    args: QueryArgs,
) -> Result<String, TradeEngineError> {
    crate::feedback::validate_query_prompt(&args.prompt)?;
    let db = BlockingMainDb::open(config.main_db_path()?)?;
    let context = db
        .feedback_review_context(args.since_hours, args.limit)
        .await?;
    let request = crate::feedback::build_query_request(args.prompt.clone(), &context)
        .map_err(|error| TradeEngineError::runtime(error.to_string()))?;
    let context_summary = crate::feedback::query_context_summary(&context);
    let provider = crate::feedback::CommandFeedbackProvider::codex_default();
    match provider.query(&request).await {
        Ok(response) => {
            let parsed_output = serde_json::to_value(response.output.output()).map_err(|error| {
                TradeEngineError::runtime(format!("failed to encode feedback output: {error}"))
            })?;
            db.record_feedback_query_completed(
                request.schema_version,
                args.since_hours,
                args.limit,
                &args.prompt,
                &context_summary,
                &response.raw_output_json,
                &parsed_output,
            )
            .await?;
            Ok(crate::feedback::render_query(&response.output))
        }
        Err(error) => {
            db.record_feedback_query_failed(
                request.schema_version,
                args.since_hours,
                args.limit,
                &args.prompt,
                &context_summary,
                &error.to_string(),
            )
            .await?;
            Err(TradeEngineError::runtime(format!(
                "feedback query failed: {error}"
            )))
        }
    }
}
```

- [ ] **Step 6: Run query tests**

Run:

```sh
cargo test --all-features record_feedback_query_audit_records_completed_and_failed_attempts command_feedback_provider_sends_request_and_parses_stdout runner_args_parse_query_command_short_prompt runner_args_parse_query_command_long_prompt
```

Expected: pass.

- [ ] **Step 7: Commit query command**

Run:

```sh
git add trade-engine/src/feedback.rs trade-engine/src/storage/mod.rs trade-engine/src/runner.rs trade-engine/tests/storage_audit.rs
git commit -m "feat: add feedback query command"
```

## Task 6: README And Final Verification

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README**

Add a short section near existing `trade-engine run`, advisor, and universe command documentation:

```markdown
### Feedback and review CLI

`trade-engine status` prints a read-only Main DB and runtime audit summary.

`trade-engine review --since-hours 24 --limit 20` prints recent universe,
advisor, decision, and risk audit context without invoking Codex.

`trade-engine query -p "..." --since-hours 24 --limit 20` builds the same
bounded audit context and sends it to `trade-engine/scripts/codex-feedback`,
which invokes `codex exec` in read-only sandbox mode. The result is a review
answer only: it cannot submit broker orders and it cannot apply policy/config
changes.
```

- [ ] **Step 2: Run formatting**

Run:

```sh
cargo fmt --check
```

Expected: pass.

- [ ] **Step 3: Run focused tests**

Run:

```sh
cargo test --all-features feedback
cargo test --all-features runner_args_parse
cargo test --all-features storage_audit
cargo test --all-features output_schema_contract
```

Expected: pass.

- [ ] **Step 4: Run full required verification**

Run:

```sh
cargo check --workspace --no-default-features
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

Expected: pass.

- [ ] **Step 5: Commit docs and any final fixes**

Run:

```sh
git add README.md
git commit -m "docs: document feedback CLI"
```

If final fixes are needed, commit them separately with a focused message that matches the changed code.

## Self-Review

- Spec coverage:
  - `trade-engine status`: Task 2.
  - `trade-engine review`: Task 3.
  - `trade-engine query --prompt/-p`: Tasks 4 and 5.
  - Codex CLI call from query: Task 4 wrapper and Task 5 runner command.
  - No broker writes/policy mutation: Task 4 prompt, Task 5 command flow, README in Task 6.
  - Query audit recording: Task 5.
- Ambiguity scan:
  - No marker strings or vague "handle cases" steps remain.
- Type consistency:
  - Storage context types are introduced before feedback request construction uses them.
  - `feedback-output.schema.json` matches `FeedbackOutput` fields.
  - The provider uses the same `FeedbackQueryRequest` and `ValidatedFeedbackOutput` types as the runner.
