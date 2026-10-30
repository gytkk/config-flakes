use serde_json::Value;

use super::common::{
    daily_order_observed_at, decimal_from_kis_string, decode_body, decode_envelope,
    ensure_order_result_code, ensure_result_code, map_kis_side, optional_decimal_from_kis_string,
    optional_kis_side, optional_quantity_from_kis_string, quantity_from_kis_string,
    required_string,
};
use crate::core::BrokerError;
use crate::protocol::{
    KisApiEnvelope, KisDailyCcldResponse, KisDailyOrderOutput, KisOrderResponseOutput,
    KisReviseCancelPossibleOutput, KisReviseCancelPossibleResponse,
};
use crate::{
    AccountKey, AmendOrderRequest, AmendResult, AmendableQuantity, BrokerEnvironment,
    BrokerOrderId, BrokerOrderOrgNo, CancelOrderRequest, CancelResult, CancelableQuantity,
    ClientOrderId, CreateOrderRequest, ExchangeCode, Fill, OrderReference, OrderState, OrderStatus,
    OrderStatusEvidence, OrderSubmission, Quantity, ReviseCancelCandidate, Symbol,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyCcldPage {
    pub orders: Vec<OrderStatus>,
    pub fills: Vec<Fill>,
    pub ctx_area_fk100: String,
    pub ctx_area_nk100: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviseCancelPossiblePage {
    pub candidates: Vec<ReviseCancelCandidate>,
    pub ctx_area_fk100: String,
    pub ctx_area_nk100: String,
}

pub fn map_daily_ccld_page_response(
    body: &Value,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<DailyCcldPage, BrokerError> {
    let response: KisDailyCcldResponse = decode_body(body)?;
    ensure_result_code(&response.rt_cd, &response.msg_cd, &response.msg1)?;

    let mut orders = Vec::new();
    let mut fills = Vec::new();
    for item in response.output1 {
        let reference = order_reference_from_daily_item(&item, account_key, environment)?;
        let evidence = order_evidence_from_daily_item(&item)?;
        let ordered_quantity = quantity_from_kis_string(item.ord_qty.as_deref(), "ord_qty")?;
        let filled_quantity =
            quantity_from_kis_string(item.tot_ccld_qty.as_deref(), "tot_ccld_qty")?;
        let rejected_quantity = optional_quantity_from_kis_string(item.rjct_qty.as_deref())?;
        let cancelled = item
            .cncl_yn
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| value == "Y");

        orders.push(OrderStatus {
            reference: reference.clone(),
            state: map_order_state(
                ordered_quantity,
                filled_quantity,
                rejected_quantity,
                cancelled,
            ),
            evidence: Some(evidence.clone()),
        });

        if !filled_quantity.is_zero() {
            fills.push(Fill {
                reference,
                quantity: filled_quantity,
                price: decimal_from_kis_string(item.avg_prvs.as_deref(), "avg_prvs")?,
                evidence: Some(evidence),
            });
        }
    }

    Ok(DailyCcldPage {
        orders,
        fills,
        ctx_area_fk100: response.ctx_area_fk100.unwrap_or_default(),
        ctx_area_nk100: response.ctx_area_nk100.unwrap_or_default(),
    })
}

pub fn map_daily_orders_response(
    body: &Value,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<Vec<OrderStatus>, BrokerError> {
    Ok(map_daily_ccld_page_response(body, account_key, environment)?.orders)
}

pub fn map_daily_fills_response(
    body: &Value,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<Vec<Fill>, BrokerError> {
    Ok(map_daily_ccld_page_response(body, account_key, environment)?.fills)
}

pub fn map_revise_cancel_possible_response(
    body: &Value,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<ReviseCancelPossiblePage, BrokerError> {
    let response: KisReviseCancelPossibleResponse = decode_body(body)?;
    ensure_result_code(&response.rt_cd, &response.msg_cd, &response.msg1)?;

    let mut candidates = Vec::new();
    for item in response.output {
        candidates.push(revise_cancel_candidate_from_item(
            &item,
            account_key,
            environment,
        )?);
    }

    Ok(ReviseCancelPossiblePage {
        candidates,
        ctx_area_fk100: response.ctx_area_fk100.unwrap_or_default(),
        ctx_area_nk100: response.ctx_area_nk100.unwrap_or_default(),
    })
}

pub fn map_order_submission_response(
    body: &Value,
    request: &CreateOrderRequest,
    environment: BrokerEnvironment,
) -> Result<OrderSubmission, BrokerError> {
    let output = decode_order_output(body)?;
    Ok(OrderSubmission {
        reference: OrderReference {
            client_order_id: request.client_order_id.clone(),
            broker_order_id: Some(BrokerOrderId::new(required_string(
                output.odno.as_deref(),
                "ODNO",
            )?)?),
            broker_order_orgno: Some(BrokerOrderOrgNo::new(required_string(
                output.krx_fwdg_ord_orgno.as_deref(),
                "KRX_FWDG_ORD_ORGNO",
            )?)?),
            exchange_code: Some(ExchangeCode::new("KRX")?),
            account_key: request.account_key.clone(),
            environment,
        },
        accepted: true,
    })
}

pub fn map_cancel_result_response(
    body: &Value,
    request: &CancelOrderRequest,
) -> Result<CancelResult, BrokerError> {
    let _output = decode_order_output(body)?;
    Ok(CancelResult {
        reference: request.reference.clone(),
        accepted: true,
    })
}

pub fn map_amend_result_response(
    body: &Value,
    request: &AmendOrderRequest,
) -> Result<AmendResult, BrokerError> {
    let _output = decode_order_output(body)?;
    Ok(AmendResult {
        reference: request.reference.clone(),
        accepted: true,
    })
}

fn decode_order_output(body: &Value) -> Result<KisOrderResponseOutput, BrokerError> {
    let envelope: KisApiEnvelope<KisOrderResponseOutput> = decode_envelope(body)?;
    ensure_order_result_code(&envelope.rt_cd, &envelope.msg_cd, &envelope.msg1)?;
    envelope
        .output
        .ok_or_else(|| BrokerError::invariant("KIS order response is missing output"))
}

fn order_reference_from_daily_item(
    item: &KisDailyOrderOutput,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<OrderReference, BrokerError> {
    let broker_order_id = required_string(item.odno.as_deref(), "odno")?;
    let broker_order_orgno = required_string(item.ord_gno_brno.as_deref(), "ord_gno_brno")?;

    Ok(OrderReference {
        client_order_id: ClientOrderId::new(format!("kis-{broker_order_id}"))?,
        broker_order_id: Some(BrokerOrderId::new(broker_order_id)?),
        broker_order_orgno: Some(BrokerOrderOrgNo::new(broker_order_orgno)?),
        exchange_code: item
            .excg_id_dvsn_cd
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                item.excg_dvsn_cd
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
            })
            .map(ExchangeCode::new)
            .transpose()?,
        account_key: account_key.clone(),
        environment,
    })
}

fn order_evidence_from_daily_item(
    item: &KisDailyOrderOutput,
) -> Result<OrderStatusEvidence, BrokerError> {
    Ok(OrderStatusEvidence {
        symbol: Symbol::new(required_string(item.pdno.as_deref(), "pdno")?)?,
        side: optional_kis_side(item.sll_buy_dvsn_cd.as_deref())?,
        ordered_quantity: quantity_from_kis_string(item.ord_qty.as_deref(), "ord_qty")?,
        filled_quantity: quantity_from_kis_string(item.tot_ccld_qty.as_deref(), "tot_ccld_qty")?,
        order_price: optional_decimal_from_kis_string(item.ord_unpr.as_deref(), "ord_unpr")?,
        trading_day: item
            .ord_dt
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        observed_at: daily_order_observed_at(item.ord_dt.as_deref(), item.ord_tmd.as_deref())?,
    })
}

fn revise_cancel_candidate_from_item(
    item: &KisReviseCancelPossibleOutput,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<ReviseCancelCandidate, BrokerError> {
    let broker_order_id = required_string(item.odno.as_deref(), "odno")?;
    let broker_order_orgno = required_string(item.ord_gno_brno.as_deref(), "ord_gno_brno")?;
    let possible_quantity = quantity_from_kis_string(item.psbl_qty.as_deref(), "psbl_qty")?;

    Ok(ReviseCancelCandidate {
        reference: OrderReference {
            client_order_id: ClientOrderId::new(format!("kis-{broker_order_id}"))?,
            broker_order_id: Some(BrokerOrderId::new(broker_order_id)?),
            broker_order_orgno: Some(BrokerOrderOrgNo::new(broker_order_orgno)?),
            exchange_code: item
                .excg_id_dvsn_cd
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    item.excg_dvsn_cd
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                })
                .map(ExchangeCode::new)
                .transpose()?,
            account_key: account_key.clone(),
            environment,
        },
        symbol: Symbol::new(required_string(item.pdno.as_deref(), "pdno")?)?,
        side: map_kis_side(required_string(
            item.sll_buy_dvsn_cd.as_deref(),
            "sll_buy_dvsn_cd",
        )?)?,
        ordered_quantity: quantity_from_kis_string(item.ord_qty.as_deref(), "ord_qty")?,
        filled_quantity: quantity_from_kis_string(item.tot_ccld_qty.as_deref(), "tot_ccld_qty")?,
        order_price: optional_decimal_from_kis_string(item.ord_unpr.as_deref(), "ord_unpr")?,
        cancelable_quantity: CancelableQuantity::new(possible_quantity),
        amendable_quantity: AmendableQuantity::new(possible_quantity),
    })
}

fn map_order_state(
    ordered_quantity: Quantity,
    filled_quantity: Quantity,
    rejected_quantity: Quantity,
    cancelled: bool,
) -> OrderState {
    if cancelled {
        return OrderState::Cancelled;
    }
    if !rejected_quantity.is_zero() {
        return OrderState::Rejected;
    }
    if filled_quantity.is_zero() {
        return OrderState::Accepted;
    }
    if filled_quantity.get() < ordered_quantity.get() {
        return OrderState::PartiallyFilled;
    }
    OrderState::Filled
}
