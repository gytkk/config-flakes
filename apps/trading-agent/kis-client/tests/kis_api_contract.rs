#![cfg(feature = "internal-test-support")]

#[path = "support/api_spec.rs"]
mod api_spec;

use api_spec::{
    ApiSpecCase, COMMON_REST_HEADERS, HashkeyPolicy, OFFICIAL_OPEN_TRADING_API_COMMIT,
    assert_environment, paper_tr_id_from_official_wrapper,
};
use trading_agent_kis_client::domain::{BrokerEnvironment, OrderSide};
use trading_agent_kis_client::internal::protocol::{
    DOMESTIC_STOCK_HOLIDAY_ENDPOINT, DOMESTIC_STOCK_QUOTE_ENDPOINT, KisDailyCcldPeriod,
    OVERSEAS_STOCK_PRICE_ENDPOINT, domestic_stock_balance_endpoint,
    domestic_stock_buying_power_endpoint, domestic_stock_cash_order_endpoint,
    domestic_stock_daily_ccld_endpoint, domestic_stock_revise_cancel_possible_endpoint,
    domestic_stock_rvsecncl_order_endpoint, domestic_stock_search_stock_info_endpoint,
    overseas_stock_balance_endpoint, overseas_stock_buying_power_endpoint,
    overseas_stock_open_orders_endpoint, overseas_stock_order_endpoint,
    overseas_stock_order_history_endpoint, overseas_stock_rvsecncl_order_endpoint,
};
use trading_agent_kis_client::transport::{EndpointGroup, KisHttpMethod};

#[test]
fn implemented_endpoints_have_official_sample_backed_contract_metadata() {
    for case in endpoint_cases() {
        case.assert_lists_are_defined();
    }
}

#[test]
fn read_endpoint_contracts_match_official_kis_samples() {
    quote_case().assert_endpoint(DOMESTIC_STOCK_QUOTE_ENDPOINT, DOMESTIC_STOCK_QUOTE_ENDPOINT);
    search_stock_info_case().assert_endpoint(
        domestic_stock_search_stock_info_endpoint(BrokerEnvironment::Paper)
            .expect("paper search stock info"),
        domestic_stock_search_stock_info_endpoint(BrokerEnvironment::Live)
            .expect("live search stock info"),
    );
    holiday_case().assert_endpoint(
        DOMESTIC_STOCK_HOLIDAY_ENDPOINT,
        DOMESTIC_STOCK_HOLIDAY_ENDPOINT,
    );

    balance_case().assert_endpoint(
        domestic_stock_balance_endpoint(BrokerEnvironment::Paper).expect("paper balance"),
        domestic_stock_balance_endpoint(BrokerEnvironment::Live).expect("live balance"),
    );
    buying_power_case().assert_endpoint(
        domestic_stock_buying_power_endpoint(BrokerEnvironment::Paper).expect("paper buying power"),
        domestic_stock_buying_power_endpoint(BrokerEnvironment::Live).expect("live buying power"),
    );
    daily_ccld_case().assert_endpoint(
        domestic_stock_daily_ccld_endpoint(
            BrokerEnvironment::Paper,
            KisDailyCcldPeriod::WithinThreeMonths,
        )
        .expect("paper daily ccld"),
        domestic_stock_daily_ccld_endpoint(
            BrokerEnvironment::Live,
            KisDailyCcldPeriod::WithinThreeMonths,
        )
        .expect("live daily ccld"),
    );
    revise_cancel_possible_case().assert_endpoint(
        domestic_stock_revise_cancel_possible_endpoint(BrokerEnvironment::Paper)
            .expect("paper possible"),
        domestic_stock_revise_cancel_possible_endpoint(BrokerEnvironment::Live)
            .expect("live possible"),
    );
    overseas_quote_case()
        .assert_endpoint(OVERSEAS_STOCK_PRICE_ENDPOINT, OVERSEAS_STOCK_PRICE_ENDPOINT);
    overseas_balance_case().assert_endpoint(
        overseas_stock_balance_endpoint(BrokerEnvironment::Paper).expect("paper overseas balance"),
        overseas_stock_balance_endpoint(BrokerEnvironment::Live).expect("live overseas balance"),
    );
    overseas_buying_power_case().assert_endpoint(
        overseas_stock_buying_power_endpoint(BrokerEnvironment::Paper)
            .expect("paper overseas buying power"),
        overseas_stock_buying_power_endpoint(BrokerEnvironment::Live)
            .expect("live overseas buying power"),
    );
    overseas_open_orders_case().assert_endpoint(
        overseas_stock_open_orders_endpoint(BrokerEnvironment::Paper)
            .expect("paper overseas open orders"),
        overseas_stock_open_orders_endpoint(BrokerEnvironment::Live)
            .expect("live overseas open orders"),
    );
    overseas_order_history_case().assert_endpoint(
        overseas_stock_order_history_endpoint(BrokerEnvironment::Paper)
            .expect("paper overseas order history"),
        overseas_stock_order_history_endpoint(BrokerEnvironment::Live)
            .expect("live overseas order history"),
    );
}

#[test]
fn write_endpoint_contracts_match_official_kis_samples() {
    cash_order_case().assert_endpoint(
        domestic_stock_cash_order_endpoint(BrokerEnvironment::Paper, OrderSide::Buy)
            .expect("paper buy cash order"),
        domestic_stock_cash_order_endpoint(BrokerEnvironment::Live, OrderSide::Buy)
            .expect("live buy cash order"),
    );
    rvsecncl_order_case().assert_endpoint(
        domestic_stock_rvsecncl_order_endpoint(BrokerEnvironment::Paper).expect("paper rvsecncl"),
        domestic_stock_rvsecncl_order_endpoint(BrokerEnvironment::Live).expect("live rvsecncl"),
    );
    overseas_order_case().assert_endpoint(
        overseas_stock_order_endpoint(
            BrokerEnvironment::Paper,
            OrderSide::Buy,
            &trading_agent_kis_client::domain::ExchangeCode::new("NASD").expect("exchange"),
        )
        .expect("paper overseas order"),
        overseas_stock_order_endpoint(
            BrokerEnvironment::Live,
            OrderSide::Buy,
            &trading_agent_kis_client::domain::ExchangeCode::new("NASD").expect("exchange"),
        )
        .expect("live overseas order"),
    );
    overseas_rvsecncl_order_case().assert_endpoint(
        overseas_stock_rvsecncl_order_endpoint(BrokerEnvironment::Paper)
            .expect("paper overseas rvsecncl"),
        overseas_stock_rvsecncl_order_endpoint(BrokerEnvironment::Live)
            .expect("live overseas rvsecncl"),
    );
}

#[test]
fn paper_tr_id_transform_matches_official_common_wrapper_rule() {
    assert_eq!(paper_tr_id_from_official_wrapper("TTTC0084R"), "VTTC0084R");
    assert_eq!(paper_tr_id_from_official_wrapper("CTCA0903R"), "VTCA0903R");
    assert_eq!(
        paper_tr_id_from_official_wrapper("FHKST01010100"),
        "FHKST01010100"
    );

    assert_environment(
        domestic_stock_revise_cancel_possible_endpoint(BrokerEnvironment::Paper)
            .expect("paper endpoint"),
        BrokerEnvironment::Paper,
    );
}

fn endpoint_cases() -> Vec<ApiSpecCase> {
    vec![
        quote_case(),
        search_stock_info_case(),
        holiday_case(),
        balance_case(),
        buying_power_case(),
        cash_order_case(),
        rvsecncl_order_case(),
        revise_cancel_possible_case(),
        daily_ccld_case(),
        overseas_quote_case(),
        overseas_balance_case(),
        overseas_buying_power_case(),
        overseas_order_case(),
        overseas_rvsecncl_order_case(),
        overseas_open_orders_case(),
        overseas_order_history_case(),
    ]
}

fn quote_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/inquire_price/inquire_price.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_quote",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/quotations/inquire-price",
        live_tr_id: "FHKST01010100",
        paper_tr_id: "FHKST01010100",
        group: EndpointGroup::ReadLight,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &["FID_COND_MRKT_DIV_CODE", "FID_INPUT_ISCD"],
        required_body_fields: &[],
        pagination_token_fields: &[],
        success_fixture: Some("market/quote_success.json"),
        error_fixture: Some("market/quote_error.json"),
    }
}

fn search_stock_info_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/search_stock_info/search_stock_info.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_search_stock_info",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/quotations/search-stock-info",
        live_tr_id: "CTPF1002R",
        paper_tr_id: "VTPF1002R",
        group: EndpointGroup::ReadLight,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &["PRDT_TYPE_CD", "PDNO"],
        required_body_fields: &[],
        pagination_token_fields: &[],
        success_fixture: Some("market/search_stock_info_success.json"),
        error_fixture: None,
    }
}

fn holiday_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/chk_holiday/chk_holiday.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_holiday",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/quotations/chk-holiday",
        live_tr_id: "CTCA0903R",
        paper_tr_id: "CTCA0903R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &["BASS_DT", "CTX_AREA_FK", "CTX_AREA_NK"],
        required_body_fields: &[],
        pagination_token_fields: &["CTX_AREA_FK", "CTX_AREA_NK"],
        success_fixture: Some("market/holiday_open.json"),
        error_fixture: None,
    }
}

fn balance_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/inquire_balance/inquire_balance.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_balance",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-balance",
        live_tr_id: "TTTC8434R",
        paper_tr_id: "VTTC8434R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "AFHR_FLPR_YN",
            "OFL_YN",
            "INQR_DVSN",
            "UNPR_DVSN",
            "FUND_STTL_ICLD_YN",
            "FNCG_AMT_AUTO_RDPT_YN",
            "PRCS_DVSN",
            "CTX_AREA_FK100",
            "CTX_AREA_NK100",
        ],
        required_body_fields: &[],
        pagination_token_fields: &["CTX_AREA_FK100", "CTX_AREA_NK100"],
        success_fixture: Some("account/balance_success.json"),
        error_fixture: None,
    }
}

fn buying_power_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/inquire_psbl_order/inquire_psbl_order.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_buying_power",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-psbl-order",
        live_tr_id: "TTTC8908R",
        paper_tr_id: "VTTC8908R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "PDNO",
            "ORD_UNPR",
            "ORD_DVSN",
            "CMA_EVLU_AMT_ICLD_YN",
            "OVRS_ICLD_YN",
        ],
        required_body_fields: &[],
        pagination_token_fields: &[],
        success_fixture: Some("account/buying_power_success.json"),
        error_fixture: None,
    }
}

fn cash_order_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/order_cash/order_cash.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_cash_order",
        method: KisHttpMethod::Post,
        path: "/uapi/domestic-stock/v1/trading/order-cash",
        live_tr_id: "TTTC0012U",
        paper_tr_id: "VTTC0012U",
        group: EndpointGroup::WriteOrder,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[],
        required_body_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "PDNO",
            "ORD_DVSN",
            "ORD_QTY",
            "ORD_UNPR",
            "EXCG_ID_DVSN_CD",
            "SLL_TYPE",
            "CNDT_PRIC",
        ],
        pagination_token_fields: &[],
        success_fixture: Some("orders/order_cash_success.json"),
        error_fixture: Some("orders/order_rejected.json"),
    }
}

fn rvsecncl_order_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/order_rvsecncl/order_rvsecncl.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_rvsecncl_order",
        method: KisHttpMethod::Post,
        path: "/uapi/domestic-stock/v1/trading/order-rvsecncl",
        live_tr_id: "TTTC0013U",
        paper_tr_id: "VTTC0013U",
        group: EndpointGroup::WriteOrder,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[],
        required_body_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "KRX_FWDG_ORD_ORGNO",
            "ORGN_ODNO",
            "ORD_DVSN",
            "RVSE_CNCL_DVSN_CD",
            "ORD_QTY",
            "ORD_UNPR",
            "QTY_ALL_ORD_YN",
            "EXCG_ID_DVSN_CD",
            "CNDT_PRIC",
        ],
        pagination_token_fields: &[],
        success_fixture: Some("orders/order_rvsecncl_success.json"),
        error_fixture: Some("orders/order_rejected.json"),
    }
}

fn revise_cancel_possible_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/inquire_psbl_rvsecncl/inquire_psbl_rvsecncl.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_revise_cancel_possible",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl",
        live_tr_id: "TTTC0084R",
        paper_tr_id: "VTTC0084R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "INQR_DVSN_1",
            "INQR_DVSN_2",
            "CTX_AREA_FK100",
            "CTX_AREA_NK100",
        ],
        required_body_fields: &[],
        pagination_token_fields: &["CTX_AREA_FK100", "CTX_AREA_NK100"],
        success_fixture: Some("orders/psbl_rvsecncl_success.json"),
        error_fixture: None,
    }
}

fn daily_ccld_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/domestic_stock/inquire_daily_ccld/inquire_daily_ccld.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "domestic_stock_daily_ccld",
        method: KisHttpMethod::Get,
        path: "/uapi/domestic-stock/v1/trading/inquire-daily-ccld",
        live_tr_id: "TTTC0081R",
        paper_tr_id: "VTTC0081R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "INQR_STRT_DT",
            "INQR_END_DT",
            "SLL_BUY_DVSN_CD",
            "PDNO",
            "CCLD_DVSN",
            "INQR_DVSN",
            "INQR_DVSN_3",
            "ORD_GNO_BRNO",
            "ODNO",
            "INQR_DVSN_1",
            "CTX_AREA_FK100",
            "CTX_AREA_NK100",
            "EXCG_ID_DVSN_CD",
        ],
        required_body_fields: &[],
        pagination_token_fields: &["CTX_AREA_FK100", "CTX_AREA_NK100"],
        success_fixture: Some("orders/daily_ccld_success.json"),
        error_fixture: None,
    }
}

fn overseas_quote_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/overseas_stock/price/price.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "overseas_stock_price",
        method: KisHttpMethod::Get,
        path: "/uapi/overseas-price/v1/quotations/price",
        live_tr_id: "HHDFS00000300",
        paper_tr_id: "HHDFS00000300",
        group: EndpointGroup::ReadLight,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &["AUTH", "EXCD", "SYMB"],
        required_body_fields: &[],
        pagination_token_fields: &[],
        success_fixture: Some("overseas/quote_success.json"),
        error_fixture: Some("overseas/quote_error.json"),
    }
}

fn overseas_balance_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/overseas_stock/inquire_balance/inquire_balance.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "overseas_stock_balance",
        method: KisHttpMethod::Get,
        path: "/uapi/overseas-stock/v1/trading/inquire-balance",
        live_tr_id: "TTTS3012R",
        paper_tr_id: "VTTS3012R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "OVRS_EXCG_CD",
            "TR_CRCY_CD",
            "CTX_AREA_FK200",
            "CTX_AREA_NK200",
        ],
        required_body_fields: &[],
        pagination_token_fields: &["CTX_AREA_FK200", "CTX_AREA_NK200"],
        success_fixture: Some("overseas/balance_success.json"),
        error_fixture: None,
    }
}

fn overseas_buying_power_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/overseas_stock/inquire_psamount/inquire_psamount.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "overseas_stock_buying_power",
        method: KisHttpMethod::Get,
        path: "/uapi/overseas-stock/v1/trading/inquire-psamount",
        live_tr_id: "TTTS3007R",
        paper_tr_id: "VTTS3007R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "OVRS_EXCG_CD",
            "OVRS_ORD_UNPR",
            "ITEM_CD",
        ],
        required_body_fields: &[],
        pagination_token_fields: &[],
        success_fixture: Some("overseas/buying_power_success.json"),
        error_fixture: None,
    }
}

fn overseas_order_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/overseas_stock/order/order.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "overseas_stock_order",
        method: KisHttpMethod::Post,
        path: "/uapi/overseas-stock/v1/trading/order",
        live_tr_id: "TTTT1002U",
        paper_tr_id: "VTTT1002U",
        group: EndpointGroup::WriteOrder,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[],
        required_body_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "OVRS_EXCG_CD",
            "PDNO",
            "ORD_QTY",
            "OVRS_ORD_UNPR",
            "CTAC_TLNO",
            "MGCO_APTM_ODNO",
            "SLL_TYPE",
            "ORD_SVR_DVSN_CD",
            "ORD_DVSN",
        ],
        pagination_token_fields: &[],
        success_fixture: Some("overseas/order_success.json"),
        error_fixture: Some("overseas/order_rejected.json"),
    }
}

fn overseas_rvsecncl_order_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/overseas_stock/order_rvsecncl/order_rvsecncl.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "overseas_stock_rvsecncl_order",
        method: KisHttpMethod::Post,
        path: "/uapi/overseas-stock/v1/trading/order-rvsecncl",
        live_tr_id: "TTTT1004U",
        paper_tr_id: "VTTT1004U",
        group: EndpointGroup::WriteOrder,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[],
        required_body_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "OVRS_EXCG_CD",
            "PDNO",
            "ORGN_ODNO",
            "RVSE_CNCL_DVSN_CD",
            "ORD_QTY",
            "OVRS_ORD_UNPR",
            "MGCO_APTM_ODNO",
            "ORD_SVR_DVSN_CD",
        ],
        pagination_token_fields: &[],
        success_fixture: Some("overseas/order_rvsecncl_success.json"),
        error_fixture: Some("overseas/order_rejected.json"),
    }
}

fn overseas_open_orders_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/overseas_stock/inquire_nccs/inquire_nccs.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "overseas_stock_open_orders",
        method: KisHttpMethod::Get,
        path: "/uapi/overseas-stock/v1/trading/inquire-nccs",
        live_tr_id: "TTTS3018R",
        paper_tr_id: "TTTS3018R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "OVRS_EXCG_CD",
            "SORT_SQN",
            "CTX_AREA_FK200",
            "CTX_AREA_NK200",
        ],
        required_body_fields: &[],
        pagination_token_fields: &["CTX_AREA_FK200", "CTX_AREA_NK200"],
        success_fixture: Some("overseas/nccs_success.json"),
        error_fixture: None,
    }
}

fn overseas_order_history_case() -> ApiSpecCase {
    ApiSpecCase {
        official_source_path: "examples_llm/overseas_stock/inquire_ccnl/inquire_ccnl.py",
        checked_commit: OFFICIAL_OPEN_TRADING_API_COMMIT,
        endpoint_name: "overseas_stock_order_history",
        method: KisHttpMethod::Get,
        path: "/uapi/overseas-stock/v1/trading/inquire-ccnl",
        live_tr_id: "TTTS3035R",
        paper_tr_id: "VTTS3035R",
        group: EndpointGroup::ReadHeavy,
        hashkey_policy: HashkeyPolicy::DisabledByOfficialSample,
        required_headers: COMMON_REST_HEADERS,
        required_query_fields: &[
            "CANO",
            "ACNT_PRDT_CD",
            "PDNO",
            "ORD_STRT_DT",
            "ORD_END_DT",
            "SLL_BUY_DVSN",
            "CCLD_NCCS_DVSN",
            "OVRS_EXCG_CD",
            "SORT_SQN",
            "ORD_DT",
            "ORD_GNO_BRNO",
            "ODNO",
            "CTX_AREA_NK200",
            "CTX_AREA_FK200",
        ],
        required_body_fields: &[],
        pagination_token_fields: &["CTX_AREA_FK200", "CTX_AREA_NK200"],
        success_fixture: Some("overseas/ccnl_success.json"),
        error_fixture: None,
    }
}
