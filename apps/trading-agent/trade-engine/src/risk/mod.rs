use std::collections::HashSet;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::decision::{DecisionOrderType, DecisionType, ValidatedDecisionProposal};
use crate::gateway::types::{AccountSnapshot, Fill, Quantity, Symbol};
use trading_agent_kis_client::OrderSide;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskPolicy {
    read_only: bool,
    live_trading_enabled: bool,
    allowed_symbols: HashSet<Symbol>,
    max_order_notional: Decimal,
    max_position_notional: Option<Decimal>,
    max_daily_loss: Option<Decimal>,
    max_daily_order_count: Option<u32>,
}

impl RiskPolicy {
    pub fn new(
        read_only: bool,
        live_trading_enabled: bool,
        allowed_symbols: impl IntoIterator<Item = Symbol>,
        max_order_notional: Decimal,
    ) -> Self {
        Self {
            read_only,
            live_trading_enabled,
            allowed_symbols: allowed_symbols.into_iter().collect(),
            max_order_notional,
            max_position_notional: None,
            max_daily_loss: None,
            max_daily_order_count: None,
        }
    }

    pub fn with_position_notional_limit(mut self, max_position_notional: Decimal) -> Self {
        self.max_position_notional = Some(max_position_notional);
        self
    }

    pub fn with_daily_order_limit(mut self, max_daily_order_count: u32) -> Self {
        self.max_daily_order_count = Some(max_daily_order_count);
        self
    }

    pub fn with_daily_loss_limit(mut self, max_daily_loss: Decimal) -> Self {
        self.max_daily_loss = Some(max_daily_loss);
        self
    }

    pub fn snapshot(&self) -> RiskPolicySnapshot {
        let mut allowed_symbols = self
            .allowed_symbols
            .iter()
            .map(|symbol| symbol.as_str().to_owned())
            .collect::<Vec<_>>();
        allowed_symbols.sort();
        RiskPolicySnapshot {
            policy_version: "risk_policy_v1".to_owned(),
            read_only: self.read_only,
            live_trading_enabled: self.live_trading_enabled,
            allowed_symbols,
            max_order_notional: self.max_order_notional,
            max_position_notional: self.max_position_notional,
            max_daily_loss: self.max_daily_loss,
            max_daily_order_count: self.max_daily_order_count,
        }
    }

    pub fn evaluate(&self, request: RiskEvaluationRequest<'_>) -> RiskEvaluation {
        let proposal = request.proposal.proposal();
        let order_notional = proposal.order_notional(None);
        if proposal.is_hold() {
            return RiskEvaluation::approved(order_notional);
        }

        let mut reasons = Vec::new();
        if self.read_only {
            reasons.push(RiskBlockReason::ReadOnlyMode);
        }
        if !self.live_trading_enabled {
            reasons.push(RiskBlockReason::LiveTradingDisabled);
        }
        if !request.context.market_session_open {
            reasons.push(RiskBlockReason::MarketClosed);
        }
        if request.context.unresolved_journal_commands {
            reasons.push(RiskBlockReason::UnresolvedJournalCommands);
        }
        if request.context.panic_stop_active {
            reasons.push(RiskBlockReason::PanicStopActive);
        }
        if !self.allowed_symbols.contains(&proposal.symbol) {
            reasons.push(RiskBlockReason::SymbolNotAllowed {
                symbol: proposal.symbol.as_str().to_owned(),
            });
        }
        if proposal.decision_type == DecisionType::Buy
            && let (Some(required), Some(available)) =
                (order_notional, request.context.available_buying_power)
            && available < required
        {
            reasons.push(RiskBlockReason::InsufficientBuyingPower {
                required,
                available,
            });
        }
        if proposal.decision_type == DecisionType::Sell
            && let Some(requested) = proposal.quantity
            && requested.get() > request.context.current_position_quantity.get()
        {
            reasons.push(RiskBlockReason::SellQuantityExceedsPosition {
                requested,
                available: request.context.current_position_quantity,
            });
        }
        if request.context.duplicate_order_intent {
            reasons.push(RiskBlockReason::DuplicateOrderIntent);
        }
        if proposal.order_type == Some(DecisionOrderType::Market) && order_notional.is_none() {
            reasons.push(RiskBlockReason::MarketOrderRequiresReferencePrice);
        }
        if let Some(notional) = order_notional
            && notional > self.max_order_notional
        {
            reasons.push(RiskBlockReason::MaxOrderNotionalExceeded {
                notional,
                max_order_notional: self.max_order_notional,
            });
        }
        if let (Some(projected_notional), Some(max_position_notional)) = (
            request.context.projected_position_notional,
            self.max_position_notional,
        ) && projected_notional > max_position_notional
        {
            reasons.push(RiskBlockReason::MaxPositionNotionalExceeded {
                projected_notional,
                max_position_notional,
            });
        }
        if let Some(max_daily_orders) = self.max_daily_order_count
            && request.context.submitted_order_count_today >= max_daily_orders
        {
            reasons.push(RiskBlockReason::MaxDailyOrderCountExceeded {
                submitted_orders: request.context.submitted_order_count_today,
                max_daily_orders,
            });
        }
        if let Some(max_daily_loss) = self.max_daily_loss {
            match request.context.realized_loss_today {
                DailyLossState::Known(realized_loss) if realized_loss >= max_daily_loss => {
                    reasons.push(RiskBlockReason::MaxDailyLossExceeded {
                        realized_loss,
                        max_daily_loss,
                    });
                }
                DailyLossState::Unavailable(reason) => {
                    reasons.push(RiskBlockReason::DailyLossUnavailable { reason });
                }
                DailyLossState::Known(_) => {}
            }
        }
        if reasons.is_empty() {
            RiskEvaluation::approved(order_notional)
        } else {
            RiskEvaluation::blocked(reasons, order_notional)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskPolicySnapshot {
    pub policy_version: String,
    pub read_only: bool,
    pub live_trading_enabled: bool,
    pub allowed_symbols: Vec<String>,
    #[serde(with = "rust_decimal::serde::str")]
    pub max_order_notional: Decimal,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub max_position_notional: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub max_daily_loss: Option<Decimal>,
    pub max_daily_order_count: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct RiskEvaluationRequest<'a> {
    pub proposal: &'a ValidatedDecisionProposal,
    pub context: RiskEvaluationContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiskEvaluationContext {
    pub market_session_open: bool,
    pub unresolved_journal_commands: bool,
    pub panic_stop_active: bool,
    pub available_buying_power: Option<Decimal>,
    pub current_position_quantity: Quantity,
    pub duplicate_order_intent: bool,
    pub projected_position_notional: Option<Decimal>,
    pub submitted_order_count_today: u32,
    pub realized_loss_today: DailyLossState,
}

impl Default for RiskEvaluationContext {
    fn default() -> Self {
        Self {
            market_session_open: true,
            unresolved_journal_commands: false,
            panic_stop_active: false,
            available_buying_power: None,
            current_position_quantity: Quantity::new(0),
            duplicate_order_intent: false,
            projected_position_notional: None,
            submitted_order_count_today: 0,
            realized_loss_today: DailyLossState::Unavailable(
                DailyLossUnavailableReason::NotComputed,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskEvaluationStatus {
    Approved,
    Blocked,
}

impl RiskEvaluationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Blocked => "blocked",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "approved" => Some(Self::Approved),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskEvaluation {
    pub status: RiskEvaluationStatus,
    pub reasons: Vec<RiskBlockReason>,
    pub order_notional: Option<Decimal>,
}

impl RiskEvaluation {
    pub fn approved(order_notional: Option<Decimal>) -> Self {
        Self {
            status: RiskEvaluationStatus::Approved,
            reasons: Vec::new(),
            order_notional,
        }
    }

    pub fn blocked(reasons: Vec<RiskBlockReason>, order_notional: Option<Decimal>) -> Self {
        Self {
            status: RiskEvaluationStatus::Blocked,
            reasons,
            order_notional,
        }
    }

    pub fn is_approved(&self) -> bool {
        self.status == RiskEvaluationStatus::Approved
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskBlockReason {
    ReadOnlyMode,
    LiveTradingDisabled,
    MarketClosed,
    UnresolvedJournalCommands,
    PanicStopActive,
    InsufficientBuyingPower {
        required: Decimal,
        available: Decimal,
    },
    SellQuantityExceedsPosition {
        requested: Quantity,
        available: Quantity,
    },
    DuplicateOrderIntent,
    SymbolNotAllowed {
        symbol: String,
    },
    MarketOrderRequiresReferencePrice,
    MaxOrderNotionalExceeded {
        notional: Decimal,
        max_order_notional: Decimal,
    },
    MaxPositionNotionalExceeded {
        projected_notional: Decimal,
        max_position_notional: Decimal,
    },
    MaxDailyOrderCountExceeded {
        submitted_orders: u32,
        max_daily_orders: u32,
    },
    MaxDailyLossExceeded {
        realized_loss: Decimal,
        max_daily_loss: Decimal,
    },
    DailyLossUnavailable {
        reason: DailyLossUnavailableReason,
    },
}

impl RiskBlockReason {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ReadOnlyMode => "read_only_mode",
            Self::LiveTradingDisabled => "live_trading_disabled",
            Self::MarketClosed => "market_closed",
            Self::UnresolvedJournalCommands => "unresolved_journal_commands",
            Self::PanicStopActive => "panic_stop_active",
            Self::InsufficientBuyingPower { .. } => "insufficient_buying_power",
            Self::SellQuantityExceedsPosition { .. } => "sell_quantity_exceeds_position",
            Self::DuplicateOrderIntent => "duplicate_order_intent",
            Self::SymbolNotAllowed { .. } => "symbol_not_allowed",
            Self::MarketOrderRequiresReferencePrice => "market_order_requires_reference_price",
            Self::MaxOrderNotionalExceeded { .. } => "max_order_notional_exceeded",
            Self::MaxPositionNotionalExceeded { .. } => "max_position_notional_exceeded",
            Self::MaxDailyOrderCountExceeded { .. } => "max_daily_order_count_exceeded",
            Self::MaxDailyLossExceeded { .. } => "max_daily_loss_exceeded",
            Self::DailyLossUnavailable { .. } => "daily_loss_unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyLossState {
    Known(Decimal),
    Unavailable(DailyLossUnavailableReason),
}

impl DailyLossState {
    pub fn from_account_and_fills(account: &AccountSnapshot, fills: &[Fill]) -> Self {
        Self::from_account_fills_and_cost_basis(account, fills, &[])
    }

    pub fn from_account_fills_and_cost_basis(
        account: &AccountSnapshot,
        fills: &[Fill],
        supplemental_cost_basis: &[(Symbol, Decimal)],
    ) -> Self {
        let mut realized_loss = Decimal::ZERO;
        for fill in fills {
            let Some(evidence) = fill.evidence.as_ref() else {
                return Self::Unavailable(DailyLossUnavailableReason::MissingFillEvidence);
            };
            let Some(side) = evidence.side else {
                return Self::Unavailable(DailyLossUnavailableReason::MissingFillSide);
            };
            if side != OrderSide::Sell {
                continue;
            }
            let Some(average_price) =
                cost_basis_for_symbol(account, supplemental_cost_basis, &evidence.symbol)
            else {
                return Self::Unavailable(DailyLossUnavailableReason::MissingCostBasis);
            };
            if fill.price < average_price {
                realized_loss += (average_price - fill.price) * Decimal::from(fill.quantity.get());
            }
        }
        Self::Known(realized_loss)
    }
}

pub fn missing_sell_cost_basis_symbols(account: &AccountSnapshot, fills: &[Fill]) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for fill in fills {
        let Some(evidence) = fill.evidence.as_ref() else {
            continue;
        };
        if evidence.side != Some(OrderSide::Sell) {
            continue;
        }
        if account
            .positions
            .iter()
            .any(|position| position.symbol == evidence.symbol)
        {
            continue;
        }
        if !symbols.iter().any(|symbol| symbol == &evidence.symbol) {
            symbols.push(evidence.symbol.clone());
        }
    }
    symbols
}

fn cost_basis_for_symbol(
    account: &AccountSnapshot,
    supplemental_cost_basis: &[(Symbol, Decimal)],
    symbol: &Symbol,
) -> Option<Decimal> {
    account
        .positions
        .iter()
        .find(|position| position.symbol == *symbol)
        .map(|position| position.average_price)
        .or_else(|| {
            supplemental_cost_basis
                .iter()
                .find(|(stored_symbol, _)| stored_symbol == symbol)
                .map(|(_, average_price)| *average_price)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyLossUnavailableReason {
    NotComputed,
    MissingFillEvidence,
    MissingFillSide,
    MissingCostBasis,
}

impl DailyLossUnavailableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotComputed => "not_computed",
            Self::MissingFillEvidence => "missing_fill_evidence",
            Self::MissingFillSide => "missing_fill_side",
            Self::MissingCostBasis => "missing_cost_basis",
        }
    }
}
