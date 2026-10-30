use rust_decimal::Decimal;
use trade_engine::decision::{
    DecisionOrderType, DecisionProposal, DecisionType, DecisionValidationContext,
};
use trade_engine::execution::{
    AmbiguousOutcomeMetadata, ExecutionEventMetadata, ExecutionEventType, ExecutionRequest,
    ExecutionRequestError, ExecutionRequestId, ExecutionRequestStatus,
};
use trade_engine::gateway::types::{Quantity, Symbol};
use trade_engine::risk::{RiskBlockReason, RiskEvaluation};

fn symbol(value: &str) -> Symbol {
    Symbol::new(value).expect("valid test symbol")
}

fn request_id() -> ExecutionRequestId {
    ExecutionRequestId::new("exec-0001").expect("valid execution request id")
}

fn decision_context() -> DecisionValidationContext {
    DecisionValidationContext::new([symbol("005930")])
}

fn valid_buy_limit() -> DecisionProposal {
    DecisionProposal {
        decision_type: DecisionType::Buy,
        symbol: symbol("005930"),
        quantity: Some(Quantity::new(10)),
        order_type: Some(DecisionOrderType::Limit),
        limit_price: Some(Decimal::new(73_000, 0)),
        confidence: Decimal::new(72, 2),
        time_horizon: Some("intraday".to_owned()),
        reason: "approved test proposal".to_owned(),
        risk_notes: Vec::new(),
    }
}

#[test]
fn creates_execution_request_for_approved_buy() {
    let validated = valid_buy_limit()
        .validate(&decision_context())
        .expect("valid buy proposal");
    let risk = RiskEvaluation::approved(Some(Decimal::new(730_000, 0)));

    let request = ExecutionRequest::new(request_id(), validated, &risk)
        .expect("approved buy creates execution request");
    let event = request.requested_event();

    assert_eq!(request.status, ExecutionRequestStatus::Requested);
    assert!(request.can_submit_to_broker());
    assert_eq!(request.proposal.decision_type, DecisionType::Buy);
    assert_eq!(request.proposal.quantity, Some(Quantity::new(10)));
    assert_eq!(request.order_notional, Some(Decimal::new(730_000, 0)));
    assert_eq!(event.event_type, ExecutionEventType::ExecutionRequested);
    assert_eq!(event.request_id, request.id);
}

#[test]
fn rejects_hold_decision_even_when_risk_is_approved() {
    let validated = DecisionProposal::hold(symbol("005930"), Decimal::new(50, 2), "no trade")
        .validate(&decision_context())
        .expect("hold proposal is valid before execution");
    let risk = RiskEvaluation::approved(None);

    let error = ExecutionRequest::new(request_id(), validated, &risk)
        .expect_err("hold does not create execution request");

    assert_eq!(error, ExecutionRequestError::HoldDecision);
}

#[test]
fn rejects_blocked_risk_evaluation() {
    let validated = valid_buy_limit()
        .validate(&decision_context())
        .expect("valid buy proposal");
    let risk = RiskEvaluation::blocked(vec![RiskBlockReason::ReadOnlyMode], None);

    let error = ExecutionRequest::new(request_id(), validated, &risk)
        .expect_err("blocked risk does not create execution request");

    assert_eq!(error, ExecutionRequestError::RiskNotApproved);
}

#[test]
fn records_reconciliation_needed_event_for_ambiguous_outcome() {
    let validated = valid_buy_limit()
        .validate(&decision_context())
        .expect("valid buy proposal");
    let risk = RiskEvaluation::approved(Some(Decimal::new(730_000, 0)));
    let mut request = ExecutionRequest::new(request_id(), validated, &risk)
        .expect("approved buy creates execution request");

    let event = request.record_reconciliation_needed(AmbiguousOutcomeMetadata::new(
        "broker_submit",
        "timeout after write may have reached broker",
    ));

    assert_eq!(request.status, ExecutionRequestStatus::ReconciliationNeeded);
    assert!(!request.can_submit_to_broker());
    assert_eq!(event.event_type, ExecutionEventType::ReconciliationNeeded);
    assert_eq!(
        event.metadata,
        Some(ExecutionEventMetadata::AmbiguousOutcome(
            AmbiguousOutcomeMetadata::new(
                "broker_submit",
                "timeout after write may have reached broker",
            )
        ))
    );
}
