use crate::core::journal::{BrokerCommandRecord, BrokerCommandType};
use crate::{
    Fill, OrderReference, OrderState, OrderStatus, OrderStatusEvidence, ReconciliationResult,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationReport {
    pub result: ReconciliationResult,
    pub matcher: ReconciliationMatcher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationMatcher {
    BrokerNativeId,
    TargetBrokerNativeId,
    ClientOrderId,
    OrderEvidence,
    FillEvidence,
    NoMatch,
    ManualIntervention,
}

impl ReconciliationMatcher {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BrokerNativeId => "broker_native_id",
            Self::TargetBrokerNativeId => "target_broker_native_id",
            Self::ClientOrderId => "client_order_id",
            Self::OrderEvidence => "order_evidence",
            Self::FillEvidence => "fill_evidence",
            Self::NoMatch => "no_match",
            Self::ManualIntervention => "manual_intervention",
        }
    }
}

pub fn reconcile_command_snapshot(
    command: &BrokerCommandRecord,
    orders: &[OrderStatus],
    fills: &[Fill],
) -> ReconciliationResult {
    reconcile_command_snapshot_report(command, orders, fills).result
}

pub fn reconcile_command_snapshot_report(
    command: &BrokerCommandRecord,
    orders: &[OrderStatus],
    fills: &[Fill],
) -> ReconciliationReport {
    if let Some(report) = match_unique_fill(
        fills
            .iter()
            .filter(|fill| broker_native_reference_matches(command, &fill.reference)),
        ReconciliationMatcher::BrokerNativeId,
    ) {
        return report;
    }
    if let Some(report) = match_unique_fill(
        fills
            .iter()
            .filter(|fill| target_reference_matches(command, &fill.reference)),
        ReconciliationMatcher::TargetBrokerNativeId,
    ) {
        return report;
    }
    if let Some(report) = match_unique_fill(
        fills
            .iter()
            .filter(|fill| client_order_id_matches(command, &fill.reference)),
        ReconciliationMatcher::ClientOrderId,
    ) {
        return report;
    }
    if let Some(report) = match_unique_fill(
        fills.iter().filter(|fill| {
            scoped_reference_matches(command, &fill.reference)
                && order_evidence_matches(command, fill.evidence.as_ref())
        }),
        ReconciliationMatcher::FillEvidence,
    ) {
        return report;
    }
    if let Some(report) = match_unique_order(
        command,
        orders
            .iter()
            .filter(|order| broker_native_reference_matches(command, &order.reference)),
        ReconciliationMatcher::BrokerNativeId,
    ) {
        return report;
    }
    if let Some(report) = match_unique_order(
        command,
        orders
            .iter()
            .filter(|order| target_reference_matches(command, &order.reference)),
        ReconciliationMatcher::TargetBrokerNativeId,
    ) {
        return report;
    }
    if let Some(report) = match_unique_order(
        command,
        orders
            .iter()
            .filter(|order| client_order_id_matches(command, &order.reference)),
        ReconciliationMatcher::ClientOrderId,
    ) {
        return report;
    }
    if let Some(report) = match_unique_order(
        command,
        orders.iter().filter(|order| {
            scoped_reference_matches(command, &order.reference)
                && order_evidence_matches(command, order.evidence.as_ref())
        }),
        ReconciliationMatcher::OrderEvidence,
    ) {
        return report;
    }

    ReconciliationReport {
        result: ReconciliationResult::StillUnknown,
        matcher: ReconciliationMatcher::NoMatch,
    }
}

fn match_unique_order<'a>(
    command: &BrokerCommandRecord,
    mut orders: impl Iterator<Item = &'a OrderStatus>,
    matcher: ReconciliationMatcher,
) -> Option<ReconciliationReport> {
    let order = orders.next()?;
    if orders.next().is_some() {
        return Some(ReconciliationReport {
            result: ReconciliationResult::ManualInterventionRequired,
            matcher: ReconciliationMatcher::ManualIntervention,
        });
    }

    Some(ReconciliationReport {
        result: result_from_order_state(command, &order.state),
        matcher,
    })
}

fn match_unique_fill<'a>(
    mut fills: impl Iterator<Item = &'a Fill>,
    matcher: ReconciliationMatcher,
) -> Option<ReconciliationReport> {
    let _fill = fills.next()?;
    if fills.next().is_some() {
        return Some(ReconciliationReport {
            result: ReconciliationResult::ManualInterventionRequired,
            matcher: ReconciliationMatcher::ManualIntervention,
        });
    }

    Some(ReconciliationReport {
        result: ReconciliationResult::ConfirmedFilled,
        matcher,
    })
}

fn result_from_order_state(
    command: &BrokerCommandRecord,
    state: &OrderState,
) -> ReconciliationResult {
    match command.command_type {
        BrokerCommandType::Place => match state {
            OrderState::Accepted => ReconciliationResult::ConfirmedSubmitted,
            OrderState::PartiallyFilled | OrderState::Filled => {
                ReconciliationResult::ConfirmedFilled
            }
            OrderState::Cancelled => ReconciliationResult::ConfirmedCancelled,
            OrderState::Rejected => ReconciliationResult::ConfirmedRejected,
            OrderState::Unknown(_) => ReconciliationResult::StillUnknown,
        },
        BrokerCommandType::Cancel => match state {
            OrderState::Cancelled => ReconciliationResult::ConfirmedCancelled,
            OrderState::PartiallyFilled | OrderState::Filled => {
                ReconciliationResult::ConfirmedFilled
            }
            OrderState::Rejected => ReconciliationResult::ConfirmedRejected,
            OrderState::Accepted | OrderState::Unknown(_) => ReconciliationResult::StillUnknown,
        },
        BrokerCommandType::Amend => match state {
            OrderState::Accepted | OrderState::PartiallyFilled | OrderState::Filled => {
                ReconciliationResult::ConfirmedAmended
            }
            OrderState::Cancelled => ReconciliationResult::ConfirmedCancelled,
            OrderState::Rejected => ReconciliationResult::ConfirmedRejected,
            OrderState::Unknown(_) => ReconciliationResult::StillUnknown,
        },
    }
}

fn broker_native_reference_matches(
    command: &BrokerCommandRecord,
    reference: &OrderReference,
) -> bool {
    native_reference_matches(
        command,
        reference,
        command.broker_order_id.as_deref(),
        command.broker_order_orgno.as_deref(),
    )
}

fn target_reference_matches(command: &BrokerCommandRecord, reference: &OrderReference) -> bool {
    if !matches!(
        command.command_type,
        BrokerCommandType::Cancel | BrokerCommandType::Amend
    ) {
        return false;
    }

    native_reference_matches(
        command,
        reference,
        command.target_broker_order_id.as_deref(),
        command.target_broker_order_orgno.as_deref(),
    )
}

fn native_reference_matches(
    command: &BrokerCommandRecord,
    reference: &OrderReference,
    broker_order_id: Option<&str>,
    broker_order_orgno: Option<&str>,
) -> bool {
    let Some(broker_order_id) = broker_order_id else {
        return false;
    };
    let Some(broker_order_orgno) = broker_order_orgno else {
        return false;
    };
    scoped_reference_matches(command, reference)
        && reference
            .broker_order_id
            .as_ref()
            .is_some_and(|value| value.as_str() == broker_order_id)
        && reference
            .broker_order_orgno
            .as_ref()
            .is_some_and(|value| value.as_str() == broker_order_orgno)
}

fn client_order_id_matches(command: &BrokerCommandRecord, reference: &OrderReference) -> bool {
    scoped_reference_matches(command, reference)
        && command.client_order_id == reference.client_order_id
}

fn scoped_reference_matches(command: &BrokerCommandRecord, reference: &OrderReference) -> bool {
    if command.account_key != reference.account_key || command.environment != reference.environment
    {
        return false;
    }

    let Some(command_exchange_code) = &command.exchange_code else {
        return true;
    };
    reference
        .exchange_code
        .as_ref()
        .is_none_or(|reference_exchange_code| {
            reference_exchange_code.as_str() == command_exchange_code
        })
}

fn order_evidence_matches(
    command: &BrokerCommandRecord,
    evidence: Option<&OrderStatusEvidence>,
) -> bool {
    let Some(evidence) = evidence else {
        return false;
    };

    if evidence.symbol != command.symbol {
        return false;
    }
    if evidence.side != Some(command.side) {
        return false;
    }
    if evidence.ordered_quantity != command.quantity {
        return false;
    }
    if command.trading_day.as_deref() != evidence.trading_day.as_deref() {
        return false;
    }

    let (Some(observed_at), Some(window_start), Some(window_end)) = (
        evidence.observed_at,
        command.broker_observation_window_start,
        command.broker_observation_window_end,
    ) else {
        return false;
    };

    observed_at >= window_start && observed_at <= window_end
}
