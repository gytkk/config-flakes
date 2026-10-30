use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rust_decimal::Decimal;
use serde::Deserialize;
use trading_agent_kis_client::config::BrokerRuntimeConfig;
use trading_agent_kis_client::domain::{
    AccountKey, BrokerEnvironment, CurrencyCode, ExchangeCode, Symbol, UsEquityOrderSession,
};
use trading_agent_kis_client::{BrokerError, KisCredential};
use trading_agent_toss_client::{TossAccountSeq, TossCredential};

use crate::error::TradeEngineError;
use crate::gateway::types::MarketScope;
use crate::market::MarketProfile;
use crate::market_alignment::{
    DEFAULT_DOMESTIC_ALIGNMENT_FEATURE_SET, DEFAULT_RELATIVE_STRENGTH_LOOKBACK_DAYS,
};

pub use crate::market_alignment::DomesticMarketAlignmentMode;

const DEFAULT_APP_KEY_ENV: &str = "KIS_APP_KEY";
const DEFAULT_APP_SECRET_ENV: &str = "KIS_APP_SECRET";
const DEFAULT_ACCOUNT_KEY_ENV: &str = "KIS_ACCOUNT_KEY";
const DEFAULT_KIS_TOKEN_CACHE_PATH: &str = ".local/state/trade-engine/kis-live-token.json";
const DEFAULT_TOSS_CLIENT_ID_ENV: &str = "TOSS_CLIENT_ID";
const DEFAULT_TOSS_CLIENT_SECRET_ENV: &str = "TOSS_CLIENT_SECRET";
const DEFAULT_TOSS_ACCOUNT_SEQ_ENV: &str = "TOSS_ACCOUNT_SEQ";
const DEFAULT_TOSS_BASE_URL: &str = "https://openapi.tossinvest.com";

const DEFAULT_MARKET_TICK_SECONDS: u64 = 30;
const DEFAULT_OFF_HOURS_MARKET_TICK_SECONDS: u64 = 900;
const DEFAULT_MAX_TICK_RUNTIME_SECONDS: u64 = 10;
const DEFAULT_SKIP_IF_PREVIOUS_TICK_RUNNING: bool = true;
const DEFAULT_MARKET_SESSION_TTL_SECONDS: u64 = 300;
const DEFAULT_DECISION_TIMEOUT_SECONDS: u64 = 15;
const DEFAULT_DECISION_MAX_STDERR_BYTES: usize = 4_096;
const DEFAULT_ADVISOR_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_ADVISOR_MAX_STDERR_BYTES: usize = 4_096;
const DEFAULT_ADVISOR_MAX_STDOUT_BYTES: usize = 262_144;
const DEFAULT_ADVISOR_RECENT_PRICE_COUNT: usize = 20;
const DEFAULT_ADVISOR_CANDIDATE_LIMIT: usize = 8;
const DEFAULT_ADVISOR_MIN_COMMAND_INTERVAL_SECONDS: u64 = 600;
const DEFAULT_ADVISOR_PRICE_CHANGE_THRESHOLD_BPS: u32 = 75;
const DEFAULT_UNIVERSE_DISCOVERY_MAX_SYMBOLS: usize = 20;
const DEFAULT_UNIVERSE_DISCOVERY_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_UNIVERSE_DISCOVERY_MAX_STDERR_BYTES: usize = 4_096;
const DEFAULT_UNIVERSE_DISCOVERY_MAX_STDOUT_BYTES: usize = 262_144;
const DEFAULT_UNIVERSE_DISCOVERY_REFRESH_SECONDS: u64 = 10_800;
const DEFAULT_UNIVERSE_DISCOVERY_AUTO_REFRESH: bool = true;
const DEFAULT_MAX_ORDER_NOTIONAL: Decimal = Decimal::from_parts(1_000_000, 0, 0, false, 0);
const DEFAULT_MAX_POSITION_NOTIONAL: Decimal = Decimal::from_parts(5_000_000, 0, 0, false, 0);
const DEFAULT_MAX_DAILY_LOSS: Decimal = Decimal::from_parts(1_000_000, 0, 0, false, 0);
const DEFAULT_MAX_DAILY_ORDER_COUNT: u32 = 3;
const DEFAULT_TARGET_ACCOUNT_FRACTION: Decimal = Decimal::from_parts(10, 0, 0, false, 2);
const DEFAULT_STOP_LOSS_PCT: Decimal = Decimal::from_parts(5, 0, 0, false, 2);
const DEFAULT_TAKE_PROFIT_PCT: Decimal = Decimal::from_parts(10, 0, 0, false, 2);
const ADVISOR_TIMEOUT_TICK_RUNTIME_BUFFER_SECONDS: u64 = 30;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EngineConfig {
    pub read_only: bool,
    pub watchlist: Vec<String>,
    #[serde(default)]
    pub broker: BrokerConfig,
    pub market: MarketConfig,
    #[serde(default)]
    pub kis: KisConfig,
    #[serde(default)]
    pub toss: TossConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    pub strategy: StrategyConfig,
    #[serde(default)]
    pub decision: DecisionConfig,
    #[serde(default)]
    pub advisor: AdvisorConfig,
    #[serde(default)]
    pub universe_discovery: UniverseDiscoveryConfig,
    #[serde(default)]
    pub domestic_market_alignment: DomesticMarketAlignmentConfig,
    #[serde(default)]
    pub sizing: SizingConfig,
    #[serde(default)]
    pub exit_policy: ExitPolicyConfig,
    pub risk: RiskConfig,
    #[serde(default)]
    pub live: LiveConfig,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct BrokerConfig {
    pub provider: BrokerProviderKind,
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            provider: BrokerProviderKind::Kis,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrokerProviderKind {
    Kis,
    Toss,
}

impl BrokerProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kis => "kis",
            Self::Toss => "toss",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq, Eq)]
pub struct StorageConfig {
    #[serde(default)]
    pub main_db_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct RuntimeConfig {
    pub market_tick_seconds: u64,
    pub off_hours_market_tick_seconds: u64,
    pub max_tick_runtime_seconds: u64,
    pub skip_if_previous_tick_running: bool,
    pub market_session_ttl_seconds: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            market_tick_seconds: DEFAULT_MARKET_TICK_SECONDS,
            off_hours_market_tick_seconds: DEFAULT_OFF_HOURS_MARKET_TICK_SECONDS,
            max_tick_runtime_seconds: DEFAULT_MAX_TICK_RUNTIME_SECONDS,
            skip_if_previous_tick_running: DEFAULT_SKIP_IF_PREVIOUS_TICK_RUNNING,
            market_session_ttl_seconds: DEFAULT_MARKET_SESSION_TTL_SECONDS,
        }
    }
}

impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        validate_positive_seconds("runtime.market_tick_seconds", self.market_tick_seconds)?;
        validate_positive_seconds(
            "runtime.off_hours_market_tick_seconds",
            self.off_hours_market_tick_seconds,
        )?;
        validate_positive_seconds(
            "runtime.max_tick_runtime_seconds",
            self.max_tick_runtime_seconds,
        )?;
        validate_positive_seconds(
            "runtime.market_session_ttl_seconds",
            self.market_session_ttl_seconds,
        )?;
        Ok(())
    }

    pub fn max_tick_runtime(&self) -> Duration {
        Duration::from_secs(self.max_tick_runtime_seconds)
    }

    pub fn market_session_ttl(&self) -> Duration {
        Duration::from_secs(self.market_session_ttl_seconds)
    }
}

impl EngineConfig {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, TradeEngineError> {
        let path = path.as_ref();
        let body =
            std::fs::read_to_string(path).map_err(|source| TradeEngineError::ConfigRead {
                path: path.to_path_buf(),
                source,
            })?;
        toml::from_str(&body).map_err(|source| TradeEngineError::ConfigParse {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn validate_read_only_smoke(&self) -> Result<(), TradeEngineError> {
        if !self.read_only {
            return Err(TradeEngineError::config(
                "read-only smoke requires read_only=true",
            ));
        }
        if self.watchlist.is_empty() {
            return Err(TradeEngineError::config(
                "read-only smoke requires at least one watchlist symbol",
            ));
        }
        self.validate_common()?;
        self.main_db_path()?;
        if self.risk.enable_live_trading {
            return Err(TradeEngineError::config(
                "risk.enable_live_trading must be false when read_only=true",
            ));
        }
        Ok(())
    }

    pub fn validate_live_write_safety(&self) -> Result<(), TradeEngineError> {
        self.validate_common()?;
        self.main_db_path()?;
        self.validate_live_write_guards()
    }

    pub fn validate_write_runtime_safety(&self) -> Result<(), TradeEngineError> {
        self.validate_common()?;
        self.main_db_path()?;
        if self.broker.provider == BrokerProviderKind::Toss {
            return Err(TradeEngineError::config(
                "Toss broker provider is read-only; order submission runtime is disabled",
            ));
        }
        if self.read_only {
            return Err(TradeEngineError::config(
                "order submission runtime requires read_only=false",
            ));
        }
        if !self.risk.enable_live_trading {
            return Err(TradeEngineError::config(
                "order submission runtime requires risk.enable_live_trading=true",
            ));
        }
        self.validate_live_write_guards()
    }

    pub fn validate_universe_refresh(&self) -> Result<(), TradeEngineError> {
        self.validate_common()?;
        self.main_db_path()?;
        if self.watchlist.is_empty() {
            return Err(TradeEngineError::config(
                "universe refresh requires at least one watchlist symbol",
            ));
        }
        if self.universe_discovery.provider == UniverseDiscoveryProviderKind::Disabled {
            return Err(TradeEngineError::config(
                "universe refresh requires universe_discovery.provider=command",
            ));
        }
        Ok(())
    }

    pub fn broker_environment(&self) -> BrokerEnvironment {
        match self.broker.provider {
            BrokerProviderKind::Kis => self.kis.environment.as_broker_environment(),
            BrokerProviderKind::Toss => BrokerEnvironment::Live,
        }
    }

    pub fn validate_backtest(&self) -> Result<(), TradeEngineError> {
        if !self.read_only {
            return Err(TradeEngineError::config("backtest requires read_only=true"));
        }
        self.validate_common()?;
        self.main_db_path()?;
        if self.watchlist.is_empty() {
            return Err(TradeEngineError::config(
                "backtest requires at least one watchlist symbol",
            ));
        }
        if self.risk.enable_live_trading {
            return Err(TradeEngineError::config(
                "risk.enable_live_trading must be false for backtest",
            ));
        }
        if self.strategy.id != "sma-crossover-v1" {
            return Err(TradeEngineError::config(
                "backtest currently supports strategy.id=sma-crossover-v1",
            ));
        }
        Ok(())
    }

    fn validate_live_write_guards(&self) -> Result<(), TradeEngineError> {
        if self.broker.provider == BrokerProviderKind::Toss {
            return Err(TradeEngineError::config(
                "Toss broker provider is read-only; live writes are disabled",
            ));
        }
        if self.kis.environment != KisEnvironment::Live {
            return Ok(());
        }
        if self.read_only {
            return Err(TradeEngineError::config(
                "live writes require read_only=false",
            ));
        }
        if !self.risk.enable_live_trading {
            return Err(TradeEngineError::config(
                "live writes require risk.enable_live_trading=true",
            ));
        }
        Ok(())
    }

    fn validate_common(&self) -> Result<(), TradeEngineError> {
        self.market.validate()?;
        self.watchlist_symbols()?;
        match self.broker.provider {
            BrokerProviderKind::Kis => self.kis.validate()?,
            BrokerProviderKind::Toss => self.toss.validate()?,
        }
        self.runtime.validate()?;
        self.strategy.validate()?;
        self.decision.validate()?;
        self.advisor.validate()?;
        self.universe_discovery.validate()?;
        self.domestic_market_alignment.validate()?;
        self.sizing.validate()?;
        self.exit_policy.validate()?;
        self.risk.validate()?;
        self.validate_runtime_provider_timeouts()?;

        Ok(())
    }

    fn validate_runtime_provider_timeouts(&self) -> Result<(), TradeEngineError> {
        if self.advisor.provider != AdvisorProviderKind::Command {
            return Ok(());
        }
        let minimum_tick_runtime = self
            .advisor
            .timeout_seconds
            .saturating_add(ADVISOR_TIMEOUT_TICK_RUNTIME_BUFFER_SECONDS);
        if self.runtime.max_tick_runtime_seconds < minimum_tick_runtime {
            return Err(TradeEngineError::config(format!(
                "runtime.max_tick_runtime_seconds must be at least advisor.timeout_seconds + {} seconds when advisor.provider=command",
                ADVISOR_TIMEOUT_TICK_RUNTIME_BUFFER_SECONDS
            )));
        }
        Ok(())
    }

    pub fn watchlist_symbols(&self) -> Result<Vec<Symbol>, TradeEngineError> {
        let market_profile = MarketProfile::new(self.market.scope()?);
        let symbols = self
            .watchlist
            .iter()
            .map(|symbol| {
                Symbol::new(symbol.clone()).map_err(|source| {
                    TradeEngineError::invalid_broker_value("watchlist symbol", source)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for symbol in &symbols {
            validate_symbol_matches_market("watchlist symbol", symbol, &market_profile)?;
        }
        Ok(symbols)
    }

    pub fn main_db_path(&self) -> Result<&Path, TradeEngineError> {
        self.storage
            .main_db_path
            .as_deref()
            .ok_or_else(|| TradeEngineError::config("storage.main_db_path is required"))
    }

    pub fn account_suffix_from_account_key(account_key: &str) -> String {
        let account_number = account_key
            .split_once('-')
            .map(|(account_number, _)| account_number)
            .unwrap_or(account_key);
        let mut suffix = account_number.chars().rev().take(4).collect::<Vec<char>>();
        suffix.reverse();
        suffix.into_iter().collect()
    }

    pub fn decision_source_id(&self) -> Result<String, TradeEngineError> {
        decision_source_id(&self.advisor)
    }
}

pub fn decision_source_id(advisor: &AdvisorConfig) -> Result<String, TradeEngineError> {
    advisor.validate()?;
    let provider_id = match advisor.provider {
        AdvisorProviderKind::Disabled => "deterministic_sma",
        AdvisorProviderKind::Command => "command",
    };
    Ok(format!("advisor-{provider_id}"))
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MarketConfig {
    pub kind: MarketKind,
    pub exchange_code: Option<String>,
    pub currency: Option<String>,
    #[serde(default)]
    pub allowed_order_sessions: Option<Vec<UsEquityOrderSession>>,
}

impl Default for MarketConfig {
    fn default() -> Self {
        Self {
            kind: MarketKind::DomesticStock,
            exchange_code: None,
            currency: None,
            allowed_order_sessions: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketKind {
    DomesticStock,
    OverseasStock,
}

impl MarketKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DomesticStock => "domestic_stock",
            Self::OverseasStock => "overseas_stock",
        }
    }
}

impl MarketConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        match self.kind {
            MarketKind::DomesticStock => {
                if self.exchange_code.is_some()
                    || self.currency.is_some()
                    || self.allowed_order_sessions.is_some()
                {
                    return Err(TradeEngineError::config(
                        "domestic_stock market must not set exchange_code, currency, or allowed_order_sessions",
                    ));
                }
            }
            MarketKind::OverseasStock => {
                self.overseas_exchange()?;
                self.overseas_currency()?;
            }
        }
        Ok(())
    }

    pub fn scope(&self) -> Result<MarketScope, TradeEngineError> {
        self.validate()?;
        match self.kind {
            MarketKind::DomesticStock => Ok(MarketScope::DomesticStock),
            MarketKind::OverseasStock => Ok(MarketScope::OverseasStock {
                exchange: self.overseas_exchange()?,
                currency: self.overseas_currency()?,
            }),
        }
    }

    fn overseas_exchange(&self) -> Result<ExchangeCode, TradeEngineError> {
        let value = self.exchange_code.as_deref().ok_or_else(|| {
            TradeEngineError::config("market.exchange_code is required for overseas_stock")
        })?;
        ExchangeCode::new(value).map_err(|source| {
            TradeEngineError::invalid_broker_value("market.exchange_code", source)
        })
    }

    fn overseas_currency(&self) -> Result<CurrencyCode, TradeEngineError> {
        let value = self.currency.as_deref().ok_or_else(|| {
            TradeEngineError::config("market.currency is required for overseas_stock")
        })?;
        CurrencyCode::new(value)
            .map_err(|source| TradeEngineError::invalid_broker_value("market.currency", source))
    }

    pub fn allowed_us_equity_order_sessions(
        &self,
    ) -> Result<Vec<UsEquityOrderSession>, TradeEngineError> {
        self.validate()?;
        if self.kind != MarketKind::OverseasStock {
            return Ok(BrokerRuntimeConfig::regular_us_equity_order_sessions());
        }
        Ok(self
            .allowed_order_sessions
            .clone()
            .unwrap_or_else(BrokerRuntimeConfig::regular_us_equity_order_sessions))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KisConfig {
    pub environment: KisEnvironment,
    pub token_cache_path: PathBuf,
    #[serde(default = "default_app_key_env")]
    pub app_key_env: String,
    #[serde(default = "default_app_secret_env")]
    pub app_secret_env: String,
    #[serde(default)]
    pub account_key: Option<String>,
    #[serde(default = "default_account_key_env")]
    pub account_key_env: String,
}

impl Default for KisConfig {
    fn default() -> Self {
        Self {
            environment: KisEnvironment::Live,
            token_cache_path: PathBuf::from(DEFAULT_KIS_TOKEN_CACHE_PATH),
            app_key_env: DEFAULT_APP_KEY_ENV.to_owned(),
            app_secret_env: DEFAULT_APP_SECRET_ENV.to_owned(),
            account_key: None,
            account_key_env: DEFAULT_ACCOUNT_KEY_ENV.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KisEnvironment {
    Paper,
    Live,
}

impl KisEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Live => "live",
        }
    }

    pub fn as_broker_environment(self) -> BrokerEnvironment {
        match self {
            Self::Paper => BrokerEnvironment::Paper,
            Self::Live => BrokerEnvironment::Live,
        }
    }
}

impl KisConfig {
    fn validate(&self) -> Result<(), TradeEngineError> {
        if self.token_cache_path.as_os_str().is_empty() {
            return Err(TradeEngineError::config(
                "kis.token_cache_path must not be empty",
            ));
        }
        validate_env_var_name("kis.app_key_env", &self.app_key_env)?;
        validate_env_var_name("kis.app_secret_env", &self.app_secret_env)?;
        validate_env_var_name("kis.account_key_env", &self.account_key_env)?;
        if let Some(account_key) = &self.account_key {
            parse_account_key("kis.account_key", account_key)?;
        }
        Ok(())
    }

    pub fn credentials_from_env(&self) -> Result<KisCredential, TradeEngineError> {
        let app_key = required_env(&self.app_key_env)?;
        let app_secret = required_env(&self.app_secret_env)?;
        KisCredential::new(app_key, app_secret).map_err(TradeEngineError::from)
    }

    pub fn account_key_from_env(&self) -> Result<AccountKey, TradeEngineError> {
        let account_key = required_env(&self.account_key_env)?;
        parse_account_key("KIS account key", &account_key)
    }

    pub fn account_key(&self) -> Result<AccountKey, TradeEngineError> {
        match &self.account_key {
            Some(account_key) => parse_account_key("kis.account_key", account_key),
            None => self.account_key_from_env(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct TossConfig {
    pub base_url: String,
    #[serde(default = "default_toss_client_id_env")]
    pub client_id_env: String,
    #[serde(default = "default_toss_client_secret_env")]
    pub client_secret_env: String,
    #[serde(default)]
    pub account_seq: Option<u64>,
    #[serde(default = "default_toss_account_seq_env")]
    pub account_seq_env: String,
}

impl Default for TossConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_TOSS_BASE_URL.to_owned(),
            client_id_env: DEFAULT_TOSS_CLIENT_ID_ENV.to_owned(),
            client_secret_env: DEFAULT_TOSS_CLIENT_SECRET_ENV.to_owned(),
            account_seq: None,
            account_seq_env: DEFAULT_TOSS_ACCOUNT_SEQ_ENV.to_owned(),
        }
    }
}

impl TossConfig {
    fn validate(&self) -> Result<(), TradeEngineError> {
        if self.base_url.trim().is_empty() {
            return Err(TradeEngineError::config("toss.base_url must not be empty"));
        }
        validate_env_var_name("toss.client_id_env", &self.client_id_env)?;
        validate_env_var_name("toss.client_secret_env", &self.client_secret_env)?;
        validate_env_var_name("toss.account_seq_env", &self.account_seq_env)?;
        if let Some(account_seq) = self.account_seq {
            parse_toss_account_seq("toss.account_seq", account_seq)?;
        }
        Ok(())
    }

    pub fn credentials_from_env(&self) -> Result<TossCredential, TradeEngineError> {
        let client_id = required_env(&self.client_id_env)?;
        let client_secret = required_env(&self.client_secret_env)?;
        TossCredential::new(client_id, client_secret).map_err(TradeEngineError::from)
    }

    pub fn account_seq_from_env(&self) -> Result<TossAccountSeq, TradeEngineError> {
        let account_seq = required_env(&self.account_seq_env)?;
        let parsed = account_seq.parse::<u64>().map_err(|error| {
            TradeEngineError::config(format!(
                "Toss account sequence in {} must be a positive integer: {error}",
                self.account_seq_env
            ))
        })?;
        parse_toss_account_seq(&self.account_seq_env, parsed)
    }

    pub fn account_seq(&self) -> Result<TossAccountSeq, TradeEngineError> {
        match self.account_seq {
            Some(account_seq) => parse_toss_account_seq("toss.account_seq", account_seq),
            None => self.account_seq_from_env(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StrategyConfig {
    pub id: String,
    pub short_window: Option<usize>,
    pub long_window: Option<usize>,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            id: "sma-crossover-v1".to_owned(),
            short_window: Some(5),
            long_window: Some(20),
        }
    }
}

impl StrategyConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        match self.id.as_str() {
            "hold-only" => {
                if self.short_window.is_some() || self.long_window.is_some() {
                    return Err(TradeEngineError::config(
                        "hold-only strategy must not set SMA window fields",
                    ));
                }
            }
            "sma-crossover-v1" => {
                let short_window = self.short_window.ok_or_else(|| {
                    TradeEngineError::config(
                        "strategy.short_window is required for sma-crossover-v1",
                    )
                })?;
                let long_window = self.long_window.ok_or_else(|| {
                    TradeEngineError::config(
                        "strategy.long_window is required for sma-crossover-v1",
                    )
                })?;
                if short_window == 0 {
                    return Err(TradeEngineError::config(
                        "strategy.short_window must be greater than zero",
                    ));
                }
                if long_window <= short_window {
                    return Err(TradeEngineError::config(
                        "strategy.long_window must be greater than strategy.short_window",
                    ));
                }
            }
            other => {
                return Err(TradeEngineError::config(format!(
                    "unsupported strategy.id: {other}"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionProviderKind {
    Deterministic,
    Command,
}

impl DecisionProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Command => "command",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DecisionConfig {
    pub provider: DecisionProviderKind,
    pub command: Option<PathBuf>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_decision_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_decision_max_stderr_bytes")]
    pub max_stderr_bytes: usize,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            provider: DecisionProviderKind::Deterministic,
            command: None,
            args: Vec::new(),
            timeout_seconds: DEFAULT_DECISION_TIMEOUT_SECONDS,
            max_stderr_bytes: DEFAULT_DECISION_MAX_STDERR_BYTES,
        }
    }
}

impl DecisionConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        validate_positive_seconds("decision.timeout_seconds", self.timeout_seconds)?;
        if self.max_stderr_bytes == 0 {
            return Err(TradeEngineError::config(
                "decision.max_stderr_bytes must be greater than zero",
            ));
        }
        match self.provider {
            DecisionProviderKind::Deterministic => {
                if self.command.is_some() || !self.args.is_empty() {
                    return Err(TradeEngineError::config(
                        "deterministic decision provider must not set command or args",
                    ));
                }
            }
            DecisionProviderKind::Command => {
                return Err(TradeEngineError::config(
                    "decision.provider=command is deprecated; use advisor.provider=command for external structured proposal providers",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdvisorProviderKind {
    Disabled,
    Command,
}

impl AdvisorProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Command => "command",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdvisorConfig {
    pub provider: AdvisorProviderKind,
    pub command: Option<PathBuf>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_advisor_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_advisor_max_stderr_bytes")]
    pub max_stderr_bytes: usize,
    #[serde(default = "default_advisor_max_stdout_bytes")]
    pub max_stdout_bytes: usize,
    #[serde(default = "default_advisor_recent_price_count")]
    pub recent_price_count: usize,
    #[serde(default = "default_advisor_candidate_limit")]
    pub candidate_limit: usize,
    #[serde(default = "default_advisor_min_command_interval_seconds")]
    pub min_command_interval_seconds: u64,
    #[serde(default = "default_advisor_price_change_threshold_bps")]
    pub price_change_threshold_bps: u32,
}

impl Default for AdvisorConfig {
    fn default() -> Self {
        Self {
            provider: AdvisorProviderKind::Disabled,
            command: None,
            args: Vec::new(),
            timeout_seconds: DEFAULT_ADVISOR_TIMEOUT_SECONDS,
            max_stderr_bytes: DEFAULT_ADVISOR_MAX_STDERR_BYTES,
            max_stdout_bytes: DEFAULT_ADVISOR_MAX_STDOUT_BYTES,
            recent_price_count: DEFAULT_ADVISOR_RECENT_PRICE_COUNT,
            candidate_limit: DEFAULT_ADVISOR_CANDIDATE_LIMIT,
            min_command_interval_seconds: DEFAULT_ADVISOR_MIN_COMMAND_INTERVAL_SECONDS,
            price_change_threshold_bps: DEFAULT_ADVISOR_PRICE_CHANGE_THRESHOLD_BPS,
        }
    }
}

impl AdvisorConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        validate_positive_seconds("advisor.timeout_seconds", self.timeout_seconds)?;
        if self.max_stderr_bytes == 0 {
            return Err(TradeEngineError::config(
                "advisor.max_stderr_bytes must be greater than zero",
            ));
        }
        if self.max_stdout_bytes == 0 {
            return Err(TradeEngineError::config(
                "advisor.max_stdout_bytes must be greater than zero",
            ));
        }
        if self.candidate_limit == 0 {
            return Err(TradeEngineError::config(
                "advisor.candidate_limit must be greater than zero",
            ));
        }
        validate_positive_seconds(
            "advisor.min_command_interval_seconds",
            self.min_command_interval_seconds,
        )?;
        if self.price_change_threshold_bps == 0 {
            return Err(TradeEngineError::config(
                "advisor.price_change_threshold_bps must be greater than zero",
            ));
        }
        match self.provider {
            AdvisorProviderKind::Disabled => {
                if self.command.is_some() || !self.args.is_empty() {
                    return Err(TradeEngineError::config(
                        "disabled advisor provider must not set command or args",
                    ));
                }
            }
            AdvisorProviderKind::Command => {
                let command = self.command.as_ref().ok_or_else(|| {
                    TradeEngineError::config(
                        "advisor.command is required when advisor.provider=command",
                    )
                })?;
                if command.as_os_str().is_empty() {
                    return Err(TradeEngineError::config(
                        "advisor.command must not be empty",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn prior_price_count(&self) -> usize {
        match self.provider {
            AdvisorProviderKind::Disabled => 0,
            AdvisorProviderKind::Command => self.recent_price_count,
        }
    }
}

fn default_advisor_recent_price_count() -> usize {
    DEFAULT_ADVISOR_RECENT_PRICE_COUNT
}

fn default_advisor_candidate_limit() -> usize {
    DEFAULT_ADVISOR_CANDIDATE_LIMIT
}

fn default_advisor_min_command_interval_seconds() -> u64 {
    DEFAULT_ADVISOR_MIN_COMMAND_INTERVAL_SECONDS
}

fn default_advisor_price_change_threshold_bps() -> u32 {
    DEFAULT_ADVISOR_PRICE_CHANGE_THRESHOLD_BPS
}

fn default_decision_timeout_seconds() -> u64 {
    DEFAULT_DECISION_TIMEOUT_SECONDS
}

fn default_decision_max_stderr_bytes() -> usize {
    DEFAULT_DECISION_MAX_STDERR_BYTES
}

fn default_advisor_timeout_seconds() -> u64 {
    DEFAULT_ADVISOR_TIMEOUT_SECONDS
}

fn default_advisor_max_stderr_bytes() -> usize {
    DEFAULT_ADVISOR_MAX_STDERR_BYTES
}

fn default_advisor_max_stdout_bytes() -> usize {
    DEFAULT_ADVISOR_MAX_STDOUT_BYTES
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UniverseDiscoveryProviderKind {
    Disabled,
    Command,
}

impl UniverseDiscoveryProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Command => "command",
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UniverseDiscoveryConfig {
    pub provider: UniverseDiscoveryProviderKind,
    pub command: Option<PathBuf>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_universe_discovery_max_symbols")]
    pub max_symbols: usize,
    #[serde(default = "default_universe_discovery_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_universe_discovery_refresh_seconds")]
    pub refresh_seconds: u64,
    #[serde(default = "default_universe_discovery_auto_refresh")]
    pub auto_refresh: bool,
    #[serde(default = "default_universe_discovery_max_stderr_bytes")]
    pub max_stderr_bytes: usize,
    #[serde(default = "default_universe_discovery_max_stdout_bytes")]
    pub max_stdout_bytes: usize,
}

impl Default for UniverseDiscoveryConfig {
    fn default() -> Self {
        Self {
            provider: UniverseDiscoveryProviderKind::Disabled,
            command: None,
            args: Vec::new(),
            max_symbols: DEFAULT_UNIVERSE_DISCOVERY_MAX_SYMBOLS,
            timeout_seconds: DEFAULT_UNIVERSE_DISCOVERY_TIMEOUT_SECONDS,
            refresh_seconds: DEFAULT_UNIVERSE_DISCOVERY_REFRESH_SECONDS,
            auto_refresh: DEFAULT_UNIVERSE_DISCOVERY_AUTO_REFRESH,
            max_stderr_bytes: DEFAULT_UNIVERSE_DISCOVERY_MAX_STDERR_BYTES,
            max_stdout_bytes: DEFAULT_UNIVERSE_DISCOVERY_MAX_STDOUT_BYTES,
        }
    }
}

impl UniverseDiscoveryConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        if self.max_symbols == 0 {
            return Err(TradeEngineError::config(
                "universe_discovery.max_symbols must be greater than zero",
            ));
        }
        validate_positive_seconds("universe_discovery.timeout_seconds", self.timeout_seconds)?;
        validate_positive_seconds("universe_discovery.refresh_seconds", self.refresh_seconds)?;
        if self.max_stderr_bytes == 0 {
            return Err(TradeEngineError::config(
                "universe_discovery.max_stderr_bytes must be greater than zero",
            ));
        }
        if self.max_stdout_bytes == 0 {
            return Err(TradeEngineError::config(
                "universe_discovery.max_stdout_bytes must be greater than zero",
            ));
        }
        match self.provider {
            UniverseDiscoveryProviderKind::Disabled => {
                if self.command.is_some() || !self.args.is_empty() {
                    return Err(TradeEngineError::config(
                        "disabled universe discovery provider must not set command or args",
                    ));
                }
            }
            UniverseDiscoveryProviderKind::Command => {
                let command = self.command.as_ref().ok_or_else(|| {
                    TradeEngineError::config(
                        "universe_discovery.command is required when universe_discovery.provider=command",
                    )
                })?;
                if command.as_os_str().is_empty() {
                    return Err(TradeEngineError::config(
                        "universe_discovery.command must not be empty",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn refresh_interval(&self) -> Duration {
        Duration::from_secs(self.refresh_seconds)
    }
}

fn default_universe_discovery_refresh_seconds() -> u64 {
    DEFAULT_UNIVERSE_DISCOVERY_REFRESH_SECONDS
}

fn default_universe_discovery_auto_refresh() -> bool {
    DEFAULT_UNIVERSE_DISCOVERY_AUTO_REFRESH
}

fn default_universe_discovery_max_symbols() -> usize {
    DEFAULT_UNIVERSE_DISCOVERY_MAX_SYMBOLS
}

fn default_universe_discovery_timeout_seconds() -> u64 {
    DEFAULT_UNIVERSE_DISCOVERY_TIMEOUT_SECONDS
}

fn default_universe_discovery_max_stderr_bytes() -> usize {
    DEFAULT_UNIVERSE_DISCOVERY_MAX_STDERR_BYTES
}

fn default_universe_discovery_max_stdout_bytes() -> usize {
    DEFAULT_UNIVERSE_DISCOVERY_MAX_STDOUT_BYTES
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DomesticMarketAlignmentConfig {
    pub enabled: bool,
    pub mode: DomesticMarketAlignmentMode,
    pub benchmark_symbol: Option<String>,
    pub feature_set: String,
    pub relative_strength_lookback_days: u32,
}

impl Default for DomesticMarketAlignmentConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: DomesticMarketAlignmentMode::AuditOnly,
            benchmark_symbol: None,
            feature_set: DEFAULT_DOMESTIC_ALIGNMENT_FEATURE_SET.to_owned(),
            relative_strength_lookback_days: DEFAULT_RELATIVE_STRENGTH_LOOKBACK_DAYS,
        }
    }
}

impl DomesticMarketAlignmentConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        if self.feature_set.trim().is_empty() {
            return Err(TradeEngineError::config(
                "domestic_market_alignment.feature_set must not be empty",
            ));
        }
        if self.relative_strength_lookback_days == 0 {
            return Err(TradeEngineError::config(
                "domestic_market_alignment.relative_strength_lookback_days must be greater than zero",
            ));
        }
        self.benchmark_symbol()?;
        Ok(())
    }

    pub fn benchmark_symbol(&self) -> Result<Option<Symbol>, TradeEngineError> {
        self.benchmark_symbol
            .as_deref()
            .map(|value| {
                Symbol::new(value.to_owned()).map_err(|source| {
                    TradeEngineError::invalid_broker_value(
                        "domestic_market_alignment.benchmark_symbol",
                        source,
                    )
                })
            })
            .transpose()
    }

    pub fn sends_advisor_context(&self) -> bool {
        self.enabled && self.mode == DomesticMarketAlignmentMode::AdvisorContext
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SizingConfig {
    pub enabled: bool,
    #[serde(with = "rust_decimal::serde::str")]
    pub target_account_fraction: Decimal,
}

impl Default for SizingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            target_account_fraction: DEFAULT_TARGET_ACCOUNT_FRACTION,
        }
    }
}

impl SizingConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        if self.enabled {
            validate_ratio(
                "sizing.target_account_fraction",
                self.target_account_fraction,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ExitPolicyConfig {
    pub enabled: bool,
    #[serde(with = "rust_decimal::serde::str")]
    pub stop_loss_pct: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub take_profit_pct: Decimal,
}

impl Default for ExitPolicyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            stop_loss_pct: DEFAULT_STOP_LOSS_PCT,
            take_profit_pct: DEFAULT_TAKE_PROFIT_PCT,
        }
    }
}

impl ExitPolicyConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        if self.enabled {
            validate_ratio("exit_policy.stop_loss_pct", self.stop_loss_pct)?;
            validate_ratio("exit_policy.take_profit_pct", self.take_profit_pct)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RiskConfig {
    pub enable_live_trading: bool,
    #[serde(with = "rust_decimal::serde::str")]
    pub max_order_notional: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub max_position_notional: Decimal,
    #[serde(default = "default_max_daily_loss", with = "rust_decimal::serde::str")]
    pub max_daily_loss: Decimal,
    pub max_daily_order_count: u32,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            enable_live_trading: false,
            max_order_notional: DEFAULT_MAX_ORDER_NOTIONAL,
            max_position_notional: DEFAULT_MAX_POSITION_NOTIONAL,
            max_daily_loss: DEFAULT_MAX_DAILY_LOSS,
            max_daily_order_count: DEFAULT_MAX_DAILY_ORDER_COUNT,
        }
    }
}

impl RiskConfig {
    pub fn validate(&self) -> Result<(), TradeEngineError> {
        validate_positive_decimal("risk.max_order_notional", self.max_order_notional)?;
        validate_positive_decimal("risk.max_position_notional", self.max_position_notional)?;
        validate_positive_decimal("risk.max_daily_loss", self.max_daily_loss)?;
        if self.max_daily_order_count == 0 {
            return Err(TradeEngineError::config(
                "risk.max_daily_order_count must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct LiveConfig {}

fn required_env(name: &str) -> Result<String, TradeEngineError> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) | Err(env::VarError::NotPresent) => Err(TradeEngineError::missing_env(name)),
        Err(env::VarError::NotUnicode(_)) => Err(TradeEngineError::invalid_broker_value(
            "environment variable",
            BrokerError::validation(format!("{name} must be valid unicode")),
        )),
    }
}

fn parse_account_key(
    field: &'static str,
    account_key: &str,
) -> Result<AccountKey, TradeEngineError> {
    AccountKey::new(account_key.to_owned())
        .map_err(|source| TradeEngineError::invalid_broker_value(field, source))
}

fn parse_toss_account_seq(
    field: &str,
    account_seq: u64,
) -> Result<TossAccountSeq, TradeEngineError> {
    TossAccountSeq::new(account_seq).map_err(|source| {
        TradeEngineError::config(format!("{field} must be a positive integer: {source}"))
    })
}

fn default_app_key_env() -> String {
    DEFAULT_APP_KEY_ENV.to_owned()
}

fn default_app_secret_env() -> String {
    DEFAULT_APP_SECRET_ENV.to_owned()
}

fn default_account_key_env() -> String {
    DEFAULT_ACCOUNT_KEY_ENV.to_owned()
}

fn default_toss_client_id_env() -> String {
    DEFAULT_TOSS_CLIENT_ID_ENV.to_owned()
}

fn default_toss_client_secret_env() -> String {
    DEFAULT_TOSS_CLIENT_SECRET_ENV.to_owned()
}

fn default_toss_account_seq_env() -> String {
    DEFAULT_TOSS_ACCOUNT_SEQ_ENV.to_owned()
}

fn default_max_daily_loss() -> Decimal {
    DEFAULT_MAX_DAILY_LOSS
}

fn validate_env_var_name(name: &str, value: &str) -> Result<(), TradeEngineError> {
    if value.trim().is_empty() {
        return Err(TradeEngineError::config(format!(
            "{name} must not be empty"
        )));
    }
    if value.chars().any(char::is_whitespace) {
        return Err(TradeEngineError::config(format!(
            "{name} must not contain whitespace"
        )));
    }
    Ok(())
}

fn validate_positive_seconds(name: &str, value: u64) -> Result<(), TradeEngineError> {
    if value == 0 {
        return Err(TradeEngineError::config(format!(
            "{name} must be greater than zero"
        )));
    }
    Ok(())
}

fn validate_positive_decimal(name: &str, value: Decimal) -> Result<(), TradeEngineError> {
    if value <= Decimal::ZERO {
        return Err(TradeEngineError::config(format!(
            "{name} must be greater than zero"
        )));
    }
    Ok(())
}

fn validate_ratio(name: &str, value: Decimal) -> Result<(), TradeEngineError> {
    if value <= Decimal::ZERO || value > Decimal::ONE {
        return Err(TradeEngineError::config(format!(
            "{name} must be greater than zero and less than or equal to one"
        )));
    }
    Ok(())
}

fn validate_symbol_matches_market(
    name: &str,
    symbol: &Symbol,
    market_profile: &MarketProfile,
) -> Result<(), TradeEngineError> {
    if !market_profile.symbol_matches(symbol) {
        return Err(TradeEngineError::config(format!(
            "{name} {} does not match market.kind={}",
            symbol.as_str(),
            market_profile.scope().kind()
        )));
    }
    Ok(())
}
