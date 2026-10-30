use std::collections::HashSet;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use trading_agent_kis_client::{BrokerEnvironment, OrderSide, OrderState};

use crate::gateway::types::{
    AccountSnapshot, Fill, OrderStatus, Position, Quantity, Quote, Symbol,
};
use crate::strategy::{StrategySignal, StrategySignalAction};

pub const MAX_DECISION_REASON_CHARS: usize = 512;
pub const MAX_DECISION_RISK_NOTES: usize = 16;
pub const MAX_DECISION_RISK_NOTE_CHARS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    Buy,
    Sell,
    Hold,
}

impl DecisionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Hold => "hold",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOrderType {
    Market,
    Limit,
}

impl DecisionOrderType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Limit => "limit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionOutputStatus {
    Parsed,
    Failed,
    Rejected,
}

impl DecisionOutputStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parsed => "parsed",
            Self::Failed => "failed",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionProposal {
    pub decision_type: DecisionType,
    pub symbol: Symbol,
    pub quantity: Option<Quantity>,
    pub order_type: Option<DecisionOrderType>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub limit_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str")]
    pub confidence: Decimal,
    pub time_horizon: Option<String>,
    pub reason: String,
    pub risk_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionRequest {
    pub runtime_mode: String,
    pub environment: String,
    pub account_suffix: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub cash: Decimal,
    pub positions: Vec<DecisionPositionSnapshot>,
    pub open_orders: Vec<DecisionOpenOrderSnapshot>,
    pub today_fills: Vec<DecisionFillSnapshot>,
    pub quote: DecisionQuoteSnapshot,
    pub strategy_signal: DecisionSignalSnapshot,
    pub recent_prices: Vec<String>,
}

impl DecisionRequest {
    pub fn dry_run(
        environment: BrokerEnvironment,
        account_suffix: impl Into<String>,
        portfolio: DecisionPortfolioSnapshot<'_>,
        quote: &Quote,
        signal: &StrategySignal,
        recent_prices: &[Decimal],
    ) -> Self {
        Self::for_runtime(
            "dry_run",
            environment,
            account_suffix,
            portfolio,
            quote,
            signal,
            recent_prices,
        )
    }

    pub fn for_runtime(
        runtime_mode: impl Into<String>,
        environment: BrokerEnvironment,
        account_suffix: impl Into<String>,
        portfolio: DecisionPortfolioSnapshot<'_>,
        quote: &Quote,
        signal: &StrategySignal,
        recent_prices: &[Decimal],
    ) -> Self {
        Self {
            runtime_mode: runtime_mode.into(),
            environment: environment.as_str().to_owned(),
            account_suffix: account_suffix.into(),
            cash: portfolio.account.cash.cash,
            positions: portfolio
                .account
                .positions
                .iter()
                .map(DecisionPositionSnapshot::from_position)
                .collect(),
            open_orders: portfolio
                .open_orders
                .iter()
                .map(DecisionOpenOrderSnapshot::from_order_status)
                .collect(),
            today_fills: portfolio
                .today_fills
                .iter()
                .map(DecisionFillSnapshot::from_fill)
                .collect(),
            quote: DecisionQuoteSnapshot {
                symbol: quote.symbol.clone(),
                full_name: quote.full_name.clone(),
                last_price: quote.last_price,
            },
            strategy_signal: DecisionSignalSnapshot {
                strategy_id: signal.strategy_id.as_str().to_owned(),
                symbol: signal.symbol.clone(),
                action: signal.action,
                strength: signal.strength,
            },
            recent_prices: recent_prices.iter().map(Decimal::to_string).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DecisionPortfolioSnapshot<'a> {
    pub account: &'a AccountSnapshot,
    pub open_orders: &'a [OrderStatus],
    pub today_fills: &'a [Fill],
}

impl<'a> DecisionPortfolioSnapshot<'a> {
    pub fn new(
        account: &'a AccountSnapshot,
        open_orders: &'a [OrderStatus],
        today_fills: &'a [Fill],
    ) -> Self {
        Self {
            account,
            open_orders,
            today_fills,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionOpenOrderSnapshot {
    pub state: OrderState,
    pub symbol: Option<Symbol>,
    pub side: Option<OrderSide>,
    pub ordered_quantity: Option<Quantity>,
    pub filled_quantity: Option<Quantity>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub order_price: Option<Decimal>,
    pub trading_day: Option<String>,
}

impl DecisionOpenOrderSnapshot {
    fn from_order_status(order: &OrderStatus) -> Self {
        let evidence = order.evidence.as_ref();
        Self {
            state: order.state.clone(),
            symbol: evidence.map(|evidence| evidence.symbol.clone()),
            side: evidence.and_then(|evidence| evidence.side),
            ordered_quantity: evidence.map(|evidence| evidence.ordered_quantity),
            filled_quantity: evidence.map(|evidence| evidence.filled_quantity),
            order_price: evidence.and_then(|evidence| evidence.order_price),
            trading_day: evidence.and_then(|evidence| evidence.trading_day.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionFillSnapshot {
    pub symbol: Option<Symbol>,
    pub side: Option<OrderSide>,
    pub quantity: Quantity,
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    pub trading_day: Option<String>,
}

impl DecisionFillSnapshot {
    fn from_fill(fill: &Fill) -> Self {
        let evidence = fill.evidence.as_ref();
        Self {
            symbol: evidence.map(|evidence| evidence.symbol.clone()),
            side: evidence.and_then(|evidence| evidence.side),
            quantity: fill.quantity,
            price: fill.price,
            trading_day: evidence.and_then(|evidence| evidence.trading_day.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionPositionSnapshot {
    pub symbol: Symbol,
    pub quantity: Quantity,
    #[serde(with = "rust_decimal::serde::str")]
    pub average_price: Decimal,
}

impl DecisionPositionSnapshot {
    fn from_position(position: &Position) -> Self {
        Self {
            symbol: position.symbol.clone(),
            quantity: position.quantity,
            average_price: position.average_price,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionQuoteSnapshot {
    pub symbol: Symbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecisionSignalSnapshot {
    pub strategy_id: String,
    pub symbol: Symbol,
    pub action: StrategySignalAction,
    #[serde(with = "rust_decimal::serde::str")]
    pub strength: Decimal,
}

impl DecisionProposal {
    pub fn hold(symbol: Symbol, confidence: Decimal, reason: impl Into<String>) -> Self {
        Self {
            decision_type: DecisionType::Hold,
            symbol,
            quantity: None,
            order_type: None,
            limit_price: None,
            confidence,
            time_horizon: None,
            reason: reason.into(),
            risk_notes: Vec::new(),
        }
    }

    pub fn validate(
        self,
        context: &DecisionValidationContext,
    ) -> Result<ValidatedDecisionProposal, DecisionValidationError> {
        if self.reason.chars().count() > MAX_DECISION_REASON_CHARS {
            return Err(DecisionValidationError::ReasonTooLong {
                max_chars: MAX_DECISION_REASON_CHARS,
            });
        }
        if self.risk_notes.len() > MAX_DECISION_RISK_NOTES {
            return Err(DecisionValidationError::TooManyRiskNotes {
                max_notes: MAX_DECISION_RISK_NOTES,
            });
        }
        if self
            .risk_notes
            .iter()
            .any(|note| note.chars().count() > MAX_DECISION_RISK_NOTE_CHARS)
        {
            return Err(DecisionValidationError::RiskNoteTooLong {
                max_chars: MAX_DECISION_RISK_NOTE_CHARS,
            });
        }
        if !context.is_allowed_symbol(&self.symbol) {
            return Err(DecisionValidationError::UnknownSymbol {
                symbol: self.symbol.as_str().to_owned(),
            });
        }
        if self.confidence < Decimal::ZERO || self.confidence > Decimal::ONE {
            return Err(DecisionValidationError::InvalidConfidence {
                confidence: self.confidence,
            });
        }
        if self.is_hold() {
            return Ok(ValidatedDecisionProposal { proposal: self });
        }

        let quantity = self
            .quantity
            .ok_or(DecisionValidationError::MissingQuantity {
                decision_type: self.decision_type,
            })?;
        if quantity.is_zero() {
            return Err(DecisionValidationError::NonPositiveQuantity {
                decision_type: self.decision_type,
            });
        }

        let order_type = self
            .order_type
            .ok_or(DecisionValidationError::MissingOrderType {
                decision_type: self.decision_type,
            })?;
        if order_type == DecisionOrderType::Limit {
            let limit_price =
                self.limit_price
                    .ok_or(DecisionValidationError::MissingLimitPrice {
                        decision_type: self.decision_type,
                    })?;
            if limit_price <= Decimal::ZERO {
                return Err(DecisionValidationError::NonPositiveLimitPrice {
                    decision_type: self.decision_type,
                });
            }
        }

        Ok(ValidatedDecisionProposal { proposal: self })
    }

    pub fn is_hold(&self) -> bool {
        self.decision_type == DecisionType::Hold
    }

    pub fn is_write(&self) -> bool {
        matches!(self.decision_type, DecisionType::Buy | DecisionType::Sell)
    }

    pub fn order_notional(&self, market_reference_price: Option<Decimal>) -> Option<Decimal> {
        if !self.is_write() {
            return None;
        }
        let quantity = Decimal::from(self.quantity?.get());
        let price = match self.order_type? {
            DecisionOrderType::Limit => self.limit_price?,
            DecisionOrderType::Market => market_reference_price?,
        };
        Some(quantity * price)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionValidationContext {
    allowed_symbols: HashSet<Symbol>,
}

impl DecisionValidationContext {
    pub fn new(allowed_symbols: impl IntoIterator<Item = Symbol>) -> Self {
        Self {
            allowed_symbols: allowed_symbols.into_iter().collect(),
        }
    }

    pub fn is_allowed_symbol(&self, symbol: &Symbol) -> bool {
        self.allowed_symbols.contains(symbol)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedDecisionProposal {
    proposal: DecisionProposal,
}

impl ValidatedDecisionProposal {
    pub fn proposal(&self) -> &DecisionProposal {
        &self.proposal
    }

    pub fn into_proposal(self) -> DecisionProposal {
        self.proposal
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecisionValidationError {
    #[error("unknown decision symbol: {symbol}")]
    UnknownSymbol { symbol: String },
    #[error("{decision_type:?} decision is missing quantity")]
    MissingQuantity { decision_type: DecisionType },
    #[error("{decision_type:?} decision quantity must be greater than zero")]
    NonPositiveQuantity { decision_type: DecisionType },
    #[error("{decision_type:?} decision is missing order type")]
    MissingOrderType { decision_type: DecisionType },
    #[error("{decision_type:?} limit decision is missing limit price")]
    MissingLimitPrice { decision_type: DecisionType },
    #[error("{decision_type:?} limit price must be greater than zero")]
    NonPositiveLimitPrice { decision_type: DecisionType },
    #[error("decision confidence must be between 0 and 1: {confidence}")]
    InvalidConfidence { confidence: Decimal },
    #[error("decision reason must be {max_chars} characters or fewer")]
    ReasonTooLong { max_chars: usize },
    #[error("decision risk_notes must contain {max_notes} notes or fewer")]
    TooManyRiskNotes { max_notes: usize },
    #[error("decision risk note must be {max_chars} characters or fewer")]
    RiskNoteTooLong { max_chars: usize },
}
