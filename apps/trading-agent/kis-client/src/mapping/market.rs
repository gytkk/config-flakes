use serde_json::Value;

use super::common::{
    decimal_from_kis_string, decode_body, decode_envelope, ensure_result_code, required_string,
};
use crate::core::BrokerError;
use crate::protocol::{
    KisApiEnvelope, KisHolidayResponse, KisQuoteOutput, KisSearchStockInfoOutput,
};
use crate::{MarketSession, Quote, Symbol};

pub fn map_quote_response(body: &Value) -> Result<Quote, BrokerError> {
    let envelope: KisApiEnvelope<KisQuoteOutput> = decode_envelope(body)?;
    ensure_result_code(&envelope.rt_cd, &envelope.msg_cd, &envelope.msg1)?;
    let output = envelope
        .output
        .ok_or_else(|| BrokerError::invariant("KIS quote response is missing output"))?;

    Ok(Quote {
        symbol: Symbol::new(required_string(
            output.stck_shrn_iscd.as_deref(),
            "stck_shrn_iscd",
        )?)?,
        full_name: None,
        last_price: decimal_from_kis_string(output.stck_prpr.as_deref(), "stck_prpr")?,
    })
}

pub fn map_stock_full_name_response(body: &Value) -> Result<Option<String>, BrokerError> {
    let envelope: KisApiEnvelope<KisSearchStockInfoOutput> = decode_envelope(body)?;
    ensure_result_code(&envelope.rt_cd, &envelope.msg_cd, &envelope.msg1)?;
    let output = envelope
        .output
        .ok_or_else(|| BrokerError::invariant("KIS stock info response is missing output"))?;

    Ok(first_non_empty([
        output.prdt_name120.as_deref(),
        output.prdt_name.as_deref(),
        output.prdt_abrv_name.as_deref(),
    ]))
}

fn first_non_empty(values: [Option<&str>; 3]) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

pub fn map_market_session_response(body: &Value) -> Result<MarketSession, BrokerError> {
    let response: KisHolidayResponse = decode_body(body)?;
    ensure_result_code(&response.rt_cd, &response.msg_cd, &response.msg1)?;
    let output = response
        .output
        .first()
        .ok_or_else(|| BrokerError::invariant("KIS holiday response is missing output"))?;
    let trading_day = required_string(output.bass_dt.as_deref(), "bass_dt")?.to_owned();
    let opnd_yn = required_string(output.opnd_yn.as_deref(), "opnd_yn")?;

    Ok(MarketSession {
        trading_day,
        is_open: opnd_yn == "Y",
    })
}
