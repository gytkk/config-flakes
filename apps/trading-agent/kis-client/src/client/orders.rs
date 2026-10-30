use std::collections::BTreeSet;
use time::{Date, OffsetDateTime};

use crate::core::BrokerError;
use crate::core::journal::{BrokerCommandRecord, BrokerCommandStatus};
use crate::mapping::{
    DailyCcldPage, ReviseCancelPossiblePage, map_amend_result_response, map_cancel_result_response,
    map_daily_ccld_page_response, map_order_submission_response,
    map_revise_cancel_possible_response,
};
use crate::protocol::{
    KisDailyCcldPeriod, domestic_stock_cash_order_endpoint, domestic_stock_daily_ccld_endpoint,
    domestic_stock_revise_cancel_possible_endpoint, domestic_stock_rvsecncl_order_endpoint,
};
use crate::safety::SingleWriterLockScope;
use crate::transport::{KisRawResponse, KisRequestOptions};
use crate::{
    AmendOrderRequest, AmendResult, CancelOrderRequest, CancelResult, CreateOrderRequest, Fill,
    OrderQuery, OrderStatus, OrderSubmission, RequestId, ReviseCancelCapability, TradingDay,
};

use super::helpers::{
    FollowUpOperation, ack_from_reference, cash_order_body, format_kis_date,
    is_definitive_write_rejection, revise_cancel_candidate_key, rvsecncl_order_body,
    split_kis_account_key,
};
use super::{KisBrokerClient, reserve_or_use_pre_reserved_command};

const DAILY_CCLD_MAX_PAGES: usize = 10;
const REVISE_CANCEL_POSSIBLE_MAX_PAGES: usize = 10;

impl KisBrokerClient {
    pub async fn get_order(&self, query: OrderQuery) -> Result<OrderStatus, BrokerError> {
        let orders = self
            .list_orders(TradingDay(OffsetDateTime::now_utc().date()))
            .await?;
        let target = query
            .reference
            .broker_order_id
            .as_ref()
            .ok_or_else(|| BrokerError::validation("order query requires broker_order_id"))?;

        orders
            .into_iter()
            .find(|order| order.reference.broker_order_id.as_ref() == Some(target))
            .ok_or_else(|| BrokerError::invariant(format!("order not found: {target}")))
    }

    pub async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        self.fetch_daily_ccld_orders(OffsetDateTime::now_utc().date(), "02")
            .await
    }

    pub async fn list_orders(&self, day: TradingDay) -> Result<Vec<OrderStatus>, BrokerError> {
        self.fetch_daily_ccld_orders(day.0, "00").await
    }

    pub async fn list_fills(&self, day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        self.fetch_daily_ccld_fills(day.0).await
    }

    pub async fn get_revise_cancel_capability(
        &self,
    ) -> Result<ReviseCancelCapability, BrokerError> {
        let pages = self.fetch_revise_cancel_possible_pages().await?;
        let mut candidates = Vec::new();
        let mut seen_native_keys = BTreeSet::new();

        for page in pages {
            for candidate in page.candidates {
                let key = revise_cancel_candidate_key(&candidate);
                if !seen_native_keys.insert(key) {
                    return Err(BrokerError::invariant(
                        "duplicate revise/cancel candidate returned by KIS pagination",
                    ));
                }
                candidates.push(candidate);
            }
        }

        Ok(ReviseCancelCapability { candidates })
    }

    pub async fn place_order(
        &self,
        order: CreateOrderRequest,
    ) -> Result<OrderSubmission, BrokerError> {
        self.validate_write_access(&order.account_key, SingleWriterLockScope::DomesticStock)
            .await?;
        self.ensure_market_write_gate().await?;
        let journal = self.journal()?;
        let command =
            BrokerCommandRecord::new_place_order(self.transport.environment(), order.clone())?;
        reserve_or_use_pre_reserved_command(journal.as_ref(), command).await?;
        let request_id = RequestId::new(uuid::Uuid::new_v4().to_string())?;
        journal
            .mark_submitting(&order.client_order_id, &request_id)
            .await?;

        let endpoint =
            domestic_stock_cash_order_endpoint(self.transport.environment(), order.side)?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[],
                Some(cash_order_body(&order)?),
            )?)
            .await;

        match response {
            Ok(response) => {
                match map_order_submission_response(
                    response.body(),
                    &order,
                    self.transport.environment(),
                ) {
                    Ok(submission) => {
                        journal
                            .mark_ack(
                                &order.client_order_id,
                                ack_from_reference(&submission.reference),
                            )
                            .await?;
                        Ok(submission)
                    }
                    Err(BrokerError::OrderRejected { message }) => {
                        journal
                            .mark_terminal(&order.client_order_id, BrokerCommandStatus::Rejected)
                            .await?;
                        Err(BrokerError::OrderRejected { message })
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => {
                if is_definitive_write_rejection(&error) {
                    journal
                        .mark_terminal(&order.client_order_id, BrokerCommandStatus::Rejected)
                        .await?;
                    return Err(error);
                }
                self.mark_ambiguous_write(&order.client_order_id, error)
                    .await?;
                Err(BrokerError::ReconciliationNeeded {
                    client_order_id: order.client_order_id,
                })
            }
        }
    }

    pub async fn cancel_order(
        &self,
        request: CancelOrderRequest,
    ) -> Result<CancelResult, BrokerError> {
        if !request.reference.is_complete_for_follow_up() {
            return Err(BrokerError::validation(
                "cancel order requires broker_order_id and broker_order_orgno",
            ));
        }
        self.validate_write_access(
            &request.reference.account_key,
            SingleWriterLockScope::DomesticStock,
        )
        .await?;
        self.ensure_market_write_gate().await?;
        let journal = self.journal()?;
        let parent = journal
            .get_command(&request.reference.client_order_id)
            .await?
            .ok_or_else(|| BrokerError::validation("cancel order parent command not found"))?;
        let command = BrokerCommandRecord::new_cancel_order(&parent, &request)?;
        self.ensure_follow_up_write_preflight(
            &parent,
            &request.reference,
            FollowUpOperation::Cancel,
            request.quantity,
        )
        .await?;
        reserve_or_use_pre_reserved_command(journal.as_ref(), command).await?;
        let request_id = RequestId::new(uuid::Uuid::new_v4().to_string())?;
        journal
            .mark_submitting(&request.client_order_id, &request_id)
            .await?;

        let endpoint = domestic_stock_rvsecncl_order_endpoint(self.transport.environment())?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[],
                Some(rvsecncl_order_body(
                    &request.reference,
                    "02",
                    request.quantity,
                    None,
                )?),
            )?)
            .await;

        match response {
            Ok(response) => match map_cancel_result_response(response.body(), &request) {
                Ok(result) => {
                    journal
                        .mark_terminal(&request.client_order_id, BrokerCommandStatus::Cancelled)
                        .await?;
                    Ok(result)
                }
                Err(BrokerError::OrderRejected { message }) => {
                    journal
                        .mark_terminal(&request.client_order_id, BrokerCommandStatus::Rejected)
                        .await?;
                    Err(BrokerError::OrderRejected { message })
                }
                Err(error) => Err(error),
            },
            Err(error) => {
                if is_definitive_write_rejection(&error) {
                    journal
                        .mark_terminal(&request.client_order_id, BrokerCommandStatus::Rejected)
                        .await?;
                    return Err(error);
                }
                self.mark_ambiguous_write(&request.client_order_id, error)
                    .await?;
                Err(BrokerError::ReconciliationNeeded {
                    client_order_id: request.client_order_id,
                })
            }
        }
    }

    pub async fn amend_order(
        &self,
        request: AmendOrderRequest,
    ) -> Result<AmendResult, BrokerError> {
        if !request.reference.is_complete_for_follow_up() {
            return Err(BrokerError::validation(
                "amend order requires broker_order_id and broker_order_orgno",
            ));
        }
        self.validate_write_access(
            &request.reference.account_key,
            SingleWriterLockScope::DomesticStock,
        )
        .await?;
        self.ensure_market_write_gate().await?;
        let journal = self.journal()?;
        let parent = journal
            .get_command(&request.reference.client_order_id)
            .await?
            .ok_or_else(|| BrokerError::validation("amend order parent command not found"))?;
        let command = BrokerCommandRecord::new_amend_order(&parent, &request)?;
        self.ensure_follow_up_write_preflight(
            &parent,
            &request.reference,
            FollowUpOperation::Amend,
            request.quantity,
        )
        .await?;
        reserve_or_use_pre_reserved_command(journal.as_ref(), command).await?;
        let request_id = RequestId::new(uuid::Uuid::new_v4().to_string())?;
        journal
            .mark_submitting(&request.client_order_id, &request_id)
            .await?;

        let endpoint = domestic_stock_rvsecncl_order_endpoint(self.transport.environment())?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[],
                Some(rvsecncl_order_body(
                    &request.reference,
                    "01",
                    request.quantity,
                    request.limit_price,
                )?),
            )?)
            .await;

        match response {
            Ok(response) => match map_amend_result_response(response.body(), &request) {
                Ok(result) => {
                    journal
                        .mark_terminal(&request.client_order_id, BrokerCommandStatus::Accepted)
                        .await?;
                    Ok(result)
                }
                Err(BrokerError::OrderRejected { message }) => {
                    journal
                        .mark_terminal(&request.client_order_id, BrokerCommandStatus::Rejected)
                        .await?;
                    Err(BrokerError::OrderRejected { message })
                }
                Err(error) => Err(error),
            },
            Err(error) => {
                if is_definitive_write_rejection(&error) {
                    journal
                        .mark_terminal(&request.client_order_id, BrokerCommandStatus::Rejected)
                        .await?;
                    return Err(error);
                }
                self.mark_ambiguous_write(&request.client_order_id, error)
                    .await?;
                Err(BrokerError::ReconciliationNeeded {
                    client_order_id: request.client_order_id,
                })
            }
        }
    }

    async fn fetch_daily_ccld_orders(
        &self,
        day: Date,
        ccld_dvsn: &str,
    ) -> Result<Vec<OrderStatus>, BrokerError> {
        let pages = self.fetch_daily_ccld_pages(day, ccld_dvsn).await?;
        Ok(pages.into_iter().flat_map(|page| page.orders).collect())
    }

    async fn fetch_daily_ccld_fills(&self, day: Date) -> Result<Vec<Fill>, BrokerError> {
        let pages = self.fetch_daily_ccld_pages(day, "01").await?;
        Ok(pages.into_iter().flat_map(|page| page.fills).collect())
    }

    async fn fetch_daily_ccld_pages(
        &self,
        day: Date,
        ccld_dvsn: &str,
    ) -> Result<Vec<DailyCcldPage>, BrokerError> {
        let account_key = self.account_key()?.clone();
        let mut pages = Vec::new();
        let mut seen_tokens = BTreeSet::new();
        let mut ctx_area_fk100 = String::new();
        let mut ctx_area_nk100 = String::new();
        let mut request_tr_cont: Option<String> = None;

        for _ in 0..DAILY_CCLD_MAX_PAGES {
            let response = self
                .fetch_daily_ccld_page(
                    day,
                    ccld_dvsn,
                    &ctx_area_fk100,
                    &ctx_area_nk100,
                    request_tr_cont.as_deref(),
                )
                .await?;
            let response_tr_cont = response.tr_cont().unwrap_or("").to_owned();
            let page = map_daily_ccld_page_response(
                response.body(),
                &account_key,
                self.transport.environment(),
            )?;
            let has_next = matches!(response_tr_cont.as_str(), "M" | "F");

            if has_next {
                let token_key = format!("{}:{}", page.ctx_area_fk100, page.ctx_area_nk100);
                if !seen_tokens.insert(token_key) {
                    return Err(BrokerError::invariant(
                        "KIS daily order continuation token repeated",
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
            "KIS daily order pagination exceeded maximum page depth",
        ))
    }

    async fn fetch_revise_cancel_possible_pages(
        &self,
    ) -> Result<Vec<ReviseCancelPossiblePage>, BrokerError> {
        let account_key = self.account_key()?.clone();
        let mut pages = Vec::new();
        let mut seen_tokens = BTreeSet::new();
        let mut ctx_area_fk100 = String::new();
        let mut ctx_area_nk100 = String::new();
        let mut request_tr_cont: Option<String> = None;

        for _ in 0..REVISE_CANCEL_POSSIBLE_MAX_PAGES {
            let response = self
                .fetch_revise_cancel_possible_page(
                    &ctx_area_fk100,
                    &ctx_area_nk100,
                    request_tr_cont.as_deref(),
                )
                .await?;
            let response_tr_cont = response.tr_cont().unwrap_or("").to_owned();
            let page = map_revise_cancel_possible_response(
                response.body(),
                &account_key,
                self.transport.environment(),
            )?;
            let has_next = matches!(response_tr_cont.as_str(), "M" | "F");

            if has_next {
                let token_key = format!("{}:{}", page.ctx_area_fk100, page.ctx_area_nk100);
                if !seen_tokens.insert(token_key) {
                    return Err(BrokerError::invariant(
                        "KIS revise/cancel possible continuation token repeated",
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
            "KIS revise/cancel possible pagination exceeded maximum page depth",
        ))
    }

    async fn fetch_revise_cancel_possible_page(
        &self,
        ctx_area_fk100: &str,
        ctx_area_nk100: &str,
        tr_cont: Option<&str>,
    ) -> Result<KisRawResponse, BrokerError> {
        let account_key = self.account_key()?;
        let (cano, product_code) = split_kis_account_key(account_key)?;
        let endpoint =
            domestic_stock_revise_cancel_possible_endpoint(self.transport.environment())?;
        self.transport
            .execute_json(self.transport.prepare_json_with_options(
                endpoint,
                &[
                    ("CANO", cano),
                    ("ACNT_PRDT_CD", product_code),
                    ("INQR_DVSN_1", "0"),
                    ("INQR_DVSN_2", "0"),
                    ("CTX_AREA_FK100", ctx_area_fk100),
                    ("CTX_AREA_NK100", ctx_area_nk100),
                ],
                None,
                KisRequestOptions {
                    tr_cont,
                    hashkey: None,
                },
            )?)
            .await
    }

    async fn fetch_daily_ccld_page(
        &self,
        day: Date,
        ccld_dvsn: &str,
        ctx_area_fk100: &str,
        ctx_area_nk100: &str,
        tr_cont: Option<&str>,
    ) -> Result<KisRawResponse, BrokerError> {
        let account_key = self.account_key()?;
        let (cano, product_code) = split_kis_account_key(account_key)?;
        let endpoint = domestic_stock_daily_ccld_endpoint(
            self.transport.environment(),
            KisDailyCcldPeriod::WithinThreeMonths,
        )?;
        let day = format_kis_date(day);
        self.transport
            .execute_json(self.transport.prepare_json_with_options(
                endpoint,
                &[
                    ("CANO", cano),
                    ("ACNT_PRDT_CD", product_code),
                    ("INQR_STRT_DT", &day),
                    ("INQR_END_DT", &day),
                    ("SLL_BUY_DVSN_CD", "00"),
                    ("PDNO", ""),
                    ("CCLD_DVSN", ccld_dvsn),
                    ("INQR_DVSN", "00"),
                    ("INQR_DVSN_3", "00"),
                    ("ORD_GNO_BRNO", ""),
                    ("ODNO", ""),
                    ("INQR_DVSN_1", ""),
                    ("CTX_AREA_FK100", ctx_area_fk100),
                    ("CTX_AREA_NK100", ctx_area_nk100),
                    ("EXCG_ID_DVSN_CD", "KRX"),
                ],
                None,
                KisRequestOptions {
                    tr_cont,
                    hashkey: None,
                },
            )?)
            .await
    }
}
