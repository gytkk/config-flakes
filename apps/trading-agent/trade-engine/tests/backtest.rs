use rust_decimal::Decimal;
use time::{Date, Month};
use trade_engine::backtest::{
    BacktestConfig, BacktestError, BacktestMetrics, BacktestTradeSide, run_sma_crossover_backtest,
};
use trade_engine::gateway::types::{Quantity, Symbol, TradingDay};
use trade_engine::strategy::StrategySignalAction;
use trade_engine::technical_analysis::DailyOhlcvBar;

fn symbol(value: &str) -> Symbol {
    Symbol::new(value).expect("valid symbol")
}

fn day(day: u8) -> TradingDay {
    TradingDay(Date::from_calendar_date(2026, Month::January, day).expect("valid date"))
}

fn bar(day_of_month: u8, close: i64) -> DailyOhlcvBar {
    DailyOhlcvBar {
        symbol: symbol("005930"),
        trading_day: day(day_of_month),
        open: Decimal::new(close - 1, 0),
        high: Decimal::new(close + 1, 0),
        low: Decimal::new(close - 2, 0),
        close: Decimal::new(close, 0),
        volume: Decimal::new(100, 0),
    }
}

fn config(initial_cash: i64) -> BacktestConfig {
    BacktestConfig::sma_crossover(
        "sma-test",
        2,
        3,
        Decimal::new(initial_cash, 0),
        Quantity::new(1),
    )
    .expect("valid config")
}

#[test]
fn sma_backtest_rejects_empty_bars() {
    let error =
        run_sma_crossover_backtest(&[], &config(1_000)).expect_err("empty bars are invalid");

    assert_eq!(error, BacktestError::EmptyBars);
}

#[test]
fn sma_backtest_rejects_mixed_symbols_and_unsorted_days() {
    let mut other_symbol = bar(2, 90);
    other_symbol.symbol = symbol("000660");
    let error = run_sma_crossover_backtest(&[bar(1, 100), other_symbol], &config(1_000))
        .expect_err("mixed symbols are invalid");

    assert_eq!(
        error,
        BacktestError::MixedSymbols {
            expected: symbol("005930"),
            actual: symbol("000660")
        }
    );

    let error = run_sma_crossover_backtest(&[bar(2, 90), bar(1, 100)], &config(1_000))
        .expect_err("unsorted days are invalid");

    assert_eq!(
        error,
        BacktestError::BarsNotStrictlyAscending {
            previous: day(2),
            current: day(1)
        }
    );
}

#[test]
fn sma_backtest_opens_and_closes_position_with_equity_metrics() {
    let bars = [
        bar(1, 100),
        bar(2, 90),
        bar(3, 90),
        bar(4, 130),
        bar(5, 130),
        bar(6, 70),
    ];

    let report = run_sma_crossover_backtest(&bars, &config(1_000)).expect("backtest");

    assert_eq!(report.symbol, symbol("005930"));
    assert_eq!(report.start_day, day(1));
    assert_eq!(report.end_day, day(6));
    assert_eq!(report.bars.len(), 6);
    assert_eq!(report.bars[3].signal.action, StrategySignalAction::Buy);
    assert_eq!(
        report.bars[3].trade.as_ref().map(|trade| trade.side),
        Some(BacktestTradeSide::Buy)
    );
    assert_eq!(report.bars[3].cash, Decimal::new(870, 0));
    assert_eq!(report.bars[3].equity, Decimal::new(1_000, 0));
    assert_eq!(report.bars[5].signal.action, StrategySignalAction::Sell);
    assert_eq!(
        report.bars[5].trade.as_ref().map(|trade| trade.side),
        Some(BacktestTradeSide::Sell)
    );
    assert_eq!(report.bars[5].realized_pnl, Decimal::new(-60, 0));

    assert_eq!(
        report.metrics,
        BacktestMetrics {
            starting_equity: Decimal::new(1_000, 0),
            ending_equity: Decimal::new(940, 0),
            total_return_pct: Decimal::new(-6, 0),
            max_drawdown_pct: Decimal::new(6, 0),
            realized_pnl: Decimal::new(-60, 0),
            unrealized_pnl: Decimal::ZERO,
            buy_count: 1,
            sell_count: 1,
            hold_count: 4,
            winning_trade_count: 0,
            losing_trade_count: 1,
            cash_blocked_buy_count: 0,
        }
    );
}

#[test]
fn sma_backtest_blocks_buy_when_cash_is_insufficient() {
    let bars = [bar(1, 100), bar(2, 90), bar(3, 90), bar(4, 130)];

    let report = run_sma_crossover_backtest(&bars, &config(100)).expect("backtest");

    assert_eq!(report.bars[3].signal.action, StrategySignalAction::Buy);
    assert!(report.bars[3].trade.is_none());
    assert_eq!(report.bars[3].cash, Decimal::new(100, 0));
    assert_eq!(report.metrics.buy_count, 0);
    assert_eq!(report.metrics.cash_blocked_buy_count, 1);
    assert_eq!(report.metrics.ending_equity, Decimal::new(100, 0));
}
