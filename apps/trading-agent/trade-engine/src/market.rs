use std::time::Duration;

use time::{Date, Month, OffsetDateTime, Time, UtcOffset, Weekday};

use crate::gateway::types::{MarketScope, Symbol, TradingDay};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketProfile {
    scope: MarketScope,
}

impl MarketProfile {
    pub fn new(scope: MarketScope) -> Self {
        Self { scope }
    }

    pub fn scope(&self) -> &MarketScope {
        &self.scope
    }

    pub fn trading_day_at(&self, now_utc: OffsetDateTime) -> Date {
        now_utc.to_offset(self.local_offset_at(now_utc)).date()
    }

    pub fn trading_day_string_at(&self, now_utc: OffsetDateTime) -> String {
        format_market_date(self.trading_day_at(now_utc))
    }

    pub fn trading_day_for_fills_at(&self, now_utc: OffsetDateTime) -> TradingDay {
        TradingDay(self.trading_day_at(now_utc))
    }

    pub fn day_start_utc_at(&self, now_utc: OffsetDateTime) -> OffsetDateTime {
        let trading_day = self.trading_day_at(now_utc);
        trading_day
            .with_time(Time::MIDNIGHT)
            .assume_offset(self.local_offset_for_day(trading_day))
            .to_offset(UtcOffset::UTC)
    }

    pub fn is_regular_session_time_at(&self, now_utc: OffsetDateTime) -> bool {
        self.is_weekday_market_window_at(now_utc, self.regular_open(), self.regular_close())
    }

    pub fn auto_universe_refresh_window_open_at(&self, now_utc: OffsetDateTime) -> bool {
        self.is_weekday_market_window_at(
            now_utc,
            self.auto_universe_refresh_start(),
            self.regular_close(),
        )
    }

    pub fn next_auto_universe_refresh_delay(
        &self,
        now_utc: OffsetDateTime,
        refresh_interval: Duration,
    ) -> Duration {
        if self.auto_universe_refresh_window_open_at(now_utc) {
            return refresh_interval;
        }
        self.next_auto_universe_refresh_window_start_utc(now_utc)
            .map(|starts_at| duration_until(starts_at, now_utc))
            .unwrap_or(refresh_interval)
    }

    pub fn symbol_matches(&self, symbol: &Symbol) -> bool {
        match self.scope {
            MarketScope::DomesticStock => is_domestic_stock_symbol(symbol.as_str()),
            MarketScope::OverseasStock { .. } => is_overseas_stock_symbol(symbol.as_str()),
        }
    }

    fn is_weekday_market_window_at(
        &self,
        now_utc: OffsetDateTime,
        starts_at: Time,
        ends_at: Time,
    ) -> bool {
        let local = now_utc.to_offset(self.local_offset_at(now_utc));
        if matches!(local.date().weekday(), Weekday::Saturday | Weekday::Sunday) {
            return false;
        }
        let local_time = local.time();
        local_time >= starts_at && local_time < ends_at
    }

    fn next_auto_universe_refresh_window_start_utc(
        &self,
        now_utc: OffsetDateTime,
    ) -> Option<OffsetDateTime> {
        let mut day = now_utc.to_offset(self.local_offset_at(now_utc)).date();
        for _ in 0..10 {
            if !matches!(day.weekday(), Weekday::Saturday | Weekday::Sunday) {
                let candidate = day
                    .with_time(self.auto_universe_refresh_start())
                    .assume_offset(self.local_offset_for_day(day))
                    .to_offset(UtcOffset::UTC);
                if candidate > now_utc {
                    return Some(candidate);
                }
            }
            day = day.next_day()?;
        }
        None
    }

    fn regular_open(&self) -> Time {
        match self.scope {
            MarketScope::DomesticStock => Time::from_hms(9, 0, 0).expect("KRX open time is valid"),
            MarketScope::OverseasStock { .. } => {
                Time::from_hms(9, 30, 0).expect("US equity open time is valid")
            }
        }
    }

    fn regular_close(&self) -> Time {
        match self.scope {
            MarketScope::DomesticStock => {
                Time::from_hms(15, 30, 0).expect("KRX close time is valid")
            }
            MarketScope::OverseasStock { .. } => {
                Time::from_hms(16, 0, 0).expect("US equity close time is valid")
            }
        }
    }

    fn auto_universe_refresh_start(&self) -> Time {
        match self.scope {
            MarketScope::DomesticStock => {
                Time::from_hms(8, 30, 0).expect("KRX preopen refresh time is valid")
            }
            MarketScope::OverseasStock { .. } => {
                Time::from_hms(9, 0, 0).expect("US equity preopen refresh time is valid")
            }
        }
    }

    fn local_offset_at(&self, now_utc: OffsetDateTime) -> UtcOffset {
        match self.scope {
            MarketScope::DomesticStock => kst_offset(),
            MarketScope::OverseasStock { .. } => us_eastern_offset_at(now_utc),
        }
    }

    fn local_offset_for_day(&self, day: Date) -> UtcOffset {
        match self.scope {
            MarketScope::DomesticStock => kst_offset(),
            MarketScope::OverseasStock { .. } => us_eastern_offset_for_market_day(day),
        }
    }
}

pub fn format_market_date(day: Date) -> String {
    format!(
        "{:04}{:02}{:02}",
        day.year(),
        u8::from(day.month()),
        day.day()
    )
}

fn duration_until(target_utc: OffsetDateTime, now_utc: OffsetDateTime) -> Duration {
    let delta = target_utc - now_utc;
    if delta <= time::Duration::ZERO {
        return Duration::ZERO;
    }
    Duration::from_secs(delta.whole_seconds().try_into().unwrap_or(u64::MAX))
}

fn is_domestic_stock_symbol(value: &str) -> bool {
    value.len() == 6 && value.chars().all(|character| character.is_ascii_digit())
}

fn is_overseas_stock_symbol(value: &str) -> bool {
    !is_domestic_stock_symbol(value)
        && (1..=10).contains(&value.len())
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
        && value.chars().all(|character| {
            character.is_ascii_uppercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '/')
        })
}

fn kst_offset() -> UtcOffset {
    UtcOffset::from_hms(9, 0, 0).expect("KST offset is valid")
}

fn us_eastern_offset_at(now_utc: OffsetDateTime) -> UtcOffset {
    if is_us_eastern_dst_utc(now_utc) {
        UtcOffset::from_hms(-4, 0, 0).expect("US Eastern daylight offset is valid")
    } else {
        UtcOffset::from_hms(-5, 0, 0).expect("US Eastern standard offset is valid")
    }
}

fn is_us_eastern_dst_utc(now_utc: OffsetDateTime) -> bool {
    let year = now_utc.year();
    let dst_start = us_dst_transition_utc(
        year,
        Month::March,
        2,
        Time::from_hms(7, 0, 0).expect("US DST start UTC transition time is valid"),
    );
    let dst_end = us_dst_transition_utc(
        year,
        Month::November,
        1,
        Time::from_hms(6, 0, 0).expect("US DST end UTC transition time is valid"),
    );
    now_utc >= dst_start && now_utc < dst_end
}

fn us_eastern_offset_for_market_day(day: Date) -> UtcOffset {
    let dst_start = nth_weekday_of_month(day.year(), Month::March, Weekday::Sunday, 2);
    let dst_end = nth_weekday_of_month(day.year(), Month::November, Weekday::Sunday, 1);
    if day >= dst_start && day < dst_end {
        UtcOffset::from_hms(-4, 0, 0).expect("US Eastern daylight offset is valid")
    } else {
        UtcOffset::from_hms(-5, 0, 0).expect("US Eastern standard offset is valid")
    }
}

fn us_dst_transition_utc(
    year: i32,
    month: Month,
    sunday_index: u8,
    time_utc: Time,
) -> OffsetDateTime {
    nth_weekday_of_month(year, month, Weekday::Sunday, sunday_index)
        .with_time(time_utc)
        .assume_utc()
}

fn nth_weekday_of_month(year: i32, month: Month, weekday: Weekday, index: u8) -> Date {
    let mut day = Date::from_calendar_date(year, month, 1).expect("valid month");
    while day.weekday() != weekday {
        day = day.next_day().expect("month has requested weekday");
    }
    for _ in 1..index {
        for _ in 0..7 {
            day = day.next_day().expect("weekday index exists in month");
        }
    }
    day
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use time::{Date, Month, OffsetDateTime, Time};
    use trading_agent_kis_client::{CurrencyCode, ExchangeCode};

    use crate::gateway::types::{MarketScope, Symbol};

    #[test]
    fn domestic_profile_uses_kst_trading_day_and_preopen_window() {
        let profile = MarketProfile::new(MarketScope::DomesticStock);

        assert_eq!(
            profile.trading_day_string_at(utc_datetime(2026, Month::April, 23, 15, 30, 0)),
            "20260424"
        );
        assert_eq!(
            profile.day_start_utc_at(utc_datetime(2026, Month::April, 24, 1, 0, 0)),
            utc_datetime(2026, Month::April, 23, 15, 0, 0)
        );
        assert!(!profile.auto_universe_refresh_window_open_at(utc_datetime(
            2026,
            Month::April,
            23,
            23,
            29,
            0
        )));
        assert!(profile.auto_universe_refresh_window_open_at(utc_datetime(
            2026,
            Month::April,
            23,
            23,
            30,
            0
        )));
        assert_eq!(
            profile.next_auto_universe_refresh_delay(
                utc_datetime(2026, Month::April, 23, 23, 29, 0),
                Duration::from_secs(300),
            ),
            Duration::from_secs(60)
        );
        assert!(profile.symbol_matches(&Symbol::new("005930").expect("symbol")));
        assert!(!profile.symbol_matches(&Symbol::new("AAPL").expect("symbol")));
    }

    #[test]
    fn overseas_profile_uses_us_eastern_trading_day_and_preopen_window() {
        let profile = MarketProfile::new(MarketScope::OverseasStock {
            exchange: ExchangeCode::new("NASD").expect("exchange"),
            currency: CurrencyCode::new("USD").expect("currency"),
        });

        assert_eq!(
            profile.trading_day_string_at(utc_datetime(2026, Month::April, 28, 2, 0, 0)),
            "20260427"
        );
        assert_eq!(
            profile.day_start_utc_at(utc_datetime(2026, Month::April, 27, 14, 0, 0)),
            utc_datetime(2026, Month::April, 27, 4, 0, 0)
        );
        assert!(!profile.auto_universe_refresh_window_open_at(utc_datetime(
            2026,
            Month::April,
            27,
            12,
            59,
            0
        )));
        assert!(profile.auto_universe_refresh_window_open_at(utc_datetime(
            2026,
            Month::April,
            27,
            13,
            0,
            0
        )));
        assert!(!profile.auto_universe_refresh_window_open_at(utc_datetime(
            2026,
            Month::April,
            27,
            20,
            0,
            0
        )));
        assert!(profile.symbol_matches(&Symbol::new("AAPL").expect("symbol")));
        assert!(!profile.symbol_matches(&Symbol::new("005930").expect("symbol")));
    }

    fn utc_datetime(
        year: i32,
        month: Month,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> OffsetDateTime {
        Date::from_calendar_date(year, month, day)
            .expect("valid date")
            .with_time(Time::from_hms(hour, minute, second).expect("valid time"))
            .assume_utc()
    }
}
