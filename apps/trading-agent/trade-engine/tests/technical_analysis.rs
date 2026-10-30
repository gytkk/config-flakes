use rust_decimal::Decimal;
use time::{Date, Month};
use trade_engine::gateway::types::{Symbol, TradingDay};
use trade_engine::technical_analysis::{
    DailyOhlcvBar, TechnicalAnalysisError, TechnicalFeatureParams, compute_latest_daily_features,
};

fn symbol() -> Symbol {
    Symbol::new("005930").expect("valid symbol")
}

fn day(day: u8) -> TradingDay {
    TradingDay(Date::from_calendar_date(2026, Month::January, day).expect("valid date"))
}

fn bar(day_of_month: u8, close: i64, volume: i64) -> DailyOhlcvBar {
    DailyOhlcvBar {
        symbol: symbol(),
        trading_day: day(day_of_month),
        open: Decimal::new(close - 1, 0),
        high: Decimal::new(close + 1, 0),
        low: Decimal::new(close - 2, 0),
        close: Decimal::new(close, 0),
        volume: Decimal::new(volume, 0),
    }
}

fn params() -> TechnicalFeatureParams {
    TechnicalFeatureParams {
        sma_short_window: 2,
        sma_long_window: 3,
        ema_short_window: 2,
        ema_long_window: 3,
        atr_window: 3,
        rsi_window: 3,
        roc_window: 3,
        average_volume_window: 3,
        volume_breakout_ratio: Decimal::new(5, 0),
    }
}

#[test]
fn compute_latest_returns_none_for_empty_bar_series() {
    let features = compute_latest_daily_features(&[], &params()).expect("empty series");

    assert_eq!(features, None);
}

#[test]
fn compute_latest_rejects_unsorted_or_duplicate_days() {
    let bars = [bar(2, 11, 10), bar(1, 10, 10)];

    let error = compute_latest_daily_features(&bars, &params()).expect_err("unsorted bars");

    assert_eq!(
        error,
        TechnicalAnalysisError::BarsNotStrictlyAscending {
            previous: day(2),
            current: day(1)
        }
    );

    let bars = [bar(1, 10, 10), bar(1, 11, 10)];

    let error = compute_latest_daily_features(&bars, &params()).expect_err("duplicate day");

    assert_eq!(
        error,
        TechnicalAnalysisError::BarsNotStrictlyAscending {
            previous: day(1),
            current: day(1)
        }
    );
}

#[test]
fn compute_latest_rejects_mixed_symbols() {
    let mut other = bar(2, 11, 10);
    other.symbol = Symbol::new("000660").expect("valid symbol");
    let bars = [bar(1, 10, 10), other];

    let error = compute_latest_daily_features(&bars, &params()).expect_err("mixed symbols");

    assert_eq!(
        error,
        TechnicalAnalysisError::MixedSymbols {
            expected: symbol(),
            actual: Symbol::new("000660").expect("valid symbol")
        }
    );
}

#[test]
fn compute_latest_rejects_invalid_ohlc_relationships() {
    let mut invalid = bar(1, 10, 10);
    invalid.high = Decimal::new(9, 0);

    let error = compute_latest_daily_features(&[invalid], &params()).expect_err("invalid high");

    assert_eq!(
        error,
        TechnicalAnalysisError::InvalidBar {
            trading_day: day(1),
            reason: "high must be greater than or equal to open, low, and close".to_owned()
        }
    );
}

#[test]
fn compute_latest_rejects_zero_or_negative_price_and_negative_volume() {
    let mut zero_close = bar(1, 10, 10);
    zero_close.close = Decimal::ZERO;

    let error = compute_latest_daily_features(&[zero_close], &params()).expect_err("zero close");

    assert_eq!(
        error,
        TechnicalAnalysisError::InvalidBar {
            trading_day: day(1),
            reason: "open, high, low, and close must be greater than zero".to_owned()
        }
    );

    let mut negative_volume = bar(1, 10, 10);
    negative_volume.volume = Decimal::new(-1, 0);

    let error =
        compute_latest_daily_features(&[negative_volume], &params()).expect_err("negative volume");

    assert_eq!(
        error,
        TechnicalAnalysisError::InvalidBar {
            trading_day: day(1),
            reason: "volume must be greater than or equal to zero".to_owned()
        }
    );
}

#[test]
fn compute_latest_returns_partial_features_when_history_is_short() {
    let features = compute_latest_daily_features(&[bar(1, 10, 10)], &params())
        .expect("short history")
        .expect("latest features");

    assert_eq!(features.symbol, symbol());
    assert_eq!(features.trading_day, day(1));
    assert_eq!(features.sma_short, None);
    assert_eq!(features.sma_long, None);
    assert_eq!(features.ema_short, None);
    assert_eq!(features.ema_long, None);
    assert_eq!(features.atr, None);
    assert_eq!(features.rsi, None);
    assert_eq!(features.roc, None);
    assert_eq!(features.average_volume, None);
    assert_eq!(features.volume_ratio, None);
    assert_eq!(features.volume_breakout, None);
}

#[test]
fn compute_latest_calculates_moving_average_momentum_and_volume_features() {
    let bars = [
        bar(1, 10, 10),
        bar(2, 20, 20),
        bar(3, 30, 30),
        bar(4, 40, 100),
    ];

    let features = compute_latest_daily_features(&bars, &params())
        .expect("feature calculation")
        .expect("latest features");

    assert_eq!(features.sma_short, Some(Decimal::new(35, 0)));
    assert_eq!(features.sma_long, Some(Decimal::new(30, 0)));
    assert_eq!(features.ema_long, Some(Decimal::new(30, 0)));
    assert_eq!(features.rsi, Some(Decimal::new(100, 0)));
    assert_eq!(features.roc, Some(Decimal::new(300, 0)));
    assert_eq!(features.average_volume, Some(Decimal::new(20, 0)));
    assert_eq!(features.volume_ratio, Some(Decimal::new(5, 0)));
    assert_eq!(features.volume_breakout, Some(true));
}

#[test]
fn compute_latest_ema_seed_equals_sma_when_history_equals_window() {
    let bars = [bar(1, 10, 10), bar(2, 20, 20), bar(3, 30, 30)];

    let features = compute_latest_daily_features(&bars, &params())
        .expect("feature calculation")
        .expect("latest features");

    assert_eq!(features.ema_long, Some(Decimal::new(20, 0)));
}

#[test]
fn compute_latest_uses_gap_aware_true_range_and_wilder_atr() {
    let bars = [
        DailyOhlcvBar {
            symbol: symbol(),
            trading_day: day(1),
            open: Decimal::new(10, 0),
            high: Decimal::new(12, 0),
            low: Decimal::new(9, 0),
            close: Decimal::new(11, 0),
            volume: Decimal::new(10, 0),
        },
        DailyOhlcvBar {
            symbol: symbol(),
            trading_day: day(2),
            open: Decimal::new(13, 0),
            high: Decimal::new(14, 0),
            low: Decimal::new(12, 0),
            close: Decimal::new(13, 0),
            volume: Decimal::new(10, 0),
        },
        DailyOhlcvBar {
            symbol: symbol(),
            trading_day: day(3),
            open: Decimal::new(14, 0),
            high: Decimal::new(16, 0),
            low: Decimal::new(14, 0),
            close: Decimal::new(15, 0),
            volume: Decimal::new(10, 0),
        },
        DailyOhlcvBar {
            symbol: symbol(),
            trading_day: day(4),
            open: Decimal::new(18, 0),
            high: Decimal::new(21, 0),
            low: Decimal::new(18, 0),
            close: Decimal::new(20, 0),
            volume: Decimal::new(10, 0),
        },
    ];

    let features = compute_latest_daily_features(&bars, &params())
        .expect("feature calculation")
        .expect("latest features");

    assert_eq!(features.atr, Some(Decimal::new(4, 0)));
}

#[test]
fn compute_latest_rsi_handles_flat_and_all_loss_windows() {
    let flat = [
        bar(1, 10, 10),
        bar(2, 10, 10),
        bar(3, 10, 10),
        bar(4, 10, 10),
    ];
    let flat_features = compute_latest_daily_features(&flat, &params())
        .expect("flat rsi")
        .expect("latest features");
    assert_eq!(flat_features.rsi, Some(Decimal::new(50, 0)));

    let losses = [
        bar(1, 40, 10),
        bar(2, 30, 10),
        bar(3, 20, 10),
        bar(4, 10, 10),
    ];
    let loss_features = compute_latest_daily_features(&losses, &params())
        .expect("loss rsi")
        .expect("latest features");
    assert_eq!(loss_features.rsi, Some(Decimal::ZERO));
}

#[test]
fn compute_latest_volume_ratio_is_absent_when_prior_average_volume_is_zero() {
    let bars = [bar(1, 10, 0), bar(2, 11, 0), bar(3, 12, 0), bar(4, 13, 100)];

    let features = compute_latest_daily_features(&bars, &params())
        .expect("feature calculation")
        .expect("latest features");

    assert_eq!(features.average_volume, Some(Decimal::ZERO));
    assert_eq!(features.volume_ratio, None);
    assert_eq!(features.volume_breakout, Some(false));
}

#[test]
fn compute_latest_rejects_invalid_feature_params() {
    let mut params = params();
    params.sma_short_window = 0;

    let error =
        compute_latest_daily_features(&[bar(1, 10, 10)], &params).expect_err("invalid params");

    assert_eq!(
        error,
        TechnicalAnalysisError::InvalidWindow {
            field: "sma_short_window",
            reason: "window must be greater than zero".to_owned()
        }
    );
}
