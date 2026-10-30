use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::gateway::types::{
    AccountSnapshot, CashBalance, Position, Quantity, Quote, Symbol, TradingDay,
};
use crate::strategy::{
    SmaCrossoverStrategy, StrategyId, StrategySignal, StrategySignalAction, TradingStrategy,
};
use crate::technical_analysis::DailyOhlcvBar;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub strategy_id: StrategyId,
    pub short_window: usize,
    pub long_window: usize,
    #[serde(with = "rust_decimal::serde::str")]
    pub initial_cash: Decimal,
    pub quantity_per_trade: Quantity,
}

impl BacktestConfig {
    pub fn sma_crossover(
        strategy_id: impl Into<String>,
        short_window: usize,
        long_window: usize,
        initial_cash: Decimal,
        quantity_per_trade: Quantity,
    ) -> Result<Self, BacktestError> {
        let strategy_id =
            StrategyId::new(strategy_id).map_err(|error| BacktestError::InvalidConfig {
                reason: error.to_string(),
            })?;
        SmaCrossoverStrategy::new(strategy_id.as_str().to_owned(), short_window, long_window)
            .map_err(|error| BacktestError::InvalidConfig {
                reason: error.to_string(),
            })?;
        if initial_cash <= Decimal::ZERO {
            return Err(BacktestError::InvalidConfig {
                reason: "initial cash must be greater than zero".to_owned(),
            });
        }
        if quantity_per_trade.is_zero() {
            return Err(BacktestError::InvalidConfig {
                reason: "quantity per trade must be greater than zero".to_owned(),
            });
        }
        Ok(Self {
            strategy_id,
            short_window,
            long_window,
            initial_cash,
            quantity_per_trade,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestReport {
    pub symbol: Symbol,
    pub start_day: TradingDay,
    pub end_day: TradingDay,
    pub config: BacktestConfig,
    pub bars: Vec<BacktestBarResult>,
    pub metrics: BacktestMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestBarResult {
    pub trading_day: TradingDay,
    #[serde(with = "rust_decimal::serde::str")]
    pub close: Decimal,
    pub signal: StrategySignal,
    pub trade: Option<BacktestTrade>,
    #[serde(with = "rust_decimal::serde::str")]
    pub cash: Decimal,
    pub position_quantity: Quantity,
    #[serde(with = "rust_decimal::serde::str")]
    pub position_market_value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub equity: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestTrade {
    pub side: BacktestTradeSide,
    pub quantity: Quantity,
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub notional: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BacktestTradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacktestMetrics {
    #[serde(with = "rust_decimal::serde::str")]
    pub starting_equity: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub ending_equity: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_return_pct: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub max_drawdown_pct: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    pub buy_count: u64,
    pub sell_count: u64,
    pub hold_count: u64,
    pub winning_trade_count: u64,
    pub losing_trade_count: u64,
    pub cash_blocked_buy_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BacktestError {
    #[error("backtest requires at least one OHLCV bar")]
    EmptyBars,
    #[error("backtest bars are not strictly ascending: previous={previous:?}, current={current:?}")]
    BarsNotStrictlyAscending {
        previous: TradingDay,
        current: TradingDay,
    },
    #[error("mixed symbols in backtest bars: expected={expected}, actual={actual}")]
    MixedSymbols { expected: Symbol, actual: Symbol },
    #[error("invalid backtest bar for {trading_day:?}: {reason}")]
    InvalidBar {
        trading_day: TradingDay,
        reason: String,
    },
    #[error("invalid backtest config: {reason}")]
    InvalidConfig { reason: String },
    #[error("backtest quantity overflow")]
    QuantityOverflow,
}

pub fn run_sma_crossover_backtest(
    bars: &[DailyOhlcvBar],
    config: &BacktestConfig,
) -> Result<BacktestReport, BacktestError> {
    validate_bars(bars)?;
    let symbol = bars[0].symbol.clone();
    let start_day = bars[0].trading_day;
    let end_day = bars[bars.len() - 1].trading_day;
    let strategy = SmaCrossoverStrategy::new(
        config.strategy_id.as_str().to_owned(),
        config.short_window,
        config.long_window,
    )
    .map_err(|error| BacktestError::InvalidConfig {
        reason: error.to_string(),
    })?;

    let mut cash = config.initial_cash;
    let mut position_quantity = Quantity::new(0);
    let mut position_average_price = Decimal::ZERO;
    let mut realized_pnl = Decimal::ZERO;
    let mut prices = Vec::with_capacity(bars.len());
    let mut results = Vec::with_capacity(bars.len());
    let mut metrics = BacktestMetrics {
        starting_equity: config.initial_cash,
        ending_equity: config.initial_cash,
        ..BacktestMetrics::default()
    };
    let mut peak_equity = config.initial_cash;

    for bar in bars {
        prices.push(bar.close);
        let account =
            account_snapshot(cash, &bar.symbol, position_quantity, position_average_price);
        let quote = Quote {
            symbol: bar.symbol.clone(),
            full_name: None,
            last_price: bar.close,
        };
        let signal = strategy.generate_signal(&crate::strategy::StrategyInput {
            account: &account,
            quote: &quote,
            recent_prices: &prices,
        });
        if signal.action == StrategySignalAction::Hold {
            metrics.hold_count += 1;
        }

        let mut trade = None;
        match signal.action {
            StrategySignalAction::Buy => {
                let quantity = config.quantity_per_trade;
                let notional = notional(quantity, bar.close);
                if cash >= notional {
                    let new_quantity = checked_add_quantity(position_quantity, quantity)?;
                    position_average_price = average_price_after_buy(
                        position_quantity,
                        position_average_price,
                        quantity,
                        bar.close,
                        new_quantity,
                    );
                    position_quantity = new_quantity;
                    cash -= notional;
                    metrics.buy_count += 1;
                    trade = Some(BacktestTrade {
                        side: BacktestTradeSide::Buy,
                        quantity,
                        price: bar.close,
                        notional,
                        realized_pnl: Decimal::ZERO,
                    });
                } else {
                    metrics.cash_blocked_buy_count += 1;
                }
            }
            StrategySignalAction::Sell => {
                if !position_quantity.is_zero() {
                    let quantity = position_quantity;
                    let notional = notional(quantity, bar.close);
                    let trade_pnl =
                        (bar.close - position_average_price) * Decimal::from(quantity.get());
                    cash += notional;
                    realized_pnl += trade_pnl;
                    position_quantity = Quantity::new(0);
                    position_average_price = Decimal::ZERO;
                    metrics.sell_count += 1;
                    if trade_pnl > Decimal::ZERO {
                        metrics.winning_trade_count += 1;
                    } else if trade_pnl < Decimal::ZERO {
                        metrics.losing_trade_count += 1;
                    }
                    trade = Some(BacktestTrade {
                        side: BacktestTradeSide::Sell,
                        quantity,
                        price: bar.close,
                        notional,
                        realized_pnl: trade_pnl,
                    });
                }
            }
            StrategySignalAction::Hold => {}
        }

        let position_market_value = notional(position_quantity, bar.close);
        let equity = cash + position_market_value;
        if equity > peak_equity {
            peak_equity = equity;
        }
        let drawdown_pct = percentage(peak_equity - equity, peak_equity);
        if drawdown_pct > metrics.max_drawdown_pct {
            metrics.max_drawdown_pct = drawdown_pct;
        }
        metrics.ending_equity = equity;
        metrics.realized_pnl = realized_pnl;

        results.push(BacktestBarResult {
            trading_day: bar.trading_day,
            close: bar.close,
            signal,
            trade,
            cash,
            position_quantity,
            position_market_value,
            equity,
            realized_pnl,
        });
    }

    let last_close = bars[bars.len() - 1].close;
    metrics.unrealized_pnl = if position_quantity.is_zero() {
        Decimal::ZERO
    } else {
        (last_close - position_average_price) * Decimal::from(position_quantity.get())
    };
    metrics.total_return_pct = percentage(
        metrics.ending_equity - metrics.starting_equity,
        metrics.starting_equity,
    );

    Ok(BacktestReport {
        symbol,
        start_day,
        end_day,
        config: config.clone(),
        bars: results,
        metrics,
    })
}

fn validate_bars(bars: &[DailyOhlcvBar]) -> Result<(), BacktestError> {
    let Some(first) = bars.first() else {
        return Err(BacktestError::EmptyBars);
    };
    for bar in bars {
        if bar.symbol != first.symbol {
            return Err(BacktestError::MixedSymbols {
                expected: first.symbol.clone(),
                actual: bar.symbol.clone(),
            });
        }
        validate_bar(bar)?;
    }
    for pair in bars.windows(2) {
        let previous = pair[0].trading_day;
        let current = pair[1].trading_day;
        if current.0 <= previous.0 {
            return Err(BacktestError::BarsNotStrictlyAscending { previous, current });
        }
    }
    Ok(())
}

fn validate_bar(bar: &DailyOhlcvBar) -> Result<(), BacktestError> {
    if bar.open <= Decimal::ZERO
        || bar.high <= Decimal::ZERO
        || bar.low <= Decimal::ZERO
        || bar.close <= Decimal::ZERO
    {
        return Err(invalid_bar(
            bar.trading_day,
            "open, high, low, and close must be greater than zero",
        ));
    }
    if bar.volume < Decimal::ZERO {
        return Err(invalid_bar(
            bar.trading_day,
            "volume must be greater than or equal to zero",
        ));
    }
    if bar.high < bar.low {
        return Err(invalid_bar(
            bar.trading_day,
            "high must be greater than or equal to low",
        ));
    }
    if bar.high < bar.open || bar.high < bar.close {
        return Err(invalid_bar(
            bar.trading_day,
            "high must be greater than or equal to open, low, and close",
        ));
    }
    if bar.low > bar.open || bar.low > bar.close {
        return Err(invalid_bar(
            bar.trading_day,
            "low must be less than or equal to open, high, and close",
        ));
    }
    Ok(())
}

fn invalid_bar(trading_day: TradingDay, reason: &str) -> BacktestError {
    BacktestError::InvalidBar {
        trading_day,
        reason: reason.to_owned(),
    }
}

fn account_snapshot(
    cash: Decimal,
    symbol: &Symbol,
    position_quantity: Quantity,
    position_average_price: Decimal,
) -> AccountSnapshot {
    AccountSnapshot {
        cash: CashBalance { cash },
        positions: if position_quantity.is_zero() {
            Vec::new()
        } else {
            vec![Position {
                symbol: symbol.clone(),
                quantity: position_quantity,
                average_price: position_average_price,
            }]
        },
    }
}

fn average_price_after_buy(
    current_quantity: Quantity,
    current_average_price: Decimal,
    buy_quantity: Quantity,
    buy_price: Decimal,
    new_quantity: Quantity,
) -> Decimal {
    let current_notional = current_average_price * Decimal::from(current_quantity.get());
    let buy_notional = buy_price * Decimal::from(buy_quantity.get());
    (current_notional + buy_notional) / Decimal::from(new_quantity.get())
}

fn checked_add_quantity(left: Quantity, right: Quantity) -> Result<Quantity, BacktestError> {
    left.get()
        .checked_add(right.get())
        .map(Quantity::new)
        .ok_or(BacktestError::QuantityOverflow)
}

fn notional(quantity: Quantity, price: Decimal) -> Decimal {
    Decimal::from(quantity.get()) * price
}

fn percentage(numerator: Decimal, denominator: Decimal) -> Decimal {
    if denominator <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    numerator / denominator * Decimal::from(100)
}
