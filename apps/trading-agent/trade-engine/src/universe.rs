use crate::command_transport::{CommandError, capped_utf8_lossy, run_command};
use std::fmt::{self, Display};
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::Command;

use crate::command_usage::{
    COMMAND_USAGE_EVENTS_PATH_ENV, CommandUsageCapture, ProviderTokenUsage,
};
use crate::decision::DecisionOutputStatus;
use crate::gateway::types::{MarketScope, Symbol};

pub const UNIVERSE_DISCOVERY_REQUEST_SCHEMA_VERSION: u32 = 1;
pub const MAX_UNIVERSE_DISCOVERY_SUMMARY_CHARS: usize = 2_000;
pub const MAX_UNIVERSE_DISCOVERY_CANDIDATES: usize = 50;
pub const MAX_UNIVERSE_DISCOVERY_REASON_CHARS: usize = 1_000;
pub const MAX_UNIVERSE_DISCOVERY_SIGNALS: usize = 8;
pub const MAX_UNIVERSE_DISCOVERY_EVIDENCE_ITEMS: usize = 8;
pub const MAX_UNIVERSE_DISCOVERY_RISK_FLAGS: usize = 8;
pub const MAX_UNIVERSE_DISCOVERY_EVIDENCE_TEXT_CHARS: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UniverseDiscoveryRequest {
    pub schema_version: u32,
    pub runtime: UniverseDiscoveryRuntimeContext,
    pub user_watchlist: Vec<Symbol>,
}

impl UniverseDiscoveryRequest {
    pub fn market_tick(
        runtime: UniverseDiscoveryRuntimeContext,
        user_watchlist: impl IntoIterator<Item = Symbol>,
    ) -> Self {
        Self {
            schema_version: UNIVERSE_DISCOVERY_REQUEST_SCHEMA_VERSION,
            runtime,
            user_watchlist: user_watchlist.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UniverseDiscoveryRuntimeContext {
    pub mode: String,
    pub environment: String,
    pub account_suffix: String,
    pub market: UniverseDiscoveryMarketContext,
    pub market_hours: String,
    pub tick_index: u64,
    pub market_tick_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UniverseDiscoveryMarketContext {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

impl UniverseDiscoveryMarketContext {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UniverseDiscoveryOutput {
    pub summary: String,
    pub candidates: Vec<UniverseDiscoveryCandidate>,
}

impl UniverseDiscoveryOutput {
    pub fn validate(
        mut self,
    ) -> Result<ValidatedUniverseDiscoveryOutput, UniverseDiscoveryValidationError> {
        validate_text(
            "summary",
            &self.summary,
            MAX_UNIVERSE_DISCOVERY_SUMMARY_CHARS,
        )?;
        if self.candidates.len() > MAX_UNIVERSE_DISCOVERY_CANDIDATES {
            return Err(UniverseDiscoveryValidationError::TooManyItems {
                field: "candidates",
                max_items: MAX_UNIVERSE_DISCOVERY_CANDIDATES,
            });
        }
        for candidate in &mut self.candidates {
            validate_text(
                "candidates.reason",
                &candidate.reason,
                MAX_UNIVERSE_DISCOVERY_REASON_CHARS,
            )?;
            if candidate.signals.len() > MAX_UNIVERSE_DISCOVERY_SIGNALS {
                return Err(UniverseDiscoveryValidationError::TooManyItems {
                    field: "candidates.signals",
                    max_items: MAX_UNIVERSE_DISCOVERY_SIGNALS,
                });
            }
            if candidate.evidence.len() > MAX_UNIVERSE_DISCOVERY_EVIDENCE_ITEMS {
                return Err(UniverseDiscoveryValidationError::TooManyItems {
                    field: "candidates.evidence",
                    max_items: MAX_UNIVERSE_DISCOVERY_EVIDENCE_ITEMS,
                });
            }
            if candidate.risk_flags.len() > MAX_UNIVERSE_DISCOVERY_RISK_FLAGS {
                return Err(UniverseDiscoveryValidationError::TooManyItems {
                    field: "candidates.risk_flags",
                    max_items: MAX_UNIVERSE_DISCOVERY_RISK_FLAGS,
                });
            }
            for evidence in &candidate.evidence {
                validate_text(
                    "candidates.evidence.metric",
                    &evidence.metric,
                    MAX_UNIVERSE_DISCOVERY_EVIDENCE_TEXT_CHARS,
                )?;
                validate_text(
                    "candidates.evidence.value",
                    &evidence.value,
                    MAX_UNIVERSE_DISCOVERY_EVIDENCE_TEXT_CHARS,
                )?;
                let as_of = evidence.as_of.as_deref().ok_or(
                    UniverseDiscoveryValidationError::MissingEvidenceField {
                        field: "candidates.evidence.as_of",
                    },
                )?;
                validate_text(
                    "candidates.evidence.as_of",
                    as_of,
                    MAX_UNIVERSE_DISCOVERY_EVIDENCE_TEXT_CHARS,
                )?;
                let source = evidence.source.as_deref().ok_or(
                    UniverseDiscoveryValidationError::MissingEvidenceField {
                        field: "candidates.evidence.source",
                    },
                )?;
                validate_text(
                    "candidates.evidence.source",
                    source,
                    MAX_UNIVERSE_DISCOVERY_EVIDENCE_TEXT_CHARS,
                )?;
            }
            if let Some(target_support) = &candidate.target_support
                && let Some(source_count) = target_support.source_count
                && source_count == 0
            {
                return Err(
                    UniverseDiscoveryValidationError::InvalidTargetSupportField {
                        field: "candidates.target_support.source_count",
                        reason: "must be greater than zero".to_owned(),
                    },
                );
            }
            if candidate.confidence < Decimal::ZERO || candidate.confidence > Decimal::from(1_u32) {
                return Err(UniverseDiscoveryValidationError::InvalidConfidence {
                    symbol: candidate.symbol.as_str().to_owned(),
                    confidence: candidate.confidence,
                });
            }
            candidate.symbol =
                Symbol::new(candidate.symbol.as_str().to_owned()).map_err(|source| {
                    UniverseDiscoveryValidationError::InvalidSymbol {
                        symbol: candidate.symbol.as_str().to_owned(),
                        reason: source.to_string(),
                    }
                })?;
        }
        Ok(ValidatedUniverseDiscoveryOutput { output: self })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UniverseDiscoveryCandidate {
    pub symbol: Symbol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<UniverseDiscoveryBucket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_support: Option<UniverseDiscoveryTargetSupport>,
    #[serde(default)]
    pub signals: Vec<UniverseDiscoverySignal>,
    #[serde(default)]
    pub evidence: Vec<UniverseDiscoveryEvidence>,
    #[serde(default)]
    pub risk_flags: Vec<UniverseDiscoveryRiskFlag>,
    pub reason: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub confidence: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UniverseDiscoveryTargetSupport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consensus: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<UniverseDiscoveryTargetFreshness>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UniverseDiscoveryTargetFreshness {
    Fresh,
    Recent,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UniverseDiscoveryBucket {
    Liquidity,
    Valuation,
    ShortTermMomentum,
    Catalyst,
    WatchlistPeer,
    DefensiveRotation,
    EtfOrProxy,
    TargetPriceGap,
}

impl UniverseDiscoveryBucket {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Liquidity => "liquidity",
            Self::Valuation => "valuation",
            Self::ShortTermMomentum => "short_term_momentum",
            Self::Catalyst => "catalyst",
            Self::WatchlistPeer => "watchlist_peer",
            Self::DefensiveRotation => "defensive_rotation",
            Self::EtfOrProxy => "etf_or_proxy",
            Self::TargetPriceGap => "target_price_gap",
        }
    }
}

impl Display for UniverseDiscoveryBucket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UniverseDiscoverySignal {
    VolumeLeader,
    TradingValueLeader,
    RelativeLowPer,
    RelativeLowPbr,
    RoeSupport,
    VolumeBreakout,
    PriceBreakout,
    RelativeStrength,
    EarningsCatalyst,
    NewsCatalyst,
    PolicyCatalyst,
    WatchlistPeer,
    DefensiveRotation,
    MacroSensitive,
    EtfProxy,
    HoldingCompanyProxy,
    AnalystTargetUpside,
    ConsensusTargetUpside,
    TargetPriceRevision,
}

impl UniverseDiscoverySignal {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VolumeLeader => "volume_leader",
            Self::TradingValueLeader => "trading_value_leader",
            Self::RelativeLowPer => "relative_low_per",
            Self::RelativeLowPbr => "relative_low_pbr",
            Self::RoeSupport => "roe_support",
            Self::VolumeBreakout => "volume_breakout",
            Self::PriceBreakout => "price_breakout",
            Self::RelativeStrength => "relative_strength",
            Self::EarningsCatalyst => "earnings_catalyst",
            Self::NewsCatalyst => "news_catalyst",
            Self::PolicyCatalyst => "policy_catalyst",
            Self::WatchlistPeer => "watchlist_peer",
            Self::DefensiveRotation => "defensive_rotation",
            Self::MacroSensitive => "macro_sensitive",
            Self::EtfProxy => "etf_proxy",
            Self::HoldingCompanyProxy => "holding_company_proxy",
            Self::AnalystTargetUpside => "analyst_target_upside",
            Self::ConsensusTargetUpside => "consensus_target_upside",
            Self::TargetPriceRevision => "target_price_revision",
        }
    }
}

impl Display for UniverseDiscoverySignal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UniverseDiscoveryEvidence {
    pub metric: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UniverseDiscoveryRiskFlag {
    StaleData,
    UnverifiedSource,
    NewsUnverified,
    ThinLiquidity,
    HighVolatility,
    EventRisk,
    CrowdedTrade,
    GapRisk,
    DataUnavailable,
    StaleTargetPrice,
    SingleBrokerTarget,
}

impl UniverseDiscoveryRiskFlag {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StaleData => "stale_data",
            Self::UnverifiedSource => "unverified_source",
            Self::NewsUnverified => "news_unverified",
            Self::ThinLiquidity => "thin_liquidity",
            Self::HighVolatility => "high_volatility",
            Self::EventRisk => "event_risk",
            Self::CrowdedTrade => "crowded_trade",
            Self::GapRisk => "gap_risk",
            Self::DataUnavailable => "data_unavailable",
            Self::StaleTargetPrice => "stale_target_price",
            Self::SingleBrokerTarget => "single_broker_target",
        }
    }
}

impl Display for UniverseDiscoveryRiskFlag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidatedUniverseDiscoveryOutput {
    output: UniverseDiscoveryOutput,
}

impl ValidatedUniverseDiscoveryOutput {
    pub fn output(&self) -> &UniverseDiscoveryOutput {
        &self.output
    }

    pub fn into_output(self) -> UniverseDiscoveryOutput {
        self.output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniverseDiscoveryAudit {
    pub request: UniverseDiscoveryRequest,
    pub provider: String,
    pub raw_output_json: String,
    pub token_usage: Option<ProviderTokenUsage>,
    pub status: DecisionOutputStatus,
    pub output: Option<ValidatedUniverseDiscoveryOutput>,
    pub error: Option<String>,
}

impl UniverseDiscoveryAudit {
    pub fn completed(
        request: UniverseDiscoveryRequest,
        provider: impl Into<String>,
        raw_output_json: String,
        output: ValidatedUniverseDiscoveryOutput,
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
        request: UniverseDiscoveryRequest,
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
pub struct UniverseDiscoveryProviderResponse {
    pub raw_output_json: String,
    pub token_usage: Option<ProviderTokenUsage>,
    pub output: ValidatedUniverseDiscoveryOutput,
}

#[async_trait]
pub trait UniverseDiscoveryProvider: Send + Sync {
    fn id(&self) -> &str;

    async fn discover(
        &self,
        request: &UniverseDiscoveryRequest,
    ) -> Result<UniverseDiscoveryProviderResponse, UniverseDiscoveryProviderError>;
}

#[derive(Debug, Clone)]
pub struct CommandUniverseDiscoveryProvider {
    command: PathBuf,
    args: Vec<String>,
    timeout: Duration,
    max_stderr_bytes: usize,
    max_stdout_bytes: usize,
}

impl CommandUniverseDiscoveryProvider {
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
impl UniverseDiscoveryProvider for CommandUniverseDiscoveryProvider {
    fn id(&self) -> &str {
        "command"
    }

    async fn discover(
        &self,
        request: &UniverseDiscoveryRequest,
    ) -> Result<UniverseDiscoveryProviderResponse, UniverseDiscoveryProviderError> {
        let request_json = serde_json::to_vec(request).map_err(|error| {
            UniverseDiscoveryProviderError::Internal {
                message: format!("failed to encode universe discovery request: {error}"),
            }
        })?;
        let usage_capture = CommandUsageCapture::new("universe");

        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .env(COMMAND_USAGE_EVENTS_PATH_ENV, usage_capture.path());
        let command_output = run_command(
            &mut command,
            &request_json,
            self.timeout,
            self.max_stdout_bytes,
            self.max_stderr_bytes,
        )
        .await
        .map_err(|error| match error {
            CommandError::Timeout => UniverseDiscoveryProviderError::Timeout {
                timeout: self.timeout,
            },
            CommandError::Io { operation, source } => UniverseDiscoveryProviderError::Io {
                operation,
                message: source.to_string(),
                raw_output_json: String::new(),
            },
        })?;
        let raw_output_json = capped_utf8_lossy(&command_output.stdout);
        let token_usage = usage_capture.read_token_usage();
        if !command_output.status.success() {
            return Err(UniverseDiscoveryProviderError::CommandFailed {
                status: command_output.status.to_string(),
                stderr: capped_utf8_lossy(&command_output.stderr),
                raw_output_json,
                token_usage,
            });
        }
        if command_output.stdout.truncated {
            return Err(UniverseDiscoveryProviderError::InvalidOutput {
                raw_output_json,
                reason: format!(
                    "stdout exceeded max_stdout_bytes ({})",
                    self.max_stdout_bytes
                ),
                token_usage,
            });
        }

        let parsed =
            serde_json::from_str::<UniverseDiscoveryOutput>(&raw_output_json).map_err(|error| {
                UniverseDiscoveryProviderError::InvalidOutput {
                    raw_output_json: raw_output_json.clone(),
                    reason: format!("failed to parse universe discovery provider output: {error}"),
                    token_usage: token_usage.clone(),
                }
            })?;
        let output =
            parsed
                .validate()
                .map_err(|error| UniverseDiscoveryProviderError::InvalidOutput {
                    raw_output_json: raw_output_json.clone(),
                    reason: error.to_string(),
                    token_usage: token_usage.clone(),
                })?;
        Ok(UniverseDiscoveryProviderResponse {
            raw_output_json,
            token_usage,
            output,
        })
    }
}

#[derive(Debug, Error)]
pub enum UniverseDiscoveryProviderError {
    #[error("universe discovery provider internal error: {message}")]
    Internal { message: String },
    #[error("universe discovery provider command I/O failed during {operation}: {message}")]
    Io {
        operation: &'static str,
        message: String,
        raw_output_json: String,
    },
    #[error("universe discovery provider timed out after {timeout:?}")]
    Timeout { timeout: Duration },
    #[error("universe discovery provider command exited with status {status}: {stderr}")]
    CommandFailed {
        status: String,
        stderr: String,
        raw_output_json: String,
        token_usage: Option<ProviderTokenUsage>,
    },
    #[error("universe discovery provider output was invalid: {reason}")]
    InvalidOutput {
        raw_output_json: String,
        reason: String,
        token_usage: Option<ProviderTokenUsage>,
    },
}

impl UniverseDiscoveryProviderError {
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
pub enum UniverseDiscoveryValidationError {
    #[error("universe discovery {field} must be {max_chars} characters or fewer")]
    TextTooLong {
        field: &'static str,
        max_chars: usize,
    },
    #[error("universe discovery {field} must contain {max_items} items or fewer")]
    TooManyItems {
        field: &'static str,
        max_items: usize,
    },
    #[error("universe discovery candidate {symbol} has invalid confidence {confidence}")]
    InvalidConfidence { symbol: String, confidence: Decimal },
    #[error("universe discovery candidate symbol {symbol} is invalid: {reason}")]
    InvalidSymbol { symbol: String, reason: String },
    #[error("universe discovery {field} is required when an evidence item is present")]
    MissingEvidenceField { field: &'static str },
    #[error("universe discovery {field} is invalid: {reason}")]
    InvalidTargetSupportField { field: &'static str, reason: String },
}

fn validate_text(
    field: &'static str,
    value: &str,
    max_chars: usize,
) -> Result<(), UniverseDiscoveryValidationError> {
    if value.chars().count() > max_chars {
        return Err(UniverseDiscoveryValidationError::TextTooLong { field, max_chars });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn command_universe_reads_token_usage_sidecar() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let script_path = tempdir.path().join("universe.sh");
        std::fs::write(
            &script_path,
            r#"printf '%s\n' '{"type":"token_count","usage":{"input_tokens":30,"output_tokens":11,"total_tokens":41}}' > "$TRADE_ENGINE_COMMAND_USAGE_EVENTS_PATH"
printf '%s\n' '{"summary":"ok","candidates":[]}'
"#,
        )
        .expect("write universe script");
        let provider = CommandUniverseDiscoveryProvider::new(
            "sh",
            vec![script_path.display().to_string()],
            Duration::from_secs(5),
            4096,
            4096,
        );

        let response = provider
            .discover(&minimal_universe_request())
            .await
            .expect("universe response");

        let usage = response.token_usage.expect("token usage");
        assert_eq!(usage.input_tokens, Some(30));
        assert_eq!(usage.output_tokens, Some(11));
        assert_eq!(usage.total_tokens, Some(41));
    }

    fn minimal_universe_request() -> UniverseDiscoveryRequest {
        UniverseDiscoveryRequest::market_tick(
            UniverseDiscoveryRuntimeContext {
                mode: "test".to_owned(),
                environment: "paper".to_owned(),
                account_suffix: "1234".to_owned(),
                market: UniverseDiscoveryMarketContext {
                    kind: "domestic_stock".to_owned(),
                    exchange_code: None,
                    currency: None,
                },
                market_hours: "open".to_owned(),
                tick_index: 1,
                market_tick_count: 1,
            },
            [Symbol::new("005930").expect("symbol")],
        )
    }
}
