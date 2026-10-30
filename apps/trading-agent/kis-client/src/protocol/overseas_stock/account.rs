use serde::Deserialize;
use serde_json::Value;

use crate::BrokerEnvironment;
use crate::core::BrokerError;
use crate::transport::{EndpointGroup, KisEndpoint, KisHttpMethod};

pub fn overseas_stock_balance_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Paper => "VTTS3012R",
        BrokerEnvironment::Live => "TTTS3012R",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS overseas balance endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "overseas_stock_balance",
        method: KisHttpMethod::Get,
        path: "/uapi/overseas-stock/v1/trading/inquire-balance",
        tr_id,
        group: EndpointGroup::ReadHeavy,
        requires_hashkey: false,
    })
}

pub fn overseas_stock_buying_power_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Paper => "VTTS3007R",
        BrokerEnvironment::Live => "TTTS3007R",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS overseas buying power endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "overseas_stock_buying_power",
        method: KisHttpMethod::Get,
        path: "/uapi/overseas-stock/v1/trading/inquire-psamount",
        tr_id,
        group: EndpointGroup::ReadHeavy,
        requires_hashkey: false,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasStockBalanceResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    #[serde(default)]
    pub output1: Vec<KisOverseasStockBalancePosition>,
    pub output2: Option<Value>,
    pub ctx_area_fk200: Option<String>,
    pub ctx_area_nk200: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasStockBalancePosition {
    pub ovrs_pdno: Option<String>,
    pub ovrs_excg_cd: Option<String>,
    pub tr_crcy_cd: Option<String>,
    pub ovrs_cblc_qty: Option<String>,
    pub pchs_avg_pric: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasStockBalanceSummary {
    pub frcr_pchs_amt1: Option<String>,
    pub ovrs_stck_evlu_amt: Option<String>,
    pub ovrs_tot_pfls: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasStockBuyingPowerOutput {
    pub tr_crcy_cd: Option<String>,
    pub ovrs_ord_psbl_amt: Option<String>,
    pub ord_psbl_frcr_amt: Option<String>,
    pub frcr_ord_psbl_amt1: Option<String>,
    pub max_ord_psbl_qty: Option<String>,
    pub ovrs_max_ord_psbl_qty: Option<String>,
    pub ord_psbl_qty: Option<String>,
}
