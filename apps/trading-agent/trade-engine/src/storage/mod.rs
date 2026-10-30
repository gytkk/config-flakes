use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use rust_decimal::Decimal;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use trading_agent_kis_client::journal::BrokerCommandType;
use trading_agent_kis_client::{
    AccountKey, AmendResult, BrokerEnvironment, BrokerOrderId, BrokerOrderOrgNo, CancelResult,
    ClientOrderId, ExchangeCode, OrderReference, OrderSide, OrderSubmission, Quantity,
    ReconciliationResult, Symbol,
};

use crate::advisor::AdvisorAudit;
use crate::command_usage::ProviderTokenUsage;
use crate::decision::{DecisionOrderType, DecisionOutputStatus, DecisionRequest, DecisionType};
use crate::error::TradeEngineError;
use crate::execution::{
    ExecutionEventType, ExecutionRequest, ExecutionRequestId, ExecutionRequestStatus,
};
use crate::gateway::types::{MarketScope, TradingDay};
use crate::risk::{RiskBlockReason, RiskEvaluationStatus};
use crate::runtime::{
    DryRunDecisionAudit, DryRunDecisionFailureAudit, DryRunRunContext, DryRunTickFailure,
    DryRunTickOutput, DryRunTickStatus, RuntimeMode, RuntimeRunStatus,
};
use crate::strategy::StrategySignal;
use crate::technical_analysis::{
    DailyOhlcvBar, DailyTechnicalFeatures, OhlcvBarInterval, PriceAdjustmentPolicy,
    TechnicalFeatureParams, compute_latest_daily_features,
};
use crate::universe::UniverseDiscoveryAudit;
use crate::universe::UniverseDiscoveryOutput;

mod sqlite_journal;

pub use sqlite_journal::SqliteBrokerCommandJournal;

const MIGRATIONS: &[(i64, &str, &str)] = &[
    (
        1,
        "runtime_audit",
        include_str!("../../migrations/0001_runtime_audit.sql"),
    ),
    (
        2,
        "broker_command_journal",
        include_str!("../../migrations/0002_broker_command_journal.sql"),
    ),
    (
        3,
        "decision_pipeline",
        include_str!("../../migrations/0003_decision_pipeline.sql"),
    ),
    (
        4,
        "system_of_record_hardening",
        include_str!("../../migrations/0004_system_of_record_hardening.sql"),
    ),
    (
        5,
        "execution_storage_integrity",
        include_str!("../../migrations/0005_execution_storage_integrity.sql"),
    ),
    (
        6,
        "broker_command_safety_halt_event",
        include_str!("../../migrations/0006_broker_command_safety_halt_event.sql"),
    ),
    (
        7,
        "execution_reconciliation_results",
        include_str!("../../migrations/0007_execution_reconciliation_results.sql"),
    ),
    (
        8,
        "panic_stop_state",
        include_str!("../../migrations/0008_panic_stop_state.sql"),
    ),
    (
        9,
        "follow_up_execution_events",
        include_str!("../../migrations/0009_follow_up_execution_events.sql"),
    ),
    (
        10,
        "market_snapshot_full_name",
        include_str!("../../migrations/0010_market_snapshot_full_name.sql"),
    ),
    (
        11,
        "universe_snapshots",
        include_str!("../../migrations/0011_universe_snapshots.sql"),
    ),
    (
        12,
        "technical_analysis_storage",
        include_str!("../../migrations/0012_technical_analysis_storage.sql"),
    ),
];

const LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_VERSION: i64 = 8;
const LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_NAME: &str = "panic_stop_certification_records";
const LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_CHECKSUM: &str = "998da44da0b04c8b";

#[async_trait]
pub trait DryRunAuditStore: Send + Sync {
    async fn start_dry_run_run(
        &self,
        context: &DryRunRunContext,
    ) -> Result<StoredRuntimeRunId, TradeEngineError>;
    async fn has_unresolved_broker_commands(&self) -> Result<bool, TradeEngineError>;
    async fn panic_stop_active(&self) -> Result<bool, TradeEngineError>;
    async fn has_active_execution_intent(
        &self,
        symbol: &Symbol,
        side: OrderSide,
        order_type: DecisionOrderType,
        limit_price: Option<Decimal>,
    ) -> Result<bool, TradeEngineError>;
    async fn execution_request_count_since(
        &self,
        since: OffsetDateTime,
    ) -> Result<u32, TradeEngineError>;
    async fn latest_position_average_price_before(
        &self,
        symbol: &Symbol,
        observed_at: OffsetDateTime,
    ) -> Result<Option<Decimal>, TradeEngineError>;
    async fn recent_market_prices(
        &self,
        symbol: &crate::gateway::types::Symbol,
        limit: usize,
    ) -> Result<Vec<Decimal>, TradeEngineError>;
    async fn latest_technical_features_for_symbols(
        &self,
        symbols: &[Symbol],
        interval: OhlcvBarInterval,
        feature_set: String,
    ) -> Result<Vec<StoredTechnicalFeatureAudit>, TradeEngineError>;
    async fn latest_universe_snapshot(
        &self,
        environment: BrokerEnvironment,
        account_suffix: String,
    ) -> Result<Option<StoredUniverseSnapshot>, TradeEngineError>;
    async fn start_dry_run_tick(
        &self,
        run_id: StoredRuntimeRunId,
        tick_sequence: u64,
        lease_duration: Duration,
    ) -> Result<StoredRuntimeTickId, TradeEngineError>;
    async fn has_active_dry_run_tick_lease(&self) -> Result<bool, TradeEngineError>;
    async fn record_dry_run_tick_skip(
        &self,
        run_id: StoredRuntimeRunId,
        tick_sequence: u64,
        status: DryRunTickStatus,
        reason: &str,
    ) -> Result<(), TradeEngineError>;
    async fn record_tick_advisory(
        &self,
        run_id: StoredRuntimeRunId,
        market_tick_sequence: u64,
        audit: &AdvisorAudit,
    ) -> Result<(), TradeEngineError>;
    async fn record_universe_discovery(
        &self,
        run_id: StoredRuntimeRunId,
        market_tick_sequence: u64,
        audit: &UniverseDiscoveryAudit,
    ) -> Result<(), TradeEngineError>;
    async fn record_universe_snapshot(
        &self,
        environment: BrokerEnvironment,
        account_suffix: String,
        provider: String,
        output: UniverseDiscoveryOutput,
        token_usage: Option<ProviderTokenUsage>,
        ttl: Duration,
    ) -> Result<StoredUniverseSnapshotId, TradeEngineError>;
    async fn complete_dry_run_tick(
        &self,
        run_id: StoredRuntimeRunId,
        tick_id: StoredRuntimeTickId,
        output: &DryRunTickOutput,
    ) -> Result<(), TradeEngineError>;
    async fn fail_dry_run_tick(
        &self,
        tick_id: StoredRuntimeTickId,
        failure: &DryRunTickFailure,
    ) -> Result<(), TradeEngineError>;
    async fn finish_dry_run_run(
        &self,
        run_id: StoredRuntimeRunId,
        status: RuntimeRunStatus,
        error: Option<&str>,
        shutdown_reason: Option<&str>,
    ) -> Result<(), TradeEngineError>;
}

#[derive(Clone)]
pub struct BlockingMainDb {
    inner: Arc<Mutex<MainDb>>,
}

impl BlockingMainDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TradeEngineError> {
        Ok(Self {
            inner: Arc::new(Mutex::new(MainDb::open(path)?)),
        })
    }

    pub async fn feedback_status_summary(&self) -> Result<FeedbackStatusSummary, TradeEngineError> {
        self.with_db(move |db| db.feedback_status_summary()).await
    }

    pub async fn feedback_review_context(
        &self,
        since_hours: u64,
        limit: usize,
    ) -> Result<FeedbackReviewContext, TradeEngineError> {
        self.with_db(move |db| db.feedback_review_context(since_hours, limit))
            .await
    }

    pub async fn record_feedback_query_completed(
        &self,
        audit: CompletedFeedbackQueryAudit,
    ) -> Result<(), TradeEngineError> {
        self.with_db(move |db| db.record_feedback_query_completed(&audit))
            .await
    }

    pub async fn record_feedback_query_failed(
        &self,
        audit: FailedFeedbackQueryAudit,
    ) -> Result<(), TradeEngineError> {
        self.with_db(move |db| db.record_feedback_query_failed(&audit))
            .await
    }

    async fn with_db<T>(
        &self,
        operation: impl FnOnce(&mut MainDb) -> Result<T, TradeEngineError> + Send + 'static,
    ) -> Result<T, TradeEngineError>
    where
        T: Send + 'static,
    {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let mut guard = inner.lock().map_err(|error| {
                TradeEngineError::storage(format!("main DB lock was poisoned: {error}"))
            })?;
            operation(&mut guard)
        })
        .await
        .map_err(|error| TradeEngineError::runtime(format!("main DB task failed: {error}")))?
    }
}

#[async_trait]
impl DryRunAuditStore for BlockingMainDb {
    async fn start_dry_run_run(
        &self,
        context: &DryRunRunContext,
    ) -> Result<StoredRuntimeRunId, TradeEngineError> {
        let context = context.clone();
        self.with_db(move |db| db.start_dry_run_run(&context)).await
    }

    async fn has_unresolved_broker_commands(&self) -> Result<bool, TradeEngineError> {
        self.with_db(move |db| db.has_unresolved_broker_commands())
            .await
    }

    async fn panic_stop_active(&self) -> Result<bool, TradeEngineError> {
        self.with_db(move |db| db.panic_stop_active()).await
    }

    async fn has_active_execution_intent(
        &self,
        symbol: &Symbol,
        side: OrderSide,
        order_type: DecisionOrderType,
        limit_price: Option<Decimal>,
    ) -> Result<bool, TradeEngineError> {
        let symbol = symbol.clone();
        self.with_db(move |db| {
            db.has_active_execution_intent(&symbol, side, order_type, limit_price)
        })
        .await
    }

    async fn execution_request_count_since(
        &self,
        since: OffsetDateTime,
    ) -> Result<u32, TradeEngineError> {
        self.with_db(move |db| db.execution_request_count_since(since))
            .await
    }

    async fn latest_position_average_price_before(
        &self,
        symbol: &Symbol,
        observed_at: OffsetDateTime,
    ) -> Result<Option<Decimal>, TradeEngineError> {
        let symbol = symbol.clone();
        self.with_db(move |db| db.latest_position_average_price_before(&symbol, observed_at))
            .await
    }

    async fn recent_market_prices(
        &self,
        symbol: &crate::gateway::types::Symbol,
        limit: usize,
    ) -> Result<Vec<Decimal>, TradeEngineError> {
        let symbol = symbol.clone();
        self.with_db(move |db| db.recent_market_prices(&symbol, limit))
            .await
    }

    async fn latest_technical_features_for_symbols(
        &self,
        symbols: &[Symbol],
        interval: OhlcvBarInterval,
        feature_set: String,
    ) -> Result<Vec<StoredTechnicalFeatureAudit>, TradeEngineError> {
        let symbols = symbols.to_vec();
        self.with_db(move |db| {
            db.latest_technical_features_for_symbols(&symbols, interval, &feature_set)
        })
        .await
    }

    async fn latest_universe_snapshot(
        &self,
        environment: BrokerEnvironment,
        account_suffix: String,
    ) -> Result<Option<StoredUniverseSnapshot>, TradeEngineError> {
        self.with_db(move |db| db.latest_universe_snapshot(environment, &account_suffix))
            .await
    }

    async fn start_dry_run_tick(
        &self,
        run_id: StoredRuntimeRunId,
        tick_sequence: u64,
        lease_duration: Duration,
    ) -> Result<StoredRuntimeTickId, TradeEngineError> {
        self.with_db(move |db| db.start_dry_run_tick(run_id, tick_sequence, lease_duration))
            .await
    }

    async fn has_active_dry_run_tick_lease(&self) -> Result<bool, TradeEngineError> {
        self.with_db(move |db| db.has_active_dry_run_tick_lease())
            .await
    }

    async fn record_dry_run_tick_skip(
        &self,
        run_id: StoredRuntimeRunId,
        tick_sequence: u64,
        status: DryRunTickStatus,
        reason: &str,
    ) -> Result<(), TradeEngineError> {
        let reason = reason.to_owned();
        self.with_db(move |db| db.record_dry_run_tick_skip(run_id, tick_sequence, status, &reason))
            .await
    }

    async fn record_tick_advisory(
        &self,
        run_id: StoredRuntimeRunId,
        market_tick_sequence: u64,
        audit: &AdvisorAudit,
    ) -> Result<(), TradeEngineError> {
        let audit = audit.clone();
        self.with_db(move |db| db.record_tick_advisory(run_id, market_tick_sequence, &audit))
            .await
    }

    async fn record_universe_discovery(
        &self,
        run_id: StoredRuntimeRunId,
        market_tick_sequence: u64,
        audit: &UniverseDiscoveryAudit,
    ) -> Result<(), TradeEngineError> {
        let audit = audit.clone();
        self.with_db(move |db| db.record_universe_discovery(run_id, market_tick_sequence, &audit))
            .await
    }

    async fn record_universe_snapshot(
        &self,
        environment: BrokerEnvironment,
        account_suffix: String,
        provider: String,
        output: UniverseDiscoveryOutput,
        token_usage: Option<ProviderTokenUsage>,
        ttl: Duration,
    ) -> Result<StoredUniverseSnapshotId, TradeEngineError> {
        self.with_db(move |db| {
            db.record_universe_snapshot(
                environment,
                &account_suffix,
                &provider,
                &output,
                token_usage.as_ref(),
                ttl,
            )
        })
        .await
    }

    async fn complete_dry_run_tick(
        &self,
        run_id: StoredRuntimeRunId,
        tick_id: StoredRuntimeTickId,
        output: &DryRunTickOutput,
    ) -> Result<(), TradeEngineError> {
        let output = output.clone();
        self.with_db(move |db| db.complete_dry_run_tick(run_id, tick_id, &output))
            .await
    }

    async fn fail_dry_run_tick(
        &self,
        tick_id: StoredRuntimeTickId,
        failure: &DryRunTickFailure,
    ) -> Result<(), TradeEngineError> {
        let failure = failure.clone();
        self.with_db(move |db| db.fail_dry_run_tick(tick_id, &failure))
            .await
    }

    async fn finish_dry_run_run(
        &self,
        run_id: StoredRuntimeRunId,
        status: RuntimeRunStatus,
        error: Option<&str>,
        shutdown_reason: Option<&str>,
    ) -> Result<(), TradeEngineError> {
        let error = error.map(str::to_owned);
        let shutdown_reason = shutdown_reason.map(str::to_owned);
        self.with_db(move |db| {
            db.finish_dry_run_run(run_id, status, error.as_deref(), shutdown_reason.as_deref())
        })
        .await
    }
}

pub struct MainDb {
    connection: Connection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuditTable {
    RuntimeRuns,
    RuntimeTicks,
    PortfolioSnapshots,
    MarketSnapshots,
    StrategySignals,
    DecisionInputs,
    DecisionOutputs,
    DecisionProposals,
    RiskEvaluations,
    RiskBlocks,
    ExecutionRequests,
    ExecutionEvents,
    BrokerCommandEvents,
    Fills,
}

impl AuditTable {
    fn count_sql(self) -> &'static str {
        match self {
            Self::RuntimeRuns => "SELECT COUNT(*) FROM runtime_runs",
            Self::RuntimeTicks => "SELECT COUNT(*) FROM runtime_ticks",
            Self::PortfolioSnapshots => "SELECT COUNT(*) FROM portfolio_snapshots",
            Self::MarketSnapshots => "SELECT COUNT(*) FROM market_snapshots",
            Self::StrategySignals => "SELECT COUNT(*) FROM strategy_signals",
            Self::DecisionInputs => "SELECT COUNT(*) FROM decision_inputs",
            Self::DecisionOutputs => "SELECT COUNT(*) FROM decision_outputs",
            Self::DecisionProposals => "SELECT COUNT(*) FROM decision_proposals",
            Self::RiskEvaluations => "SELECT COUNT(*) FROM risk_evaluations",
            Self::RiskBlocks => "SELECT COUNT(*) FROM risk_blocks",
            Self::ExecutionRequests => "SELECT COUNT(*) FROM execution_requests",
            Self::ExecutionEvents => "SELECT COUNT(*) FROM execution_events",
            Self::BrokerCommandEvents => "SELECT COUNT(*) FROM broker_command_events",
            Self::Fills => "SELECT COUNT(*) FROM fills",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUniverseSnapshot {
    pub id: StoredUniverseSnapshotId,
    pub observed_at: String,
    pub expires_at: String,
    pub environment: BrokerEnvironment,
    pub account_suffix: String,
    pub provider: String,
    pub summary: String,
    pub symbols: Vec<crate::gateway::types::Symbol>,
    pub output: UniverseDiscoveryOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTokenUsageSource {
    Advisor,
    UniverseDiscovery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTokenUsageSummary {
    pub source: CommandTokenUsageSource,
    pub recorded_invocations: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
    pub total_tokens: u64,
}

impl CommandTokenUsageSummary {
    fn empty(source: CommandTokenUsageSource) -> Self {
        Self {
            source,
            recorded_invocations: 0,
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens: 0,
        }
    }

    fn add_usage(&mut self, usage: ProviderTokenUsage) {
        self.recorded_invocations += 1;
        self.input_tokens += usage.input_tokens.unwrap_or(0);
        self.cached_input_tokens += usage.cached_input_tokens.unwrap_or(0);
        self.output_tokens += usage.output_tokens.unwrap_or(0);
        self.reasoning_output_tokens += usage.reasoning_output_tokens.unwrap_or(0);
        self.total_tokens += usage
            .total_tokens
            .unwrap_or_else(|| usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredUniverseSnapshotId(i64);

impl StoredUniverseSnapshotId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredRuntimeRunId(i64);

impl StoredRuntimeRunId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredRuntimeTickId(i64);

impl StoredRuntimeTickId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredOhlcvBarId(i64);

impl StoredOhlcvBarId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredTechnicalFeatureAuditId(i64);

impl StoredTechnicalFeatureAuditId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredExecutionRequestId(i64);

impl StoredExecutionRequestId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOhlcvBarRecord {
    pub observed_at: OffsetDateTime,
    pub source: String,
    pub environment: BrokerEnvironment,
    pub market_scope: MarketScope,
    pub interval: OhlcvBarInterval,
    pub bar_start_at: OffsetDateTime,
    pub bar_end_at: OffsetDateTime,
    pub adjustment_policy: PriceAdjustmentPolicy,
    pub data_quality_flags: Vec<String>,
    pub bar: DailyOhlcvBar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTechnicalFeatureAuditRecord {
    pub observed_at: OffsetDateTime,
    pub source: String,
    pub interval: OhlcvBarInterval,
    pub ohlcv_bar_id: Option<StoredOhlcvBarId>,
    pub feature_set: String,
    pub features: DailyTechnicalFeatures,
    pub input_bar_ids: Vec<StoredOhlcvBarId>,
    pub params: TechnicalFeatureParams,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTechnicalFeatureAuditComputation {
    pub observed_at: OffsetDateTime,
    pub source: String,
    pub symbol: Symbol,
    pub interval: OhlcvBarInterval,
    pub feature_set: String,
    pub params: TechnicalFeatureParams,
    pub lookback_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredTechnicalFeatureAudit {
    pub id: StoredTechnicalFeatureAuditId,
    pub observed_at: String,
    pub source: String,
    pub symbol: Symbol,
    pub interval: OhlcvBarInterval,
    pub ohlcv_bar_id: Option<StoredOhlcvBarId>,
    pub feature_set: String,
    pub features: DailyTechnicalFeatures,
    pub input_bar_ids: Vec<StoredOhlcvBarId>,
    pub params: TechnicalFeatureParams,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewExecutionRequestRecord {
    pub execution_request_id: ExecutionRequestId,
    pub run_id: i64,
    pub tick_id: i64,
    pub decision_proposal_id: i64,
    pub risk_evaluation_id: i64,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub quantity: Quantity,
    pub order_type: DecisionOrderType,
    pub limit_price: Option<Decimal>,
    pub order_notional: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredExecutionRequestForSubmit {
    pub id: StoredExecutionRequestId,
    pub execution_request_id: ExecutionRequestId,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub quantity: Quantity,
    pub order_type: DecisionOrderType,
    pub limit_price: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFollowUpOrderTarget {
    pub reference: OrderReference,
}

impl MainDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TradeEngineError> {
        Ok(Self {
            connection: open_migrated_connection(path)?,
        })
    }

    pub fn applied_migrations(&self) -> Result<Vec<i64>, TradeEngineError> {
        let mut statement = self
            .connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")
            .map_err(sqlite_error)?;
        let versions = statement
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        Ok(versions)
    }

    pub fn audit_counts(&self) -> Result<AuditCounts, TradeEngineError> {
        Ok(AuditCounts {
            runtime_runs: self.count_table(AuditTable::RuntimeRuns)?,
            runtime_ticks: self.count_table(AuditTable::RuntimeTicks)?,
            portfolio_snapshots: self.count_table(AuditTable::PortfolioSnapshots)?,
            market_snapshots: self.count_table(AuditTable::MarketSnapshots)?,
            strategy_signals: self.count_table(AuditTable::StrategySignals)?,
            decision_inputs: self.count_table(AuditTable::DecisionInputs)?,
            decision_outputs: self.count_table(AuditTable::DecisionOutputs)?,
            decision_proposals: self.count_table(AuditTable::DecisionProposals)?,
            risk_evaluations: self.count_table(AuditTable::RiskEvaluations)?,
            risk_blocks: self.count_table(AuditTable::RiskBlocks)?,
            execution_requests: self.count_table(AuditTable::ExecutionRequests)?,
            execution_events: self.count_table(AuditTable::ExecutionEvents)?,
            broker_command_events: self.count_table(AuditTable::BrokerCommandEvents)?,
            fills: self.count_table(AuditTable::Fills)?,
        })
    }

    pub fn command_token_usage_summaries(
        &self,
    ) -> Result<Vec<CommandTokenUsageSummary>, TradeEngineError> {
        let mut advisor = CommandTokenUsageSummary::empty(CommandTokenUsageSource::Advisor);
        let mut universe =
            CommandTokenUsageSummary::empty(CommandTokenUsageSource::UniverseDiscovery);
        let mut statement = self
            .connection
            .prepare(
                "SELECT action_type, metadata_json
                 FROM operator_actions
                 WHERE action_type IN (
                     'tick_advisory_completed',
                     'tick_advisory_failed',
                     'universe_discovery_completed',
                     'universe_discovery_failed',
                     'universe_snapshot_refreshed'
                 )
                 ORDER BY id",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(sqlite_error)?;

        for row in rows {
            let (action_type, metadata_json) = row.map_err(sqlite_error)?;
            let metadata =
                serde_json::from_str::<serde_json::Value>(&metadata_json).map_err(|error| {
                    TradeEngineError::storage(format!(
                        "invalid command usage metadata JSON for {action_type}: {error}"
                    ))
                })?;
            let Some(token_usage) = metadata.get("token_usage") else {
                continue;
            };
            if token_usage.is_null() {
                continue;
            }
            let usage = provider_token_usage_from_json(token_usage)?;
            match action_type.as_str() {
                "tick_advisory_completed" | "tick_advisory_failed" => advisor.add_usage(usage),
                "universe_discovery_completed"
                | "universe_discovery_failed"
                | "universe_snapshot_refreshed" => universe.add_usage(usage),
                _ => {}
            }
        }

        Ok(vec![advisor, universe]
            .into_iter()
            .filter(|summary| summary.recorded_invocations > 0)
            .collect())
    }

    pub fn feedback_status_summary(&self) -> Result<FeedbackStatusSummary, TradeEngineError> {
        Ok(FeedbackStatusSummary {
            applied_migrations: self.applied_migrations()?,
            audit_counts: self.audit_counts()?,
            unresolved_broker_commands: self.has_unresolved_broker_commands()?,
            panic_stop_active: self.panic_stop_active()?,
            command_token_usage: self.command_token_usage_summaries()?,
        })
    }

    pub fn feedback_review_context(
        &self,
        since_hours: u64,
        limit: usize,
    ) -> Result<FeedbackReviewContext, TradeEngineError> {
        let since_hours_i64 = i64::try_from(since_hours.min(876_000)).unwrap_or(876_000);
        let observed_after = (OffsetDateTime::now_utc() - time::Duration::hours(since_hours_i64))
            .format(&Rfc3339)
            .map_err(|error| {
                TradeEngineError::storage(format!("failed to format feedback window: {error}"))
            })?;
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
            let metadata =
                serde_json::from_str::<serde_json::Value>(&metadata_json).map_err(|error| {
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

    pub fn record_feedback_query_completed(
        &mut self,
        audit: &CompletedFeedbackQueryAudit,
    ) -> Result<(), TradeEngineError> {
        self.record_feedback_query_action(
            "feedback_query_completed",
            &audit.input,
            Some(&audit.raw_output_json),
            Some(&audit.parsed_output),
            None,
        )
    }

    pub fn record_feedback_query_failed(
        &mut self,
        audit: &FailedFeedbackQueryAudit,
    ) -> Result<(), TradeEngineError> {
        self.record_feedback_query_action(
            "feedback_query_failed",
            &audit.input,
            None,
            None,
            Some(&audit.error),
        )
    }

    fn record_feedback_query_action(
        &mut self,
        action_type: &str,
        input: &FeedbackQueryAuditInput,
        raw_output_json: Option<&str>,
        parsed_output: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let metadata_json = serde_json::json!({
            "schema_version": input.schema_version,
            "review_window": {
                "since_hours": input.since_hours,
                "limit": input.limit
            },
            "prompt": input.prompt,
            "context_summary": input.context_summary,
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

    pub fn set_panic_stop(
        &mut self,
        active: bool,
        operator_id: Option<&str>,
        reason: &str,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO panic_stop_state (
                    id,
                    active,
                    updated_at,
                    operator_id,
                    reason
                 )
                 VALUES (1, ?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                     active = excluded.active,
                     updated_at = excluded.updated_at,
                     operator_id = excluded.operator_id,
                     reason = excluded.reason",
                params![active, observed_at, operator_id, reason],
            )
            .map_err(sqlite_error)?;
        transaction
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
                VALUES (?1, ?2, ?3, 'panic_stop_state', '1', ?4, ?5)",
                params![
                    observed_at,
                    operator_id,
                    if active {
                        "panic_stop_activated"
                    } else {
                        "panic_stop_cleared"
                    },
                    reason,
                    serde_json::json!({ "active": active }).to_string(),
                ],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_startup_doctor_result(
        &mut self,
        environment: BrokerEnvironment,
        account_suffix: &str,
        passed: bool,
        reason: Option<&str>,
        metadata_json: &str,
    ) -> Result<(), TradeEngineError> {
        validate_json("startup doctor metadata", metadata_json)?;
        let observed_at = timestamp()?;
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
                VALUES (?1, NULL, ?2, 'startup_doctor', ?3, ?4, ?5)",
                params![
                    observed_at,
                    if passed {
                        "startup_doctor_passed"
                    } else {
                        "startup_doctor_failed"
                    },
                    format!("{}:{account_suffix}", environment.as_str()),
                    reason,
                    metadata_json,
                ],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }

    pub fn record_tick_advisory(
        &mut self,
        run_id: StoredRuntimeRunId,
        market_tick_sequence: u64,
        audit: &AdvisorAudit,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let request_json = serde_json::to_value(&audit.request).map_err(|error| {
            TradeEngineError::storage(format!("failed to encode advisor request: {error}"))
        })?;
        let output_json = audit
            .output
            .as_ref()
            .map(|output| serde_json::to_value(output.output()))
            .transpose()
            .map_err(|error| {
                TradeEngineError::storage(format!("failed to encode advisor output: {error}"))
            })?;
        let metadata_json = serde_json::json!({
            "market_tick_sequence": market_tick_sequence,
            "provider": audit.provider.as_str(),
            "status": audit.status.as_str(),
            "token_usage": audit.token_usage.as_ref(),
            "request": request_json,
            "raw_output_json": audit.raw_output_json.as_str(),
            "output": output_json,
            "error": audit.error.as_deref(),
        })
        .to_string();
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
                VALUES (?1, NULL, ?2, 'runtime_run', ?3, ?4, ?5)",
                params![
                    observed_at,
                    if audit.status == DecisionOutputStatus::Parsed {
                        "tick_advisory_completed"
                    } else {
                        "tick_advisory_failed"
                    },
                    run_id.as_i64().to_string(),
                    audit.error.as_deref(),
                    metadata_json,
                ],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }

    pub fn record_universe_discovery(
        &mut self,
        run_id: StoredRuntimeRunId,
        market_tick_sequence: u64,
        audit: &UniverseDiscoveryAudit,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let request_json = serde_json::to_value(&audit.request).map_err(|error| {
            TradeEngineError::storage(format!(
                "failed to encode universe discovery request: {error}"
            ))
        })?;
        let output_json = audit
            .output
            .as_ref()
            .map(|output| serde_json::to_value(output.output()))
            .transpose()
            .map_err(|error| {
                TradeEngineError::storage(format!(
                    "failed to encode universe discovery output: {error}"
                ))
            })?;
        let metadata_json = serde_json::json!({
            "market_tick_sequence": market_tick_sequence,
            "provider": audit.provider.as_str(),
            "status": audit.status.as_str(),
            "token_usage": audit.token_usage.as_ref(),
            "request": request_json,
            "raw_output_json": audit.raw_output_json.as_str(),
            "output": output_json,
            "error": audit.error.as_deref(),
        })
        .to_string();
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
                VALUES (?1, NULL, ?2, 'runtime_run', ?3, ?4, ?5)",
                params![
                    observed_at,
                    if audit.status == DecisionOutputStatus::Parsed {
                        "universe_discovery_completed"
                    } else {
                        "universe_discovery_failed"
                    },
                    run_id.as_i64().to_string(),
                    audit.error.as_deref(),
                    metadata_json,
                ],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }

    pub fn panic_stop_active(&self) -> Result<bool, TradeEngineError> {
        self.connection
            .query_row(
                "SELECT active FROM panic_stop_state WHERE id = 1",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error)
    }

    pub fn has_unresolved_broker_commands(&self) -> Result<bool, TradeEngineError> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS (
                    SELECT 1
                    FROM broker_command_journal
                    WHERE reconcile_required = 1
                       OR last_known_state IN ('submitting', 'submitted_ack_unknown', 'reconcile_failed')
                    LIMIT 1
                )",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(sqlite_error)?;
        Ok(exists != 0)
    }

    pub fn record_execution_request(
        &mut self,
        record: &NewExecutionRequestRecord,
    ) -> Result<StoredExecutionRequestId, TradeEngineError> {
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let risk = load_risk_evaluation_for_execution(&transaction, record.risk_evaluation_id)?;
        if risk.status != RiskEvaluationStatus::Approved {
            return Err(TradeEngineError::storage(
                "execution request requires an approved risk evaluation",
            ));
        }
        if risk.decision_proposal_id != record.decision_proposal_id {
            return Err(TradeEngineError::storage(
                "execution request decision proposal does not match risk evaluation",
            ));
        }
        if risk.run_id != record.run_id || risk.tick_id != record.tick_id {
            return Err(TradeEngineError::storage(
                "execution request run/tick does not match risk evaluation",
            ));
        }
        let proposal =
            load_decision_proposal_for_execution(&transaction, record.decision_proposal_id)?;
        validate_execution_request_matches_proposal(record, &proposal)?;
        if risk_evaluation_has_execution_request(&transaction, record.risk_evaluation_id)? {
            return Err(TradeEngineError::storage(
                "risk evaluation already has an execution request",
            ));
        }

        let created_at = timestamp()?;
        transaction
            .execute(
                "INSERT INTO execution_requests (
                    run_id,
                    tick_id,
                    decision_proposal_id,
                    created_at,
                    status,
                    symbol,
                    side,
                    quantity,
                    order_type,
                    limit_price,
                    order_notional,
                    execution_request_id,
                    risk_evaluation_id
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    record.run_id,
                    record.tick_id,
                    record.decision_proposal_id,
                    created_at,
                    ExecutionRequestStatus::Requested.as_str(),
                    record.symbol.as_str(),
                    order_side_as_str(record.side),
                    quantity_to_i64(record.quantity)?,
                    record.order_type.as_str(),
                    record.limit_price.map(|price| price.to_string()),
                    record.order_notional.map(|notional| notional.to_string()),
                    record.execution_request_id.as_str(),
                    record.risk_evaluation_id,
                ],
            )
            .map_err(sqlite_error)?;
        let id = StoredExecutionRequestId(transaction.last_insert_rowid());
        insert_execution_event(
            &transaction,
            id,
            ExecutionEventType::ExecutionRequested,
            None,
            serde_json::json!({
                "execution_request_id": record.execution_request_id.as_str()
            })
            .to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(id)
    }

    pub fn link_execution_request_to_journal(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: &ClientOrderId,
    ) -> Result<(), TradeEngineError> {
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let execution =
            load_execution_request_for_journal_link(&transaction, execution_request_id)?
                .ok_or_else(|| TradeEngineError::storage("execution request was not found"))?;
        if let Some(current_client_order_id) = &execution.client_order_id
            && current_client_order_id != client_order_id.as_str()
        {
            return Err(TradeEngineError::storage(
                "execution request is already linked to a different client order id",
            ));
        }
        let journal = load_journal_command_for_execution_link(&transaction, client_order_id)?
            .ok_or_else(|| {
                TradeEngineError::storage("broker command journal record was not found")
            })?;
        if let Some(current_execution_request_id) = journal.execution_request_id
            && current_execution_request_id != execution_request_id.as_i64()
        {
            return Err(TradeEngineError::storage(
                "broker command journal record is already linked",
            ));
        }
        validate_journal_matches_execution_request(&journal, &execution)?;

        let affected_execution_requests = transaction
            .execute(
                "UPDATE execution_requests
                 SET client_order_id = ?2,
                     status = ?3
                 WHERE id = ?1",
                params![
                    execution_request_id.as_i64(),
                    client_order_id.as_str(),
                    ExecutionRequestStatus::JournalReserved.as_str(),
                ],
            )
            .map_err(sqlite_error)?;
        if affected_execution_requests == 0 {
            return Err(TradeEngineError::storage("execution request was not found"));
        }
        let affected_journal_commands = transaction
            .execute(
                "UPDATE broker_command_journal
                 SET execution_request_id = ?2,
                     updated_at = ?3
                 WHERE client_order_id = ?1
                   AND (
                       execution_request_id IS NULL
                       OR execution_request_id = ?2
                   )",
                params![
                    client_order_id.as_str(),
                    execution_request_id.as_i64(),
                    timestamp()?,
                ],
            )
            .map_err(sqlite_error)?;
        if affected_journal_commands == 0 {
            return Err(TradeEngineError::storage(
                "broker command journal record is already linked",
            ));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            ExecutionEventType::BrokerCommandReserved,
            Some(client_order_id.as_str()),
            serde_json::json!({
                "client_order_id": client_order_id.as_str()
            })
            .to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn follow_up_order_target(
        &self,
        execution_request_id: StoredExecutionRequestId,
    ) -> Result<StoredFollowUpOrderTarget, TradeEngineError> {
        self.connection
            .query_row(
                "SELECT
                    er.client_order_id,
                    COALESCE(er.broker_order_id, bcj.broker_order_id),
                    COALESCE(er.broker_order_orgno, bcj.broker_order_orgno),
                    bcj.exchange_code,
                    bcj.account_key,
                    bcj.environment
                 FROM execution_requests er
                 JOIN broker_command_journal bcj
                   ON bcj.client_order_id = er.client_order_id
                 WHERE er.id = ?1
                   AND er.status = 'accepted'",
                [execution_request_id.as_i64()],
                |row| {
                    let client_order_id =
                        ClientOrderId::new(row.get::<_, String>(0)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    let broker_order_id =
                        parse_required_broker_order_id(row.get::<_, Option<String>>(1)?, 1)?;
                    let broker_order_orgno =
                        parse_required_broker_order_orgno(row.get::<_, Option<String>>(2)?, 2)?;
                    let exchange_code =
                        parse_optional_exchange_code(row.get::<_, Option<String>>(3)?, 3)?;
                    let account_key =
                        AccountKey::new(row.get::<_, String>(4)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    let environment =
                        BrokerEnvironment::parse(&row.get::<_, String>(5)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(StoredFollowUpOrderTarget {
                        reference: OrderReference {
                            client_order_id,
                            broker_order_id: Some(broker_order_id),
                            broker_order_orgno: Some(broker_order_orgno),
                            exchange_code,
                            account_key,
                            environment,
                        },
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| {
                TradeEngineError::storage(
                    "accepted execution request is not ready for follow-up write",
                )
            })
    }

    pub fn link_follow_up_command_to_execution(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: &ClientOrderId,
        command_type: BrokerCommandType,
    ) -> Result<(), TradeEngineError> {
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let parent_client_order_id = transaction
            .query_row(
                "SELECT client_order_id
                 FROM execution_requests
                 WHERE id = ?1",
                [execution_request_id.as_i64()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| TradeEngineError::storage("execution request was not found"))?
            .ok_or_else(|| {
                TradeEngineError::storage("execution request has no parent client order id")
            })?;
        let journal = transaction
            .query_row(
                "SELECT execution_request_id, parent_client_order_id, command_type
                 FROM broker_command_journal
                 WHERE client_order_id = ?1",
                [client_order_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| {
                TradeEngineError::storage("follow-up broker command journal record was not found")
            })?;
        if journal.1.as_deref() != Some(parent_client_order_id.as_str())
            || journal.2 != command_type.as_str()
        {
            return Err(TradeEngineError::storage(
                "follow-up broker command journal record does not match execution request",
            ));
        }
        if let Some(current_execution_request_id) = journal.0
            && current_execution_request_id != execution_request_id.as_i64()
        {
            return Err(TradeEngineError::storage(
                "follow-up broker command journal record is already linked",
            ));
        }
        let affected = transaction
            .execute(
                "UPDATE broker_command_journal
                 SET execution_request_id = ?2,
                     updated_at = ?3
                 WHERE client_order_id = ?1
                   AND (
                       execution_request_id IS NULL
                       OR execution_request_id = ?2
                   )",
                params![
                    client_order_id.as_str(),
                    execution_request_id.as_i64(),
                    timestamp()?,
                ],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage(
                "follow-up broker command journal record is already linked",
            ));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            ExecutionEventType::BrokerCommandReserved,
            Some(client_order_id.as_str()),
            serde_json::json!({
                "client_order_id": client_order_id.as_str(),
                "parent_client_order_id": parent_client_order_id,
                "command_type": command_type.as_str(),
            })
            .to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_execution_order_accepted(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        submission: &OrderSubmission,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let affected = transaction
            .execute(
                "UPDATE execution_requests
                 SET status = ?2,
                     broker_order_id = ?3,
                     broker_order_orgno = ?4,
                     submitted_at = COALESCE(submitted_at, ?5),
                     completed_at = ?5
                 WHERE id = ?1",
                params![
                    execution_request_id.as_i64(),
                    ExecutionRequestStatus::Accepted.as_str(),
                    submission
                        .reference
                        .broker_order_id
                        .as_ref()
                        .map(|value| value.as_str()),
                    submission
                        .reference
                        .broker_order_orgno
                        .as_ref()
                        .map(|value| value.as_str()),
                    observed_at,
                ],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage("execution request was not found"));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            ExecutionEventType::OrderAccepted,
            None,
            serde_json::json!({
                "client_order_id": submission.reference.client_order_id.as_str(),
                "broker_order_id": submission
                    .reference
                    .broker_order_id
                    .as_ref()
                    .map(|value| value.as_str()),
                "broker_order_orgno": submission
                    .reference
                    .broker_order_orgno
                    .as_ref()
                    .map(|value| value.as_str()),
            })
            .to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_execution_order_cancelled(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: &ClientOrderId,
        result: &CancelResult,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let affected = transaction
            .execute(
                "UPDATE execution_requests
                 SET status = ?2,
                     completed_at = ?3
                 WHERE id = ?1",
                params![
                    execution_request_id.as_i64(),
                    ExecutionRequestStatus::Cancelled.as_str(),
                    observed_at,
                ],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage("execution request was not found"));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            ExecutionEventType::OrderCancelled,
            Some(client_order_id.as_str()),
            serde_json::json!({
                "client_order_id": client_order_id.as_str(),
                "parent_client_order_id": result.reference.client_order_id.as_str(),
                "broker_order_id": result
                    .reference
                    .broker_order_id
                    .as_ref()
                    .map(|value| value.as_str()),
                "broker_order_orgno": result
                    .reference
                    .broker_order_orgno
                    .as_ref()
                    .map(|value| value.as_str()),
            })
            .to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_execution_order_amended(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: &ClientOrderId,
        result: &AmendResult,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let affected = transaction
            .execute(
                "UPDATE execution_requests
                 SET status = ?2,
                     last_error = NULL,
                     completed_at = ?3
                 WHERE id = ?1",
                params![
                    execution_request_id.as_i64(),
                    ExecutionRequestStatus::Accepted.as_str(),
                    observed_at,
                ],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage("execution request was not found"));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            ExecutionEventType::OrderAmended,
            Some(client_order_id.as_str()),
            serde_json::json!({
                "client_order_id": client_order_id.as_str(),
                "parent_client_order_id": result.reference.client_order_id.as_str(),
                "broker_order_id": result
                    .reference
                    .broker_order_id
                    .as_ref()
                    .map(|value| value.as_str()),
                "broker_order_orgno": result
                    .reference
                    .broker_order_orgno
                    .as_ref()
                    .map(|value| value.as_str()),
            })
            .to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_execution_order_rejected(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        reason: &str,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let affected = transaction
            .execute(
                "UPDATE execution_requests
                 SET status = ?2,
                     rejection_reason = ?3,
                     completed_at = ?4
                 WHERE id = ?1",
                params![
                    execution_request_id.as_i64(),
                    ExecutionRequestStatus::Rejected.as_str(),
                    reason,
                    observed_at,
                ],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage("execution request was not found"));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            ExecutionEventType::OrderRejected,
            None,
            serde_json::json!({ "reason": reason }).to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_execution_follow_up_rejected(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: &ClientOrderId,
        command_type: BrokerCommandType,
        reason: &str,
    ) -> Result<(), TradeEngineError> {
        self.record_execution_follow_up_non_terminal_event(
            execution_request_id,
            Some(client_order_id),
            ExecutionEventType::FollowUpRejected,
            serde_json::json!({
                "client_order_id": client_order_id.as_str(),
                "command_type": command_type.as_str(),
                "reason": reason,
            })
            .to_string(),
            Some(reason),
        )
    }

    pub fn record_execution_reconciliation_needed(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        stage: &str,
        message: &str,
    ) -> Result<(), TradeEngineError> {
        self.record_execution_reconciliation_needed_inner(
            execution_request_id,
            None,
            stage,
            message,
        )
    }

    pub fn record_execution_reconciliation_needed_for_command(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: &ClientOrderId,
        stage: &str,
        message: &str,
    ) -> Result<(), TradeEngineError> {
        self.record_execution_reconciliation_needed_inner(
            execution_request_id,
            Some(client_order_id),
            stage,
            message,
        )
    }

    fn record_execution_reconciliation_needed_inner(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: Option<&ClientOrderId>,
        stage: &str,
        message: &str,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let affected = transaction
            .execute(
                "UPDATE execution_requests
                 SET status = ?2,
                     last_error = ?3,
                     submitted_at = COALESCE(submitted_at, ?4)
                 WHERE id = ?1",
                params![
                    execution_request_id.as_i64(),
                    ExecutionRequestStatus::ReconciliationNeeded.as_str(),
                    message,
                    observed_at,
                ],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage("execution request was not found"));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            ExecutionEventType::ReconciliationNeeded,
            client_order_id.map(ClientOrderId::as_str),
            serde_json::json!({
                "client_order_id": client_order_id.map(ClientOrderId::as_str),
                "stage": stage,
                "message": message,
            })
            .to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_execution_follow_up_failed(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: Option<&ClientOrderId>,
        command_type: BrokerCommandType,
        reason: &str,
    ) -> Result<(), TradeEngineError> {
        self.record_execution_follow_up_non_terminal_event(
            execution_request_id,
            client_order_id,
            ExecutionEventType::ExecutionFailed,
            serde_json::json!({
                "client_order_id": client_order_id.map(ClientOrderId::as_str),
                "command_type": command_type.as_str(),
                "reason": reason,
            })
            .to_string(),
            Some(reason),
        )
    }

    fn record_execution_follow_up_non_terminal_event(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        client_order_id: Option<&ClientOrderId>,
        event_type: ExecutionEventType,
        metadata_json: String,
        last_error: Option<&str>,
    ) -> Result<(), TradeEngineError> {
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let affected = transaction
            .execute(
                "UPDATE execution_requests
                 SET last_error = COALESCE(?2, last_error)
                 WHERE id = ?1",
                params![execution_request_id.as_i64(), last_error],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage("execution request was not found"));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            event_type,
            client_order_id.map(ClientOrderId::as_str),
            metadata_json,
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_execution_failed(
        &mut self,
        execution_request_id: StoredExecutionRequestId,
        reason: &str,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let affected = transaction
            .execute(
                "UPDATE execution_requests
                 SET status = ?2,
                     last_error = ?3,
                     completed_at = ?4
                 WHERE id = ?1",
                params![
                    execution_request_id.as_i64(),
                    ExecutionRequestStatus::Failed.as_str(),
                    reason,
                    observed_at,
                ],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage("execution request was not found"));
        }
        insert_execution_event(
            &transaction,
            execution_request_id,
            ExecutionEventType::ExecutionFailed,
            None,
            serde_json::json!({ "reason": reason }).to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn record_execution_reconciliation_result(
        &mut self,
        client_order_id: &ClientOrderId,
        result: &ReconciliationResult,
    ) -> Result<Option<StoredExecutionRequestId>, TradeEngineError> {
        let observed_at = timestamp()?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        let execution_request_id = transaction
            .query_row(
                "SELECT COALESCE(
                    (
                        SELECT execution_request_id
                        FROM broker_command_journal
                        WHERE client_order_id = ?1
                    ),
                    (
                        SELECT id
                        FROM execution_requests
                        WHERE client_order_id = ?1
                    )
                 )",
                [client_order_id.as_str()],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some(execution_request_id) = execution_request_id.flatten() else {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(None);
        };
        let execution_request_id = StoredExecutionRequestId(execution_request_id);

        let (status, event_type, message, complete) = reconciliation_execution_update(result);
        transaction
            .execute(
                "UPDATE execution_requests
                 SET status = ?2,
                     broker_order_id = COALESCE(
                         broker_order_id,
                         (
                             SELECT broker_order_id
                             FROM broker_command_journal
                             WHERE client_order_id = ?3
                         )
                     ),
                     broker_order_orgno = COALESCE(
                         broker_order_orgno,
                         (
                             SELECT broker_order_orgno
                             FROM broker_command_journal
                             WHERE client_order_id = ?3
                         )
                     ),
                     submitted_at = COALESCE(submitted_at, ?4),
                     completed_at = CASE WHEN ?5 THEN ?4 ELSE completed_at END,
                     rejection_reason = CASE
                         WHEN ?2 = 'rejected' THEN COALESCE(rejection_reason, ?6)
                         ELSE rejection_reason
                     END,
                     last_error = CASE
                         WHEN ?2 = 'reconciliation_needed' THEN ?6
                         ELSE last_error
                     END
                 WHERE id = ?1",
                params![
                    execution_request_id.as_i64(),
                    status.as_str(),
                    client_order_id.as_str(),
                    observed_at,
                    complete,
                    message,
                ],
            )
            .map_err(sqlite_error)?;
        insert_execution_event(
            &transaction,
            execution_request_id,
            event_type,
            Some(client_order_id.as_str()),
            serde_json::json!({
                "client_order_id": client_order_id.as_str(),
                "reconciliation_result": reconciliation_result_as_str(result),
                "message": message,
            })
            .to_string(),
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(Some(execution_request_id))
    }

    pub fn next_requested_execution_request(
        &self,
    ) -> Result<Option<StoredExecutionRequestForSubmit>, TradeEngineError> {
        self.connection
            .query_row(
                "SELECT
                    id,
                    execution_request_id,
                    symbol,
                    side,
                    quantity,
                    order_type,
                    limit_price
                 FROM execution_requests
                 WHERE status = 'requested'
                 ORDER BY id
                 LIMIT 1",
                [],
                |row| {
                    Ok(StoredExecutionRequestForSubmit {
                        id: StoredExecutionRequestId(row.get(0)?),
                        execution_request_id: ExecutionRequestId::new(row.get::<_, String>(1)?)
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    1,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?,
                        symbol: Symbol::new(row.get::<_, String>(2)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        side: parse_order_side(row.get::<_, String>(3)?.as_str()).map_err(
                            |error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    3,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            },
                        )?,
                        quantity: i64_to_quantity(row.get(4)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Integer,
                                Box::new(error),
                            )
                        })?,
                        order_type: parse_decision_order_type(row.get::<_, String>(5)?.as_str())
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    5,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?,
                        limit_price: parse_optional_decimal(row.get(6)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                6,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn next_requested_execution_request_for_run(
        &self,
        run_id: StoredRuntimeRunId,
    ) -> Result<Option<StoredExecutionRequestForSubmit>, TradeEngineError> {
        self.connection
            .query_row(
                "SELECT
                    id,
                    execution_request_id,
                    symbol,
                    side,
                    quantity,
                    order_type,
                    limit_price
                 FROM execution_requests
                 WHERE status = 'requested'
                   AND run_id = ?1
                 ORDER BY id
                 LIMIT 1",
                [run_id.as_i64()],
                |row| {
                    Ok(StoredExecutionRequestForSubmit {
                        id: StoredExecutionRequestId(row.get(0)?),
                        execution_request_id: ExecutionRequestId::new(row.get::<_, String>(1)?)
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    1,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?,
                        symbol: Symbol::new(row.get::<_, String>(2)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                        side: parse_order_side(row.get::<_, String>(3)?.as_str()).map_err(
                            |error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    3,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            },
                        )?,
                        quantity: i64_to_quantity(row.get(4)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                rusqlite::types::Type::Integer,
                                Box::new(error),
                            )
                        })?,
                        order_type: parse_decision_order_type(row.get::<_, String>(5)?.as_str())
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    5,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })?,
                        limit_price: parse_optional_decimal(row.get(6)?).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                6,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?,
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn has_active_execution_intent(
        &self,
        symbol: &Symbol,
        side: OrderSide,
        order_type: DecisionOrderType,
        limit_price: Option<Decimal>,
    ) -> Result<bool, TradeEngineError> {
        let exists = self
            .connection
            .query_row(
                "SELECT EXISTS (
                    SELECT 1
                    FROM execution_requests
                    WHERE symbol = ?1
                      AND side = ?2
                      AND order_type = ?3
                      AND COALESCE(limit_price, '') = COALESCE(?4, '')
                      AND status IN (
                          'requested',
                          'journal_reserved',
                          'accepted',
                          'reconciliation_needed'
                      )
                    LIMIT 1
                )",
                params![
                    symbol.as_str(),
                    order_side_as_str(side),
                    order_type.as_str(),
                    limit_price.map(|price| price.to_string()),
                ],
                |row| row.get::<_, i64>(0),
            )
            .map_err(sqlite_error)?;
        Ok(exists != 0)
    }

    pub fn execution_request_count_since(
        &self,
        since: OffsetDateTime,
    ) -> Result<u32, TradeEngineError> {
        self.execution_request_count_since_excluding(since, None)
    }

    pub fn latest_position_average_price_before(
        &self,
        symbol: &Symbol,
        observed_at: OffsetDateTime,
    ) -> Result<Option<Decimal>, TradeEngineError> {
        let observed_at = format_timestamp(observed_at)?;
        let average_price = self
            .connection
            .query_row(
                "SELECT json_extract(position.value, '$.average_price')
                 FROM portfolio_snapshots AS snapshot,
                      json_each(snapshot.positions_json) AS position
                 WHERE snapshot.observed_at <= ?1
                   AND json_extract(position.value, '$.symbol') = ?2
                 ORDER BY snapshot.observed_at DESC, snapshot.id DESC
                 LIMIT 1",
                params![observed_at, symbol.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        average_price
            .map(|value| {
                parse_stored_decimal("portfolio_snapshots.positions_json.average_price", &value)
            })
            .transpose()
    }

    pub fn execution_request_count_since_excluding(
        &self,
        since: OffsetDateTime,
        excluded_id: Option<StoredExecutionRequestId>,
    ) -> Result<u32, TradeEngineError> {
        let since = since.format(&Rfc3339).map_err(|error| {
            TradeEngineError::storage(format!(
                "failed to format execution count boundary: {error}"
            ))
        })?;
        let count = self
            .connection
            .query_row(
                "SELECT COUNT(*)
                 FROM execution_requests
                 WHERE created_at >= ?1
                   AND (?2 IS NULL OR id != ?2)",
                params![since, excluded_id.map(StoredExecutionRequestId::as_i64)],
                |row| row.get::<_, i64>(0),
            )
            .map_err(sqlite_error)?;
        u32::try_from(count).map_err(|error| {
            TradeEngineError::storage(format!("execution count overflow: {error}"))
        })
    }

    pub fn record_standalone_dry_run_tick(
        &mut self,
        output: &DryRunTickOutput,
    ) -> Result<(), TradeEngineError> {
        let context = DryRunRunContext::live(output.snapshot.account_suffix.clone());
        let run_id = self.start_dry_run_run(&context)?;
        let tick_id = self.start_dry_run_tick(run_id, 0, Duration::from_secs(60))?;
        self.complete_dry_run_tick(run_id, tick_id, output)?;
        self.finish_dry_run_run(run_id, RuntimeRunStatus::Completed, None, None)
    }

    fn count_table(&self, table: AuditTable) -> Result<u64, TradeEngineError> {
        let sql = table.count_sql();
        let count = self
            .connection
            .query_row(sql, [], |row| row.get::<_, i64>(0))
            .map_err(sqlite_error)?;
        u64::try_from(count)
            .map_err(|error| TradeEngineError::storage(format!("invalid table count: {error}")))
    }
}

impl MainDb {
    pub fn start_dry_run_run(
        &mut self,
        context: &DryRunRunContext,
    ) -> Result<StoredRuntimeRunId, TradeEngineError> {
        let started_at = timestamp()?;
        self.connection
            .execute(
                "INSERT INTO runtime_runs (
                    mode,
                    environment,
                    account_suffix,
                    started_at,
                    completed_at,
                    status,
                    error,
                    shutdown_reason
                )
	                VALUES (?1, ?2, ?3, ?4, NULL, ?5, NULL, NULL)",
                params![
                    context.mode.as_str(),
                    context.environment.as_str(),
                    context.account_suffix.as_str(),
                    started_at,
                    RuntimeRunStatus::Started.as_str(),
                ],
            )
            .map_err(sqlite_error)?;
        Ok(StoredRuntimeRunId(self.connection.last_insert_rowid()))
    }

    pub fn record_ohlcv_bar(
        &mut self,
        record: &NewOhlcvBarRecord,
    ) -> Result<StoredOhlcvBarId, TradeEngineError> {
        let observed_at = format_timestamp(record.observed_at)?;
        let bar_start_at = format_timestamp(record.bar_start_at)?;
        let bar_end_at = format_timestamp(record.bar_end_at)?;
        let data_quality_json =
            serde_json::to_string(&record.data_quality_flags).map_err(|error| {
                TradeEngineError::storage(format!("failed to encode data quality flags: {error}"))
            })?;
        let (market_scope, exchange, currency) = market_scope_storage_columns(&record.market_scope);
        self.connection
            .execute(
                "INSERT INTO ohlcv_bars (
                    observed_at,
                    source,
                    environment,
                    symbol,
                    market_scope,
                    exchange,
                    currency,
                    interval,
                    trading_day,
                    bar_start_at,
                    bar_end_at,
                    adjustment_policy,
                    data_quality_json,
                    open,
                    high,
                    low,
                    close,
                    volume
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                params![
                    observed_at,
                    record.source.as_str(),
                    record.environment.as_str(),
                    record.bar.symbol.as_str(),
                    market_scope,
                    exchange,
                    currency,
                    record.interval.as_str(),
                    record.bar.trading_day.0.to_string(),
                    bar_start_at,
                    bar_end_at,
                    record.adjustment_policy.as_str(),
                    data_quality_json,
                    record.bar.open.to_string(),
                    record.bar.high.to_string(),
                    record.bar.low.to_string(),
                    record.bar.close.to_string(),
                    record.bar.volume.to_string(),
                ],
            )
            .map_err(sqlite_error)?;
        Ok(StoredOhlcvBarId(self.connection.last_insert_rowid()))
    }

    pub fn record_technical_feature_audit(
        &mut self,
        record: &NewTechnicalFeatureAuditRecord,
    ) -> Result<StoredTechnicalFeatureAuditId, TradeEngineError> {
        let observed_at = format_timestamp(record.observed_at)?;
        let features_json = serde_json::to_string(&record.features).map_err(|error| {
            TradeEngineError::storage(format!("failed to encode technical features: {error}"))
        })?;
        let input_bar_ids = record
            .input_bar_ids
            .iter()
            .map(|id| id.as_i64())
            .collect::<Vec<_>>();
        let inputs_json = serde_json::json!({
            "input_bar_ids": input_bar_ids,
            "params": &record.params,
        })
        .to_string();
        self.connection
            .execute(
                "INSERT INTO technical_feature_audits (
                    observed_at,
                    source,
                    symbol,
                    interval,
                    ohlcv_bar_id,
                    feature_set,
                    features_json,
                    inputs_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    observed_at,
                    record.source.as_str(),
                    record.features.symbol.as_str(),
                    record.interval.as_str(),
                    record.ohlcv_bar_id.map(StoredOhlcvBarId::as_i64),
                    record.feature_set.as_str(),
                    features_json,
                    inputs_json,
                ],
            )
            .map_err(sqlite_error)?;
        Ok(StoredTechnicalFeatureAuditId(
            self.connection.last_insert_rowid(),
        ))
    }

    pub fn recent_ohlcv_bars(
        &self,
        symbol: &Symbol,
        interval: OhlcvBarInterval,
        limit: usize,
    ) -> Result<Vec<DailyOhlcvBar>, TradeEngineError> {
        Ok(self
            .recent_ohlcv_bar_records(symbol, interval, limit)?
            .into_iter()
            .map(|record| record.bar)
            .collect())
    }

    pub fn ohlcv_bars_for_symbol_between(
        &self,
        symbol: &Symbol,
        market_scope: &MarketScope,
        interval: OhlcvBarInterval,
        from: TradingDay,
        to: TradingDay,
    ) -> Result<Vec<DailyOhlcvBar>, TradeEngineError> {
        if to.0 < from.0 {
            return Err(TradeEngineError::storage(
                "OHLCV range end must be greater than or equal to start",
            ));
        }
        let (market_scope_value, exchange, currency) = market_scope_storage_columns(market_scope);
        let mut statement = self
            .connection
            .prepare(
                "SELECT bars.id, bars.trading_day, bars.open, bars.high, bars.low, bars.close,
                        bars.volume
                 FROM ohlcv_bars AS bars
                 WHERE bars.symbol = ?1
                   AND bars.interval = ?2
                   AND bars.market_scope = ?3
                   AND ((bars.exchange IS NULL AND ?4 IS NULL) OR bars.exchange = ?4)
                   AND ((bars.currency IS NULL AND ?5 IS NULL) OR bars.currency = ?5)
                   AND bars.trading_day >= ?6
                   AND bars.trading_day <= ?7
                   AND bars.id = (
                        SELECT latest.id
                        FROM ohlcv_bars AS latest
                        WHERE latest.symbol = bars.symbol
                          AND latest.interval = bars.interval
                          AND latest.market_scope = bars.market_scope
                          AND ((latest.exchange IS NULL AND bars.exchange IS NULL)
                               OR latest.exchange = bars.exchange)
                          AND ((latest.currency IS NULL AND bars.currency IS NULL)
                               OR latest.currency = bars.currency)
                          AND latest.trading_day = bars.trading_day
                        ORDER BY latest.bar_end_at DESC, latest.id DESC
                        LIMIT 1
                   )
                 ORDER BY bars.trading_day ASC, bars.bar_end_at ASC, bars.id ASC",
            )
            .map_err(sqlite_error)?;
        statement
            .query_map(
                params![
                    symbol.as_str(),
                    interval.as_str(),
                    market_scope_value,
                    exchange,
                    currency,
                    from.0.to_string(),
                    to.0.to_string(),
                ],
                |row| {
                    Ok(StoredOhlcvBarRow {
                        id: row.get(0)?,
                        trading_day: row.get(1)?,
                        open: row.get(2)?,
                        high: row.get(3)?,
                        low: row.get(4)?,
                        close: row.get(5)?,
                        volume: row.get(6)?,
                    })
                },
            )
            .map_err(sqlite_error)?
            .map(|row| {
                row.map_err(sqlite_error)
                    .and_then(|row| row.into_record(symbol))
                    .map(|record| record.bar)
            })
            .collect()
    }

    pub fn compute_and_record_latest_daily_technical_features(
        &mut self,
        record: &NewTechnicalFeatureAuditComputation,
    ) -> Result<Option<StoredTechnicalFeatureAuditId>, TradeEngineError> {
        let bars =
            self.recent_ohlcv_bar_records(&record.symbol, record.interval, record.lookback_limit)?;
        if bars.is_empty() {
            return Ok(None);
        }
        let input_bar_ids = bars.iter().map(|stored| stored.id).collect::<Vec<_>>();
        let daily_bars = bars
            .iter()
            .map(|stored| stored.bar.clone())
            .collect::<Vec<_>>();
        let Some(features) =
            compute_latest_daily_features(&daily_bars, &record.params).map_err(|error| {
                TradeEngineError::storage(format!("failed to compute features: {error}"))
            })?
        else {
            return Ok(None);
        };
        let latest_bar_id = bars.last().map(|stored| stored.id);
        let audit_id = self.record_technical_feature_audit(&NewTechnicalFeatureAuditRecord {
            observed_at: record.observed_at,
            source: record.source.clone(),
            interval: record.interval,
            ohlcv_bar_id: latest_bar_id,
            feature_set: record.feature_set.clone(),
            features,
            input_bar_ids,
            params: record.params.clone(),
        })?;
        Ok(Some(audit_id))
    }

    pub fn latest_technical_features_for_symbols(
        &self,
        symbols: &[Symbol],
        interval: OhlcvBarInterval,
        feature_set: &str,
    ) -> Result<Vec<StoredTechnicalFeatureAudit>, TradeEngineError> {
        let mut audits = Vec::new();
        for symbol in symbols {
            if let Some(audit) =
                self.latest_technical_features_for_symbol(symbol, interval, feature_set)?
            {
                audits.push(audit);
            }
        }
        Ok(audits)
    }

    pub fn latest_technical_features_for_symbol(
        &self,
        symbol: &Symbol,
        interval: OhlcvBarInterval,
        feature_set: &str,
    ) -> Result<Option<StoredTechnicalFeatureAudit>, TradeEngineError> {
        self.connection
            .query_row(
                "SELECT id, observed_at, source, symbol, interval, ohlcv_bar_id,
                        feature_set, features_json, inputs_json
                 FROM technical_feature_audits
                 WHERE symbol = ?1
                   AND interval = ?2
                   AND feature_set = ?3
                 ORDER BY observed_at DESC, id DESC
                 LIMIT 1",
                params![symbol.as_str(), interval.as_str(), feature_set],
                |row| {
                    Ok(StoredTechnicalFeatureAuditRow {
                        id: row.get(0)?,
                        observed_at: row.get(1)?,
                        source: row.get(2)?,
                        symbol: row.get(3)?,
                        interval: row.get(4)?,
                        ohlcv_bar_id: row.get(5)?,
                        feature_set: row.get(6)?,
                        features_json: row.get(7)?,
                        inputs_json: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .map(StoredTechnicalFeatureAuditRow::into_audit)
            .transpose()
    }

    fn recent_ohlcv_bar_records(
        &self,
        symbol: &Symbol,
        interval: OhlcvBarInterval,
        limit: usize,
    ) -> Result<Vec<StoredOhlcvBarRecord>, TradeEngineError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(limit)
            .map_err(|error| TradeEngineError::storage(format!("OHLCV limit overflow: {error}")))?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, trading_day, open, high, low, close, volume
                 FROM ohlcv_bars
                 WHERE symbol = ?1 AND interval = ?2
                 ORDER BY bar_end_at DESC, id DESC
                 LIMIT ?3",
            )
            .map_err(sqlite_error)?;
        let mut bars = statement
            .query_map(params![symbol.as_str(), interval.as_str(), limit], |row| {
                Ok(StoredOhlcvBarRow {
                    id: row.get(0)?,
                    trading_day: row.get(1)?,
                    open: row.get(2)?,
                    high: row.get(3)?,
                    low: row.get(4)?,
                    close: row.get(5)?,
                    volume: row.get(6)?,
                })
            })
            .map_err(sqlite_error)?
            .map(|row| {
                row.map_err(sqlite_error)
                    .and_then(|row| row.into_record(symbol))
            })
            .collect::<Result<Vec<_>, _>>()?;
        bars.reverse();
        Ok(bars)
    }

    pub fn recent_market_prices(
        &self,
        symbol: &crate::gateway::types::Symbol,
        limit: usize,
    ) -> Result<Vec<Decimal>, TradeEngineError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let limit = i64::try_from(limit).map_err(|error| {
            TradeEngineError::storage(format!("recent market price limit overflow: {error}"))
        })?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT last_price
                 FROM market_snapshots
                 WHERE symbol = ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )
            .map_err(sqlite_error)?;
        let mut prices = statement
            .query_map(params![symbol.as_str(), limit], |row| {
                row.get::<_, String>(0)
            })
            .map_err(sqlite_error)?
            .map(|row| {
                row.map_err(sqlite_error).and_then(|value| {
                    Decimal::from_str(&value).map_err(|error| {
                        TradeEngineError::storage(format!(
                            "invalid market snapshot price for {symbol}: {error}"
                        ))
                    })
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        prices.reverse();
        Ok(prices)
    }

    pub fn latest_universe_snapshot(
        &self,
        environment: BrokerEnvironment,
        account_suffix: &str,
    ) -> Result<Option<StoredUniverseSnapshot>, TradeEngineError> {
        let now = timestamp()?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, observed_at, expires_at, environment, account_suffix, provider,
                        summary, symbols_json, output_json
                 FROM universe_snapshots
                 WHERE environment = ?1
                   AND account_suffix = ?2
                   AND expires_at > ?3
                 ORDER BY expires_at DESC, id DESC
                 LIMIT 1",
            )
            .map_err(sqlite_error)?;
        let row = statement
            .query_row(params![environment.as_str(), account_suffix, now], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .optional()
            .map_err(sqlite_error)?;
        let Some((
            id,
            observed_at,
            expires_at,
            environment,
            account_suffix,
            provider,
            summary,
            symbols_json,
            output_json,
        )) = row
        else {
            return Ok(None);
        };
        let environment = BrokerEnvironment::parse(&environment)
            .map_err(|error| TradeEngineError::storage(format!("invalid environment: {error}")))?;
        let symbol_values =
            serde_json::from_str::<Vec<String>>(&symbols_json).map_err(|error| {
                TradeEngineError::storage(format!(
                    "invalid universe snapshot symbols JSON: {error}"
                ))
            })?;
        let symbols = symbol_values
            .into_iter()
            .map(|symbol| {
                Symbol::new(symbol).map_err(|error| {
                    TradeEngineError::storage(format!("invalid universe snapshot symbol: {error}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let output =
            serde_json::from_str::<UniverseDiscoveryOutput>(&output_json).map_err(|error| {
                TradeEngineError::storage(format!("invalid universe snapshot output JSON: {error}"))
            })?;
        Ok(Some(StoredUniverseSnapshot {
            id: StoredUniverseSnapshotId(id),
            observed_at,
            expires_at,
            environment,
            account_suffix,
            provider,
            summary,
            symbols,
            output,
        }))
    }

    pub fn record_universe_snapshot(
        &mut self,
        environment: BrokerEnvironment,
        account_suffix: &str,
        provider: &str,
        output: &UniverseDiscoveryOutput,
        token_usage: Option<&ProviderTokenUsage>,
        ttl: Duration,
    ) -> Result<StoredUniverseSnapshotId, TradeEngineError> {
        let observed_at = timestamp()?;
        let expires_at = timestamp_after(ttl)?;
        let symbols = output
            .candidates
            .iter()
            .map(|candidate| candidate.symbol.as_str().to_owned())
            .collect::<Vec<_>>();
        let symbols_json = serde_json::to_string(&symbols).map_err(|error| {
            TradeEngineError::storage(format!(
                "failed to encode universe snapshot symbols: {error}"
            ))
        })?;
        let output_json = serde_json::to_string(output).map_err(|error| {
            TradeEngineError::storage(format!(
                "failed to encode universe snapshot output: {error}"
            ))
        })?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO universe_snapshots (
                    observed_at,
                    expires_at,
                    environment,
                    account_suffix,
                    provider,
                    summary,
                    symbols_json,
                    output_json
                )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    observed_at,
                    expires_at,
                    environment.as_str(),
                    account_suffix,
                    provider,
                    output.summary.as_str(),
                    symbols_json,
                    output_json,
                ],
            )
            .map_err(sqlite_error)?;
        let snapshot_id = StoredUniverseSnapshotId(transaction.last_insert_rowid());
        let metadata_json = serde_json::json!({
            "provider": provider,
            "status": "parsed",
            "environment": environment.as_str(),
            "account_suffix": account_suffix,
            "observed_at": observed_at,
            "expires_at": expires_at,
            "token_usage": token_usage,
            "symbols": symbols,
            "output": output,
        })
        .to_string();
        transaction
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
                VALUES (?1, NULL, 'universe_snapshot_refreshed', 'universe_snapshot', ?2, NULL, ?3)",
                params![observed_at, snapshot_id.as_i64().to_string(), metadata_json],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(snapshot_id)
    }

    pub fn start_dry_run_tick(
        &mut self,
        run_id: StoredRuntimeRunId,
        tick_sequence: u64,
        lease_duration: Duration,
    ) -> Result<StoredRuntimeTickId, TradeEngineError> {
        let mode = runtime_run_mode(&self.connection, run_id)?;
        let started_at = timestamp()?;
        let lease_expires_at = timestamp_after(lease_duration)?;
        self.connection
            .execute(
                "INSERT INTO runtime_ticks (
                    run_id,
                    mode,
                    started_at,
                    completed_at,
                    status,
                    error,
                    tick_sequence,
                    lease_owner,
                    lease_expires_at,
                    duration_ms,
                    status_detail_json
                )
	                VALUES (
	                    ?1,
	                    (SELECT mode FROM runtime_runs WHERE id = ?1),
	                    ?2,
	                    NULL,
	                    ?3,
	                    NULL,
	                    ?4,
	                    ?5,
	                    ?6,
	                    NULL,
	                    ?7
	                )",
                params![
                    run_id.as_i64(),
                    started_at,
                    DryRunTickStatus::Started.as_str(),
                    tick_sequence_to_i64(tick_sequence)?,
                    runtime_lease_owner(mode),
                    lease_expires_at,
                    serde_json::json!({ "status": DryRunTickStatus::Started.as_str() }).to_string(),
                ],
            )
            .map_err(sqlite_error)?;
        Ok(StoredRuntimeTickId(self.connection.last_insert_rowid()))
    }

    pub fn has_active_dry_run_tick_lease(&self) -> Result<bool, TradeEngineError> {
        let now = timestamp()?;
        let count = self
            .connection
            .query_row(
                "SELECT COUNT(*)
                 FROM runtime_ticks
	                 WHERE status = 'started'
	                   AND (lease_expires_at IS NULL OR lease_expires_at > ?1)",
                params![now],
                |row| row.get::<_, i64>(0),
            )
            .map_err(sqlite_error)?;
        Ok(count > 0)
    }

    pub fn record_dry_run_tick_skip(
        &mut self,
        run_id: StoredRuntimeRunId,
        tick_sequence: u64,
        status: DryRunTickStatus,
        reason: &str,
    ) -> Result<(), TradeEngineError> {
        let observed_at = timestamp()?;
        let status_detail_json = serde_json::json!({
            "status": status.as_str(),
            "reason": reason
        })
        .to_string();
        self.connection
            .execute(
                "INSERT INTO runtime_ticks (
                    run_id,
                    mode,
                    started_at,
                    completed_at,
                    status,
                    error,
                    tick_sequence,
                    duration_ms,
                    status_detail_json
                )
	                VALUES (
	                    ?1,
	                    (SELECT mode FROM runtime_runs WHERE id = ?1),
	                    ?2,
	                    ?2,
	                    ?3,
	                    NULL,
	                    ?4,
	                    0,
	                    ?5
	                )",
                params![
                    run_id.as_i64(),
                    observed_at,
                    status.as_str(),
                    tick_sequence_to_i64(tick_sequence)?,
                    status_detail_json,
                ],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }

    pub fn complete_dry_run_tick(
        &mut self,
        run_id: StoredRuntimeRunId,
        tick_id: StoredRuntimeTickId,
        output: &DryRunTickOutput,
    ) -> Result<(), TradeEngineError> {
        let completed_at = timestamp()?;
        let status_detail_json = Self::completed_dry_run_tick_detail_json(output)?;
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        update_tick_with_detail(
            &transaction,
            tick_id,
            output.terminal_status(),
            None,
            &completed_at,
            &status_detail_json,
        )?;
        insert_snapshots(
            &transaction,
            run_id,
            tick_id,
            &output.snapshot,
            &completed_at,
        )?;
        for decision in &output.decisions {
            insert_decision_audit(
                &transaction,
                run_id.as_i64(),
                tick_id.as_i64(),
                &completed_at,
                decision,
            )?;
        }
        for failure in &output.decision_failures {
            insert_decision_failure_audit(
                &transaction,
                run_id.as_i64(),
                tick_id.as_i64(),
                &completed_at,
                failure,
            )?;
        }

        transaction.commit().map_err(sqlite_error)
    }

    fn completed_dry_run_tick_detail_json(
        output: &DryRunTickOutput,
    ) -> Result<String, TradeEngineError> {
        let mut detail = serde_json::Map::new();
        detail.insert(
            "status".to_owned(),
            serde_json::Value::String(output.terminal_status().as_str().to_owned()),
        );
        if let Some(alignment) = &output.domestic_market_alignment {
            let alignment = serde_json::to_value(alignment).map_err(|error| {
                TradeEngineError::storage(format!(
                    "failed to encode domestic market alignment tick detail: {error}"
                ))
            })?;
            detail.insert("domestic_market_alignment".to_owned(), alignment);
        }
        serde_json::to_string(&detail).map_err(|error| {
            TradeEngineError::storage(format!("failed to encode dry-run tick detail: {error}"))
        })
    }

    pub fn fail_dry_run_tick(
        &mut self,
        tick_id: StoredRuntimeTickId,
        failure: &DryRunTickFailure,
    ) -> Result<(), TradeEngineError> {
        let completed_at = timestamp()?;
        let status_detail_json = serde_json::json!({
            "status": failure.status.as_str(),
            "stage": failure.stage.as_str(),
            "symbol": failure.symbol.as_ref().map(Symbol::as_str),
            "message": failure.message.as_str(),
            "retryable": failure.retryable
        })
        .to_string();
        let transaction = self.connection.transaction().map_err(sqlite_error)?;
        update_tick_with_detail(
            &transaction,
            tick_id,
            failure.status,
            Some(failure.message.as_str()),
            &completed_at,
            &status_detail_json,
        )?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn finish_dry_run_run(
        &mut self,
        run_id: StoredRuntimeRunId,
        status: RuntimeRunStatus,
        error: Option<&str>,
        shutdown_reason: Option<&str>,
    ) -> Result<(), TradeEngineError> {
        let affected = self
            .connection
            .execute(
                "UPDATE runtime_runs
                 SET completed_at = ?2,
                     status = ?3,
                     error = ?4,
                     shutdown_reason = ?5
                 WHERE id = ?1",
                params![
                    run_id.as_i64(),
                    timestamp()?,
                    status.as_str(),
                    error,
                    shutdown_reason,
                ],
            )
            .map_err(sqlite_error)?;
        if affected == 0 {
            return Err(TradeEngineError::storage("runtime run was not found"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuditCounts {
    pub runtime_runs: u64,
    pub runtime_ticks: u64,
    pub portfolio_snapshots: u64,
    pub market_snapshots: u64,
    pub strategy_signals: u64,
    pub decision_inputs: u64,
    pub decision_outputs: u64,
    pub decision_proposals: u64,
    pub risk_evaluations: u64,
    pub risk_blocks: u64,
    pub execution_requests: u64,
    pub execution_events: u64,
    pub broker_command_events: u64,
    pub fills: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackStatusSummary {
    pub applied_migrations: Vec<i64>,
    pub audit_counts: AuditCounts,
    pub unresolved_broker_commands: bool,
    pub panic_stop_active: bool,
    pub command_token_usage: Vec<CommandTokenUsageSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackReviewWindow {
    pub since_hours: u64,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackOperatorAction {
    pub id: i64,
    pub observed_at: String,
    pub action_type: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub reason: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackReviewContext {
    pub window: FeedbackReviewWindow,
    pub status: FeedbackStatusSummary,
    pub operator_actions: Vec<FeedbackOperatorAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackQueryAuditInput {
    pub schema_version: u32,
    pub since_hours: u64,
    pub limit: usize,
    pub prompt: String,
    pub context_summary: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletedFeedbackQueryAudit {
    pub input: FeedbackQueryAuditInput,
    pub raw_output_json: String,
    pub parsed_output: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FailedFeedbackQueryAudit {
    pub input: FeedbackQueryAuditInput,
    pub error: String,
}

struct StoredOhlcvBarRow {
    id: i64,
    trading_day: String,
    open: String,
    high: String,
    low: String,
    close: String,
    volume: String,
}

impl StoredOhlcvBarRow {
    fn into_record(self, symbol: &Symbol) -> Result<StoredOhlcvBarRecord, TradeEngineError> {
        Ok(StoredOhlcvBarRecord {
            id: StoredOhlcvBarId(self.id),
            bar: DailyOhlcvBar {
                symbol: symbol.clone(),
                trading_day: parse_trading_day(&self.trading_day)?,
                open: parse_stored_decimal("ohlcv_bars.open", &self.open)?,
                high: parse_stored_decimal("ohlcv_bars.high", &self.high)?,
                low: parse_stored_decimal("ohlcv_bars.low", &self.low)?,
                close: parse_stored_decimal("ohlcv_bars.close", &self.close)?,
                volume: parse_stored_decimal("ohlcv_bars.volume", &self.volume)?,
            },
        })
    }
}

struct StoredOhlcvBarRecord {
    id: StoredOhlcvBarId,
    bar: DailyOhlcvBar,
}

struct StoredTechnicalFeatureAuditRow {
    id: i64,
    observed_at: String,
    source: String,
    symbol: String,
    interval: String,
    ohlcv_bar_id: Option<i64>,
    feature_set: String,
    features_json: String,
    inputs_json: String,
}

#[derive(serde::Deserialize)]
struct StoredTechnicalFeatureInputs {
    input_bar_ids: Vec<i64>,
    params: TechnicalFeatureParams,
}

impl StoredTechnicalFeatureAuditRow {
    fn into_audit(self) -> Result<StoredTechnicalFeatureAudit, TradeEngineError> {
        let symbol = Symbol::new(self.symbol)
            .map_err(|error| TradeEngineError::invalid_broker_value("symbol", error))?;
        let interval = OhlcvBarInterval::parse(&self.interval).ok_or_else(|| {
            TradeEngineError::storage(format!(
                "invalid technical feature interval: {}",
                self.interval
            ))
        })?;
        let features = serde_json::from_str::<DailyTechnicalFeatures>(&self.features_json)
            .map_err(|error| {
                TradeEngineError::storage(format!("failed to decode technical features: {error}"))
            })?;
        let inputs = serde_json::from_str::<StoredTechnicalFeatureInputs>(&self.inputs_json)
            .map_err(|error| {
                TradeEngineError::storage(format!(
                    "failed to decode technical feature inputs: {error}"
                ))
            })?;
        Ok(StoredTechnicalFeatureAudit {
            id: StoredTechnicalFeatureAuditId(self.id),
            observed_at: self.observed_at,
            source: self.source,
            symbol,
            interval,
            ohlcv_bar_id: self.ohlcv_bar_id.map(StoredOhlcvBarId),
            feature_set: self.feature_set,
            features,
            input_bar_ids: inputs
                .input_bar_ids
                .into_iter()
                .map(StoredOhlcvBarId)
                .collect(),
            params: inputs.params,
        })
    }
}

struct StoredRiskEvaluationForExecution {
    run_id: i64,
    tick_id: i64,
    decision_proposal_id: i64,
    status: RiskEvaluationStatus,
}

struct StoredDecisionProposalForExecution {
    decision_type: String,
    symbol: String,
    quantity: Option<i64>,
    order_type: Option<String>,
    limit_price: Option<String>,
    status: String,
}

struct StoredExecutionRequestForJournalLink {
    client_order_id: Option<String>,
    symbol: String,
    side: String,
    quantity: i64,
    order_type: String,
    limit_price: Option<String>,
}

struct StoredJournalCommandForExecutionLink {
    execution_request_id: Option<i64>,
    command_type: String,
    symbol: String,
    side: String,
    quantity: i64,
    order_type: String,
    limit_price: Option<String>,
}

fn update_tick_with_detail(
    transaction: &Transaction<'_>,
    tick_id: StoredRuntimeTickId,
    status: DryRunTickStatus,
    error: Option<&str>,
    completed_at: &str,
    status_detail_json: &str,
) -> Result<(), TradeEngineError> {
    let affected = transaction
        .execute(
            "UPDATE runtime_ticks
             SET completed_at = ?2,
                 status = ?3,
                 error = ?4,
                 duration_ms = 0,
                 lease_owner = NULL,
                 lease_expires_at = NULL,
                 status_detail_json = ?5
             WHERE id = ?1 AND status = 'started'",
            params![
                tick_id.as_i64(),
                completed_at,
                status.as_str(),
                error,
                status_detail_json,
            ],
        )
        .map_err(sqlite_error)?;
    if affected == 0 {
        return Err(TradeEngineError::storage(
            "runtime tick was not found or was not started",
        ));
    }
    Ok(())
}

fn insert_snapshots(
    transaction: &Transaction<'_>,
    run_id: StoredRuntimeRunId,
    tick_id: StoredRuntimeTickId,
    output: &crate::runtime::DryRunSnapshot,
    completed_at: &str,
) -> Result<(), TradeEngineError> {
    let positions_json = serde_json::to_string(&output.account.positions).map_err(|error| {
        TradeEngineError::storage(format!("failed to encode positions snapshot: {error}"))
    })?;
    let open_orders_json = serde_json::to_string(&output.open_orders).map_err(|error| {
        TradeEngineError::storage(format!("failed to encode open orders snapshot: {error}"))
    })?;
    let today_fills_json = serde_json::to_string(&output.today_fills).map_err(|error| {
        TradeEngineError::storage(format!("failed to encode fills snapshot: {error}"))
    })?;
    transaction
        .execute(
            "INSERT INTO portfolio_snapshots (
                run_id,
                tick_id,
                observed_at,
                account_suffix,
                cash,
                positions_json,
                open_orders_json,
                today_fills_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_id.as_i64(),
                tick_id.as_i64(),
                completed_at,
                output.account_suffix,
                output.account.cash.cash.to_string(),
                positions_json,
                open_orders_json,
                today_fills_json,
            ],
        )
        .map_err(sqlite_error)?;

    for quote in output.all_quotes() {
        transaction
            .execute(
                "INSERT INTO market_snapshots (
                    run_id,
                    tick_id,
                    observed_at,
                    symbol,
                    full_name,
                    last_price
                )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    run_id.as_i64(),
                    tick_id.as_i64(),
                    completed_at,
                    quote.symbol.as_str(),
                    quote.full_name.as_deref(),
                    quote.last_price.to_string(),
                ],
            )
            .map_err(sqlite_error)?;
    }

    Ok(())
}

fn insert_decision_input_audit(
    transaction: &Transaction<'_>,
    run_id: i64,
    tick_id: i64,
    observed_at: &str,
    signal: &StrategySignal,
    decision_provider: &str,
    decision_request: &DecisionRequest,
) -> Result<i64, TradeEngineError> {
    transaction
        .execute(
            "INSERT INTO strategy_signals (run_id, tick_id, observed_at, strategy_id, symbol, action, strength)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run_id,
                tick_id,
                observed_at,
                signal.strategy_id.as_str(),
                signal.symbol.as_str(),
                signal.action.as_str(),
                signal.strength.to_string(),
            ],
        )
        .map_err(sqlite_error)?;
    let strategy_signal_id = transaction.last_insert_rowid();

    let input_summary_json = serde_json::json!({
        "strategy_id": signal.strategy_id.as_str(),
        "symbol": signal.symbol.as_str(),
        "signal_action": signal.action.as_str(),
        "signal_strength": signal.strength.to_string()
    })
    .to_string();
    let decision_request_json = serde_json::to_value(decision_request).map_err(|error| {
        TradeEngineError::storage(format!("failed to encode decision request: {error}"))
    })?;
    let context_json = serde_json::json!({
        "source": "dry_run",
        "provider": decision_provider,
        "decision_request": decision_request_json
    })
    .to_string();
    transaction
        .execute(
            "INSERT INTO decision_inputs (
                run_id,
                tick_id,
                strategy_signal_id,
                observed_at,
                provider,
                symbol,
                input_summary_json,
                context_json,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?4)",
            params![
                run_id,
                tick_id,
                strategy_signal_id,
                observed_at,
                decision_provider,
                signal.symbol.as_str(),
                input_summary_json,
                context_json,
            ],
        )
        .map_err(sqlite_error)?;
    Ok(transaction.last_insert_rowid())
}

fn insert_decision_audit(
    transaction: &Transaction<'_>,
    run_id: i64,
    tick_id: i64,
    observed_at: &str,
    decision: &DryRunDecisionAudit,
) -> Result<(), TradeEngineError> {
    let decision_input_id = insert_decision_input_audit(
        transaction,
        run_id,
        tick_id,
        observed_at,
        &decision.signal,
        decision.decision_provider.as_str(),
        &decision.decision_request,
    )?;

    transaction
        .execute(
            "INSERT INTO decision_outputs (
                run_id,
                tick_id,
                observed_at,
                provider,
                raw_output_json,
                status,
                error,
                decision_input_id
            )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
            params![
                run_id,
                tick_id,
                observed_at,
                decision.decision_provider.as_str(),
                decision.raw_decision_output_json.as_str(),
                DecisionOutputStatus::Parsed.as_str(),
                decision_input_id,
            ],
        )
        .map_err(sqlite_error)?;
    let decision_output_id = transaction.last_insert_rowid();

    let proposal = decision.proposal.proposal();
    let risk_notes_json = serde_json::to_string(&proposal.risk_notes).map_err(|error| {
        TradeEngineError::storage(format!("failed to encode risk notes: {error}"))
    })?;
    transaction
        .execute(
            "INSERT INTO decision_proposals (
                run_id,
                tick_id,
                decision_output_id,
                observed_at,
                decision_type,
                symbol,
                quantity,
                order_type,
                limit_price,
                confidence,
                time_horizon,
                reason,
                risk_notes_json,
                status,
                rejection_reason
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'valid', NULL)",
            params![
                run_id,
                tick_id,
                decision_output_id,
                observed_at,
                proposal.decision_type.as_str(),
                proposal.symbol.as_str(),
                proposal.quantity.map(|quantity| quantity.get()),
                proposal.order_type.map(DecisionOrderType::as_str),
                proposal.limit_price.map(|price| price.to_string()),
                proposal.confidence.to_string(),
                proposal.time_horizon.as_deref(),
                proposal.reason.as_str(),
                risk_notes_json,
            ],
        )
        .map_err(sqlite_error)?;
    let decision_proposal_id = transaction.last_insert_rowid();

    let risk_reasons = decision
        .risk
        .reasons
        .iter()
        .map(RiskBlockReason::code)
        .collect::<Vec<_>>();
    let reasons_json = serde_json::to_string(&risk_reasons).map_err(|error| {
        TradeEngineError::storage(format!("failed to encode risk reasons: {error}"))
    })?;
    transaction
        .execute(
            "INSERT INTO risk_evaluations (
                run_id,
                tick_id,
                decision_proposal_id,
                observed_at,
                status,
                order_notional,
                reasons_json,
                policy_snapshot_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_id,
                tick_id,
                decision_proposal_id,
                observed_at,
                decision.risk.status.as_str(),
                decision
                    .risk
                    .order_notional
                    .map(|notional| notional.to_string()),
                reasons_json,
                serde_json::to_string(&decision.risk_policy_snapshot).map_err(|error| {
                    TradeEngineError::storage(format!(
                        "failed to encode risk policy snapshot: {error}"
                    ))
                })?,
            ],
        )
        .map_err(sqlite_error)?;
    let risk_evaluation_id = transaction.last_insert_rowid();

    for reason in &decision.risk.reasons {
        let detail_json = risk_block_reason_detail_json(reason)?;
        transaction
            .execute(
                "INSERT INTO risk_blocks (risk_evaluation_id, reason, detail_json)
                 VALUES (?1, ?2, ?3)",
                params![risk_evaluation_id, reason.code(), detail_json,],
            )
            .map_err(sqlite_error)?;
    }

    insert_execution_request_for_approved_decision(
        transaction,
        run_id,
        tick_id,
        observed_at,
        decision_proposal_id,
        risk_evaluation_id,
        decision,
    )?;

    Ok(())
}

fn insert_execution_request_for_approved_decision(
    transaction: &Transaction<'_>,
    run_id: i64,
    tick_id: i64,
    observed_at: &str,
    decision_proposal_id: i64,
    risk_evaluation_id: i64,
    decision: &DryRunDecisionAudit,
) -> Result<(), TradeEngineError> {
    if !decision.risk.is_approved() || !decision.proposal.proposal().is_write() {
        return Ok(());
    }

    let execution_request_id = ExecutionRequestId::new(format!(
        "exec-{run_id}-{tick_id}-{risk_evaluation_id}"
    ))
    .map_err(|error| TradeEngineError::storage(format!("invalid execution request id: {error}")))?;
    let request = ExecutionRequest::new(
        execution_request_id,
        decision.proposal.clone(),
        &decision.risk,
    )
    .map_err(|error| TradeEngineError::storage(format!("invalid execution request: {error}")))?;
    let side = match request.proposal.decision_type {
        DecisionType::Buy => OrderSide::Buy,
        DecisionType::Sell => OrderSide::Sell,
        DecisionType::Hold => return Ok(()),
    };

    transaction
        .execute(
            "INSERT INTO execution_requests (
                run_id,
                tick_id,
                decision_proposal_id,
                created_at,
                status,
                symbol,
                side,
                quantity,
                order_type,
                limit_price,
                order_notional,
                execution_request_id,
                risk_evaluation_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                run_id,
                tick_id,
                decision_proposal_id,
                observed_at,
                ExecutionRequestStatus::Requested.as_str(),
                request.proposal.symbol.as_str(),
                order_side_as_str(side),
                quantity_to_i64(request.quantity)?,
                request.order_type.as_str(),
                request.limit_price.map(|price| price.to_string()),
                request.order_notional.map(|notional| notional.to_string()),
                request.id.as_str(),
                risk_evaluation_id,
            ],
        )
        .map_err(sqlite_error)?;
    let stored_id = StoredExecutionRequestId(transaction.last_insert_rowid());
    insert_execution_event(
        transaction,
        stored_id,
        ExecutionEventType::ExecutionRequested,
        None,
        serde_json::json!({
            "execution_request_id": request.id.as_str()
        })
        .to_string(),
    )
}

fn insert_decision_failure_audit(
    transaction: &Transaction<'_>,
    run_id: i64,
    tick_id: i64,
    observed_at: &str,
    failure: &DryRunDecisionFailureAudit,
) -> Result<(), TradeEngineError> {
    let decision_input_id = insert_decision_input_audit(
        transaction,
        run_id,
        tick_id,
        observed_at,
        &failure.signal,
        failure.decision_provider.as_str(),
        &failure.decision_request,
    )?;
    transaction
        .execute(
            "INSERT INTO decision_outputs (
                run_id,
                tick_id,
                observed_at,
                provider,
                raw_output_json,
                status,
                error,
                decision_input_id
            )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_id,
                tick_id,
                observed_at,
                failure.decision_provider.as_str(),
                failure.raw_decision_output_json.as_str(),
                failure.status.as_str(),
                failure.error.as_str(),
                decision_input_id,
            ],
        )
        .map(|_| ())
        .map_err(sqlite_error)
}

fn order_side_as_str(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

fn risk_block_reason_detail_json(reason: &RiskBlockReason) -> Result<String, TradeEngineError> {
    let value = match reason {
        RiskBlockReason::ReadOnlyMode => serde_json::json!({}),
        RiskBlockReason::LiveTradingDisabled => serde_json::json!({}),
        RiskBlockReason::MarketClosed => serde_json::json!({}),
        RiskBlockReason::UnresolvedJournalCommands => serde_json::json!({}),
        RiskBlockReason::PanicStopActive => serde_json::json!({}),
        RiskBlockReason::InsufficientBuyingPower {
            required,
            available,
        } => serde_json::json!({
            "required": required.to_string(),
            "available": available.to_string()
        }),
        RiskBlockReason::SellQuantityExceedsPosition {
            requested,
            available,
        } => serde_json::json!({
            "requested": requested.get(),
            "available": available.get()
        }),
        RiskBlockReason::DuplicateOrderIntent => serde_json::json!({}),
        RiskBlockReason::SymbolNotAllowed { symbol } => serde_json::json!({ "symbol": symbol }),
        RiskBlockReason::MarketOrderRequiresReferencePrice => serde_json::json!({}),
        RiskBlockReason::MaxOrderNotionalExceeded {
            notional,
            max_order_notional,
        } => serde_json::json!({
            "notional": notional.to_string(),
            "max_order_notional": max_order_notional.to_string()
        }),
        RiskBlockReason::MaxPositionNotionalExceeded {
            projected_notional,
            max_position_notional,
        } => serde_json::json!({
            "projected_notional": projected_notional.to_string(),
            "max_position_notional": max_position_notional.to_string()
        }),
        RiskBlockReason::MaxDailyOrderCountExceeded {
            submitted_orders,
            max_daily_orders,
        } => serde_json::json!({
            "submitted_orders": submitted_orders,
            "max_daily_orders": max_daily_orders
        }),
        RiskBlockReason::MaxDailyLossExceeded {
            realized_loss,
            max_daily_loss,
        } => serde_json::json!({
            "realized_loss": realized_loss.to_string(),
            "max_daily_loss": max_daily_loss.to_string()
        }),
        RiskBlockReason::DailyLossUnavailable { reason } => serde_json::json!({
            "reason": reason.as_str()
        }),
    };
    Ok(value.to_string())
}

fn reconciliation_execution_update(
    result: &ReconciliationResult,
) -> (
    ExecutionRequestStatus,
    ExecutionEventType,
    &'static str,
    bool,
) {
    match result {
        ReconciliationResult::ConfirmedSubmitted | ReconciliationResult::ConfirmedAmended => (
            ExecutionRequestStatus::Accepted,
            ExecutionEventType::ReconciliationSucceeded,
            "reconciliation confirmed submitted order",
            true,
        ),
        ReconciliationResult::ConfirmedFilled => (
            ExecutionRequestStatus::Filled,
            ExecutionEventType::OrderFilled,
            "reconciliation confirmed filled order",
            true,
        ),
        ReconciliationResult::ConfirmedRejected => (
            ExecutionRequestStatus::Rejected,
            ExecutionEventType::OrderRejected,
            "reconciliation confirmed rejected order",
            true,
        ),
        ReconciliationResult::ConfirmedCancelled => (
            ExecutionRequestStatus::Cancelled,
            ExecutionEventType::OrderCancelled,
            "reconciliation confirmed cancelled order",
            true,
        ),
        ReconciliationResult::StillUnknown => (
            ExecutionRequestStatus::ReconciliationNeeded,
            ExecutionEventType::ReconciliationStillUnknown,
            "reconciliation still has no unique broker match",
            false,
        ),
        ReconciliationResult::ManualInterventionRequired => (
            ExecutionRequestStatus::ReconciliationNeeded,
            ExecutionEventType::ManualInterventionRequired,
            "reconciliation found multiple broker matches",
            false,
        ),
    }
}

fn reconciliation_result_as_str(result: &ReconciliationResult) -> &'static str {
    match result {
        ReconciliationResult::ConfirmedSubmitted => "confirmed_submitted",
        ReconciliationResult::ConfirmedAmended => "confirmed_amended",
        ReconciliationResult::ConfirmedRejected => "confirmed_rejected",
        ReconciliationResult::ConfirmedCancelled => "confirmed_cancelled",
        ReconciliationResult::ConfirmedFilled => "confirmed_filled",
        ReconciliationResult::StillUnknown => "still_unknown",
        ReconciliationResult::ManualInterventionRequired => "manual_intervention_required",
    }
}

fn validate_json(label: &str, value: &str) -> Result<(), TradeEngineError> {
    serde_json::from_str::<serde_json::Value>(value)
        .map(|_| ())
        .map_err(|error| TradeEngineError::storage(format!("invalid {label} JSON: {error}")))
}

fn provider_token_usage_from_json(
    value: &serde_json::Value,
) -> Result<ProviderTokenUsage, TradeEngineError> {
    Ok(ProviderTokenUsage {
        input_tokens: optional_u64_field(value, "input_tokens")?,
        cached_input_tokens: optional_u64_field(value, "cached_input_tokens")?,
        output_tokens: optional_u64_field(value, "output_tokens")?,
        reasoning_output_tokens: optional_u64_field(value, "reasoning_output_tokens")?,
        total_tokens: optional_u64_field(value, "total_tokens")?,
    })
}

fn optional_u64_field(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<u64>, TradeEngineError> {
    match value.get(field) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => number.as_u64().map(Some).ok_or_else(|| {
            TradeEngineError::storage(format!("invalid token_usage.{field}: expected u64"))
        }),
        Some(_) => Err(TradeEngineError::storage(format!(
            "invalid token_usage.{field}: expected u64"
        ))),
    }
}

fn format_timestamp(time: OffsetDateTime) -> Result<String, TradeEngineError> {
    time.format(&Rfc3339)
        .map_err(|error| TradeEngineError::storage(format!("failed to format timestamp: {error}")))
}

fn timestamp() -> Result<String, TradeEngineError> {
    format_timestamp(OffsetDateTime::now_utc())
}

fn timestamp_after(duration: Duration) -> Result<String, TradeEngineError> {
    let duration = time::Duration::try_from(duration).map_err(|error| {
        TradeEngineError::storage(format!("tick lease duration overflow: {error}"))
    })?;
    (OffsetDateTime::now_utc() + duration)
        .format(&Rfc3339)
        .map_err(|error| TradeEngineError::storage(format!("failed to format timestamp: {error}")))
}

fn sqlite_error(error: rusqlite::Error) -> TradeEngineError {
    TradeEngineError::storage(error.to_string())
}

fn runtime_run_mode(
    connection: &Connection,
    run_id: StoredRuntimeRunId,
) -> Result<RuntimeMode, TradeEngineError> {
    let mode = connection
        .query_row(
            "SELECT mode FROM runtime_runs WHERE id = ?1",
            [run_id.as_i64()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| TradeEngineError::storage("runtime run was not found"))?;
    match mode.as_str() {
        "dry_run" => Ok(RuntimeMode::DryRun),
        "submit_orders" => Ok(RuntimeMode::SubmitOrders),
        other => Err(TradeEngineError::storage(format!(
            "invalid runtime run mode: {other}"
        ))),
    }
}

fn runtime_lease_owner(mode: RuntimeMode) -> &'static str {
    match mode {
        RuntimeMode::DryRun => "dry_run_runtime",
        RuntimeMode::SubmitOrders => "submit_orders_runtime",
    }
}

fn quantity_to_i64(quantity: Quantity) -> Result<i64, TradeEngineError> {
    i64::try_from(quantity.get())
        .map_err(|error| TradeEngineError::storage(format!("quantity overflow: {error}")))
}

fn i64_to_quantity(value: i64) -> Result<Quantity, TradeEngineError> {
    let value = u64::try_from(value).map_err(|error| {
        TradeEngineError::storage(format!("stored execution quantity is invalid: {error}"))
    })?;
    Ok(Quantity::new(value))
}

fn parse_order_side(value: &str) -> Result<OrderSide, TradeEngineError> {
    match value {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        other => Err(TradeEngineError::storage(format!(
            "invalid execution side: {other}"
        ))),
    }
}

fn parse_decision_order_type(value: &str) -> Result<DecisionOrderType, TradeEngineError> {
    match value {
        "market" => Ok(DecisionOrderType::Market),
        "limit" => Ok(DecisionOrderType::Limit),
        other => Err(TradeEngineError::storage(format!(
            "invalid execution order type: {other}"
        ))),
    }
}

fn parse_optional_decimal(value: Option<String>) -> Result<Option<Decimal>, TradeEngineError> {
    value
        .map(|value| {
            Decimal::from_str(value.as_str()).map_err(|error| {
                TradeEngineError::storage(format!("invalid execution decimal: {error}"))
            })
        })
        .transpose()
}

fn parse_stored_decimal(field: &str, value: &str) -> Result<Decimal, TradeEngineError> {
    Decimal::from_str(value).map_err(|error| {
        TradeEngineError::storage(format!("invalid stored decimal in {field}: {error}"))
    })
}

fn parse_trading_day(value: &str) -> Result<TradingDay, TradeEngineError> {
    let format = time::format_description::parse("[year]-[month]-[day]").map_err(|error| {
        TradeEngineError::storage(format!("failed to build trading day parser: {error}"))
    })?;
    let date = time::Date::parse(value, &format)
        .map_err(|error| TradeEngineError::storage(format!("invalid trading day: {error}")))?;
    Ok(TradingDay(date))
}

fn market_scope_storage_columns(
    market_scope: &MarketScope,
) -> (&'static str, Option<&str>, Option<&str>) {
    match market_scope {
        MarketScope::DomesticStock => ("domestic_stock", None, None),
        MarketScope::OverseasStock { exchange, currency } => (
            "overseas_stock",
            Some(exchange.as_str()),
            Some(currency.as_str()),
        ),
    }
}

fn parse_required_broker_order_id(
    value: Option<String>,
    column: usize,
) -> rusqlite::Result<BrokerOrderId> {
    let value = value.ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Null,
            Box::new(trading_agent_kis_client::BrokerError::invariant(
                "missing broker_order_id",
            )),
        )
    })?;
    BrokerOrderId::new(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn parse_required_broker_order_orgno(
    value: Option<String>,
    column: usize,
) -> rusqlite::Result<BrokerOrderOrgNo> {
    let value = value.ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Null,
            Box::new(trading_agent_kis_client::BrokerError::invariant(
                "missing broker_order_orgno",
            )),
        )
    })?;
    BrokerOrderOrgNo::new(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn parse_optional_exchange_code(
    value: Option<String>,
    column: usize,
) -> rusqlite::Result<Option<ExchangeCode>> {
    value
        .map(|value| {
            ExchangeCode::new(value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    column,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()
}

fn tick_sequence_to_i64(tick_sequence: u64) -> Result<i64, TradeEngineError> {
    i64::try_from(tick_sequence)
        .map_err(|error| TradeEngineError::storage(format!("tick sequence overflow: {error}")))
}

fn parse_risk_evaluation_status(value: String) -> rusqlite::Result<RiskEvaluationStatus> {
    RiskEvaluationStatus::parse(&value).ok_or_else(|| {
        rusqlite::Error::InvalidParameterName(format!("invalid risk_evaluations.status: {value}"))
    })
}

fn load_risk_evaluation_for_execution(
    transaction: &Transaction<'_>,
    risk_evaluation_id: i64,
) -> Result<StoredRiskEvaluationForExecution, TradeEngineError> {
    transaction
        .query_row(
            "SELECT run_id, tick_id, decision_proposal_id, status
             FROM risk_evaluations
             WHERE id = ?1",
            [risk_evaluation_id],
            |row| {
                Ok(StoredRiskEvaluationForExecution {
                    run_id: row.get(0)?,
                    tick_id: row.get(1)?,
                    decision_proposal_id: row.get(2)?,
                    status: parse_risk_evaluation_status(row.get::<_, String>(3)?)?,
                })
            },
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| TradeEngineError::storage("risk evaluation was not found"))
}

fn load_decision_proposal_for_execution(
    transaction: &Transaction<'_>,
    decision_proposal_id: i64,
) -> Result<StoredDecisionProposalForExecution, TradeEngineError> {
    transaction
        .query_row(
            "SELECT decision_type, symbol, quantity, order_type, limit_price, status
             FROM decision_proposals
             WHERE id = ?1",
            [decision_proposal_id],
            |row| {
                Ok(StoredDecisionProposalForExecution {
                    decision_type: row.get(0)?,
                    symbol: row.get(1)?,
                    quantity: row.get(2)?,
                    order_type: row.get(3)?,
                    limit_price: row.get(4)?,
                    status: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| TradeEngineError::storage("decision proposal was not found"))
}

fn risk_evaluation_has_execution_request(
    transaction: &Transaction<'_>,
    risk_evaluation_id: i64,
) -> Result<bool, TradeEngineError> {
    transaction
        .query_row(
            "SELECT 1
             FROM execution_requests
             WHERE risk_evaluation_id = ?1",
            [risk_evaluation_id],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(sqlite_error)
}

fn validate_execution_request_matches_proposal(
    record: &NewExecutionRequestRecord,
    proposal: &StoredDecisionProposalForExecution,
) -> Result<(), TradeEngineError> {
    if proposal.status != "valid" {
        return Err(TradeEngineError::storage(
            "execution request requires a valid decision proposal",
        ));
    }
    if !matches!(proposal.decision_type.as_str(), "buy" | "sell") {
        return Err(TradeEngineError::storage(
            "execution request requires a buy or sell decision proposal",
        ));
    }
    if proposal.symbol != record.symbol.as_str() {
        return Err(TradeEngineError::storage(
            "execution request symbol does not match decision proposal",
        ));
    }
    if proposal.quantity != Some(quantity_to_i64(record.quantity)?) {
        return Err(TradeEngineError::storage(
            "execution request quantity does not match decision proposal",
        ));
    }
    if proposal.order_type.as_deref() != Some(record.order_type.as_str()) {
        return Err(TradeEngineError::storage(
            "execution request order type does not match decision proposal",
        ));
    }
    let proposal_limit_price = proposal
        .limit_price
        .as_deref()
        .map(|value| {
            Decimal::from_str(value).map_err(|error| {
                TradeEngineError::storage(format!("invalid proposal limit price: {error}"))
            })
        })
        .transpose()?;
    if proposal_limit_price != record.limit_price {
        return Err(TradeEngineError::storage(
            "execution request limit price does not match decision proposal",
        ));
    }
    Ok(())
}

fn load_execution_request_for_journal_link(
    transaction: &Transaction<'_>,
    id: StoredExecutionRequestId,
) -> Result<Option<StoredExecutionRequestForJournalLink>, TradeEngineError> {
    transaction
        .query_row(
            "SELECT client_order_id, symbol, side, quantity, order_type, limit_price
             FROM execution_requests
             WHERE id = ?1",
            [id.as_i64()],
            |row| {
                Ok(StoredExecutionRequestForJournalLink {
                    client_order_id: row.get(0)?,
                    symbol: row.get(1)?,
                    side: row.get(2)?,
                    quantity: row.get(3)?,
                    order_type: row.get(4)?,
                    limit_price: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(sqlite_error)
}

fn load_journal_command_for_execution_link(
    transaction: &Transaction<'_>,
    client_order_id: &ClientOrderId,
) -> Result<Option<StoredJournalCommandForExecutionLink>, TradeEngineError> {
    transaction
        .query_row(
            "SELECT execution_request_id, command_type, symbol, side, quantity, order_type, limit_price
             FROM broker_command_journal
             WHERE client_order_id = ?1",
            [client_order_id.as_str()],
            |row| {
                Ok(StoredJournalCommandForExecutionLink {
                    execution_request_id: row.get(0)?,
                    command_type: row.get(1)?,
                    symbol: row.get(2)?,
                    side: row.get(3)?,
                    quantity: row.get(4)?,
                    order_type: row.get(5)?,
                    limit_price: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(sqlite_error)
}

fn validate_journal_matches_execution_request(
    journal: &StoredJournalCommandForExecutionLink,
    execution: &StoredExecutionRequestForJournalLink,
) -> Result<(), TradeEngineError> {
    if journal.command_type != "place"
        || journal.symbol != execution.symbol
        || journal.side != execution.side
        || journal.quantity != execution.quantity
        || journal.order_type != execution.order_type
        || parse_optional_decimal_for_link("journal limit_price", journal.limit_price.as_deref())?
            != parse_optional_decimal_for_link(
                "execution limit_price",
                execution.limit_price.as_deref(),
            )?
    {
        return Err(TradeEngineError::storage(
            "broker command journal record does not match execution request",
        ));
    }
    Ok(())
}

fn parse_optional_decimal_for_link(
    field: &str,
    value: Option<&str>,
) -> Result<Option<Decimal>, TradeEngineError> {
    value
        .map(|value| {
            Decimal::from_str(value)
                .map_err(|error| TradeEngineError::storage(format!("invalid {field}: {error}")))
        })
        .transpose()
}

fn insert_execution_event(
    transaction: &Transaction<'_>,
    execution_request_id: StoredExecutionRequestId,
    event_type: ExecutionEventType,
    client_order_id: Option<&str>,
    metadata_json: String,
) -> Result<(), TradeEngineError> {
    transaction
        .execute(
            "INSERT INTO execution_events (
                execution_request_id,
                event_type,
                observed_at,
                client_order_id,
                metadata_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                execution_request_id.as_i64(),
                event_type.as_str(),
                timestamp()?,
                client_order_id,
                metadata_json,
            ],
        )
        .map(|_| ())
        .map_err(sqlite_error)
}

fn open_migrated_connection(path: impl AsRef<Path>) -> Result<Connection, TradeEngineError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TradeEngineError::storage(format!(
                "failed to create main DB directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let mut connection = Connection::open(path).map_err(sqlite_error)?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

fn apply_migrations(connection: &mut Connection) -> Result<(), TradeEngineError> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(sqlite_error)?;
    apply_migration_list(connection, MIGRATIONS)?;
    backfill_missing_migration_checksums(connection)?;
    Ok(())
}

fn apply_migration_list(
    connection: &mut Connection,
    migrations: &[(i64, &str, &str)],
) -> Result<(), TradeEngineError> {
    for (version, name, sql) in migrations {
        if let Some(record) = load_migration_record(connection, *version)? {
            let checksum = migration_checksum(sql);
            if record.name != *name
                && !is_accepted_legacy_migration_record(*version, &record, checksum.as_str())
            {
                return Err(TradeEngineError::storage(format!(
                    "migration {version} name mismatch: stored {}, expected {name}",
                    record.name
                )));
            }
            if let Some(stored_checksum) = record.checksum.as_deref()
                && stored_checksum != checksum
                && !is_accepted_legacy_migration_record(*version, &record, checksum.as_str())
            {
                return Err(TradeEngineError::storage(format!(
                    "migration {version} checksum mismatch"
                )));
            }
            continue;
        }
        let transaction = connection.transaction().map_err(sqlite_error)?;
        transaction.execute_batch(sql).map_err(sqlite_error)?;
        record_migration(&transaction, *version, name, sql)?;
        transaction.commit().map_err(sqlite_error)?;
    }
    Ok(())
}

fn is_accepted_legacy_migration_record(
    version: i64,
    record: &MigrationRecord,
    current_checksum: &str,
) -> bool {
    if version != LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_VERSION
        || record.name != LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_NAME
    {
        return false;
    }
    match record.checksum.as_deref() {
        Some(checksum) => {
            checksum == LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_CHECKSUM
                || checksum == current_checksum
        }
        None => true,
    }
}

struct MigrationRecord {
    name: String,
    checksum: Option<String>,
}

fn load_migration_record(
    connection: &Connection,
    version: i64,
) -> Result<Option<MigrationRecord>, TradeEngineError> {
    if !table_exists(connection, "schema_migrations")? {
        return Ok(None);
    }
    let has_checksum = column_exists(
        connection,
        SchemaTable::SchemaMigrations,
        SchemaColumn::Checksum,
    )?;
    let sql = if has_checksum {
        "SELECT name, checksum FROM schema_migrations WHERE version = ?1"
    } else {
        "SELECT name, NULL AS checksum FROM schema_migrations WHERE version = ?1"
    };
    connection
        .query_row(sql, [version], |row| {
            Ok(MigrationRecord {
                name: row.get(0)?,
                checksum: row.get(1)?,
            })
        })
        .optional()
        .map_err(sqlite_error)
}

fn record_migration(
    connection: &Connection,
    version: i64,
    name: &str,
    sql: &str,
) -> Result<(), TradeEngineError> {
    if column_exists(
        connection,
        SchemaTable::SchemaMigrations,
        SchemaColumn::Checksum,
    )? {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, applied_at, checksum)
                 VALUES (?1, ?2, ?3, ?4)",
                params![version, name, timestamp()?, migration_checksum(sql)],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    } else {
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, applied_at)
                 VALUES (?1, ?2, ?3)",
                params![version, name, timestamp()?],
            )
            .map(|_| ())
            .map_err(sqlite_error)
    }
}

fn backfill_missing_migration_checksums(connection: &Connection) -> Result<(), TradeEngineError> {
    if !column_exists(
        connection,
        SchemaTable::SchemaMigrations,
        SchemaColumn::Checksum,
    )? {
        return Ok(());
    }
    for (version, name, sql) in MIGRATIONS {
        connection
            .execute(
                "UPDATE schema_migrations
                 SET checksum = ?2
                 WHERE version = ?1
                   AND checksum IS NULL
                   AND name = ?3",
                params![version, migration_checksum(sql), name],
            )
            .map_err(sqlite_error)?;
    }
    Ok(())
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool, TradeEngineError> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(sqlite_error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchemaTable {
    SchemaMigrations,
}

impl SchemaTable {
    fn table_info_sql(self) -> &'static str {
        match self {
            Self::SchemaMigrations => "PRAGMA table_info(schema_migrations)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SchemaColumn {
    Checksum,
}

impl SchemaColumn {
    fn as_str(self) -> &'static str {
        match self {
            Self::Checksum => "checksum",
        }
    }
}

fn column_exists(
    connection: &Connection,
    table: SchemaTable,
    column: SchemaColumn,
) -> Result<bool, TradeEngineError> {
    let mut statement = connection
        .prepare(table.table_info_sql())
        .map_err(sqlite_error)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    Ok(columns.iter().any(|stored| stored == column.as_str()))
}

fn migration_checksum(sql: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_migration_rolls_back_sql_batch_and_version_record() {
        let mut connection = Connection::open_in_memory().expect("open in-memory db");
        let migrations = [
            (
                1,
                "schema",
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            ),
            (
                2,
                "partial_failure",
                "CREATE TABLE partial_migration_marker (id INTEGER);
                 CREATE TABLE partial_migration_marker (id INTEGER);",
            ),
        ];

        let error = apply_migration_list(&mut connection, &migrations)
            .expect_err("duplicate table migration should fail");

        assert!(error.to_string().contains("partial_migration_marker"));
        assert!(table_exists(&connection, "schema_migrations").expect("schema table exists"));
        assert!(
            !table_exists(&connection, "partial_migration_marker")
                .expect("partial table lookup succeeds")
        );
        assert_eq!(applied_versions(&connection), vec![1]);
    }

    #[test]
    fn legacy_panic_stop_certification_migration_record_is_accepted() {
        let mut connection = Connection::open_in_memory().expect("open in-memory db");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL,
                    checksum TEXT
                );",
            )
            .expect("create schema_migrations");
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, applied_at, checksum)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_VERSION,
                    LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_NAME,
                    "2026-04-27T00:00:00Z",
                    LEGACY_PANIC_STOP_CERTIFICATION_MIGRATION_CHECKSUM,
                ],
            )
            .expect("insert legacy migration");

        apply_migration_list(
            &mut connection,
            &[(
                8,
                "panic_stop_state",
                include_str!("../../migrations/0008_panic_stop_state.sql"),
            )],
        )
        .expect("legacy migration record remains compatible");
    }

    #[test]
    fn record_tick_advisory_persists_token_usage_metadata() {
        let mut db = MainDb::open(":memory:").expect("open db");
        let run_id = db
            .start_dry_run_run(&DryRunRunContext::new("1234", BrokerEnvironment::Paper))
            .expect("start run");
        let request = minimal_advisor_request();
        let output = crate::advisor::AdvisorOutput {
            summary: "ok".to_owned(),
            observations: Vec::new(),
            risks: Vec::new(),
            next_actions: Vec::new(),
            rationale: "ok".to_owned(),
            evidence: Vec::new(),
            proposals: Vec::new(),
        }
        .validate()
        .expect("valid advisor output");
        let mut audit = AdvisorAudit::completed(request, "command", "status".to_owned(), output);
        audit.token_usage = Some(ProviderTokenUsage {
            input_tokens: Some(10),
            cached_input_tokens: Some(2),
            output_tokens: Some(5),
            reasoning_output_tokens: Some(3),
            total_tokens: Some(15),
        });

        db.record_tick_advisory(run_id, 0, &audit)
            .expect("record advisory");

        let metadata = latest_operator_action_metadata(&db, "tick_advisory_completed");
        assert_eq!(metadata["token_usage"]["input_tokens"], 10);
        assert_eq!(metadata["token_usage"]["cached_input_tokens"], 2);
        assert_eq!(metadata["token_usage"]["output_tokens"], 5);
        assert_eq!(metadata["token_usage"]["reasoning_output_tokens"], 3);
        assert_eq!(metadata["token_usage"]["total_tokens"], 15);
    }

    #[test]
    fn record_universe_snapshot_persists_token_usage_metadata() {
        let mut db = MainDb::open(":memory:").expect("open db");
        let output = UniverseDiscoveryOutput {
            summary: "ok".to_owned(),
            candidates: Vec::new(),
        };
        let usage = ProviderTokenUsage {
            input_tokens: Some(30),
            cached_input_tokens: None,
            output_tokens: Some(11),
            reasoning_output_tokens: None,
            total_tokens: Some(41),
        };

        db.record_universe_snapshot(
            BrokerEnvironment::Paper,
            "1234",
            "command",
            &output,
            Some(&usage),
            Duration::from_secs(60),
        )
        .expect("record universe snapshot");

        let metadata = latest_operator_action_metadata(&db, "universe_snapshot_refreshed");
        assert_eq!(metadata["token_usage"]["input_tokens"], 30);
        assert_eq!(metadata["token_usage"]["output_tokens"], 11);
        assert_eq!(metadata["token_usage"]["total_tokens"], 41);
    }

    #[test]
    fn command_token_usage_summary_groups_advisor_and_universe_usage() {
        let mut db = MainDb::open(":memory:").expect("open db");
        let run_id = db
            .start_dry_run_run(&DryRunRunContext::new("1234", BrokerEnvironment::Paper))
            .expect("start run");
        let request = minimal_advisor_request();
        let output = crate::advisor::AdvisorOutput {
            summary: "ok".to_owned(),
            observations: Vec::new(),
            risks: Vec::new(),
            next_actions: Vec::new(),
            rationale: "ok".to_owned(),
            evidence: Vec::new(),
            proposals: Vec::new(),
        }
        .validate()
        .expect("valid advisor output");
        let mut audit = AdvisorAudit::completed(request, "command", "status".to_owned(), output);
        audit.token_usage = Some(ProviderTokenUsage {
            input_tokens: Some(10),
            cached_input_tokens: Some(2),
            output_tokens: Some(5),
            reasoning_output_tokens: Some(3),
            total_tokens: Some(15),
        });
        db.record_tick_advisory(run_id, 0, &audit)
            .expect("record advisory");

        let universe_output = UniverseDiscoveryOutput {
            summary: "ok".to_owned(),
            candidates: Vec::new(),
        };
        let universe_usage = ProviderTokenUsage {
            input_tokens: Some(30),
            cached_input_tokens: Some(4),
            output_tokens: Some(11),
            reasoning_output_tokens: Some(6),
            total_tokens: Some(41),
        };
        db.record_universe_snapshot(
            BrokerEnvironment::Paper,
            "1234",
            "command",
            &universe_output,
            Some(&universe_usage),
            Duration::from_secs(60),
        )
        .expect("record universe snapshot");

        let summaries = db.command_token_usage_summaries().expect("usage summaries");

        assert_eq!(summaries.len(), 2);
        assert_eq!(
            summaries[0],
            CommandTokenUsageSummary {
                source: CommandTokenUsageSource::Advisor,
                recorded_invocations: 1,
                input_tokens: 10,
                cached_input_tokens: 2,
                output_tokens: 5,
                reasoning_output_tokens: 3,
                total_tokens: 15,
            }
        );
        assert_eq!(
            summaries[1],
            CommandTokenUsageSummary {
                source: CommandTokenUsageSource::UniverseDiscovery,
                recorded_invocations: 1,
                input_tokens: 30,
                cached_input_tokens: 4,
                output_tokens: 11,
                reasoning_output_tokens: 6,
                total_tokens: 41,
            }
        );
    }

    fn minimal_advisor_request() -> crate::advisor::AdvisorRequest {
        let symbol = Symbol::new("AAPL").expect("symbol");
        crate::advisor::AdvisorRequest::dry_run_market_tick(
            BrokerEnvironment::Paper,
            "1234",
            "open",
            1,
            1,
            [symbol],
            crate::advisor::AdvisorMarketTickObservation {
                stage: "test".to_owned(),
                market_session_open: true,
                technical_analysis: Vec::new(),
                domestic_market_alignment: None,
                kis_api: crate::advisor::AdvisorKisApiSummary {
                    account: crate::advisor::AdvisorAccountSummary {
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

    fn latest_operator_action_metadata(db: &MainDb, action_type: &str) -> serde_json::Value {
        let metadata_json = db
            .connection
            .query_row(
                "SELECT metadata_json
                 FROM operator_actions
                 WHERE action_type = ?1
                 ORDER BY id DESC
                 LIMIT 1",
                [action_type],
                |row| row.get::<_, String>(0),
            )
            .expect("operator action metadata");
        serde_json::from_str(&metadata_json).expect("metadata json")
    }

    fn applied_versions(connection: &Connection) -> Vec<i64> {
        let mut statement = connection
            .prepare("SELECT version FROM schema_migrations ORDER BY version")
            .expect("prepare versions");
        statement
            .query_map([], |row| row.get::<_, i64>(0))
            .expect("query versions")
            .collect::<Result<Vec<_>, _>>()
            .expect("versions")
    }
}
