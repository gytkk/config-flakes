use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use rust_decimal::Decimal;
use trading_agent_kis_client::BrokerError;
use trading_agent_kis_client::domain::{
    BrokerEnvironment, BrokerHealth, BuyingPower, CashBalance, MarketSession,
};

use crate::gateway::traits::TradingGateway;
use crate::gateway::types::{
    AccountSnapshot, AmendOrderRequest, AmendResult, CancelOrderRequest, CancelResult,
    CreateOrderRequest, Fill, OrderStatus, OrderSubmission, Position, Quote, Symbol, TradingDay,
};

#[derive(Debug, Clone)]
pub struct MockTradingGateway {
    cash: CashBalance,
    buying_power: BuyingPower,
    positions: Vec<Position>,
    open_orders: Vec<OrderStatus>,
    fills: Vec<Fill>,
    quotes: HashMap<String, Quote>,
    health: BrokerHealth,
    market_session: MarketSession,
    order_submissions: Arc<Mutex<VecDeque<Result<OrderSubmission, BrokerError>>>>,
    cancel_results: Arc<Mutex<VecDeque<Result<CancelResult, BrokerError>>>>,
    amend_results: Arc<Mutex<VecDeque<Result<AmendResult, BrokerError>>>>,
    call_counts: Arc<GatewayCallCounters>,
}

impl MockTradingGateway {
    pub fn new() -> Self {
        Self {
            cash: CashBalance {
                cash: Decimal::ZERO,
            },
            buying_power: BuyingPower {
                available_cash: Decimal::ZERO,
            },
            positions: Vec::new(),
            open_orders: Vec::new(),
            fills: Vec::new(),
            quotes: HashMap::new(),
            health: BrokerHealth {
                environment: BrokerEnvironment::Paper,
                writable: false,
            },
            market_session: MarketSession {
                trading_day: "19700101".to_owned(),
                is_open: true,
            },
            order_submissions: Arc::new(Mutex::new(VecDeque::new())),
            cancel_results: Arc::new(Mutex::new(VecDeque::new())),
            amend_results: Arc::new(Mutex::new(VecDeque::new())),
            call_counts: Arc::new(GatewayCallCounters::default()),
        }
    }

    pub fn with_cash(mut self, cash: Decimal) -> Self {
        self.cash = CashBalance { cash };
        self.buying_power = BuyingPower {
            available_cash: cash,
        };
        self
    }

    pub fn with_buying_power(mut self, available_cash: Decimal) -> Self {
        self.buying_power = BuyingPower { available_cash };
        self
    }

    pub fn with_position(mut self, position: Position) -> Self {
        self.positions.push(position);
        self
    }

    pub fn with_quote(mut self, quote: Quote) -> Self {
        self.quotes.insert(quote.symbol.as_str().to_owned(), quote);
        self
    }

    pub fn with_open_order(mut self, order: OrderStatus) -> Self {
        self.open_orders.push(order);
        self
    }

    pub fn with_fill(mut self, fill: Fill) -> Self {
        self.fills.push(fill);
        self
    }

    pub fn with_market_session(mut self, market_session: MarketSession) -> Self {
        self.market_session = market_session;
        self
    }

    pub fn with_order_submission(self, submission: OrderSubmission) -> Self {
        self.order_submissions
            .lock()
            .expect("mock order submission lock")
            .push_back(Ok(submission));
        self
    }

    pub fn with_order_error(self, error: BrokerError) -> Self {
        self.order_submissions
            .lock()
            .expect("mock order submission lock")
            .push_back(Err(error));
        self
    }

    pub fn with_cancel_result(self, result: CancelResult) -> Self {
        self.cancel_results
            .lock()
            .expect("mock cancel result lock")
            .push_back(Ok(result));
        self
    }

    pub fn with_cancel_error(self, error: BrokerError) -> Self {
        self.cancel_results
            .lock()
            .expect("mock cancel result lock")
            .push_back(Err(error));
        self
    }

    pub fn with_amend_result(self, result: AmendResult) -> Self {
        self.amend_results
            .lock()
            .expect("mock amend result lock")
            .push_back(Ok(result));
        self
    }

    pub fn with_amend_error(self, error: BrokerError) -> Self {
        self.amend_results
            .lock()
            .expect("mock amend result lock")
            .push_back(Err(error));
        self
    }

    pub fn call_counts(&self) -> GatewayCallCounts {
        GatewayCallCounts {
            ping: self.call_counts.ping.load(Ordering::SeqCst),
            account_snapshot: self.call_counts.account_snapshot.load(Ordering::SeqCst),
            cash_balance: self.call_counts.cash_balance.load(Ordering::SeqCst),
            positions: self.call_counts.positions.load(Ordering::SeqCst),
            quotes: self.call_counts.quotes.load(Ordering::SeqCst),
            market_session: self.call_counts.market_session.load(Ordering::SeqCst),
            buying_power: self.call_counts.buying_power.load(Ordering::SeqCst),
            open_orders: self.call_counts.open_orders.load(Ordering::SeqCst),
            fills: self.call_counts.fills.load(Ordering::SeqCst),
            place_order: self.call_counts.place_order.load(Ordering::SeqCst),
            cancel_order: self.call_counts.cancel_order.load(Ordering::SeqCst),
            amend_order: self.call_counts.amend_order.load(Ordering::SeqCst),
        }
    }
}

impl Default for MockTradingGateway {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TradingGateway for MockTradingGateway {
    async fn ping(&self) -> Result<BrokerHealth, BrokerError> {
        self.call_counts.ping.fetch_add(1, Ordering::SeqCst);
        Ok(self.health.clone())
    }

    async fn get_account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        self.call_counts
            .account_snapshot
            .fetch_add(1, Ordering::SeqCst);
        Ok(AccountSnapshot {
            cash: self.cash.clone(),
            positions: self.positions.clone(),
        })
    }

    async fn get_cash_balance(&self) -> Result<CashBalance, BrokerError> {
        self.call_counts.cash_balance.fetch_add(1, Ordering::SeqCst);
        Ok(self.cash.clone())
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        self.call_counts.positions.fetch_add(1, Ordering::SeqCst);
        Ok(self.positions.clone())
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        self.call_counts.quotes.fetch_add(1, Ordering::SeqCst);
        self.quotes
            .get(symbol.as_str())
            .cloned()
            .ok_or_else(|| BrokerError::validation(format!("missing quote for symbol {symbol}")))
    }

    async fn get_market_session(&self, _trading_day: &str) -> Result<MarketSession, BrokerError> {
        self.call_counts
            .market_session
            .fetch_add(1, Ordering::SeqCst);
        Ok(self.market_session.clone())
    }

    async fn get_buying_power(&self, _symbol: Option<&Symbol>) -> Result<BuyingPower, BrokerError> {
        self.call_counts.buying_power.fetch_add(1, Ordering::SeqCst);
        Ok(self.buying_power.clone())
    }

    async fn list_open_orders(&self) -> Result<Vec<OrderStatus>, BrokerError> {
        self.call_counts.open_orders.fetch_add(1, Ordering::SeqCst);
        Ok(self.open_orders.clone())
    }

    async fn list_fills(&self, _day: TradingDay) -> Result<Vec<Fill>, BrokerError> {
        self.call_counts.fills.fetch_add(1, Ordering::SeqCst);
        Ok(self.fills.clone())
    }

    async fn place_order(
        &self,
        _order: CreateOrderRequest,
    ) -> Result<OrderSubmission, BrokerError> {
        self.call_counts.place_order.fetch_add(1, Ordering::SeqCst);
        self.order_submissions
            .lock()
            .expect("mock order submission lock")
            .pop_front()
            .unwrap_or_else(|| Err(BrokerError::validation("missing mock order submission")))
    }

    async fn cancel_order(
        &self,
        _request: CancelOrderRequest,
    ) -> Result<CancelResult, BrokerError> {
        self.call_counts.cancel_order.fetch_add(1, Ordering::SeqCst);
        self.cancel_results
            .lock()
            .expect("mock cancel result lock")
            .pop_front()
            .unwrap_or_else(|| Err(BrokerError::validation("missing mock cancel result")))
    }

    async fn amend_order(&self, _request: AmendOrderRequest) -> Result<AmendResult, BrokerError> {
        self.call_counts.amend_order.fetch_add(1, Ordering::SeqCst);
        self.amend_results
            .lock()
            .expect("mock amend result lock")
            .pop_front()
            .unwrap_or_else(|| Err(BrokerError::validation("missing mock amend result")))
    }
}

#[derive(Debug, Default)]
struct GatewayCallCounters {
    ping: AtomicUsize,
    account_snapshot: AtomicUsize,
    cash_balance: AtomicUsize,
    positions: AtomicUsize,
    quotes: AtomicUsize,
    market_session: AtomicUsize,
    buying_power: AtomicUsize,
    open_orders: AtomicUsize,
    fills: AtomicUsize,
    place_order: AtomicUsize,
    cancel_order: AtomicUsize,
    amend_order: AtomicUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayCallCounts {
    pub ping: usize,
    pub account_snapshot: usize,
    pub cash_balance: usize,
    pub positions: usize,
    pub quotes: usize,
    pub market_session: usize,
    pub buying_power: usize,
    pub open_orders: usize,
    pub fills: usize,
    pub place_order: usize,
    pub cancel_order: usize,
    pub amend_order: usize,
}
