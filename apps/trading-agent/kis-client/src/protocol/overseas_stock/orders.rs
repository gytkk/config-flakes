use serde::Deserialize;

use crate::core::BrokerError;
use crate::transport::{EndpointGroup, KisEndpoint, KisHttpMethod};
use crate::{BrokerEnvironment, ExchangeCode, OrderSide};

pub fn overseas_stock_order_endpoint(
    environment: BrokerEnvironment,
    side: OrderSide,
    exchange: &ExchangeCode,
) -> Result<KisEndpoint, BrokerError> {
    let live_tr_id = match (side, exchange.as_str()) {
        (OrderSide::Buy, "NASD" | "NYSE" | "AMEX") => "TTTT1002U",
        (OrderSide::Buy, "SEHK") => "TTTS1002U",
        (OrderSide::Buy, "SHAA") => "TTTS0202U",
        (OrderSide::Buy, "SZAA") => "TTTS0305U",
        (OrderSide::Buy, "TKSE") => "TTTS0308U",
        (OrderSide::Buy, "HASE" | "VNSE") => "TTTS0311U",
        (OrderSide::Sell, "NASD" | "NYSE" | "AMEX") => "TTTT1006U",
        (OrderSide::Sell, "SEHK") => "TTTS1001U",
        (OrderSide::Sell, "SHAA") => "TTTS1005U",
        (OrderSide::Sell, "SZAA") => "TTTS0304U",
        (OrderSide::Sell, "TKSE") => "TTTS0307U",
        (OrderSide::Sell, "HASE" | "VNSE") => "TTTS0310U",
        (_, other) => {
            return Err(BrokerError::validation(format!(
                "unsupported KIS overseas stock order exchange: {other}"
            )));
        }
    };
    let tr_id = match environment {
        BrokerEnvironment::Live => live_tr_id,
        BrokerEnvironment::Paper => paper_tr_id(live_tr_id),
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS overseas stock order endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "overseas_stock_order",
        method: KisHttpMethod::Post,
        path: "/uapi/overseas-stock/v1/trading/order",
        tr_id,
        group: EndpointGroup::WriteOrder,
        requires_hashkey: false,
    })
}

pub fn overseas_stock_daytime_order_endpoint(
    environment: BrokerEnvironment,
    side: OrderSide,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match (environment, side) {
        (BrokerEnvironment::Live, OrderSide::Buy) => "TTTS6036U",
        (BrokerEnvironment::Live, OrderSide::Sell) => "TTTS6037U",
        (BrokerEnvironment::Paper, _) => {
            return Err(BrokerError::validation(
                "KIS overseas US daytime orders are not supported in paper environment",
            ));
        }
        (BrokerEnvironment::Mock, _) => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS overseas US daytime order endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "overseas_stock_daytime_order",
        method: KisHttpMethod::Post,
        path: "/uapi/overseas-stock/v1/trading/daytime-order",
        tr_id,
        group: EndpointGroup::WriteOrder,
        requires_hashkey: false,
    })
}

pub fn overseas_stock_rvsecncl_order_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Live => "TTTT1004U",
        BrokerEnvironment::Paper => "VTTT1004U",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS overseas stock revise/cancel endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "overseas_stock_rvsecncl_order",
        method: KisHttpMethod::Post,
        path: "/uapi/overseas-stock/v1/trading/order-rvsecncl",
        tr_id,
        group: EndpointGroup::WriteOrder,
        requires_hashkey: false,
    })
}

pub fn overseas_stock_open_orders_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    match environment {
        BrokerEnvironment::Live | BrokerEnvironment::Paper => Ok(KisEndpoint {
            name: "overseas_stock_open_orders",
            method: KisHttpMethod::Get,
            path: "/uapi/overseas-stock/v1/trading/inquire-nccs",
            tr_id: "TTTS3018R",
            group: EndpointGroup::ReadHeavy,
            requires_hashkey: false,
        }),
        BrokerEnvironment::Mock => Err(BrokerError::validation(
            "mock environment does not use KIS overseas stock open orders endpoint",
        )),
    }
}

pub fn overseas_stock_order_history_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Live => "TTTS3035R",
        BrokerEnvironment::Paper => "VTTS3035R",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS overseas stock order history endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "overseas_stock_order_history",
        method: KisHttpMethod::Get,
        path: "/uapi/overseas-stock/v1/trading/inquire-ccnl",
        tr_id,
        group: EndpointGroup::ReadHeavy,
        requires_hashkey: false,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasOrderResponseOutput {
    #[serde(rename = "ODNO", alias = "odno")]
    pub odno: Option<String>,
    #[serde(rename = "KRX_FWDG_ORD_ORGNO", alias = "krx_fwdg_ord_orgno")]
    pub krx_fwdg_ord_orgno: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasOrderListResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    #[serde(default)]
    pub output: Vec<KisOverseasOrderOutput>,
    pub ctx_area_fk200: Option<String>,
    pub ctx_area_nk200: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasOrderOutput {
    #[serde(alias = "ORD_DT")]
    pub ord_dt: Option<String>,
    #[serde(alias = "ORD_TMD")]
    pub ord_tmd: Option<String>,
    #[serde(alias = "ORD_GNO_BRNO", alias = "KRX_FWDG_ORD_ORGNO")]
    pub ord_gno_brno: Option<String>,
    #[serde(alias = "ODNO")]
    pub odno: Option<String>,
    #[serde(alias = "OVRS_EXCG_CD")]
    pub ovrs_excg_cd: Option<String>,
    #[serde(alias = "PDNO")]
    pub pdno: Option<String>,
    #[serde(alias = "SLL_BUY_DVSN_CD")]
    pub sll_buy_dvsn_cd: Option<String>,
    #[serde(alias = "ORD_QTY", alias = "FT_ORD_QTY")]
    pub ft_ord_qty: Option<String>,
    #[serde(alias = "ORD_UNPR", alias = "OVRS_ORD_UNPR", alias = "FT_ORD_UNPR3")]
    pub ft_ord_unpr3: Option<String>,
    #[serde(alias = "TOT_CCLD_QTY", alias = "FT_CCLD_QTY", alias = "CCLD_QTY")]
    pub ft_ccld_qty: Option<String>,
    #[serde(alias = "AVG_PRVS", alias = "FT_CCLD_UNPR3", alias = "AVG_CCLD_UNPR3")]
    pub ft_ccld_unpr3: Option<String>,
    #[serde(alias = "NCCS_QTY")]
    pub nccs_qty: Option<String>,
    #[serde(alias = "RJCT_QTY")]
    pub rjct_qty: Option<String>,
    #[serde(alias = "CNCL_YN")]
    pub cncl_yn: Option<String>,
}

fn paper_tr_id(live_tr_id: &'static str) -> &'static str {
    match live_tr_id {
        "TTTT1002U" => "VTTT1002U",
        "TTTT1006U" => "VTTT1001U",
        "TTTS1002U" => "VTTS1002U",
        "TTTS0202U" => "VTTS0202U",
        "TTTS0305U" => "VTTS0305U",
        "TTTS0308U" => "VTTS0308U",
        "TTTS0311U" => "VTTS0311U",
        "TTTS1001U" => "VTTS1001U",
        "TTTS1005U" => "VTTS1005U",
        "TTTS0304U" => "VTTS0304U",
        "TTTS0307U" => "VTTS0307U",
        "TTTS0310U" => "VTTS0310U",
        _ => live_tr_id,
    }
}
