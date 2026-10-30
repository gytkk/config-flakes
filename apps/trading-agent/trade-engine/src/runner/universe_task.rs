use std::path::PathBuf;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::config::UniverseDiscoveryProviderKind;
use crate::error::TradeEngineError;
use crate::gateway::types::Symbol;
use crate::market::MarketProfile;
use crate::runtime::{
    DryRunRuntimeSettings, RuntimeShutdownToken, UniverseRefreshContext, refresh_universe_once,
};
use crate::storage::BlockingMainDb;
#[cfg(test)]
use crate::{runtime::RuntimeOutputSink, stdout::StructuredOutput};

const AUTO_UNIVERSE_REFRESH_MARGIN: Duration = Duration::from_secs(30);
const MIN_AUTO_UNIVERSE_REFRESH_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub(super) struct UniverseRefreshScheduler {
    refresh_interval: Duration,
    refresh_lead: Duration,
    market_profile: MarketProfile,
}

impl UniverseRefreshScheduler {
    pub(super) fn new(settings: &DryRunRuntimeSettings, context: &UniverseRefreshContext) -> Self {
        Self {
            refresh_interval: settings.universe_discovery.refresh_interval(),
            refresh_lead: Duration::from_secs(
                settings
                    .universe_discovery
                    .timeout_seconds
                    .saturating_add(AUTO_UNIVERSE_REFRESH_MARGIN.as_secs()),
            ),
            market_profile: MarketProfile::new(context.market_scope.clone()),
        }
    }

    #[cfg(test)]
    pub(super) fn refresh_interval(&self) -> Duration {
        self.refresh_interval
    }

    pub(super) fn successive_refresh_delay(&self) -> Duration {
        if self.refresh_interval <= MIN_AUTO_UNIVERSE_REFRESH_DELAY {
            return self.refresh_interval;
        }
        let max_lead = self
            .refresh_interval
            .saturating_sub(MIN_AUTO_UNIVERSE_REFRESH_DELAY);
        self.refresh_interval
            .saturating_sub(self.refresh_lead.min(max_lead))
    }

    pub(super) fn refresh_window_open_at(&self, now_utc: OffsetDateTime) -> bool {
        self.market_profile
            .auto_universe_refresh_window_open_at(now_utc)
    }

    pub(super) fn next_refresh_delay_at(&self, now_utc: OffsetDateTime) -> Duration {
        self.market_profile
            .next_auto_universe_refresh_delay(now_utc, self.refresh_interval)
    }
}

#[cfg(test)]
pub(super) async fn run_auto_universe_refresh_once_at(
    db: &BlockingMainDb,
    settings: &DryRunRuntimeSettings,
    context: &UniverseRefreshContext,
    watchlist: &[Symbol],
    output: &mut String,
    output_sink: Option<&mut dyn RuntimeOutputSink>,
    _now_utc: OffsetDateTime,
) -> Result<(), TradeEngineError> {
    if !auto_universe_refresh_enabled(settings) {
        return Ok(());
    }

    let rendered = match refresh_universe_once(db, settings, context, watchlist).await {
        Ok(refresh) => refresh.render(),
        Err(error) => render_auto_universe_refresh_error(&error),
    };
    if let Some(sink) = output_sink {
        sink.write(&rendered)?;
    }
    output.push_str(&rendered);
    Ok(())
}

pub(super) fn spawn_auto_universe_refresh_loop(
    db_path: PathBuf,
    settings: DryRunRuntimeSettings,
    context: UniverseRefreshContext,
    watchlist: Vec<Symbol>,
    shutdown: RuntimeShutdownToken,
) -> Option<JoinHandle<()>> {
    if !auto_universe_refresh_enabled(&settings) {
        return None;
    }
    let scheduler = UniverseRefreshScheduler::new(&settings, &context);
    Some(tokio::spawn(async move {
        loop {
            if shutdown.is_shutdown_requested() {
                break;
            }
            let db = match BlockingMainDb::open(&db_path) {
                Ok(db) => db,
                Err(error) => {
                    tracing::warn!(
                        event = "auto_universe_refresh",
                        status = "failed",
                        error = %error
                    );
                    continue;
                }
            };
            tracing::info!(event = "auto_universe_refresh", status = "started");
            tokio::select! {
                result = refresh_universe_once(&db, &settings, &context, &watchlist) => {
                    match result {
                        Ok(refresh) => {
                            tracing::info!(
                                event = "auto_universe_refresh",
                                status = refresh.status.as_str(),
                                provider = refresh.provider.as_str(),
                                snapshot_id = refresh.snapshot_id.map(|id| id.as_i64()),
                                error = refresh.error.as_deref().unwrap_or("")
                            );
                        }
                        Err(error) => {
                            tracing::warn!(
                                event = "auto_universe_refresh",
                                status = "failed",
                                error = %error
                            );
                        }
                    }
                }
                _ = wait_for_runtime_shutdown(shutdown.clone()) => break,
            }
            tokio::select! {
                _ = sleep(scheduler.successive_refresh_delay()) => {}
                _ = wait_for_runtime_shutdown(shutdown.clone()) => break,
            }
        }
    }))
}

pub(super) async fn stop_auto_universe_refresh_loop(
    shutdown: &RuntimeShutdownToken,
    handle: Option<JoinHandle<()>>,
) -> Result<(), TradeEngineError> {
    let Some(handle) = handle else {
        return Ok(());
    };
    shutdown.request_shutdown();
    match timeout(Duration::from_secs(2), handle).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(TradeEngineError::runtime(format!(
            "auto universe refresh task failed: {error}"
        ))),
        Err(_) => Ok(()),
    }
}

pub(super) async fn wait_for_runtime_shutdown(shutdown: RuntimeShutdownToken) {
    while !shutdown.is_shutdown_requested() {
        sleep(Duration::from_millis(100)).await;
    }
}

fn auto_universe_refresh_enabled(settings: &DryRunRuntimeSettings) -> bool {
    settings.universe_discovery.auto_refresh
        && !matches!(
            settings.universe_discovery.provider,
            UniverseDiscoveryProviderKind::Disabled
        )
}

#[cfg(test)]
fn render_auto_universe_refresh_error(error: &TradeEngineError) -> String {
    let mut output = StructuredOutput::new();
    output.heading("Universe refresh");
    output
        .field("status", "failed")
        .field("provider", "auto")
        .field("error", error.to_string())
        .blank();
    output.finish()
}
