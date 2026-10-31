use crate::runtime::stream_runtime_loop_with_execution_and_shutdown;
use std::io::{self, Write};
use std::str::FromStr;

mod args;
mod execution_driver;
mod universe_task;
pub use args::RunnerArgs;
pub use execution_driver::DbRuntimeExecutionDriver;

use crate::backtest::{BacktestConfig, BacktestReport, run_sma_crossover_backtest};
use crate::config::{BrokerProviderKind, EngineConfig};
use crate::doctor::{
    StartupDoctorContext, StartupDoctorRun, complete_read_only_startup_doctor,
    complete_write_startup_doctor, record_startup_doctor_build_failure,
    start_read_only_startup_doctor, start_write_startup_doctor,
};
use crate::error::TradeEngineError;
use crate::feedback::FeedbackProvider as _;
use crate::gateway::adapters::kis::{
    KisGatewayBundle, build_read_only_kis_gateway, build_write_enabled_kis_gateway,
};
use crate::gateway::adapters::toss::{TossGatewayBundle, build_read_only_toss_gateway};
use crate::gateway::traits::TradingGateway;
use crate::gateway::types::{MarketSession, Quantity, Symbol, TradingDay};
use crate::market::MarketProfile;
use crate::runtime::{
    DryRunRunContext, DryRunRuntimeSettings, RuntimeLoopControl, RuntimeLoopExecutionOptions,
    RuntimeLoopOutputOptions, RuntimeMarketHours, RuntimeMode, RuntimeOutputSink,
    RuntimeShutdownToken, UniverseRefreshContext, run_dry_run_loop_with_shutdown, run_dry_run_tick,
    run_runtime_loop_with_execution_and_shutdown, run_runtime_tick_with_execution,
    stream_dry_run_loop_with_output,
};
use crate::stdout::{ColorMode, StdoutRenderOptions, render_stdout_chunk};
use crate::storage::{
    BlockingMainDb, CompletedFeedbackQueryAudit, FailedFeedbackQueryAudit, FeedbackQueryAuditInput,
    MainDb,
};
use args::{BacktestArgs, QueryArgs, ReviewArgs, RunArgs, RunnerCommand, StatusArgs, UniverseArgs};
use rust_decimal::Decimal;
use time::{Date, OffsetDateTime};
use tokio::time::sleep;
use trading_agent_kis_client::{AccountKey, BrokerEnvironment};
use universe_task::{
    UniverseRefreshScheduler, spawn_auto_universe_refresh_loop, stop_auto_universe_refresh_loop,
};
#[cfg(test)]
use universe_task::{run_auto_universe_refresh_once_at, wait_for_runtime_shutdown};

const UNKNOWN_STARTUP_ACCOUNT_SUFFIX: &str = "unknown";

struct RuntimeGatewayBundle {
    gateway: Box<dyn TradingGateway>,
    account_key: Option<AccountKey>,
    account_suffix: String,
    environment: BrokerEnvironment,
}

pub async fn run(config: EngineConfig, args: RunnerArgs) -> Result<String, TradeEngineError> {
    run_with_output_sink(config, args, None).await
}

pub async fn run_to_stdout(config: EngineConfig, args: RunnerArgs) -> Result<(), TradeEngineError> {
    let render_options = args.stdout_render_options();
    let mut stdout = StdoutRuntimeOutputSink::new(render_options);
    let output = run_with_output_sink(config, args, Some(&mut stdout)).await?;
    if !stdout.has_written {
        stdout.write(&output)?;
    }
    Ok(())
}

async fn run_with_output_sink(
    config: EngineConfig,
    args: RunnerArgs,
    output_sink: Option<&mut dyn RuntimeOutputSink>,
) -> Result<String, TradeEngineError> {
    match args.command {
        RunnerCommand::Backtest(args) => backtest_command(config, args),
        RunnerCommand::Doctor(_) => doctor_command(config).await,
        RunnerCommand::Query(args) => query_command(config, args).await,
        RunnerCommand::Review(args) => review_command(config, args).await,
        RunnerCommand::Run(args) => run_command(config, args, output_sink).await,
        RunnerCommand::Status(args) => status_command(config, args).await,
        RunnerCommand::Universe(args) => universe_command(config, args, output_sink).await,
    }
}

struct StdoutRuntimeOutputSink {
    has_written: bool,
    render_options: StdoutRenderOptions,
}

impl StdoutRuntimeOutputSink {
    fn new(render_options: StdoutRenderOptions) -> Self {
        Self {
            has_written: false,
            render_options,
        }
    }
}

impl RuntimeOutputSink for StdoutRuntimeOutputSink {
    fn write(&mut self, chunk: &str) -> Result<(), TradeEngineError> {
        let rendered = render_stdout_chunk(chunk, self.render_options);
        let mut stdout =
            anstream::AutoStream::new(io::stdout(), color_choice(self.render_options.color)).lock();
        stdout.write_all(rendered.as_bytes()).map_err(|error| {
            TradeEngineError::runtime(format!("failed to write stdout: {error}"))
        })?;
        stdout.flush().map_err(|error| {
            TradeEngineError::runtime(format!("failed to flush stdout: {error}"))
        })?;
        self.has_written = true;
        Ok(())
    }
}

fn color_choice(color: ColorMode) -> anstream::ColorChoice {
    match color {
        ColorMode::Auto => anstream::ColorChoice::Auto,
        ColorMode::Always => anstream::ColorChoice::Always,
        ColorMode::Never => anstream::ColorChoice::Never,
    }
}

async fn doctor_command(config: EngineConfig) -> Result<String, TradeEngineError> {
    let watchlist = config.watchlist_symbols()?;
    let (bundle, doctor_run) = build_gateway_after_doctor_preflight(&config, &watchlist).await?;
    run_doctor_with_gateway(
        &config,
        bundle.gateway.as_ref(),
        &bundle.account_suffix,
        &watchlist,
        doctor_run,
    )
    .await
}

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

async fn query_command(config: EngineConfig, args: QueryArgs) -> Result<String, TradeEngineError> {
    crate::feedback::validate_query_prompt(&args.prompt)?;
    let db = BlockingMainDb::open(config.main_db_path()?)?;
    let context = db
        .feedback_review_context(args.since_hours, args.limit)
        .await?;
    let request = crate::feedback::build_query_request(args.prompt.clone(), &context)
        .map_err(|error| TradeEngineError::runtime(error.to_string()))?;
    let context_summary = crate::feedback::query_context_summary(&context);
    let audit_input = FeedbackQueryAuditInput {
        schema_version: request.schema_version,
        since_hours: args.since_hours,
        limit: args.limit,
        prompt: args.prompt,
        context_summary,
    };
    let provider = crate::feedback::CommandFeedbackProvider::codex_default();
    match provider.query(&request).await {
        Ok(response) => {
            let parsed_output =
                serde_json::to_value(response.output.output()).map_err(|error| {
                    TradeEngineError::runtime(format!("failed to encode feedback output: {error}"))
                })?;
            db.record_feedback_query_completed(CompletedFeedbackQueryAudit {
                input: audit_input,
                raw_output_json: response.raw_output_json,
                parsed_output,
            })
            .await?;
            Ok(crate::feedback::render_query(&response.output))
        }
        Err(error) => {
            db.record_feedback_query_failed(FailedFeedbackQueryAudit {
                input: audit_input,
                error: error.to_string(),
            })
            .await?;
            Err(TradeEngineError::runtime(format!(
                "feedback query failed: {error}"
            )))
        }
    }
}

fn backtest_command(config: EngineConfig, args: BacktestArgs) -> Result<String, TradeEngineError> {
    config.validate_backtest()?;
    let from = parse_backtest_trading_day("backtest.from", &args.from)?;
    let to = parse_backtest_trading_day("backtest.to", &args.to)?;
    if to.0 < from.0 {
        return Err(TradeEngineError::config(
            "backtest --to must be greater than or equal to --from",
        ));
    }
    let initial_cash = Decimal::from_str(&args.initial_cash).map_err(|error| {
        TradeEngineError::config(format!(
            "backtest --initial-cash must be a decimal: {error}"
        ))
    })?;
    let short_window = config.strategy.short_window.ok_or_else(|| {
        TradeEngineError::config("strategy.short_window is required for backtest")
    })?;
    let long_window = config
        .strategy
        .long_window
        .ok_or_else(|| TradeEngineError::config("strategy.long_window is required for backtest"))?;
    let backtest_config = BacktestConfig::sma_crossover(
        config.strategy.id.clone(),
        short_window,
        long_window,
        initial_cash,
        Quantity::new(args.quantity_per_trade),
    )
    .map_err(|error| TradeEngineError::runtime(error.to_string()))?;
    let symbols = backtest_symbols(&config, &args)?;
    let market_scope = config.market.scope()?;
    let db = MainDb::open(config.main_db_path()?)?;
    let mut reports = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        let bars = db.ohlcv_bars_for_symbol_between(
            &symbol,
            &market_scope,
            crate::technical_analysis::OhlcvBarInterval::Daily,
            from,
            to,
        )?;
        let report = run_sma_crossover_backtest(&bars, &backtest_config)
            .map_err(|error| TradeEngineError::runtime(error.to_string()))?;
        reports.push(report);
    }
    Ok(render_backtest_reports(
        &config.strategy.id,
        crate::technical_analysis::OhlcvBarInterval::Daily,
        from,
        to,
        &reports,
    ))
}

async fn run_command(
    config: EngineConfig,
    args: RunArgs,
    output_sink: Option<&mut dyn RuntimeOutputSink>,
) -> Result<String, TradeEngineError> {
    validate_run_config(&config, &args)?;
    if args.submit_orders {
        return submit_orders_command(config, args, output_sink).await;
    }

    let watchlist = config.watchlist_symbols()?;
    let (bundle, doctor_run) = build_gateway_after_doctor_preflight(&config, &watchlist).await?;
    let mut output = run_doctor_with_gateway(
        &config,
        bundle.gateway.as_ref(),
        &bundle.account_suffix,
        &watchlist,
        doctor_run,
    )
    .await?;
    let main_db_path = config.main_db_path()?.to_path_buf();
    let db = BlockingMainDb::open(&main_db_path)?;
    let environment = bundle.environment;
    let market_scope = config.market.scope()?;
    let mut run_context = DryRunRunContext::new(bundle.account_suffix.clone(), environment)
        .with_market_scope(market_scope.clone());
    if let Some(account_key) = bundle.account_key.clone() {
        run_context = run_context.with_account_key(account_key);
    }
    let universe_context = UniverseRefreshContext {
        environment,
        account_suffix: bundle.account_suffix.clone(),
        market_scope,
    };
    let settings = DryRunRuntimeSettings::with_risk(
        config.runtime,
        config.strategy,
        config.decision,
        config.risk,
        config.read_only,
    )
    .with_universe_discovery(config.universe_discovery.clone())
    .with_domestic_market_alignment(config.domestic_market_alignment.clone())
    .with_advisor(config.advisor)
    .with_sizing(config.sizing)
    .with_exit_policy(config.exit_policy);
    let runtime_output = match (args.once, output_sink) {
        (true, _) => {
            run_dry_run_tick(
                bundle.gateway.as_ref(),
                &db,
                &run_context,
                &watchlist,
                settings,
            )
            .await
        }
        (false, Some(sink)) => {
            sink.write(&output)?;
            sink.write("\n")?;
            output.push('\n');
            let market_hours =
                resolve_runtime_market_hours(bundle.gateway.as_ref(), &run_context.market_scope)
                    .await?;
            let shutdown = RuntimeShutdownToken::new();
            spawn_ctrl_c_shutdown_listener(shutdown.clone());
            let universe_refresh = spawn_auto_universe_refresh_loop(
                main_db_path.clone(),
                settings.clone(),
                universe_context.clone(),
                watchlist.clone(),
                shutdown.clone(),
            );
            let runtime_output = stream_dry_run_loop_with_output(
                bundle.gateway.as_ref(),
                &db,
                &run_context,
                &watchlist,
                settings,
                RuntimeLoopControl {
                    market_hours,
                    max_market_ticks: args.max_market_ticks,
                },
                RuntimeLoopOutputOptions {
                    shutdown: shutdown.clone(),
                    output_sink: sink,
                },
            )
            .await;
            stop_auto_universe_refresh_loop(&shutdown, universe_refresh).await?;
            runtime_output
        }
        (false, None) => {
            let market_hours =
                resolve_runtime_market_hours(bundle.gateway.as_ref(), &run_context.market_scope)
                    .await?;
            let shutdown = RuntimeShutdownToken::new();
            spawn_ctrl_c_shutdown_listener(shutdown.clone());
            output.push('\n');
            let universe_refresh = spawn_auto_universe_refresh_loop(
                main_db_path.clone(),
                settings.clone(),
                universe_context.clone(),
                watchlist.clone(),
                shutdown.clone(),
            );
            let loop_shutdown = shutdown.clone();
            let runtime_output = run_dry_run_loop_with_shutdown(
                bundle.gateway.as_ref(),
                &db,
                &run_context,
                &watchlist,
                settings,
                RuntimeLoopControl {
                    market_hours,
                    max_market_ticks: args.max_market_ticks,
                },
                loop_shutdown,
            )
            .await;
            stop_auto_universe_refresh_loop(&shutdown, universe_refresh).await?;
            runtime_output
        }
    }?;
    output.push('\n');
    output.push_str(&runtime_output);
    Ok(output)
}

fn backtest_symbols(
    config: &EngineConfig,
    args: &BacktestArgs,
) -> Result<Vec<Symbol>, TradeEngineError> {
    if args.symbols.is_empty() {
        return config.watchlist_symbols();
    }
    let profile = MarketProfile::new(config.market.scope()?);
    args.symbols
        .iter()
        .map(|symbol| {
            let symbol = Symbol::new(symbol.clone()).map_err(|source| {
                TradeEngineError::invalid_broker_value("backtest symbol", source)
            })?;
            if !profile.symbol_matches(&symbol) {
                return Err(TradeEngineError::config(format!(
                    "backtest symbol {} does not match configured market scope {}",
                    symbol.as_str(),
                    profile.scope().kind()
                )));
            }
            Ok(symbol)
        })
        .collect()
}

fn parse_backtest_trading_day(
    field: &'static str,
    value: &str,
) -> Result<TradingDay, TradeEngineError> {
    let format = time::format_description::parse("[year]-[month]-[day]").map_err(|error| {
        TradeEngineError::runtime(format!("failed to build backtest date parser: {error}"))
    })?;
    Date::parse(value, &format)
        .map(TradingDay)
        .map_err(|error| TradeEngineError::config(format!("{field} must use YYYY-MM-DD: {error}")))
}

fn render_backtest_reports(
    strategy_id: &str,
    interval: crate::technical_analysis::OhlcvBarInterval,
    from: TradingDay,
    to: TradingDay,
    reports: &[BacktestReport],
) -> String {
    let mut output = String::new();
    output.push_str("Backtest\n");
    output.push_str(&format!("  strategy: {strategy_id}\n"));
    output.push_str(&format!("  interval: {}\n", interval.as_str()));
    output.push_str(&format!("  from: {}\n", from.0));
    output.push_str(&format!("  to: {}\n", to.0));
    for (index, report) in reports.iter().enumerate() {
        output.push_str(&format!("Backtest result #{}\n", index + 1));
        output.push_str(&format!("  symbol: {}\n", report.symbol.as_str()));
        output.push_str(&format!("  bars: {}\n", report.bars.len()));
        output.push_str(&format!("  start: {}\n", report.start_day.0));
        output.push_str(&format!("  end: {}\n", report.end_day.0));
        output.push_str(&format!(
            "  starting_equity: {}\n",
            report.metrics.starting_equity
        ));
        output.push_str(&format!(
            "  ending_equity: {}\n",
            report.metrics.ending_equity
        ));
        output.push_str(&format!(
            "  total_return_pct: {}\n",
            report.metrics.total_return_pct
        ));
        output.push_str(&format!(
            "  max_drawdown_pct: {}\n",
            report.metrics.max_drawdown_pct
        ));
        output.push_str(&format!(
            "  realized_pnl: {}\n",
            report.metrics.realized_pnl
        ));
        output.push_str(&format!(
            "  unrealized_pnl: {}\n",
            report.metrics.unrealized_pnl
        ));
        output.push_str(&format!("  buy_count: {}\n", report.metrics.buy_count));
        output.push_str(&format!("  sell_count: {}\n", report.metrics.sell_count));
        output.push_str(&format!("  hold_count: {}\n", report.metrics.hold_count));
        output.push_str(&format!(
            "  cash_blocked_buy_count: {}\n",
            report.metrics.cash_blocked_buy_count
        ));
    }
    output
}

async fn universe_command(
    config: EngineConfig,
    args: UniverseArgs,
    mut output_sink: Option<&mut dyn RuntimeOutputSink>,
) -> Result<String, TradeEngineError> {
    config.validate_universe_refresh()?;
    let watchlist = config.watchlist_symbols()?;
    let account_suffix = broker_account_suffix(&config)?;
    let context = UniverseRefreshContext {
        environment: config.broker_environment(),
        account_suffix,
        market_scope: config.market.scope()?,
    };
    let db = BlockingMainDb::open(config.main_db_path()?)?;
    let settings = DryRunRuntimeSettings::with_risk(
        config.runtime,
        config.strategy,
        config.decision,
        config.risk,
        config.read_only,
    )
    .with_universe_discovery(config.universe_discovery.clone())
    .with_domestic_market_alignment(config.domestic_market_alignment.clone())
    .with_advisor(config.advisor)
    .with_sizing(config.sizing)
    .with_exit_policy(config.exit_policy);
    let scheduler = UniverseRefreshScheduler::new(&settings, &context);
    let mut output = String::new();
    let mut refresh_count = 0_u64;
    loop {
        if args.loop_forever {
            loop {
                let now_utc = OffsetDateTime::now_utc();
                if scheduler.refresh_window_open_at(now_utc) {
                    break;
                }
                tokio::select! {
                    _ = sleep(scheduler.next_refresh_delay_at(now_utc)) => {}
                    _ = tokio::signal::ctrl_c() => return Ok(output),
                }
            }
        }
        let refresh =
            crate::runtime::refresh_universe_once(&db, &settings, &context, &watchlist).await?;
        let rendered = refresh.render();
        if let Some(sink) = output_sink.as_deref_mut() {
            sink.write(&rendered)?;
        }
        output.push_str(&rendered);
        refresh_count += 1;
        if !args.loop_forever || args.max_refreshes == Some(refresh_count) {
            break;
        }
        tokio::select! {
            _ = sleep(scheduler.successive_refresh_delay()) => {}
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }
    Ok(output)
}

async fn submit_orders_command(
    config: EngineConfig,
    args: RunArgs,
    output_sink: Option<&mut dyn RuntimeOutputSink>,
) -> Result<String, TradeEngineError> {
    let watchlist = config.watchlist_symbols()?;
    let (bundle, doctor_run) =
        build_write_gateway_after_doctor_preflight(&config, &watchlist).await?;
    let doctor_context = StartupDoctorContext {
        account_suffix: &bundle.account_suffix,
        watchlist: &watchlist,
    };
    if let Err(error) = ensure_order_submission_runtime_readiness(&config, &bundle) {
        record_startup_doctor_build_failure(&config, doctor_context, doctor_run, &error)?;
        return Err(error);
    }
    let report =
        complete_write_startup_doctor(&config, &bundle.gateway, doctor_context, doctor_run).await?;
    let mut output = report.render_for("submit_orders", config.read_only);
    let main_db_path = config.main_db_path()?.to_path_buf();
    let db = BlockingMainDb::open(&main_db_path)?;
    let run_context = DryRunRunContext::new(
        bundle.account_suffix.clone(),
        config.kis.environment.as_broker_environment(),
    )
    .with_market_scope(config.market.scope()?)
    .with_account_key(bundle.account_key.clone())
    .with_mode(RuntimeMode::SubmitOrders);
    let settings = DryRunRuntimeSettings::with_risk(
        config.runtime,
        config.strategy.clone(),
        config.decision,
        config.risk.clone(),
        config.read_only,
    )
    .with_universe_discovery(config.universe_discovery.clone())
    .with_domestic_market_alignment(config.domestic_market_alignment.clone())
    .with_advisor(config.advisor.clone())
    .with_sizing(config.sizing.clone())
    .with_exit_policy(config.exit_policy.clone());
    let execution_driver = DbRuntimeExecutionDriver::new(
        main_db_path.clone(),
        bundle.account_key,
        config.kis.environment.as_broker_environment(),
        config.read_only,
        config.risk,
    );
    let universe_context = UniverseRefreshContext {
        environment: config.kis.environment.as_broker_environment(),
        account_suffix: bundle.account_suffix.clone(),
        market_scope: config.market.scope()?,
    };
    let runtime_output = match (args.once, output_sink) {
        (true, _) => {
            run_runtime_tick_with_execution(
                &bundle.gateway,
                &db,
                &run_context,
                &watchlist,
                settings,
                &execution_driver,
            )
            .await
        }
        (false, Some(sink)) => {
            sink.write(&output)?;
            sink.write("\n")?;
            output.push('\n');
            let market_hours =
                resolve_runtime_market_hours(&bundle.gateway, &run_context.market_scope).await?;
            let shutdown = RuntimeShutdownToken::new();
            spawn_ctrl_c_shutdown_listener(shutdown.clone());
            let universe_refresh = spawn_auto_universe_refresh_loop(
                main_db_path.clone(),
                settings.clone(),
                universe_context.clone(),
                watchlist.clone(),
                shutdown.clone(),
            );
            let runtime_output = stream_runtime_loop_with_execution_and_shutdown(
                &bundle.gateway,
                &db,
                &run_context,
                &watchlist,
                RuntimeLoopExecutionOptions {
                    settings,
                    control: RuntimeLoopControl {
                        market_hours,
                        max_market_ticks: args.max_market_ticks,
                    },
                    shutdown: shutdown.clone(),
                    execution_driver: &execution_driver,
                    output_sink: Some(sink),
                },
            )
            .await;
            stop_auto_universe_refresh_loop(&shutdown, universe_refresh).await?;
            runtime_output
        }
        (false, None) => {
            let market_hours =
                resolve_runtime_market_hours(&bundle.gateway, &run_context.market_scope).await?;
            let shutdown = RuntimeShutdownToken::new();
            spawn_ctrl_c_shutdown_listener(shutdown.clone());
            output.push('\n');
            let universe_refresh = spawn_auto_universe_refresh_loop(
                main_db_path.clone(),
                settings.clone(),
                universe_context.clone(),
                watchlist.clone(),
                shutdown.clone(),
            );
            let runtime_output = run_runtime_loop_with_execution_and_shutdown(
                &bundle.gateway,
                &db,
                &run_context,
                &watchlist,
                RuntimeLoopExecutionOptions {
                    settings,
                    control: RuntimeLoopControl {
                        market_hours,
                        max_market_ticks: args.max_market_ticks,
                    },
                    shutdown: shutdown.clone(),
                    execution_driver: &execution_driver,
                    output_sink: None,
                },
            )
            .await;
            stop_auto_universe_refresh_loop(&shutdown, universe_refresh).await?;
            runtime_output
        }
    }?;
    output.push('\n');
    output.push_str(&runtime_output);
    Ok(output)
}

async fn build_gateway_after_doctor_preflight(
    config: &EngineConfig,
    watchlist: &[Symbol],
) -> Result<(RuntimeGatewayBundle, StartupDoctorRun), TradeEngineError> {
    let preflight_context = StartupDoctorContext {
        account_suffix: UNKNOWN_STARTUP_ACCOUNT_SUFFIX,
        watchlist,
    };
    let doctor_run = start_read_only_startup_doctor(config, preflight_context).await?;
    let bundle = match config.broker.provider {
        BrokerProviderKind::Kis => build_read_only_kis_gateway(config)
            .await
            .map(|bundle| runtime_bundle_from_kis(bundle, config.broker_environment())),
        BrokerProviderKind::Toss => build_read_only_toss_gateway(config)
            .await
            .map(runtime_bundle_from_toss),
    };
    match bundle {
        Ok(bundle) => Ok((bundle, doctor_run)),
        Err(error) => {
            record_startup_doctor_build_failure(config, preflight_context, doctor_run, &error)?;
            Err(error)
        }
    }
}

fn runtime_bundle_from_kis(
    bundle: KisGatewayBundle,
    environment: BrokerEnvironment,
) -> RuntimeGatewayBundle {
    RuntimeGatewayBundle {
        gateway: Box::new(bundle.gateway),
        account_key: Some(bundle.account_key),
        account_suffix: bundle.account_suffix,
        environment,
    }
}

fn runtime_bundle_from_toss(bundle: TossGatewayBundle) -> RuntimeGatewayBundle {
    RuntimeGatewayBundle {
        gateway: Box::new(bundle.gateway),
        account_key: None,
        account_suffix: bundle.account_suffix,
        environment: BrokerEnvironment::Live,
    }
}

fn broker_account_suffix(config: &EngineConfig) -> Result<String, TradeEngineError> {
    match config.broker.provider {
        BrokerProviderKind::Kis => {
            let account_key = config.kis.account_key()?;
            Ok(EngineConfig::account_suffix_from_account_key(
                account_key.as_str(),
            ))
        }
        BrokerProviderKind::Toss => Ok(config.toss.account_seq()?.as_u64().to_string()),
    }
}

async fn build_write_gateway_after_doctor_preflight(
    config: &EngineConfig,
    watchlist: &[Symbol],
) -> Result<(KisGatewayBundle, StartupDoctorRun), TradeEngineError> {
    let preflight_context = StartupDoctorContext {
        account_suffix: UNKNOWN_STARTUP_ACCOUNT_SUFFIX,
        watchlist,
    };
    let doctor_run = start_write_startup_doctor(config, preflight_context).await?;
    match build_write_enabled_kis_gateway(config).await {
        Ok(bundle) => Ok((bundle, doctor_run)),
        Err(error) => {
            record_startup_doctor_build_failure(config, preflight_context, doctor_run, &error)?;
            Err(error)
        }
    }
}

fn ensure_order_submission_runtime_readiness(
    config: &EngineConfig,
    _bundle: &KisGatewayBundle,
) -> Result<(), TradeEngineError> {
    if config.broker_environment() != BrokerEnvironment::Live {
        return Ok(());
    }
    let db = MainDb::open(config.main_db_path()?)?;
    if db.panic_stop_active()? {
        return Err(TradeEngineError::runtime(
            "order submission runtime blocked by active panic stop",
        ));
    }
    Ok(())
}

fn spawn_ctrl_c_shutdown_listener(shutdown: RuntimeShutdownToken) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            shutdown.request_shutdown();
        }
    });
}

fn validate_run_config(config: &EngineConfig, args: &RunArgs) -> Result<(), TradeEngineError> {
    if args.dry_run == args.submit_orders {
        return Err(TradeEngineError::config(
            "run requires exactly one of --dry-run or --submit-orders",
        ));
    }
    if args.submit_orders {
        return config.validate_write_runtime_safety();
    }
    config.validate_read_only_smoke()?;
    if config.storage.main_db_path.is_none() {
        return Err(TradeEngineError::config(
            "decision pipeline run requires storage.main_db_path",
        ));
    }
    Ok(())
}

async fn run_doctor_with_gateway<G>(
    config: &EngineConfig,
    gateway: &G,
    account_suffix: &str,
    watchlist: &[Symbol],
    doctor_run: StartupDoctorRun,
) -> Result<String, TradeEngineError>
where
    G: TradingGateway + ?Sized,
{
    let report = complete_read_only_startup_doctor(
        config,
        gateway,
        StartupDoctorContext {
            account_suffix,
            watchlist,
        },
        doctor_run,
    )
    .await?;
    Ok(report.render())
}

async fn resolve_runtime_market_hours<G>(
    gateway: &G,
    market_scope: &crate::gateway::types::MarketScope,
) -> Result<RuntimeMarketHours, TradeEngineError>
where
    G: TradingGateway + ?Sized,
{
    let now_utc = OffsetDateTime::now_utc();
    let profile = MarketProfile::new(market_scope.clone());
    let trading_day = profile.trading_day_string_at(now_utc);
    let session = gateway
        .get_market_session(&trading_day)
        .await
        .map_err(|source| TradeEngineError::gateway("market_session", source))?;
    runtime_market_hours_for_session_at(&session, &profile, now_utc)
}

fn runtime_market_hours_for_session_at(
    session: &MarketSession,
    profile: &MarketProfile,
    now_utc: OffsetDateTime,
) -> Result<RuntimeMarketHours, TradeEngineError> {
    let expected_trading_day = profile.trading_day_string_at(now_utc);
    if session.trading_day != expected_trading_day {
        return Err(TradeEngineError::config(format!(
            "market session response day {} does not match requested trading day {expected_trading_day}",
            session.trading_day
        )));
    }
    if !session.is_open {
        return Ok(RuntimeMarketHours::OffHours);
    }
    match profile.scope() {
        crate::gateway::types::MarketScope::DomesticStock => {
            if profile.is_regular_session_time_at(now_utc) {
                Ok(RuntimeMarketHours::Regular)
            } else {
                Ok(RuntimeMarketHours::OffHours)
            }
        }
        crate::gateway::types::MarketScope::OverseasStock { .. } => Ok(RuntimeMarketHours::Regular),
    }
}

pub fn render_error(error: &TradeEngineError) -> String {
    format!(
        "status=failed error=\"{}\"",
        escape_log_value(&error.to_string())
    )
}

fn escape_log_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::*;
    use rust_decimal::Decimal;
    use tempfile::tempdir;
    use time::{Date, Month, Time};
    use tokio::time::timeout;
    use trading_agent_kis_client::journal::{BrokerCommandJournal, BrokerCommandRecord};
    use trading_agent_kis_client::{
        AccountKey, ClientOrderId, CreateOrderRequest, OrderSide, Quantity, RequestId,
    };

    #[test]
    fn run_requires_dry_run_flag() {
        let config = EngineConfig {
            read_only: true,
            watchlist: vec!["005930".to_owned()],
            broker: Default::default(),
            market: Default::default(),
            kis: Default::default(),
            toss: Default::default(),
            storage: crate::config::StorageConfig {
                main_db_path: Some(PathBuf::from("main.db")),
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
        let args = RunArgs {
            dry_run: false,
            submit_orders: false,
            once: true,
            config: PathBuf::from("config.toml"),
            max_market_ticks: None,
        };
        let error = validate_run_config(&config, &args).expect_err("dry-run flag is required");
        assert!(error.to_string().contains("--dry-run"));
    }

    #[test]
    fn dry_run_requires_main_db() {
        let mut config = EngineConfig {
            read_only: true,
            watchlist: vec!["005930".to_owned()],
            broker: Default::default(),
            market: Default::default(),
            kis: Default::default(),
            toss: Default::default(),
            storage: Default::default(),
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
        let args = RunArgs {
            dry_run: true,
            submit_orders: false,
            once: false,
            config: PathBuf::from("config.toml"),
            max_market_ticks: None,
        };

        assert!(validate_run_config(&config, &args).is_err());

        config.storage.main_db_path = Some(PathBuf::from("main.db"));
        validate_run_config(&config, &args).expect("dry-run config");
    }

    #[test]
    fn runtime_market_hours_uses_market_session_and_market_profile_window() {
        let profile = MarketProfile::new(crate::gateway::types::MarketScope::DomesticStock);
        let session = MarketSession {
            trading_day: "20260424".to_owned(),
            is_open: true,
        };
        assert_eq!(
            runtime_market_hours_for_session_at(
                &session,
                &profile,
                utc_datetime(2026, Month::April, 24, 1, 0, 0)
            )
            .expect("regular session"),
            RuntimeMarketHours::Regular
        );
        assert_eq!(
            runtime_market_hours_for_session_at(
                &session,
                &profile,
                utc_datetime(2026, Month::April, 24, 7, 0, 0)
            )
            .expect("after regular session"),
            RuntimeMarketHours::OffHours
        );

        let closed_session = MarketSession {
            trading_day: "20260424".to_owned(),
            is_open: false,
        };
        assert_eq!(
            runtime_market_hours_for_session_at(
                &closed_session,
                &profile,
                utc_datetime(2026, Month::April, 24, 1, 0, 0)
            )
            .expect("closed trading day"),
            RuntimeMarketHours::OffHours
        );
    }

    #[test]
    fn overseas_runtime_market_hours_uses_gateway_session_open_state() {
        let profile = MarketProfile::new(crate::gateway::types::MarketScope::OverseasStock {
            exchange: trading_agent_kis_client::ExchangeCode::new("NASD").expect("exchange"),
            currency: trading_agent_kis_client::CurrencyCode::new("USD").expect("currency"),
        });
        let daytime = utc_datetime(2026, Month::April, 24, 1, 0, 0);
        let premarket = utc_datetime(2026, Month::April, 24, 9, 0, 0);
        let aftermarket = utc_datetime(2026, Month::April, 24, 21, 0, 0);

        for now in [daytime, premarket, aftermarket] {
            let session = MarketSession {
                trading_day: profile.trading_day_string_at(now),
                is_open: true,
            };
            assert_eq!(
                runtime_market_hours_for_session_at(&session, &profile, now)
                    .expect("active overseas session"),
                RuntimeMarketHours::Regular
            );
        }
    }

    #[test]
    fn market_session_day_must_match_requested_trading_day() {
        let session = MarketSession {
            trading_day: "20260425".to_owned(),
            is_open: true,
        };
        let error = runtime_market_hours_for_session_at(
            &session,
            &MarketProfile::new(crate::gateway::types::MarketScope::DomesticStock),
            utc_datetime(2026, Month::April, 24, 1, 0, 0),
        )
        .expect_err("mismatched market session day");

        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn runner_args_parse_single_run_command() {
        use clap::Parser as _;

        let args = RunnerArgs::try_parse_from([
            "trade-engine",
            "run",
            "--dry-run",
            "--once",
            "--config",
            "custom.toml",
        ])
        .expect("parse run command");

        assert_eq!(args.config_path(), Path::new("custom.toml"));
        assert_eq!(
            args.stdout_render_options(),
            StdoutRenderOptions {
                format: crate::stdout::OutputFormat::Pretty,
                color: ColorMode::Auto,
            }
        );
        let RunnerCommand::Run(run_args) = args.command else {
            panic!("expected run command");
        };
        assert!(run_args.dry_run);
        assert!(run_args.once);
    }

    #[test]
    fn runner_args_parse_pretty_stdout_options() {
        use clap::Parser as _;

        let args = RunnerArgs::try_parse_from([
            "trade-engine",
            "--output",
            "pretty",
            "--color",
            "never",
            "run",
            "--dry-run",
            "--once",
        ])
        .expect("parse pretty output options");

        assert_eq!(
            args.stdout_render_options(),
            StdoutRenderOptions {
                format: crate::stdout::OutputFormat::Pretty,
                color: ColorMode::Never,
            }
        );
    }

    #[test]
    fn runner_args_parse_submit_orders_command() {
        use clap::Parser as _;

        let args = RunnerArgs::try_parse_from([
            "trade-engine",
            "run",
            "--submit-orders",
            "--once",
            "--config",
            "custom.toml",
        ])
        .expect("parse submit orders command");

        assert_eq!(args.config_path(), Path::new("custom.toml"));
        let RunnerCommand::Run(run_args) = args.command else {
            panic!("expected run command");
        };
        assert!(run_args.submit_orders);
        assert!(run_args.once);
    }

    #[test]
    fn runner_args_parse_universe_command() {
        use clap::Parser as _;

        let args = RunnerArgs::try_parse_from([
            "trade-engine",
            "universe",
            "--loop",
            "--max-refreshes",
            "2",
            "--config",
            "custom.toml",
        ])
        .expect("parse universe command");

        assert_eq!(args.config_path(), Path::new("custom.toml"));
        let RunnerCommand::Universe(universe_args) = args.command else {
            panic!("expected universe command");
        };
        assert!(universe_args.loop_forever);
        assert_eq!(universe_args.max_refreshes, Some(2));
    }

    #[test]
    fn runner_args_parse_status_command() {
        use clap::Parser as _;

        let args =
            RunnerArgs::try_parse_from(["trade-engine", "status", "--config", "custom.toml"])
                .expect("parse status command");

        assert_eq!(args.config_path(), Path::new("custom.toml"));
        let RunnerCommand::Status(_) = args.command else {
            panic!("expected status command");
        };
    }

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
            kis: crate::config::KisConfig {
                account_key: Some("12345678-01".to_owned()),
                ..Default::default()
            },
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

        assert!(output.contains("Trade Engine Status"));
        assert!(output.contains("purpose: read-only local runtime snapshot"));
        assert!(output.contains("Runtime readiness"));
        assert!(output.contains("account_suffix: 5678"));
        assert!(output.contains("database: migrations 1..12 (12 applied)"));
        assert!(output.contains("broker_commands: clear"));
        assert!(output.contains("panic_stop: inactive"));
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

    #[tokio::test]
    async fn query_command_rejects_blank_prompt_before_provider() {
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
        let args = RunnerArgs::try_parse_from(["trade-engine", "query", "-p", "   "])
            .expect("parse query");

        let error = run(config, args)
            .await
            .expect_err("blank prompt should fail");

        assert!(
            error
                .to_string()
                .contains("query --prompt must not be empty")
        );
    }

    #[test]
    fn runner_args_parse_backtest_command() {
        use clap::Parser as _;

        let args = RunnerArgs::try_parse_from([
            "trade-engine",
            "backtest",
            "--from",
            "2026-01-01",
            "--to",
            "2026-01-31",
            "--symbols",
            "005930,000660",
            "--initial-cash",
            "1000000",
            "--quantity-per-trade",
            "2",
            "--config",
            "custom.toml",
        ])
        .expect("parse backtest command");

        assert_eq!(args.config_path(), Path::new("custom.toml"));
        let RunnerCommand::Backtest(backtest_args) = args.command else {
            panic!("expected backtest command");
        };
        assert_eq!(backtest_args.from, "2026-01-01");
        assert_eq!(backtest_args.to, "2026-01-31");
        assert_eq!(backtest_args.symbols, vec!["005930", "000660"]);
        assert_eq!(backtest_args.initial_cash, "1000000");
        assert_eq!(backtest_args.quantity_per_trade, 2);
    }

    #[test]
    fn universe_refresh_scheduler_uses_market_profile_window_and_delay() {
        let settings = DryRunRuntimeSettings::with_risk(
            crate::config::RuntimeConfig::default(),
            crate::config::StrategyConfig::default(),
            crate::config::DecisionConfig::default(),
            crate::config::RiskConfig::default(),
            true,
        )
        .with_universe_discovery(crate::config::UniverseDiscoveryConfig {
            provider: crate::config::UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec!["trade-engine/scripts/codex-universe".to_owned()],
            max_symbols: 5,
            timeout_seconds: 5,
            refresh_seconds: 300,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });
        let context = UniverseRefreshContext {
            environment: trading_agent_kis_client::BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: crate::gateway::types::MarketScope::DomesticStock,
        };
        let scheduler = UniverseRefreshScheduler::new(&settings, &context);

        assert!(!scheduler.refresh_window_open_at(utc_datetime(2026, Month::April, 23, 23, 29, 0)));
        assert_eq!(
            scheduler.next_refresh_delay_at(utc_datetime(2026, Month::April, 23, 23, 29, 0)),
            Duration::from_secs(60)
        );
        assert!(scheduler.refresh_window_open_at(utc_datetime(2026, Month::April, 23, 23, 30, 0)));
        assert_eq!(scheduler.refresh_interval(), Duration::from_secs(300));
    }

    #[test]
    fn universe_refresh_scheduler_starts_next_refresh_before_cache_expiry() {
        let settings = DryRunRuntimeSettings::with_risk(
            crate::config::RuntimeConfig::default(),
            crate::config::StrategyConfig::default(),
            crate::config::DecisionConfig::default(),
            crate::config::RiskConfig::default(),
            true,
        )
        .with_universe_discovery(crate::config::UniverseDiscoveryConfig {
            provider: crate::config::UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec!["trade-engine/scripts/codex-universe".to_owned()],
            max_symbols: 5,
            timeout_seconds: 600,
            refresh_seconds: 7200,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });
        let context = UniverseRefreshContext {
            environment: trading_agent_kis_client::BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: crate::gateway::types::MarketScope::DomesticStock,
        };
        let scheduler = UniverseRefreshScheduler::new(&settings, &context);

        assert_eq!(scheduler.refresh_interval(), Duration::from_secs(7200));
        assert_eq!(
            scheduler.successive_refresh_delay(),
            Duration::from_secs(6570)
        );
    }

    #[test]
    fn universe_refresh_scheduler_keeps_a_minimum_retry_delay_for_short_intervals() {
        let settings = DryRunRuntimeSettings::with_risk(
            crate::config::RuntimeConfig::default(),
            crate::config::StrategyConfig::default(),
            crate::config::DecisionConfig::default(),
            crate::config::RiskConfig::default(),
            true,
        )
        .with_universe_discovery(crate::config::UniverseDiscoveryConfig {
            provider: crate::config::UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec!["trade-engine/scripts/codex-universe".to_owned()],
            max_symbols: 5,
            timeout_seconds: 120,
            refresh_seconds: 60,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });
        let context = UniverseRefreshContext {
            environment: trading_agent_kis_client::BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: crate::gateway::types::MarketScope::DomesticStock,
        };
        let scheduler = UniverseRefreshScheduler::new(&settings, &context);

        assert_eq!(
            scheduler.successive_refresh_delay(),
            Duration::from_secs(30)
        );
    }

    #[tokio::test]
    async fn auto_universe_refresh_once_records_snapshot_for_runtime_cache() {
        let tempdir = tempdir().expect("tempdir");
        let db_path = tempdir.path().join("main.db");
        let script_path = tempdir.path().join("codex-universe.sh");
        std::fs::write(
            &script_path,
            r#"printf '%s\n' '{"summary":"auto refresh","candidates":[{"symbol":"000660","reason":"test candidate","confidence":"0.70"}]}'"#,
        )
        .expect("write universe script");
        let db = BlockingMainDb::open(&db_path).expect("open db");
        let settings = DryRunRuntimeSettings::with_risk(
            crate::config::RuntimeConfig::default(),
            crate::config::StrategyConfig {
                id: "hold-only".to_owned(),
                short_window: None,
                long_window: None,
            },
            crate::config::DecisionConfig::default(),
            crate::config::RiskConfig::default(),
            true,
        )
        .with_universe_discovery(crate::config::UniverseDiscoveryConfig {
            provider: crate::config::UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            max_symbols: 5,
            timeout_seconds: 5,
            refresh_seconds: 60,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });
        let context = UniverseRefreshContext {
            environment: trading_agent_kis_client::BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: crate::gateway::types::MarketScope::DomesticStock,
        };
        let watchlist = vec![Symbol::new("005930").expect("symbol")];
        let mut output = String::new();

        run_auto_universe_refresh_once_at(
            &db,
            &settings,
            &context,
            &watchlist,
            &mut output,
            None,
            utc_datetime(2026, Month::April, 24, 0, 0, 0),
        )
        .await
        .expect("auto universe refresh");

        assert!(output.contains("Universe refresh"));
        assert!(output.contains("status: parsed"));
        let snapshot = MainDb::open(&db_path)
            .expect("open main db")
            .latest_universe_snapshot(trading_agent_kis_client::BrokerEnvironment::Paper, "5678")
            .expect("latest snapshot")
            .expect("snapshot recorded");
        assert_eq!(snapshot.output.candidates[0].symbol.as_str(), "000660");
    }

    #[tokio::test]
    async fn auto_universe_refresh_once_runs_outside_preopen_window() {
        let tempdir = tempdir().expect("tempdir");
        let db_path = tempdir.path().join("main.db");
        let request_path = tempdir.path().join("universe-request.json");
        let script_path = tempdir.path().join("codex-universe.sh");
        std::fs::write(
            &script_path,
            format!(
                r#"cat > "{}"
printf '%s\n' '{{"summary":"auto refresh","candidates":[{{"symbol":"000660","reason":"test candidate","confidence":"0.70"}}]}}'
"#,
                request_path.display()
            ),
        )
        .expect("write universe script");
        let db = BlockingMainDb::open(&db_path).expect("open db");
        let settings = DryRunRuntimeSettings::with_risk(
            crate::config::RuntimeConfig::default(),
            crate::config::StrategyConfig {
                id: "hold-only".to_owned(),
                short_window: None,
                long_window: None,
            },
            crate::config::DecisionConfig::default(),
            crate::config::RiskConfig::default(),
            true,
        )
        .with_universe_discovery(crate::config::UniverseDiscoveryConfig {
            provider: crate::config::UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            max_symbols: 5,
            timeout_seconds: 5,
            refresh_seconds: 60,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });
        let context = UniverseRefreshContext {
            environment: trading_agent_kis_client::BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: crate::gateway::types::MarketScope::DomesticStock,
        };
        let watchlist = vec![Symbol::new("005930").expect("symbol")];
        let mut output = String::new();

        run_auto_universe_refresh_once_at(
            &db,
            &settings,
            &context,
            &watchlist,
            &mut output,
            None,
            utc_datetime(2026, Month::April, 23, 23, 29, 0),
        )
        .await
        .expect("auto universe refresh");

        assert!(output.contains("Universe refresh"));
        assert!(
            request_path.exists(),
            "main-loop universe refresh should invoke the provider outside preopen"
        );
        let snapshot = MainDb::open(&db_path)
            .expect("open main db")
            .latest_universe_snapshot(trading_agent_kis_client::BrokerEnvironment::Paper, "5678")
            .expect("latest snapshot")
            .expect("snapshot recorded");
        assert_eq!(snapshot.output.candidates[0].symbol.as_str(), "000660");
    }

    #[tokio::test]
    async fn auto_universe_refresh_loop_starts_first_refresh_immediately() {
        let tempdir = tempdir().expect("tempdir");
        let db_path = tempdir.path().join("main.db");
        let request_path = tempdir.path().join("universe-request.json");
        let script_path = tempdir.path().join("codex-universe.sh");
        std::fs::write(
            &script_path,
            format!(
                r#"cat > "{}"
printf '%s\n' '{{"summary":"loop refresh","candidates":[{{"symbol":"000660","reason":"test candidate","confidence":"0.70"}}]}}'
"#,
                request_path.display()
            ),
        )
        .expect("write universe script");
        BlockingMainDb::open(&db_path).expect("open db");
        let settings = DryRunRuntimeSettings::with_risk(
            crate::config::RuntimeConfig::default(),
            crate::config::StrategyConfig {
                id: "hold-only".to_owned(),
                short_window: None,
                long_window: None,
            },
            crate::config::DecisionConfig::default(),
            crate::config::RiskConfig::default(),
            true,
        )
        .with_universe_discovery(crate::config::UniverseDiscoveryConfig {
            provider: crate::config::UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            max_symbols: 5,
            timeout_seconds: 5,
            refresh_seconds: 3600,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });
        let context = UniverseRefreshContext {
            environment: trading_agent_kis_client::BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: crate::gateway::types::MarketScope::DomesticStock,
        };
        let watchlist = vec![Symbol::new("005930").expect("symbol")];
        let shutdown = RuntimeShutdownToken::new();
        let handle = spawn_auto_universe_refresh_loop(
            db_path.clone(),
            settings,
            context,
            watchlist,
            shutdown.clone(),
        )
        .expect("spawn universe refresh loop");

        timeout(Duration::from_secs(2), async {
            loop {
                if request_path.exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("first refresh should start without waiting for refresh_seconds");

        timeout(Duration::from_secs(2), async {
            loop {
                let snapshot = MainDb::open(&db_path)
                    .expect("open main db")
                    .latest_universe_snapshot(
                        trading_agent_kis_client::BrokerEnvironment::Paper,
                        "5678",
                    )
                    .expect("latest snapshot");
                if snapshot.is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("first refresh should record a snapshot");

        stop_auto_universe_refresh_loop(&shutdown, Some(handle))
            .await
            .expect("stop universe refresh loop");
    }

    #[tokio::test]
    async fn auto_universe_refresh_once_skips_when_configured_out_of_band() {
        let tempdir = tempdir().expect("tempdir");
        let db_path = tempdir.path().join("main.db");
        let request_path = tempdir.path().join("universe-request.json");
        let script_path = tempdir.path().join("codex-universe.sh");
        std::fs::write(
            &script_path,
            format!(
                r#"cat > "{}"
printf '%s\n' '{{"summary":"auto refresh","candidates":[{{"symbol":"000660","reason":"test candidate","confidence":"0.70"}}]}}'
"#,
                request_path.display()
            ),
        )
        .expect("write universe script");
        let db = BlockingMainDb::open(&db_path).expect("open db");
        let settings = DryRunRuntimeSettings::with_risk(
            crate::config::RuntimeConfig::default(),
            crate::config::StrategyConfig {
                id: "hold-only".to_owned(),
                short_window: None,
                long_window: None,
            },
            crate::config::DecisionConfig::default(),
            crate::config::RiskConfig::default(),
            true,
        )
        .with_universe_discovery(crate::config::UniverseDiscoveryConfig {
            provider: crate::config::UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            max_symbols: 5,
            timeout_seconds: 5,
            refresh_seconds: 60,
            auto_refresh: false,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });
        let context = UniverseRefreshContext {
            environment: trading_agent_kis_client::BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: crate::gateway::types::MarketScope::DomesticStock,
        };
        let watchlist = vec![Symbol::new("005930").expect("symbol")];
        let mut output = String::new();

        run_auto_universe_refresh_once_at(
            &db,
            &settings,
            &context,
            &watchlist,
            &mut output,
            None,
            utc_datetime(2026, Month::April, 24, 0, 0, 0),
        )
        .await
        .expect("auto universe refresh skip");

        assert!(output.is_empty());
        assert!(
            !request_path.exists(),
            "out-of-band universe refresh config must not invoke the auto refresh command"
        );
        assert!(
            MainDb::open(&db_path)
                .expect("open main db")
                .latest_universe_snapshot(
                    trading_agent_kis_client::BrokerEnvironment::Paper,
                    "5678"
                )
                .expect("latest snapshot")
                .is_none()
        );
    }

    #[tokio::test]
    async fn auto_universe_refresh_once_stops_when_shutdown_is_requested() {
        let tempdir = tempdir().expect("tempdir");
        let db_path = tempdir.path().join("main.db");
        let script_path = tempdir.path().join("slow-codex-universe.sh");
        std::fs::write(&script_path, "while :; do :; done\n").expect("write universe script");
        let db = BlockingMainDb::open(&db_path).expect("open db");
        let settings = DryRunRuntimeSettings::with_risk(
            crate::config::RuntimeConfig::default(),
            crate::config::StrategyConfig {
                id: "hold-only".to_owned(),
                short_window: None,
                long_window: None,
            },
            crate::config::DecisionConfig::default(),
            crate::config::RiskConfig::default(),
            true,
        )
        .with_universe_discovery(crate::config::UniverseDiscoveryConfig {
            provider: crate::config::UniverseDiscoveryProviderKind::Command,
            command: Some(PathBuf::from("sh")),
            args: vec![script_path.to_string_lossy().into_owned()],
            max_symbols: 5,
            timeout_seconds: 30,
            refresh_seconds: 60,
            auto_refresh: true,
            max_stderr_bytes: 4096,
            max_stdout_bytes: 262_144,
        });
        let context = UniverseRefreshContext {
            environment: trading_agent_kis_client::BrokerEnvironment::Paper,
            account_suffix: "5678".to_owned(),
            market_scope: crate::gateway::types::MarketScope::DomesticStock,
        };
        let watchlist = vec![Symbol::new("005930").expect("symbol")];
        let shutdown = RuntimeShutdownToken::new();
        let shutdown_for_task = shutdown.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            shutdown_for_task.request_shutdown();
        });
        let mut output = String::new();

        let completed = timeout(Duration::from_secs(2), async {
            tokio::select! {
                result = run_auto_universe_refresh_once_at(
                    &db,
                    &settings,
                    &context,
                    &watchlist,
                    &mut output,
                    None,
                    utc_datetime(2026, Month::April, 24, 0, 0, 0),
                ) => {
                    result?;
                    Ok::<bool, TradeEngineError>(true)
                }
                _ = wait_for_runtime_shutdown(shutdown) => Ok(false),
            }
        })
        .await
        .expect("shutdown should not wait for universe command timeout")
        .expect("refresh shutdown");

        assert!(!completed);
        assert!(output.is_empty());
    }

    #[test]
    fn runner_args_reject_dry_run_and_submit_orders_together() {
        use clap::Parser as _;

        let args =
            RunnerArgs::try_parse_from(["trade-engine", "run", "--dry-run", "--submit-orders"])
                .expect("parse run command");
        let RunnerCommand::Run(run_args) = args.command else {
            panic!("expected run command");
        };
        let config = EngineConfig {
            read_only: true,
            watchlist: vec!["005930".to_owned()],
            broker: Default::default(),
            market: Default::default(),
            kis: Default::default(),
            toss: Default::default(),
            storage: crate::config::StorageConfig {
                main_db_path: Some(PathBuf::from("main.db")),
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

        let error = validate_run_config(&config, &run_args).expect_err("run modes are exclusive");
        assert!(error.to_string().contains("exactly one"));
    }

    #[test]
    fn runner_args_parse_doctor_command() {
        use clap::Parser as _;

        let args =
            RunnerArgs::try_parse_from(["trade-engine", "doctor", "--config", "custom.toml"])
                .expect("parse doctor command");

        assert_eq!(args.config_path(), Path::new("custom.toml"));
        let RunnerCommand::Doctor(doctor_args) = args.command else {
            panic!("expected doctor command");
        };
        assert_eq!(doctor_args.config, PathBuf::from("custom.toml"));
    }

    #[test]
    fn runner_args_reject_manual_market_hours() {
        use clap::Parser as _;

        assert!(
            RunnerArgs::try_parse_from([
                "trade-engine",
                "run",
                "--dry-run",
                "--market-hours",
                "regular",
            ])
            .is_err()
        );
    }

    #[test]
    fn runner_args_reject_removed_modes() {
        use clap::Parser as _;

        assert!(RunnerArgs::try_parse_from(["trade-engine", "run", "--paper", "--once",]).is_err());
        assert!(
            RunnerArgs::try_parse_from(["trade-engine", "run", "--read-only", "--once",]).is_err()
        );
    }

    #[tokio::test]
    async fn doctor_command_runs_db_preflight_before_kis_gateway_build() {
        let tempdir = tempdir().expect("tempdir");
        let db_path = tempdir.path().join("main.db");
        seed_unresolved_command(&db_path).await;
        let config = doctor_config_with_missing_env(
            db_path.clone(),
            tempdir.path().join("token.json"),
            "DOCTOR",
        );

        let error = doctor_command(config)
            .await
            .expect_err("unresolved journal should block before KIS env lookup");

        assert!(
            error.to_string().contains("unresolved broker command"),
            "unexpected error: {error}"
        );
        assert_eq!(startup_doctor_audit_count(&db_path), 1);
    }

    #[tokio::test]
    async fn doctor_command_audits_kis_gateway_build_failure_after_preflight() {
        let tempdir = tempdir().expect("tempdir");
        let db_path = tempdir.path().join("main.db");
        let config = doctor_config_with_missing_env(
            db_path.clone(),
            tempdir.path().join("token.json"),
            "DOCTOR_BUILD",
        );

        let error = doctor_command(config)
            .await
            .expect_err("missing KIS env should fail gateway build");

        assert!(
            error
                .to_string()
                .contains("__TRADE_ENGINE_TEST_MISSING_APP_KEY_DOCTOR_BUILD__"),
            "unexpected error: {error}"
        );
        assert_eq!(startup_doctor_audit_count(&db_path), 1);
    }

    #[test]
    fn backtest_command_reads_stored_bars_without_kis_env_or_doctor() {
        let tempdir = tempdir().expect("tempdir");
        let db_path = tempdir.path().join("main.db");
        seed_backtest_bars(&db_path);
        let mut config = doctor_config_with_missing_env(
            db_path.clone(),
            tempdir.path().join("token.json"),
            "BACKTEST",
        );
        config.strategy = crate::config::StrategyConfig {
            id: "sma-crossover-v1".to_owned(),
            short_window: Some(2),
            long_window: Some(3),
        };
        let args = BacktestArgs {
            config: PathBuf::from("config.toml"),
            from: "2026-01-01".to_owned(),
            to: "2026-01-04".to_owned(),
            symbols: Vec::new(),
            initial_cash: "1000".to_owned(),
            quantity_per_trade: 1,
        };

        let output = backtest_command(config, args).expect("backtest command");

        assert!(output.contains("Backtest"));
        assert!(output.contains("symbol: 005930"));
        assert!(output.contains("bars: 4"));
        assert!(output.contains("ending_equity: 1000"));
        assert!(output.contains("buy_count: 1"));
        assert_eq!(startup_doctor_audit_count(&db_path), 0);
    }

    fn doctor_config_with_missing_env(
        db_path: PathBuf,
        token_cache_path: PathBuf,
        suffix: &str,
    ) -> EngineConfig {
        EngineConfig {
            read_only: true,
            watchlist: vec!["005930".to_owned()],
            broker: Default::default(),
            market: Default::default(),
            kis: crate::config::KisConfig {
                environment: crate::config::KisEnvironment::Paper,
                token_cache_path,
                app_key_env: format!("__TRADE_ENGINE_TEST_MISSING_APP_KEY_{suffix}__"),
                app_secret_env: format!("__TRADE_ENGINE_TEST_MISSING_APP_SECRET_{suffix}__"),
                account_key: None,
                account_key_env: format!("__TRADE_ENGINE_TEST_MISSING_ACCOUNT_KEY_{suffix}__"),
            },
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
        }
    }

    fn seed_backtest_bars(path: &Path) {
        let mut db = MainDb::open(path).expect("open main db");
        for (day, close) in [(1, 100), (2, 90), (3, 90), (4, 130)] {
            db.record_ohlcv_bar(&crate::storage::NewOhlcvBarRecord {
                observed_at: utc_datetime(2026, Month::January, day, 0, 0, 0),
                source: "runner-test".to_owned(),
                environment: crate::config::KisEnvironment::Paper.as_broker_environment(),
                market_scope: crate::gateway::types::MarketScope::DomesticStock,
                interval: crate::technical_analysis::OhlcvBarInterval::Daily,
                bar_start_at: utc_datetime(2026, Month::January, day, 0, 0, 0),
                bar_end_at: utc_datetime(2026, Month::January, day, 23, 59, 0),
                adjustment_policy: crate::technical_analysis::PriceAdjustmentPolicy::Raw,
                data_quality_flags: vec!["complete".to_owned()],
                bar: crate::technical_analysis::DailyOhlcvBar {
                    symbol: Symbol::new("005930").expect("symbol"),
                    trading_day: crate::gateway::types::TradingDay(
                        Date::from_calendar_date(2026, Month::January, day).expect("date"),
                    ),
                    open: Decimal::new(close - 1, 0),
                    high: Decimal::new(close + 1, 0),
                    low: Decimal::new(close - 2, 0),
                    close: Decimal::new(close, 0),
                    volume: Decimal::new(100, 0),
                },
            })
            .expect("record OHLCV bar");
        }
    }

    async fn seed_unresolved_command(path: &Path) {
        let journal = crate::storage::SqliteBrokerCommandJournal::open(path).expect("open journal");
        let client_order_id =
            ClientOrderId::new("runner-preflight-unresolved").expect("client order id");
        journal
            .reserve_command(
                BrokerCommandRecord::new_place_order(
                    crate::config::KisEnvironment::Paper.as_broker_environment(),
                    CreateOrderRequest::cash_limit(
                        client_order_id.clone(),
                        AccountKey::new("12345678-01").expect("account key"),
                        Symbol::new("005930").expect("symbol"),
                        OrderSide::Buy,
                        Quantity::new(1),
                        Decimal::new(73_000, 0),
                    )
                    .expect("order request"),
                )
                .expect("broker command"),
            )
            .await
            .expect("reserve command");
        journal
            .mark_submitting(
                &client_order_id,
                &RequestId::new("runner-preflight-request").expect("request id"),
            )
            .await
            .expect("mark submitting");
        journal
            .mark_reconcile_needed(&client_order_id, "runner preflight")
            .await
            .expect("mark reconcile needed");
    }

    fn startup_doctor_audit_count(path: &Path) -> i64 {
        let connection = rusqlite::Connection::open(path).expect("open db");
        connection
            .query_row(
                "SELECT COUNT(*)
                 FROM operator_actions
                 WHERE target_type = 'startup_doctor'",
                [],
                |row| row.get(0),
            )
            .expect("startup doctor audit count")
    }

    fn utc_datetime(
        year: i32,
        month: Month,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> OffsetDateTime {
        Date::from_calendar_date(year, month, day)
            .expect("valid date")
            .with_time(Time::from_hms(hour, minute, second).expect("valid time"))
            .assume_utc()
    }
}
