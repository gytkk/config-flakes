use trading_agent_kis_client::journal::{
    BrokerCommandJournal, BrokerCommandRecord, BrokerCommandStatus, BrokerCommandType,
};
use trading_agent_kis_client::{
    AccountKey, AmendOrderRequest, BrokerEnvironment, BrokerError, CancelOrderRequest,
    ClientOrderId, OrderSide, Quantity,
};

use rust_decimal::Decimal;
use time::OffsetDateTime;

use crate::config::RiskConfig;
use crate::decision::{
    DecisionOrderType, DecisionProposal, DecisionType, DecisionValidationContext,
};
use crate::error::TradeEngineError;
use crate::gateway::traits::TradingGateway;
use crate::gateway::types::{AccountSnapshot, CreateOrderRequest, MarketScope, Symbol};
use crate::market::MarketProfile;
use crate::risk::{
    DailyLossState, RiskEvaluationContext, RiskEvaluationRequest, RiskPolicy,
    missing_sell_cost_basis_symbols,
};
use crate::storage::{
    MainDb, StoredExecutionRequestForSubmit, StoredExecutionRequestId, StoredRuntimeRunId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionWorkflowConfig {
    pub read_only: bool,
    pub account_key: AccountKey,
    pub environment: BrokerEnvironment,
    pub market_scope: MarketScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalExecutionRiskDecision {
    Approved,
    Blocked { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelExecutionRequest {
    pub parent_execution_request_id: StoredExecutionRequestId,
    pub client_order_id: ClientOrderId,
    pub quantity: Option<Quantity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmendExecutionRequest {
    pub parent_execution_request_id: StoredExecutionRequestId,
    pub client_order_id: ClientOrderId,
    pub quantity: Option<Quantity>,
    pub limit_price: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowUpExecutionRiskRequest {
    pub parent_execution_request_id: StoredExecutionRequestId,
    pub client_order_id: ClientOrderId,
    pub command_type: BrokerCommandType,
}

#[derive(Debug, Clone)]
pub struct FinalExecutionRiskInputs<'a> {
    pub risk: &'a RiskConfig,
    pub read_only: bool,
    pub market_scope: MarketScope,
    pub allowed_symbols: &'a [Symbol],
    pub now: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowUpExecutionOutcome {
    Accepted,
    Rejected,
    ReconciliationNeeded,
    Failed,
}

pub trait FinalExecutionRiskGate: Send + Sync {
    fn evaluate(
        &self,
        request: &StoredExecutionRequestForSubmit,
    ) -> Result<FinalExecutionRiskDecision, TradeEngineError>;
}

pub trait FinalFollowUpExecutionRiskGate: Send + Sync {
    fn evaluate(
        &self,
        request: &FollowUpExecutionRiskRequest,
    ) -> Result<FinalExecutionRiskDecision, TradeEngineError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AllowAllFinalExecutionRiskGate;

impl FinalExecutionRiskGate for AllowAllFinalExecutionRiskGate {
    fn evaluate(
        &self,
        _request: &StoredExecutionRequestForSubmit,
    ) -> Result<FinalExecutionRiskDecision, TradeEngineError> {
        Ok(FinalExecutionRiskDecision::Approved)
    }
}

impl FinalFollowUpExecutionRiskGate for AllowAllFinalExecutionRiskGate {
    fn evaluate(
        &self,
        _request: &FollowUpExecutionRiskRequest,
    ) -> Result<FinalExecutionRiskDecision, TradeEngineError> {
        Ok(FinalExecutionRiskDecision::Approved)
    }
}

#[derive(Debug, Clone)]
pub struct PolicyFinalExecutionRiskGate {
    policy: RiskPolicy,
    context: RiskEvaluationContext,
}

impl PolicyFinalExecutionRiskGate {
    pub fn new(policy: RiskPolicy, context: RiskEvaluationContext) -> Self {
        Self { policy, context }
    }
}

impl FinalExecutionRiskGate for PolicyFinalExecutionRiskGate {
    fn evaluate(
        &self,
        request: &StoredExecutionRequestForSubmit,
    ) -> Result<FinalExecutionRiskDecision, TradeEngineError> {
        let proposal = final_risk_proposal(request)?;
        let evaluation = self.policy.evaluate(RiskEvaluationRequest {
            proposal: &proposal,
            context: self.context,
        });
        if evaluation.is_approved() {
            return Ok(FinalExecutionRiskDecision::Approved);
        }
        let reasons = evaluation
            .reasons
            .iter()
            .map(|reason| reason.code())
            .collect::<Vec<_>>()
            .join(",");
        Ok(FinalExecutionRiskDecision::Blocked {
            reason: format!("final risk check blocked: {reasons}"),
        })
    }
}

pub async fn submit_cancel_execution_request<G, J, R>(
    gateway: &G,
    db: &mut MainDb,
    journal: &J,
    request: CancelExecutionRequest,
    final_risk_gate: &R,
    config: &ExecutionWorkflowConfig,
) -> Result<FollowUpExecutionOutcome, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    J: BrokerCommandJournal + ?Sized,
    R: FinalFollowUpExecutionRiskGate + ?Sized,
{
    if config.read_only {
        db.record_execution_follow_up_failed(
            request.parent_execution_request_id,
            Some(&request.client_order_id),
            BrokerCommandType::Cancel,
            "read-only execution workflow is disabled",
        )?;
        return Ok(FollowUpExecutionOutcome::Failed);
    }

    let cancel = match build_cancel_order_request(db, journal, &request).await {
        Ok(cancel) => cancel,
        Err(error) => {
            let message = error.to_string();
            db.record_execution_follow_up_failed(
                request.parent_execution_request_id,
                Some(&request.client_order_id),
                BrokerCommandType::Cancel,
                message.as_str(),
            )?;
            return Err(error);
        }
    };
    if let Some(outcome) = halt_follow_up_if_final_risk_blocks(
        db,
        journal,
        final_risk_gate,
        request.parent_execution_request_id,
        &request.client_order_id,
        BrokerCommandType::Cancel,
    )
    .await?
    {
        return Ok(outcome);
    }

    match gateway.cancel_order(cancel).await {
        Ok(result) if result.accepted => {
            if journal_command_has_status(
                journal,
                &request.client_order_id,
                BrokerCommandStatus::Cancelled,
            )
            .await?
            {
                db.record_execution_order_cancelled(
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    &result,
                )?;
                Ok(FollowUpExecutionOutcome::Accepted)
            } else {
                record_follow_up_reconciliation_needed(
                    db,
                    journal,
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    "cancel_order",
                    "gateway accepted cancel order but broker command journal did not record cancellation",
                )
                .await
            }
        }
        Ok(_) => {
            if journal_command_has_status(
                journal,
                &request.client_order_id,
                BrokerCommandStatus::Rejected,
            )
            .await?
            {
                db.record_execution_follow_up_rejected(
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    BrokerCommandType::Cancel,
                    "broker rejected cancel order",
                )?;
                Ok(FollowUpExecutionOutcome::Rejected)
            } else {
                record_follow_up_reconciliation_needed(
                    db,
                    journal,
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    "cancel_order",
                    "gateway rejected cancel order but broker command journal did not record rejection",
                )
                .await
            }
        }
        Err(BrokerError::OrderRejected { message }) => {
            if journal_command_has_status(
                journal,
                &request.client_order_id,
                BrokerCommandStatus::Rejected,
            )
            .await?
            {
                db.record_execution_follow_up_rejected(
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    BrokerCommandType::Cancel,
                    message.as_str(),
                )?;
                Ok(FollowUpExecutionOutcome::Rejected)
            } else {
                let reason = format!(
                    "broker rejected cancel order but broker command journal did not record rejection: {message}"
                );
                record_follow_up_reconciliation_needed(
                    db,
                    journal,
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    "cancel_order",
                    reason.as_str(),
                )
                .await
            }
        }
        Err(error) => {
            handle_follow_up_gateway_error(
                db,
                journal,
                request.parent_execution_request_id,
                &request.client_order_id,
                BrokerCommandType::Cancel,
                "cancel_order",
                error,
            )
            .await
        }
    }
}

pub async fn submit_amend_execution_request<G, J, R>(
    gateway: &G,
    db: &mut MainDb,
    journal: &J,
    request: AmendExecutionRequest,
    final_risk_gate: &R,
    config: &ExecutionWorkflowConfig,
) -> Result<FollowUpExecutionOutcome, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    J: BrokerCommandJournal + ?Sized,
    R: FinalFollowUpExecutionRiskGate + ?Sized,
{
    if config.read_only {
        db.record_execution_follow_up_failed(
            request.parent_execution_request_id,
            Some(&request.client_order_id),
            BrokerCommandType::Amend,
            "read-only execution workflow is disabled",
        )?;
        return Ok(FollowUpExecutionOutcome::Failed);
    }

    let amend = match build_amend_order_request(db, journal, &request).await {
        Ok(amend) => amend,
        Err(error) => {
            let message = error.to_string();
            db.record_execution_follow_up_failed(
                request.parent_execution_request_id,
                Some(&request.client_order_id),
                BrokerCommandType::Amend,
                message.as_str(),
            )?;
            return Err(error);
        }
    };
    if let Some(outcome) = halt_follow_up_if_final_risk_blocks(
        db,
        journal,
        final_risk_gate,
        request.parent_execution_request_id,
        &request.client_order_id,
        BrokerCommandType::Amend,
    )
    .await?
    {
        return Ok(outcome);
    }

    match gateway.amend_order(amend).await {
        Ok(result) if result.accepted => {
            if journal_command_has_status(
                journal,
                &request.client_order_id,
                BrokerCommandStatus::Accepted,
            )
            .await?
            {
                db.record_execution_order_amended(
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    &result,
                )?;
                Ok(FollowUpExecutionOutcome::Accepted)
            } else {
                record_follow_up_reconciliation_needed(
                    db,
                    journal,
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    "amend_order",
                    "gateway accepted amend order but broker command journal did not record acceptance",
                )
                .await
            }
        }
        Ok(_) => {
            if journal_command_has_status(
                journal,
                &request.client_order_id,
                BrokerCommandStatus::Rejected,
            )
            .await?
            {
                db.record_execution_follow_up_rejected(
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    BrokerCommandType::Amend,
                    "broker rejected amend order",
                )?;
                Ok(FollowUpExecutionOutcome::Rejected)
            } else {
                record_follow_up_reconciliation_needed(
                    db,
                    journal,
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    "amend_order",
                    "gateway rejected amend order but broker command journal did not record rejection",
                )
                .await
            }
        }
        Err(BrokerError::OrderRejected { message }) => {
            if journal_command_has_status(
                journal,
                &request.client_order_id,
                BrokerCommandStatus::Rejected,
            )
            .await?
            {
                db.record_execution_follow_up_rejected(
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    BrokerCommandType::Amend,
                    message.as_str(),
                )?;
                Ok(FollowUpExecutionOutcome::Rejected)
            } else {
                let reason = format!(
                    "broker rejected amend order but broker command journal did not record rejection: {message}"
                );
                record_follow_up_reconciliation_needed(
                    db,
                    journal,
                    request.parent_execution_request_id,
                    &request.client_order_id,
                    "amend_order",
                    reason.as_str(),
                )
                .await
            }
        }
        Err(error) => {
            handle_follow_up_gateway_error(
                db,
                journal,
                request.parent_execution_request_id,
                &request.client_order_id,
                BrokerCommandType::Amend,
                "amend_order",
                error,
            )
            .await
        }
    }
}

async fn halt_follow_up_if_final_risk_blocks<J, R>(
    db: &mut MainDb,
    journal: &J,
    final_risk_gate: &R,
    parent_execution_request_id: StoredExecutionRequestId,
    client_order_id: &ClientOrderId,
    command_type: BrokerCommandType,
) -> Result<Option<FollowUpExecutionOutcome>, TradeEngineError>
where
    J: BrokerCommandJournal + ?Sized,
    R: FinalFollowUpExecutionRiskGate + ?Sized,
{
    let risk_request = FollowUpExecutionRiskRequest {
        parent_execution_request_id,
        client_order_id: client_order_id.clone(),
        command_type,
    };
    let FinalExecutionRiskDecision::Blocked { reason } = final_risk_gate.evaluate(&risk_request)?
    else {
        return Ok(None);
    };
    journal
        .mark_safety_halted(client_order_id, reason.as_str())
        .await?;
    db.record_execution_follow_up_failed(
        parent_execution_request_id,
        Some(client_order_id),
        command_type,
        reason.as_str(),
    )?;
    Ok(Some(FollowUpExecutionOutcome::Failed))
}

async fn handle_follow_up_gateway_error<J>(
    db: &mut MainDb,
    journal: &J,
    parent_execution_request_id: StoredExecutionRequestId,
    client_order_id: &ClientOrderId,
    command_type: BrokerCommandType,
    stage: &str,
    error: BrokerError,
) -> Result<FollowUpExecutionOutcome, TradeEngineError>
where
    J: BrokerCommandJournal + ?Sized,
{
    let message = error.to_string();
    if matches!(error, BrokerError::ReconciliationNeeded { .. }) {
        return record_follow_up_reconciliation_needed(
            db,
            journal,
            parent_execution_request_id,
            client_order_id,
            stage,
            message.as_str(),
        )
        .await;
    }

    let command = journal.get_command(client_order_id).await?.ok_or_else(|| {
        TradeEngineError::runtime("follow-up child journal command was not found")
    })?;
    if matches!(
        command.last_known_state,
        BrokerCommandStatus::CommandRecorded | BrokerCommandStatus::ReadyToSend
    ) {
        journal
            .mark_safety_halted(client_order_id, message.as_str())
            .await?;
        db.record_execution_follow_up_failed(
            parent_execution_request_id,
            Some(client_order_id),
            command_type,
            message.as_str(),
        )?;
        return Ok(FollowUpExecutionOutcome::Failed);
    }

    record_follow_up_reconciliation_needed(
        db,
        journal,
        parent_execution_request_id,
        client_order_id,
        stage,
        message.as_str(),
    )
    .await
}

async fn record_follow_up_reconciliation_needed<J>(
    db: &mut MainDb,
    journal: &J,
    parent_execution_request_id: StoredExecutionRequestId,
    client_order_id: &ClientOrderId,
    stage: &str,
    reason: &str,
) -> Result<FollowUpExecutionOutcome, TradeEngineError>
where
    J: BrokerCommandJournal + ?Sized,
{
    mark_command_reconciliation_needed_if_active(journal, client_order_id, reason).await?;
    db.record_execution_reconciliation_needed_for_command(
        parent_execution_request_id,
        client_order_id,
        stage,
        reason,
    )?;
    Ok(FollowUpExecutionOutcome::ReconciliationNeeded)
}

pub async fn submit_next_execution_request<G, J, R>(
    gateway: &G,
    db: &mut MainDb,
    journal: &J,
    final_risk_gate: &R,
    config: &ExecutionWorkflowConfig,
) -> Result<Option<StoredExecutionRequestId>, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    J: BrokerCommandJournal + ?Sized,
    R: FinalExecutionRiskGate + ?Sized,
{
    let Some(request) = db.next_requested_execution_request()? else {
        return Ok(None);
    };

    submit_execution_request(gateway, db, journal, request, final_risk_gate, config).await
}

pub async fn submit_next_execution_request_with_policy<G, J>(
    gateway: &G,
    db: &mut MainDb,
    journal: &J,
    config: &ExecutionWorkflowConfig,
    risk_inputs: FinalExecutionRiskInputs<'_>,
) -> Result<Option<StoredExecutionRequestId>, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    J: BrokerCommandJournal + ?Sized,
{
    let Some(request) = db.next_requested_execution_request()? else {
        return Ok(None);
    };
    if config.read_only {
        return submit_execution_request(
            gateway,
            db,
            journal,
            request,
            &AllowAllFinalExecutionRiskGate,
            config,
        )
        .await;
    }

    let final_risk_gate =
        build_policy_final_execution_risk_gate(gateway, db, &request, risk_inputs).await?;
    submit_execution_request(gateway, db, journal, request, &final_risk_gate, config).await
}

pub async fn submit_next_execution_request_for_run_with_policy<G, J>(
    gateway: &G,
    db: &mut MainDb,
    journal: &J,
    run_id: StoredRuntimeRunId,
    config: &ExecutionWorkflowConfig,
    risk_inputs: FinalExecutionRiskInputs<'_>,
) -> Result<Option<StoredExecutionRequestId>, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    J: BrokerCommandJournal + ?Sized,
{
    let Some(request) = db.next_requested_execution_request_for_run(run_id)? else {
        return Ok(None);
    };
    if config.read_only {
        return submit_execution_request(
            gateway,
            db,
            journal,
            request,
            &AllowAllFinalExecutionRiskGate,
            config,
        )
        .await;
    }

    let final_risk_gate =
        build_policy_final_execution_risk_gate(gateway, db, &request, risk_inputs).await?;
    submit_execution_request(gateway, db, journal, request, &final_risk_gate, config).await
}

async fn submit_execution_request<G, J, R>(
    gateway: &G,
    db: &mut MainDb,
    journal: &J,
    request: StoredExecutionRequestForSubmit,
    final_risk_gate: &R,
    config: &ExecutionWorkflowConfig,
) -> Result<Option<StoredExecutionRequestId>, TradeEngineError>
where
    G: TradingGateway + ?Sized,
    J: BrokerCommandJournal + ?Sized,
    R: FinalExecutionRiskGate + ?Sized,
{
    let request_id = request.id;

    if config.read_only {
        db.record_execution_failed(request.id, "read-only execution workflow is disabled")?;
        return Ok(Some(request.id));
    }

    let order = match build_create_order_request(&request, config) {
        Ok(order) => order,
        Err(error) => {
            let message = error.to_string();
            db.record_execution_failed(request.id, message.as_str())?;
            return Ok(Some(request.id));
        }
    };
    let client_order_id = order.client_order_id.clone();
    let command = place_order_command(config, order.clone())?;
    if let Err(error) = journal.reserve_command(command).await {
        let message = format!("journal reserve failed before broker submit: {error}");
        db.record_execution_failed(request.id, message.as_str())?;
        return Err(error.into());
    }
    db.link_execution_request_to_journal(request.id, &client_order_id)?;

    if let FinalExecutionRiskDecision::Blocked { reason } = final_risk_gate.evaluate(&request)? {
        if let Err(error) = journal
            .mark_safety_halted(&client_order_id, reason.as_str())
            .await
        {
            let message = format!("final risk blocked but journal safety halt failed: {error}");
            db.record_execution_failed(request.id, message.as_str())?;
            return Err(error.into());
        }
        db.record_execution_failed(request.id, reason.as_str())?;
        return Ok(Some(request.id));
    }

    match gateway.place_order(order).await {
        Ok(submission) if submission.accepted => {
            if journal_command_has_status(journal, &client_order_id, BrokerCommandStatus::Accepted)
                .await?
            {
                db.record_execution_order_accepted(request.id, &submission)?;
            } else {
                record_submit_reconciliation_needed(
                    db,
                    journal,
                    request.id,
                    &client_order_id,
                    "place_order",
                    "gateway accepted order but broker command journal did not record acceptance",
                )
                .await?;
            }
        }
        Ok(_) => {
            if journal_command_has_status(journal, &client_order_id, BrokerCommandStatus::Rejected)
                .await?
            {
                db.record_execution_order_rejected(request.id, "broker rejected order")?;
            } else {
                record_submit_reconciliation_needed(
                    db,
                    journal,
                    request.id,
                    &client_order_id,
                    "place_order",
                    "gateway rejected order but broker command journal did not record rejection",
                )
                .await?;
            }
        }
        Err(BrokerError::OrderRejected { message }) => {
            if journal_command_has_status(journal, &client_order_id, BrokerCommandStatus::Rejected)
                .await?
            {
                db.record_execution_order_rejected(request.id, message.as_str())?;
            } else {
                let reason = format!(
                    "broker rejected order but broker command journal did not record rejection: {message}"
                );
                record_submit_reconciliation_needed(
                    db,
                    journal,
                    request.id,
                    &client_order_id,
                    "place_order",
                    reason.as_str(),
                )
                .await?;
            }
        }
        Err(BrokerError::SafetyHalt { reason }) => {
            let message = format!("safety halt: {reason}");
            let command = journal
                .get_command(&client_order_id)
                .await?
                .ok_or_else(|| TradeEngineError::runtime("submit journal command was not found"))?;
            if matches!(
                command.last_known_state,
                BrokerCommandStatus::CommandRecorded | BrokerCommandStatus::ReadyToSend
            ) {
                journal
                    .mark_safety_halted(&client_order_id, message.as_str())
                    .await?;
                db.record_execution_failed(request.id, message.as_str())?;
            } else {
                record_submit_reconciliation_needed(
                    db,
                    journal,
                    request.id,
                    &client_order_id,
                    "place_order",
                    message.as_str(),
                )
                .await?;
            }
        }
        Err(error) => {
            let reason = error.to_string();
            record_submit_reconciliation_needed(
                db,
                journal,
                request.id,
                &client_order_id,
                "place_order",
                reason.as_str(),
            )
            .await?;
        }
    }

    Ok(Some(request_id))
}

async fn journal_command_has_status<J>(
    journal: &J,
    client_order_id: &ClientOrderId,
    status: BrokerCommandStatus,
) -> Result<bool, TradeEngineError>
where
    J: BrokerCommandJournal + ?Sized,
{
    let command = journal
        .get_command(client_order_id)
        .await?
        .ok_or_else(|| TradeEngineError::runtime("submit journal command was not found"))?;
    Ok(command.last_known_state == status)
}

async fn record_submit_reconciliation_needed<J>(
    db: &mut MainDb,
    journal: &J,
    request_id: StoredExecutionRequestId,
    client_order_id: &ClientOrderId,
    stage: &str,
    reason: &str,
) -> Result<(), TradeEngineError>
where
    J: BrokerCommandJournal + ?Sized,
{
    mark_command_reconciliation_needed_if_active(journal, client_order_id, reason).await?;
    db.record_execution_reconciliation_needed(request_id, stage, reason)?;
    Ok(())
}

async fn mark_command_reconciliation_needed_if_active<J>(
    journal: &J,
    client_order_id: &ClientOrderId,
    reason: &str,
) -> Result<(), TradeEngineError>
where
    J: BrokerCommandJournal + ?Sized,
{
    let command = journal
        .get_command(client_order_id)
        .await?
        .ok_or_else(|| TradeEngineError::runtime("journal command was not found"))?;
    if matches!(
        command.last_known_state,
        BrokerCommandStatus::CommandRecorded
            | BrokerCommandStatus::ReadyToSend
            | BrokerCommandStatus::Submitting
    ) {
        journal
            .mark_reconcile_needed(client_order_id, reason)
            .await?;
    }
    Ok(())
}

async fn build_policy_final_execution_risk_gate<G>(
    gateway: &G,
    db: &MainDb,
    request: &StoredExecutionRequestForSubmit,
    inputs: FinalExecutionRiskInputs<'_>,
) -> Result<PolicyFinalExecutionRiskGate, TradeEngineError>
where
    G: TradingGateway + ?Sized,
{
    let policy = RiskPolicy::new(
        inputs.read_only,
        inputs.risk.enable_live_trading,
        inputs.allowed_symbols.iter().cloned(),
        inputs.risk.max_order_notional,
    )
    .with_position_notional_limit(inputs.risk.max_position_notional)
    .with_daily_loss_limit(inputs.risk.max_daily_loss)
    .with_daily_order_limit(inputs.risk.max_daily_order_count);

    let unresolved_journal_commands = db.has_unresolved_broker_commands()?;
    let panic_stop_active = db.panic_stop_active()?;
    if unresolved_journal_commands || panic_stop_active {
        return Ok(PolicyFinalExecutionRiskGate::new(
            policy,
            RiskEvaluationContext {
                market_session_open: true,
                unresolved_journal_commands,
                panic_stop_active,
                available_buying_power: None,
                current_position_quantity: Quantity::new(0),
                duplicate_order_intent: false,
                projected_position_notional: None,
                submitted_order_count_today: 0,
                realized_loss_today: DailyLossState::Known(Decimal::ZERO),
            },
        ));
    }

    let available_buying_power = if request.side == OrderSide::Buy {
        Some(
            gateway
                .get_buying_power(Some(&request.symbol))
                .await
                .map_err(|source| TradeEngineError::gateway("buying_power", source))?
                .available_cash,
        )
    } else {
        None
    };
    let account = gateway
        .get_account_snapshot()
        .await
        .map_err(|source| TradeEngineError::gateway("account_snapshot", source))?;
    let market_profile = MarketProfile::new(inputs.market_scope);
    let market_session = gateway
        .get_market_session(market_profile.trading_day_string_at(inputs.now).as_str())
        .await
        .map_err(|source| TradeEngineError::gateway("market_session", source))?;
    let today_fills = gateway
        .list_fills(market_profile.trading_day_for_fills_at(inputs.now))
        .await
        .map_err(|source| TradeEngineError::gateway("fills", source))?;
    let supplemental_cost_basis =
        supplemental_cost_basis_from_db(db, &account, &today_fills, inputs.now)?;
    let day_start = market_profile.day_start_utc_at(inputs.now);
    let submitted_order_count_today =
        db.execution_request_count_since_excluding(day_start, Some(request.id))?;

    Ok(PolicyFinalExecutionRiskGate::new(
        policy,
        RiskEvaluationContext {
            market_session_open: market_session.is_open,
            unresolved_journal_commands,
            panic_stop_active,
            available_buying_power,
            current_position_quantity: current_position_quantity_for_submit(&account, request),
            duplicate_order_intent: false,
            projected_position_notional: projected_position_notional_for_submit(&account, request),
            submitted_order_count_today,
            realized_loss_today: DailyLossState::from_account_fills_and_cost_basis(
                &account,
                &today_fills,
                &supplemental_cost_basis,
            ),
        },
    ))
}

fn supplemental_cost_basis_from_db(
    db: &MainDb,
    account: &AccountSnapshot,
    fills: &[crate::gateway::types::Fill],
    observed_at: OffsetDateTime,
) -> Result<Vec<(Symbol, Decimal)>, TradeEngineError> {
    let mut cost_basis = Vec::new();
    for symbol in missing_sell_cost_basis_symbols(account, fills) {
        if let Some(average_price) =
            db.latest_position_average_price_before(&symbol, observed_at)?
        {
            cost_basis.push((symbol, average_price));
        }
    }
    Ok(cost_basis)
}

async fn build_cancel_order_request<J>(
    db: &mut MainDb,
    journal: &J,
    request: &CancelExecutionRequest,
) -> Result<CancelOrderRequest, TradeEngineError>
where
    J: BrokerCommandJournal + ?Sized,
{
    let target = db.follow_up_order_target(request.parent_execution_request_id)?;
    let parent = journal
        .get_command(&target.reference.client_order_id)
        .await?
        .ok_or_else(|| {
            TradeEngineError::runtime("follow-up parent journal command was not found")
        })?;
    let cancel = CancelOrderRequest {
        client_order_id: request.client_order_id.clone(),
        reference: target.reference,
        quantity: request.quantity,
    };
    let command = BrokerCommandRecord::new_cancel_order(&parent, &cancel)?;
    journal.reserve_command(command).await?;
    db.link_follow_up_command_to_execution(
        request.parent_execution_request_id,
        &request.client_order_id,
        BrokerCommandType::Cancel,
    )?;
    Ok(cancel)
}

async fn build_amend_order_request<J>(
    db: &mut MainDb,
    journal: &J,
    request: &AmendExecutionRequest,
) -> Result<AmendOrderRequest, TradeEngineError>
where
    J: BrokerCommandJournal + ?Sized,
{
    let target = db.follow_up_order_target(request.parent_execution_request_id)?;
    let parent = journal
        .get_command(&target.reference.client_order_id)
        .await?
        .ok_or_else(|| {
            TradeEngineError::runtime("follow-up parent journal command was not found")
        })?;
    let amend = AmendOrderRequest {
        client_order_id: request.client_order_id.clone(),
        reference: target.reference,
        quantity: request.quantity,
        limit_price: request.limit_price,
    };
    let command = BrokerCommandRecord::new_amend_order(&parent, &amend)?;
    journal.reserve_command(command).await?;
    db.link_follow_up_command_to_execution(
        request.parent_execution_request_id,
        &request.client_order_id,
        BrokerCommandType::Amend,
    )?;
    Ok(amend)
}

fn final_risk_proposal(
    request: &StoredExecutionRequestForSubmit,
) -> Result<crate::decision::ValidatedDecisionProposal, TradeEngineError> {
    let decision_type = match request.side {
        OrderSide::Buy => DecisionType::Buy,
        OrderSide::Sell => DecisionType::Sell,
    };
    DecisionProposal {
        decision_type,
        symbol: request.symbol.clone(),
        quantity: Some(request.quantity),
        order_type: Some(request.order_type),
        limit_price: request.limit_price,
        confidence: Decimal::ONE,
        time_horizon: None,
        reason: "final execution risk check".to_owned(),
        risk_notes: Vec::new(),
    }
    .validate(&DecisionValidationContext::new([request.symbol.clone()]))
    .map_err(|error| {
        TradeEngineError::runtime(format!(
            "stored execution request failed final risk validation: {error}"
        ))
    })
}

fn current_position_quantity_for_submit(
    account: &AccountSnapshot,
    request: &StoredExecutionRequestForSubmit,
) -> Quantity {
    account
        .positions
        .iter()
        .find(|position| position.symbol == request.symbol)
        .map(|position| position.quantity)
        .unwrap_or_else(|| Quantity::new(0))
}

fn projected_position_notional_for_submit(
    account: &AccountSnapshot,
    request: &StoredExecutionRequestForSubmit,
) -> Option<Decimal> {
    let limit_price = request.limit_price?;
    let current_notional = account
        .positions
        .iter()
        .find(|position| position.symbol == request.symbol)
        .map(|position| Decimal::from(position.quantity.get()) * limit_price)
        .unwrap_or(Decimal::ZERO);
    let order_notional = Decimal::from(request.quantity.get()) * limit_price;
    match request.side {
        OrderSide::Buy => Some(current_notional + order_notional),
        OrderSide::Sell => Some((current_notional - order_notional).max(Decimal::ZERO)),
    }
}

fn build_create_order_request(
    request: &StoredExecutionRequestForSubmit,
    config: &ExecutionWorkflowConfig,
) -> Result<CreateOrderRequest, TradeEngineError> {
    if request.order_type != DecisionOrderType::Limit {
        return Err(TradeEngineError::runtime(
            "execution MVP supports limit orders only",
        ));
    }
    let limit_price = request
        .limit_price
        .ok_or_else(|| TradeEngineError::runtime("limit execution request is missing price"))?;
    let client_order_id =
        ClientOrderId::new(request.execution_request_id.as_str()).map_err(|source| {
            TradeEngineError::invalid_broker_value("execution client order id", source)
        })?;
    CreateOrderRequest::cash_limit(
        client_order_id,
        config.account_key.clone(),
        request.symbol.clone(),
        request.side,
        request.quantity,
        limit_price,
    )
    .map_err(|source| TradeEngineError::invalid_broker_value("execution order request", source))
}

fn place_order_command(
    config: &ExecutionWorkflowConfig,
    order: CreateOrderRequest,
) -> Result<BrokerCommandRecord, BrokerError> {
    let mut command = BrokerCommandRecord::new_place_order(config.environment, order)?;
    if let MarketScope::OverseasStock { exchange, .. } = &config.market_scope {
        command.exchange_code = Some(exchange.to_string());
    }
    Ok(command)
}
