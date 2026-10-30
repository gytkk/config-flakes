use std::path::{Path, PathBuf};

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};

use crate::stdout::{ColorMode, OutputFormat, StdoutRenderOptions};

#[derive(Debug, Parser)]
#[command(name = "trade-engine")]
#[command(about = "Dry-run trading runtime")]
pub struct RunnerArgs {
    #[arg(long, value_enum, default_value = "pretty", global = true)]
    pub(super) output: OutputFormatArg,
    #[arg(long, value_enum, default_value = "auto", global = true)]
    pub(super) color: ColorArg,
    #[command(subcommand)]
    pub(super) command: RunnerCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum OutputFormatArg {
    Plain,
    Pretty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum ColorArg {
    Auto,
    Always,
    Never,
}

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
pub(super) struct BacktestArgs {
    #[arg(long, default_value = "configs/trade-engine.live-dry-run.toml")]
    pub(super) config: PathBuf,
    #[arg(long)]
    pub(super) from: String,
    #[arg(long)]
    pub(super) to: String,
    #[arg(long, value_delimiter = ',')]
    pub(super) symbols: Vec<String>,
    #[arg(long, default_value = "10000000")]
    pub(super) initial_cash: String,
    #[arg(long, default_value_t = 1)]
    pub(super) quantity_per_trade: u64,
}

#[derive(Debug, Args)]
pub(super) struct DoctorArgs {
    #[arg(long, default_value = "configs/trade-engine.live-dry-run.toml")]
    pub(super) config: PathBuf,
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

#[derive(Debug, Args)]
pub(super) struct RunArgs {
    #[arg(long, action = ArgAction::SetTrue)]
    pub(super) dry_run: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub(super) submit_orders: bool,
    #[arg(long, action = ArgAction::SetTrue)]
    pub(super) once: bool,
    #[arg(long, default_value = "configs/trade-engine.live-dry-run.toml")]
    pub(super) config: PathBuf,
    #[arg(long)]
    pub(super) max_market_ticks: Option<u64>,
}

#[derive(Debug, Args)]
pub(super) struct UniverseArgs {
    #[arg(long, default_value = "configs/trade-engine.live-dry-run.toml")]
    pub(super) config: PathBuf,
    #[arg(long = "loop", action = ArgAction::SetTrue)]
    pub(super) loop_forever: bool,
    #[arg(long)]
    pub(super) max_refreshes: Option<u64>,
}

impl RunnerArgs {
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

    pub(super) fn stdout_render_options(&self) -> StdoutRenderOptions {
        StdoutRenderOptions {
            format: match self.output {
                OutputFormatArg::Plain => OutputFormat::Plain,
                OutputFormatArg::Pretty => OutputFormat::Pretty,
            },
            color: match self.color {
                ColorArg::Auto => ColorMode::Auto,
                ColorArg::Always => ColorMode::Always,
                ColorArg::Never => ColorMode::Never,
            },
        }
    }
}
