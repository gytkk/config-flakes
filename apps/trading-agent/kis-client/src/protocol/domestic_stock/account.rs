use serde::Deserialize;

use crate::BrokerEnvironment;
use crate::core::BrokerError;
use crate::transport::{EndpointGroup, KisEndpoint, KisHttpMethod};

pub fn domestic_stock_balance_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Paper => "VTTC8434R",
        BrokerEnvironment::Live => "TTTC8434R",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS balance endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "domestic_stock_balance",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-balance",
        tr_id,
        group: EndpointGroup::ReadHeavy,
        requires_hashkey: false,
    })
}

pub fn domestic_stock_buying_power_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Paper => "VTTC8908R",
        BrokerEnvironment::Live => "TTTC8908R",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS buying power endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "domestic_stock_buying_power",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-psbl-order",
        tr_id,
        group: EndpointGroup::ReadHeavy,
        requires_hashkey: false,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisBalanceResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    #[serde(default)]
    pub output1: Vec<KisBalancePosition>,
    #[serde(default)]
    pub output2: Vec<KisBalanceSummary>,
    pub ctx_area_fk100: Option<String>,
    pub ctx_area_nk100: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisBalancePosition {
    pub pdno: Option<String>,
    pub hldg_qty: Option<String>,
    pub pchs_avg_pric: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisBalanceSummary {
    pub dnca_tot_amt: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisBuyingPowerOutput {
    pub nrcvb_buy_amt: Option<String>,
    pub max_buy_amt: Option<String>,
    pub ord_psbl_cash: Option<String>,
}
