use rust_decimal::Decimal;
use serde_json::Value;

use super::common::{
    daily_order_observed_at, decimal_from_kis_string, decode_body, decode_envelope,
    ensure_order_result_code, ensure_result_code, map_kis_side, optional_decimal_from_kis_string,
    optional_kis_side, optional_quantity_from_kis_string, quantity_from_kis_string,
    required_string,
};
use crate::core::BrokerError;
use crate::protocol::{
    KisApiEnvelope, KisOverseasCountriesHolidayResponse, KisOverseasOrderListResponse,
    KisOverseasOrderOutput, KisOverseasOrderResponseOutput, KisOverseasStockBalanceResponse,
    KisOverseasStockBalanceSummary, KisOverseasStockBuyingPowerOutput, KisOverseasStockQuoteOutput,
};
use crate::{
    AccountKey, AmendOrderRequest, AmendResult, AmendableQuantity, BrokerEnvironment,
    BrokerOrderId, BrokerOrderOrgNo, CancelOrderRequest, CancelResult, CancelableQuantity,
    ClientOrderId, CurrencyCode, ExchangeCode, Fill, OrderReference, OrderState, OrderStatus,
    OrderStatusEvidence, OrderSubmission, OverseasStock, OverseasStockAccountSnapshot,
    OverseasStockBalanceRequest, OverseasStockBalanceSummary as DomainOverseasStockBalanceSummary,
    OverseasStockBuyingPower, OverseasStockOrderRequest, OverseasStockPosition, OverseasStockQuote,
    Quantity, ReviseCancelCandidate, Symbol,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverseasStockBalancePage {
    pub positions: Vec<OverseasStockPosition>,
    pub summary: DomainOverseasStockBalanceSummary,
    pub ctx_area_fk200: String,
    pub ctx_area_nk200: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverseasStockOrderPage {
    pub orders: Vec<OrderStatus>,
    pub fills: Vec<Fill>,
    pub ctx_area_fk200: String,
    pub ctx_area_nk200: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverseasStockOpenOrdersPage {
    pub candidates: Vec<ReviseCancelCandidate>,
    pub orders: Vec<OrderStatus>,
    pub ctx_area_fk200: String,
    pub ctx_area_nk200: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverseasStockMarketSessionPage {
    pub trading_day: String,
    pub is_open: bool,
    pub ctx_area_fk: String,
    pub ctx_area_nk: String,
}

pub fn map_overseas_stock_quote_response(
    body: &Value,
    instrument: &OverseasStock,
) -> Result<OverseasStockQuote, BrokerError> {
    let envelope: KisApiEnvelope<KisOverseasStockQuoteOutput> = decode_envelope(body)?;
    ensure_result_code(&envelope.rt_cd, &envelope.msg_cd, &envelope.msg1)?;
    let output = envelope
        .output
        .ok_or_else(|| BrokerError::invariant("KIS overseas quote response is missing output"))?;

    Ok(OverseasStockQuote {
        instrument: instrument.clone(),
        last_price: decimal_from_kis_string(output.last.as_deref(), "last")?,
    })
}

pub fn map_overseas_stock_market_session_page_response(
    body: &Value,
    exchange: &ExchangeCode,
    trading_day: &str,
) -> Result<OverseasStockMarketSessionPage, BrokerError> {
    let response: KisOverseasCountriesHolidayResponse = decode_body(body)?;
    ensure_result_code(&response.rt_cd, &response.msg_cd, &response.msg1)?;

    let mut is_open = false;
    for item in &response.output {
        let Some(market_code) = item.tr_mket_cd.as_deref() else {
            continue;
        };
        if !overseas_market_code_matches(exchange, market_code)? {
            continue;
        }

        let local_settlement_day = required_string(item.acpl_sttl_dt.as_deref(), "acpl_sttl_dt")?;
        let domestic_settlement_day =
            required_string(item.dmst_sttl_dt.as_deref(), "dmst_sttl_dt")?;
        validate_kis_date_field("acpl_sttl_dt", local_settlement_day)?;
        validate_kis_date_field("dmst_sttl_dt", domestic_settlement_day)?;
        is_open = true;
        break;
    }

    Ok(OverseasStockMarketSessionPage {
        trading_day: trading_day.to_owned(),
        is_open,
        ctx_area_fk: response.ctx_area_fk.unwrap_or_default(),
        ctx_area_nk: response.ctx_area_nk.unwrap_or_default(),
    })
}

pub fn map_overseas_stock_balance_page_response(
    body: &Value,
    request: &OverseasStockBalanceRequest,
) -> Result<OverseasStockBalancePage, BrokerError> {
    let response: KisOverseasStockBalanceResponse = decode_body(body)?;
    ensure_result_code(&response.rt_cd, &response.msg_cd, &response.msg1)?;

    let mut positions = Vec::new();
    for item in response.output1 {
        let quantity = quantity_from_kis_string(item.ovrs_cblc_qty.as_deref(), "ovrs_cblc_qty")?;
        if quantity.is_zero() {
            continue;
        }

        let exchange = item
            .ovrs_excg_cd
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ExchangeCode::new)
            .transpose()?
            .unwrap_or_else(|| request.exchange.clone());
        let currency = item
            .tr_crcy_cd
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(CurrencyCode::new)
            .transpose()?
            .unwrap_or_else(|| request.currency.clone());

        positions.push(OverseasStockPosition {
            instrument: OverseasStock::new(
                exchange,
                Symbol::new(required_string(item.ovrs_pdno.as_deref(), "ovrs_pdno")?)?,
            ),
            currency,
            quantity,
            average_price: decimal_from_kis_string(item.pchs_avg_pric.as_deref(), "pchs_avg_pric")?,
        });
    }

    let raw_summary = response.output2.ok_or_else(|| {
        BrokerError::invariant("KIS overseas balance response is missing output2")
    })?;
    let summary = map_balance_summary(raw_summary, &request.currency)?;

    Ok(OverseasStockBalancePage {
        positions,
        summary,
        ctx_area_fk200: response.ctx_area_fk200.unwrap_or_default(),
        ctx_area_nk200: response.ctx_area_nk200.unwrap_or_default(),
    })
}

fn overseas_market_code_matches(
    exchange: &ExchangeCode,
    market_code: &str,
) -> Result<bool, BrokerError> {
    let aliases = match exchange.as_str() {
        "NASD" => &["01", "NASD", "NAS"][..],
        "NYSE" => &["02", "NYSE", "NYS"][..],
        "AMEX" => &["05", "AMEX", "AMS"][..],
        other => {
            return Err(BrokerError::validation(format!(
                "overseas market session gate currently supports NASD, NYSE, and AMEX only: {other}"
            )));
        }
    };
    Ok(aliases
        .iter()
        .any(|alias| market_code.trim().eq_ignore_ascii_case(alias)))
}

fn validate_kis_date_field(name: &str, value: &str) -> Result<(), BrokerError> {
    if value.len() != 8 || !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(BrokerError::invariant(format!(
            "KIS overseas market session field {name} must be YYYYMMDD"
        )));
    }
    Ok(())
}

pub fn map_overseas_stock_account_snapshot(
    pages: Vec<OverseasStockBalancePage>,
) -> Result<OverseasStockAccountSnapshot, BrokerError> {
    let summary = pages
        .first()
        .map(|page| page.summary.clone())
        .ok_or_else(|| BrokerError::invariant("KIS overseas balance response returned no pages"))?;
    let positions = pages.into_iter().flat_map(|page| page.positions).collect();
    Ok(OverseasStockAccountSnapshot { summary, positions })
}

pub fn map_overseas_stock_buying_power_response(
    body: &Value,
) -> Result<OverseasStockBuyingPower, BrokerError> {
    let envelope: KisApiEnvelope<KisOverseasStockBuyingPowerOutput> = decode_envelope(body)?;
    ensure_result_code(&envelope.rt_cd, &envelope.msg_cd, &envelope.msg1)?;
    let output = envelope.output.ok_or_else(|| {
        BrokerError::invariant("KIS overseas buying power response is missing output")
    })?;
    let currency = CurrencyCode::new(required_string(output.tr_crcy_cd.as_deref(), "tr_crcy_cd")?)?;
    let available_cash = output
        .ovrs_ord_psbl_amt
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            output
                .ord_psbl_frcr_amt
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            output
                .frcr_ord_psbl_amt1
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        });
    let max_order_quantity = output
        .max_ord_psbl_qty
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            output
                .ovrs_max_ord_psbl_qty
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            output
                .ord_psbl_qty
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .map(|value| quantity_from_kis_string(Some(value), "max_ord_psbl_qty"))
        .transpose()?;

    Ok(OverseasStockBuyingPower {
        currency,
        available_cash: decimal_from_kis_string(available_cash, "ovrs_ord_psbl_amt")?,
        max_order_quantity,
    })
}

pub fn map_overseas_stock_order_submission_response(
    body: &Value,
    request: &OverseasStockOrderRequest,
    environment: BrokerEnvironment,
) -> Result<OrderSubmission, BrokerError> {
    let output = decode_overseas_order_output(body)?;
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
            exchange_code: Some(request.instrument.exchange.clone()),
            account_key: request.account_key.clone(),
            environment,
        },
        accepted: true,
    })
}

pub fn map_overseas_stock_cancel_result_response(
    body: &Value,
    request: &CancelOrderRequest,
) -> Result<CancelResult, BrokerError> {
    let _output = decode_overseas_order_output(body)?;
    Ok(CancelResult {
        reference: request.reference.clone(),
        accepted: true,
    })
}

pub fn map_overseas_stock_amend_result_response(
    body: &Value,
    request: &AmendOrderRequest,
) -> Result<AmendResult, BrokerError> {
    let _output = decode_overseas_order_output(body)?;
    Ok(AmendResult {
        reference: request.reference.clone(),
        accepted: true,
    })
}

pub fn map_overseas_stock_open_orders_response(
    body: &Value,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<OverseasStockOpenOrdersPage, BrokerError> {
    let response: KisOverseasOrderListResponse = decode_body(body)?;
    ensure_result_code(&response.rt_cd, &response.msg_cd, &response.msg1)?;

    let mut candidates = Vec::new();
    let mut orders = Vec::new();
    for item in response.output {
        let reference = overseas_order_reference_from_item(&item, account_key, environment)?;
        let evidence = overseas_order_evidence_from_item(&item)?;
        let ordered_quantity = quantity_from_kis_string(item.ft_ord_qty.as_deref(), "ft_ord_qty")?;
        let filled_quantity = optional_quantity_from_kis_string(item.ft_ccld_qty.as_deref())?;
        let rejected_quantity = optional_quantity_from_kis_string(item.rjct_qty.as_deref())?;
        let cancelled = item
            .cncl_yn
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| value == "Y");
        orders.push(OrderStatus {
            reference,
            state: map_overseas_order_state(
                ordered_quantity,
                filled_quantity,
                rejected_quantity,
                cancelled,
            ),
            evidence: Some(evidence),
        });
        candidates.push(overseas_revise_cancel_candidate_from_item(
            &item,
            account_key,
            environment,
        )?);
    }

    Ok(OverseasStockOpenOrdersPage {
        candidates,
        orders,
        ctx_area_fk200: response.ctx_area_fk200.unwrap_or_default(),
        ctx_area_nk200: response.ctx_area_nk200.unwrap_or_default(),
    })
}

pub fn map_overseas_stock_order_history_response(
    body: &Value,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<OverseasStockOrderPage, BrokerError> {
    let response: KisOverseasOrderListResponse = decode_body(body)?;
    ensure_result_code(&response.rt_cd, &response.msg_cd, &response.msg1)?;

    let mut orders = Vec::new();
    let mut fills = Vec::new();
    for item in response.output {
        let reference = overseas_order_reference_from_item(&item, account_key, environment)?;
        let evidence = overseas_order_evidence_from_item(&item)?;
        let ordered_quantity = quantity_from_kis_string(item.ft_ord_qty.as_deref(), "ft_ord_qty")?;
        let filled_quantity = optional_quantity_from_kis_string(item.ft_ccld_qty.as_deref())?;
        let rejected_quantity = optional_quantity_from_kis_string(item.rjct_qty.as_deref())?;
        let cancelled = item
            .cncl_yn
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| value == "Y");

        orders.push(OrderStatus {
            reference: reference.clone(),
            state: map_overseas_order_state(
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
                price: decimal_from_kis_string(item.ft_ccld_unpr3.as_deref(), "ft_ccld_unpr3")?,
                evidence: Some(evidence),
            });
        }
    }

    Ok(OverseasStockOrderPage {
        orders,
        fills,
        ctx_area_fk200: response.ctx_area_fk200.unwrap_or_default(),
        ctx_area_nk200: response.ctx_area_nk200.unwrap_or_default(),
    })
}

pub fn map_overseas_stock_order_fills_response(
    body: &Value,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<Vec<Fill>, BrokerError> {
    Ok(map_overseas_stock_order_history_response(body, account_key, environment)?.fills)
}

fn map_balance_summary(
    raw_summary: Value,
    currency: &CurrencyCode,
) -> Result<DomainOverseasStockBalanceSummary, BrokerError> {
    let summary_value = match raw_summary {
        Value::Array(items) => items
            .into_iter()
            .next()
            .ok_or_else(|| BrokerError::invariant("KIS overseas balance output2 is empty"))?,
        other => other,
    };
    let summary: KisOverseasStockBalanceSummary =
        serde_json::from_value(summary_value).map_err(|error| BrokerError::Transport {
            message: format!("failed to decode KIS overseas balance summary: {error}"),
        })?;

    Ok(DomainOverseasStockBalanceSummary {
        currency: currency.clone(),
        purchase_amount: optional_decimal_from_kis_string(
            summary.frcr_pchs_amt1.as_deref(),
            "frcr_pchs_amt1",
        )?
        .unwrap_or(Decimal::ZERO),
        evaluation_amount: optional_decimal_from_kis_string(
            summary.ovrs_stck_evlu_amt.as_deref(),
            "ovrs_stck_evlu_amt",
        )?
        .unwrap_or(Decimal::ZERO),
        profit_loss: optional_decimal_from_kis_string(
            summary.ovrs_tot_pfls.as_deref(),
            "ovrs_tot_pfls",
        )?
        .unwrap_or(Decimal::ZERO),
    })
}

fn decode_overseas_order_output(
    body: &Value,
) -> Result<KisOverseasOrderResponseOutput, BrokerError> {
    let envelope: KisApiEnvelope<KisOverseasOrderResponseOutput> = decode_envelope(body)?;
    ensure_order_result_code(&envelope.rt_cd, &envelope.msg_cd, &envelope.msg1)?;
    envelope
        .output
        .ok_or_else(|| BrokerError::invariant("KIS overseas order response is missing output"))
}

fn overseas_order_reference_from_item(
    item: &KisOverseasOrderOutput,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<OrderReference, BrokerError> {
    let broker_order_id = required_string(item.odno.as_deref(), "odno")?;
    let broker_order_orgno = required_string(item.ord_gno_brno.as_deref(), "ord_gno_brno")?;

    Ok(OrderReference {
        client_order_id: ClientOrderId::new(format!("kis-ovrs-{broker_order_id}"))?,
        broker_order_id: Some(BrokerOrderId::new(broker_order_id)?),
        broker_order_orgno: Some(BrokerOrderOrgNo::new(broker_order_orgno)?),
        exchange_code: item
            .ovrs_excg_cd
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ExchangeCode::new)
            .transpose()?,
        account_key: account_key.clone(),
        environment,
    })
}

fn overseas_order_evidence_from_item(
    item: &KisOverseasOrderOutput,
) -> Result<OrderStatusEvidence, BrokerError> {
    Ok(OrderStatusEvidence {
        symbol: Symbol::new(required_string(item.pdno.as_deref(), "pdno")?)?,
        side: optional_kis_side(item.sll_buy_dvsn_cd.as_deref())?,
        ordered_quantity: quantity_from_kis_string(item.ft_ord_qty.as_deref(), "ft_ord_qty")?,
        filled_quantity: optional_quantity_from_kis_string(item.ft_ccld_qty.as_deref())?,
        order_price: optional_decimal_from_kis_string(
            item.ft_ord_unpr3.as_deref(),
            "ft_ord_unpr3",
        )?,
        trading_day: item
            .ord_dt
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        observed_at: daily_order_observed_at(item.ord_dt.as_deref(), item.ord_tmd.as_deref())?,
    })
}

fn overseas_revise_cancel_candidate_from_item(
    item: &KisOverseasOrderOutput,
    account_key: &AccountKey,
    environment: BrokerEnvironment,
) -> Result<ReviseCancelCandidate, BrokerError> {
    let possible_quantity = possible_overseas_quantity(item)?;
    let reference = overseas_order_reference_from_item(item, account_key, environment)?;

    Ok(ReviseCancelCandidate {
        reference,
        symbol: Symbol::new(required_string(item.pdno.as_deref(), "pdno")?)?,
        side: map_kis_side(required_string(
            item.sll_buy_dvsn_cd.as_deref(),
            "sll_buy_dvsn_cd",
        )?)?,
        ordered_quantity: quantity_from_kis_string(item.ft_ord_qty.as_deref(), "ft_ord_qty")?,
        filled_quantity: optional_quantity_from_kis_string(item.ft_ccld_qty.as_deref())?,
        order_price: optional_decimal_from_kis_string(
            item.ft_ord_unpr3.as_deref(),
            "ft_ord_unpr3",
        )?,
        cancelable_quantity: CancelableQuantity::new(possible_quantity),
        amendable_quantity: AmendableQuantity::new(possible_quantity),
    })
}

fn possible_overseas_quantity(item: &KisOverseasOrderOutput) -> Result<Quantity, BrokerError> {
    let nccs_quantity = optional_quantity_from_kis_string(item.nccs_qty.as_deref())?;
    if !nccs_quantity.is_zero() {
        return Ok(nccs_quantity);
    }

    let ordered_quantity = quantity_from_kis_string(item.ft_ord_qty.as_deref(), "ft_ord_qty")?;
    let filled_quantity = optional_quantity_from_kis_string(item.ft_ccld_qty.as_deref())?;
    Ok(Quantity::new(
        ordered_quantity.get().saturating_sub(filled_quantity.get()),
    ))
}

fn map_overseas_order_state(
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
