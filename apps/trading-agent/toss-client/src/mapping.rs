use std::str::FromStr;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use trading_agent_kis_client::{
    AccountKey, BrokerEnvironment, BrokerError, BrokerOrderId, OrderReference, OrderSide,
    OrderState, OrderStatus, OrderStatusEvidence, Position, Quantity, Symbol,
};

use crate::client::{RawHoldingItem, RawOrder, TossAccountSeq};

pub fn decimal_from_str(field: &'static str, value: &str) -> Result<Decimal, BrokerError> {
    Decimal::from_str(value).map_err(|error| {
        BrokerError::invariant(format!("Toss field {field} must be a decimal: {error}"))
    })
}

pub fn quantity_from_decimal_str(
    field: &'static str,
    value: &str,
) -> Result<Quantity, BrokerError> {
    let decimal = decimal_from_str(field, value)?;
    if decimal < Decimal::ZERO || decimal.fract() != Decimal::ZERO {
        return Err(BrokerError::invariant(format!(
            "Toss field {field} must be a non-negative integer quantity for the shared broker domain"
        )));
    }
    let quantity = decimal.to_u64().ok_or_else(|| {
        BrokerError::invariant(format!("Toss field {field} is too large for Quantity"))
    })?;
    Ok(Quantity::new(quantity))
}

pub(crate) fn position_from_holding(item: RawHoldingItem) -> Result<Position, BrokerError> {
    Ok(Position {
        symbol: Symbol::new(item.symbol)?,
        quantity: quantity_from_decimal_str("holdings.items[].quantity", &item.quantity)?,
        average_price: decimal_from_str(
            "holdings.items[].averagePurchasePrice",
            &item.average_purchase_price,
        )?,
    })
}

pub(crate) fn order_status_from_raw(
    account_seq: TossAccountSeq,
    order: RawOrder,
) -> Result<OrderStatus, BrokerError> {
    let symbol = Symbol::new(order.symbol)?;
    let reference = OrderReference {
        client_order_id: crate::client::synthetic_client_order_id(&order.order_id)?,
        broker_order_id: Some(BrokerOrderId::new(order.order_id.as_str())?),
        broker_order_orgno: None,
        exchange_code: None,
        account_key: synthetic_account_key(account_seq)?,
        environment: BrokerEnvironment::Live,
    };
    let side = match order.side.as_str() {
        "BUY" => Some(OrderSide::Buy),
        "SELL" => Some(OrderSide::Sell),
        _ => None,
    };
    Ok(OrderStatus {
        reference,
        state: order_state(order.status.as_str()),
        evidence: Some(OrderStatusEvidence {
            symbol,
            side,
            ordered_quantity: quantity_from_decimal_str("orders[].quantity", &order.quantity)?,
            filled_quantity: quantity_from_decimal_str(
                "orders[].execution.filledQuantity",
                &order.execution.filled_quantity,
            )?,
            order_price: order
                .price
                .as_deref()
                .map(|price| decimal_from_str("orders[].price", price))
                .transpose()?,
            trading_day: Some(order.ordered_at.chars().take(10).collect()),
            observed_at: None,
        }),
    })
}

pub fn synthetic_account_key(account_seq: TossAccountSeq) -> Result<AccountKey, BrokerError> {
    AccountKey::new(format!("toss-{}", account_seq.as_u64()))
}

fn order_state(status: &str) -> OrderState {
    match status {
        "PENDING" | "PENDING_CANCEL" | "PENDING_REPLACE" => OrderState::Accepted,
        "PARTIAL_FILLED" => OrderState::PartiallyFilled,
        "FILLED" => OrderState::Filled,
        "CANCELED" => OrderState::Cancelled,
        "REJECTED" | "CANCEL_REJECTED" | "REPLACE_REJECTED" => OrderState::Rejected,
        other => OrderState::Unknown(other.to_owned()),
    }
}
