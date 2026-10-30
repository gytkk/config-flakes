use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::timeout;
use trading_agent_kis_client::BrokerEnvironment;

use crate::command_usage::{
    COMMAND_USAGE_EVENTS_PATH_ENV, CommandUsageCapture, ProviderTokenUsage,
};
use crate::decision::{DecisionOutputStatus, DecisionProposal};
use crate::gateway::types::{MarketScope, Symbol};
use crate::market_alignment::DomesticMarketAlignmentContext;
use crate::technical_analysis::{DailyTechnicalFeatures, OhlcvBarInterval};
use crate::universe::UniverseDiscoveryCandidate;

pub const MAX_ADVISOR_SUMMARY_CHARS: usize = 2_000;
pub const MAX_ADVISOR_ITEMS: usize = 16;
pub const MAX_ADVISOR_ITEM_CHARS: usize = 1_000;
pub const MAX_ADVISOR_EVIDENCE_SOURCE_CHARS: usize = 100;
pub const MAX_ADVISOR_PROPOSALS: usize = 16;
pub const ADVISOR_REQUEST_SCHEMA_VERSION: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorRequest {
    pub schema_version: u32,
    pub runtime: AdvisorRuntimeContext,
    pub current_state: AdvisorCurrentState,
}

impl AdvisorRequest {
    pub fn dry_run_market_tick(
        environment: BrokerEnvironment,
        account_suffix: impl Into<String>,
        market_hours: impl Into<String>,
        tick_index: u64,
        market_tick_count: u64,
        watchlist: impl IntoIterator<Item = Symbol>,
        current_tick: AdvisorMarketTickObservation,
    ) -> Self {
        let watchlist = watchlist.into_iter().collect::<Vec<_>>();
        let effective_candidates = watchlist.clone();
        Self::market_tick(
            AdvisorRuntimeContext {
                mode: "dry_run".to_owned(),
                environment: environment.as_str().to_owned(),
                account_suffix: account_suffix.into(),
                market: AdvisorMarketContext {
                    kind: "domestic_stock".to_owned(),
                    exchange_code: None,
                    currency: None,
                },
                market_hours: market_hours.into(),
                tick_index,
                market_tick_count,
                user_watchlist: watchlist.clone(),
                discovered_universe: Vec::new(),
                effective_candidates: effective_candidates.clone(),
                universe_discovery: AdvisorUniverseDiscoveryContext {
                    discovered_candidates: Vec::new(),
                    source: AdvisorUniverseDiscoverySource::disabled(),
                },
            },
            current_tick,
        )
    }

    pub fn market_tick(
        runtime: AdvisorRuntimeContext,
        current_tick: AdvisorMarketTickObservation,
    ) -> Self {
        Self {
            schema_version: ADVISOR_REQUEST_SCHEMA_VERSION,
            runtime,
            current_state: AdvisorCurrentState { current_tick },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorRuntimeContext {
    pub mode: String,
    pub environment: String,
    pub account_suffix: String,
    pub market: AdvisorMarketContext,
    pub market_hours: String,
    pub tick_index: u64,
    pub market_tick_count: u64,
    pub user_watchlist: Vec<Symbol>,
    pub discovered_universe: Vec<Symbol>,
    pub effective_candidates: Vec<Symbol>,
    pub universe_discovery: AdvisorUniverseDiscoveryContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorMarketContext {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

impl AdvisorMarketContext {
    pub fn from_market_scope(market_scope: &MarketScope) -> Self {
        match market_scope {
            MarketScope::DomesticStock => Self {
                kind: market_scope.kind().to_owned(),
                exchange_code: None,
                currency: None,
            },
            MarketScope::OverseasStock { exchange, currency } => Self {
                kind: market_scope.kind().to_owned(),
                exchange_code: Some(exchange.as_str().to_owned()),
                currency: Some(currency.as_str().to_owned()),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorUniverseDiscoveryContext {
    pub discovered_candidates: Vec<UniverseDiscoveryCandidate>,
    pub source: AdvisorUniverseDiscoverySource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorUniverseDiscoverySource {
    pub provider: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AdvisorUniverseDiscoverySource {
    pub fn disabled() -> Self {
        Self {
            provider: "disabled".to_owned(),
            status: "disabled".to_owned(),
            snapshot_id: None,
            snapshot_provider: None,
            observed_at: None,
            expires_at: None,
            summary: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorCurrentState {
    pub current_tick: AdvisorMarketTickObservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorMarketTickObservation {
    pub stage: String,
    pub market_session_open: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub technical_analysis: Vec<AdvisorTechnicalAnalysisEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domestic_market_alignment: Option<DomesticMarketAlignmentContext>,
    pub kis_api: AdvisorKisApiSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorTechnicalAnalysisEvidence {
    pub symbol: Symbol,
    pub audit_id: i64,
    pub observed_at: String,
    pub source: String,
    pub interval: OhlcvBarInterval,
    pub feature_set: String,
    pub features: DailyTechnicalFeatures,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorKisApiSummary {
    pub account: AdvisorAccountSummary,
    pub open_order_count: usize,
    pub open_orders: Vec<AdvisorOpenOrderSummary>,
    pub today_fill_count: usize,
    pub today_fills: Vec<AdvisorFillSummary>,
    pub quote_count: usize,
    pub quotes: Vec<AdvisorQuoteSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorAccountSummary {
    #[serde(with = "rust_decimal::serde::str")]
    pub cash: Decimal,
    pub position_count: usize,
    pub positions: Vec<AdvisorPositionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorPositionSummary {
    pub symbol: Symbol,
    pub quantity: u64,
    #[serde(with = "rust_decimal::serde::str")]
    pub average_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorQuoteSummary {
    pub symbol: Symbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    pub recent_prices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorOpenOrderSummary {
    pub state: String,
    pub symbol: Option<Symbol>,
    pub side: Option<String>,
    pub ordered_quantity: Option<u64>,
    pub filled_quantity: Option<u64>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub order_price: Option<Decimal>,
    pub trading_day: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvisorFillSummary {
    pub symbol: Option<Symbol>,
    pub side: Option<String>,
    pub quantity: u64,
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    pub trading_day: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvisorOutput {
    pub summary: String,
    pub observations: Vec<String>,
    pub risks: Vec<String>,
    pub next_actions: Vec<String>,
    pub rationale: String,
    pub evidence: Vec<AdvisorEvidence>,
    pub proposals: Vec<DecisionProposal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvisorEvidence {
    pub source: String,
    pub detail: String,
}

impl AdvisorOutput {
    pub fn validate(self) -> Result<ValidatedAdvisorOutput, AdvisorValidationError> {
        validate_text("summary", &self.summary, MAX_ADVISOR_SUMMARY_CHARS)?;
        validate_items("observations", &self.observations)?;
        validate_items("risks", &self.risks)?;
        validate_items("next_actions", &self.next_actions)?;
        validate_text("rationale", &self.rationale, MAX_ADVISOR_SUMMARY_CHARS)?;
        validate_evidence(&self.evidence)?;
        if self.proposals.len() > MAX_ADVISOR_PROPOSALS {
            return Err(AdvisorValidationError::TooManyItems {
                field: "proposals",
                max_items: MAX_ADVISOR_PROPOSALS,
            });
        }
        Ok(ValidatedAdvisorOutput { output: self })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidatedAdvisorOutput {
    output: AdvisorOutput,
}

impl ValidatedAdvisorOutput {
    pub fn output(&self) -> &AdvisorOutput {
        &self.output
    }

    pub fn into_output(self) -> AdvisorOutput {
        self.output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisorAudit {
    pub request: AdvisorRequest,
    pub provider: String,
    pub raw_output_json: String,
    pub token_usage: Option<ProviderTokenUsage>,
    pub status: DecisionOutputStatus,
    pub output: Option<ValidatedAdvisorOutput>,
    pub error: Option<String>,
}

impl AdvisorAudit {
    pub fn completed(
        request: AdvisorRequest,
        provider: impl Into<String>,
        raw_output_json: String,
        output: ValidatedAdvisorOutput,
    ) -> Self {
        Self {
            request,
            provider: provider.into(),
            raw_output_json,
            token_usage: None,
            status: DecisionOutputStatus::Parsed,
            output: Some(output),
            error: None,
        }
    }

    pub fn failed(
        request: AdvisorRequest,
        provider: impl Into<String>,
        raw_output_json: String,
        status: DecisionOutputStatus,
        error: impl Into<String>,
    ) -> Self {
        Self {
            request,
            provider: provider.into(),
            raw_output_json,
            token_usage: None,
            status,
            output: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisorProviderResponse {
    pub raw_output_json: String,
    pub token_usage: Option<ProviderTokenUsage>,
    pub output: ValidatedAdvisorOutput,
}

#[async_trait]
pub trait AdvisorProvider: Send + Sync {
    fn id(&self) -> &str;

    async fn advise(
        &self,
        request: &AdvisorRequest,
    ) -> Result<AdvisorProviderResponse, AdvisorProviderError>;
}

#[derive(Debug, Clone)]
pub struct CommandAdvisorProvider {
    command: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    max_stderr_bytes: usize,
    max_stdout_bytes: usize,
}

impl CommandAdvisorProvider {
    pub fn new(
        command: impl Into<PathBuf>,
        args: Vec<String>,
        timeout: Duration,
        max_stderr_bytes: usize,
        max_stdout_bytes: usize,
    ) -> Self {
        Self {
            command: command.into(),
            args,
            timeout,
            max_stderr_bytes,
            max_stdout_bytes,
        }
    }
}

#[async_trait]
impl AdvisorProvider for CommandAdvisorProvider {
    fn id(&self) -> &str {
        "command"
    }

    async fn advise(
        &self,
        request: &AdvisorRequest,
    ) -> Result<AdvisorProviderResponse, AdvisorProviderError> {
        let request_json =
            serde_json::to_vec(request).map_err(|error| AdvisorProviderError::Internal {
                message: format!("failed to encode advisor request: {error}"),
            })?;
        let usage_capture = CommandUsageCapture::new("advisor");

        let mut child = Command::new(&self.command)
            .args(&self.args)
            .env(COMMAND_USAGE_EVENTS_PATH_ENV, usage_capture.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| AdvisorProviderError::Io {
                operation: "spawn",
                message: error.to_string(),
                raw_output_json: String::new(),
            })?;

        let mut stdin = child.stdin.take().ok_or_else(|| AdvisorProviderError::Io {
            operation: "open stdin",
            message: "child stdin was not available".to_owned(),
            raw_output_json: String::new(),
        })?;
        stdin
            .write_all(&request_json)
            .await
            .map_err(|error| AdvisorProviderError::Io {
                operation: "write stdin",
                message: error.to_string(),
                raw_output_json: String::new(),
            })?;
        drop(stdin);

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdvisorProviderError::Io {
                operation: "open stdout",
                message: "child stdout was not available".to_owned(),
                raw_output_json: String::new(),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AdvisorProviderError::Io {
                operation: "open stderr",
                message: "child stderr was not available".to_owned(),
                raw_output_json: String::new(),
            })?;
        let max_stdout_bytes = self.max_stdout_bytes;
        let max_stderr_bytes = self.max_stderr_bytes;
        let command_output = timeout(self.timeout, async move {
            let (status, stdout, stderr) = tokio::try_join!(
                child.wait(),
                read_capped_output(stdout, max_stdout_bytes),
                read_capped_output(stderr, max_stderr_bytes),
            )?;
            Ok::<_, std::io::Error>(CommandOutput {
                status,
                stdout,
                stderr,
            })
        })
        .await
        .map_err(|_| AdvisorProviderError::Timeout {
            timeout: self.timeout,
        })?
        .map_err(|error| AdvisorProviderError::Io {
            operation: "read output",
            message: error.to_string(),
            raw_output_json: String::new(),
        })?;
        let raw_output_json = capped_utf8_lossy(&command_output.stdout);
        let token_usage = usage_capture.read_token_usage();
        if !command_output.status.success() {
            return Err(AdvisorProviderError::CommandFailed {
                status: command_output.status.to_string(),
                stderr: capped_utf8_lossy(&command_output.stderr),
                raw_output_json,
                token_usage,
            });
        }
        if command_output.stdout.truncated {
            return Err(AdvisorProviderError::InvalidOutput {
                raw_output_json,
                reason: format!(
                    "stdout exceeded max_stdout_bytes ({})",
                    self.max_stdout_bytes
                ),
                token_usage,
            });
        }

        let parsed = serde_json::from_str::<AdvisorOutput>(&raw_output_json).map_err(|error| {
            AdvisorProviderError::InvalidOutput {
                raw_output_json: raw_output_json.clone(),
                reason: format!("failed to parse advisor provider output: {error}"),
                token_usage: token_usage.clone(),
            }
        })?;
        let output = parsed
            .validate()
            .map_err(|error| AdvisorProviderError::InvalidOutput {
                raw_output_json: raw_output_json.clone(),
                reason: error.to_string(),
                token_usage: token_usage.clone(),
            })?;
        Ok(AdvisorProviderResponse {
            raw_output_json,
            token_usage,
            output,
        })
    }
}

#[derive(Debug, Error)]
pub enum AdvisorProviderError {
    #[error("advisor provider internal error: {message}")]
    Internal { message: String },
    #[error("advisor provider command I/O failed during {operation}: {message}")]
    Io {
        operation: &'static str,
        message: String,
        raw_output_json: String,
    },
    #[error("advisor provider timed out after {timeout:?}")]
    Timeout { timeout: Duration },
    #[error("advisor provider command exited with status {status}: {stderr}")]
    CommandFailed {
        status: String,
        stderr: String,
        raw_output_json: String,
        token_usage: Option<ProviderTokenUsage>,
    },
    #[error("advisor provider output was invalid: {reason}")]
    InvalidOutput {
        raw_output_json: String,
        reason: String,
        token_usage: Option<ProviderTokenUsage>,
    },
}

impl AdvisorProviderError {
    pub fn output_status(&self) -> DecisionOutputStatus {
        match self {
            Self::InvalidOutput { .. } => DecisionOutputStatus::Rejected,
            Self::Internal { .. }
            | Self::Io { .. }
            | Self::Timeout { .. }
            | Self::CommandFailed { .. } => DecisionOutputStatus::Failed,
        }
    }

    pub fn raw_output_json(&self) -> &str {
        match self {
            Self::CommandFailed {
                raw_output_json, ..
            }
            | Self::InvalidOutput {
                raw_output_json, ..
            }
            | Self::Io {
                raw_output_json, ..
            } => raw_output_json,
            Self::Internal { .. } | Self::Timeout { .. } => "",
        }
    }

    pub fn token_usage(&self) -> Option<&ProviderTokenUsage> {
        match self {
            Self::CommandFailed { token_usage, .. } | Self::InvalidOutput { token_usage, .. } => {
                token_usage.as_ref()
            }
            Self::Internal { .. } | Self::Io { .. } | Self::Timeout { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdvisorValidationError {
    #[error("advisor {field} must be {max_chars} characters or fewer")]
    TextTooLong {
        field: &'static str,
        max_chars: usize,
    },
    #[error("advisor {field} must contain {max_items} items or fewer")]
    TooManyItems {
        field: &'static str,
        max_items: usize,
    },
}

fn validate_text(
    field: &'static str,
    value: &str,
    max_chars: usize,
) -> Result<(), AdvisorValidationError> {
    if value.chars().count() > max_chars {
        return Err(AdvisorValidationError::TextTooLong { field, max_chars });
    }
    Ok(())
}

fn validate_items(field: &'static str, values: &[String]) -> Result<(), AdvisorValidationError> {
    if values.len() > MAX_ADVISOR_ITEMS {
        return Err(AdvisorValidationError::TooManyItems {
            field,
            max_items: MAX_ADVISOR_ITEMS,
        });
    }
    for value in values {
        validate_text(field, value, MAX_ADVISOR_ITEM_CHARS)?;
    }
    Ok(())
}

fn validate_evidence(values: &[AdvisorEvidence]) -> Result<(), AdvisorValidationError> {
    if values.len() > MAX_ADVISOR_ITEMS {
        return Err(AdvisorValidationError::TooManyItems {
            field: "evidence",
            max_items: MAX_ADVISOR_ITEMS,
        });
    }
    for value in values {
        validate_text(
            "evidence.source",
            &value.source,
            MAX_ADVISOR_EVIDENCE_SOURCE_CHARS,
        )?;
        validate_text("evidence.detail", &value.detail, MAX_ADVISOR_ITEM_CHARS)?;
    }
    Ok(())
}

#[derive(Debug)]
struct CommandOutput {
    status: std::process::ExitStatus,
    stdout: CappedOutput,
    stderr: CappedOutput,
}

#[derive(Debug)]
struct CappedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

async fn read_capped_output<R>(mut reader: R, max_bytes: usize) -> std::io::Result<CappedOutput>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(bytes.len());
        if remaining > 0 {
            let keep = remaining.min(read);
            bytes.extend_from_slice(&buffer[..keep]);
            if keep < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    Ok(CappedOutput { bytes, truncated })
}

fn capped_utf8_lossy(output: &CappedOutput) -> String {
    let mut value = String::from_utf8_lossy(&output.bytes).into_owned();
    if output.truncated {
        value.push_str("<truncated>");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn command_advisor_reads_token_usage_sidecar() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let script_path = tempdir.path().join("advisor.sh");
        std::fs::write(
            &script_path,
            r#"printf '%s\n' '{"type":"token_count","usage":{"input_tokens":12,"output_tokens":8,"total_tokens":20}}' > "$TRADE_ENGINE_COMMAND_USAGE_EVENTS_PATH"
printf '%s\n' '{"summary":"ok","observations":[],"risks":[],"next_actions":[],"rationale":"ok","evidence":[],"proposals":[]}'
"#,
        )
        .expect("write advisor script");
        let provider = CommandAdvisorProvider::new(
            "sh",
            vec![script_path.display().to_string()],
            Duration::from_secs(5),
            4096,
            4096,
        );

        let response = provider
            .advise(&minimal_advisor_request())
            .await
            .expect("advisor response");

        let usage = response.token_usage.expect("token usage");
        assert_eq!(usage.input_tokens, Some(12));
        assert_eq!(usage.output_tokens, Some(8));
        assert_eq!(usage.total_tokens, Some(20));
    }

    fn minimal_advisor_request() -> AdvisorRequest {
        let symbol = Symbol::new("AAPL").expect("symbol");
        AdvisorRequest::dry_run_market_tick(
            BrokerEnvironment::Paper,
            "1234",
            "open",
            1,
            1,
            [symbol],
            AdvisorMarketTickObservation {
                stage: "test".to_owned(),
                market_session_open: true,
                technical_analysis: Vec::new(),
                domestic_market_alignment: None,
                kis_api: AdvisorKisApiSummary {
                    account: AdvisorAccountSummary {
                        cash: Decimal::ZERO,
                        position_count: 0,
                        positions: Vec::new(),
                    },
                    open_order_count: 0,
                    open_orders: Vec::new(),
                    today_fill_count: 0,
                    today_fills: Vec::new(),
                    quote_count: 0,
                    quotes: Vec::new(),
                },
            },
        )
    }
}
