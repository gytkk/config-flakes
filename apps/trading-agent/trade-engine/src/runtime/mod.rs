use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use time::OffsetDateTime;
use tokio::time::{sleep, timeout};
use trading_agent_kis_client::{AccountKey, BrokerEnvironment, BrokerError, OrderSide, OrderState};

use crate::advisor::{
    AdvisorAccountSummary, AdvisorAudit, AdvisorEvidence, AdvisorFillSummary, AdvisorKisApiSummary,
    AdvisorMarketContext, AdvisorMarketTickObservation, AdvisorOpenOrderSummary, AdvisorOutput,
    AdvisorPositionSummary, AdvisorProvider, AdvisorProviderError, AdvisorProviderResponse,
    AdvisorQuoteSummary, AdvisorRequest, AdvisorRuntimeContext, AdvisorTechnicalAnalysisEvidence,
    AdvisorUniverseDiscoveryContext, AdvisorUniverseDiscoverySource, CommandAdvisorProvider,
    MAX_ADVISOR_PROPOSALS,
};
use crate::config::{
    AdvisorConfig, AdvisorProviderKind, DecisionConfig, DomesticMarketAlignmentConfig,
    ExitPolicyConfig, RiskConfig, RuntimeConfig, SizingConfig, StrategyConfig,
    UniverseDiscoveryConfig, UniverseDiscoveryProviderKind, decision_source_id,
};
use crate::decision::{
    DecisionOrderType, DecisionOutputStatus, DecisionPortfolioSnapshot, DecisionProposal,
    DecisionRequest, DecisionType, DecisionValidationContext, DecisionValidationError,
    ValidatedDecisionProposal,
};
use crate::error::TradeEngineError;
use crate::gateway::traits::TradingGateway;
use crate::gateway::types::{
    AccountSnapshot, Fill, MarketScope, MarketSession, OrderStatus, Quantity, Quote, Symbol,
};
use crate::market::MarketProfile;
use crate::market_alignment::{DomesticMarketAlignmentContext, compute_domestic_market_alignment};
use crate::order_price::normalize_overseas_stock_limit_price;
use crate::risk::{
    DailyLossState, RiskEvaluation, RiskEvaluationContext, RiskEvaluationRequest, RiskPolicy,
    RiskPolicySnapshot, missing_sell_cost_basis_symbols,
};
use crate::stdout::{OutputField, StructuredOutput};
use crate::storage::{DryRunAuditStore, StoredRuntimeRunId, StoredUniverseSnapshotId};
use crate::strategy::{
    HoldOnlyStrategy, SmaCrossoverStrategy, StrategyInput, StrategySignal, StrategySignalAction,
    TradingStrategy,
};
use crate::technical_analysis::OhlcvBarInterval;
use crate::universe::{
    CommandUniverseDiscoveryProvider, UniverseDiscoveryBucket, UniverseDiscoveryCandidate,
    UniverseDiscoveryMarketContext, UniverseDiscoveryOutput, UniverseDiscoveryProvider,
    UniverseDiscoveryProviderError, UniverseDiscoveryProviderResponse, UniverseDiscoveryRequest,
    UniverseDiscoveryRuntimeContext, UniverseDiscoverySignal, UniverseDiscoveryTargetFreshness,
};

const ADVISOR_DISPLAY_MAX_ITEMS: usize = 3;
const ADVISOR_DISPLAY_MAX_CHARS: usize = 240;
const UNIVERSE_DISPLAY_MAX_CANDIDATES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLoopControl {
    pub market_hours: RuntimeMarketHours,
    pub max_market_ticks: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeShutdownToken {
    shutdown_requested: Arc<AtomicBool>,
}

impl RuntimeShutdownToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_shutdown(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }

    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone)]
struct MarketSessionCache {
    cached: Option<CachedMarketSession>,
}

#[derive(Debug, Clone)]
struct CachedMarketSession {
    trading_day: String,
    fetched_at: Instant,
    session: MarketSession,
}

impl MarketSessionCache {
    fn empty() -> Self {
        Self { cached: None }
    }

    async fn get<G>(
        &mut self,
        gateway: &G,
        runtime: &RuntimeConfig,
        market_profile: &MarketProfile,
    ) -> Result<MarketSession, TradeEngineError>
    where
        G: TradingGateway + ?Sized,
    {
        let now = Instant::now();
        let trading_day = market_profile.trading_day_string_at(OffsetDateTime::now_utc());
        if let Some(cached) = &self.cached
            && cached.trading_day == trading_day
            && now.duration_since(cached.fetched_at) < runtime.market_session_ttl()
        {
            return Ok(cached.session.clone());
        }

        let session = gateway
            .get_market_session(&trading_day)
            .await
            .map_err(|source| TradeEngineError::gateway("market_session", source))?;
        self.cached = Some(CachedMarketSession {
            trading_day,
            fetched_at: now,
            session: session.clone(),
        });
        Ok(session)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMarketHours {
    Regular,
    OffHours,
}

impl RuntimeMarketHours {
    fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::OffHours => "off_hours",
        }
    }

    fn market_tick_interval(self, runtime: &RuntimeConfig) -> Duration {
        match self {
            Self::Regular => Duration::from_secs(runtime.market_tick_seconds),
            Self::OffHours => Duration::from_secs(runtime.off_hours_market_tick_seconds),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    DryRun,
    SubmitOrders,
}

impl RuntimeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::SubmitOrders => "submit_orders",
        }
    }

    fn loop_heading(self) -> &'static str {
        match self {
            Self::DryRun => "Dry-run loop",
            Self::SubmitOrders => "Order submission loop",
        }
    }

    fn snapshot_heading(self) -> &'static str {
        match self {
            Self::DryRun => "Dry-run snapshot",
            Self::SubmitOrders => "Runtime snapshot",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunSnapshot {
    pub account_suffix: String,
    pub account: AccountSnapshot,
    pub open_orders: Vec<OrderStatus>,
    pub today_fills: Vec<Fill>,
    pub quotes: Vec<Quote>,
    pub position_quotes: Vec<Quote>,
}

impl DryRunSnapshot {
    pub(crate) fn all_quotes(&self) -> Vec<&Quote> {
        let mut seen = HashSet::new();
        let mut quotes = Vec::with_capacity(self.quotes.len() + self.position_quotes.len());
        for quote in self.quotes.iter().chain(self.position_quotes.iter()) {
            if seen.insert(quote.symbol.clone()) {
                quotes.push(quote);
            }
        }
        quotes
    }

    fn quote_for_position(&self, symbol: &Symbol) -> Option<&Quote> {
        self.quotes
            .iter()
            .chain(self.position_quotes.iter())
            .find(|quote| quote.symbol == *symbol)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TickObservation {
    snapshot: DryRunSnapshot,
    price_histories: Vec<QuotePriceHistory>,
    technical_analysis: Vec<AdvisorTechnicalAnalysisEvidence>,
    domestic_market_alignment: Option<DomesticMarketAlignmentContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotePriceHistory {
    symbol: Symbol,
    prices: Vec<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TickAdvisory {
    run_id: crate::storage::StoredRuntimeRunId,
    tick_sequence: u64,
    provider: String,
    raw_output_json: String,
    status: DecisionOutputStatus,
    output: Option<AdvisorOutput>,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TickUniverseDiscovery {
    run_id: crate::storage::StoredRuntimeRunId,
    tick_sequence: u64,
    provider: String,
    snapshot_id: Option<i64>,
    expires_at: Option<String>,
    status: DecisionOutputStatus,
    output: Option<UniverseDiscoveryOutput>,
    error: Option<String>,
    user_watchlist: Vec<Symbol>,
    discovered_symbols: Vec<Symbol>,
    effective_candidates: Vec<Symbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunDecisionAudit {
    pub signal: StrategySignal,
    pub decision_request: DecisionRequest,
    pub decision_provider: String,
    pub raw_decision_output_json: String,
    pub proposal: ValidatedDecisionProposal,
    pub risk: RiskEvaluation,
    pub risk_policy_snapshot: RiskPolicySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunDecisionFailureAudit {
    pub signal: StrategySignal,
    pub decision_request: DecisionRequest,
    pub decision_provider: String,
    pub raw_decision_output_json: String,
    pub status: DecisionOutputStatus,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunAdvisoryAudit {
    pub run_id: Option<i64>,
    pub tick_sequence: u64,
    pub provider: String,
    pub status: DecisionOutputStatus,
    pub output: Option<AdvisorOutput>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunUniverseDiscoveryAudit {
    pub run_id: Option<i64>,
    pub tick_sequence: u64,
    pub provider: String,
    pub snapshot_id: Option<i64>,
    pub expires_at: Option<String>,
    pub status: DecisionOutputStatus,
    pub output: Option<UniverseDiscoveryOutput>,
    pub error: Option<String>,
    pub user_watchlist: Vec<Symbol>,
    pub discovered_symbols: Vec<Symbol>,
    pub effective_candidates: Vec<Symbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunTickOutput {
    pub snapshot: DryRunSnapshot,
    pub technical_analysis: Vec<AdvisorTechnicalAnalysisEvidence>,
    pub domestic_market_alignment: Option<DomesticMarketAlignmentContext>,
    pub universe_discovery: Option<DryRunUniverseDiscoveryAudit>,
    pub advisory: DryRunAdvisoryAudit,
    pub decisions: Vec<DryRunDecisionAudit>,
    pub decision_failures: Vec<DryRunDecisionFailureAudit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniverseRefreshContext {
    pub environment: BrokerEnvironment,
    pub account_suffix: String,
    pub market_scope: MarketScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniverseRefreshOutput {
    pub provider: String,
    pub status: DecisionOutputStatus,
    pub snapshot_id: Option<StoredUniverseSnapshotId>,
    pub output: Option<UniverseDiscoveryOutput>,
    pub error: Option<String>,
}

impl UniverseRefreshOutput {
    pub fn render(&self) -> String {
        let mut block = StructuredOutput::new();
        block
            .heading("Universe refresh")
            .field("status", self.status.as_str())
            .field("provider", self.provider.as_str());
        if let Some(snapshot_id) = self.snapshot_id {
            block.field("snapshot_id", snapshot_id.as_i64());
        }
        if let Some(output) = &self.output {
            block
                .field(
                    "summary",
                    truncate_display_text(&output.summary, ADVISOR_DISPLAY_MAX_CHARS),
                )
                .field("discovered", output.candidates.len());
            write_universe_candidate_items(&mut block, &output.candidates);
        }
        if let Some(error) = self.error.as_deref() {
            block.field("error", error);
        }
        block.blank();
        block.finish()
    }
}

impl DryRunTickOutput {
    pub fn terminal_status(&self) -> DryRunTickStatus {
        if self
            .decision_failures
            .iter()
            .any(|failure| failure.status == DecisionOutputStatus::Failed)
        {
            return DryRunTickStatus::DecisionFailed;
        }
        if self
            .decision_failures
            .iter()
            .any(|failure| failure.status == DecisionOutputStatus::Rejected)
        {
            return DryRunTickStatus::DecisionRejected;
        }
        DryRunTickStatus::Completed
    }

    fn effective_candidates(&self) -> Vec<Symbol> {
        self.universe_discovery
            .as_ref()
            .map(|discovery| discovery.effective_candidates.clone())
            .unwrap_or_else(|| {
                self.snapshot
                    .quotes
                    .iter()
                    .map(|quote| quote.symbol.clone())
                    .collect()
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletedDryRunTick {
    rendered_output: String,
    effective_candidates: Vec<Symbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunRunContext {
    pub account_suffix: String,
    pub environment: BrokerEnvironment,
    pub market_scope: MarketScope,
    pub account_key: Option<AccountKey>,
    pub mode: RuntimeMode,
}

impl DryRunRunContext {
    pub fn new(account_suffix: impl Into<String>, environment: BrokerEnvironment) -> Self {
        Self {
            account_suffix: account_suffix.into(),
            environment,
            market_scope: MarketScope::DomesticStock,
            account_key: None,
            mode: RuntimeMode::DryRun,
        }
    }

    pub fn with_market_scope(mut self, market_scope: MarketScope) -> Self {
        self.market_scope = market_scope;
        self
    }

    pub fn with_account_key(mut self, account_key: AccountKey) -> Self {
        self.account_key = Some(account_key);
        self
    }

    pub fn with_mode(mut self, mode: RuntimeMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn live(account_suffix: impl Into<String>) -> Self {
        Self::new(account_suffix, BrokerEnvironment::Live)
    }

    pub fn paper(account_suffix: impl Into<String>) -> Self {
        Self::new(account_suffix, BrokerEnvironment::Paper)
    }
}

#[derive(Debug, Clone)]
pub struct DryRunRuntimeSettings {
    pub read_only: bool,
    pub runtime: RuntimeConfig,
    pub strategy: StrategyConfig,
    pub decision: DecisionConfig,
    pub advisor: AdvisorConfig,
    pub universe_discovery: UniverseDiscoveryConfig,
    pub domestic_market_alignment: DomesticMarketAlignmentConfig,
    pub sizing: SizingConfig,
    pub exit_policy: ExitPolicyConfig,
    pub risk: RiskConfig,
}

impl DryRunRuntimeSettings {
    pub fn new(runtime: RuntimeConfig, strategy: StrategyConfig) -> Self {
        Self {
            read_only: false,
            runtime,
            strategy,
            decision: DecisionConfig::default(),
            advisor: AdvisorConfig::default(),
            universe_discovery: UniverseDiscoveryConfig::default(),
            domestic_market_alignment: DomesticMarketAlignmentConfig::default(),
            sizing: SizingConfig::default(),
            exit_policy: ExitPolicyConfig::default(),
            risk: RiskConfig::default(),
        }
    }

    pub fn with_decision(
        runtime: RuntimeConfig,
        strategy: StrategyConfig,
        decision: DecisionConfig,
    ) -> Self {
        Self {
            read_only: false,
            runtime,
            strategy,
            decision,
            advisor: AdvisorConfig::default(),
            universe_discovery: UniverseDiscoveryConfig::default(),
            domestic_market_alignment: DomesticMarketAlignmentConfig::default(),
            sizing: SizingConfig::default(),
            exit_policy: ExitPolicyConfig::default(),
            risk: RiskConfig::default(),
        }
    }

    pub fn with_risk(
        runtime: RuntimeConfig,
        strategy: StrategyConfig,
        decision: DecisionConfig,
        risk: RiskConfig,
        read_only: bool,
    ) -> Self {
        Self {
            read_only,
            runtime,
            strategy,
            decision,
            advisor: AdvisorConfig::default(),
            universe_discovery: UniverseDiscoveryConfig::default(),
            domestic_market_alignment: DomesticMarketAlignmentConfig::default(),
            sizing: SizingConfig::default(),
            exit_policy: ExitPolicyConfig::default(),
            risk,
        }
    }

    pub fn with_universe_discovery(mut self, universe_discovery: UniverseDiscoveryConfig) -> Self {
        self.universe_discovery = universe_discovery;
        self
    }

    pub fn with_domestic_market_alignment(
        mut self,
        domestic_market_alignment: DomesticMarketAlignmentConfig,
    ) -> Self {
        self.domestic_market_alignment = domestic_market_alignment;
        self
    }

    pub fn with_advisor(mut self, advisor: AdvisorConfig) -> Self {
        self.advisor = advisor;
        self
    }

    pub fn with_sizing(mut self, sizing: SizingConfig) -> Self {
        self.sizing = sizing;
        self
    }

    pub fn with_exit_policy(mut self, exit_policy: ExitPolicyConfig) -> Self {
        self.exit_policy = exit_policy;
        self
    }

    pub fn validate(&self) -> Result<(), TradeEngineError> {
        self.runtime.validate()?;
        self.strategy.validate()?;
        self.decision.validate()?;
        self.advisor.validate()?;
        self.universe_discovery.validate()?;
        self.domestic_market_alignment.validate()?;
        self.sizing.validate()?;
        self.exit_policy.validate()?;
        self.risk.validate()?;
        Ok(())
    }

    pub fn decision_source_id(&self) -> Result<String, TradeEngineError> {
        decision_source_id(&self.advisor)
    }
}

#[async_trait(?Send)]
pub trait RuntimeTickExecutionDriver<G>: Send + Sync
where
    G: TradingGateway + ?Sized,
{
    async fn reconcile_before_tick(
        &self,
        _gateway: &G,
        _tick_sequence: u64,
    ) -> Result<String, TradeEngineError> {
        Ok(String::new())
    }

    async fn submit_after_tick(
        &self,
        gateway: &G,
        run_id: StoredRuntimeRunId,
        tick_sequence: u64,
        run_context: &DryRunRunContext,
        watchlist: &[Symbol],
        settings: &DryRunRuntimeSettings,
    ) -> Result<String, TradeEngineError>;
}

pub trait RuntimeOutputSink {
    fn write(&mut self, chunk: &str) -> Result<(), TradeEngineError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopRuntimeExecutionDriver;

#[async_trait(?Send)]
impl<G> RuntimeTickExecutionDriver<G> for NoopRuntimeExecutionDriver
where
    G: TradingGateway + Sync + ?Sized,
{
    async fn submit_after_tick(
        &self,
        _gateway: &G,
        _run_id: StoredRuntimeRunId,
        _tick_sequence: u64,
        _run_context: &DryRunRunContext,
        _watchlist: &[Symbol],
        _settings: &DryRunRuntimeSettings,
    ) -> Result<String, TradeEngineError> {
        Ok(String::new())
    }
}

pub struct RuntimeLoopExecutionOptions<'a, E: ?Sized> {
    pub settings: DryRunRuntimeSettings,
    pub control: RuntimeLoopControl,
    pub shutdown: RuntimeShutdownToken,
    pub execution_driver: &'a E,
    pub output_sink: Option<&'a mut dyn RuntimeOutputSink>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeRunStatus {
    Started,
    Completed,
    Failed,
    Interrupted,
}

impl RuntimeRunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DryRunTickStatus {
    Started,
    SkippedPreviousTickRunning,
    DecisionFailed,
    DecisionRejected,
    Completed,
    SnapshotFailed,
    Failed,
}

impl DryRunTickStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::SkippedPreviousTickRunning => "skipped_previous_tick_running",
            Self::DecisionFailed => "decision_failed",
            Self::DecisionRejected => "decision_rejected",
            Self::Completed => "completed",
            Self::SnapshotFailed => "snapshot_failed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunTickFailure {
    pub status: DryRunTickStatus,
    pub stage: String,
    pub symbol: Option<Symbol>,
    pub message: String,
    pub retryable: bool,
}

impl DryRunTickFailure {
    fn snapshot_failed_from_gateway(
        stage: impl Into<String>,
        symbol: Option<Symbol>,
        gateway_stage: &'static str,
        source: BrokerError,
    ) -> Self {
        let retryable = is_retryable_gateway_read_error(&source);
        Self {
            status: DryRunTickStatus::SnapshotFailed,
            stage: stage.into(),
            symbol,
            message: TradeEngineError::gateway(gateway_stage, source).to_string(),
            retryable,
        }
    }

    fn failed(
        stage: impl Into<String>,
        symbol: Option<Symbol>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: DryRunTickStatus::Failed,
            stage: stage.into(),
            symbol,
            message: message.into(),
            retryable: false,
        }
    }

    fn should_retry_next_tick(&self) -> bool {
        self.retryable
    }

    fn to_error(&self) -> TradeEngineError {
        let symbol = self
            .symbol
            .as_ref()
            .map(|symbol| format!(" symbol={symbol}"))
            .unwrap_or_default();
        TradeEngineError::runtime(format!(
            "dry-run tick {} at {}{}: {}",
            self.status.as_str(),
            self.stage,
            symbol,
            self.message
        ))
    }
}

fn is_retryable_gateway_read_error(error: &BrokerError) -> bool {
    matches!(
        error,
        BrokerError::RateLimit { .. }
            | BrokerError::Temporary { .. }
            | BrokerError::Transport { .. }
    )
}

pub async fn run_dry_run_loop<G, S>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    settings: DryRunRuntimeSettings,
    control: RuntimeLoopControl,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let execution_driver = NoopRuntimeExecutionDriver;
    run_runtime_loop_with_execution_and_shutdown(
        gateway,
        store,
        run_context,
        watchlist,
        RuntimeLoopExecutionOptions {
            settings,
            control,
            shutdown: RuntimeShutdownToken::new(),
            execution_driver: &execution_driver,
            output_sink: None,
        },
    )
    .await
}

pub async fn run_dry_run_loop_with_shutdown<G, S>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    settings: DryRunRuntimeSettings,
    control: RuntimeLoopControl,
    shutdown: RuntimeShutdownToken,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let execution_driver = NoopRuntimeExecutionDriver;
    run_runtime_loop_with_execution_and_shutdown(
        gateway,
        store,
        run_context,
        watchlist,
        RuntimeLoopExecutionOptions {
            settings,
            control,
            shutdown,
            execution_driver: &execution_driver,
            output_sink: None,
        },
    )
    .await
}

pub async fn run_dry_run_loop_with_output<G, S>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    settings: DryRunRuntimeSettings,
    control: RuntimeLoopControl,
    output: RuntimeLoopOutputOptions<'_>,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let RuntimeLoopOutputOptions {
        shutdown,
        output_sink,
    } = output;
    let execution_driver = NoopRuntimeExecutionDriver;
    run_runtime_loop_with_execution_and_shutdown(
        gateway,
        store,
        run_context,
        watchlist,
        RuntimeLoopExecutionOptions {
            settings,
            control,
            shutdown,
            execution_driver: &execution_driver,
            output_sink: Some(output_sink),
        },
    )
    .await
}

/// Stream chunks without retaining the complete runtime log. Returns an empty string.
pub async fn stream_dry_run_loop_with_output<G, S>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    settings: DryRunRuntimeSettings,
    control: RuntimeLoopControl,
    output: RuntimeLoopOutputOptions<'_>,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let RuntimeLoopOutputOptions {
        shutdown,
        output_sink,
    } = output;
    let execution_driver = NoopRuntimeExecutionDriver;
    stream_runtime_loop_with_execution_and_shutdown(
        gateway,
        store,
        run_context,
        watchlist,
        RuntimeLoopExecutionOptions {
            settings,
            control,
            shutdown,
            execution_driver: &execution_driver,
            output_sink: Some(output_sink),
        },
    )
    .await
}

pub struct RuntimeLoopOutputOptions<'a> {
    pub shutdown: RuntimeShutdownToken,
    pub output_sink: &'a mut dyn RuntimeOutputSink,
}

pub async fn run_runtime_loop_with_execution<G, S, E>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    settings: DryRunRuntimeSettings,
    control: RuntimeLoopControl,
    execution_driver: &E,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
    E: RuntimeTickExecutionDriver<G> + ?Sized,
{
    run_runtime_loop_with_execution_and_shutdown(
        gateway,
        store,
        run_context,
        watchlist,
        RuntimeLoopExecutionOptions {
            settings,
            control,
            shutdown: RuntimeShutdownToken::new(),
            execution_driver,
            output_sink: None,
        },
    )
    .await
}

pub async fn run_runtime_loop_with_execution_and_shutdown<G, S, E>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    options: RuntimeLoopExecutionOptions<'_, E>,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
    E: RuntimeTickExecutionDriver<G> + ?Sized,
{
    run_runtime_loop(gateway, store, run_context, watchlist, options, true).await
}

/// Stream chunks without retaining the complete runtime log. Returns an empty string.
pub async fn stream_runtime_loop_with_execution_and_shutdown<G, S, E>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    options: RuntimeLoopExecutionOptions<'_, E>,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
    E: RuntimeTickExecutionDriver<G> + ?Sized,
{
    run_runtime_loop(gateway, store, run_context, watchlist, options, false).await
}

async fn run_runtime_loop<G, S, E>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    options: RuntimeLoopExecutionOptions<'_, E>,
    collect_output: bool,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
    E: RuntimeTickExecutionDriver<G> + ?Sized,
{
    let RuntimeLoopExecutionOptions {
        settings,
        control,
        shutdown,
        execution_driver,
        output_sink,
    } = options;
    let mut output_sink = output_sink;
    settings.validate()?;
    let run_id = store.start_dry_run_run(run_context).await?;
    let result = async {
        let market_interval = control.market_hours.market_tick_interval(&settings.runtime);
        let mut loop_header = StructuredOutput::new();
        loop_header
            .heading(run_context.mode.loop_heading())
            .field("status", "started")
            .field("mode", run_context.mode.as_str())
            .field("market_interval_seconds", market_interval.as_secs())
            .field(
                "max_tick_runtime_seconds",
                settings.runtime.max_tick_runtime().as_secs(),
            )
            .field(
                "skip_if_previous_tick_running",
                settings.runtime.skip_if_previous_tick_running,
            )
            .blank();
        let mut output = collect_output.then(String::new);
        append_runtime_output(&mut output, &mut output_sink, loop_header.finish())?;
        tracing::info!(
            event = "runtime_loop",
            status = RuntimeRunStatus::Started.as_str(),
            mode = run_context.mode.as_str(),
            market_interval_seconds = market_interval.as_secs(),
            max_tick_runtime_seconds = settings.runtime.max_tick_runtime().as_secs(),
            skip_if_previous_tick_running = settings.runtime.skip_if_previous_tick_running,
        );

        let mut market_ticks = 0_u64;
        let mut next_market_tick = Instant::now();
        let mut market_session_cache = MarketSessionCache::empty();
        let mut advisor_invocation_state = AdvisorInvocationState::default();
        loop {
            if shutdown.is_shutdown_requested() {
                append_runtime_output(
                    &mut output,
                    &mut output_sink,
                    loop_stopped_block(run_context.mode, "shutdown_signal"),
                )?;
                return Ok((output, RuntimeRunStatus::Interrupted, "shutdown_signal"));
            }

            let now = Instant::now();
            let mut did_work = false;

            if now >= next_market_tick {
                let tick_sequence = market_ticks;
                let tick_index = market_ticks + 1;
                if settings.runtime.skip_if_previous_tick_running
                    && store.has_active_dry_run_tick_lease().await?
                {
                    market_ticks = tick_index;
                    append_runtime_output(
                        &mut output,
                        &mut output_sink,
                        market_tick_status_block(
                            tick_index,
                            DryRunTickStatus::Started.as_str(),
                            &[],
                        ),
                    )?;
                    tracing::info!(
                        event = "market_tick",
                        index = tick_index,
                        tick_sequence,
                        status = DryRunTickStatus::Started.as_str(),
                    );
                    store
                        .record_dry_run_tick_skip(
                            run_id,
                            tick_sequence,
                            DryRunTickStatus::SkippedPreviousTickRunning,
                            "previous tick lease is active",
                        )
                        .await?;
                    append_runtime_output(
                        &mut output,
                        &mut output_sink,
                        market_tick_status_block(
                            tick_index,
                            DryRunTickStatus::SkippedPreviousTickRunning.as_str(),
                            &[],
                        ),
                    )?;
                    tracing::warn!(
                        event = "market_tick",
                        index = tick_index,
                        tick_sequence,
                        status = DryRunTickStatus::SkippedPreviousTickRunning.as_str(),
                    );
                    next_market_tick = now + market_interval;
                    did_work = true;
                } else {
                    match execution_driver
                        .reconcile_before_tick(gateway, tick_sequence)
                        .await
                    {
                        Ok(reconciliation_output) => {
                            append_runtime_output(
                                &mut output,
                                &mut output_sink,
                                reconciliation_output,
                            )?;
                        }
                        Err(error) => {
                            return Err(error);
                        }
                    }
                    market_ticks = tick_index;
                    append_runtime_output(
                        &mut output,
                        &mut output_sink,
                        market_tick_status_block(
                            tick_index,
                            DryRunTickStatus::Started.as_str(),
                            &[],
                        ),
                    )?;
                    tracing::info!(
                        event = "market_tick",
                        index = tick_index,
                        tick_sequence,
                        status = DryRunTickStatus::Started.as_str(),
                    );
                    let market_profile = MarketProfile::new(run_context.market_scope.clone());
                    let market_session = market_session_cache
                        .get(gateway, &settings.runtime, &market_profile)
                        .await?;
                    match run_dry_run_tick_for_run(
                        gateway,
                        store,
                        DryRunMarketTickContext {
                            run_id,
                            tick_sequence,
                            run_context,
                            watchlist,
                            settings: &settings,
                            market_session_open: market_session.is_open,
                        },
                        &mut advisor_invocation_state,
                    )
                    .await
                    {
                        Ok(completed_tick) => {
                            append_runtime_output(
                                &mut output,
                                &mut output_sink,
                                completed_tick.rendered_output,
                            )?;
                            match execution_driver
                                .submit_after_tick(
                                    gateway,
                                    run_id,
                                    tick_sequence,
                                    run_context,
                                    &completed_tick.effective_candidates,
                                    &settings,
                                )
                                .await
                            {
                                Ok(execution_output) => {
                                    append_runtime_output(
                                        &mut output,
                                        &mut output_sink,
                                        execution_output,
                                    )?;
                                }
                                Err(error) => {
                                    return Err(error);
                                }
                            }
                            append_runtime_output(
                                &mut output,
                                &mut output_sink,
                                market_tick_status_block(
                                    tick_index,
                                    DryRunTickStatus::Completed.as_str(),
                                    &[],
                                ),
                            )?;
                            tracing::info!(
                                event = "market_tick",
                                index = tick_index,
                                tick_sequence,
                                status = DryRunTickStatus::Completed.as_str(),
                            );
                        }
                        Err(failure) => {
                            tracing::warn!(
                                event = "market_tick",
                                index = tick_index,
                                tick_sequence,
                                status = failure.status.as_str(),
                                stage = failure.stage.as_str(),
                                symbol = failure.symbol.as_ref().map(Symbol::as_str).unwrap_or(""),
                                error = failure.message.as_str(),
                            );
                            if failure.should_retry_next_tick() {
                                let mut fields = vec![
                                    OutputField::new("stage", failure.stage.as_str()),
                                    OutputField::new("error", failure.message.as_str()),
                                    OutputField::new("retry", "next_tick"),
                                ];
                                if let Some(symbol) = failure.symbol.as_ref() {
                                    fields.push(OutputField::new("symbol", symbol.as_str()));
                                }
                                append_runtime_output(
                                    &mut output,
                                    &mut output_sink,
                                    market_tick_status_block(
                                        tick_index,
                                        failure.status.as_str(),
                                        &fields,
                                    ),
                                )?;
                            } else {
                                return Err(failure.to_error());
                            }
                        }
                    }
                    next_market_tick = now + market_interval;
                    did_work = true;
                }
            }

            if shutdown.is_shutdown_requested() {
                append_runtime_output(
                    &mut output,
                    &mut output_sink,
                    loop_stopped_block(run_context.mode, "shutdown_signal"),
                )?;
                return Ok((output, RuntimeRunStatus::Interrupted, "shutdown_signal"));
            }

            if control
                .max_market_ticks
                .is_some_and(|max_ticks| market_ticks >= max_ticks)
            {
                append_runtime_output(
                    &mut output,
                    &mut output_sink,
                    loop_stopped_block(run_context.mode, "max_market_ticks"),
                )?;
                tracing::info!(
                    event = "runtime_loop",
                    status = RuntimeRunStatus::Completed.as_str(),
                    shutdown_reason = "max_market_ticks",
                );
                return Ok((output, RuntimeRunStatus::Completed, "max_market_ticks"));
            }

            if !did_work {
                tokio::select! {
                    _ = sleep_until_next_tick(next_market_tick) => {}
                    _ = wait_for_runtime_shutdown(shutdown.clone()) => {
                        append_runtime_output(&mut output, &mut output_sink,
                        loop_stopped_block(run_context.mode, "shutdown_signal"))?;
                return Ok((output, RuntimeRunStatus::Interrupted, "shutdown_signal"));
                    }
                }
            }
        }
    }
    .await;

    match result {
        Ok((output, status, reason)) => {
            store
                .finish_dry_run_run(run_id, status, None, Some(reason))
                .await?;
            Ok(output.unwrap_or_default())
        }
        Err(error) => {
            let message = error.to_string();
            if let Err(finalization_error) = store
                .finish_dry_run_run(run_id, RuntimeRunStatus::Failed, Some(&message), None)
                .await
            {
                tracing::error!(event = "runtime_finalization_failed", run_id = ?run_id,
                    error = %finalization_error, "could not persist failed runtime status");
            }
            Err(error)
        }
    }
}

fn append_runtime_output(
    output: &mut Option<String>,
    output_sink: &mut Option<&mut dyn RuntimeOutputSink>,
    chunk: String,
) -> Result<(), TradeEngineError> {
    if let Some(output) = output {
        output.push_str(&chunk);
    }
    if let Some(sink) = output_sink.as_deref_mut() {
        sink.write(&chunk)?;
    }
    Ok(())
}

fn loop_stopped_block(mode: RuntimeMode, reason: &str) -> String {
    let mut block = StructuredOutput::new();
    block
        .heading(mode.loop_heading())
        .field("status", "stopped")
        .field("reason", reason)
        .blank();
    block.finish()
}

fn market_tick_status_block(tick_index: u64, status: &str, fields: &[OutputField]) -> String {
    let mut block = StructuredOutput::new();
    block.heading(format!("Market tick #{tick_index}"));
    block.field("status", status);
    for field in fields {
        block.field(field.key(), field.value());
    }
    block.blank();
    block.finish()
}

fn write_universe_discovery_result(output: &mut String, discovery: &DryRunUniverseDiscoveryAudit) {
    let mut block = StructuredOutput::new();
    block
        .heading(format!(
            "Universe discovery #{}",
            discovery.tick_sequence + 1
        ))
        .field("status", discovery.status.as_str())
        .field("provider", discovery.provider.as_str())
        .field("audit", universe_discovery_audit_display(discovery))
        .field("user_watchlist", discovery.user_watchlist.len())
        .field("discovered", discovery.discovered_symbols.len())
        .field("effective_candidates", discovery.effective_candidates.len());
    if let Some(snapshot_id) = discovery.snapshot_id {
        block.field("snapshot_id", snapshot_id);
    }
    if let Some(expires_at) = discovery.expires_at.as_deref() {
        block.field("expires_at", expires_at);
    }
    if let Some(output) = &discovery.output {
        block.field(
            "summary",
            truncate_display_text(&output.summary, ADVISOR_DISPLAY_MAX_CHARS),
        );
        write_universe_candidate_items(&mut block, &output.candidates);
    }
    if let Some(error) = discovery.error.as_deref() {
        block.field("error", error);
    }
    block.blank();
    output.push_str(&block.finish());
}

fn write_universe_candidate_items(
    block: &mut StructuredOutput,
    candidates: &[UniverseDiscoveryCandidate],
) {
    for candidate in candidates.iter().take(UNIVERSE_DISPLAY_MAX_CANDIDATES) {
        let bucket = candidate
            .bucket
            .map_or("unclassified", |bucket| bucket.as_str());
        block.item(
            candidate.symbol.as_str(),
            &[
                OutputField::new("bucket", bucket),
                OutputField::new("confidence", candidate.confidence),
                OutputField::new(
                    "reason",
                    truncate_display_text(&candidate.reason, ADVISOR_DISPLAY_MAX_CHARS),
                ),
            ],
        );
    }
    if candidates.len() > UNIVERSE_DISPLAY_MAX_CANDIDATES {
        block.field(
            "candidates_truncated",
            format!(
                "showing first {} of {}",
                UNIVERSE_DISPLAY_MAX_CANDIDATES,
                candidates.len()
            ),
        );
    }
}

fn universe_discovery_audit_display(discovery: &DryRunUniverseDiscoveryAudit) -> String {
    if discovery.provider == "cache" {
        return discovery
            .snapshot_id
            .map(|id| format!("universe_snapshots id={id}"))
            .unwrap_or_else(|| "none cache_miss".to_owned());
    }
    format!(
        "operator_actions action={} target=runtime_run:{}",
        if discovery.status == DecisionOutputStatus::Parsed {
            "universe_discovery_completed"
        } else {
            "universe_discovery_failed"
        },
        discovery
            .run_id
            .map(|run_id| run_id.to_string())
            .unwrap_or_else(|| "unknown".to_owned())
    )
}

fn write_tick_advisory_result(output: &mut String, result: TickAdvisoryResultDisplay<'_>) {
    let mut block = StructuredOutput::new();
    block
        .heading(format!("Tick advisory #{}", result.tick_index))
        .field("status", result.status)
        .field("provider", result.provider)
        .field(
            "audit",
            format!(
                "operator_actions action={} target=runtime_run:{}",
                result.audit_action,
                result
                    .run_id
                    .map(|run_id| run_id.to_string())
                    .unwrap_or_else(|| "unknown".to_owned())
            ),
        );
    if let Some(advisor_output) = &result.advisor_output {
        block
            .field("summary", advisor_output.summary.as_str())
            .field("risks", advisor_output.risks.as_str())
            .field("next_actions", advisor_output.next_actions.as_str());
    }
    if let Some(error) = result.error {
        block.field("error", error);
    }
    block.blank();
    output.push_str(&block.finish());
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdvisorOutputDisplay {
    summary: String,
    risks: String,
    next_actions: String,
}

#[derive(Clone)]
struct TickAdvisoryResultDisplay<'a> {
    tick_index: u64,
    status: &'a str,
    provider: &'a str,
    audit_action: &'a str,
    run_id: Option<i64>,
    advisor_output: Option<AdvisorOutputDisplay>,
    error: Option<&'a str>,
}

fn advisor_output_display(output: &AdvisorOutput) -> AdvisorOutputDisplay {
    AdvisorOutputDisplay {
        summary: truncate_display_text(&output.summary, ADVISOR_DISPLAY_MAX_CHARS),
        risks: compact_advisor_items(&output.risks),
        next_actions: compact_advisor_items(&output.next_actions),
    }
}

fn compact_advisor_items(items: &[String]) -> String {
    if items.is_empty() {
        return "none".to_owned();
    }

    let mut compact = items
        .iter()
        .take(ADVISOR_DISPLAY_MAX_ITEMS)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" | ");
    let remaining = items.len().saturating_sub(ADVISOR_DISPLAY_MAX_ITEMS);
    if remaining > 0 {
        compact.push_str(&format!(" | +{remaining} more"));
    }
    truncate_display_text(&compact, ADVISOR_DISPLAY_MAX_CHARS)
}

fn truncate_display_text(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UniverseDiscoveryStage {
    audit: Option<TickUniverseDiscovery>,
    source: AdvisorUniverseDiscoverySource,
    discovered_symbols: Vec<Symbol>,
    discovered_candidates: Vec<UniverseDiscoveryCandidate>,
    effective_candidates: Vec<Symbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdvisorCandidateScope {
    effective_candidates: Vec<Symbol>,
    discovered_symbols: Vec<Symbol>,
    discovered_candidates: Vec<UniverseDiscoveryCandidate>,
    observation: TickObservation,
}

#[derive(Debug, Clone, Default)]
struct AdvisorInvocationState {
    last_command_invocation: Option<AdvisorCommandInvocation>,
}

#[derive(Debug, Clone)]
struct AdvisorCommandInvocation {
    invoked_at: Instant,
    signature: AdvisorInvocationSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdvisorInvocationSignature {
    cash: Decimal,
    positions: Vec<AdvisorPositionSummary>,
    open_orders: Vec<AdvisorOpenOrderSummary>,
    today_fills: Vec<AdvisorFillSummary>,
    domestic_market_alignment: Option<String>,
    effective_candidates: Vec<Symbol>,
    quotes: Vec<AdvisorInvocationQuote>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdvisorInvocationQuote {
    symbol: Symbol,
    last_price: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AdvisorInvocationPolicy {
    min_interval: Duration,
    price_change_threshold_bps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdvisorInvocationDecision {
    Invoke {
        reason: AdvisorInvocationReason,
    },
    Skip {
        reason: AdvisorInvocationSkipReason,
        elapsed: Duration,
        remaining: Duration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdvisorInvocationReason {
    Initial,
    MinIntervalElapsed,
    CandidateSetChanged,
    AccountStateChanged,
    OrderStateChanged,
    FillStateChanged,
    AdvisorContextChanged,
    QuoteSetChanged,
    MaterialPriceChange,
}

impl AdvisorInvocationReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::MinIntervalElapsed => "min_interval_elapsed",
            Self::CandidateSetChanged => "candidate_set_changed",
            Self::AccountStateChanged => "account_state_changed",
            Self::OrderStateChanged => "order_state_changed",
            Self::FillStateChanged => "fill_state_changed",
            Self::AdvisorContextChanged => "advisor_context_changed",
            Self::QuoteSetChanged => "quote_set_changed",
            Self::MaterialPriceChange => "material_price_change",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdvisorInvocationSkipReason {
    MinIntervalActive,
}

impl AdvisorInvocationSkipReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::MinIntervalActive => "min_interval_active",
        }
    }
}

impl AdvisorInvocationState {
    fn evaluate_policy(
        &self,
        policy: &AdvisorInvocationPolicy,
        now: Instant,
        current: &AdvisorInvocationSignature,
    ) -> AdvisorInvocationDecision {
        let Some(previous) = &self.last_command_invocation else {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::Initial,
            };
        };

        if current.effective_candidates != previous.signature.effective_candidates {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::CandidateSetChanged,
            };
        }
        if current.cash != previous.signature.cash
            || current.positions != previous.signature.positions
        {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::AccountStateChanged,
            };
        }
        if current.open_orders != previous.signature.open_orders {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::OrderStateChanged,
            };
        }
        if current.today_fills != previous.signature.today_fills {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::FillStateChanged,
            };
        }
        if current.domestic_market_alignment != previous.signature.domestic_market_alignment {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::AdvisorContextChanged,
            };
        }
        if quote_symbols(&current.quotes) != quote_symbols(&previous.signature.quotes) {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::QuoteSetChanged,
            };
        }
        if current
            .has_material_price_change_since(&previous.signature, policy.price_change_threshold_bps)
        {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::MaterialPriceChange,
            };
        }

        let elapsed = now.duration_since(previous.invoked_at);
        if elapsed >= policy.min_interval {
            return AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::MinIntervalElapsed,
            };
        }

        AdvisorInvocationDecision::Skip {
            reason: AdvisorInvocationSkipReason::MinIntervalActive,
            elapsed,
            remaining: policy.min_interval.saturating_sub(elapsed),
        }
    }

    fn record_command_invocation(
        &mut self,
        invoked_at: Instant,
        signature: AdvisorInvocationSignature,
    ) {
        self.last_command_invocation = Some(AdvisorCommandInvocation {
            invoked_at,
            signature,
        });
    }
}

impl AdvisorInvocationSignature {
    fn from_request(request: &AdvisorRequest) -> Self {
        let tick = &request.current_state.current_tick.kis_api;
        Self {
            cash: tick.account.cash,
            positions: tick.account.positions.clone(),
            open_orders: tick.open_orders.clone(),
            today_fills: tick.today_fills.clone(),
            domestic_market_alignment: request
                .current_state
                .current_tick
                .domestic_market_alignment
                .as_ref()
                .map(domestic_market_alignment_signature),
            effective_candidates: request.runtime.effective_candidates.clone(),
            quotes: tick
                .quotes
                .iter()
                .map(|quote| AdvisorInvocationQuote {
                    symbol: quote.symbol.clone(),
                    last_price: quote.last_price,
                })
                .collect(),
        }
    }

    fn has_material_price_change_since(&self, previous: &Self, threshold_bps: u32) -> bool {
        self.quotes.iter().any(|current| {
            let Some(previous_quote) = previous
                .quotes
                .iter()
                .find(|quote| quote.symbol == current.symbol)
            else {
                return true;
            };
            price_change_bps(current.last_price, previous_quote.last_price)
                >= Decimal::from(threshold_bps)
        })
    }
}

fn domestic_market_alignment_signature(context: &DomesticMarketAlignmentContext) -> String {
    serde_json::to_string(context)
        .unwrap_or_else(|error| format!("domestic_market_alignment_serialization_error:{error}"))
}

impl AdvisorInvocationPolicy {
    fn from_config(config: &AdvisorConfig) -> Self {
        Self {
            min_interval: Duration::from_secs(config.min_command_interval_seconds),
            price_change_threshold_bps: config.price_change_threshold_bps,
        }
    }
}

fn quote_symbols(quotes: &[AdvisorInvocationQuote]) -> Vec<Symbol> {
    quotes.iter().map(|quote| quote.symbol.clone()).collect()
}

fn price_change_bps(current: Decimal, previous: Decimal) -> Decimal {
    let delta = if current >= previous {
        current - previous
    } else {
        previous - current
    };
    let previous_abs = if previous >= Decimal::ZERO {
        previous
    } else {
        -previous
    };
    if previous_abs == Decimal::ZERO {
        return if delta == Decimal::ZERO {
            Decimal::ZERO
        } else {
            Decimal::from(10_000_u32)
        };
    }
    delta * Decimal::from(10_000_u32) / previous_abs
}

impl UniverseDiscoveryStage {
    fn retain_quoted_discoveries(&mut self, user_watchlist: &[Symbol], quotes: &[Quote]) {
        let quoted_symbols = quotes
            .iter()
            .map(|quote| quote.symbol.clone())
            .collect::<HashSet<_>>();
        self.discovered_symbols
            .retain(|symbol| quoted_symbols.contains(symbol));
        self.discovered_candidates
            .retain(|candidate| quoted_symbols.contains(&candidate.symbol));
        self.effective_candidates = effective_candidates(user_watchlist, &self.discovered_symbols);

        if let Some(audit) = &mut self.audit {
            audit.discovered_symbols = self.discovered_symbols.clone();
            audit.effective_candidates = self.effective_candidates.clone();
            if let Some(output) = &mut audit.output {
                output
                    .candidates
                    .retain(|candidate| quoted_symbols.contains(&candidate.symbol));
            }
        }
    }
}

#[derive(Clone, Copy)]
struct UniverseDiscoveryContext<'a> {
    run_id: crate::storage::StoredRuntimeRunId,
    tick_sequence: u64,
    run_context: &'a DryRunRunContext,
    user_watchlist: &'a [Symbol],
    provider: &'a ConfiguredUniverseDiscoveryProvider,
    max_symbols: usize,
}

async fn run_universe_discovery<S>(
    store: &S,
    context: UniverseDiscoveryContext<'_>,
) -> Result<UniverseDiscoveryStage, DryRunTickFailure>
where
    S: DryRunAuditStore,
{
    if matches!(
        context.provider,
        ConfiguredUniverseDiscoveryProvider::Disabled
    ) {
        return Ok(UniverseDiscoveryStage {
            audit: None,
            source: AdvisorUniverseDiscoverySource::disabled(),
            discovered_symbols: Vec::new(),
            discovered_candidates: Vec::new(),
            effective_candidates: context.user_watchlist.to_vec(),
        });
    }

    let tick_index = context.tick_sequence + 1;
    let snapshot = store
        .latest_universe_snapshot(
            context.run_context.environment,
            context.run_context.account_suffix.clone(),
        )
        .await
        .map_err(|error| DryRunTickFailure::failed("UniverseCacheRead", None, error.to_string()))?;
    if let Some(snapshot) = snapshot {
        let discovered_candidates = select_discovered_candidates(
            &snapshot.output,
            context.user_watchlist,
            &context.run_context.market_scope,
            context.max_symbols,
        );
        let discovered_symbols = discovered_candidates
            .iter()
            .map(|candidate| candidate.symbol.clone())
            .collect::<Vec<_>>();
        let effective_candidates =
            effective_candidates(context.user_watchlist, &discovered_symbols);
        tracing::info!(
            event = "universe_cache",
            index = tick_index,
            status = "reused",
            snapshot_id = snapshot.id.as_i64(),
            discovered = discovered_symbols.len(),
            effective_candidates = effective_candidates.len(),
        );
        let source = AdvisorUniverseDiscoverySource {
            provider: "cache".to_owned(),
            status: DecisionOutputStatus::Parsed.as_str().to_owned(),
            snapshot_id: Some(snapshot.id.as_i64()),
            snapshot_provider: Some(snapshot.provider.clone()),
            observed_at: Some(snapshot.observed_at.clone()),
            expires_at: Some(snapshot.expires_at.clone()),
            summary: Some(snapshot.summary.clone()),
            error: None,
        };
        return Ok(UniverseDiscoveryStage {
            audit: Some(TickUniverseDiscovery {
                run_id: context.run_id,
                tick_sequence: context.tick_sequence,
                provider: "cache".to_owned(),
                snapshot_id: Some(snapshot.id.as_i64()),
                expires_at: Some(snapshot.expires_at),
                status: DecisionOutputStatus::Parsed,
                output: Some(snapshot.output),
                error: None,
                user_watchlist: context.user_watchlist.to_vec(),
                discovered_symbols: discovered_symbols.clone(),
                effective_candidates: effective_candidates.clone(),
            }),
            source,
            discovered_symbols,
            discovered_candidates,
            effective_candidates,
        });
    }
    let message = "no valid cached universe snapshot; using user watchlist".to_owned();
    tracing::warn!(
        event = "universe_cache",
        index = tick_index,
        status = "miss",
        provider = context.provider.id(),
        error = message.as_str(),
    );
    Ok(UniverseDiscoveryStage {
        audit: Some(TickUniverseDiscovery {
            run_id: context.run_id,
            tick_sequence: context.tick_sequence,
            provider: "cache".to_owned(),
            snapshot_id: None,
            expires_at: None,
            status: DecisionOutputStatus::Failed,
            output: None,
            error: Some(message.clone()),
            user_watchlist: context.user_watchlist.to_vec(),
            discovered_symbols: Vec::new(),
            effective_candidates: context.user_watchlist.to_vec(),
        }),
        source: AdvisorUniverseDiscoverySource {
            provider: "cache".to_owned(),
            status: DecisionOutputStatus::Failed.as_str().to_owned(),
            snapshot_id: None,
            snapshot_provider: None,
            observed_at: None,
            expires_at: None,
            summary: None,
            error: Some(message),
        },
        discovered_symbols: Vec::new(),
        discovered_candidates: Vec::new(),
        effective_candidates: context.user_watchlist.to_vec(),
    })
}

pub async fn refresh_universe_once<S>(
    store: &S,
    settings: &DryRunRuntimeSettings,
    context: &UniverseRefreshContext,
    user_watchlist: &[Symbol],
) -> Result<UniverseRefreshOutput, TradeEngineError>
where
    S: DryRunAuditStore,
{
    settings.validate()?;
    let provider = ConfiguredUniverseDiscoveryProvider::from_settings(settings)?;
    if matches!(provider, ConfiguredUniverseDiscoveryProvider::Disabled) {
        return Ok(UniverseRefreshOutput {
            provider: provider.id().to_owned(),
            status: DecisionOutputStatus::Failed,
            snapshot_id: None,
            output: None,
            error: Some("universe discovery is disabled".to_owned()),
        });
    }
    let request = UniverseDiscoveryRequest::market_tick(
        UniverseDiscoveryRuntimeContext {
            mode: "universe_refresh".to_owned(),
            environment: context.environment.as_str().to_owned(),
            account_suffix: context.account_suffix.clone(),
            market: UniverseDiscoveryMarketContext::from_market_scope(&context.market_scope),
            market_hours: "refresh".to_owned(),
            tick_index: 1,
            market_tick_count: 1,
        },
        user_watchlist.to_vec(),
    );
    match provider.discover(&request).await {
        Ok(response) => {
            let output = filter_universe_output(
                response.output.output(),
                user_watchlist,
                &context.market_scope,
                settings.universe_discovery.max_symbols,
            );
            let snapshot_id = store
                .record_universe_snapshot(
                    context.environment,
                    context.account_suffix.clone(),
                    provider.id().to_owned(),
                    output.clone(),
                    response.token_usage.clone(),
                    settings.universe_discovery.refresh_interval(),
                )
                .await?;
            Ok(UniverseRefreshOutput {
                provider: provider.id().to_owned(),
                status: DecisionOutputStatus::Parsed,
                snapshot_id: Some(snapshot_id),
                output: Some(output),
                error: None,
            })
        }
        Err(error) => Ok(UniverseRefreshOutput {
            provider: provider.id().to_owned(),
            status: error.output_status(),
            snapshot_id: None,
            output: None,
            error: Some(error.to_string()),
        }),
    }
}

fn select_discovered_candidates(
    output: &UniverseDiscoveryOutput,
    user_watchlist: &[Symbol],
    market_scope: &MarketScope,
    max_symbols: usize,
) -> Vec<UniverseDiscoveryCandidate> {
    let mut seen = user_watchlist.iter().cloned().collect::<HashSet<_>>();
    let market_profile = MarketProfile::new(market_scope.clone());
    let mut candidates = Vec::new();
    for candidate in &output.candidates {
        if candidates.len() >= max_symbols {
            break;
        }
        if !market_profile.symbol_matches(&candidate.symbol) {
            continue;
        }
        if seen.insert(candidate.symbol.clone()) {
            candidates.push(candidate.clone());
        }
    }
    candidates
}

fn select_discovered_symbols(
    output: &UniverseDiscoveryOutput,
    user_watchlist: &[Symbol],
    market_scope: &MarketScope,
    max_symbols: usize,
) -> Vec<Symbol> {
    select_discovered_candidates(output, user_watchlist, market_scope, max_symbols)
        .into_iter()
        .map(|candidate| candidate.symbol)
        .collect()
}

fn filter_universe_output(
    output: &UniverseDiscoveryOutput,
    user_watchlist: &[Symbol],
    market_scope: &MarketScope,
    max_symbols: usize,
) -> UniverseDiscoveryOutput {
    let selected = select_discovered_symbols(output, user_watchlist, market_scope, max_symbols);
    let selected = selected.into_iter().collect::<HashSet<_>>();
    UniverseDiscoveryOutput {
        summary: output.summary.clone(),
        candidates: output
            .candidates
            .iter()
            .filter(|candidate| selected.contains(&candidate.symbol))
            .cloned()
            .collect(),
    }
}

fn effective_candidates(user_watchlist: &[Symbol], discovered_symbols: &[Symbol]) -> Vec<Symbol> {
    let mut effective = Vec::with_capacity(user_watchlist.len() + discovered_symbols.len());
    effective.extend(user_watchlist.iter().cloned());
    effective.extend(discovered_symbols.iter().cloned());
    effective
}

fn advisor_candidate_scope(
    provider: &ConfiguredAdvisorProvider,
    settings: &DryRunRuntimeSettings,
    user_watchlist: &[Symbol],
    universe: &UniverseDiscoveryStage,
    observation: &TickObservation,
) -> AdvisorCandidateScope {
    if !provider.uses_external_command() {
        return AdvisorCandidateScope {
            effective_candidates: universe.effective_candidates.clone(),
            discovered_symbols: universe.discovered_symbols.clone(),
            discovered_candidates: universe.discovered_candidates.clone(),
            observation: observation.clone(),
        };
    }

    let effective_candidates = select_advisor_candidate_symbols(
        user_watchlist,
        &universe.discovered_candidates,
        &observation.snapshot.quotes,
        settings.advisor.candidate_limit,
    );
    let user_symbols = user_watchlist.iter().cloned().collect::<HashSet<_>>();
    let discovered_symbols = effective_candidates
        .iter()
        .filter(|symbol| !user_symbols.contains(*symbol))
        .cloned()
        .collect::<Vec<_>>();
    let discovered_symbol_set = discovered_symbols.iter().cloned().collect::<HashSet<_>>();
    let discovered_candidates = universe
        .discovered_candidates
        .iter()
        .filter(|candidate| discovered_symbol_set.contains(&candidate.symbol))
        .cloned()
        .collect::<Vec<_>>();
    let observation = observation_for_symbols(observation, &effective_candidates);

    AdvisorCandidateScope {
        effective_candidates,
        discovered_symbols,
        discovered_candidates,
        observation,
    }
}

fn select_advisor_candidate_symbols(
    user_watchlist: &[Symbol],
    discovered_candidates: &[UniverseDiscoveryCandidate],
    quotes: &[Quote],
    candidate_limit: usize,
) -> Vec<Symbol> {
    let quoted_symbols = quotes
        .iter()
        .map(|quote| quote.symbol.clone())
        .collect::<HashSet<_>>();
    let mut selected = Vec::with_capacity(candidate_limit);
    let mut seen = HashSet::new();

    for symbol in user_watchlist {
        if selected.len() >= candidate_limit {
            return selected;
        }
        if quoted_symbols.contains(symbol) && seen.insert(symbol.clone()) {
            selected.push(symbol.clone());
        }
    }

    let mut ranked = discovered_candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            quoted_symbols.contains(&candidate.symbol) && !seen.contains(&candidate.symbol)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_index, left), (right_index, right)| {
        advisor_candidate_priority(right)
            .cmp(&advisor_candidate_priority(left))
            .then_with(|| left.risk_flags.len().cmp(&right.risk_flags.len()))
            .then_with(|| right.confidence.cmp(&left.confidence))
            .then_with(|| left_index.cmp(right_index))
    });

    for (_, candidate) in ranked {
        if selected.len() >= candidate_limit {
            break;
        }
        if seen.insert(candidate.symbol.clone()) {
            selected.push(candidate.symbol.clone());
        }
    }

    selected
}

fn advisor_candidate_priority(candidate: &UniverseDiscoveryCandidate) -> (u8, u8) {
    let has_live_confirmation = has_positive_catalyst_or_momentum_hint(candidate);
    (
        target_price_dislocation_priority(candidate, has_live_confirmation),
        u8::from(has_live_confirmation),
    )
}

fn target_price_dislocation_priority(
    candidate: &UniverseDiscoveryCandidate,
    has_live_confirmation: bool,
) -> u8 {
    if !has_target_price_dislocation_hint(candidate) {
        return 0;
    }
    if has_reliable_target_price_support(candidate) && !has_weak_target_price_support(candidate) {
        return 2;
    }

    // Weak target-price evidence (stale, single-broker, or unverified) should not win on
    // upside alone. Keep it in the running only when fresh price action or catalysts suggest
    // the gap may be starting to close now.
    u8::from(has_live_confirmation)
}

fn has_target_price_dislocation_hint(candidate: &UniverseDiscoveryCandidate) -> bool {
    matches!(
        candidate.bucket,
        Some(UniverseDiscoveryBucket::TargetPriceGap)
    ) || candidate.signals.iter().any(|signal| {
        matches!(
            signal,
            UniverseDiscoverySignal::AnalystTargetUpside
                | UniverseDiscoverySignal::ConsensusTargetUpside
                | UniverseDiscoverySignal::TargetPriceRevision
        )
    })
}

fn has_positive_catalyst_or_momentum_hint(candidate: &UniverseDiscoveryCandidate) -> bool {
    matches!(
        candidate.bucket,
        Some(UniverseDiscoveryBucket::Catalyst) | Some(UniverseDiscoveryBucket::ShortTermMomentum)
    ) || candidate.signals.iter().any(|signal| {
        matches!(
            signal,
            UniverseDiscoverySignal::EarningsCatalyst
                | UniverseDiscoverySignal::NewsCatalyst
                | UniverseDiscoverySignal::PolicyCatalyst
                | UniverseDiscoverySignal::VolumeBreakout
                | UniverseDiscoverySignal::PriceBreakout
                | UniverseDiscoverySignal::RelativeStrength
                | UniverseDiscoverySignal::TargetPriceRevision
        )
    })
}

fn has_reliable_target_price_support(candidate: &UniverseDiscoveryCandidate) -> bool {
    let Some(target_support) = candidate.target_support.as_ref() else {
        return false;
    };

    !matches!(
        target_support.freshness,
        Some(UniverseDiscoveryTargetFreshness::Stale)
    ) && (matches!(target_support.consensus, Some(true))
        || target_support.source_count.is_some_and(|count| count >= 2))
}

fn has_weak_target_price_support(candidate: &UniverseDiscoveryCandidate) -> bool {
    candidate.risk_flags.iter().any(|flag| {
        matches!(
            flag,
            crate::universe::UniverseDiscoveryRiskFlag::StaleTargetPrice
                | crate::universe::UniverseDiscoveryRiskFlag::SingleBrokerTarget
                | crate::universe::UniverseDiscoveryRiskFlag::UnverifiedSource
                | crate::universe::UniverseDiscoveryRiskFlag::NewsUnverified
                | crate::universe::UniverseDiscoveryRiskFlag::StaleData
                | crate::universe::UniverseDiscoveryRiskFlag::DataUnavailable
        )
    })
}

fn observation_for_symbols(observation: &TickObservation, symbols: &[Symbol]) -> TickObservation {
    let selected_symbols = symbols.iter().cloned().collect::<HashSet<_>>();
    let mut snapshot = observation.snapshot.clone();
    snapshot
        .quotes
        .retain(|quote| selected_symbols.contains(&quote.symbol));
    let price_histories = observation
        .price_histories
        .iter()
        .filter(|history| selected_symbols.contains(&history.symbol))
        .cloned()
        .collect::<Vec<_>>();
    let technical_analysis = observation
        .technical_analysis
        .iter()
        .filter(|evidence| selected_symbols.contains(&evidence.symbol))
        .cloned()
        .collect::<Vec<_>>();
    let domestic_market_alignment =
        observation
            .domestic_market_alignment
            .clone()
            .map(|mut context| {
                context
                    .candidates
                    .retain(|candidate| selected_symbols.contains(&candidate.symbol));
                context
            });
    TickObservation {
        snapshot,
        price_histories,
        technical_analysis,
        domestic_market_alignment,
    }
}

fn universe_discovery_audit_from_stage(
    discovery: TickUniverseDiscovery,
) -> DryRunUniverseDiscoveryAudit {
    DryRunUniverseDiscoveryAudit {
        run_id: Some(discovery.run_id.as_i64()),
        tick_sequence: discovery.tick_sequence,
        provider: discovery.provider,
        snapshot_id: discovery.snapshot_id,
        expires_at: discovery.expires_at,
        status: discovery.status,
        output: discovery.output,
        error: discovery.error,
        user_watchlist: discovery.user_watchlist,
        discovered_symbols: discovery.discovered_symbols,
        effective_candidates: discovery.effective_candidates,
    }
}

#[derive(Clone, Copy)]
struct TickAdvisoryContext<'a> {
    run_id: crate::storage::StoredRuntimeRunId,
    tick_sequence: u64,
    run_context: &'a DryRunRunContext,
    user_watchlist: &'a [Symbol],
    discovered_universe: &'a [Symbol],
    discovered_candidates: &'a [UniverseDiscoveryCandidate],
    universe_source: &'a AdvisorUniverseDiscoverySource,
    watchlist: &'a [Symbol],
    advisor_provider: &'a ConfiguredAdvisorProvider,
    invocation_policy: AdvisorInvocationPolicy,
    market_session_open: bool,
    strategy: &'a ConfiguredStrategy,
    observation: &'a TickObservation,
    domestic_market_alignment: &'a DomesticMarketAlignmentConfig,
}

async fn run_tick_advisory<S>(
    store: &S,
    context: TickAdvisoryContext<'_>,
    advisor_invocation_state: &mut AdvisorInvocationState,
) -> Result<TickAdvisory, DryRunTickFailure>
where
    S: DryRunAuditStore,
{
    let tick_index = context.tick_sequence + 1;
    let request = AdvisorRequest::market_tick(
        AdvisorRuntimeContext {
            mode: context.run_context.mode.as_str().to_owned(),
            environment: context.run_context.environment.as_str().to_owned(),
            account_suffix: context.run_context.account_suffix.clone(),
            market: AdvisorMarketContext::from_market_scope(&context.run_context.market_scope),
            market_hours: market_hours_for_session(context.market_session_open)
                .as_str()
                .to_owned(),
            tick_index,
            market_tick_count: tick_index,
            user_watchlist: context.user_watchlist.to_vec(),
            discovered_universe: context.discovered_universe.to_vec(),
            effective_candidates: context.watchlist.to_vec(),
            universe_discovery: AdvisorUniverseDiscoveryContext {
                discovered_candidates: context.discovered_candidates.to_vec(),
                source: context.universe_source.clone(),
            },
        },
        advisor_market_tick_observation(
            context.observation,
            context.market_session_open,
            context.domestic_market_alignment.sends_advisor_context(),
        ),
    );

    if !context.market_session_open {
        let output = market_closed_advisor_output(context.observation);
        let validated = output
            .validate()
            .map_err(|error| DryRunTickFailure::failed("TickAdvisory", None, error.to_string()))?;
        let raw_output_json = serde_json::to_string(validated.output()).map_err(|error| {
            DryRunTickFailure::failed(
                "TickAdvisory",
                None,
                format!("failed to encode market-closed advisor output: {error}"),
            )
        })?;
        let output = validated.output().clone();
        let provider = "market_closed";
        let audit = AdvisorAudit::completed(request, provider, raw_output_json.clone(), validated);
        store
            .record_tick_advisory(context.run_id, context.tick_sequence, &audit)
            .await
            .map_err(|error| {
                DryRunTickFailure::failed("TickPersistence", None, error.to_string())
            })?;
        tracing::info!(
            event = "tick_advisory",
            index = tick_index,
            status = "skipped",
            provider,
            reason = "market_closed",
        );
        return Ok(TickAdvisory {
            run_id: context.run_id,
            tick_sequence: context.tick_sequence,
            provider: provider.to_owned(),
            raw_output_json,
            status: DecisionOutputStatus::Parsed,
            output: Some(output),
            error: None,
        });
    }

    match context.advisor_provider {
        ConfiguredAdvisorProvider::DeterministicSma => {
            let output = deterministic_sma_advisor_output(context.strategy, context.observation);
            let validated = output.validate().map_err(|error| {
                DryRunTickFailure::failed("TickAdvisory", None, error.to_string())
            })?;
            let raw_output_json = serde_json::to_string(validated.output()).map_err(|error| {
                DryRunTickFailure::failed(
                    "TickAdvisory",
                    None,
                    format!("failed to encode deterministic advisor output: {error}"),
                )
            })?;
            let output = validated.output().clone();
            let audit = AdvisorAudit::completed(
                request.clone(),
                context.advisor_provider.id(),
                raw_output_json.clone(),
                validated,
            );
            store
                .record_tick_advisory(context.run_id, context.tick_sequence, &audit)
                .await
                .map_err(|error| {
                    DryRunTickFailure::failed("TickPersistence", None, error.to_string())
                })?;
            tracing::info!(
                event = "tick_advisory",
                index = tick_index,
                status = "completed",
                provider = context.advisor_provider.id(),
            );
            Ok(TickAdvisory {
                run_id: context.run_id,
                tick_sequence: context.tick_sequence,
                provider: context.advisor_provider.id().to_owned(),
                raw_output_json,
                status: DecisionOutputStatus::Parsed,
                output: Some(output),
                error: None,
            })
        }
        ConfiguredAdvisorProvider::Command(_) => {
            let signature = AdvisorInvocationSignature::from_request(&request);
            let now = Instant::now();
            match advisor_invocation_state.evaluate_policy(
                &context.invocation_policy,
                now,
                &signature,
            ) {
                AdvisorInvocationDecision::Skip {
                    reason,
                    elapsed,
                    remaining,
                } => {
                    return tick_advisory_from_policy_skip(
                        store,
                        AdvisorPolicySkipContext {
                            run_id: context.run_id,
                            tick_sequence: context.tick_sequence,
                            tick_index,
                            request,
                            reason,
                            elapsed,
                            remaining,
                        },
                    )
                    .await;
                }
                AdvisorInvocationDecision::Invoke { reason } => {
                    tracing::info!(
                        event = "tick_advisory",
                        index = tick_index,
                        status = "invoking",
                        provider = context.advisor_provider.id(),
                        reason = reason.as_str(),
                    );
                }
            }
            let result = context.advisor_provider.advise(&request).await;
            let advisory = tick_advisory_from_provider_result(
                store,
                context.run_id,
                context.tick_sequence,
                tick_index,
                context.advisor_provider.id(),
                request,
                result,
            )
            .await?;
            if advisory.status == DecisionOutputStatus::Parsed && advisory.output.is_some() {
                advisor_invocation_state.record_command_invocation(now, signature);
            }
            Ok(advisory)
        }
    }
}

fn market_closed_advisor_output(observation: &TickObservation) -> AdvisorOutput {
    AdvisorOutput {
        summary: "market is closed; advisor provider was not invoked".to_owned(),
        observations: vec!["market_session_open=false".to_owned()],
        risks: vec!["market is closed".to_owned()],
        next_actions: vec!["wait for the regular market session".to_owned()],
        rationale: "provider calls are skipped while the market is closed".to_owned(),
        evidence: vec![AdvisorEvidence {
            source: "runtime".to_owned(),
            detail: "market_session_open=false".to_owned(),
        }],
        proposals: observation
            .snapshot
            .quotes
            .iter()
            .map(|quote| {
                DecisionProposal::hold(
                    quote.symbol.clone(),
                    Decimal::ZERO,
                    "market is closed; no advisor provider call was made",
                )
            })
            .collect(),
    }
}

struct AdvisorPolicySkipContext {
    run_id: crate::storage::StoredRuntimeRunId,
    tick_sequence: u64,
    tick_index: u64,
    request: AdvisorRequest,
    reason: AdvisorInvocationSkipReason,
    elapsed: Duration,
    remaining: Duration,
}

async fn tick_advisory_from_policy_skip<S>(
    store: &S,
    context: AdvisorPolicySkipContext,
) -> Result<TickAdvisory, DryRunTickFailure>
where
    S: DryRunAuditStore,
{
    let output = policy_skipped_advisor_output(
        &context.request,
        context.reason,
        context.elapsed,
        context.remaining,
    );
    let validated = output
        .validate()
        .map_err(|error| DryRunTickFailure::failed("TickAdvisory", None, error.to_string()))?;
    let raw_output_json = serde_json::to_string(validated.output()).map_err(|error| {
        DryRunTickFailure::failed(
            "TickAdvisory",
            None,
            format!("failed to encode policy-skip advisor output: {error}"),
        )
    })?;
    let output = validated.output().clone();
    let provider = "advisor_policy_skip";
    let audit = AdvisorAudit::completed(
        context.request,
        provider,
        raw_output_json.clone(),
        validated,
    );
    store
        .record_tick_advisory(context.run_id, context.tick_sequence, &audit)
        .await
        .map_err(|error| DryRunTickFailure::failed("TickPersistence", None, error.to_string()))?;
    tracing::info!(
        event = "tick_advisory",
        index = context.tick_index,
        status = "skipped",
        provider,
        reason = context.reason.as_str(),
        elapsed_seconds = context.elapsed.as_secs(),
        remaining_seconds = context.remaining.as_secs(),
    );
    Ok(TickAdvisory {
        run_id: context.run_id,
        tick_sequence: context.tick_sequence,
        provider: provider.to_owned(),
        raw_output_json,
        status: DecisionOutputStatus::Parsed,
        output: Some(output),
        error: None,
    })
}

fn policy_skipped_advisor_output(
    request: &AdvisorRequest,
    reason: AdvisorInvocationSkipReason,
    elapsed: Duration,
    remaining: Duration,
) -> AdvisorOutput {
    AdvisorOutput {
        summary: "advisor command skipped by invocation policy".to_owned(),
        observations: vec![
            format!("advisor_invocation_policy={}", reason.as_str()),
            format!("elapsed_seconds={}", elapsed.as_secs()),
            format!("remaining_seconds={}", remaining.as_secs()),
        ],
        risks: Vec::new(),
        next_actions: vec!["continue fast runtime ticks until advisor policy is due".to_owned()],
        rationale: "command advisor calls are throttled while candidates, account state, orders, fills, and material prices remain stable".to_owned(),
        evidence: vec![AdvisorEvidence {
            source: "runtime".to_owned(),
            detail: format!(
                "reason={} elapsed_seconds={} remaining_seconds={}",
                reason.as_str(),
                elapsed.as_secs(),
                remaining.as_secs()
            ),
        }],
        proposals: request
            .current_state
            .current_tick
            .kis_api
            .quotes
            .iter()
            .take(MAX_ADVISOR_PROPOSALS)
            .map(|quote| {
                DecisionProposal::hold(
                    quote.symbol.clone(),
                    Decimal::ZERO,
                    "advisor command invocation policy skipped this tick",
                )
            })
            .collect(),
    }
}

async fn tick_advisory_from_provider_result<S>(
    store: &S,
    run_id: crate::storage::StoredRuntimeRunId,
    tick_sequence: u64,
    tick_index: u64,
    provider: &str,
    request: AdvisorRequest,
    result: Result<AdvisorProviderResponse, AdvisorProviderError>,
) -> Result<TickAdvisory, DryRunTickFailure>
where
    S: DryRunAuditStore,
{
    match result {
        Ok(response) => {
            let output = response.output.output().clone();
            let mut audit = AdvisorAudit::completed(
                request.clone(),
                provider,
                response.raw_output_json.clone(),
                response.output,
            );
            audit.token_usage = response.token_usage.clone();
            store
                .record_tick_advisory(run_id, tick_sequence, &audit)
                .await
                .map_err(|error| {
                    DryRunTickFailure::failed("TickPersistence", None, error.to_string())
                })?;
            tracing::info!(
                event = "tick_advisory",
                index = tick_index,
                status = "completed",
                provider,
            );
            Ok(TickAdvisory {
                run_id,
                tick_sequence,
                provider: provider.to_owned(),
                raw_output_json: response.raw_output_json,
                status: DecisionOutputStatus::Parsed,
                output: Some(output),
                error: None,
            })
        }
        Err(error) => {
            let status = error.output_status();
            let raw_output_json = error.raw_output_json().to_owned();
            let token_usage = error.token_usage().cloned();
            let message = error.to_string();
            let mut audit = AdvisorAudit::failed(
                request.clone(),
                provider,
                raw_output_json.clone(),
                status,
                message.clone(),
            );
            audit.token_usage = token_usage;
            store
                .record_tick_advisory(run_id, tick_sequence, &audit)
                .await
                .map_err(|error| {
                    DryRunTickFailure::failed("TickPersistence", None, error.to_string())
                })?;
            tracing::warn!(
                event = "tick_advisory",
                index = tick_index,
                status = status.as_str(),
                provider,
                error = message.as_str(),
            );
            Ok(TickAdvisory {
                run_id,
                tick_sequence,
                provider: provider.to_owned(),
                raw_output_json,
                status,
                output: None,
                error: Some(message),
            })
        }
    }
}

fn market_hours_for_session(is_open: bool) -> RuntimeMarketHours {
    if is_open {
        RuntimeMarketHours::Regular
    } else {
        RuntimeMarketHours::OffHours
    }
}

fn deterministic_sma_advisor_output(
    strategy: &ConfiguredStrategy,
    observation: &TickObservation,
) -> AdvisorOutput {
    let mut proposals = Vec::with_capacity(observation.snapshot.quotes.len());
    let mut observations = Vec::with_capacity(observation.snapshot.quotes.len());
    for quote in &observation.snapshot.quotes {
        let prices = price_history_for_symbol(observation, &quote.symbol)
            .unwrap_or_else(|| std::slice::from_ref(&quote.last_price));
        let signal = strategy.generate_signal(&StrategyInput {
            account: &observation.snapshot.account,
            quote,
            recent_prices: prices,
        });
        observations.push(format!(
            "{} {} signal from {} prices",
            quote.symbol,
            signal.action.as_str(),
            prices.len()
        ));
        proposals.push(proposal_from_strategy_signal(&signal, quote));
    }

    AdvisorOutput {
        summary: "deterministic SMA advisor produced tick proposals".to_owned(),
        observations,
        risks: Vec::new(),
        next_actions: vec!["run deterministic engine risk gate".to_owned()],
        rationale: "fallback advisor maps configured strategy signals into structured proposals"
            .to_owned(),
        evidence: vec![crate::advisor::AdvisorEvidence {
            source: "strategy_signal".to_owned(),
            detail: "fallback deterministic advisor used because command advisor is disabled"
                .to_owned(),
        }],
        proposals,
    }
}

fn proposal_from_strategy_signal(signal: &StrategySignal, quote: &Quote) -> DecisionProposal {
    let reason = format!(
        "strategy {} {} signal",
        signal.strategy_id.as_str(),
        signal.action.as_str()
    );
    match signal.action {
        StrategySignalAction::Hold => {
            DecisionProposal::hold(signal.symbol.clone(), signal.strength, reason)
        }
        StrategySignalAction::Buy | StrategySignalAction::Sell => DecisionProposal {
            decision_type: match signal.action {
                StrategySignalAction::Buy => DecisionType::Buy,
                StrategySignalAction::Sell => DecisionType::Sell,
                StrategySignalAction::Hold => DecisionType::Hold,
            },
            symbol: signal.symbol.clone(),
            quantity: Some(Quantity::new(1)),
            order_type: Some(DecisionOrderType::Limit),
            limit_price: Some(quote.last_price),
            confidence: signal.strength,
            time_horizon: Some("intraday".to_owned()),
            reason,
            risk_notes: Vec::new(),
        },
    }
}

fn advisor_market_tick_observation(
    observation: &TickObservation,
    market_session_open: bool,
    include_domestic_market_alignment: bool,
) -> AdvisorMarketTickObservation {
    AdvisorMarketTickObservation {
        stage: "TickObservation".to_owned(),
        market_session_open,
        technical_analysis: observation.technical_analysis.clone(),
        domestic_market_alignment: include_domestic_market_alignment
            .then(|| observation.domestic_market_alignment.clone())
            .flatten(),
        kis_api: advisor_kis_api_summary(observation),
    }
}

fn advisor_kis_api_summary(observation: &TickObservation) -> AdvisorKisApiSummary {
    AdvisorKisApiSummary {
        account: AdvisorAccountSummary {
            cash: observation.snapshot.account.cash.cash,
            position_count: observation.snapshot.account.positions.len(),
            positions: observation
                .snapshot
                .account
                .positions
                .iter()
                .map(|position| AdvisorPositionSummary {
                    symbol: position.symbol.clone(),
                    quantity: position.quantity.get(),
                    average_price: position.average_price,
                })
                .collect(),
        },
        open_order_count: observation.snapshot.open_orders.len(),
        open_orders: observation
            .snapshot
            .open_orders
            .iter()
            .map(advisor_open_order_summary)
            .collect(),
        today_fill_count: observation.snapshot.today_fills.len(),
        today_fills: observation
            .snapshot
            .today_fills
            .iter()
            .map(advisor_fill_summary)
            .collect(),
        quote_count: observation.snapshot.quotes.len(),
        quotes: observation
            .snapshot
            .quotes
            .iter()
            .map(|quote| AdvisorQuoteSummary {
                symbol: quote.symbol.clone(),
                full_name: quote.full_name.clone(),
                last_price: quote.last_price,
                recent_prices: price_history_for_symbol(observation, &quote.symbol)
                    .unwrap_or_else(|| std::slice::from_ref(&quote.last_price))
                    .iter()
                    .map(Decimal::to_string)
                    .collect(),
            })
            .collect(),
    }
}

fn price_history_for_symbol<'a>(
    observation: &'a TickObservation,
    symbol: &Symbol,
) -> Option<&'a [Decimal]> {
    observation
        .price_histories
        .iter()
        .find(|history| history.symbol == *symbol)
        .map(|history| history.prices.as_slice())
}

fn decision_action_for_proposal(proposal: &DecisionProposal) -> StrategySignalAction {
    match proposal.decision_type {
        DecisionType::Buy => StrategySignalAction::Buy,
        DecisionType::Sell => StrategySignalAction::Sell,
        DecisionType::Hold => StrategySignalAction::Hold,
    }
}

fn advisor_open_order_summary(order: &OrderStatus) -> AdvisorOpenOrderSummary {
    let evidence = order.evidence.as_ref();
    AdvisorOpenOrderSummary {
        state: advisor_order_state(&order.state),
        symbol: evidence.map(|evidence| evidence.symbol.clone()),
        side: evidence.and_then(|evidence| evidence.side.map(|side| side.as_str().to_owned())),
        ordered_quantity: evidence.map(|evidence| evidence.ordered_quantity.get()),
        filled_quantity: evidence.map(|evidence| evidence.filled_quantity.get()),
        order_price: evidence.and_then(|evidence| evidence.order_price),
        trading_day: evidence.and_then(|evidence| evidence.trading_day.clone()),
    }
}

fn advisor_fill_summary(fill: &Fill) -> AdvisorFillSummary {
    let evidence = fill.evidence.as_ref();
    AdvisorFillSummary {
        symbol: evidence.map(|evidence| evidence.symbol.clone()),
        side: evidence.and_then(|evidence| evidence.side.map(|side| side.as_str().to_owned())),
        quantity: fill.quantity.get(),
        price: fill.price,
        trading_day: evidence.and_then(|evidence| evidence.trading_day.clone()),
    }
}

fn advisor_order_state(state: &OrderState) -> String {
    match state {
        OrderState::Accepted => "accepted".to_owned(),
        OrderState::PartiallyFilled => "partially_filled".to_owned(),
        OrderState::Filled => "filled".to_owned(),
        OrderState::Cancelled => "cancelled".to_owned(),
        OrderState::Rejected => "rejected".to_owned(),
        OrderState::Unknown(value) => format!("unknown:{value}"),
    }
}

pub async fn run_dry_run_tick<G, S>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    settings: DryRunRuntimeSettings,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let execution_driver = NoopRuntimeExecutionDriver;
    run_runtime_tick_with_execution(
        gateway,
        store,
        run_context,
        watchlist,
        settings,
        &execution_driver,
    )
    .await
}

pub async fn run_runtime_tick_with_execution<G, S, E>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    watchlist: &[Symbol],
    settings: DryRunRuntimeSettings,
    execution_driver: &E,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
    E: RuntimeTickExecutionDriver<G> + ?Sized,
{
    settings.validate()?;
    let run_id = store.start_dry_run_run(run_context).await?;
    let mut market_session_cache = MarketSessionCache::empty();
    let mut advisor_invocation_state = AdvisorInvocationState::default();
    let market_profile = MarketProfile::new(run_context.market_scope.clone());
    let market_session = market_session_cache
        .get(gateway, &settings.runtime, &market_profile)
        .await?;
    match run_dry_run_tick_for_run(
        gateway,
        store,
        DryRunMarketTickContext {
            run_id,
            tick_sequence: 0,
            run_context,
            watchlist,
            settings: &settings,
            market_session_open: market_session.is_open,
        },
        &mut advisor_invocation_state,
    )
    .await
    {
        Ok(completed_tick) => {
            let mut output = completed_tick.rendered_output;
            match execution_driver
                .submit_after_tick(
                    gateway,
                    run_id,
                    0,
                    run_context,
                    &completed_tick.effective_candidates,
                    &settings,
                )
                .await
            {
                Ok(execution_output) => {
                    output.push_str(&execution_output);
                    store
                        .finish_dry_run_run(run_id, RuntimeRunStatus::Completed, None, None)
                        .await?;
                    Ok(output)
                }
                Err(error) => {
                    let message = error.to_string();
                    store
                        .finish_dry_run_run(
                            run_id,
                            RuntimeRunStatus::Failed,
                            Some(message.as_str()),
                            None,
                        )
                        .await?;
                    Err(error)
                }
            }
        }
        Err(failure) => {
            store
                .finish_dry_run_run(
                    run_id,
                    RuntimeRunStatus::Failed,
                    Some(failure.message.as_str()),
                    None,
                )
                .await?;
            Err(failure.to_error())
        }
    }
}

#[derive(Clone, Copy)]
struct DryRunMarketTickContext<'a> {
    run_id: crate::storage::StoredRuntimeRunId,
    tick_sequence: u64,
    run_context: &'a DryRunRunContext,
    watchlist: &'a [Symbol],
    settings: &'a DryRunRuntimeSettings,
    market_session_open: bool,
}

async fn run_dry_run_tick_for_run<G, S>(
    gateway: &G,
    store: &S,
    context: DryRunMarketTickContext<'_>,
    advisor_invocation_state: &mut AdvisorInvocationState,
) -> Result<CompletedDryRunTick, DryRunTickFailure>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let tick_id = store
        .start_dry_run_tick(
            context.run_id,
            context.tick_sequence,
            context.settings.runtime.max_tick_runtime(),
        )
        .await
        .map_err(|error| {
            DryRunTickFailure::failed("start_dry_run_tick", None, error.to_string())
        })?;
    let output = match timeout(
        context.settings.runtime.max_tick_runtime(),
        build_dry_run_tick_from_gateway(gateway, store, context, advisor_invocation_state),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(failure)) => {
            if let Err(record_error) = store.fail_dry_run_tick(tick_id, &failure).await {
                return Err(DryRunTickFailure::failed(
                    "fail_dry_run_tick",
                    None,
                    record_error.to_string(),
                ));
            }
            return Err(failure);
        }
        Err(_) => {
            let failure = DryRunTickFailure::failed(
                "timeout",
                None,
                format!(
                    "tick timeout exceeded max_tick_runtime_seconds={}",
                    context.settings.runtime.max_tick_runtime().as_secs()
                ),
            );
            if let Err(record_error) = store.fail_dry_run_tick(tick_id, &failure).await {
                return Err(DryRunTickFailure::failed(
                    "fail_dry_run_tick",
                    None,
                    record_error.to_string(),
                ));
            }
            return Err(failure);
        }
    };
    store
        .complete_dry_run_tick(context.run_id, tick_id, &output)
        .await
        .map_err(|error| {
            DryRunTickFailure::failed("complete_dry_run_tick", None, error.to_string())
        })?;
    let effective_candidates = output.effective_candidates();
    Ok(CompletedDryRunTick {
        rendered_output: render_dry_run_tick(&output, context.run_context.mode),
        effective_candidates,
    })
}

async fn build_dry_run_tick_from_gateway<G, S>(
    gateway: &G,
    store: &S,
    context: DryRunMarketTickContext<'_>,
    advisor_invocation_state: &mut AdvisorInvocationState,
) -> Result<DryRunTickOutput, DryRunTickFailure>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let strategy = ConfiguredStrategy::from_config(&context.settings.strategy)
        .map_err(|error| DryRunTickFailure::failed("strategy", None, error.to_string()))?;
    let universe_provider = ConfiguredUniverseDiscoveryProvider::from_settings(context.settings)
        .map_err(|error| DryRunTickFailure::failed("UniverseDiscovery", None, error.to_string()))?;
    let advisor_provider = ConfiguredAdvisorProvider::from_settings(context.settings)
        .map_err(|error| DryRunTickFailure::failed("TickAdvisory", None, error.to_string()))?;
    let mut universe = run_universe_discovery(
        store,
        UniverseDiscoveryContext {
            run_id: context.run_id,
            tick_sequence: context.tick_sequence,
            run_context: context.run_context,
            user_watchlist: context.watchlist,
            provider: &universe_provider,
            max_symbols: context.settings.universe_discovery.max_symbols,
        },
    )
    .await?;
    let observation = collect_tick_observation(
        gateway,
        store,
        TickObservationContext {
            account_suffix: context.run_context.account_suffix.as_str(),
            market_scope: &context.run_context.market_scope,
            user_watchlist: context.watchlist,
            discovered_symbols: &universe.discovered_symbols,
            prior_price_count: strategy
                .prior_price_count()
                .max(context.settings.advisor.prior_price_count()),
            domestic_market_alignment_config: &context.settings.domestic_market_alignment,
        },
    )
    .await?;
    universe.retain_quoted_discoveries(context.watchlist, &observation.snapshot.quotes);
    let advisor_scope = advisor_candidate_scope(
        &advisor_provider,
        context.settings,
        context.watchlist,
        &universe,
        &observation,
    );
    let risk_policy = build_risk_policy(context.settings, &universe.effective_candidates);
    let advisory = run_tick_advisory(
        store,
        TickAdvisoryContext {
            run_id: context.run_id,
            tick_sequence: context.tick_sequence,
            run_context: context.run_context,
            user_watchlist: context.watchlist,
            discovered_universe: &advisor_scope.discovered_symbols,
            discovered_candidates: &advisor_scope.discovered_candidates,
            universe_source: &universe.source,
            watchlist: &advisor_scope.effective_candidates,
            advisor_provider: &advisor_provider,
            invocation_policy: AdvisorInvocationPolicy::from_config(&context.settings.advisor),
            market_session_open: context.market_session_open,
            strategy: &strategy,
            observation: &advisor_scope.observation,
            domestic_market_alignment: &context.settings.domestic_market_alignment,
        },
        advisor_invocation_state,
    )
    .await?;
    let risk_runtime_context = collect_risk_runtime_context(
        store,
        context.run_context,
        context.settings,
        &context.run_context.market_scope,
        context.market_session_open,
    )
    .await
    .map_err(|error| DryRunTickFailure::failed("RiskGate", None, error.to_string()))?;
    build_dry_run_tick_output(
        gateway,
        store,
        context.run_context,
        observation,
        DryRunDecisionPipeline {
            universe_discovery: universe.audit.map(universe_discovery_audit_from_stage),
            watchlist: &universe.effective_candidates,
            advisory,
            sizing: &context.settings.sizing,
            exit_policy: &context.settings.exit_policy,
            risk: &context.settings.risk,
            risk_policy,
            risk_runtime_context,
        },
    )
    .await
}

async fn collect_tick_observation<G, S>(
    gateway: &G,
    store: &S,
    context: TickObservationContext<'_>,
) -> Result<TickObservation, DryRunTickFailure>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let snapshot = collect_dry_run_snapshot(
        gateway,
        context.account_suffix,
        context.market_scope,
        context.user_watchlist,
        context.discovered_symbols,
    )
    .await?;
    let mut price_histories = Vec::with_capacity(snapshot.quotes.len());
    for quote in &snapshot.quotes {
        let mut prices = store
            .recent_market_prices(&quote.symbol, context.prior_price_count)
            .await
            .map_err(|error| {
                DryRunTickFailure::failed(
                    "TickObservation.recent_market_prices",
                    Some(quote.symbol.clone()),
                    error.to_string(),
                )
            })?;
        prices.push(quote.last_price);
        price_histories.push(QuotePriceHistory {
            symbol: quote.symbol.clone(),
            prices,
        });
    }
    let technical_analysis = store
        .latest_technical_features_for_symbols(
            &snapshot
                .quotes
                .iter()
                .map(|quote| quote.symbol.clone())
                .collect::<Vec<_>>(),
            OhlcvBarInterval::Daily,
            "daily-core-v1".to_owned(),
        )
        .await
        .map_err(|error| {
            DryRunTickFailure::failed(
                "TickObservation.latest_technical_features",
                None,
                error.to_string(),
            )
        })?
        .into_iter()
        .map(|audit| AdvisorTechnicalAnalysisEvidence {
            symbol: audit.symbol,
            audit_id: audit.id.as_i64(),
            observed_at: audit.observed_at,
            source: audit.source,
            interval: audit.interval,
            feature_set: audit.feature_set,
            features: audit.features,
        })
        .collect();
    let domestic_market_alignment = collect_domestic_market_alignment(
        store,
        context.market_scope,
        &snapshot.quotes,
        context.domestic_market_alignment_config,
    )
    .await?;
    Ok(TickObservation {
        snapshot,
        price_histories,
        technical_analysis,
        domestic_market_alignment,
    })
}

#[derive(Clone, Copy)]
struct TickObservationContext<'a> {
    account_suffix: &'a str,
    market_scope: &'a MarketScope,
    user_watchlist: &'a [Symbol],
    discovered_symbols: &'a [Symbol],
    prior_price_count: usize,
    domestic_market_alignment_config: &'a DomesticMarketAlignmentConfig,
}

async fn collect_domestic_market_alignment<S>(
    store: &S,
    market_scope: &MarketScope,
    quotes: &[Quote],
    config: &DomesticMarketAlignmentConfig,
) -> Result<Option<DomesticMarketAlignmentContext>, DryRunTickFailure>
where
    S: DryRunAuditStore,
{
    if !config.enabled || !matches!(market_scope, MarketScope::DomesticStock) {
        return Ok(None);
    }

    let benchmark_symbol = config.benchmark_symbol().map_err(|error| {
        DryRunTickFailure::failed(
            "TickObservation.domestic_market_alignment.config",
            None,
            error.to_string(),
        )
    })?;
    let candidate_symbols = quotes
        .iter()
        .map(|quote| quote.symbol.clone())
        .collect::<Vec<_>>();
    let candidate_audits = store
        .latest_technical_features_for_symbols(
            &candidate_symbols,
            OhlcvBarInterval::Daily,
            config.feature_set.clone(),
        )
        .await
        .map_err(|error| {
            DryRunTickFailure::failed(
                "TickObservation.domestic_market_alignment.candidates",
                None,
                error.to_string(),
            )
        })?;
    let candidate_features = candidate_audits
        .into_iter()
        .map(|audit| audit.features)
        .collect::<Vec<_>>();
    let benchmark_features = if let Some(symbol) = benchmark_symbol.as_ref() {
        store
            .latest_technical_features_for_symbols(
                std::slice::from_ref(symbol),
                OhlcvBarInterval::Daily,
                config.feature_set.clone(),
            )
            .await
            .map_err(|error| {
                DryRunTickFailure::failed(
                    "TickObservation.domestic_market_alignment.benchmark",
                    None,
                    error.to_string(),
                )
            })?
            .into_iter()
            .next()
            .map(|audit| audit.features)
    } else {
        None
    };

    Ok(Some(compute_domestic_market_alignment(
        config.mode,
        benchmark_symbol,
        benchmark_features.as_ref(),
        &candidate_features,
        config.relative_strength_lookback_days,
    )))
}

async fn collect_dry_run_snapshot<G>(
    gateway: &G,
    account_suffix: &str,
    market_scope: &MarketScope,
    user_watchlist: &[Symbol],
    discovered_symbols: &[Symbol],
) -> Result<DryRunSnapshot, DryRunTickFailure>
where
    G: TradingGateway + ?Sized,
{
    let account = gateway.get_account_snapshot().await.map_err(|source| {
        DryRunTickFailure::snapshot_failed_from_gateway(
            "TickObservation.account_snapshot",
            None,
            "account_snapshot",
            source,
        )
    })?;
    let open_orders = gateway.list_open_orders().await.map_err(|source| {
        DryRunTickFailure::snapshot_failed_from_gateway(
            "TickObservation.open_orders",
            None,
            "open_orders",
            source,
        )
    })?;
    let today_fills = gateway
        .list_fills(
            MarketProfile::new(market_scope.clone())
                .trading_day_for_fills_at(OffsetDateTime::now_utc()),
        )
        .await
        .map_err(|source| {
            DryRunTickFailure::snapshot_failed_from_gateway(
                "TickObservation.fills",
                None,
                "fills",
                source,
            )
        })?;

    let mut quotes = Vec::with_capacity(user_watchlist.len() + discovered_symbols.len());
    let mut collected_symbols = HashSet::new();
    for symbol in user_watchlist {
        if collected_symbols.contains(symbol) {
            continue;
        }
        let quote = gateway.get_quote(symbol).await.map_err(|source| {
            DryRunTickFailure::snapshot_failed_from_gateway(
                "TickObservation.quote",
                Some(symbol.clone()),
                "quote",
                source,
            )
        })?;
        collected_symbols.insert(symbol.clone());
        quotes.push(quote);
    }
    for symbol in discovered_symbols {
        if collected_symbols.contains(symbol) {
            continue;
        }
        match gateway.get_quote(symbol).await {
            Ok(quote) => {
                collected_symbols.insert(symbol.clone());
                quotes.push(quote);
            }
            Err(source) => {
                tracing::warn!(
                    event = "tick_observation",
                    stage = "quote",
                    symbol = symbol.as_str(),
                    error = %TradeEngineError::gateway("quote", source),
                    "skipping discovered universe candidate with unavailable quote"
                );
            }
        }
    }
    let mut position_quotes = Vec::with_capacity(account.positions.len());
    for position in &account.positions {
        if collected_symbols.contains(&position.symbol) {
            continue;
        }
        let quote = gateway
            .get_quote(&position.symbol)
            .await
            .map_err(|source| {
                DryRunTickFailure::snapshot_failed_from_gateway(
                    "TickObservation.position_quote",
                    Some(position.symbol.clone()),
                    "quote",
                    source,
                )
            })?;
        collected_symbols.insert(position.symbol.clone());
        position_quotes.push(quote);
    }

    Ok(DryRunSnapshot {
        account_suffix: account_suffix.to_owned(),
        account,
        open_orders,
        today_fills,
        quotes,
        position_quotes,
    })
}

enum ConfiguredStrategy {
    HoldOnly(HoldOnlyStrategy),
    SmaCrossover(SmaCrossoverStrategy),
}

impl ConfiguredStrategy {
    fn from_config(config: &StrategyConfig) -> Result<Self, TradeEngineError> {
        config.validate()?;
        match config.id.as_str() {
            "hold-only" => Ok(Self::HoldOnly(
                HoldOnlyStrategy::new(config.id.clone())
                    .map_err(|error| TradeEngineError::config(error.to_string()))?,
            )),
            "sma-crossover-v1" => {
                let short_window = config.short_window.ok_or_else(|| {
                    TradeEngineError::config(
                        "strategy.short_window is required for sma-crossover-v1",
                    )
                })?;
                let long_window = config.long_window.ok_or_else(|| {
                    TradeEngineError::config(
                        "strategy.long_window is required for sma-crossover-v1",
                    )
                })?;
                Ok(Self::SmaCrossover(
                    SmaCrossoverStrategy::new(config.id.clone(), short_window, long_window)
                        .map_err(|error| TradeEngineError::config(error.to_string()))?,
                ))
            }
            other => Err(TradeEngineError::config(format!(
                "unsupported strategy.id: {other}"
            ))),
        }
    }

    fn prior_price_count(&self) -> usize {
        match self {
            Self::HoldOnly(_) => 0,
            Self::SmaCrossover(strategy) => strategy.prior_price_count(),
        }
    }

    fn generate_signal(&self, input: &StrategyInput<'_>) -> StrategySignal {
        match self {
            Self::HoldOnly(strategy) => TradingStrategy::generate_signal(strategy, input),
            Self::SmaCrossover(strategy) => strategy.generate_signal(input),
        }
    }
}

enum ConfiguredUniverseDiscoveryProvider {
    Disabled,
    Command(CommandUniverseDiscoveryProvider),
}

impl ConfiguredUniverseDiscoveryProvider {
    fn from_settings(settings: &DryRunRuntimeSettings) -> Result<Self, TradeEngineError> {
        settings.universe_discovery.validate()?;
        match settings.universe_discovery.provider {
            UniverseDiscoveryProviderKind::Disabled => Ok(Self::Disabled),
            UniverseDiscoveryProviderKind::Command => {
                let command = settings
                    .universe_discovery
                    .command
                    .clone()
                    .ok_or_else(|| {
                        TradeEngineError::config(
                            "universe_discovery.command is required when universe_discovery.provider=command",
                        )
                    })?;
                Ok(Self::Command(CommandUniverseDiscoveryProvider::new(
                    command,
                    settings.universe_discovery.args.clone(),
                    Duration::from_secs(settings.universe_discovery.timeout_seconds),
                    settings.universe_discovery.max_stderr_bytes,
                    settings.universe_discovery.max_stdout_bytes,
                )))
            }
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Disabled => "disabled",
            Self::Command(provider) => provider.id(),
        }
    }

    async fn discover(
        &self,
        request: &UniverseDiscoveryRequest,
    ) -> Result<UniverseDiscoveryProviderResponse, UniverseDiscoveryProviderError> {
        match self {
            Self::Disabled => Err(UniverseDiscoveryProviderError::Internal {
                message: "universe discovery is disabled".to_owned(),
            }),
            Self::Command(provider) => provider.discover(request).await,
        }
    }
}

enum ConfiguredAdvisorProvider {
    DeterministicSma,
    Command(CommandAdvisorProvider),
}

impl ConfiguredAdvisorProvider {
    fn from_settings(settings: &DryRunRuntimeSettings) -> Result<Self, TradeEngineError> {
        settings.advisor.validate()?;
        settings.decision.validate()?;
        match settings.advisor.provider {
            AdvisorProviderKind::Disabled => Ok(Self::DeterministicSma),
            AdvisorProviderKind::Command => {
                let command = settings.advisor.command.clone().ok_or_else(|| {
                    TradeEngineError::config(
                        "advisor.command is required when advisor.provider=command",
                    )
                })?;
                Ok(Self::Command(CommandAdvisorProvider::new(
                    command,
                    settings.advisor.args.clone(),
                    Duration::from_secs(settings.advisor.timeout_seconds),
                    settings.advisor.max_stderr_bytes,
                    settings.advisor.max_stdout_bytes,
                )))
            }
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::DeterministicSma => "deterministic_sma",
            Self::Command(provider) => provider.id(),
        }
    }

    fn uses_external_command(&self) -> bool {
        matches!(self, Self::Command(_))
    }

    async fn advise(
        &self,
        request: &AdvisorRequest,
    ) -> Result<AdvisorProviderResponse, AdvisorProviderError> {
        match self {
            Self::DeterministicSma => Err(AdvisorProviderError::Internal {
                message: "deterministic SMA advisor is built in".to_owned(),
            }),
            Self::Command(provider) => provider.advise(request).await,
        }
    }
}

struct DryRunDecisionPipeline<'a> {
    universe_discovery: Option<DryRunUniverseDiscoveryAudit>,
    watchlist: &'a [Symbol],
    advisory: TickAdvisory,
    sizing: &'a SizingConfig,
    exit_policy: &'a ExitPolicyConfig,
    risk: &'a RiskConfig,
    risk_policy: RiskPolicy,
    risk_runtime_context: RiskRuntimeContext,
}

fn build_risk_policy(settings: &DryRunRuntimeSettings, watchlist: &[Symbol]) -> RiskPolicy {
    RiskPolicy::new(
        settings.read_only,
        settings.risk.enable_live_trading,
        watchlist.iter().cloned(),
        settings.risk.max_order_notional,
    )
    .with_position_notional_limit(settings.risk.max_position_notional)
    .with_daily_loss_limit(settings.risk.max_daily_loss)
    .with_daily_order_limit(settings.risk.max_daily_order_count)
}

async fn sleep_until_next_tick(next_market_tick: Instant) {
    let now = Instant::now();
    let duration = next_market_tick
        .checked_duration_since(now)
        .unwrap_or(Duration::ZERO);
    sleep(duration).await;
}

async fn wait_for_runtime_shutdown(shutdown: RuntimeShutdownToken) {
    while !shutdown.is_shutdown_requested() {
        sleep(Duration::from_millis(100)).await;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RiskRuntimeContext {
    market_session_open: bool,
    unresolved_journal_commands: bool,
    panic_stop_active: bool,
    submitted_order_count_today: u32,
}

async fn collect_risk_runtime_context<S>(
    store: &S,
    _run_context: &DryRunRunContext,
    _settings: &DryRunRuntimeSettings,
    market_scope: &MarketScope,
    market_session_open: bool,
) -> Result<RiskRuntimeContext, TradeEngineError>
where
    S: DryRunAuditStore,
{
    let now = OffsetDateTime::now_utc();
    let day_start = MarketProfile::new(market_scope.clone()).day_start_utc_at(now);
    let unresolved_journal_commands = store.has_unresolved_broker_commands().await?;
    let panic_stop_active = store.panic_stop_active().await?;
    let submitted_order_count_today = store.execution_request_count_since(day_start).await?;

    Ok(RiskRuntimeContext {
        market_session_open,
        unresolved_journal_commands,
        panic_stop_active,
        submitted_order_count_today,
    })
}

async fn build_dry_run_tick_output<G, S>(
    gateway: &G,
    store: &S,
    run_context: &DryRunRunContext,
    observation: TickObservation,
    pipeline: DryRunDecisionPipeline<'_>,
) -> Result<DryRunTickOutput, DryRunTickFailure>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let validation_context = DecisionValidationContext::new(pipeline.watchlist.iter().cloned());
    let advisory_audit = advisory_audit_from_stage(&pipeline.advisory);
    let domestic_market_alignment = observation.domestic_market_alignment.clone();

    let mut decisions = Vec::new();
    let mut decision_failures = Vec::new();
    let Some(advisor_output) = pipeline.advisory.output.as_ref() else {
        decision_failures.push(decision_failure_from_advisory_error(
            run_context,
            &observation,
            &pipeline.advisory,
        )?);
        return Ok(DryRunTickOutput {
            snapshot: observation.snapshot,
            technical_analysis: observation.technical_analysis,
            domestic_market_alignment,
            universe_discovery: pipeline.universe_discovery,
            advisory: advisory_audit,
            decisions,
            decision_failures,
        });
    };

    if advisor_output.proposals.is_empty() {
        decision_failures.push(decision_failure_from_advisory_rejection(
            run_context,
            &observation,
            &pipeline.advisory,
            "advisor output did not include any proposals".to_owned(),
        )?);
        return Ok(DryRunTickOutput {
            snapshot: observation.snapshot,
            technical_analysis: observation.technical_analysis,
            domestic_market_alignment,
            universe_discovery: pipeline.universe_discovery,
            advisory: advisory_audit,
            decisions,
            decision_failures,
        });
    }

    let proposal_candidates = proposal_candidates_for_tick(
        &observation,
        advisor_output.proposals.clone(),
        &pipeline.advisory,
        pipeline.sizing,
        pipeline.exit_policy,
        pipeline.risk,
    )
    .map_err(|error| DryRunTickFailure::failed("ProposalPolicy", None, error.to_string()))?;

    for candidate in proposal_candidates {
        let proposal = normalize_limit_proposal_for_market_scope(
            candidate.proposal,
            &run_context.market_scope,
        );
        let (signal, request, quote) = decision_request_for_advisor_proposal(
            run_context,
            &observation,
            candidate.provider.as_str(),
            &proposal,
        )?;
        let proposal = match proposal.validate(&validation_context) {
            Ok(proposal) => proposal,
            Err(error) => {
                decision_failures.push(decision_failure_from_validation_error(
                    signal,
                    request,
                    candidate.provider.as_str(),
                    candidate.raw_output_json.as_str(),
                    error,
                ));
                break;
            }
        };
        let risk_context = build_proposal_risk_context(
            gateway,
            store,
            &observation.snapshot,
            quote,
            &proposal,
            pipeline.risk_runtime_context,
        )
        .await?;
        let risk = pipeline.risk_policy.evaluate(RiskEvaluationRequest {
            proposal: &proposal,
            context: risk_context,
        });
        decisions.push(DryRunDecisionAudit {
            signal,
            decision_request: request,
            decision_provider: candidate.provider,
            raw_decision_output_json: candidate.raw_output_json,
            proposal,
            risk,
            risk_policy_snapshot: pipeline.risk_policy.snapshot(),
        });
    }

    if !decision_failures.is_empty() {
        decisions.clear();
    }

    Ok(DryRunTickOutput {
        snapshot: observation.snapshot,
        technical_analysis: observation.technical_analysis,
        domestic_market_alignment,
        universe_discovery: pipeline.universe_discovery,
        advisory: advisory_audit,
        decisions,
        decision_failures,
    })
}

fn advisory_audit_from_stage(advisory: &TickAdvisory) -> DryRunAdvisoryAudit {
    DryRunAdvisoryAudit {
        run_id: Some(advisory.run_id.as_i64()),
        tick_sequence: advisory.tick_sequence,
        provider: advisory.provider.clone(),
        status: advisory.status,
        output: advisory.output.clone(),
        error: advisory.error.clone(),
    }
}

#[derive(Debug, Clone)]
struct ProposalCandidate {
    provider: String,
    raw_output_json: String,
    proposal: DecisionProposal,
}

fn proposal_candidates_for_tick(
    observation: &TickObservation,
    advisor_proposals: Vec<DecisionProposal>,
    advisory: &TickAdvisory,
    sizing: &SizingConfig,
    exit_policy: &ExitPolicyConfig,
    risk: &RiskConfig,
) -> Result<Vec<ProposalCandidate>, serde_json::Error> {
    let mut candidates = Vec::new();
    let exit_proposals = exit_policy_proposals(observation, exit_policy);
    let exit_symbols = exit_proposals
        .iter()
        .map(|proposal| proposal.symbol.clone())
        .collect::<HashSet<_>>();

    for proposal in advisor_proposals {
        if exit_symbols.contains(&proposal.symbol) {
            continue;
        }
        let quote = quote_for_symbol(observation, &proposal.symbol);
        let proposal = quote
            .map(|quote| sized_buy_proposal(observation, quote, proposal.clone(), sizing, risk))
            .unwrap_or(proposal);
        candidates.push(ProposalCandidate {
            provider: advisory.provider.clone(),
            raw_output_json: advisory.raw_output_json.clone(),
            proposal,
        });
    }

    for proposal in exit_proposals {
        let raw_output_json = serde_json::to_string(&proposal)?;
        candidates.push(ProposalCandidate {
            provider: "exit_policy".to_owned(),
            raw_output_json,
            proposal,
        });
    }

    Ok(candidates)
}

fn sized_buy_proposal(
    observation: &TickObservation,
    quote: &Quote,
    mut proposal: DecisionProposal,
    sizing: &SizingConfig,
    risk: &RiskConfig,
) -> DecisionProposal {
    if !sizing.enabled || proposal.decision_type != DecisionType::Buy {
        return proposal;
    }
    let price = proposal.limit_price.unwrap_or(quote.last_price);
    if price <= Decimal::ZERO {
        return proposal;
    }

    let account_value = estimated_account_value(observation);
    let target_notional = account_value * sizing.target_account_fraction;
    let current_notional = position_notional(&observation.snapshot.account, quote);
    let remaining_position_room =
        (risk.max_position_notional - current_notional).max(Decimal::ZERO);
    let max_notional = [
        target_notional,
        risk.max_order_notional,
        observation.snapshot.account.cash.cash,
        remaining_position_room,
    ]
    .into_iter()
    .min()
    .unwrap_or(Decimal::ZERO);
    let sized_quantity = (max_notional / price).floor().to_u64().unwrap_or_default();

    if sized_quantity == 0 {
        return DecisionProposal::hold(
            proposal.symbol,
            proposal.confidence,
            format!(
                "sizing policy held buy because target notional {max_notional} is below one share at {price}"
            ),
        );
    }

    let original_quantity = proposal.quantity.map(Quantity::get);
    if original_quantity != Some(sized_quantity) {
        proposal.quantity = Some(Quantity::new(sized_quantity));
        proposal.reason = format!(
            "{}; sizing adjusted quantity from {} to {} using target_account_fraction {}",
            proposal.reason,
            original_quantity
                .map(|quantity| quantity.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            sized_quantity,
            sizing.target_account_fraction
        );
    }
    proposal
}

fn exit_policy_proposals(
    observation: &TickObservation,
    exit_policy: &ExitPolicyConfig,
) -> Vec<DecisionProposal> {
    if !exit_policy.enabled {
        return Vec::new();
    }

    let mut proposals = Vec::new();
    for quote in &observation.snapshot.quotes {
        let Some(position) = observation
            .snapshot
            .account
            .positions
            .iter()
            .find(|position| position.symbol == quote.symbol)
        else {
            continue;
        };
        if position.quantity.is_zero() || position.average_price <= Decimal::ZERO {
            continue;
        }

        let stop_price = position.average_price * (Decimal::ONE - exit_policy.stop_loss_pct);
        let take_profit_price =
            position.average_price * (Decimal::ONE + exit_policy.take_profit_pct);
        let reason = if quote.last_price <= stop_price {
            Some(format!(
                "exit policy stop-loss: last price {} is at or below stop price {} from average price {}",
                quote.last_price, stop_price, position.average_price
            ))
        } else if quote.last_price >= take_profit_price {
            Some(format!(
                "exit policy take-profit: last price {} is at or above take-profit price {} from average price {}",
                quote.last_price, take_profit_price, position.average_price
            ))
        } else {
            None
        };

        if let Some(reason) = reason {
            proposals.push(DecisionProposal {
                decision_type: DecisionType::Sell,
                symbol: quote.symbol.clone(),
                quantity: Some(position.quantity),
                order_type: Some(DecisionOrderType::Limit),
                limit_price: Some(quote.last_price),
                confidence: Decimal::ONE,
                time_horizon: Some("risk_exit".to_owned()),
                reason,
                risk_notes: vec!["engine exit policy generated this proposal".to_owned()],
            });
        }
    }
    proposals
}

fn estimated_account_value(observation: &TickObservation) -> Decimal {
    observation.snapshot.account.cash.cash
        + observation
            .snapshot
            .account
            .positions
            .iter()
            .map(|position| {
                let price = observation
                    .snapshot
                    .quotes
                    .iter()
                    .find(|quote| quote.symbol == position.symbol)
                    .map(|quote| quote.last_price)
                    .unwrap_or(position.average_price);
                Decimal::from(position.quantity.get()) * price
            })
            .sum::<Decimal>()
}

fn position_notional(account: &AccountSnapshot, quote: &Quote) -> Decimal {
    account
        .positions
        .iter()
        .find(|position| position.symbol == quote.symbol)
        .map(|position| Decimal::from(position.quantity.get()) * quote.last_price)
        .unwrap_or(Decimal::ZERO)
}

fn decision_request_for_advisor_proposal<'a>(
    run_context: &DryRunRunContext,
    observation: &'a TickObservation,
    provider: &str,
    proposal: &DecisionProposal,
) -> Result<(StrategySignal, DecisionRequest, &'a Quote), DryRunTickFailure> {
    let quote = quote_for_symbol(observation, &proposal.symbol)
        .or_else(|| observation.snapshot.quotes.first())
        .ok_or_else(|| {
            DryRunTickFailure::failed(
                "ProposalValidation",
                Some(proposal.symbol.clone()),
                "advisor proposal cannot be audited without any quote",
            )
        })?;
    let prices = price_history_for_symbol(observation, &quote.symbol)
        .unwrap_or_else(|| std::slice::from_ref(&quote.last_price));
    let signal = StrategySignal {
        strategy_id: crate::strategy::StrategyId::new(format!("advisor-{provider}")).map_err(
            |error| DryRunTickFailure::failed("ProposalValidation", None, error.to_string()),
        )?,
        symbol: proposal.symbol.clone(),
        action: decision_action_for_proposal(proposal),
        strength: proposal.confidence,
    };
    let request = DecisionRequest::for_runtime(
        run_context.mode.as_str(),
        run_context.environment,
        observation.snapshot.account_suffix.as_str(),
        DecisionPortfolioSnapshot::new(
            &observation.snapshot.account,
            &observation.snapshot.open_orders,
            &observation.snapshot.today_fills,
        ),
        quote,
        &signal,
        prices,
    );
    Ok((signal, request, quote))
}

fn quote_for_symbol<'a>(observation: &'a TickObservation, symbol: &Symbol) -> Option<&'a Quote> {
    observation
        .snapshot
        .quotes
        .iter()
        .find(|quote| quote.symbol == *symbol)
}

fn decision_failure_from_advisory_error(
    run_context: &DryRunRunContext,
    observation: &TickObservation,
    advisory: &TickAdvisory,
) -> Result<DryRunDecisionFailureAudit, DryRunTickFailure> {
    let symbol = observation
        .snapshot
        .quotes
        .first()
        .map(|quote| quote.symbol.clone())
        .ok_or_else(|| {
            DryRunTickFailure::failed(
                "TickAdvisory",
                None,
                "advisor failure cannot be audited without any quote",
            )
        })?;
    decision_failure_from_advisory_rejection(
        run_context,
        observation,
        advisory,
        advisory
            .error
            .clone()
            .unwrap_or_else(|| "advisor did not return output".to_owned()),
    )
    .map(|mut failure| {
        failure.signal.symbol = symbol;
        failure.status = advisory.status;
        failure
    })
}

fn decision_failure_from_advisory_rejection(
    run_context: &DryRunRunContext,
    observation: &TickObservation,
    advisory: &TickAdvisory,
    error: String,
) -> Result<DryRunDecisionFailureAudit, DryRunTickFailure> {
    let quote = observation.snapshot.quotes.first().ok_or_else(|| {
        DryRunTickFailure::failed(
            "ProposalValidation",
            None,
            "advisor rejection cannot be audited without any quote",
        )
    })?;
    let proposal = DecisionProposal::hold(quote.symbol.clone(), Decimal::ZERO, "advisor rejected");
    let (signal, decision_request, _) = decision_request_for_advisor_proposal(
        run_context,
        observation,
        advisory.provider.as_str(),
        &proposal,
    )?;
    Ok(DryRunDecisionFailureAudit {
        signal,
        decision_request,
        decision_provider: advisory.provider.clone(),
        raw_decision_output_json: advisory.raw_output_json.clone(),
        status: DecisionOutputStatus::Rejected,
        error,
    })
}

fn decision_failure_from_validation_error(
    signal: StrategySignal,
    decision_request: DecisionRequest,
    provider: &str,
    raw_output_json: &str,
    error: DecisionValidationError,
) -> DryRunDecisionFailureAudit {
    DryRunDecisionFailureAudit {
        signal,
        decision_request,
        decision_provider: provider.to_owned(),
        raw_decision_output_json: raw_output_json.to_owned(),
        status: DecisionOutputStatus::Rejected,
        error: error.to_string(),
    }
}

async fn build_proposal_risk_context<G, S>(
    gateway: &G,
    store: &S,
    snapshot: &DryRunSnapshot,
    quote: &Quote,
    proposal: &ValidatedDecisionProposal,
    runtime_context: RiskRuntimeContext,
) -> Result<RiskEvaluationContext, DryRunTickFailure>
where
    G: TradingGateway + ?Sized,
    S: DryRunAuditStore,
{
    let proposal_inner = proposal.proposal();
    let available_buying_power = if proposal_inner.decision_type == DecisionType::Buy {
        let buying_power = gateway
            .get_buying_power(Some(&proposal_inner.symbol))
            .await
            .map_err(|source| {
                DryRunTickFailure::snapshot_failed_from_gateway(
                    "buying_power",
                    Some(proposal_inner.symbol.clone()),
                    "buying_power",
                    source,
                )
            })?;
        Some(buying_power.available_cash)
    } else {
        None
    };
    let duplicate_order_intent = match (
        order_side_for_decision(proposal_inner.decision_type),
        proposal_inner.order_type,
    ) {
        (Some(side), Some(order_type)) => {
            let stored_duplicate = store
                .has_active_execution_intent(
                    &proposal_inner.symbol,
                    side,
                    order_type,
                    proposal_inner.limit_price,
                )
                .await
                .map_err(|error| {
                    DryRunTickFailure::failed(
                        "active_execution_intent",
                        Some(proposal_inner.symbol.clone()),
                        error.to_string(),
                    )
                })?;
            stored_duplicate
                || snapshot.open_orders.iter().any(|order| {
                    open_order_matches_intent(
                        order,
                        &proposal_inner.symbol,
                        side,
                        order_type,
                        proposal_inner.limit_price,
                    )
                })
        }
        _ => false,
    };

    let supplemental_cost_basis =
        supplemental_cost_basis_from_store(store, &snapshot.account, &snapshot.today_fills).await?;

    Ok(RiskEvaluationContext {
        market_session_open: runtime_context.market_session_open,
        unresolved_journal_commands: runtime_context.unresolved_journal_commands,
        panic_stop_active: runtime_context.panic_stop_active,
        available_buying_power,
        current_position_quantity: current_position_quantity(snapshot, &proposal_inner.symbol),
        duplicate_order_intent,
        projected_position_notional: projected_position_notional(snapshot, quote, proposal),
        submitted_order_count_today: runtime_context.submitted_order_count_today,
        realized_loss_today: DailyLossState::from_account_fills_and_cost_basis(
            &snapshot.account,
            &snapshot.today_fills,
            &supplemental_cost_basis,
        ),
    })
}

fn normalize_limit_proposal_for_market_scope(
    mut proposal: DecisionProposal,
    market_scope: &MarketScope,
) -> DecisionProposal {
    if !matches!(market_scope, MarketScope::OverseasStock { .. })
        || !proposal.is_write()
        || proposal.order_type != Some(DecisionOrderType::Limit)
    {
        return proposal;
    }
    let Some(limit_price) = proposal.limit_price else {
        return proposal;
    };
    let Some(side) = order_side_for_decision(proposal.decision_type) else {
        return proposal;
    };
    proposal.limit_price = Some(normalize_overseas_stock_limit_price(side, limit_price));
    proposal
}

async fn supplemental_cost_basis_from_store<S>(
    store: &S,
    account: &AccountSnapshot,
    fills: &[Fill],
) -> Result<Vec<(Symbol, Decimal)>, DryRunTickFailure>
where
    S: DryRunAuditStore,
{
    let mut cost_basis = Vec::new();
    for symbol in missing_sell_cost_basis_symbols(account, fills) {
        let average_price = store
            .latest_position_average_price_before(&symbol, OffsetDateTime::now_utc())
            .await
            .map_err(|error| {
                DryRunTickFailure::failed(
                    "cost_basis_lookup",
                    Some(symbol.clone()),
                    error.to_string(),
                )
            })?;
        if let Some(average_price) = average_price {
            cost_basis.push((symbol, average_price));
        }
    }
    Ok(cost_basis)
}

fn open_order_matches_intent(
    order: &OrderStatus,
    symbol: &Symbol,
    side: OrderSide,
    order_type: DecisionOrderType,
    limit_price: Option<Decimal>,
) -> bool {
    if !matches!(
        order.state,
        OrderState::Accepted | OrderState::PartiallyFilled | OrderState::Unknown(_)
    ) {
        return false;
    }

    let Some(evidence) = &order.evidence else {
        return false;
    };
    if evidence.symbol != *symbol {
        return false;
    }
    if let Some(open_side) = evidence.side
        && open_side != side
    {
        return false;
    }

    match order_type {
        DecisionOrderType::Limit => match evidence.order_price {
            Some(order_price) => Some(order_price) == limit_price,
            None => true,
        },
        DecisionOrderType::Market => true,
    }
}

fn order_side_for_decision(decision_type: DecisionType) -> Option<OrderSide> {
    match decision_type {
        DecisionType::Buy => Some(OrderSide::Buy),
        DecisionType::Sell => Some(OrderSide::Sell),
        DecisionType::Hold => None,
    }
}

fn current_position_quantity(snapshot: &DryRunSnapshot, symbol: &Symbol) -> Quantity {
    snapshot
        .account
        .positions
        .iter()
        .find(|position| position.symbol == *symbol)
        .map(|position| position.quantity)
        .unwrap_or_else(|| Quantity::new(0))
}

fn projected_position_notional(
    snapshot: &DryRunSnapshot,
    quote: &Quote,
    proposal: &ValidatedDecisionProposal,
) -> Option<Decimal> {
    let proposal = proposal.proposal();
    if !proposal.is_write() {
        return None;
    }
    let current_notional = snapshot
        .account
        .positions
        .iter()
        .find(|position| position.symbol == proposal.symbol)
        .map(|position| Decimal::from(position.quantity.get()) * quote.last_price)
        .unwrap_or(Decimal::ZERO);
    let order_notional = proposal.order_notional(Some(quote.last_price))?;
    match proposal.decision_type {
        DecisionType::Buy => Some(current_notional + order_notional),
        DecisionType::Sell => Some((current_notional - order_notional).max(Decimal::ZERO)),
        DecisionType::Hold => None,
    }
}

fn render_dry_run_tick(run: &DryRunTickOutput, mode: RuntimeMode) -> String {
    let mut output = StructuredOutput::new();
    output
        .heading(mode.snapshot_heading())
        .field("mode", mode.as_str())
        .field("account_suffix", run.snapshot.account_suffix.as_str())
        .field("cash", run.snapshot.account.cash.cash)
        .field("positions", run.snapshot.account.positions.len())
        .field("quotes", run.snapshot.all_quotes().len())
        .blank()
        .heading("Quotes");

    let quotes = run.snapshot.all_quotes();
    if quotes.is_empty() {
        output.empty_item();
    }
    for quote in quotes {
        let mut fields = Vec::new();
        if let Some(full_name) = quote.full_name.as_deref() {
            fields.push(OutputField::new("full_name", full_name));
        }
        fields.push(OutputField::new("last_price", quote.last_price));
        output.item(&quote.symbol, &fields);
    }
    if !run.technical_analysis.is_empty() {
        output.blank().heading("Technical analysis");
        for evidence in &run.technical_analysis {
            output.item(
                &evidence.symbol,
                &[
                    OutputField::new("interval", evidence.interval.as_str()),
                    OutputField::new("feature_set", evidence.feature_set.as_str()),
                    OutputField::new(
                        "sma_short",
                        optional_decimal_display(evidence.features.sma_short),
                    ),
                    OutputField::new(
                        "volume_breakout",
                        optional_bool_display(evidence.features.volume_breakout),
                    ),
                    OutputField::new("audit_id", evidence.audit_id),
                ],
            );
        }
    }
    if let Some(alignment) = &run.domestic_market_alignment {
        output
            .blank()
            .heading("Domestic market alignment")
            .field("status", alignment.status.as_str())
            .field("mode", alignment.mode.as_str())
            .field(
                "benchmark_symbol",
                alignment
                    .benchmark_symbol
                    .as_ref()
                    .map(Symbol::as_str)
                    .unwrap_or("none"),
            )
            .field("benchmark_regime", alignment.benchmark_regime.as_str());
        for candidate in &alignment.candidates {
            output.item(
                &candidate.symbol,
                &[
                    OutputField::new("label", candidate.alignment_label.as_str()),
                    OutputField::new("symbol_roc", optional_decimal_display(candidate.symbol_roc)),
                    OutputField::new(
                        "benchmark_roc",
                        optional_decimal_display(candidate.benchmark_roc),
                    ),
                ],
            );
        }
    }
    output.blank();

    let mut rendered = output.finish();
    if let Some(discovery) = &run.universe_discovery {
        write_universe_discovery_result(&mut rendered, discovery);
    }
    write_tick_advisory_result(
        &mut rendered,
        TickAdvisoryResultDisplay {
            tick_index: run.advisory.tick_sequence + 1,
            status: run.advisory.status.as_str(),
            provider: run.advisory.provider.as_str(),
            audit_action: if run.advisory.status == DecisionOutputStatus::Parsed {
                "tick_advisory_completed"
            } else {
                "tick_advisory_failed"
            },
            run_id: run.advisory.run_id,
            advisor_output: run.advisory.output.as_ref().map(advisor_output_display),
            error: run.advisory.error.as_deref(),
        },
    );

    let mut output = StructuredOutput::new();
    output.heading("Decisions");

    if run.decisions.is_empty() {
        output.empty_item();
    }
    for decision in &run.decisions {
        let proposal = decision.proposal.proposal();
        output.item(
            &proposal.symbol,
            &[
                OutputField::new("strategy", decision.signal.strategy_id.as_str()),
                OutputField::new("signal", decision.signal.action.as_str()),
                OutputField::new("strength", decision.signal.strength),
                OutputField::new("decision", proposal.decision_type.as_str()),
                OutputField::new("confidence", proposal.confidence),
                OutputField::new("risk", decision.risk.status.as_str()),
            ],
        );
    }
    output.blank();

    if !run.decision_failures.is_empty() {
        output.heading("Decision failures");
    }
    for failure in &run.decision_failures {
        output.item(
            &failure.signal.symbol,
            &[
                OutputField::new("strategy", failure.signal.strategy_id.as_str()),
                OutputField::new("signal", failure.signal.action.as_str()),
                OutputField::new("strength", failure.signal.strength),
                OutputField::new("status", failure.status.as_str()),
                OutputField::new("provider", failure.decision_provider.as_str()),
                OutputField::new("error", failure.error.as_str()),
            ],
        );
    }
    if !run.decision_failures.is_empty() {
        output.blank();
    }
    write_positions_section(&mut output, run);
    rendered.push_str(&output.finish());
    rendered
}

fn write_positions_section(output: &mut StructuredOutput, run: &DryRunTickOutput) {
    output.heading("Positions");
    if run.snapshot.account.positions.is_empty() {
        output.empty_item();
        return;
    }
    for position in &run.snapshot.account.positions {
        let mut fields = vec![
            OutputField::new("quantity", position.quantity.get()),
            OutputField::new("average_price", position.average_price),
        ];
        if let Some(quote) = run.snapshot.quote_for_position(&position.symbol) {
            fields.push(OutputField::new("current_price", quote.last_price));
            let return_pct = unrealized_return_pct(position.average_price, quote.last_price)
                .map_or_else(|| "unavailable".to_owned(), |value| value.to_string());
            fields.push(OutputField::new("unrealized_return_pct", return_pct));
        } else {
            fields.push(OutputField::new("current_price", "unavailable"));
            fields.push(OutputField::new("unrealized_return_pct", "unavailable"));
        }
        output.item(&position.symbol, &fields);
    }
}

fn unrealized_return_pct(average_price: Decimal, current_price: Decimal) -> Option<Decimal> {
    if average_price <= Decimal::ZERO {
        return None;
    }
    Some(((current_price - average_price) / average_price * Decimal::new(100, 0)).round_dp(2))
}

fn optional_decimal_display(value: Option<Decimal>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |value| value.to_string())
}

fn optional_bool_display(value: Option<bool>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovery_candidate(
        symbol: &str,
        bucket: Option<UniverseDiscoveryBucket>,
        target_support: Option<crate::universe::UniverseDiscoveryTargetSupport>,
        signals: Vec<UniverseDiscoverySignal>,
        risk_flags: Vec<crate::universe::UniverseDiscoveryRiskFlag>,
        evidence: Vec<crate::universe::UniverseDiscoveryEvidence>,
        confidence: Decimal,
    ) -> UniverseDiscoveryCandidate {
        UniverseDiscoveryCandidate {
            symbol: Symbol::new(symbol).expect("valid symbol"),
            bucket,
            target_support,
            signals,
            evidence,
            risk_flags,
            reason: format!("{symbol} fixture"),
            confidence,
        }
    }

    fn advisor_policy_signature(symbol: &str, last_price: Decimal) -> AdvisorInvocationSignature {
        let symbol = Symbol::new(symbol).expect("valid symbol");
        AdvisorInvocationSignature {
            cash: Decimal::new(1_000_000, 0),
            positions: Vec::new(),
            open_orders: Vec::new(),
            today_fills: Vec::new(),
            domestic_market_alignment: None,
            effective_candidates: vec![symbol.clone()],
            quotes: vec![AdvisorInvocationQuote { symbol, last_price }],
        }
    }

    #[test]
    fn advisor_invocation_policy_skips_unchanged_state_inside_min_interval() {
        let policy = AdvisorInvocationPolicy {
            min_interval: Duration::from_secs(300),
            price_change_threshold_bps: 50,
        };
        let signature = advisor_policy_signature("005930", Decimal::new(73_000, 0));
        let mut state = AdvisorInvocationState::default();
        state.record_command_invocation(Instant::now(), signature.clone());

        let decision = state.evaluate_policy(&policy, Instant::now(), &signature);

        assert!(matches!(
            decision,
            AdvisorInvocationDecision::Skip {
                reason: AdvisorInvocationSkipReason::MinIntervalActive,
                ..
            }
        ));
    }

    #[test]
    fn advisor_invocation_policy_invokes_on_material_price_change_inside_min_interval() {
        let policy = AdvisorInvocationPolicy {
            min_interval: Duration::from_secs(300),
            price_change_threshold_bps: 50,
        };
        let mut state = AdvisorInvocationState::default();
        state.record_command_invocation(
            Instant::now(),
            advisor_policy_signature("005930", Decimal::new(73_000, 0)),
        );
        let moved = advisor_policy_signature("005930", Decimal::new(73_400, 0));

        let decision = state.evaluate_policy(&policy, Instant::now(), &moved);

        assert!(matches!(
            decision,
            AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::MaterialPriceChange
            }
        ));
    }

    #[test]
    fn advisor_invocation_policy_invokes_on_candidate_change_inside_min_interval() {
        let policy = AdvisorInvocationPolicy {
            min_interval: Duration::from_secs(300),
            price_change_threshold_bps: 50,
        };
        let mut state = AdvisorInvocationState::default();
        state.record_command_invocation(
            Instant::now(),
            advisor_policy_signature("005930", Decimal::new(73_000, 0)),
        );
        let changed = advisor_policy_signature("000660", Decimal::new(120_000, 0));

        let decision = state.evaluate_policy(&policy, Instant::now(), &changed);

        assert!(matches!(
            decision,
            AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::CandidateSetChanged
            }
        ));
    }

    #[test]
    fn advisor_invocation_policy_invokes_on_alignment_context_change_inside_min_interval() {
        let policy = AdvisorInvocationPolicy {
            min_interval: Duration::from_secs(300),
            price_change_threshold_bps: 50,
        };
        let mut state = AdvisorInvocationState::default();
        let mut previous = advisor_policy_signature("005930", Decimal::new(73_000, 0));
        previous.domestic_market_alignment = Some("aligned".to_owned());
        state.record_command_invocation(Instant::now(), previous);
        let mut changed = advisor_policy_signature("005930", Decimal::new(73_000, 0));
        changed.domestic_market_alignment = Some("weak".to_owned());

        let decision = state.evaluate_policy(&policy, Instant::now(), &changed);

        assert!(matches!(
            decision,
            AdvisorInvocationDecision::Invoke {
                reason: AdvisorInvocationReason::AdvisorContextChanged
            }
        ));
    }

    #[test]
    fn advisor_candidate_selection_prefers_target_price_gap_with_momentum() {
        let selected = select_advisor_candidate_symbols(
            &[],
            &[
                discovery_candidate(
                    "035420",
                    Some(UniverseDiscoveryBucket::Liquidity),
                    None,
                    vec![UniverseDiscoverySignal::TradingValueLeader],
                    Vec::new(),
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "trading value rank".to_owned(),
                        value: "top decile".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: Some("universe snapshot".to_owned()),
                    }],
                    Decimal::new(78, 2),
                ),
                discovery_candidate(
                    "000660",
                    Some(UniverseDiscoveryBucket::TargetPriceGap),
                    Some(crate::universe::UniverseDiscoveryTargetSupport {
                        source_count: Some(3),
                        consensus: Some(true),
                        freshness: Some(UniverseDiscoveryTargetFreshness::Recent),
                    }),
                    vec![
                        UniverseDiscoverySignal::ConsensusTargetUpside,
                        UniverseDiscoverySignal::RelativeStrength,
                        UniverseDiscoverySignal::TargetPriceRevision,
                    ],
                    Vec::new(),
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "broker target price gap".to_owned(),
                        value: "roughly 28% upside versus consensus target price".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: Some("compiled broker notes".to_owned()),
                    }],
                    Decimal::new(64, 2),
                ),
            ],
            &[
                Quote {
                    symbol: Symbol::new("035420").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(250_000, 0),
                },
                Quote {
                    symbol: Symbol::new("000660").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(190_000, 0),
                },
            ],
            1,
        );

        assert_eq!(selected, vec![Symbol::new("000660").expect("valid symbol")]);
    }

    #[test]
    fn advisor_candidate_selection_does_not_boost_weak_target_price_support() {
        let selected = select_advisor_candidate_symbols(
            &[],
            &[
                discovery_candidate(
                    "035420",
                    Some(UniverseDiscoveryBucket::Liquidity),
                    None,
                    vec![UniverseDiscoverySignal::TradingValueLeader],
                    Vec::new(),
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "trading value rank".to_owned(),
                        value: "top decile".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: Some("universe snapshot".to_owned()),
                    }],
                    Decimal::new(78, 2),
                ),
                discovery_candidate(
                    "000660",
                    Some(UniverseDiscoveryBucket::TargetPriceGap),
                    Some(crate::universe::UniverseDiscoveryTargetSupport {
                        source_count: Some(1),
                        consensus: Some(false),
                        freshness: Some(UniverseDiscoveryTargetFreshness::Recent),
                    }),
                    vec![UniverseDiscoverySignal::ConsensusTargetUpside],
                    vec![crate::universe::UniverseDiscoveryRiskFlag::SingleBrokerTarget],
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "broker target price gap".to_owned(),
                        value: "roughly 28% upside versus single broker target".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: Some("single broker note".to_owned()),
                    }],
                    Decimal::new(79, 2),
                ),
            ],
            &[
                Quote {
                    symbol: Symbol::new("035420").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(250_000, 0),
                },
                Quote {
                    symbol: Symbol::new("000660").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(190_000, 0),
                },
            ],
            1,
        );

        assert_eq!(selected, vec![Symbol::new("035420").expect("valid symbol")]);
    }

    #[test]
    fn advisor_candidate_selection_does_not_boost_unverified_target_price_support() {
        let selected = select_advisor_candidate_symbols(
            &[],
            &[
                discovery_candidate(
                    "035420",
                    Some(UniverseDiscoveryBucket::Liquidity),
                    None,
                    vec![UniverseDiscoverySignal::TradingValueLeader],
                    Vec::new(),
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "trading value rank".to_owned(),
                        value: "top decile".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: Some("universe snapshot".to_owned()),
                    }],
                    Decimal::new(78, 2),
                ),
                discovery_candidate(
                    "000660",
                    Some(UniverseDiscoveryBucket::TargetPriceGap),
                    Some(crate::universe::UniverseDiscoveryTargetSupport {
                        source_count: Some(2),
                        consensus: Some(true),
                        freshness: Some(UniverseDiscoveryTargetFreshness::Recent),
                    }),
                    vec![UniverseDiscoverySignal::ConsensusTargetUpside],
                    vec![crate::universe::UniverseDiscoveryRiskFlag::UnverifiedSource],
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "broker target price gap".to_owned(),
                        value: "roughly 28% upside versus unsourced target".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: None,
                    }],
                    Decimal::new(79, 2),
                ),
            ],
            &[
                Quote {
                    symbol: Symbol::new("035420").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(250_000, 0),
                },
                Quote {
                    symbol: Symbol::new("000660").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(190_000, 0),
                },
            ],
            1,
        );

        assert_eq!(selected, vec![Symbol::new("035420").expect("valid symbol")]);
    }

    #[test]
    fn advisor_candidate_selection_keeps_weak_target_gap_when_fresh_momentum_confirms_it() {
        let selected = select_advisor_candidate_symbols(
            &[],
            &[
                discovery_candidate(
                    "035420",
                    Some(UniverseDiscoveryBucket::ShortTermMomentum),
                    None,
                    vec![UniverseDiscoverySignal::RelativeStrength],
                    Vec::new(),
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "recent price momentum".to_owned(),
                        value: "stronger recent closes".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: Some("quote/recent prices".to_owned()),
                    }],
                    Decimal::new(80, 2),
                ),
                discovery_candidate(
                    "000660",
                    Some(UniverseDiscoveryBucket::TargetPriceGap),
                    Some(crate::universe::UniverseDiscoveryTargetSupport {
                        source_count: Some(1),
                        consensus: Some(false),
                        freshness: Some(UniverseDiscoveryTargetFreshness::Fresh),
                    }),
                    vec![
                        UniverseDiscoverySignal::ConsensusTargetUpside,
                        UniverseDiscoverySignal::VolumeBreakout,
                        UniverseDiscoverySignal::NewsCatalyst,
                    ],
                    vec![crate::universe::UniverseDiscoveryRiskFlag::SingleBrokerTarget],
                    vec![
                        crate::universe::UniverseDiscoveryEvidence {
                            metric: "broker target price gap".to_owned(),
                            value: "roughly 30% upside versus a recent broker target".to_owned(),
                            as_of: Some("latest broker note".to_owned()),
                            source: Some("broker note".to_owned()),
                        },
                        crate::universe::UniverseDiscoveryEvidence {
                            metric: "recent price momentum".to_owned(),
                            value: "volume breakout after catalyst".to_owned(),
                            as_of: Some("latest available tick".to_owned()),
                            source: Some("quote/recent prices".to_owned()),
                        },
                    ],
                    Decimal::new(68, 2),
                ),
            ],
            &[
                Quote {
                    symbol: Symbol::new("035420").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(250_000, 0),
                },
                Quote {
                    symbol: Symbol::new("000660").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(190_000, 0),
                },
            ],
            1,
        );

        assert_eq!(selected, vec![Symbol::new("000660").expect("valid symbol")]);
    }

    #[test]
    fn advisor_candidate_selection_does_not_boost_stale_target_price_support() {
        let selected = select_advisor_candidate_symbols(
            &[],
            &[
                discovery_candidate(
                    "035420",
                    Some(UniverseDiscoveryBucket::Liquidity),
                    None,
                    vec![UniverseDiscoverySignal::TradingValueLeader],
                    Vec::new(),
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "trading value rank".to_owned(),
                        value: "top decile".to_owned(),
                        as_of: Some("latest available tick".to_owned()),
                        source: Some("universe snapshot".to_owned()),
                    }],
                    Decimal::new(78, 2),
                ),
                discovery_candidate(
                    "000660",
                    Some(UniverseDiscoveryBucket::TargetPriceGap),
                    Some(crate::universe::UniverseDiscoveryTargetSupport {
                        source_count: Some(2),
                        consensus: Some(true),
                        freshness: Some(UniverseDiscoveryTargetFreshness::Stale),
                    }),
                    vec![UniverseDiscoverySignal::ConsensusTargetUpside],
                    vec![crate::universe::UniverseDiscoveryRiskFlag::StaleTargetPrice],
                    vec![crate::universe::UniverseDiscoveryEvidence {
                        metric: "broker target price gap".to_owned(),
                        value: "roughly 28% upside versus old target".to_owned(),
                        as_of: Some("older broker note".to_owned()),
                        source: Some("broker note archive".to_owned()),
                    }],
                    Decimal::new(79, 2),
                ),
            ],
            &[
                Quote {
                    symbol: Symbol::new("035420").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(250_000, 0),
                },
                Quote {
                    symbol: Symbol::new("000660").expect("valid symbol"),
                    full_name: None,
                    last_price: Decimal::new(190_000, 0),
                },
            ],
            1,
        );

        assert_eq!(selected, vec![Symbol::new("035420").expect("valid symbol")]);
    }
}
