use async_trait::async_trait;
use rust_decimal::Decimal;
use time::OffsetDateTime;

use super::domain::{
    AccountKey, AmendOrderRequest, BrokerEnvironment, CancelOrderRequest, ClientOrderId,
    CreateOrderRequest, OrderSide, OrderType, OverseasStockOrderRequest, Quantity, RequestId,
    Symbol,
};
use super::error::BrokerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerCommandType {
    Place,
    Cancel,
    Amend,
}

impl BrokerCommandType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Place => "place",
            Self::Cancel => "cancel",
            Self::Amend => "amend",
        }
    }

    pub fn parse(value: &str) -> Result<Self, BrokerError> {
        match value {
            "place" => Ok(Self::Place),
            "cancel" => Ok(Self::Cancel),
            "amend" => Ok(Self::Amend),
            other => Err(BrokerError::invariant(format!(
                "unknown broker command type: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerCommandStatus {
    DecisionCreated,
    CommandRecorded,
    ReadyToSend,
    Submitting,
    SubmittedAckUnknown,
    Accepted,
    PartiallyFilled,
    Filled,
    CancelRequested,
    Cancelled,
    Rejected,
    Failed,
    ReconcileFailed,
    SafetyHalted,
}

impl BrokerCommandStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DecisionCreated => "decision_created",
            Self::CommandRecorded => "command_recorded",
            Self::ReadyToSend => "ready_to_send",
            Self::Submitting => "submitting",
            Self::SubmittedAckUnknown => "submitted_ack_unknown",
            Self::Accepted => "accepted",
            Self::PartiallyFilled => "partially_filled",
            Self::Filled => "filled",
            Self::CancelRequested => "cancel_requested",
            Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
            Self::ReconcileFailed => "reconcile_failed",
            Self::SafetyHalted => "safety_halted",
        }
    }

    pub fn parse(value: &str) -> Result<Self, BrokerError> {
        match value {
            "decision_created" => Ok(Self::DecisionCreated),
            "command_recorded" => Ok(Self::CommandRecorded),
            "ready_to_send" => Ok(Self::ReadyToSend),
            "submitting" => Ok(Self::Submitting),
            "submitted_ack_unknown" => Ok(Self::SubmittedAckUnknown),
            "accepted" => Ok(Self::Accepted),
            "partially_filled" => Ok(Self::PartiallyFilled),
            "filled" => Ok(Self::Filled),
            "cancel_requested" => Ok(Self::CancelRequested),
            "cancelled" => Ok(Self::Cancelled),
            "rejected" => Ok(Self::Rejected),
            "failed" => Ok(Self::Failed),
            "reconcile_failed" => Ok(Self::ReconcileFailed),
            "safety_halted" => Ok(Self::SafetyHalted),
            other => Err(BrokerError::invariant(format!(
                "unknown broker command status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerCommandAck {
    pub broker_order_id: Option<String>,
    pub broker_order_orgno: Option<String>,
    pub raw_ack_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerCommandRecord {
    pub client_order_id: ClientOrderId,
    pub parent_client_order_id: Option<ClientOrderId>,
    pub decision_id: Option<String>,
    pub command_type: BrokerCommandType,
    pub environment: BrokerEnvironment,
    pub account_key: AccountKey,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub quantity: Quantity,
    pub limit_price: Option<Decimal>,
    pub order_type: OrderType,
    pub requested_at: OffsetDateTime,
    pub request_id: Option<String>,
    pub transport_send_started_at: Option<OffsetDateTime>,
    pub broker_order_id: Option<String>,
    pub broker_order_orgno: Option<String>,
    pub target_broker_order_id: Option<String>,
    pub target_broker_order_orgno: Option<String>,
    pub cancel_or_amend_quantity: Option<Quantity>,
    pub amend_price: Option<Decimal>,
    pub exchange_code: Option<String>,
    pub last_known_state: BrokerCommandStatus,
    pub raw_ack_code: Option<String>,
    pub reconcile_required: bool,
    pub last_reconciled_at: Option<OffsetDateTime>,
    pub last_reconcile_reason: Option<String>,
    pub trading_day: Option<String>,
    pub broker_observation_window_start: Option<OffsetDateTime>,
    pub broker_observation_window_end: Option<OffsetDateTime>,
    pub sanitized_last_request_metadata_id: Option<String>,
    pub sanitized_last_response_metadata_id: Option<String>,
}

impl BrokerCommandRecord {
    pub fn new_place_order(
        environment: BrokerEnvironment,
        request: CreateOrderRequest,
    ) -> Result<Self, BrokerError> {
        if request.quantity.is_zero() {
            return Err(BrokerError::validation(
                "journal command quantity must be greater than zero",
            ));
        }
        if request.order_type == OrderType::Limit && request.limit_price.is_none() {
            return Err(BrokerError::validation(
                "limit journal command requires limit_price",
            ));
        }

        let requested_at = OffsetDateTime::now_utc();

        Ok(Self {
            client_order_id: request.client_order_id,
            parent_client_order_id: None,
            decision_id: None,
            command_type: BrokerCommandType::Place,
            environment,
            account_key: request.account_key,
            symbol: request.symbol,
            side: request.side,
            quantity: request.quantity,
            limit_price: request.limit_price,
            order_type: request.order_type,
            requested_at,
            request_id: None,
            transport_send_started_at: None,
            broker_order_id: None,
            broker_order_orgno: None,
            target_broker_order_id: None,
            target_broker_order_orgno: None,
            cancel_or_amend_quantity: None,
            amend_price: None,
            exchange_code: None,
            last_known_state: BrokerCommandStatus::CommandRecorded,
            raw_ack_code: None,
            reconcile_required: false,
            last_reconciled_at: None,
            last_reconcile_reason: None,
            trading_day: Some(format_trading_day(requested_at)),
            broker_observation_window_start: Some(requested_at),
            broker_observation_window_end: Some(default_observation_window_end(requested_at)),
            sanitized_last_request_metadata_id: None,
            sanitized_last_response_metadata_id: None,
        })
    }

    pub fn new_place_overseas_stock_order(
        environment: BrokerEnvironment,
        request: OverseasStockOrderRequest,
    ) -> Result<Self, BrokerError> {
        if request.quantity.is_zero() {
            return Err(BrokerError::validation(
                "journal command quantity must be greater than zero",
            ));
        }
        if request.order_type == OrderType::Limit && request.limit_price.is_none() {
            return Err(BrokerError::validation(
                "limit journal command requires limit_price",
            ));
        }

        let requested_at = OffsetDateTime::now_utc();

        Ok(Self {
            client_order_id: request.client_order_id,
            parent_client_order_id: None,
            decision_id: None,
            command_type: BrokerCommandType::Place,
            environment,
            account_key: request.account_key,
            symbol: request.instrument.symbol,
            side: request.side,
            quantity: request.quantity,
            limit_price: request.limit_price,
            order_type: request.order_type,
            requested_at,
            request_id: None,
            transport_send_started_at: None,
            broker_order_id: None,
            broker_order_orgno: None,
            target_broker_order_id: None,
            target_broker_order_orgno: None,
            cancel_or_amend_quantity: None,
            amend_price: None,
            exchange_code: Some(request.instrument.exchange.to_string()),
            last_known_state: BrokerCommandStatus::CommandRecorded,
            raw_ack_code: None,
            reconcile_required: false,
            last_reconciled_at: None,
            last_reconcile_reason: None,
            trading_day: Some(format_trading_day(requested_at)),
            broker_observation_window_start: Some(requested_at),
            broker_observation_window_end: Some(default_observation_window_end(requested_at)),
            sanitized_last_request_metadata_id: None,
            sanitized_last_response_metadata_id: None,
        })
    }

    pub fn new_cancel_order(
        parent: &BrokerCommandRecord,
        request: &CancelOrderRequest,
    ) -> Result<Self, BrokerError> {
        if !request.reference.is_complete_for_follow_up() {
            return Err(BrokerError::validation(
                "cancel order requires broker_order_id and broker_order_orgno",
            ));
        }

        let requested_at = OffsetDateTime::now_utc();

        Ok(Self {
            client_order_id: request.client_order_id.clone(),
            parent_client_order_id: Some(parent.client_order_id.clone()),
            decision_id: None,
            command_type: BrokerCommandType::Cancel,
            environment: request.reference.environment,
            account_key: request.reference.account_key.clone(),
            symbol: parent.symbol.clone(),
            side: parent.side,
            quantity: request.quantity.unwrap_or(parent.quantity),
            limit_price: parent.limit_price,
            order_type: parent.order_type,
            requested_at,
            request_id: None,
            transport_send_started_at: None,
            broker_order_id: None,
            broker_order_orgno: None,
            target_broker_order_id: request
                .reference
                .broker_order_id
                .as_ref()
                .map(ToString::to_string),
            target_broker_order_orgno: request
                .reference
                .broker_order_orgno
                .as_ref()
                .map(ToString::to_string),
            cancel_or_amend_quantity: request.quantity,
            amend_price: None,
            exchange_code: request
                .reference
                .exchange_code
                .as_ref()
                .map(ToString::to_string),
            last_known_state: BrokerCommandStatus::CommandRecorded,
            raw_ack_code: None,
            reconcile_required: false,
            last_reconciled_at: None,
            last_reconcile_reason: None,
            trading_day: parent
                .trading_day
                .clone()
                .or_else(|| Some(format_trading_day(requested_at))),
            broker_observation_window_start: Some(requested_at),
            broker_observation_window_end: Some(default_observation_window_end(requested_at)),
            sanitized_last_request_metadata_id: None,
            sanitized_last_response_metadata_id: None,
        })
    }

    pub fn new_amend_order(
        parent: &BrokerCommandRecord,
        request: &AmendOrderRequest,
    ) -> Result<Self, BrokerError> {
        if !request.reference.is_complete_for_follow_up() {
            return Err(BrokerError::validation(
                "amend order requires broker_order_id and broker_order_orgno",
            ));
        }
        if request.quantity.is_none() && request.limit_price.is_none() {
            return Err(BrokerError::validation(
                "amend order requires quantity or limit_price",
            ));
        }

        let requested_at = OffsetDateTime::now_utc();

        Ok(Self {
            client_order_id: request.client_order_id.clone(),
            parent_client_order_id: Some(parent.client_order_id.clone()),
            decision_id: None,
            command_type: BrokerCommandType::Amend,
            environment: request.reference.environment,
            account_key: request.reference.account_key.clone(),
            symbol: parent.symbol.clone(),
            side: parent.side,
            quantity: request.quantity.unwrap_or(parent.quantity),
            limit_price: request.limit_price.or(parent.limit_price),
            order_type: parent.order_type,
            requested_at,
            request_id: None,
            transport_send_started_at: None,
            broker_order_id: None,
            broker_order_orgno: None,
            target_broker_order_id: request
                .reference
                .broker_order_id
                .as_ref()
                .map(ToString::to_string),
            target_broker_order_orgno: request
                .reference
                .broker_order_orgno
                .as_ref()
                .map(ToString::to_string),
            cancel_or_amend_quantity: request.quantity,
            amend_price: request.limit_price,
            exchange_code: request
                .reference
                .exchange_code
                .as_ref()
                .map(ToString::to_string),
            last_known_state: BrokerCommandStatus::CommandRecorded,
            raw_ack_code: None,
            reconcile_required: false,
            last_reconciled_at: None,
            last_reconcile_reason: None,
            trading_day: parent
                .trading_day
                .clone()
                .or_else(|| Some(format_trading_day(requested_at))),
            broker_observation_window_start: Some(requested_at),
            broker_observation_window_end: Some(default_observation_window_end(requested_at)),
            sanitized_last_request_metadata_id: None,
            sanitized_last_response_metadata_id: None,
        })
    }
}

#[async_trait]
pub trait BrokerCommandJournal: Send + Sync {
    async fn get_command(
        &self,
        client_order_id: &ClientOrderId,
    ) -> Result<Option<BrokerCommandRecord>, BrokerError>;
    async fn reserve_command(&self, command: BrokerCommandRecord) -> Result<(), BrokerError>;
    async fn mark_submitting(
        &self,
        client_order_id: &ClientOrderId,
        request_id: &RequestId,
    ) -> Result<(), BrokerError>;
    async fn mark_ack(
        &self,
        client_order_id: &ClientOrderId,
        ack: BrokerCommandAck,
    ) -> Result<(), BrokerError>;
    async fn mark_reconcile_needed(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError>;
    async fn mark_reconciled(
        &self,
        client_order_id: &ClientOrderId,
        status: BrokerCommandStatus,
    ) -> Result<(), BrokerError>;
    async fn mark_reconcile_failed(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError>;
    async fn mark_terminal(
        &self,
        client_order_id: &ClientOrderId,
        status: BrokerCommandStatus,
    ) -> Result<(), BrokerError>;
    async fn mark_safety_halted(
        &self,
        client_order_id: &ClientOrderId,
        reason: &str,
    ) -> Result<(), BrokerError>;
    async fn list_unreconciled(&self) -> Result<Vec<BrokerCommandRecord>, BrokerError>;
}

fn format_trading_day(time: OffsetDateTime) -> String {
    let day = time.to_offset(time::macros::offset!(+9)).date();
    format!(
        "{:04}{:02}{:02}",
        day.year(),
        u8::from(day.month()),
        day.day()
    )
}

fn default_observation_window_end(start: OffsetDateTime) -> OffsetDateTime {
    start + time::Duration::days(1)
}
