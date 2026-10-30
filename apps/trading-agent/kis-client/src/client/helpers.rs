use crate::core::journal::BrokerCommandAck;
use crate::{
    AccountKey, BrokerOrderId, BrokerOrderOrgNo, CreateOrderRequest, OrderReference, OrderSide,
    OrderType, OverseasStockOrderRequest, Quantity, ReviseCancelCandidate, UsEquityOrderSession,
};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use time::macros::format_description;
use time::{Date, Month, OffsetDateTime, Time, UtcOffset, Weekday};

use crate::core::BrokerError;

const KRX_REGULAR_SESSION_OPEN: Time = time::macros::time!(09:00:00);
const KRX_REGULAR_SESSION_CLOSE: Time = time::macros::time!(15:30:00);
const US_DAYTIME_SESSION_OPEN_KST: Time = time::macros::time!(10:00:00);
const US_DAYTIME_SESSION_CLOSE_KST: Time = time::macros::time!(18:00:00);
const US_PREMARKET_OPEN_KST: Time = time::macros::time!(18:00:00);
const US_PREMARKET_CLOSE_KST: Time = time::macros::time!(23:30:00);
const US_REGULAR_OPEN_KST: Time = time::macros::time!(23:30:00);
const US_REGULAR_CLOSE_KST: Time = time::macros::time!(06:00:00);
const US_AFTERMARKET_OPEN_KST: Time = time::macros::time!(06:00:00);
const US_DST_PREMARKET_OPEN_KST: Time = time::macros::time!(17:00:00);
const US_DST_PREMARKET_CLOSE_KST: Time = time::macros::time!(22:30:00);
const US_DST_REGULAR_OPEN_KST: Time = time::macros::time!(22:30:00);
const US_DST_REGULAR_CLOSE_KST: Time = time::macros::time!(05:00:00);
const US_AFTERMARKET_CLOSE_KST: Time = time::macros::time!(07:00:00);

pub(super) fn split_kis_account_key(account_key: &AccountKey) -> Result<(&str, &str), BrokerError> {
    let (cano, product_code) = account_key.as_str().split_once('-').ok_or_else(|| {
        BrokerError::validation(
            "KIS account key must be formatted as 8-digit-number-2-digit-product",
        )
    })?;

    if cano.len() != 8
        || product_code.len() != 2
        || !cano.chars().all(|character| character.is_ascii_digit())
        || !product_code
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(BrokerError::validation(
            "KIS account key must be formatted as 8-digit-number-2-digit-product",
        ));
    }

    Ok((cano, product_code))
}

pub(super) fn format_kis_date(day: Date) -> String {
    format!(
        "{:04}{:02}{:02}",
        day.year(),
        u8::from(day.month()),
        day.day()
    )
}

pub(super) fn parse_kis_date(value: &str) -> Result<Date, BrokerError> {
    let format = format_description!("[year][month][day]");
    Date::parse(value, format)
        .map_err(|error| BrokerError::invariant(format!("invalid KIS date {value}: {error}")))
}

pub(super) fn current_kst_trading_day_at(now_utc: OffsetDateTime) -> String {
    format_kis_date(now_utc.to_offset(time::macros::offset!(+9)).date())
}

pub(super) fn current_us_equity_trading_day_at(now_utc: OffsetDateTime) -> String {
    format_kis_date(now_utc.to_offset(us_eastern_offset_at(now_utc)).date())
}

pub(crate) fn is_krx_regular_session_time(now_utc: OffsetDateTime) -> bool {
    let now_kst = now_utc.to_offset(time::macros::offset!(+9));
    if matches!(
        now_kst.date().weekday(),
        Weekday::Saturday | Weekday::Sunday
    ) {
        return false;
    }
    let now_kst_time = now_kst.time();
    now_kst_time >= KRX_REGULAR_SESSION_OPEN && now_kst_time < KRX_REGULAR_SESSION_CLOSE
}

pub(crate) fn is_us_equity_regular_session_time(now_utc: OffsetDateTime) -> bool {
    matches!(
        us_equity_order_session_at(now_utc),
        Some(UsEquityOrderSession::Regular)
    )
}

pub(crate) fn us_equity_order_session_at(now_utc: OffsetDateTime) -> Option<UsEquityOrderSession> {
    let now_kst = now_utc.to_offset(time::macros::offset!(+9));
    let now_kst_time = now_kst.time();
    let session = if is_us_eastern_dst_utc(now_utc) {
        us_equity_dst_order_session_for_kst_time(now_kst_time)
    } else {
        us_equity_standard_order_session_for_kst_time(now_kst_time)
    }?;

    match session {
        UsEquityOrderSession::Daytime => {
            if is_weekday(now_kst.date().weekday()) {
                Some(session)
            } else {
                None
            }
        }
        UsEquityOrderSession::Premarket
        | UsEquityOrderSession::Regular
        | UsEquityOrderSession::Aftermarket => {
            let now_eastern = now_utc.to_offset(us_eastern_offset_at(now_utc));
            if is_weekday(now_eastern.date().weekday()) {
                Some(session)
            } else {
                None
            }
        }
    }
}

pub(crate) fn us_equity_order_session_trading_day_at(
    now_utc: OffsetDateTime,
    session: UsEquityOrderSession,
) -> String {
    match session {
        UsEquityOrderSession::Daytime => current_kst_trading_day_at(now_utc),
        UsEquityOrderSession::Premarket
        | UsEquityOrderSession::Regular
        | UsEquityOrderSession::Aftermarket => current_us_equity_trading_day_at(now_utc),
    }
}

fn us_equity_standard_order_session_for_kst_time(
    now_kst_time: Time,
) -> Option<UsEquityOrderSession> {
    if time_in_range(now_kst_time, US_REGULAR_OPEN_KST, US_REGULAR_CLOSE_KST) {
        Some(UsEquityOrderSession::Regular)
    } else if time_in_range(
        now_kst_time,
        US_AFTERMARKET_OPEN_KST,
        US_AFTERMARKET_CLOSE_KST,
    ) {
        Some(UsEquityOrderSession::Aftermarket)
    } else if time_in_range(now_kst_time, US_PREMARKET_OPEN_KST, US_PREMARKET_CLOSE_KST) {
        Some(UsEquityOrderSession::Premarket)
    } else if time_in_range(
        now_kst_time,
        US_DAYTIME_SESSION_OPEN_KST,
        US_DAYTIME_SESSION_CLOSE_KST,
    ) {
        Some(UsEquityOrderSession::Daytime)
    } else {
        None
    }
}

fn us_equity_dst_order_session_for_kst_time(now_kst_time: Time) -> Option<UsEquityOrderSession> {
    if time_in_range(
        now_kst_time,
        US_DST_REGULAR_OPEN_KST,
        US_DST_REGULAR_CLOSE_KST,
    ) {
        Some(UsEquityOrderSession::Regular)
    } else if time_in_range(
        now_kst_time,
        US_DST_REGULAR_CLOSE_KST,
        US_AFTERMARKET_CLOSE_KST,
    ) {
        Some(UsEquityOrderSession::Aftermarket)
    } else if time_in_range(
        now_kst_time,
        US_DST_PREMARKET_OPEN_KST,
        US_DST_PREMARKET_CLOSE_KST,
    ) {
        Some(UsEquityOrderSession::Premarket)
    } else if time_in_range(
        now_kst_time,
        US_DAYTIME_SESSION_OPEN_KST,
        US_DST_PREMARKET_OPEN_KST,
    ) {
        Some(UsEquityOrderSession::Daytime)
    } else {
        None
    }
}

fn time_in_range(value: Time, start: Time, end: Time) -> bool {
    if start <= end {
        value >= start && value < end
    } else {
        value >= start || value < end
    }
}

fn is_weekday(weekday: Weekday) -> bool {
    !matches!(weekday, Weekday::Saturday | Weekday::Sunday)
}

fn us_eastern_offset_at(now_utc: OffsetDateTime) -> UtcOffset {
    if is_us_eastern_dst_utc(now_utc) {
        time::macros::offset!(-4)
    } else {
        time::macros::offset!(-5)
    }
}

fn is_us_eastern_dst_utc(now_utc: OffsetDateTime) -> bool {
    let year = now_utc.year();
    let dst_start = us_dst_transition_utc(year, Month::March, 2, time::macros::time!(07:00:00));
    let dst_end = us_dst_transition_utc(year, Month::November, 1, time::macros::time!(06:00:00));
    now_utc >= dst_start && now_utc < dst_end
}

fn us_dst_transition_utc(
    year: i32,
    month: Month,
    sunday_index: u8,
    time_utc: Time,
) -> OffsetDateTime {
    let mut day = Date::from_calendar_date(year, month, 1).expect("valid DST transition month");
    while day.weekday() != Weekday::Sunday {
        day = day.next_day().expect("month has a Sunday");
    }
    for _ in 1..sunday_index {
        for _ in 0..7 {
            day = day.next_day().expect("DST transition day exists");
        }
    }
    day.with_time(time_utc).assume_utc()
}

pub(super) fn is_definitive_write_rejection(error: &BrokerError) -> bool {
    matches!(
        error,
        BrokerError::Auth { .. }
            | BrokerError::Permission { .. }
            | BrokerError::RateLimit { .. }
            | BrokerError::Validation { .. }
            | BrokerError::MarketClosed { .. }
            | BrokerError::OrderRejected { .. }
    )
}

pub(super) fn add_business_days(start: Date, days: u16) -> Result<Date, BrokerError> {
    let mut day = start;
    for _ in 0..days {
        day = next_business_day(day)?;
    }
    Ok(day)
}

pub(super) fn next_business_day(day: Date) -> Result<Date, BrokerError> {
    let mut next = day
        .next_day()
        .ok_or_else(|| BrokerError::invariant("business day iteration overflowed"))?;
    while matches!(next.weekday(), Weekday::Saturday | Weekday::Sunday) {
        next = next
            .next_day()
            .ok_or_else(|| BrokerError::invariant("business day iteration overflowed"))?;
    }
    Ok(next)
}

pub(super) fn cash_order_body(order: &CreateOrderRequest) -> Result<Value, BrokerError> {
    let (cano, product_code) = split_kis_account_key(&order.account_key)?;
    let order_division = match order.order_type {
        OrderType::Limit => "00",
        OrderType::Market => "01",
    };
    let price = order
        .limit_price
        .filter(|_| order.order_type == OrderType::Limit)
        .unwrap_or_default();

    Ok(json!({
        "CANO": cano,
        "ACNT_PRDT_CD": product_code,
        "PDNO": order.symbol.as_str(),
        "ORD_DVSN": order_division,
        "ORD_QTY": order.quantity.get().to_string(),
        "ORD_UNPR": price.to_string(),
        "EXCG_ID_DVSN_CD": "KRX",
        "SLL_TYPE": if order.side == OrderSide::Sell { "01" } else { "" },
        "CNDT_PRIC": ""
    }))
}

pub(super) fn rvsecncl_order_body(
    reference: &OrderReference,
    revision_cancel_code: &str,
    quantity: Option<Quantity>,
    limit_price: Option<Decimal>,
) -> Result<Value, BrokerError> {
    let (cano, product_code) = split_kis_account_key(&reference.account_key)?;
    let broker_order_id = reference
        .broker_order_id
        .as_ref()
        .ok_or_else(|| BrokerError::validation("follow-up order requires broker_order_id"))?;
    let broker_order_orgno = reference
        .broker_order_orgno
        .as_ref()
        .ok_or_else(|| BrokerError::validation("follow-up order requires broker_order_orgno"))?;
    let quantity_all = if quantity.is_none() { "Y" } else { "N" };
    let order_quantity = quantity.map(Quantity::get).unwrap_or(0);
    let order_price = limit_price.unwrap_or_default();
    let exchange_code = reference
        .exchange_code
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "KRX".to_owned());

    Ok(json!({
        "CANO": cano,
        "ACNT_PRDT_CD": product_code,
        "KRX_FWDG_ORD_ORGNO": broker_order_orgno.as_str(),
        "ORGN_ODNO": broker_order_id.as_str(),
        "ORD_DVSN": "00",
        "RVSE_CNCL_DVSN_CD": revision_cancel_code,
        "ORD_QTY": order_quantity.to_string(),
        "ORD_UNPR": order_price.to_string(),
        "QTY_ALL_ORD_YN": quantity_all,
        "EXCG_ID_DVSN_CD": exchange_code,
        "CNDT_PRIC": ""
    }))
}

pub(super) fn overseas_stock_order_body(
    order: &OverseasStockOrderRequest,
) -> Result<Value, BrokerError> {
    let (cano, product_code) = split_kis_account_key(&order.account_key)?;
    if order.order_type != OrderType::Limit {
        return Err(BrokerError::validation(
            "overseas stock order currently supports limit orders only",
        ));
    }
    let price = order
        .limit_price
        .ok_or_else(|| BrokerError::validation("overseas stock limit order requires price"))?;

    Ok(json!({
        "CANO": cano,
        "ACNT_PRDT_CD": product_code,
        "OVRS_EXCG_CD": order.instrument.exchange.as_str(),
        "PDNO": order.instrument.symbol.as_str(),
        "ORD_QTY": order.quantity.get().to_string(),
        "OVRS_ORD_UNPR": price.to_string(),
        "CTAC_TLNO": "",
        "MGCO_APTM_ODNO": "",
        "SLL_TYPE": if order.side == OrderSide::Sell { "00" } else { "" },
        "ORD_SVR_DVSN_CD": "0",
        "ORD_DVSN": "00",
    }))
}

pub(super) fn overseas_stock_rvsecncl_order_body(
    parent: &crate::core::journal::BrokerCommandRecord,
    reference: &OrderReference,
    revision_cancel_code: &str,
    quantity: Quantity,
    limit_price: Option<Decimal>,
) -> Result<Value, BrokerError> {
    let (cano, product_code) = split_kis_account_key(&reference.account_key)?;
    let broker_order_id = reference
        .broker_order_id
        .as_ref()
        .ok_or_else(|| BrokerError::validation("follow-up order requires broker_order_id"))?;
    let exchange_code = reference
        .exchange_code
        .as_ref()
        .map(ToString::to_string)
        .or_else(|| parent.exchange_code.clone())
        .ok_or_else(|| {
            BrokerError::validation("overseas follow-up order requires exchange code")
        })?;
    let order_price = limit_price.unwrap_or_default();

    Ok(json!({
        "CANO": cano,
        "ACNT_PRDT_CD": product_code,
        "OVRS_EXCG_CD": exchange_code,
        "PDNO": parent.symbol.as_str(),
        "ORGN_ODNO": broker_order_id.as_str(),
        "RVSE_CNCL_DVSN_CD": revision_cancel_code,
        "ORD_QTY": quantity.get().to_string(),
        "OVRS_ORD_UNPR": order_price.to_string(),
        "MGCO_APTM_ODNO": "",
        "ORD_SVR_DVSN_CD": "0",
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FollowUpOperation {
    Cancel,
    Amend,
}

impl FollowUpOperation {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Cancel => "cancel",
            Self::Amend => "amend",
        }
    }
}

pub(super) fn revise_cancel_candidate_matches(
    expected: &OrderReference,
    actual: &ReviseCancelCandidate,
) -> bool {
    expected.account_key == actual.reference.account_key
        && expected.environment == actual.reference.environment
        && expected.broker_order_id == actual.reference.broker_order_id
        && expected.broker_order_orgno == actual.reference.broker_order_orgno
        && expected.exchange_code.as_ref().is_none_or(|exchange_code| {
            actual.reference.exchange_code.as_ref() == Some(exchange_code)
        })
}

pub(super) fn revise_cancel_candidate_key(candidate: &ReviseCancelCandidate) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        candidate
            .reference
            .broker_order_orgno
            .as_ref()
            .map(BrokerOrderOrgNo::to_string)
            .unwrap_or_default(),
        candidate
            .reference
            .broker_order_id
            .as_ref()
            .map(BrokerOrderId::to_string)
            .unwrap_or_default(),
        candidate.symbol,
        candidate
            .reference
            .exchange_code
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        candidate.side.as_str()
    )
}

pub(super) fn ack_from_reference(reference: &OrderReference) -> BrokerCommandAck {
    BrokerCommandAck {
        broker_order_id: reference
            .broker_order_id
            .as_ref()
            .map(BrokerOrderId::to_string),
        broker_order_orgno: reference
            .broker_order_orgno
            .as_ref()
            .map(BrokerOrderOrgNo::to_string),
        raw_ack_code: Some("0".to_owned()),
    }
}
