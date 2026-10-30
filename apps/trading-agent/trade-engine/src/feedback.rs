use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::TradeEngineError;
use crate::stdout::{OutputField, StructuredOutput};
use crate::storage::{
    AuditCounts, CommandTokenUsageSource, CommandTokenUsageSummary, FeedbackReviewContext,
    FeedbackStatusSummary,
};

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
            for evidence_ref in &candidate.evidence_refs {
                validate_feedback_text(
                    "policy_candidates.evidence_refs",
                    evidence_ref,
                    MAX_FEEDBACK_ITEM_CHARS,
                )?;
            }
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
    #[serde(rename = "ref")]
    pub ref_: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFeedbackOutput {
    output: FeedbackOutput,
}

impl ValidatedFeedbackOutput {
    pub fn output(&self) -> &FeedbackOutput {
        &self.output
    }
}

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

    pub fn new_for_test(command: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
            timeout: Duration::from_secs(5),
            max_stdout_bytes: 262_144,
            max_stderr_bytes: 16_384,
        }
    }
}

#[derive(Debug, Error)]
pub enum FeedbackProviderError {
    #[error("failed to encode feedback request: {message}")]
    RequestEncoding { message: String },
    #[error("feedback provider command I/O failed during {operation}: {message}")]
    CommandIo {
        operation: &'static str,
        message: String,
    },
    #[error("feedback provider timed out after {timeout:?}")]
    Timeout { timeout: Duration },
    #[error("feedback provider command exited with status {status}: {stderr}")]
    CommandFailed { status: String, stderr: String },
    #[error("feedback provider output exceeded {max_bytes} bytes for {stream}")]
    OutputTooLarge {
        stream: &'static str,
        max_bytes: usize,
    },
    #[error("feedback provider output was invalid: {reason}")]
    InvalidOutput { reason: String },
}

#[async_trait]
impl FeedbackProvider for CommandFeedbackProvider {
    async fn query(
        &self,
        request: &FeedbackQueryRequest,
    ) -> Result<FeedbackProviderResponse, FeedbackProviderError> {
        let request_json = serde_json::to_string(request).map_err(|error| {
            FeedbackProviderError::RequestEncoding {
                message: error.to_string(),
            }
        })?;
        let mut child = Command::new(&self.command)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| FeedbackProviderError::CommandIo {
                operation: "spawn",
                message: error.to_string(),
            })?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| FeedbackProviderError::CommandIo {
                operation: "stdin",
                message: "feedback provider stdin was unavailable".to_owned(),
            })?;
        stdin
            .write_all(request_json.as_bytes())
            .await
            .map_err(|error| FeedbackProviderError::CommandIo {
                operation: "stdin",
                message: error.to_string(),
            })?;
        drop(stdin);

        let output = timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| FeedbackProviderError::Timeout {
                timeout: self.timeout,
            })?
            .map_err(|error| FeedbackProviderError::CommandIo {
                operation: "wait",
                message: error.to_string(),
            })?;
        if output.stdout.len() > self.max_stdout_bytes {
            return Err(FeedbackProviderError::OutputTooLarge {
                stream: "stdout",
                max_bytes: self.max_stdout_bytes,
            });
        }
        if output.stderr.len() > self.max_stderr_bytes {
            return Err(FeedbackProviderError::OutputTooLarge {
                stream: "stderr",
                max_bytes: self.max_stderr_bytes,
            });
        }
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() {
            return Err(FeedbackProviderError::CommandFailed {
                status: output.status.to_string(),
                stderr,
            });
        }
        let raw_output_json = String::from_utf8(output.stdout).map_err(|error| {
            FeedbackProviderError::InvalidOutput {
                reason: format!("stdout was not UTF-8: {error}"),
            }
        })?;
        let output = serde_json::from_str::<FeedbackOutput>(&raw_output_json)
            .map_err(|error| FeedbackProviderError::InvalidOutput {
                reason: format!("failed to parse feedback provider output: {error}"),
            })?
            .validate()
            .map_err(|error| FeedbackProviderError::InvalidOutput {
                reason: error.to_string(),
            })?;
        Ok(FeedbackProviderResponse {
            raw_output_json,
            output,
        })
    }
}

pub fn render_status(
    summary: &FeedbackStatusSummary,
    environment: &str,
    account_suffix: &str,
) -> String {
    let mut output = StructuredOutput::new();
    output
        .heading("Trade Engine Status")
        .field("purpose", "read-only local runtime snapshot")
        .heading("Runtime readiness")
        .field("environment", environment)
        .field("account_suffix", account_suffix)
        .field(
            "database",
            format!(
                "migrations {}",
                migration_summary(&summary.applied_migrations)
            ),
        )
        .field(
            "broker_commands",
            broker_command_status(summary.unresolved_broker_commands),
        )
        .field("panic_stop", panic_stop_status(summary.panic_stop_active));

    output
        .heading("Audit totals")
        .field("runtime_runs", summary.audit_counts.runtime_runs)
        .field("runtime_ticks", summary.audit_counts.runtime_ticks)
        .field("decision_outputs", summary.audit_counts.decision_outputs)
        .field(
            "decision_proposals",
            summary.audit_counts.decision_proposals,
        )
        .field("risk_evaluations", summary.audit_counts.risk_evaluations)
        .field("risk_blocks", summary.audit_counts.risk_blocks)
        .field(
            "execution_requests",
            summary.audit_counts.execution_requests,
        )
        .field("fills", summary.audit_counts.fills);

    output.heading("AI command usage");
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

pub fn render_review(context: &FeedbackReviewContext) -> String {
    let mut output = StructuredOutput::new();
    output
        .heading("Activity Review")
        .field("purpose", "recent local audit events")
        .heading("Window")
        .field("since_hours", context.window.since_hours)
        .field("limit", context.window.limit)
        .field("events", context.operator_actions.len())
        .heading("Decision and risk activity")
        .field(
            "decision_outputs",
            context.status.audit_counts.decision_outputs,
        )
        .field(
            "decision_proposals",
            context.status.audit_counts.decision_proposals,
        )
        .field(
            "risk_evaluations",
            context.status.audit_counts.risk_evaluations,
        )
        .field("risk_blocks", context.status.audit_counts.risk_blocks);

    output.heading("Recent events");
    if context.operator_actions.is_empty() {
        output.empty_item();
    } else {
        for action in &context.operator_actions {
            output.item(
                format!("#{} {}", action.id, action.action_type),
                &[
                    OutputField::new("time", &action.observed_at),
                    OutputField::new("target", action_target(action)),
                    OutputField::new("reason", action.reason.as_deref().unwrap_or("none")),
                    OutputField::new("summary", metadata_summary(&action.metadata)),
                ],
            );
        }
    }
    output.blank();
    output.finish()
}

fn broker_command_status(unresolved: bool) -> &'static str {
    if unresolved {
        "attention_required"
    } else {
        "clear"
    }
}

fn panic_stop_status(active: bool) -> &'static str {
    if active { "active" } else { "inactive" }
}

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
        return Err(TradeEngineError::config("query --prompt must not be empty"));
    }
    Ok(())
}

pub fn query_context_summary(context: &FeedbackReviewContext) -> serde_json::Value {
    serde_json::json!({
        "window": {
            "since_hours": context.window.since_hours,
            "limit": context.window.limit
        },
        "operator_actions": context.operator_actions.len(),
        "audit_counts": audit_counts_json(&context.status.audit_counts),
        "unresolved_broker_commands": context.status.unresolved_broker_commands,
        "panic_stop_active": context.status.panic_stop_active
    })
}

pub fn render_query(output: &ValidatedFeedbackOutput) -> String {
    let answer = output.output();
    let mut block = StructuredOutput::new();
    block
        .heading("Query answer")
        .field("summary", &answer.summary);

    block.heading("Findings");
    if answer.findings.is_empty() {
        block.empty_item();
    } else {
        for finding in &answer.findings {
            block.item(
                &finding.title,
                &[OutputField::new(
                    "detail",
                    truncate_feedback_text(&finding.detail),
                )],
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

    block.heading("Suggested followups");
    if answer.suggested_followups.is_empty() {
        block.empty_item();
    } else {
        for followup in &answer.suggested_followups {
            block.item(truncate_feedback_text(followup), &[]);
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

fn review_context_json(context: &FeedbackReviewContext) -> serde_json::Value {
    let operator_actions = context
        .operator_actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            serde_json::json!({
                "evidence_ref": format!("operator_actions[{index}]"),
                "id": action.id,
                "observed_at": action.observed_at,
                "action_type": action.action_type,
                "target": action_target(action),
                "reason": action.reason,
                "summary": metadata_summary(&action.metadata)
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "window": {
            "since_hours": context.window.since_hours,
            "limit": context.window.limit
        },
        "status": {
            "applied_migrations": context.status.applied_migrations,
            "audit_counts": audit_counts_json(&context.status.audit_counts),
            "unresolved_broker_commands": context.status.unresolved_broker_commands,
            "panic_stop_active": context.status.panic_stop_active,
            "command_token_usage": command_token_usage_json(&context.status.command_token_usage)
        },
        "operator_actions": operator_actions
    })
}

fn audit_counts_json(counts: &AuditCounts) -> serde_json::Value {
    serde_json::json!({
        "runtime_runs": counts.runtime_runs,
        "runtime_ticks": counts.runtime_ticks,
        "portfolio_snapshots": counts.portfolio_snapshots,
        "market_snapshots": counts.market_snapshots,
        "strategy_signals": counts.strategy_signals,
        "decision_inputs": counts.decision_inputs,
        "decision_outputs": counts.decision_outputs,
        "decision_proposals": counts.decision_proposals,
        "risk_evaluations": counts.risk_evaluations,
        "risk_blocks": counts.risk_blocks,
        "execution_requests": counts.execution_requests,
        "execution_events": counts.execution_events,
        "broker_command_events": counts.broker_command_events,
        "fills": counts.fills
    })
}

fn command_token_usage_json(summaries: &[CommandTokenUsageSummary]) -> serde_json::Value {
    summaries
        .iter()
        .map(|summary| {
            serde_json::json!({
                "source": command_usage_source_label(summary.source),
                "recorded_invocations": summary.recorded_invocations,
                "input_tokens": summary.input_tokens,
                "cached_input_tokens": summary.cached_input_tokens,
                "output_tokens": summary.output_tokens,
                "reasoning_output_tokens": summary.reasoning_output_tokens,
                "total_tokens": summary.total_tokens
            })
        })
        .collect::<Vec<_>>()
        .into()
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

fn action_target(action: &crate::storage::FeedbackOperatorAction) -> String {
    action
        .target_type
        .as_deref()
        .zip(action.target_id.as_deref())
        .map(|(target_type, target_id)| format!("{target_type}:{target_id}"))
        .unwrap_or_else(|| "none".to_owned())
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

fn validate_feedback_text(
    field: &'static str,
    value: &str,
    max_chars: usize,
) -> Result<(), FeedbackValidationError> {
    if value.chars().count() > max_chars {
        return Err(FeedbackValidationError::TextTooLong { field, max_chars });
    }
    Ok(())
}

fn validate_feedback_items(
    field: &'static str,
    item_count: usize,
) -> Result<(), FeedbackValidationError> {
    if item_count > MAX_FEEDBACK_ITEMS {
        return Err(FeedbackValidationError::TooManyItems {
            field,
            max_items: MAX_FEEDBACK_ITEMS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        AuditCounts, FeedbackOperatorAction, FeedbackReviewWindow, FeedbackStatusSummary,
    };

    #[test]
    fn render_status_labels_local_runtime_readiness() {
        let summary = FeedbackStatusSummary {
            applied_migrations: vec![1, 2, 3],
            audit_counts: AuditCounts {
                runtime_runs: 2,
                runtime_ticks: 5,
                risk_blocks: 1,
                ..AuditCounts::default()
            },
            unresolved_broker_commands: false,
            panic_stop_active: false,
            command_token_usage: Vec::new(),
        };

        let rendered = render_status(&summary, "paper", "5678");

        assert!(rendered.contains("Trade Engine Status"));
        assert!(rendered.contains("purpose: read-only local runtime snapshot"));
        assert!(rendered.contains("Runtime readiness"));
        assert!(rendered.contains("environment: paper"));
        assert!(rendered.contains("account_suffix: 5678"));
        assert!(rendered.contains("database: migrations 1..3 (3 applied)"));
        assert!(rendered.contains("broker_commands: clear"));
        assert!(rendered.contains("panic_stop: inactive"));
        assert!(rendered.contains("Audit totals"));
        assert!(rendered.contains("AI command usage"));
    }

    #[test]
    fn render_review_labels_recent_activity_context() {
        let context = feedback_context();

        let rendered = render_review(&context);

        assert!(rendered.contains("Activity Review"));
        assert!(rendered.contains("purpose: recent local audit events"));
        assert!(rendered.contains("Window"));
        assert!(rendered.contains("events: 1"));
        assert!(rendered.contains("Decision and risk activity"));
        assert!(rendered.contains("Recent events"));
        assert!(rendered.contains("#7 risk_blocked"));
        assert!(rendered.contains("time=2026-06-22T00:00:00Z"));
        assert!(rendered.contains("target=risk_evaluation:5"));
        assert!(rendered.contains("reason=oversized"));
        assert!(rendered.contains("summary=blocked oversized order"));
    }

    #[test]
    fn build_query_request_serializes_bounded_redacted_review_context() {
        let context = feedback_context();

        let request = build_query_request("summarize recent risk blocks".to_owned(), &context)
            .expect("query request");

        assert_eq!(request.schema_version, FEEDBACK_REQUEST_SCHEMA_VERSION);
        assert_eq!(request.prompt, "summarize recent risk blocks");
        assert_eq!(
            request.context.redaction_notes,
            vec![
                "account_suffix_only".to_owned(),
                "no_secrets_or_full_account_keys".to_owned()
            ]
        );
        assert_eq!(request.context.review["window"]["since_hours"], 24);
        assert_eq!(
            request.context.review["operator_actions"][0]["action_type"],
            "risk_blocked"
        );
        assert_eq!(
            request.context.review["operator_actions"][0]["summary"],
            "blocked oversized order"
        );
    }

    #[test]
    fn validate_query_prompt_rejects_blank_prompt() {
        let error = validate_query_prompt("   ").expect_err("blank prompt should fail");

        assert!(
            error
                .to_string()
                .contains("query --prompt must not be empty")
        );
    }

    #[test]
    fn render_query_outputs_validated_feedback_sections() {
        let output = FeedbackOutput {
            summary: "recent risk blocks came from concentration limits".to_owned(),
            findings: vec![FeedbackFinding {
                title: "Concentration block".to_owned(),
                detail: "Risk blocked a proposal after exposure exceeded the configured cap"
                    .to_owned(),
                evidence_refs: vec!["operator_actions[0]".to_owned()],
            }],
            risks: vec!["The review context has one operator action only".to_owned()],
            suggested_followups: vec!["Review concentration settings".to_owned()],
            policy_candidates: vec![FeedbackPolicyCandidate {
                scope: "risk.concentration".to_owned(),
                proposal: "Consider a lower default cap after offline review".to_owned(),
                requires_approval: true,
                evidence_refs: vec!["operator_actions[0]".to_owned()],
            }],
            evidence: vec![FeedbackEvidence {
                ref_: "operator_actions[0]".to_owned(),
                description: "risk_blocked action".to_owned(),
            }],
        }
        .validate()
        .expect("validated output");

        let rendered = render_query(&output);

        assert!(rendered.contains("Query answer"));
        assert!(rendered.contains("recent risk blocks"));
        assert!(rendered.contains("Concentration block"));
        assert!(rendered.contains("Policy candidates"));
        assert!(rendered.contains("requires_approval"));
    }

    fn feedback_context() -> FeedbackReviewContext {
        FeedbackReviewContext {
            window: FeedbackReviewWindow {
                since_hours: 24,
                limit: 20,
            },
            status: FeedbackStatusSummary {
                applied_migrations: vec![1, 2, 3],
                audit_counts: AuditCounts {
                    risk_blocks: 1,
                    ..AuditCounts::default()
                },
                unresolved_broker_commands: false,
                panic_stop_active: false,
                command_token_usage: Vec::new(),
            },
            operator_actions: vec![FeedbackOperatorAction {
                id: 7,
                observed_at: "2026-06-22T00:00:00Z".to_owned(),
                action_type: "risk_blocked".to_owned(),
                target_type: Some("risk_evaluation".to_owned()),
                target_id: Some("5".to_owned()),
                reason: Some("oversized".to_owned()),
                metadata: serde_json::json!({
                    "summary": "blocked oversized order",
                    "details": {
                        "account": "redacted"
                    }
                }),
            }],
        }
    }
}
