use rust_decimal::Decimal;
use thiserror::Error;

use crate::decision::{
    DecisionOrderType, DecisionProposal, DecisionType, ValidatedDecisionProposal,
};
use crate::gateway::types::Quantity;
use crate::risk::RiskEvaluation;

mod workflow;

pub use workflow::{
    AllowAllFinalExecutionRiskGate, AmendExecutionRequest, CancelExecutionRequest,
    ExecutionWorkflowConfig, FinalExecutionRiskDecision, FinalExecutionRiskGate,
    FinalExecutionRiskInputs, FinalFollowUpExecutionRiskGate, FollowUpExecutionOutcome,
    FollowUpExecutionRiskRequest, PolicyFinalExecutionRiskGate, submit_amend_execution_request,
    submit_cancel_execution_request, submit_next_execution_request,
    submit_next_execution_request_for_run_with_policy, submit_next_execution_request_with_policy,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecutionRequestId(String);

impl ExecutionRequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, ExecutionRequestError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ExecutionRequestError::InvalidRequestId {
                reason: "execution request id must not be empty".to_owned(),
            });
        }
        if value.chars().any(char::is_whitespace) {
            return Err(ExecutionRequestError::InvalidRequestId {
                reason: "execution request id must not contain whitespace".to_owned(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionRequestStatus {
    Requested,
    JournalReserved,
    Accepted,
    Rejected,
    ReconciliationNeeded,
    Failed,
    Filled,
    Cancelled,
}

impl ExecutionRequestStatus {
    pub const ALL: [Self; 8] = [
        Self::Requested,
        Self::JournalReserved,
        Self::Accepted,
        Self::Rejected,
        Self::ReconciliationNeeded,
        Self::Failed,
        Self::Filled,
        Self::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::JournalReserved => "journal_reserved",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::ReconciliationNeeded => "reconciliation_needed",
            Self::Failed => "failed",
            Self::Filled => "filled",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "requested" => Some(Self::Requested),
            "journal_reserved" => Some(Self::JournalReserved),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "reconciliation_needed" => Some(Self::ReconciliationNeeded),
            "failed" => Some(Self::Failed),
            "filled" => Some(Self::Filled),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRequest {
    pub id: ExecutionRequestId,
    pub status: ExecutionRequestStatus,
    pub proposal: DecisionProposal,
    pub quantity: Quantity,
    pub order_type: DecisionOrderType,
    pub limit_price: Option<Decimal>,
    pub order_notional: Option<Decimal>,
}

impl ExecutionRequest {
    pub fn new(
        id: ExecutionRequestId,
        proposal: ValidatedDecisionProposal,
        risk: &RiskEvaluation,
    ) -> Result<Self, ExecutionRequestError> {
        if !risk.is_approved() {
            return Err(ExecutionRequestError::RiskNotApproved);
        }

        let proposal = proposal.into_proposal();
        if proposal.is_hold() {
            return Err(ExecutionRequestError::HoldDecision);
        }

        let quantity =
            proposal
                .quantity
                .ok_or(ExecutionRequestError::MissingValidatedOrderField {
                    decision_type: proposal.decision_type,
                    field: "quantity",
                })?;
        let order_type =
            proposal
                .order_type
                .ok_or(ExecutionRequestError::MissingValidatedOrderField {
                    decision_type: proposal.decision_type,
                    field: "order_type",
                })?;

        Ok(Self {
            id,
            status: ExecutionRequestStatus::Requested,
            limit_price: proposal.limit_price,
            order_notional: risk.order_notional,
            proposal,
            quantity,
            order_type,
        })
    }

    pub fn can_submit_to_broker(&self) -> bool {
        self.status == ExecutionRequestStatus::Requested
    }

    pub fn requested_event(&self) -> ExecutionEvent {
        ExecutionEvent {
            request_id: self.id.clone(),
            event_type: ExecutionEventType::ExecutionRequested,
            metadata: None,
        }
    }

    pub fn record_reconciliation_needed(
        &mut self,
        metadata: AmbiguousOutcomeMetadata,
    ) -> ExecutionEvent {
        self.status = ExecutionRequestStatus::ReconciliationNeeded;
        ExecutionEvent {
            request_id: self.id.clone(),
            event_type: ExecutionEventType::ReconciliationNeeded,
            metadata: Some(ExecutionEventMetadata::AmbiguousOutcome(metadata)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionEvent {
    pub request_id: ExecutionRequestId,
    pub event_type: ExecutionEventType,
    pub metadata: Option<ExecutionEventMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionEventType {
    ExecutionRequested,
    BrokerCommandReserved,
    OrderAccepted,
    OrderRejected,
    ReconciliationNeeded,
    ExecutionFailed,
    ReconciliationSucceeded,
    ReconciliationStillUnknown,
    ManualInterventionRequired,
    OrderFilled,
    OrderCancelled,
    OrderAmended,
    FollowUpRejected,
}

impl ExecutionEventType {
    pub const ALL: [Self; 13] = [
        Self::ExecutionRequested,
        Self::BrokerCommandReserved,
        Self::OrderAccepted,
        Self::OrderRejected,
        Self::ReconciliationNeeded,
        Self::ExecutionFailed,
        Self::ReconciliationSucceeded,
        Self::ReconciliationStillUnknown,
        Self::ManualInterventionRequired,
        Self::OrderFilled,
        Self::OrderCancelled,
        Self::OrderAmended,
        Self::FollowUpRejected,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExecutionRequested => "execution_requested",
            Self::BrokerCommandReserved => "broker_command_reserved",
            Self::OrderAccepted => "order_accepted",
            Self::OrderRejected => "order_rejected",
            Self::ReconciliationNeeded => "reconciliation_needed",
            Self::ExecutionFailed => "execution_failed",
            Self::ReconciliationSucceeded => "reconciliation_succeeded",
            Self::ReconciliationStillUnknown => "reconciliation_still_unknown",
            Self::ManualInterventionRequired => "manual_intervention_required",
            Self::OrderFilled => "order_filled",
            Self::OrderCancelled => "order_cancelled",
            Self::OrderAmended => "order_amended",
            Self::FollowUpRejected => "follow_up_rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionEventMetadata {
    AmbiguousOutcome(AmbiguousOutcomeMetadata),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbiguousOutcomeMetadata {
    pub stage: String,
    pub message: String,
}

impl AmbiguousOutcomeMetadata {
    pub fn new(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExecutionRequestError {
    #[error("execution request id is invalid: {reason}")]
    InvalidRequestId { reason: String },
    #[error("hold decisions do not create execution requests")]
    HoldDecision,
    #[error("execution requires an approved risk evaluation")]
    RiskNotApproved,
    #[error("validated {decision_type:?} decision is missing {field}")]
    MissingValidatedOrderField {
        decision_type: DecisionType,
        field: &'static str,
    },
}
