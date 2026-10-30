use serde::Deserialize;

use crate::core::BrokerError;
use crate::transport::{EndpointGroup, KisEndpoint, KisHttpMethod};
use crate::{BrokerEnvironment, OrderSide};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KisDailyCcldPeriod {
    WithinThreeMonths,
    BeforeThreeMonths,
}

pub fn domestic_stock_daily_ccld_endpoint(
    environment: BrokerEnvironment,
    period: KisDailyCcldPeriod,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match (environment, period) {
        (BrokerEnvironment::Live, KisDailyCcldPeriod::WithinThreeMonths) => "TTTC0081R",
        (BrokerEnvironment::Live, KisDailyCcldPeriod::BeforeThreeMonths) => "CTSC9215R",
        (BrokerEnvironment::Paper, KisDailyCcldPeriod::WithinThreeMonths) => "VTTC0081R",
        (BrokerEnvironment::Paper, KisDailyCcldPeriod::BeforeThreeMonths) => "VTSC9215R",
        (BrokerEnvironment::Mock, _) => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS daily order endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "domestic_stock_daily_ccld",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-daily-ccld",
        tr_id,
        group: EndpointGroup::ReadHeavy,
        requires_hashkey: false,
    })
}

pub fn domestic_stock_revise_cancel_possible_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Live => "TTTC0084R",
        BrokerEnvironment::Paper => "VTTC0084R",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS revise/cancel possible endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "domestic_stock_revise_cancel_possible",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl",
        tr_id,
        group: EndpointGroup::ReadHeavy,
        requires_hashkey: false,
    })
}

pub fn domestic_stock_cash_order_endpoint(
    environment: BrokerEnvironment,
    side: OrderSide,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match (environment, side) {
        (BrokerEnvironment::Live, OrderSide::Sell) => "TTTC0011U",
        (BrokerEnvironment::Live, OrderSide::Buy) => "TTTC0012U",
        (BrokerEnvironment::Paper, OrderSide::Sell) => "VTTC0011U",
        (BrokerEnvironment::Paper, OrderSide::Buy) => "VTTC0012U",
        (BrokerEnvironment::Mock, _) => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS cash order endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "domestic_stock_cash_order",
        method: KisHttpMethod::Post,
        path: "/uapi/domestic-stock/v1/trading/order-cash",
        tr_id,
        group: EndpointGroup::WriteOrder,
        requires_hashkey: false,
    })
}

pub fn domestic_stock_rvsecncl_order_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Live => "TTTC0013U",
        BrokerEnvironment::Paper => "VTTC0013U",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS revise/cancel endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "domestic_stock_rvsecncl_order",
        method: KisHttpMethod::Post,
        path: "/uapi/domestic-stock/v1/trading/order-rvsecncl",
        tr_id,
        group: EndpointGroup::WriteOrder,
        requires_hashkey: false,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisDailyCcldResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    #[serde(default)]
    pub output1: Vec<KisDailyOrderOutput>,
    pub ctx_area_fk100: Option<String>,
    pub ctx_area_nk100: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisDailyOrderOutput {
    pub ord_dt: Option<String>,
    pub pdno: Option<String>,
    pub odno: Option<String>,
    pub ord_gno_brno: Option<String>,
    pub excg_id_dvsn_cd: Option<String>,
    pub excg_dvsn_cd: Option<String>,
    pub sll_buy_dvsn_cd: Option<String>,
    pub ord_qty: Option<String>,
    pub ord_unpr: Option<String>,
    pub ord_tmd: Option<String>,
    pub tot_ccld_qty: Option<String>,
    pub avg_prvs: Option<String>,
    pub cncl_yn: Option<String>,
    pub rjct_qty: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisReviseCancelPossibleResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    #[serde(default)]
    pub output: Vec<KisReviseCancelPossibleOutput>,
    pub ctx_area_fk100: Option<String>,
    pub ctx_area_nk100: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisReviseCancelPossibleOutput {
    pub ord_gno_brno: Option<String>,
    pub odno: Option<String>,
    pub pdno: Option<String>,
    pub ord_qty: Option<String>,
    pub ord_unpr: Option<String>,
    pub tot_ccld_qty: Option<String>,
    pub psbl_qty: Option<String>,
    pub sll_buy_dvsn_cd: Option<String>,
    pub excg_dvsn_cd: Option<String>,
    pub excg_id_dvsn_cd: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOrderResponseOutput {
    #[serde(rename = "ODNO")]
    pub odno: Option<String>,
    #[serde(rename = "KRX_FWDG_ORD_ORGNO")]
    pub krx_fwdg_ord_orgno: Option<String>,
}
