use std::path::PathBuf;

use async_trait::async_trait;
use time::OffsetDateTime;
use trading_agent_kis_client::journal::BrokerCommandJournal;
use trading_agent_kis_client::{AccountKey, BrokerEnvironment, ReconciliationResult};

use crate::config::RiskConfig;
use crate::error::TradeEngineError;
use crate::execution::{
    ExecutionWorkflowConfig, FinalExecutionRiskInputs,
    submit_next_execution_request_for_run_with_policy,
};
use crate::gateway::traits::TradingGateway;
use crate::gateway::types::Symbol;
use crate::reconciliation::{
    ReconciliationGateway, ReconciliationWorkflowConfig, reconcile_pending_execution_requests,
};
use crate::runtime::{DryRunRunContext, DryRunRuntimeSettings, RuntimeTickExecutionDriver};
use crate::stdout::StructuredOutput;
use crate::storage::{MainDb, SqliteBrokerCommandJournal, StoredRuntimeRunId};

const RECONCILIATION_LOOKBACK_DAYS: u16 = 1;

#[derive(Debug, Clone)]
pub struct DbRuntimeExecutionDriver {
    main_db_path: PathBuf,
    account_key: AccountKey,
    environment: BrokerEnvironment,
    read_only: bool,
    risk: RiskConfig,
}

impl DbRuntimeExecutionDriver {
    pub fn new(
        main_db_path: PathBuf,
        account_key: AccountKey,
        environment: BrokerEnvironment,
        read_only: bool,
        risk: RiskConfig,
    ) -> Self {
        Self {
            main_db_path,
            account_key,
            environment,
            read_only,
            risk,
        }
    }
}

#[async_trait(?Send)]
impl<G> RuntimeTickExecutionDriver<G> for DbRuntimeExecutionDriver
where
    G: TradingGateway + ReconciliationGateway + Sync + ?Sized,
{
    async fn reconcile_before_tick(
        &self,
        gateway: &G,
        tick_sequence: u64,
    ) -> Result<String, TradeEngineError> {
        let mut db = MainDb::open(&self.main_db_path)?;
        let journal = SqliteBrokerCommandJournal::open(&self.main_db_path)?;
        let outcomes = reconcile_pending_execution_requests(
            gateway,
            &mut db,
            &journal,
            ReconciliationWorkflowConfig {
                lookback_days: RECONCILIATION_LOOKBACK_DAYS,
            },
        )
        .await?;

        if outcomes.is_empty() {
            return Ok(String::new());
        }

        let unresolved_count = journal.list_unreconciled().await?.len();
        let still_unknown_count = outcomes
            .iter()
            .filter(|outcome| {
                matches!(
                    outcome.result,
                    ReconciliationResult::StillUnknown
                        | ReconciliationResult::ManualInterventionRequired
                )
            })
            .count();
        let mut output = StructuredOutput::new();
        output
            .heading(format!("Reconciliation #{}", tick_sequence + 1))
            .field(
                "status",
                if unresolved_count == 0 {
                    "reconciled"
                } else {
                    "unresolved"
                },
            )
            .field("count", outcomes.len())
            .field("still_unknown_count", still_unknown_count)
            .field("unresolved_count", unresolved_count)
            .blank();
        Ok(output.finish())
    }

    async fn submit_after_tick(
        &self,
        gateway: &G,
        run_id: StoredRuntimeRunId,
        tick_sequence: u64,
        run_context: &DryRunRunContext,
        watchlist: &[Symbol],
        _settings: &DryRunRuntimeSettings,
    ) -> Result<String, TradeEngineError> {
        let mut db = MainDb::open(&self.main_db_path)?;
        let journal = SqliteBrokerCommandJournal::open(&self.main_db_path)?;
        let workflow_config = ExecutionWorkflowConfig {
            read_only: self.read_only,
            account_key: self.account_key.clone(),
            environment: self.environment,
            market_scope: run_context.market_scope.clone(),
        };
        let mut processed_count = 0_u64;

        loop {
            let risk_inputs = FinalExecutionRiskInputs {
                risk: &self.risk,
                read_only: self.read_only,
                market_scope: run_context.market_scope.clone(),
                allowed_symbols: watchlist,
                now: OffsetDateTime::now_utc(),
            };
            let submitted = submit_next_execution_request_for_run_with_policy(
                gateway,
                &mut db,
                &journal,
                run_id,
                &workflow_config,
                risk_inputs,
            )
            .await?;
            if submitted.is_none() {
                break;
            }
            processed_count += 1;
        }

        let mut output = StructuredOutput::new();
        output
            .heading(format!("Execution submissions #{}", tick_sequence + 1))
            .field(
                "status",
                if processed_count == 0 {
                    "none"
                } else {
                    "submitted"
                },
            )
            .field("count", processed_count)
            .blank();
        Ok(output.finish())
    }
}
