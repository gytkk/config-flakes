use std::collections::BTreeSet;

use crate::core::BrokerError;
use crate::mapping::{
    BalancePage, map_balance_page_response, map_buying_power_response, map_cash_balance_response,
};
use crate::protocol::{domestic_stock_balance_endpoint, domestic_stock_buying_power_endpoint};
use crate::transport::{KisRawResponse, KisRequestOptions};
use crate::{AccountSnapshot, BuyingPower, CashBalance, Position, Symbol};

use super::KisBrokerClient;
use super::helpers::split_kis_account_key;

const BALANCE_MAX_PAGES: usize = 10;

impl KisBrokerClient {
    pub async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        let pages = self.fetch_balance_pages().await?;
        let cash = pages
            .first()
            .map(|page| page.cash.clone())
            .ok_or_else(|| BrokerError::invariant("KIS balance response returned no pages"))?;
        let positions = pages.into_iter().flat_map(|page| page.positions).collect();
        Ok(AccountSnapshot { cash, positions })
    }

    pub async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        let pages = self.fetch_balance_pages().await?;
        Ok(pages.into_iter().flat_map(|page| page.positions).collect())
    }

    pub async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        let response = self.fetch_balance_page("", "", None).await?;
        map_cash_balance_response(response.body())
    }

    pub async fn get_buying_power(
        &self,
        symbol: Option<&Symbol>,
    ) -> Result<BuyingPower, BrokerError> {
        let symbol = symbol
            .ok_or_else(|| BrokerError::validation("KIS buying power lookup requires a symbol"))?;
        let account_key = self.account_key()?;
        let (cano, product_code) = split_kis_account_key(account_key)?;
        let endpoint = domestic_stock_buying_power_endpoint(self.transport.environment())?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[
                    ("CANO", cano),
                    ("ACNT_PRDT_CD", product_code),
                    ("PDNO", symbol.as_str()),
                    ("ORD_UNPR", "0"),
                    ("ORD_DVSN", "01"),
                    ("CMA_EVLU_AMT_ICLD_YN", "N"),
                    ("OVRS_ICLD_YN", "N"),
                ],
                None,
            )?)
            .await?;

        map_buying_power_response(response.body())
    }

    async fn fetch_balance_pages(&self) -> Result<Vec<BalancePage>, BrokerError> {
        let mut pages = Vec::new();
        let mut seen_tokens = BTreeSet::new();
        let mut ctx_area_fk100 = String::new();
        let mut ctx_area_nk100 = String::new();
        let mut request_tr_cont: Option<String> = None;

        for _ in 0..BALANCE_MAX_PAGES {
            let response = self
                .fetch_balance_page(&ctx_area_fk100, &ctx_area_nk100, request_tr_cont.as_deref())
                .await?;
            let response_tr_cont = response.tr_cont().unwrap_or("").to_owned();
            let page = map_balance_page_response(response.body())?;
            let has_next = matches!(response_tr_cont.as_str(), "M" | "F");

            if has_next {
                let token_key = format!("{}:{}", page.ctx_area_fk100, page.ctx_area_nk100);
                if !seen_tokens.insert(token_key) {
                    return Err(BrokerError::invariant(
                        "KIS balance continuation token repeated",
                    ));
                }
                ctx_area_fk100 = page.ctx_area_fk100.clone();
                ctx_area_nk100 = page.ctx_area_nk100.clone();
                request_tr_cont = Some("N".to_owned());
            }

            pages.push(page);

            if !has_next {
                return Ok(pages);
            }
        }

        Err(BrokerError::invariant(
            "KIS balance pagination exceeded maximum page depth",
        ))
    }

    async fn fetch_balance_page(
        &self,
        ctx_area_fk100: &str,
        ctx_area_nk100: &str,
        tr_cont: Option<&str>,
    ) -> Result<KisRawResponse, BrokerError> {
        let account_key = self.account_key()?;
        let (cano, product_code) = split_kis_account_key(account_key)?;
        let endpoint = domestic_stock_balance_endpoint(self.transport.environment())?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json_with_options(
                endpoint,
                &[
                    ("CANO", cano),
                    ("ACNT_PRDT_CD", product_code),
                    ("AFHR_FLPR_YN", "N"),
                    ("OFL_YN", ""),
                    ("INQR_DVSN", "01"),
                    ("UNPR_DVSN", "01"),
                    ("FUND_STTL_ICLD_YN", "N"),
                    ("FNCG_AMT_AUTO_RDPT_YN", "N"),
                    ("PRCS_DVSN", "00"),
                    ("CTX_AREA_FK100", ctx_area_fk100),
                    ("CTX_AREA_NK100", ctx_area_nk100),
                ],
                None,
                KisRequestOptions {
                    tr_cont,
                    hashkey: None,
                },
            )?)
            .await?;
        Ok(response)
    }
}
