use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::gateway::types::{Symbol, TradingDay};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OhlcvBarInterval {
    Daily,
}

impl OhlcvBarInterval {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "daily" => Some(Self::Daily),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceAdjustmentPolicy {
    Raw,
    Adjusted,
}

impl PriceAdjustmentPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Adjusted => "adjusted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyOhlcvBar {
    pub symbol: Symbol,
    #[serde(with = "trading_day_serde")]
    pub trading_day: TradingDay,
    #[serde(with = "rust_decimal::serde::str")]
    pub open: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub high: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub low: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub close: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TechnicalFeatureParams {
    pub sma_short_window: usize,
    pub sma_long_window: usize,
    pub ema_short_window: usize,
    pub ema_long_window: usize,
    pub atr_window: usize,
    pub rsi_window: usize,
    pub roc_window: usize,
    pub average_volume_window: usize,
    #[serde(with = "rust_decimal::serde::str")]
    pub volume_breakout_ratio: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTechnicalFeatures {
    pub symbol: Symbol,
    #[serde(with = "trading_day_serde")]
    pub trading_day: TradingDay,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sma_short: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub sma_long: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub ema_short: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub ema_long: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub atr: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub rsi: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub roc: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub average_volume: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub volume_ratio: Option<Decimal>,
    pub volume_breakout: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TechnicalAnalysisError {
    #[error("invalid {field}: {reason}")]
    InvalidWindow { field: &'static str, reason: String },
    #[error("bars are not strictly ascending: previous={previous:?}, current={current:?}")]
    BarsNotStrictlyAscending {
        previous: TradingDay,
        current: TradingDay,
    },
    #[error("mixed symbols in bar series: expected={expected}, actual={actual}")]
    MixedSymbols { expected: Symbol, actual: Symbol },
    #[error("invalid bar for {trading_day:?}: {reason}")]
    InvalidBar {
        trading_day: TradingDay,
        reason: String,
    },
}

pub fn compute_latest_daily_features(
    bars: &[DailyOhlcvBar],
    params: &TechnicalFeatureParams,
) -> Result<Option<DailyTechnicalFeatures>, TechnicalAnalysisError> {
    validate_params(params)?;
    let Some(latest) = bars.last() else {
        return Ok(None);
    };
    validate_bars(bars)?;

    let average_volume = average_volume(bars, params.average_volume_window);
    let (volume_ratio, volume_breakout) =
        volume_features(latest.volume, average_volume, params.volume_breakout_ratio);

    Ok(Some(DailyTechnicalFeatures {
        symbol: latest.symbol.clone(),
        trading_day: latest.trading_day,
        sma_short: simple_moving_average(bars, params.sma_short_window),
        sma_long: simple_moving_average(bars, params.sma_long_window),
        ema_short: exponential_moving_average(bars, params.ema_short_window),
        ema_long: exponential_moving_average(bars, params.ema_long_window),
        atr: average_true_range(bars, params.atr_window),
        rsi: relative_strength_index(bars, params.rsi_window),
        roc: rate_of_change(bars, params.roc_window),
        average_volume,
        volume_ratio,
        volume_breakout,
    }))
}

fn validate_params(params: &TechnicalFeatureParams) -> Result<(), TechnicalAnalysisError> {
    for (field, window) in [
        ("sma_short_window", params.sma_short_window),
        ("sma_long_window", params.sma_long_window),
        ("ema_short_window", params.ema_short_window),
        ("ema_long_window", params.ema_long_window),
        ("atr_window", params.atr_window),
        ("rsi_window", params.rsi_window),
        ("roc_window", params.roc_window),
        ("average_volume_window", params.average_volume_window),
    ] {
        if window == 0 {
            return Err(TechnicalAnalysisError::InvalidWindow {
                field,
                reason: "window must be greater than zero".to_owned(),
            });
        }
    }
    if params.sma_long_window <= params.sma_short_window {
        return Err(TechnicalAnalysisError::InvalidWindow {
            field: "sma_long_window",
            reason: "long window must be greater than short window".to_owned(),
        });
    }
    if params.ema_long_window <= params.ema_short_window {
        return Err(TechnicalAnalysisError::InvalidWindow {
            field: "ema_long_window",
            reason: "long window must be greater than short window".to_owned(),
        });
    }
    if params.volume_breakout_ratio <= Decimal::ZERO {
        return Err(TechnicalAnalysisError::InvalidWindow {
            field: "volume_breakout_ratio",
            reason: "ratio must be greater than zero".to_owned(),
        });
    }
    Ok(())
}

fn validate_bars(bars: &[DailyOhlcvBar]) -> Result<(), TechnicalAnalysisError> {
    let expected_symbol = &bars[0].symbol;
    for bar in bars {
        if bar.symbol != *expected_symbol {
            return Err(TechnicalAnalysisError::MixedSymbols {
                expected: expected_symbol.clone(),
                actual: bar.symbol.clone(),
            });
        }
        validate_bar(bar)?;
    }
    for pair in bars.windows(2) {
        let previous = pair[0].trading_day;
        let current = pair[1].trading_day;
        if current.0 <= previous.0 {
            return Err(TechnicalAnalysisError::BarsNotStrictlyAscending { previous, current });
        }
    }
    Ok(())
}

fn validate_bar(bar: &DailyOhlcvBar) -> Result<(), TechnicalAnalysisError> {
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

fn invalid_bar(trading_day: TradingDay, reason: &str) -> TechnicalAnalysisError {
    TechnicalAnalysisError::InvalidBar {
        trading_day,
        reason: reason.to_owned(),
    }
}

fn simple_moving_average(bars: &[DailyOhlcvBar], window: usize) -> Option<Decimal> {
    if bars.len() < window {
        return None;
    }
    let sum = bars[bars.len() - window..]
        .iter()
        .map(|bar| bar.close)
        .sum::<Decimal>();
    Some(sum / window_decimal(window))
}

fn exponential_moving_average(bars: &[DailyOhlcvBar], window: usize) -> Option<Decimal> {
    if bars.len() < window {
        return None;
    }
    let mut average =
        bars[..window].iter().map(|bar| bar.close).sum::<Decimal>() / window_decimal(window);
    let multiplier = Decimal::new(2, 0) / window_decimal(window + 1);
    for bar in &bars[window..] {
        average = bar.close * multiplier + average * (Decimal::ONE - multiplier);
    }
    Some(average)
}

fn average_true_range(bars: &[DailyOhlcvBar], window: usize) -> Option<Decimal> {
    if bars.len() < window {
        return None;
    }
    let true_ranges = true_ranges(bars);
    let mut average =
        true_ranges[..window].iter().copied().sum::<Decimal>() / window_decimal(window);
    let smoothing_count = window_decimal(window - 1);
    let divisor = window_decimal(window);
    for true_range in &true_ranges[window..] {
        average = (average * smoothing_count + *true_range) / divisor;
    }
    Some(average)
}

fn true_ranges(bars: &[DailyOhlcvBar]) -> Vec<Decimal> {
    let mut ranges = Vec::with_capacity(bars.len());
    for (index, bar) in bars.iter().enumerate() {
        if index == 0 {
            ranges.push(bar.high - bar.low);
            continue;
        }
        let previous_close = bars[index - 1].close;
        ranges.push(max_decimal(
            bar.high - bar.low,
            max_decimal(
                (bar.high - previous_close).abs(),
                (bar.low - previous_close).abs(),
            ),
        ));
    }
    ranges
}

fn relative_strength_index(bars: &[DailyOhlcvBar], window: usize) -> Option<Decimal> {
    if bars.len() <= window {
        return None;
    }
    let divisor = window_decimal(window);
    let mut average_gain = Decimal::ZERO;
    let mut average_loss = Decimal::ZERO;

    for index in 1..=window {
        let (gain, loss) = gain_loss(bars[index].close - bars[index - 1].close);
        average_gain += gain;
        average_loss += loss;
    }
    average_gain /= divisor;
    average_loss /= divisor;

    let smoothing_count = window_decimal(window - 1);
    for index in window + 1..bars.len() {
        let (gain, loss) = gain_loss(bars[index].close - bars[index - 1].close);
        average_gain = (average_gain * smoothing_count + gain) / divisor;
        average_loss = (average_loss * smoothing_count + loss) / divisor;
    }

    Some(rsi_from_averages(average_gain, average_loss))
}

fn gain_loss(delta: Decimal) -> (Decimal, Decimal) {
    if delta > Decimal::ZERO {
        (delta, Decimal::ZERO)
    } else {
        (Decimal::ZERO, -delta)
    }
}

fn rsi_from_averages(average_gain: Decimal, average_loss: Decimal) -> Decimal {
    if average_loss.is_zero() && average_gain.is_zero() {
        return Decimal::new(50, 0);
    }
    if average_loss.is_zero() {
        return Decimal::new(100, 0);
    }
    if average_gain.is_zero() {
        return Decimal::ZERO;
    }
    let relative_strength = average_gain / average_loss;
    Decimal::new(100, 0) - (Decimal::new(100, 0) / (Decimal::ONE + relative_strength))
}

fn rate_of_change(bars: &[DailyOhlcvBar], window: usize) -> Option<Decimal> {
    if bars.len() <= window {
        return None;
    }
    let latest_close = bars.last()?.close;
    let prior_close = bars[bars.len() - 1 - window].close;
    Some(((latest_close - prior_close) / prior_close) * Decimal::new(100, 0))
}

fn average_volume(bars: &[DailyOhlcvBar], window: usize) -> Option<Decimal> {
    if bars.len() <= window {
        return None;
    }
    let latest_index = bars.len() - 1;
    let sum = bars[latest_index - window..latest_index]
        .iter()
        .map(|bar| bar.volume)
        .sum::<Decimal>();
    Some(sum / window_decimal(window))
}

fn volume_features(
    latest_volume: Decimal,
    average_volume: Option<Decimal>,
    breakout_ratio: Decimal,
) -> (Option<Decimal>, Option<bool>) {
    let Some(average_volume) = average_volume else {
        return (None, None);
    };
    if average_volume.is_zero() {
        return (None, Some(false));
    }
    let ratio = latest_volume / average_volume;
    (Some(ratio), Some(ratio >= breakout_ratio))
}

fn max_decimal(left: Decimal, right: Decimal) -> Decimal {
    if left >= right { left } else { right }
}

fn window_decimal(window: usize) -> Decimal {
    Decimal::from(window as u64)
}

mod trading_day_serde {
    use super::*;
    use serde::de::Error as DeError;

    pub fn serialize<S>(value: &TradingDay, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.0.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<TradingDay, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let format =
            time::format_description::parse("[year]-[month]-[day]").map_err(D::Error::custom)?;
        let date = time::Date::parse(&value, &format).map_err(D::Error::custom)?;
        Ok(TradingDay(date))
    }
}
