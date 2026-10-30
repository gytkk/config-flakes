use clap::Parser;
use trade_engine::config::EngineConfig;
use trade_engine::error::TradeEngineError;
use trade_engine::runner::{self, RunnerArgs};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{}", runner::render_error(&error));
        std::process::exit(1);
    }
}

async fn run() -> Result<(), TradeEngineError> {
    let args = RunnerArgs::parse();
    let config = EngineConfig::load_from_path(args.config_path())?;
    runner::run_to_stdout(config, args).await
}
