use std::collections::BTreeSet;

use crate::core::BrokerError;
use crate::core::journal::{BrokerCommandRecord, BrokerCommandStatus};
use crate::mapping::{
    OverseasStockBalancePage, OverseasStockOpenOrdersPage, OverseasStockOrderPage,
    map_overseas_stock_account_snapshot, map_overseas_stock_amend_result_response,
    map_overseas_stock_balance_page_response, map_overseas_stock_buying_power_response,
    map_overseas_stock_cancel_result_response, map_overseas_stock_market_session_page_response,
    map_overseas_stock_open_orders_response, map_overseas_stock_order_history_response,
    map_overseas_stock_order_submission_response, map_overseas_stock_quote_response,
};
use crate::protocol::{
    OVERSEAS_STOCK_COUNTRIES_HOLIDAY_ENDPOINT, OVERSEAS_STOCK_PRICE_ENDPOINT,
    overseas_stock_balance_endpoint, overseas_stock_buying_power_endpoint,
    overseas_stock_daytime_order_endpoint, overseas_stock_open_orders_endpoint,
    overseas_stock_order_endpoint, overseas_stock_order_history_endpoint,
    overseas_stock_rvsecncl_order_endpoint,
};
use crate::safety::SingleWriterLockScope;
use crate::transport::{KisRawResponse, KisRequestOptions};
use crate::{
    AmendOrderRequest, AmendResult, CancelOrderRequest, CancelResult, ExchangeCode, Fill,
    MarketSession, OrderStatus, OrderSubmission, OverseasStock, OverseasStockAccountSnapshot,
    OverseasStockBalanceRequest, OverseasStockBuyingPower, OverseasStockBuyingPowerRequest,
    OverseasStockOrderHistoryRequest, OverseasStockOrderRequest, OverseasStockPosition,
    OverseasStockQuote, ReviseCancelCapability, UsEquityOrderSession,
};

use super::helpers::{
    FollowUpOperation, ack_from_reference, current_us_equity_trading_day_at, format_kis_date,
    is_definitive_write_rejection, overseas_stock_order_body, overseas_stock_rvsecncl_order_body,
    revise_cancel_candidate_key, split_kis_account_key, us_equity_order_session_at,
    us_equity_order_session_trading_day_at,
};
use super::{KisBrokerClient, reserve_or_use_pre_reserved_command};

const OVERSEAS_BALANCE_MAX_PAGES: usize = 10;
const OVERSEAS_MARKET_SESSION_MAX_PAGES: usize = 10;
const OVERSEAS_OPEN_ORDERS_MAX_PAGES: usize = 10;
const OVERSEAS_ORDER_HISTORY_MAX_PAGES: usize = 10;

impl KisBrokerClient {
    pub async fn get_overseas_stock_quote(
        &self,
        instrument: &OverseasStock,
    ) -> Result<OverseasStockQuote, BrokerError> {
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                OVERSEAS_STOCK_PRICE_ENDPOINT,
                &[
                    ("AUTH", ""),
                    ("EXCD", instrument.exchange.as_str()),
                    ("SYMB", instrument.symbol.as_str()),
                ],
                None,
            )?)
            .await?;

        map_overseas_stock_quote_response(response.body(), instrument)
    }

    pub async fn get_overseas_stock_market_session(
        &self,
        exchange: &ExchangeCode,
        trading_day: &str,
    ) -> Result<MarketSession, BrokerError> {
        validate_kis_trading_day(trading_day)?;
        let mut seen_tokens = BTreeSet::new();
        let mut ctx_area_fk = String::new();
        let mut ctx_area_nk = String::new();
        let mut request_tr_cont: Option<String> = None;

        for _ in 0..OVERSEAS_MARKET_SESSION_MAX_PAGES {
            let response = self
                .fetch_overseas_stock_market_session_page(
                    trading_day,
                    &ctx_area_fk,
                    &ctx_area_nk,
                    request_tr_cont.as_deref(),
                )
                .await?;
            let response_tr_cont = response.tr_cont().unwrap_or("").to_owned();
            let page = map_overseas_stock_market_session_page_response(
                response.body(),
                exchange,
                trading_day,
            )?;

            if page.is_open {
                return Ok(MarketSession {
                    trading_day: page.trading_day,
                    is_open: true,
                });
            }

            let has_next = matches!(response_tr_cont.as_str(), "M" | "F");
            if has_next {
                let token_key = format!("{}:{}", page.ctx_area_fk, page.ctx_area_nk);
                if !seen_tokens.insert(token_key) {
                    return Err(BrokerError::invariant(
                        "KIS overseas market session continuation token repeated",
                    ));
                }
                ctx_area_fk = page.ctx_area_fk;
                ctx_area_nk = page.ctx_area_nk;
                request_tr_cont = Some("N".to_owned());
            } else {
                return Ok(MarketSession {
                    trading_day: trading_day.to_owned(),
                    is_open: false,
                });
            }
        }

        Err(BrokerError::invariant(
            "KIS overseas market session pagination exceeded maximum page depth",
        ))
    }

    pub async fn get_overseas_stock_regular_market_session(
        &self,
        exchange: &ExchangeCode,
    ) -> Result<MarketSession, BrokerError> {
        let now_utc = self
            .market_session_now
            .unwrap_or_else(time::OffsetDateTime::now_utc);
        let trading_day = current_us_equity_trading_day_at(now_utc);
        let mut session = self
            .get_overseas_stock_market_session(exchange, &trading_day)
            .await?;
        session.is_open =
            session.is_open && super::helpers::is_us_equity_regular_session_time(now_utc);
        Ok(session)
    }

    pub async fn is_overseas_stock_order_session_open(
        &self,
        exchange: &ExchangeCode,
        allowed_sessions: &[UsEquityOrderSession],
    ) -> Result<bool, BrokerError> {
        let now_utc = self
            .market_session_now
            .unwrap_or_else(time::OffsetDateTime::now_utc);
        let Some(order_session) = us_equity_order_session_at(now_utc) else {
            return Ok(false);
        };
        if !allowed_sessions.contains(&order_session) {
            return Ok(false);
        }
        let trading_day = us_equity_order_session_trading_day_at(now_utc, order_session);
        let session = self
            .get_overseas_stock_market_session(exchange, &trading_day)
            .await?;
        if session.trading_day != trading_day {
            return Err(BrokerError::invariant(format!(
                "KIS overseas market session response day {} does not match requested trading day {trading_day}",
                session.trading_day
            )));
        }
        Ok(session.is_open)
    }

    pub async fn get_overseas_stock_account_snapshot(
        &self,
        request: &OverseasStockBalanceRequest,
    ) -> Result<OverseasStockAccountSnapshot, BrokerError> {
        let pages = self.fetch_overseas_stock_balance_pages(request).await?;
        map_overseas_stock_account_snapshot(pages)
    }

    pub async fn get_overseas_stock_positions(
        &self,
        request: &OverseasStockBalanceRequest,
    ) -> Result<Vec<OverseasStockPosition>, BrokerError> {
        let pages = self.fetch_overseas_stock_balance_pages(request).await?;
        Ok(pages.into_iter().flat_map(|page| page.positions).collect())
    }

    pub async fn get_overseas_stock_buying_power(
        &self,
        request: &OverseasStockBuyingPowerRequest,
    ) -> Result<OverseasStockBuyingPower, BrokerError> {
        let account_key = self.account_key()?;
        let (cano, product_code) = split_kis_account_key(account_key)?;
        let endpoint = overseas_stock_buying_power_endpoint(self.transport.environment())?;
        let order_price = request.order_price.to_string();
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[
                    ("CANO", cano),
                    ("ACNT_PRDT_CD", product_code),
                    ("OVRS_EXCG_CD", request.instrument.exchange.as_str()),
                    ("OVRS_ORD_UNPR", &order_price),
                    ("ITEM_CD", request.instrument.symbol.as_str()),
                ],
                None,
            )?)
            .await?;

        map_overseas_stock_buying_power_response(response.body())
    }

    pub async fn place_overseas_stock_order(
        &self,
        order: OverseasStockOrderRequest,
    ) -> Result<OrderSubmission, BrokerError> {
        self.validate_write_access(&order.account_key, SingleWriterLockScope::OverseasStock)
            .await?;
        let order_session = self
            .ensure_overseas_market_write_gate(&order.instrument.exchange)
            .await?;
        let journal = self.journal()?;
        let command = BrokerCommandRecord::new_place_overseas_stock_order(
            self.transport.environment(),
            order.clone(),
        )?;
        reserve_or_use_pre_reserved_command(journal.as_ref(), command).await?;
        let request_id = crate::RequestId::new(uuid::Uuid::new_v4().to_string())?;
        journal
            .mark_submitting(&order.client_order_id, &request_id)
            .await?;

        let endpoint = overseas_order_endpoint_for_session(
            self.transport.environment(),
            order.side,
            &order.instrument.exchange,
            order_session,
        )?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[],
                Some(overseas_stock_order_body(&order)?),
            )?)
            .await;

        match response {
            Ok(response) => match map_overseas_stock_order_submission_response(
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
            },
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

    pub async fn cancel_overseas_stock_order(
        &self,
        request: CancelOrderRequest,
    ) -> Result<CancelResult, BrokerError> {
        if !request.reference.is_complete_for_follow_up() {
            return Err(BrokerError::validation(
                "cancel overseas order requires broker_order_id and broker_order_orgno",
            ));
        }
        self.validate_write_access(
            &request.reference.account_key,
            SingleWriterLockScope::OverseasStock,
        )
        .await?;
        let journal = self.journal()?;
        let parent = journal
            .get_command(&request.reference.client_order_id)
            .await?
            .ok_or_else(|| {
                BrokerError::validation("cancel overseas order parent command not found")
            })?;
        self.ensure_overseas_market_write_gate(&overseas_follow_up_exchange(
            &parent,
            &request.reference,
        )?)
        .await?;
        let candidate = self
            .ensure_overseas_follow_up_write_preflight(
                &parent,
                &request.reference,
                FollowUpOperation::Cancel,
                request.quantity,
            )
            .await?;
        let order_quantity = request
            .quantity
            .unwrap_or_else(|| candidate.cancelable_quantity.quantity());
        let command = BrokerCommandRecord::new_cancel_order(&parent, &request)?;
        journal.reserve_command(command).await?;
        let request_id = crate::RequestId::new(uuid::Uuid::new_v4().to_string())?;
        journal
            .mark_submitting(&request.client_order_id, &request_id)
            .await?;

        let endpoint = overseas_stock_rvsecncl_order_endpoint(self.transport.environment())?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[],
                Some(overseas_stock_rvsecncl_order_body(
                    &parent,
                    &request.reference,
                    "02",
                    order_quantity,
                    None,
                )?),
            )?)
            .await;

        match response {
            Ok(response) => {
                match map_overseas_stock_cancel_result_response(response.body(), &request) {
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
                }
            }
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

    pub async fn amend_overseas_stock_order(
        &self,
        request: AmendOrderRequest,
    ) -> Result<AmendResult, BrokerError> {
        if !request.reference.is_complete_for_follow_up() {
            return Err(BrokerError::validation(
                "amend overseas order requires broker_order_id and broker_order_orgno",
            ));
        }
        self.validate_write_access(
            &request.reference.account_key,
            SingleWriterLockScope::OverseasStock,
        )
        .await?;
        let journal = self.journal()?;
        let parent = journal
            .get_command(&request.reference.client_order_id)
            .await?
            .ok_or_else(|| {
                BrokerError::validation("amend overseas order parent command not found")
            })?;
        self.ensure_overseas_market_write_gate(&overseas_follow_up_exchange(
            &parent,
            &request.reference,
        )?)
        .await?;
        let candidate = self
            .ensure_overseas_follow_up_write_preflight(
                &parent,
                &request.reference,
                FollowUpOperation::Amend,
                request.quantity,
            )
            .await?;
        let order_quantity = request
            .quantity
            .unwrap_or_else(|| candidate.amendable_quantity.quantity());
        let command = BrokerCommandRecord::new_amend_order(&parent, &request)?;
        journal.reserve_command(command).await?;
        let request_id = crate::RequestId::new(uuid::Uuid::new_v4().to_string())?;
        journal
            .mark_submitting(&request.client_order_id, &request_id)
            .await?;

        let endpoint = overseas_stock_rvsecncl_order_endpoint(self.transport.environment())?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json(
                endpoint,
                &[],
                Some(overseas_stock_rvsecncl_order_body(
                    &parent,
                    &request.reference,
                    "01",
                    order_quantity,
                    request.limit_price,
                )?),
            )?)
            .await;

        match response {
            Ok(response) => {
                match map_overseas_stock_amend_result_response(response.body(), &request) {
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
                }
            }
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

    pub async fn get_overseas_stock_revise_cancel_capability(
        &self,
        exchange: &ExchangeCode,
    ) -> Result<ReviseCancelCapability, BrokerError> {
        let pages = self.fetch_overseas_stock_open_order_pages(exchange).await?;
        let mut candidates = Vec::new();
        let mut seen_native_keys = BTreeSet::new();

        for page in pages {
            for candidate in page.candidates {
                let key = revise_cancel_candidate_key(&candidate);
                if !seen_native_keys.insert(key) {
                    return Err(BrokerError::invariant(
                        "duplicate overseas revise/cancel candidate returned by KIS pagination",
                    ));
                }
                candidates.push(candidate);
            }
        }

        Ok(ReviseCancelCapability { candidates })
    }

    pub async fn list_overseas_stock_open_orders(
        &self,
        exchange: &ExchangeCode,
    ) -> Result<Vec<OrderStatus>, BrokerError> {
        let pages = self.fetch_overseas_stock_open_order_pages(exchange).await?;
        Ok(pages.into_iter().flat_map(|page| page.orders).collect())
    }

    pub async fn list_overseas_stock_orders(
        &self,
        request: &OverseasStockOrderHistoryRequest,
    ) -> Result<Vec<OrderStatus>, BrokerError> {
        let pages = self
            .fetch_overseas_stock_order_history_pages(request)
            .await?;
        Ok(pages.into_iter().flat_map(|page| page.orders).collect())
    }

    pub async fn list_overseas_stock_fills(
        &self,
        request: &OverseasStockOrderHistoryRequest,
    ) -> Result<Vec<Fill>, BrokerError> {
        let pages = self
            .fetch_overseas_stock_order_history_pages(request)
            .await?;
        Ok(pages.into_iter().flat_map(|page| page.fills).collect())
    }

    async fn fetch_overseas_stock_balance_pages(
        &self,
        request: &OverseasStockBalanceRequest,
    ) -> Result<Vec<OverseasStockBalancePage>, BrokerError> {
        let mut pages = Vec::new();
        let mut seen_tokens = BTreeSet::new();
        let mut ctx_area_fk200 = String::new();
        let mut ctx_area_nk200 = String::new();
        let mut request_tr_cont: Option<String> = None;

        for _ in 0..OVERSEAS_BALANCE_MAX_PAGES {
            let response = self
                .fetch_overseas_stock_balance_page(
                    request,
                    &ctx_area_fk200,
                    &ctx_area_nk200,
                    request_tr_cont.as_deref(),
                )
                .await?;
            let response_tr_cont = response.tr_cont().unwrap_or("").to_owned();
            let page = map_overseas_stock_balance_page_response(response.body(), request)?;
            let has_next = matches!(response_tr_cont.as_str(), "M" | "F");

            if has_next {
                let token_key = format!("{}:{}", page.ctx_area_fk200, page.ctx_area_nk200);
                if !seen_tokens.insert(token_key) {
                    return Err(BrokerError::invariant(
                        "KIS overseas balance continuation token repeated",
                    ));
                }
                ctx_area_fk200 = page.ctx_area_fk200.clone();
                ctx_area_nk200 = page.ctx_area_nk200.clone();
                request_tr_cont = Some("N".to_owned());
            }

            pages.push(page);

            if !has_next {
                return Ok(pages);
            }
        }

        Err(BrokerError::invariant(
            "KIS overseas balance pagination exceeded maximum page depth",
        ))
    }

    async fn fetch_overseas_stock_market_session_page(
        &self,
        trading_day: &str,
        ctx_area_fk: &str,
        ctx_area_nk: &str,
        tr_cont: Option<&str>,
    ) -> Result<KisRawResponse, BrokerError> {
        self.transport
            .execute_json(self.transport.prepare_json_with_options(
                OVERSEAS_STOCK_COUNTRIES_HOLIDAY_ENDPOINT,
                &[
                    ("TRAD_DT", trading_day),
                    ("CTX_AREA_NK", ctx_area_nk),
                    ("CTX_AREA_FK", ctx_area_fk),
                ],
                None,
                KisRequestOptions {
                    tr_cont,
                    hashkey: None,
                },
            )?)
            .await
    }

    async fn fetch_overseas_stock_balance_page(
        &self,
        request: &OverseasStockBalanceRequest,
        ctx_area_fk200: &str,
        ctx_area_nk200: &str,
        tr_cont: Option<&str>,
    ) -> Result<KisRawResponse, BrokerError> {
        let account_key = self.account_key()?;
        let (cano, product_code) = split_kis_account_key(account_key)?;
        let endpoint = overseas_stock_balance_endpoint(self.transport.environment())?;
        let response = self
            .transport
            .execute_json(self.transport.prepare_json_with_options(
                endpoint,
                &[
                    ("CANO", cano),
                    ("ACNT_PRDT_CD", product_code),
                    ("OVRS_EXCG_CD", request.exchange.as_str()),
                    ("TR_CRCY_CD", request.currency.as_str()),
                    ("CTX_AREA_FK200", ctx_area_fk200),
                    ("CTX_AREA_NK200", ctx_area_nk200),
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

    async fn fetch_overseas_stock_open_order_pages(
        &self,
        exchange: &ExchangeCode,
    ) -> Result<Vec<OverseasStockOpenOrdersPage>, BrokerError> {
        let mut pages = Vec::new();
        let mut seen_tokens = BTreeSet::new();
        let mut ctx_area_fk200 = String::new();
        let mut ctx_area_nk200 = String::new();
        let mut request_tr_cont: Option<String> = None;

        for _ in 0..OVERSEAS_OPEN_ORDERS_MAX_PAGES {
            let response = self
                .fetch_overseas_stock_open_order_page(
                    exchange,
                    &ctx_area_fk200,
                    &ctx_area_nk200,
                    request_tr_cont.as_deref(),
                )
                .await?;
            let response_tr_cont = response.tr_cont().unwrap_or("").to_owned();
            let account_key = self.account_key()?.clone();
            let page = map_overseas_stock_open_orders_response(
                response.body(),
                &account_key,
                self.transport.environment(),
            )?;
            let has_next = matches!(response_tr_cont.as_str(), "M" | "F");

            if has_next {
                let token_key = format!("{}:{}", page.ctx_area_fk200, page.ctx_area_nk200);
                if !seen_tokens.insert(token_key) {
                    return Err(BrokerError::invariant(
                        "KIS overseas open orders continuation token repeated",
                    ));
                }
                ctx_area_fk200 = page.ctx_area_fk200.clone();
                ctx_area_nk200 = page.ctx_area_nk200.clone();
                request_tr_cont = Some("N".to_owned());
            }

            pages.push(page);

            if !has_next {
                return Ok(pages);
            }
        }

        Err(BrokerError::invariant(
            "KIS overseas open orders pagination exceeded maximum page depth",
        ))
    }

    async fn fetch_overseas_stock_open_order_page(
        &self,
        exchange: &ExchangeCode,
        ctx_area_fk200: &str,
        ctx_area_nk200: &str,
        tr_cont: Option<&str>,
    ) -> Result<KisRawResponse, BrokerError> {
        let account_key = self.account_key()?;
        let (cano, product_code) = split_kis_account_key(account_key)?;
        let endpoint = overseas_stock_open_orders_endpoint(self.transport.environment())?;
        self.transport
            .execute_json(self.transport.prepare_json_with_options(
                endpoint,
                &[
                    ("CANO", cano),
                    ("ACNT_PRDT_CD", product_code),
                    ("OVRS_EXCG_CD", exchange.as_str()),
                    ("SORT_SQN", "DS"),
                    ("CTX_AREA_FK200", ctx_area_fk200),
                    ("CTX_AREA_NK200", ctx_area_nk200),
                ],
                None,
                KisRequestOptions {
                    tr_cont,
                    hashkey: None,
                },
            )?)
            .await
    }

    async fn fetch_overseas_stock_order_history_pages(
        &self,
        request: &OverseasStockOrderHistoryRequest,
    ) -> Result<Vec<OverseasStockOrderPage>, BrokerError> {
        let mut pages = Vec::new();
        let mut seen_tokens = BTreeSet::new();
        let mut ctx_area_fk200 = String::new();
        let mut ctx_area_nk200 = String::new();
        let mut request_tr_cont: Option<String> = None;

        for _ in 0..OVERSEAS_ORDER_HISTORY_MAX_PAGES {
            let response = self
                .fetch_overseas_stock_order_history_page(
                    request,
                    &ctx_area_fk200,
                    &ctx_area_nk200,
                    request_tr_cont.as_deref(),
                )
                .await?;
            let response_tr_cont = response.tr_cont().unwrap_or("").to_owned();
            let account_key = self.account_key()?.clone();
            let page = map_overseas_stock_order_history_response(
                response.body(),
                &account_key,
                self.transport.environment(),
            )?;
            let has_next = matches!(response_tr_cont.as_str(), "M" | "F");

            if has_next {
                let token_key = format!("{}:{}", page.ctx_area_fk200, page.ctx_area_nk200);
                if !seen_tokens.insert(token_key) {
                    return Err(BrokerError::invariant(
                        "KIS overseas order history continuation token repeated",
                    ));
                }
                ctx_area_fk200 = page.ctx_area_fk200.clone();
                ctx_area_nk200 = page.ctx_area_nk200.clone();
                request_tr_cont = Some("N".to_owned());
            }

            pages.push(page);

            if !has_next {
                return Ok(pages);
            }
        }

        Err(BrokerError::invariant(
            "KIS overseas order history pagination exceeded maximum page depth",
        ))
    }

    async fn fetch_overseas_stock_order_history_page(
        &self,
        request: &OverseasStockOrderHistoryRequest,
        ctx_area_fk200: &str,
        ctx_area_nk200: &str,
        tr_cont: Option<&str>,
    ) -> Result<KisRawResponse, BrokerError> {
        let account_key = self.account_key()?;
        let (cano, product_code) = split_kis_account_key(account_key)?;
        let endpoint = overseas_stock_order_history_endpoint(self.transport.environment())?;
        let start_day = format_kis_date(request.start_day.0);
        let end_day = format_kis_date(request.end_day.0);
        let symbol = request
            .symbol
            .as_ref()
            .map(crate::Symbol::as_str)
            .unwrap_or("");
        let exchange = request
            .exchange
            .as_ref()
            .map(ExchangeCode::as_str)
            .unwrap_or("");

        self.transport
            .execute_json(self.transport.prepare_json_with_options(
                endpoint,
                &[
                    ("CANO", cano),
                    ("ACNT_PRDT_CD", product_code),
                    ("PDNO", symbol),
                    ("ORD_STRT_DT", &start_day),
                    ("ORD_END_DT", &end_day),
                    ("SLL_BUY_DVSN", request.side.as_kis_code()),
                    ("CCLD_NCCS_DVSN", request.status.as_kis_code()),
                    ("OVRS_EXCG_CD", exchange),
                    ("SORT_SQN", request.sort.as_kis_code()),
                    ("ORD_DT", ""),
                    ("ORD_GNO_BRNO", ""),
                    ("ODNO", ""),
                    ("CTX_AREA_NK200", ctx_area_nk200),
                    ("CTX_AREA_FK200", ctx_area_fk200),
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

fn validate_kis_trading_day(trading_day: &str) -> Result<(), BrokerError> {
    if trading_day.len() != 8
        || !trading_day
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(BrokerError::validation(
            "trading_day must be formatted as YYYYMMDD",
        ));
    }
    Ok(())
}

fn overseas_follow_up_exchange(
    parent: &BrokerCommandRecord,
    reference: &crate::OrderReference,
) -> Result<ExchangeCode, BrokerError> {
    if let Some(exchange) = &reference.exchange_code {
        return Ok(exchange.clone());
    }
    if let Some(exchange) = &parent.exchange_code {
        return ExchangeCode::new(exchange.clone());
    }
    Err(BrokerError::validation(
        "overseas follow-up order requires exchange code",
    ))
}

fn overseas_order_endpoint_for_session(
    environment: crate::BrokerEnvironment,
    side: crate::OrderSide,
    exchange: &ExchangeCode,
    order_session: UsEquityOrderSession,
) -> Result<crate::transport::KisEndpoint, BrokerError> {
    match order_session {
        UsEquityOrderSession::Daytime => overseas_stock_daytime_order_endpoint(environment, side),
        UsEquityOrderSession::Premarket
        | UsEquityOrderSession::Regular
        | UsEquityOrderSession::Aftermarket => {
            overseas_stock_order_endpoint(environment, side, exchange)
        }
    }
}
