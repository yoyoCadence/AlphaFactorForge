//! DATA-QUALITY-001 — market-data admission contract (Rust side).
//!
//! Exact mirror of `src/core/market-data/quality.ts`; the two are held together
//! by `fixtures/rs-core/market-data-quality-v1.json`.
//!
//! Dataset identity proves only that bytes were not altered, never that they
//! describe a possible market. This module is that missing semantic gate. It is
//! deliberately NOT part of the identity path — nothing here may be called from
//! `crate::identity`, and the dataset hash preimage is unchanged.
//!
//! The entry point is field-level so `db::Candle` callers can validate straight
//! out of a row iterator without allocating a converted vector.

use chrono::{TimeZone, Utc};

use super::types::Candle;

pub const MARKET_DATA_QUALITY_VERSION: &str = "market-data-quality-v1";

/// Adjudicated product plausibility boundary (planning decision 1), NOT a
/// language limit: 2000-01-01T00:00:00Z inclusive to 2100-01-01T00:00:00Z
/// exclusive. Identical decimal values to the TypeScript side.
pub const MIN_MARKET_TIMESTAMP_MS: i64 = 946_684_800_000;
pub const MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE: i64 = 4_102_444_800_000;

/// `Number.MAX_SAFE_INTEGER`. Rule 1 is "not a safe integer" in TypeScript, so
/// Rust must reject the same magnitudes to classify identically.
const JS_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Stable rule ids, evaluated in exactly this order.
pub const MARKET_DATA_RULE_IDS: [&str; 7] = [
    "timestamp_not_integer",
    "timestamp_out_of_range",
    "timestamp_not_representable",
    "price_not_positive",
    "volume_negative",
    "high_below_low",
    "ohlc_out_of_range",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketDataRule {
    TimestampNotInteger,
    TimestampOutOfRange,
    TimestampNotRepresentable,
    PriceNotPositive,
    VolumeNegative,
    HighBelowLow,
    OhlcOutOfRange,
}

impl MarketDataRule {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TimestampNotInteger => MARKET_DATA_RULE_IDS[0],
            Self::TimestampOutOfRange => MARKET_DATA_RULE_IDS[1],
            Self::TimestampNotRepresentable => MARKET_DATA_RULE_IDS[2],
            Self::PriceNotPositive => MARKET_DATA_RULE_IDS[3],
            Self::VolumeNegative => MARKET_DATA_RULE_IDS[4],
            Self::HighBelowLow => MARKET_DATA_RULE_IDS[5],
            Self::OhlcOutOfRange => MARKET_DATA_RULE_IDS[6],
        }
    }
}

/// One candle's numeric fields. `timestamp` is `f64` so the integrality rule is
/// expressible for callers whose timestamps did not arrive as `i64`.
#[derive(Clone, Copy, Debug)]
pub struct CandleFields {
    pub timestamp: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct MarketDataIssue {
    pub index: usize,
    pub timestamp: f64,
    pub rule: MarketDataRule,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketDataError(pub String);

impl std::fmt::Display for MarketDataError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MarketDataError {}

/// Stable technical message, matching `describeMarketDataIssue` in TypeScript.
impl std::fmt::Display for MarketDataIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "market data rejected at candle {} (timestamp {}): {}",
            self.index,
            self.timestamp,
            self.rule.as_str()
        )
    }
}

/// Rule 3's predicate, asserted independently of the range rather than left as
/// an implied consequence of it. Every value that survives rule 2 is
/// representable, so rule 3 is unreachable today — it stays as defence in depth
/// against a future range change, and is unit-tested directly below.
pub fn is_representable_timestamp(timestamp: f64) -> bool {
    if !timestamp.is_finite() || timestamp.fract() != 0.0 {
        return false;
    }
    if timestamp < i64::MIN as f64 || timestamp > i64::MAX as f64 {
        return false;
    }
    Utc.timestamp_millis_opt(timestamp as i64).single().is_some()
}

fn is_positive_price(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// Classify one candle, or return `None` when it is admissible.
pub fn inspect_candle(index: usize, fields: CandleFields) -> Option<MarketDataIssue> {
    let CandleFields {
        timestamp,
        open,
        high,
        low,
        close,
        volume,
    } = fields;
    let at = |rule: MarketDataRule| {
        Some(MarketDataIssue {
            index,
            timestamp,
            rule,
        })
    };

    if !timestamp.is_finite() || timestamp.fract() != 0.0 || timestamp.abs() > JS_MAX_SAFE_INTEGER {
        return at(MarketDataRule::TimestampNotInteger);
    }
    if timestamp < MIN_MARKET_TIMESTAMP_MS as f64
        || timestamp >= MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE as f64
    {
        return at(MarketDataRule::TimestampOutOfRange);
    }
    if !is_representable_timestamp(timestamp) {
        return at(MarketDataRule::TimestampNotRepresentable);
    }
    if !is_positive_price(open)
        || !is_positive_price(high)
        || !is_positive_price(low)
        || !is_positive_price(close)
    {
        return at(MarketDataRule::PriceNotPositive);
    }
    // A NaN volume fails this test, because `!(NaN >= 0.0)`.
    if !(volume.is_finite() && volume >= 0.0) {
        return at(MarketDataRule::VolumeNegative);
    }
    if high < low {
        return at(MarketDataRule::HighBelowLow);
    }
    if open < low || open > high || close < low || close > high {
        return at(MarketDataRule::OhlcOutOfRange);
    }
    None
}

/// The single mapping point from the pure core candle to validator fields.
pub fn core_candle_fields(candle: &Candle) -> CandleFields {
    CandleFields {
        timestamp: candle.timestamp as f64,
        open: candle.open,
        high: candle.high,
        low: candle.low,
        close: candle.close,
        volume: candle.volume,
    }
}

/// Evaluation stops at the first failing candle, so both runtimes report the
/// same index and rule for the same input. A dataset is admitted whole or
/// rejected whole; individual bad candles are never dropped.
pub fn first_issue<I: IntoIterator<Item = CandleFields>>(rows: I) -> Option<MarketDataIssue> {
    rows.into_iter()
        .enumerate()
        .find_map(|(index, fields)| inspect_candle(index, fields))
}

pub fn ensure_admissible<I: IntoIterator<Item = CandleFields>>(
    rows: I,
) -> Result<(), MarketDataError> {
    match first_issue(rows) {
        Some(issue) => Err(MarketDataError(issue.to_string())),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_fields() -> CandleFields {
        CandleFields {
            timestamp: 1_721_001_600_000.0,
            open: 100.0,
            high: 103.0,
            low: 99.0,
            close: 102.0,
            volume: 10.0,
        }
    }

    /// Rule 3 has no fixture rejection row because it is unreachable by
    /// construction, so the predicate is asserted directly instead. Rust and
    /// TypeScript deliberately disagree OUTSIDE the product range: chrono's
    /// UTC millisecond range is narrower than JavaScript's Date limit, which is
    /// exactly why representability cannot be a shared parity row.
    #[test]
    fn representability_predicate_is_asserted_independently_of_the_range() {
        assert!(is_representable_timestamp(
            MIN_MARKET_TIMESTAMP_MS as f64
        ));
        assert!(is_representable_timestamp(
            MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE as f64
        ));
        // JavaScript can represent this; chrono cannot.
        assert!(!is_representable_timestamp(8_500_000_000_000_000.0));
        assert!(!is_representable_timestamp(f64::NAN));
        assert!(!is_representable_timestamp(f64::INFINITY));
        assert!(!is_representable_timestamp(1_721_001_600_000.5));
    }

    /// Rule 3 cannot fire for any value that survives rule 2: the whole product
    /// range is representable in both runtimes.
    #[test]
    fn rule_three_is_unreachable_across_the_admitted_range() {
        let mut timestamp = MIN_MARKET_TIMESTAMP_MS;
        while timestamp < MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE {
            assert!(is_representable_timestamp(timestamp as f64));
            // ~9.5-day stride: enough samples to cover the range cheaply.
            timestamp += 823_543_211;
        }
        assert!(is_representable_timestamp(
            (MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE - 1) as f64
        ));
    }

    #[test]
    fn non_finite_fields_are_classified_by_the_first_failing_rule() {
        let nan_high = CandleFields {
            high: f64::NAN,
            ..valid_fields()
        };
        assert_eq!(
            inspect_candle(0, nan_high).expect("NaN high rejected").rule,
            MarketDataRule::PriceNotPositive
        );
        let infinite_volume = CandleFields {
            volume: f64::INFINITY,
            ..valid_fields()
        };
        assert_eq!(
            inspect_candle(0, infinite_volume)
                .expect("infinite volume rejected")
                .rule,
            MarketDataRule::VolumeNegative
        );
        assert!(inspect_candle(
            0,
            CandleFields {
                volume: 0.0,
                ..valid_fields()
            }
        )
        .is_none());
    }

    #[test]
    fn the_technical_message_names_the_index_timestamp_and_rule() {
        let error = ensure_admissible([
            valid_fields(),
            CandleFields {
                timestamp: 1_704_067_200.0,
                ..valid_fields()
            },
        ])
        .expect_err("epoch-seconds candle rejected");
        assert_eq!(
            error.0,
            "market data rejected at candle 1 (timestamp 1704067200): timestamp_out_of_range"
        );
    }

    #[test]
    fn the_core_candle_mapping_round_trips_every_field() {
        let candle = Candle {
            timestamp: 1_721_001_600_000,
            open: 100.0,
            high: 103.0,
            low: 99.0,
            close: 102.0,
            volume: 10.0,
        };
        let fields = core_candle_fields(&candle);
        assert_eq!(fields.timestamp, candle.timestamp as f64);
        assert_eq!(fields.open, candle.open);
        assert_eq!(fields.high, candle.high);
        assert_eq!(fields.low, candle.low);
        assert_eq!(fields.close, candle.close);
        assert_eq!(fields.volume, candle.volume);
        assert!(first_issue([fields]).is_none());
    }
}
