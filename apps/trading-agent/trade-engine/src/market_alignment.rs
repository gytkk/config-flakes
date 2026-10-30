use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::gateway::types::Symbol;
use crate::technical_analysis::DailyTechnicalFeatures;

pub const DEFAULT_DOMESTIC_ALIGNMENT_FEATURE_SET: &str = "daily-core-v1";
pub const DEFAULT_RELATIVE_STRENGTH_LOOKBACK_DAYS: u32 = 20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomesticMarketAlignmentMode {
    #[default]
    AuditOnly,
    AdvisorContext,
}

impl DomesticMarketAlignmentMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuditOnly => "audit_only",
            Self::AdvisorContext => "advisor_context",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomesticMarketAlignmentStatus {
    Computed,
    UnknownInsufficientData,
}

impl DomesticMarketAlignmentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Computed => "computed",
            Self::UnknownInsufficientData => "unknown_insufficient_data",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomesticMarketRegime {
    StrongUptrend,
    Uptrend,
    Range,
    Downtrend,
    UnknownInsufficientData,
}

impl DomesticMarketRegime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StrongUptrend => "strong_uptrend",
            Self::Uptrend => "uptrend",
            Self::Range => "range",
            Self::Downtrend => "downtrend",
            Self::UnknownInsufficientData => "unknown_insufficient_data",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DomesticCandidateAlignmentLabel {
    Aligned,
    Weak,
    AntiIndex,
    Unknown,
}

impl DomesticCandidateAlignmentLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::Weak => "weak",
            Self::AntiIndex => "anti_index",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomesticMarketAlignmentContext {
    pub status: DomesticMarketAlignmentStatus,
    pub mode: DomesticMarketAlignmentMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_symbol: Option<Symbol>,
    pub benchmark_regime: DomesticMarketRegime,
    pub candidates: Vec<DomesticCandidateAlignment>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DomesticCandidateAlignment {
    pub symbol: Symbol,
    pub relative_strength_lookback_days: u32,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub symbol_roc: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub benchmark_roc: Option<Decimal>,
    pub alignment_label: DomesticCandidateAlignmentLabel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_breakout: Option<bool>,
    pub notes: Vec<String>,
}

pub fn compute_domestic_market_alignment(
    mode: DomesticMarketAlignmentMode,
    benchmark_symbol: Option<Symbol>,
    benchmark_features: Option<&DailyTechnicalFeatures>,
    candidate_features: &[DailyTechnicalFeatures],
    relative_strength_lookback_days: u32,
) -> DomesticMarketAlignmentContext {
    let benchmark_roc = benchmark_features.and_then(|features| features.roc);
    let mut notes = Vec::new();
    if benchmark_symbol.is_none() {
        notes.push("benchmark_symbol_not_configured".to_owned());
    }
    if benchmark_features.is_none() {
        notes.push("benchmark_features_missing".to_owned());
    } else if benchmark_roc.is_none() {
        notes.push("benchmark_roc_missing".to_owned());
    }

    let status = if benchmark_roc.is_some() {
        DomesticMarketAlignmentStatus::Computed
    } else {
        DomesticMarketAlignmentStatus::UnknownInsufficientData
    };
    let benchmark_regime = classify_regime(benchmark_roc);
    let candidates = candidate_features
        .iter()
        .map(|features| {
            candidate_alignment(features, benchmark_roc, relative_strength_lookback_days)
        })
        .collect();

    DomesticMarketAlignmentContext {
        status,
        mode,
        benchmark_symbol,
        benchmark_regime,
        candidates,
        notes,
    }
}

fn classify_regime(benchmark_roc: Option<Decimal>) -> DomesticMarketRegime {
    let Some(roc) = benchmark_roc else {
        return DomesticMarketRegime::UnknownInsufficientData;
    };
    if roc >= Decimal::new(500, 2) {
        DomesticMarketRegime::StrongUptrend
    } else if roc >= Decimal::new(200, 2) {
        DomesticMarketRegime::Uptrend
    } else if roc <= Decimal::new(-200, 2) {
        DomesticMarketRegime::Downtrend
    } else {
        DomesticMarketRegime::Range
    }
}

fn candidate_alignment(
    features: &DailyTechnicalFeatures,
    benchmark_roc: Option<Decimal>,
    relative_strength_lookback_days: u32,
) -> DomesticCandidateAlignment {
    let symbol_roc = features.roc;
    let alignment_label = match (symbol_roc, benchmark_roc) {
        (Some(symbol), Some(benchmark)) if benchmark > Decimal::ZERO && symbol < Decimal::ZERO => {
            DomesticCandidateAlignmentLabel::AntiIndex
        }
        (Some(symbol), Some(benchmark)) if symbol >= benchmark => {
            DomesticCandidateAlignmentLabel::Aligned
        }
        (Some(_), Some(_)) => DomesticCandidateAlignmentLabel::Weak,
        _ => DomesticCandidateAlignmentLabel::Unknown,
    };
    let mut notes = Vec::new();
    if symbol_roc.is_none() {
        notes.push("candidate_roc_missing".to_owned());
    }
    if benchmark_roc.is_none() {
        notes.push("benchmark_roc_missing".to_owned());
    }

    DomesticCandidateAlignment {
        symbol: features.symbol.clone(),
        relative_strength_lookback_days,
        symbol_roc,
        benchmark_roc,
        alignment_label,
        volume_breakout: features.volume_breakout,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal::Decimal;
    use time::{Date, Month};

    use crate::gateway::types::{Symbol, TradingDay};
    use crate::technical_analysis::DailyTechnicalFeatures;

    fn symbol(value: &str) -> Symbol {
        Symbol::new(value).expect("valid symbol")
    }

    fn features(
        value: &str,
        roc: Option<Decimal>,
        volume_breakout: Option<bool>,
    ) -> DailyTechnicalFeatures {
        DailyTechnicalFeatures {
            symbol: symbol(value),
            trading_day: TradingDay(
                Date::from_calendar_date(2026, Month::June, 22).expect("valid date"),
            ),
            sma_short: None,
            sma_long: None,
            ema_short: None,
            ema_long: None,
            atr: None,
            rsi: None,
            roc,
            average_volume: None,
            volume_ratio: None,
            volume_breakout,
        }
    }

    #[test]
    fn classifies_benchmark_regime_from_roc() {
        let benchmark = features("KOSPI", Some(Decimal::new(550, 2)), None);

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AdvisorContext,
            Some(symbol("KOSPI")),
            Some(&benchmark),
            &[],
            20,
        );

        assert_eq!(context.status, DomesticMarketAlignmentStatus::Computed);
        assert_eq!(
            context.benchmark_regime,
            DomesticMarketRegime::StrongUptrend
        );
        assert!(context.notes.is_empty());
    }

    #[test]
    fn classifies_benchmark_regime_threshold_boundaries() {
        let cases = [
            (Decimal::new(500, 2), DomesticMarketRegime::StrongUptrend),
            (Decimal::new(200, 2), DomesticMarketRegime::Uptrend),
            (Decimal::new(199, 2), DomesticMarketRegime::Range),
            (Decimal::new(-200, 2), DomesticMarketRegime::Downtrend),
        ];

        for (roc, expected_regime) in cases {
            let benchmark = features("KOSPI", Some(roc), None);

            let context = compute_domestic_market_alignment(
                DomesticMarketAlignmentMode::AdvisorContext,
                Some(symbol("KOSPI")),
                Some(&benchmark),
                &[],
                20,
            );

            assert_eq!(context.benchmark_regime, expected_regime);
        }
    }

    #[test]
    fn labels_candidates_against_benchmark_roc() {
        let benchmark = features("KOSPI", Some(Decimal::new(550, 2)), None);
        let candidates = vec![
            features("005930", Some(Decimal::new(800, 2)), Some(true)),
            features("000660", Some(Decimal::new(100, 2)), Some(false)),
            features("035720", Some(Decimal::new(-100, 2)), None),
            features("068270", None, None),
        ];

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AdvisorContext,
            Some(symbol("KOSPI")),
            Some(&benchmark),
            &candidates,
            20,
        );

        let labels = context
            .candidates
            .iter()
            .map(|candidate| candidate.alignment_label)
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                DomesticCandidateAlignmentLabel::Aligned,
                DomesticCandidateAlignmentLabel::Weak,
                DomesticCandidateAlignmentLabel::AntiIndex,
                DomesticCandidateAlignmentLabel::Unknown,
            ]
        );
        assert_eq!(context.candidates[0].symbol.as_str(), "005930");
        assert_eq!(context.candidates[0].symbol_roc, Some(Decimal::new(800, 2)));
        assert_eq!(
            context.candidates[0].benchmark_roc,
            Some(Decimal::new(550, 2))
        );
        assert_eq!(context.candidates[0].volume_breakout, Some(true));
    }

    #[test]
    fn fails_soft_when_benchmark_features_are_missing() {
        let candidates = vec![features("005930", Some(Decimal::new(800, 2)), Some(true))];

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AuditOnly,
            Some(symbol("KOSPI")),
            None,
            &candidates,
            20,
        );

        assert_eq!(
            context.status,
            DomesticMarketAlignmentStatus::UnknownInsufficientData
        );
        assert_eq!(
            context.benchmark_regime,
            DomesticMarketRegime::UnknownInsufficientData
        );
        assert_eq!(
            context.candidates[0].alignment_label,
            DomesticCandidateAlignmentLabel::Unknown
        );
        assert_eq!(context.notes, vec!["benchmark_features_missing"]);
    }

    #[test]
    fn reports_benchmark_roc_missing_when_benchmark_features_lack_roc() {
        let benchmark = features("KOSPI", None, None);

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AuditOnly,
            Some(symbol("KOSPI")),
            Some(&benchmark),
            &[],
            20,
        );

        assert_eq!(
            context.status,
            DomesticMarketAlignmentStatus::UnknownInsufficientData
        );
        assert_eq!(context.notes, vec!["benchmark_roc_missing"]);
    }

    #[test]
    fn reports_candidate_and_benchmark_roc_missing_independently() {
        let benchmark = features("KOSPI", None, None);
        let candidates = vec![features("005930", None, Some(true))];

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AdvisorContext,
            Some(symbol("KOSPI")),
            Some(&benchmark),
            &candidates,
            20,
        );

        assert_eq!(
            context.candidates[0].notes,
            vec!["candidate_roc_missing", "benchmark_roc_missing"]
        );
    }

    #[test]
    fn serializes_snake_case_and_decimal_strings() {
        let benchmark = features("KOSPI", Some(Decimal::new(550, 2)), None);
        let candidates = vec![features("005930", Some(Decimal::new(800, 2)), Some(true))];

        let context = compute_domestic_market_alignment(
            DomesticMarketAlignmentMode::AdvisorContext,
            Some(symbol("KOSPI")),
            Some(&benchmark),
            &candidates,
            20,
        );
        let json = serde_json::to_value(&context).expect("alignment json");

        assert_eq!(json["status"], "computed");
        assert_eq!(json["mode"], "advisor_context");
        assert_eq!(json["benchmark_regime"], "strong_uptrend");
        assert_eq!(json["candidates"][0]["symbol"], "005930");
        assert_eq!(json["candidates"][0]["symbol_roc"], "8.00");
        assert_eq!(json["candidates"][0]["benchmark_roc"], "5.50");
        assert_eq!(json["candidates"][0]["alignment_label"], "aligned");
    }
}
