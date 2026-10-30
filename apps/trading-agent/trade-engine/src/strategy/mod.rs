use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::gateway::types::{AccountSnapshot, Quote, Symbol};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StrategyId(String);

impl StrategyId {
    pub fn new(value: impl Into<String>) -> Result<Self, StrategyError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(StrategyError::EmptyStrategyId);
        }
        if value.chars().any(char::is_whitespace) {
            return Err(StrategyError::InvalidStrategyId { strategy_id: value });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategySignalAction {
    Buy,
    Sell,
    Hold,
}

impl StrategySignalAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Hold => "hold",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategySignal {
    pub strategy_id: StrategyId,
    pub symbol: Symbol,
    pub action: StrategySignalAction,
    #[serde(with = "rust_decimal::serde::str")]
    pub strength: Decimal,
}

#[derive(Debug, Clone, Copy)]
pub struct StrategyInput<'a> {
    pub account: &'a AccountSnapshot,
    pub quote: &'a Quote,
    pub recent_prices: &'a [Decimal],
}

pub trait TradingStrategy {
    fn id(&self) -> &StrategyId;
    fn generate_signal(&self, input: &StrategyInput<'_>) -> StrategySignal;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldOnlyStrategy {
    strategy_id: StrategyId,
}

impl HoldOnlyStrategy {
    pub fn new(strategy_id: impl Into<String>) -> Result<Self, StrategyError> {
        Ok(Self {
            strategy_id: StrategyId::new(strategy_id)?,
        })
    }

    pub fn generate_signal(&self, symbol: Symbol) -> StrategySignal {
        StrategySignal {
            strategy_id: self.strategy_id.clone(),
            symbol,
            action: StrategySignalAction::Hold,
            strength: Decimal::ZERO,
        }
    }
}

impl TradingStrategy for HoldOnlyStrategy {
    fn id(&self) -> &StrategyId {
        &self.strategy_id
    }

    fn generate_signal(&self, input: &StrategyInput<'_>) -> StrategySignal {
        self.generate_signal(input.quote.symbol.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmaCrossoverStrategy {
    strategy_id: StrategyId,
    short_window: usize,
    long_window: usize,
}

impl SmaCrossoverStrategy {
    pub fn new(
        strategy_id: impl Into<String>,
        short_window: usize,
        long_window: usize,
    ) -> Result<Self, StrategyError> {
        if short_window == 0 {
            return Err(StrategyError::InvalidWindow {
                reason: "short window must be greater than zero".to_owned(),
            });
        }
        if long_window <= short_window {
            return Err(StrategyError::InvalidWindow {
                reason: "long window must be greater than short window".to_owned(),
            });
        }
        Ok(Self {
            strategy_id: StrategyId::new(strategy_id)?,
            short_window,
            long_window,
        })
    }

    pub fn prior_price_count(&self) -> usize {
        self.long_window
    }
}

impl TradingStrategy for SmaCrossoverStrategy {
    fn id(&self) -> &StrategyId {
        &self.strategy_id
    }

    fn generate_signal(&self, input: &StrategyInput<'_>) -> StrategySignal {
        if input.recent_prices.len() <= self.long_window {
            return self.hold(input);
        }

        let previous_prices = &input.recent_prices[..input.recent_prices.len() - 1];
        let Some(previous_short) = moving_average(previous_prices, self.short_window) else {
            return self.hold(input);
        };
        let Some(previous_long) = moving_average(previous_prices, self.long_window) else {
            return self.hold(input);
        };
        let Some(current_short) = moving_average(input.recent_prices, self.short_window) else {
            return self.hold(input);
        };
        let Some(current_long) = moving_average(input.recent_prices, self.long_window) else {
            return self.hold(input);
        };

        let strength = crossover_strength(current_short, current_long);
        if previous_short <= previous_long && current_short > current_long {
            return StrategySignal {
                strategy_id: self.strategy_id.clone(),
                symbol: input.quote.symbol.clone(),
                action: StrategySignalAction::Buy,
                strength,
            };
        }
        if previous_short >= previous_long
            && current_short < current_long
            && has_position(input.account, &input.quote.symbol)
        {
            return StrategySignal {
                strategy_id: self.strategy_id.clone(),
                symbol: input.quote.symbol.clone(),
                action: StrategySignalAction::Sell,
                strength,
            };
        }

        self.hold(input)
    }
}

impl SmaCrossoverStrategy {
    fn hold(&self, input: &StrategyInput<'_>) -> StrategySignal {
        StrategySignal {
            strategy_id: self.strategy_id.clone(),
            symbol: input.quote.symbol.clone(),
            action: StrategySignalAction::Hold,
            strength: Decimal::ZERO,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StrategyError {
    #[error("strategy id must not be empty")]
    EmptyStrategyId,
    #[error("strategy id must not contain whitespace: {strategy_id}")]
    InvalidStrategyId { strategy_id: String },
    #[error("invalid strategy window: {reason}")]
    InvalidWindow { reason: String },
}

fn moving_average(prices: &[Decimal], window: usize) -> Option<Decimal> {
    if window == 0 || prices.len() < window {
        return None;
    }
    let sum = prices[prices.len() - window..]
        .iter()
        .copied()
        .sum::<Decimal>();
    Some(sum / Decimal::from(window))
}

fn crossover_strength(short_average: Decimal, long_average: Decimal) -> Decimal {
    if long_average <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let difference = (short_average - long_average).abs() / long_average;
    difference.min(Decimal::ONE)
}

fn has_position(account: &AccountSnapshot, symbol: &Symbol) -> bool {
    account
        .positions
        .iter()
        .any(|position| position.symbol == *symbol && !position.quantity.is_zero())
}
