use serde::Deserialize;

use crate::BrokerEnvironment;
use crate::core::BrokerError;
use crate::transport::{EndpointGroup, KisEndpoint, KisHttpMethod};

pub const DOMESTIC_STOCK_QUOTE_ENDPOINT: KisEndpoint = KisEndpoint {
    name: "domestic_stock_quote",
    method: KisHttpMethod::Get,
    path: "/uapi/domestic-stock/v1/quotations/inquire-price",
    tr_id: "FHKST01010100",
    group: EndpointGroup::ReadLight,
    requires_hashkey: false,
};

pub const DOMESTIC_STOCK_HOLIDAY_ENDPOINT: KisEndpoint = KisEndpoint {
    name: "domestic_stock_holiday",
    method: KisHttpMethod::Get,
    path: "/uapi/domestic-stock/v1/quotations/chk-holiday",
    tr_id: "CTCA0903R",
    group: EndpointGroup::ReadHeavy,
    requires_hashkey: false,
};

pub fn domestic_stock_search_stock_info_endpoint(
    environment: BrokerEnvironment,
) -> Result<KisEndpoint, BrokerError> {
    let tr_id = match environment {
        BrokerEnvironment::Live => "CTPF1002R",
        BrokerEnvironment::Paper => "VTPF1002R",
        BrokerEnvironment::Mock => {
            return Err(BrokerError::validation(
                "mock environment does not use KIS stock info endpoint",
            ));
        }
    };

    Ok(KisEndpoint {
        name: "domestic_stock_search_stock_info",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/quotations/search-stock-info",
        tr_id,
        group: EndpointGroup::ReadLight,
        requires_hashkey: false,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisQuoteOutput {
    pub stck_shrn_iscd: Option<String>,
    pub stck_prpr: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisSearchStockInfoOutput {
    pub prdt_name: Option<String>,
    pub prdt_name120: Option<String>,
    pub prdt_abrv_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisHolidayResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    #[serde(default)]
    pub output: Vec<KisHolidayOutput>,
    pub ctx_area_fk: Option<String>,
    pub ctx_area_nk: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisHolidayOutput {
    pub bass_dt: Option<String>,
    pub opnd_yn: Option<String>,
}
