use serde::Deserialize;

use crate::transport::{EndpointGroup, KisEndpoint, KisHttpMethod};

pub const OVERSEAS_STOCK_PRICE_ENDPOINT: KisEndpoint = KisEndpoint {
    name: "overseas_stock_price",
    method: KisHttpMethod::Get,
    path: "/uapi/overseas-price/v1/quotations/price",
    tr_id: "HHDFS00000300",
    group: EndpointGroup::ReadLight,
    requires_hashkey: false,
};

pub const OVERSEAS_STOCK_COUNTRIES_HOLIDAY_ENDPOINT: KisEndpoint = KisEndpoint {
    name: "overseas_stock_countries_holiday",
    method: KisHttpMethod::Get,
    path: "/uapi/overseas-stock/v1/quotations/countries-holiday",
    tr_id: "CTOS5011R",
    group: EndpointGroup::ReadHeavy,
    requires_hashkey: false,
};

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasStockQuoteOutput {
    pub last: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasCountriesHolidayResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    #[serde(default)]
    pub output: Vec<KisOverseasCountriesHolidayOutput>,
    pub ctx_area_fk: Option<String>,
    pub ctx_area_nk: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KisOverseasCountriesHolidayOutput {
    pub tr_mket_cd: Option<String>,
    pub acpl_sttl_dt: Option<String>,
    pub dmst_sttl_dt: Option<String>,
}
