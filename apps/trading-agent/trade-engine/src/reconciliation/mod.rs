use async_trait::async_trait;
use trading_agent_kis_client::journal::BrokerCommandJournal;
use trading_agent_kis_client::{BrokerError, ClientOrderId, ReconciliationResult};

use crate::error::TradeEngineError;
use crate::storage::{MainDb, StoredExecutionRequestId};

#[async_trait]
pub trait ReconciliationGateway: Send + Sync {
    async fn reconcile_order(
        &self,
        client_order_id: &ClientOrderId,
        lookback_days: u16,
    ) -> Result<ReconciliationResult, BrokerError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconciliationWorkflowConfig {
    pub lookback_days: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReconciliationOutcome {
    pub client_order_id: ClientOrderId,
    pub execution_request_id: Option<StoredExecutionRequestId>,
    pub result: ReconciliationResult,
}

pub async fn reconcile_pending_execution_requests<G, J>(
    gateway: &G,
    db: &mut MainDb,
    journal: &J,
    config: ReconciliationWorkflowConfig,
) -> Result<Vec<ExecutionReconciliationOutcome>, TradeEngineError>
where
    G: ReconciliationGateway + ?Sized,
    J: BrokerCommandJournal + ?Sized,
{
    let commands = journal.list_unreconciled().await?;
    let mut outcomes = Vec::with_capacity(commands.len());

    for command in commands {
        let result = gateway
            .reconcile_order(&command.client_order_id, config.lookback_days)
            .await
            .map_err(|source| TradeEngineError::gateway("reconcile_order", source))?;
        let execution_request_id =
            db.record_execution_reconciliation_result(&command.client_order_id, &result)?;
        outcomes.push(ExecutionReconciliationOutcome {
            client_order_id: command.client_order_id,
            execution_request_id,
            result,
        });
    }

    Ok(outcomes)
}
