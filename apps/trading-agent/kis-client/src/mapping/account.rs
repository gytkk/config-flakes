use serde_json::Value;

use super::common::{
    decimal_from_kis_string, decode_body, decode_envelope, ensure_result_code, required_string,
};
use crate::core::BrokerError;
use crate::protocol::{KisApiEnvelope, KisBalanceResponse, KisBuyingPowerOutput};
use crate::{BuyingPower, CashBalance, Position, Symbol};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalancePage {
    pub positions: Vec<Position>,
    pub cash: CashBalance,
    pub ctx_area_fk100: String,
    pub ctx_area_nk100: String,
}

pub fn map_balance_page_response(body: &Value) -> Result<BalancePage, BrokerError> {
    let response: KisBalanceResponse = decode_body(body)?;
    ensure_result_code(&response.rt_cd, &response.msg_cd, &response.msg1)?;
    let cash = response
        .output2
        .first()
        .ok_or_else(|| BrokerError::invariant("KIS balance response is missing output2"))
        .and_then(|summary| {
            Ok(CashBalance {
                cash: decimal_from_kis_string(summary.dnca_tot_amt.as_deref(), "dnca_tot_amt")?,
            })
        })?;

    let mut positions = Vec::new();
    for item in response.output1 {
        let quantity =
            super::common::quantity_from_kis_string(item.hldg_qty.as_deref(), "hldg_qty")?;
        if quantity.is_zero() {
            continue;
        }

        positions.push(Position {
            symbol: Symbol::new(required_string(item.pdno.as_deref(), "pdno")?)?,
            quantity,
            average_price: decimal_from_kis_string(item.pchs_avg_pric.as_deref(), "pchs_avg_pric")?,
        });
    }

    Ok(BalancePage {
        positions,
        cash,
        ctx_area_fk100: response.ctx_area_fk100.unwrap_or_default(),
        ctx_area_nk100: response.ctx_area_nk100.unwrap_or_default(),
    })
}

pub fn map_positions_response(body: &Value) -> Result<Vec<Position>, BrokerError> {
    Ok(map_balance_page_response(body)?.positions)
}

pub fn map_cash_balance_response(body: &Value) -> Result<CashBalance, BrokerError> {
    Ok(map_balance_page_response(body)?.cash)
}

pub fn map_buying_power_response(body: &Value) -> Result<BuyingPower, BrokerError> {
    let envelope: KisApiEnvelope<KisBuyingPowerOutput> = decode_envelope(body)?;
    ensure_result_code(&envelope.rt_cd, &envelope.msg_cd, &envelope.msg1)?;
    let output = envelope
        .output
        .ok_or_else(|| BrokerError::invariant("KIS buying power response is missing output"))?;

    let available_cash = output
        .nrcvb_buy_amt
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            output
                .ord_psbl_cash
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            output
                .max_buy_amt
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        });

    Ok(BuyingPower {
        available_cash: decimal_from_kis_string(available_cash, "nrcvb_buy_amt")?,
    })
}
