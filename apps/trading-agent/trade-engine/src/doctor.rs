use std::path::Path;

use time::OffsetDateTime;
use trading_agent_kis_client::journal::BrokerCommandJournal;
use trading_agent_kis_client::{BrokerEnvironment, BrokerHealth};

use crate::config::EngineConfig;
use crate::error::TradeEngineError;
use crate::gateway::traits::TradingGateway;
use crate::gateway::types::{MarketSession, Symbol};
use crate::market::MarketProfile;
use crate::stdout::{OutputField, StructuredOutput};
use crate::storage::{MainDb, SqliteBrokerCommandJournal};

#[derive(Debug, Clone, Copy)]
pub struct StartupDoctorContext<'a> {
    pub account_suffix: &'a str,
    pub watchlist: &'a [Symbol],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupDoctorReport {
    pub account_suffix: String,
    pub broker_environment: BrokerEnvironment,
    pub broker_writable: bool,
    pub position_count: usize,
    pub watchlist_count: usize,
    pub quote_count: usize,
    pub unresolved_command_count: usize,
    pub market_session: MarketSession,
}

pub struct StartupDoctorRun {
    db: MainDb,
    unresolved_command_count: usize,
}

impl StartupDoctorReport {
    pub fn render(&self) -> String {
        self.render_for("dry_run", true)
    }

    pub fn render_for(&self, mode: &str, read_only: bool) -> String {
        let mut output = StructuredOutput::new();
        output
            .heading("Startup Doctor")
            .field("status", "passed")
            .field(
                "purpose",
                "verify config, local state, broker read access, and market data before runtime start",
            )
            .field("mode", mode)
            .blank()
            .heading("Preflight checks")
            .status_line(
                "configuration",
                "passed",
                &[
                    OutputField::new("read_only", read_only),
                    OutputField::new("watchlist", self.watchlist_count),
                ],
            )
            .status_line(
                "local_state",
                "passed",
                &[OutputField::new(
                    "unresolved_commands",
                    self.unresolved_command_count,
                )],
            )
            .status_line(
                "broker_connection",
                "passed",
                &[
                    OutputField::new("environment", self.broker_environment.as_str()),
                    OutputField::new("writable", self.broker_writable),
                ],
            )
            .status_line(
                "account_access",
                "passed",
                &[
                    OutputField::new("account_suffix", self.account_suffix.as_str()),
                    OutputField::new("positions", self.position_count),
                ],
            )
            .status_line(
                "market_data",
                "passed",
                &[OutputField::new("quotes", self.quote_count)],
            )
            .status_line(
                "market_session",
                "passed",
                &[
                    OutputField::new("trading_day", self.market_session.trading_day.as_str()),
                    OutputField::new("open", self.market_session.is_open),
                ],
            );
        output.finish()
    }
}

pub async fn run_read_only_startup_doctor<G>(
    config: &EngineConfig,
    gateway: &G,
    context: StartupDoctorContext<'_>,
) -> Result<StartupDoctorReport, TradeEngineError>
where
    G: TradingGateway + ?Sized,
{
    let run = start_read_only_startup_doctor(config, context).await?;
    complete_read_only_startup_doctor(config, gateway, context, run).await
}

pub async fn start_read_only_startup_doctor(
    config: &EngineConfig,
    context: StartupDoctorContext<'_>,
) -> Result<StartupDoctorRun, TradeEngineError> {
    config.validate_read_only_smoke()?;
    if context.watchlist.is_empty() {
        return Err(TradeEngineError::config(
            "startup doctor requires at least one watchlist symbol",
        ));
    }

    let main_db_path = config.main_db_path()?;
    let mut db = MainDb::open(main_db_path)?;
    let result = unresolved_command_count(main_db_path).await;
    match result {
        Ok(unresolved_command_count) => Ok(StartupDoctorRun {
            db,
            unresolved_command_count,
        }),
        Err(error) => {
            record_startup_doctor_failure(config, context, &mut db, &error)?;
            Err(error)
        }
    }
}

pub async fn start_write_startup_doctor(
    config: &EngineConfig,
    context: StartupDoctorContext<'_>,
) -> Result<StartupDoctorRun, TradeEngineError> {
    config.validate_write_runtime_safety()?;
    if context.watchlist.is_empty() {
        return Err(TradeEngineError::config(
            "startup doctor requires at least one watchlist symbol",
        ));
    }

    let main_db_path = config.main_db_path()?;
    let mut db = MainDb::open(main_db_path)?;
    let result = unresolved_command_count(main_db_path).await;
    match result {
        Ok(unresolved_command_count) => Ok(StartupDoctorRun {
            db,
            unresolved_command_count,
        }),
        Err(error) => {
            record_startup_doctor_failure(config, context, &mut db, &error)?;
            Err(error)
        }
    }
}

pub async fn complete_read_only_startup_doctor<G>(
    config: &EngineConfig,
    gateway: &G,
    context: StartupDoctorContext<'_>,
    mut run: StartupDoctorRun,
) -> Result<StartupDoctorReport, TradeEngineError>
where
    G: TradingGateway + ?Sized,
{
    let result =
        run_startup_doctor_checks(config, gateway, context, run.unresolved_command_count, true)
            .await;
    match result {
        Ok(report) => {
            let metadata_json = startup_doctor_passed_metadata_json(&report)?;
            run.db.record_startup_doctor_result(
                report.broker_environment,
                report.account_suffix.as_str(),
                true,
                None,
                metadata_json.as_str(),
            )?;
            Ok(report)
        }
        Err(error) => {
            record_startup_doctor_failure(config, context, &mut run.db, &error)?;
            Err(error)
        }
    }
}

pub async fn complete_write_startup_doctor<G>(
    config: &EngineConfig,
    gateway: &G,
    context: StartupDoctorContext<'_>,
    mut run: StartupDoctorRun,
) -> Result<StartupDoctorReport, TradeEngineError>
where
    G: TradingGateway + ?Sized,
{
    let result = run_startup_doctor_checks(
        config,
        gateway,
        context,
        run.unresolved_command_count,
        false,
    )
    .await;
    match result {
        Ok(report) => {
            let metadata_json = startup_doctor_passed_metadata_json(&report)?;
            run.db.record_startup_doctor_result(
                report.broker_environment,
                report.account_suffix.as_str(),
                true,
                None,
                metadata_json.as_str(),
            )?;
            Ok(report)
        }
        Err(error) => {
            record_startup_doctor_failure(config, context, &mut run.db, &error)?;
            Err(error)
        }
    }
}

pub fn record_startup_doctor_build_failure(
    config: &EngineConfig,
    context: StartupDoctorContext<'_>,
    mut run: StartupDoctorRun,
    error: &TradeEngineError,
) -> Result<(), TradeEngineError> {
    record_startup_doctor_failure(config, context, &mut run.db, error)
}

async fn run_startup_doctor_checks<G>(
    config: &EngineConfig,
    gateway: &G,
    context: StartupDoctorContext<'_>,
    unresolved_command_count: usize,
    require_read_only_health: bool,
) -> Result<StartupDoctorReport, TradeEngineError>
where
    G: TradingGateway + ?Sized,
{
    let health = gateway
        .ping()
        .await
        .map_err(|source| TradeEngineError::gateway("ping", source))?;
    if health.environment != config.broker_environment() {
        return Err(TradeEngineError::runtime(format!(
            "startup doctor broker environment {} does not match config broker environment={}",
            health.environment.as_str(),
            config.broker_environment().as_str()
        )));
    }
    if require_read_only_health {
        ensure_read_only_health(&health)?;
    }

    let account = gateway
        .get_account_snapshot()
        .await
        .map_err(|source| TradeEngineError::gateway("account_snapshot", source))?;

    let mut quote_count = 0_usize;
    for symbol in context.watchlist {
        gateway
            .get_quote(symbol)
            .await
            .map_err(|source| TradeEngineError::gateway("quote", source))?;
        quote_count += 1;
    }

    let trading_day =
        MarketProfile::new(config.market.scope()?).trading_day_string_at(OffsetDateTime::now_utc());
    let market_session = gateway
        .get_market_session(&trading_day)
        .await
        .map_err(|source| TradeEngineError::gateway("market_session", source))?;

    Ok(StartupDoctorReport {
        account_suffix: context.account_suffix.to_owned(),
        broker_environment: health.environment,
        broker_writable: health.writable,
        position_count: account.positions.len(),
        watchlist_count: context.watchlist.len(),
        quote_count,
        unresolved_command_count,
        market_session,
    })
}

async fn unresolved_command_count(main_db_path: &Path) -> Result<usize, TradeEngineError> {
    let journal = SqliteBrokerCommandJournal::open(main_db_path)?;
    let unresolved_commands = journal.list_unreconciled().await?;
    if !unresolved_commands.is_empty() {
        return Err(TradeEngineError::runtime(format!(
            "startup doctor blocked by {} unresolved broker command journal record(s)",
            unresolved_commands.len()
        )));
    }
    Ok(unresolved_commands.len())
}

fn record_startup_doctor_failure(
    config: &EngineConfig,
    context: StartupDoctorContext<'_>,
    db: &mut MainDb,
    error: &TradeEngineError,
) -> Result<(), TradeEngineError> {
    let message = error.to_string();
    let metadata_json = startup_doctor_failed_metadata_json(config, context, &message)?;
    db.record_startup_doctor_result(
        config.broker_environment(),
        context.account_suffix,
        false,
        Some(message.as_str()),
        metadata_json.as_str(),
    )
}

fn startup_doctor_passed_metadata_json(
    report: &StartupDoctorReport,
) -> Result<String, TradeEngineError> {
    serde_json::to_string(&serde_json::json!({
        "status": "passed",
        "environment": report.broker_environment.as_str(),
        "account_suffix": report.account_suffix.as_str(),
        "broker_writable": report.broker_writable,
        "position_count": report.position_count,
        "watchlist_count": report.watchlist_count,
        "quote_count": report.quote_count,
        "unresolved_command_count": report.unresolved_command_count,
        "market_session": {
            "trading_day": report.market_session.trading_day.as_str(),
            "is_open": report.market_session.is_open,
        },
    }))
    .map_err(|error| {
        TradeEngineError::storage(format!("failed to encode startup doctor metadata: {error}"))
    })
}

fn startup_doctor_failed_metadata_json(
    config: &EngineConfig,
    context: StartupDoctorContext<'_>,
    error: &str,
) -> Result<String, TradeEngineError> {
    serde_json::to_string(&serde_json::json!({
        "status": "failed",
        "environment": config.broker_environment().as_str(),
        "account_suffix": context.account_suffix,
        "watchlist_count": context.watchlist.len(),
        "error": error,
    }))
    .map_err(|source| {
        TradeEngineError::storage(format!(
            "failed to encode startup doctor metadata: {source}"
        ))
    })
}

fn ensure_read_only_health(health: &BrokerHealth) -> Result<(), TradeEngineError> {
    if health.writable {
        return Err(TradeEngineError::runtime(
            "startup doctor expected broker writable=false in read-only mode",
        ));
    }
    Ok(())
}
