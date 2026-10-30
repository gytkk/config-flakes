use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::str::FromStr;
use time::macros::format_description;
use time::{Date, PrimitiveDateTime, Time, UtcOffset};

use crate::core::BrokerError;
use crate::protocol::KisApiEnvelope;
use crate::{OrderSide, Quantity};

pub(crate) fn decode_envelope<T>(body: &Value) -> Result<KisApiEnvelope<T>, BrokerError>
where
    T: DeserializeOwned,
{
    decode_body(body)
}

pub(crate) fn decode_body<T>(body: &Value) -> Result<T, BrokerError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(body.clone()).map_err(|error| BrokerError::Transport {
        message: format!("failed to decode KIS response body: {error}"),
    })
}

pub(crate) fn ensure_result_code(rt_cd: &str, msg_cd: &str, msg1: &str) -> Result<(), BrokerError> {
    if rt_cd == "0" {
        return Ok(());
    }

    Err(BrokerError::Transport {
        message: format!("KIS returned error {msg_cd}: {msg1}"),
    })
}

pub(crate) fn ensure_order_result_code(
    rt_cd: &str,
    msg_cd: &str,
    msg1: &str,
) -> Result<(), BrokerError> {
    if rt_cd == "0" {
        return Ok(());
    }

    Err(BrokerError::OrderRejected {
        message: format!("KIS order rejected {msg_cd}: {msg1}"),
    })
}

pub(crate) fn required_string<'a>(
    value: Option<&'a str>,
    field_name: &str,
) -> Result<&'a str, BrokerError> {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return Err(BrokerError::invariant(format!(
            "KIS response field {field_name} must not be empty"
        )));
    }
    Ok(value)
}

pub(crate) fn decimal_from_kis_string(
    value: Option<&str>,
    field_name: &str,
) -> Result<Decimal, BrokerError> {
    let value = required_string(value, field_name)?.replace(',', "");
    Decimal::from_str(&value).map_err(|error| {
        BrokerError::invariant(format!("invalid KIS decimal field {field_name}: {error}"))
    })
}

pub(crate) fn quantity_from_kis_string(
    value: Option<&str>,
    field_name: &str,
) -> Result<Quantity, BrokerError> {
    let value = required_string(value, field_name)?.replace(',', "");
    let quantity = value.parse::<u64>().map_err(|error| {
        BrokerError::invariant(format!("invalid KIS quantity field {field_name}: {error}"))
    })?;
    Ok(Quantity::new(quantity))
}

pub(crate) fn optional_quantity_from_kis_string(
    value: Option<&str>,
) -> Result<Quantity, BrokerError> {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return Ok(Quantity::new(0));
    }
    quantity_from_kis_string(Some(value), "optional_quantity")
}

pub(crate) fn optional_decimal_from_kis_string(
    value: Option<&str>,
    field_name: &str,
) -> Result<Option<Decimal>, BrokerError> {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return Ok(None);
    }
    Ok(Some(decimal_from_kis_string(Some(value), field_name)?))
}

pub(crate) fn map_kis_side(value: &str) -> Result<OrderSide, BrokerError> {
    match value.trim() {
        "01" => Ok(OrderSide::Sell),
        "02" => Ok(OrderSide::Buy),
        other => Err(BrokerError::invariant(format!(
            "unknown KIS side code for revise/cancel possible row: {other}"
        ))),
    }
}

pub(crate) fn optional_kis_side(value: Option<&str>) -> Result<Option<OrderSide>, BrokerError> {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return Ok(None);
    }
    map_kis_side(value).map(Some)
}

pub(crate) fn daily_order_observed_at(
    ord_dt: Option<&str>,
    ord_tmd: Option<&str>,
) -> Result<Option<time::OffsetDateTime>, BrokerError> {
    let ord_dt = ord_dt.unwrap_or("").trim();
    let ord_tmd = ord_tmd.unwrap_or("").trim();
    if ord_dt.is_empty() || ord_tmd.is_empty() {
        return Ok(None);
    }

    let date_format = format_description!("[year][month][day]");
    let time_format = format_description!("[hour][minute][second]");
    let date = Date::parse(ord_dt, date_format).map_err(|error| {
        BrokerError::invariant(format!(
            "invalid KIS daily order date field ord_dt: {error}"
        ))
    })?;
    let time = Time::parse(ord_tmd, time_format).map_err(|error| {
        BrokerError::invariant(format!(
            "invalid KIS daily order time field ord_tmd: {error}"
        ))
    })?;
    let kst_offset = UtcOffset::from_hms(9, 0, 0)
        .map_err(|error| BrokerError::invariant(format!("invalid KST offset: {error}")))?;

    Ok(Some(
        PrimitiveDateTime::new(date, time)
            .assume_offset(kst_offset)
            .to_offset(UtcOffset::UTC),
    ))
}
