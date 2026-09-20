//! P06 — market foundation contract (`market-foundation-v1`, Rust side).
//!
//! Exact mirror of `src/core/market-data/foundation.ts`; the two are held
//! together by `fixtures/rs-core/market-foundation-v1.json`. Shapes and
//! invariants: `docs/market-foundation-v1.md`. Upstream contract:
//! `docs/market-contract.md`.
//!
//! Four questions, and only these four:
//!
//!   1. Is this instrument id well formed, and may two series be combined?
//!   2. Which unit are these raw timestamps in, and do they change mid-batch?
//!   3. Which bars does the calendar say SHOULD exist in a range?
//!   4. Which of them are missing, duplicated, unaligned, unexpected, or not
//!      closed yet — and what is the actionable next step?
//!
//! Nothing here converts timestamps between units, repairs a series, drops a
//! bar, or decides qualification. A unit change is evidence to adjudicate
//! (`docs/market-contract.md` §2), never something to normalise away.
//!
//! Relationship to `market-data-quality-v1` (`super::market_data`): that
//! contract is the per-candle plausibility gate at admission and is
//! unchanged. This one sits above it, about a series' relationship to its
//! instrument, calendar, and sources. The plausibility window is imported
//! rather than restated so the two cannot drift.
//!
//! Pure module: no database, no host, no IO, no user-facing copy.

use chrono::{NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use super::market_data::{MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE, MIN_MARKET_TIMESTAMP_MS};

pub const MARKET_FOUNDATION_VERSION: &str = "market-foundation-v1";
pub const MARKET_INSTRUMENT_VERSION: &str = "market-instrument-v1";
pub const SESSION_CALENDAR_VERSION: &str = "session-calendar-v1";
pub const MARKET_PROVENANCE_VERSION: &str = "market-provenance-v1";
pub const MARKET_SNAPSHOT_VERSION: &str = "market-snapshot-v1";

// ---------------------------------------------------------------- intervals

/// Bar cadence in milliseconds (`market-interval-v1`), in declared order.
///
/// Strict on purpose: an unknown interval is an error, never a silent daily
/// fallback. This is NOT `benchmarks::bars_per_year`, which keeps its
/// documented legacy behaviour (unknown -> 365, `1d` -> 365). Cadence and
/// annualisation are different questions and stay apart.
pub const INTERVAL_MS: [(&str, i64); 7] = [
    ("1m", 60_000),
    ("3m", 180_000),
    ("5m", 300_000),
    ("15m", 900_000),
    ("1h", 3_600_000),
    ("4h", 14_400_000),
    ("1d", 86_400_000),
];

const MS_PER_DAY: i64 = 86_400_000;

/// The cadence of `interval`, or `None` when this contract does not know it.
pub fn interval_ms(interval: &str) -> Option<i64> {
    INTERVAL_MS
        .iter()
        .find(|(name, _)| *name == interval)
        .map(|(_, ms)| *ms)
}

// -------------------------------------------------------------- instruments

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Market {
    Crypto,
    UsEtf,
    TwEtf,
}

pub const MARKET_IDS: [&str; 3] = ["crypto", "us-etf", "tw-etf"];

impl Market {
    pub fn as_str(self) -> &'static str {
        match self {
            Market::Crypto => MARKET_IDS[0],
            Market::UsEtf => MARKET_IDS[1],
            Market::TwEtf => MARKET_IDS[2],
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "crypto" => Some(Market::Crypto),
            "us-etf" => Some(Market::UsEtf),
            "tw-etf" => Some(Market::TwEtf),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetType {
    SpotCrypto,
    Etf,
}

pub const ASSET_TYPE_IDS: [&str; 2] = ["spot-crypto", "etf"];

impl AssetType {
    pub fn as_str(self) -> &'static str {
        match self {
            AssetType::SpotCrypto => ASSET_TYPE_IDS[0],
            AssetType::Etf => ASSET_TYPE_IDS[1],
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "spot-crypto" => Some(AssetType::SpotCrypto),
            "etf" => Some(AssetType::Etf),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PriceBasis {
    Raw,
    AdjustedSplit,
    AdjustedTotalReturn,
}

pub const PRICE_BASIS_IDS: [&str; 3] = ["raw", "adjusted-split", "adjusted-total-return"];

impl PriceBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            PriceBasis::Raw => PRICE_BASIS_IDS[0],
            PriceBasis::AdjustedSplit => PRICE_BASIS_IDS[1],
            PriceBasis::AdjustedTotalReturn => PRICE_BASIS_IDS[2],
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "raw" => Some(PriceBasis::Raw),
            "adjusted-split" => Some(PriceBasis::AdjustedSplit),
            "adjusted-total-return" => Some(PriceBasis::AdjustedTotalReturn),
            _ => None,
        }
    }
}

/// Stable rule ids for instrument-id parsing, evaluated in this order.
pub const INSTRUMENT_ID_RULE_IDS: [&str; 4] = [
    "instrument_id_shape",
    "unknown_market",
    "venue_not_normalized",
    "symbol_not_supported",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentIdRule {
    Shape,
    UnknownMarket,
    VenueNotNormalized,
    SymbolNotSupported,
}

impl InstrumentIdRule {
    pub fn as_str(self) -> &'static str {
        match self {
            InstrumentIdRule::Shape => INSTRUMENT_ID_RULE_IDS[0],
            InstrumentIdRule::UnknownMarket => INSTRUMENT_ID_RULE_IDS[1],
            InstrumentIdRule::VenueNotNormalized => INSTRUMENT_ID_RULE_IDS[2],
            InstrumentIdRule::SymbolNotSupported => INSTRUMENT_ID_RULE_IDS[3],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentId {
    pub market: Market,
    pub venue: String,
    pub symbol: String,
}

fn is_venue_char(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
}

fn is_symbol_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

/// Parse `<market>:<venue>:<symbol>` (`docs/market-contract.md` §1).
///
/// The symbol keeps the venue's own spelling: `USDT` is never renamed `USD`,
/// and case is never folded, because a renamed quote currency is exactly the
/// kind of silent merge this contract exists to prevent.
pub fn parse_instrument_id(value: &str) -> Result<InstrumentId, InstrumentIdRule> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return Err(InstrumentIdRule::Shape);
    }
    let market = Market::parse(parts[0]).ok_or(InstrumentIdRule::UnknownMarket)?;
    if !parts[1].chars().all(is_venue_char) {
        return Err(InstrumentIdRule::VenueNotNormalized);
    }
    if !parts[2].chars().all(is_symbol_char) {
        return Err(InstrumentIdRule::SymbolNotSupported);
    }
    Ok(InstrumentId {
        market,
        venue: parts[1].to_string(),
        symbol: parts[2].to_string(),
    })
}

pub fn format_instrument_id(id: &InstrumentId) -> String {
    format!("{}:{}:{}", id.market.as_str(), id.venue, id.symbol)
}

// ------------------------------------------------------- source combination

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeriesRole {
    Primary,
    Comparison,
}

impl SeriesRole {
    pub fn as_str(self) -> &'static str {
        match self {
            SeriesRole::Primary => "primary",
            SeriesRole::Comparison => "comparison",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "primary" => Some(SeriesRole::Primary),
            "comparison" => Some(SeriesRole::Comparison),
            _ => None,
        }
    }
}

/// What a series is, for the question "may these two be one series?".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesIdentity {
    pub instrument_id: String,
    pub quote: String,
    pub interval: String,
    /// The retrieval source, e.g. `binance-archive`, `tiingo`, `csv`.
    pub source: String,
    pub role: SeriesRole,
    pub price_basis: PriceBasis,
}

/// Stable conflict codes, reported sorted.
pub const SERIES_CONFLICT_CODES: [&str; 9] = [
    "comparison_role_not_combinable",
    "interval_mismatch",
    "invalid_instrument_id",
    "market_mismatch",
    "price_basis_mismatch",
    "quote_mismatch",
    "source_mismatch",
    "symbol_mismatch",
    "venue_mismatch",
];

/// The publisher a source id belongs to: `<origin>[-<endpoint>]`.
///
/// `binance-archive` and `binance-rest` are two endpoints of ONE exchange
/// publishing its own market; `coinbase-rest` is a different exchange. The
/// distinction is what lets the contract say "the archive's gap may be
/// filled from the same exchange's REST endpoint" (`docs/market-contract.md`
/// §4, plan §5 P07) while still refusing to splice two exchanges together.
pub fn source_origin(source: &str) -> &str {
    match source.find('-') {
        Some(separator) => &source[..separator],
        None => source,
    }
}

/// Whether two source ids are the same publisher. Empty is never a match.
pub fn sources_share_origin(a: &str, b: &str) -> bool {
    let origin = source_origin(a);
    !origin.is_empty() && origin == source_origin(b)
}

/// Why two series may not be combined into one tradable series — empty means
/// they may (`docs/market-contract.md` §6: never splice different venues or
/// quote currencies together, and a second source produces comparison
/// evidence rather than backfill).
///
/// `source_mismatch` is reported for any two different source ids, including
/// two endpoints of the same publisher. It is a fact about the retrieval,
/// and whoever composes a series decides what to do with it: the snapshot
/// builder tolerates it when `sources_share_origin` holds, and never
/// otherwise.
pub fn series_conflicts(a: &SeriesIdentity, b: &SeriesIdentity) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    let mut add = |code: &'static str| {
        if !found.contains(&code) {
            found.push(code);
        }
    };
    match (
        parse_instrument_id(&a.instrument_id),
        parse_instrument_id(&b.instrument_id),
    ) {
        (Ok(left), Ok(right)) => {
            if left.market != right.market {
                add("market_mismatch");
            }
            if left.venue != right.venue {
                add("venue_mismatch");
            }
            if left.symbol != right.symbol {
                add("symbol_mismatch");
            }
        }
        _ => add("invalid_instrument_id"),
    }
    if a.quote != b.quote {
        add("quote_mismatch");
    }
    if a.interval != b.interval {
        add("interval_mismatch");
    }
    if a.source != b.source {
        add("source_mismatch");
    }
    if a.price_basis != b.price_basis {
        add("price_basis_mismatch");
    }
    if a.role == SeriesRole::Comparison || b.role == SeriesRole::Comparison {
        add("comparison_role_not_combinable");
    }
    found.sort_unstable();
    found
}

// --------------------------------------------------------------- time units

pub const TIME_UNIT_IDS: [&str; 4] = ["seconds", "milliseconds", "microseconds", "nanoseconds"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}

impl TimeUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            TimeUnit::Seconds => TIME_UNIT_IDS[0],
            TimeUnit::Milliseconds => TIME_UNIT_IDS[1],
            TimeUnit::Microseconds => TIME_UNIT_IDS[2],
            TimeUnit::Nanoseconds => TIME_UNIT_IDS[3],
        }
    }

    /// The scale of this unit relative to milliseconds. The band edges are
    /// derived from the ONE plausibility window in `market-data-quality-v1`,
    /// with the same f64 arithmetic as the TypeScript side.
    fn scale(self) -> f64 {
        match self {
            TimeUnit::Seconds => 1.0 / 1000.0,
            TimeUnit::Milliseconds => 1.0,
            TimeUnit::Microseconds => 1000.0,
            TimeUnit::Nanoseconds => 1_000_000.0,
        }
    }
}

pub const TIME_UNIT_ISSUE_IDS: [&str; 3] =
    ["no_timestamps", "time_unit_mixed", "time_unit_unknown"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnitIssue {
    NoTimestamps,
    Mixed,
    Unknown,
}

impl TimeUnitIssue {
    pub fn as_str(self) -> &'static str {
        match self {
            TimeUnitIssue::NoTimestamps => TIME_UNIT_ISSUE_IDS[0],
            TimeUnitIssue::Mixed => TIME_UNIT_ISSUE_IDS[1],
            TimeUnitIssue::Unknown => TIME_UNIT_ISSUE_IDS[2],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeUnitVerdict {
    /// The single unit every timestamp is in, or `None` when there is none.
    pub unit: Option<TimeUnit>,
    pub issue: Option<TimeUnitIssue>,
    /// The offending timestamp's index, or `None`.
    pub index: Option<usize>,
}

const TIME_UNITS: [TimeUnit; 4] = [
    TimeUnit::Seconds,
    TimeUnit::Milliseconds,
    TimeUnit::Microseconds,
    TimeUnit::Nanoseconds,
];

fn unit_of(timestamp: f64) -> Option<TimeUnit> {
    if !timestamp.is_finite() {
        return None;
    }
    TIME_UNITS.into_iter().find(|unit| {
        let scale = unit.scale();
        timestamp >= MIN_MARKET_TIMESTAMP_MS as f64 * scale
            && timestamp < MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE as f64 * scale
    })
}

/// Which unit a raw batch of timestamps is in (`docs/market-contract.md` §4:
/// the Binance archive changed from milliseconds to microseconds, so a batch
/// that changes unit half way through must be visible rather than averaged
/// away).
///
/// Detection only. Nothing here converts: a mixed batch is evidence for
/// adjudication, and a converted-by-guess series is the silent repair this
/// contract forbids.
pub fn detect_time_unit(timestamps: &[f64]) -> TimeUnitVerdict {
    let verdict = |unit, issue, index| TimeUnitVerdict { unit, issue, index };
    let Some(first_value) = timestamps.first() else {
        return verdict(None, Some(TimeUnitIssue::NoTimestamps), None);
    };
    let Some(first) = unit_of(*first_value) else {
        return verdict(None, Some(TimeUnitIssue::Unknown), Some(0));
    };
    for (index, timestamp) in timestamps.iter().enumerate().skip(1) {
        match unit_of(*timestamp) {
            None => return verdict(None, Some(TimeUnitIssue::Unknown), Some(index)),
            Some(unit) if unit != first => {
                return verdict(None, Some(TimeUnitIssue::Mixed), Some(index))
            }
            Some(_) => {}
        }
    }
    verdict(Some(first), None, None)
}

// ---------------------------------------------------------------- calendars

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CalendarKind {
    Continuous,
    TradingDays,
}

pub const CALENDAR_KIND_IDS: [&str; 2] = ["continuous", "trading-days"];

impl CalendarKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CalendarKind::Continuous => CALENDAR_KIND_IDS[0],
            CalendarKind::TradingDays => CALENDAR_KIND_IDS[1],
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "continuous" => Some(CalendarKind::Continuous),
            "trading-days" => Some(CalendarKind::TradingDays),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCalendar {
    /// Carries its own version, e.g. `crypto-24x7-v1`, `nyse-v1`, `twse-v1`.
    pub calendar_id: String,
    pub kind: CalendarKind,
    /// IANA zone. Recorded for session semantics (P08); v1 maths is UTC.
    pub timezone: String,
    /// ISO weekdays (Mon = 1 ... Sun = 7), ascending and unique.
    pub trading_weekdays: Vec<i64>,
    /// Non-trading dates, `YYYY-MM-DD`, ascending and unique.
    pub holidays: Vec<String>,
    /// Short sessions, `YYYY-MM-DD`. Recorded in v1, not used by range maths.
    pub early_closes: Vec<String>,
}

pub const CALENDAR_RULE_IDS: [&str; 9] = [
    "calendar_id_shape",
    "unknown_kind",
    "empty_timezone",
    "continuous_carries_trading_days",
    "trading_days_without_weekdays",
    "weekday_out_of_range",
    "weekdays_not_sorted",
    "invalid_date",
    "dates_not_sorted",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarRule {
    IdShape,
    UnknownKind,
    EmptyTimezone,
    ContinuousCarriesTradingDays,
    TradingDaysWithoutWeekdays,
    WeekdayOutOfRange,
    WeekdaysNotSorted,
    InvalidDate,
    DatesNotSorted,
}

impl CalendarRule {
    pub fn as_str(self) -> &'static str {
        match self {
            CalendarRule::IdShape => CALENDAR_RULE_IDS[0],
            CalendarRule::UnknownKind => CALENDAR_RULE_IDS[1],
            CalendarRule::EmptyTimezone => CALENDAR_RULE_IDS[2],
            CalendarRule::ContinuousCarriesTradingDays => CALENDAR_RULE_IDS[3],
            CalendarRule::TradingDaysWithoutWeekdays => CALENDAR_RULE_IDS[4],
            CalendarRule::WeekdayOutOfRange => CALENDAR_RULE_IDS[5],
            CalendarRule::WeekdaysNotSorted => CALENDAR_RULE_IDS[6],
            CalendarRule::InvalidDate => CALENDAR_RULE_IDS[7],
            CalendarRule::DatesNotSorted => CALENDAR_RULE_IDS[8],
        }
    }
}

/// `^[a-z0-9]+(-[a-z0-9]+)*-v[0-9]+$`, checked without a regex dependency.
fn calendar_id_is_well_formed(id: &str) -> bool {
    let segments: Vec<&str> = id.split('-').collect();
    if segments.len() < 2 {
        return false;
    }
    let (last, head) = segments.split_last().expect("at least two segments");
    let Some(version) = last.strip_prefix('v') else {
        return false;
    };
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    head.iter().all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    })
}

/// `YYYY-MM-DD` -> UTC midnight in epoch ms, or `None` when it is not a date.
pub fn utc_date_to_ms(date: &str) -> Option<i64> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !date
        .chars()
        .enumerate()
        .all(|(index, c)| index == 4 || index == 7 || c.is_ascii_digit())
    {
        return None;
    }
    let year: i32 = date[0..4].parse().ok()?;
    let month: u32 = date[5..7].parse().ok()?;
    let day: u32 = date[8..10].parse().ok()?;
    let naive = NaiveDate::from_ymd_opt(year, month, day)?;
    Some(naive.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis())
}

/// `YYYY-MM-DD` of a UTC timestamp.
pub fn utc_date_of(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .map(|moment| moment.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// ISO weekday (Mon = 1 ... Sun = 7) of a UTC timestamp.
pub fn iso_weekday_of_utc_ms(ms: i64) -> i64 {
    let day = ms.div_euclid(MS_PER_DAY);
    // 1970-01-01 was a Thursday (ISO 4).
    (day + 3).rem_euclid(7) + 1
}

/// The first calendar defect, or `None`.
pub fn validate_calendar(calendar: &SessionCalendar) -> Option<CalendarRule> {
    if !calendar_id_is_well_formed(&calendar.calendar_id) {
        return Some(CalendarRule::IdShape);
    }
    if calendar.timezone.trim().is_empty() {
        return Some(CalendarRule::EmptyTimezone);
    }
    match calendar.kind {
        CalendarKind::Continuous => {
            if !calendar.trading_weekdays.is_empty() || !calendar.holidays.is_empty() {
                return Some(CalendarRule::ContinuousCarriesTradingDays);
            }
        }
        CalendarKind::TradingDays => {
            if calendar.trading_weekdays.is_empty() {
                return Some(CalendarRule::TradingDaysWithoutWeekdays);
            }
        }
    }
    if calendar
        .trading_weekdays
        .iter()
        .any(|weekday| !(1..=7).contains(weekday))
    {
        return Some(CalendarRule::WeekdayOutOfRange);
    }
    if calendar
        .trading_weekdays
        .windows(2)
        .any(|pair| pair[1] <= pair[0])
    {
        return Some(CalendarRule::WeekdaysNotSorted);
    }
    for dates in [&calendar.holidays, &calendar.early_closes] {
        if dates.iter().any(|date| utc_date_to_ms(date).is_none()) {
            return Some(CalendarRule::InvalidDate);
        }
        if dates.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Some(CalendarRule::DatesNotSorted);
        }
    }
    None
}

// ----------------------------------------------------------- expected range

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suspension {
    pub from_ms: i64,
    pub to_ms_exclusive: i64,
}

#[derive(Clone, Debug)]
pub struct ExpectedRangeRequest<'a> {
    pub calendar: &'a SessionCalendar,
    pub interval: &'a str,
    /// Inclusive.
    pub from_ms: i64,
    /// Exclusive.
    pub to_ms_exclusive: i64,
    /// Inclusive listing time; `None` means "no recorded listing date".
    pub listed_from_ms: Option<i64>,
    /// Exclusive delisting time; `None` means "still listed".
    pub delisted_at_ms: Option<i64>,
    /// Halted periods; bars inside them are not expected.
    pub suspensions: &'a [Suspension],
}

pub const EXPECTED_RANGE_ISSUE_IDS: [&str; 6] = [
    "unknown_interval",
    "invalid_calendar",
    "unsupported_interval_for_calendar",
    "invalid_range",
    "invalid_suspension",
    "range_too_large",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedRangeIssue {
    UnknownInterval,
    InvalidCalendar,
    UnsupportedIntervalForCalendar,
    InvalidRange,
    InvalidSuspension,
    RangeTooLarge,
}

impl ExpectedRangeIssue {
    pub fn as_str(self) -> &'static str {
        match self {
            ExpectedRangeIssue::UnknownInterval => EXPECTED_RANGE_ISSUE_IDS[0],
            ExpectedRangeIssue::InvalidCalendar => EXPECTED_RANGE_ISSUE_IDS[1],
            ExpectedRangeIssue::UnsupportedIntervalForCalendar => EXPECTED_RANGE_ISSUE_IDS[2],
            ExpectedRangeIssue::InvalidRange => EXPECTED_RANGE_ISSUE_IDS[3],
            ExpectedRangeIssue::InvalidSuspension => EXPECTED_RANGE_ISSUE_IDS[4],
            ExpectedRangeIssue::RangeTooLarge => EXPECTED_RANGE_ISSUE_IDS[5],
        }
    }
}

/// A refused request returns its issue and no timestamps; never a guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedRangeResult {
    pub timestamps: Vec<i64>,
    pub issue: Option<ExpectedRangeIssue>,
}

/// Defence against an unbounded range: ~114 years of hourly bars.
pub const MAX_EXPECTED_BARS: usize = 1_000_000;

/// `Number.MAX_SAFE_INTEGER`; the TypeScript mirror refuses a range it cannot
/// represent exactly, so the same magnitudes are refused here.
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

fn is_safe_integer(value: i64) -> bool {
    value.abs() <= JS_MAX_SAFE_INTEGER
}

fn ceil_to(value: i64, step: i64) -> i64 {
    let quotient = value.div_euclid(step);
    if value.rem_euclid(step) == 0 {
        quotient * step
    } else {
        (quotient + 1) * step
    }
}

/// Which bar starts the calendar says should exist in
/// `[from_ms, to_ms_exclusive)` (`docs/market-contract.md` §6 step 1).
/// Weekends, holidays, halts, and pre-listing/post-delisting time are NOT
/// gaps, which is the point of deriving the expectation from a versioned
/// calendar instead of from the data.
///
/// v1 supports intraday cadences on a `continuous` calendar and `1d` on a
/// `trading-days` calendar. A daily bar's timestamp is UTC midnight of the
/// trading date; intraday ETF sessions (and therefore local-time session
/// boundaries) are P08, which is why no timezone database is consulted here.
pub fn expected_bar_starts(request: &ExpectedRangeRequest<'_>) -> ExpectedRangeResult {
    let none = |issue: ExpectedRangeIssue| ExpectedRangeResult {
        timestamps: Vec::new(),
        issue: Some(issue),
    };
    let Some(cadence) = interval_ms(request.interval) else {
        return none(ExpectedRangeIssue::UnknownInterval);
    };
    if validate_calendar(request.calendar).is_some() {
        return none(ExpectedRangeIssue::InvalidCalendar);
    }
    if request.calendar.kind == CalendarKind::TradingDays && cadence != MS_PER_DAY {
        return none(ExpectedRangeIssue::UnsupportedIntervalForCalendar);
    }
    if !is_safe_integer(request.from_ms)
        || !is_safe_integer(request.to_ms_exclusive)
        || request.to_ms_exclusive <= request.from_ms
    {
        return none(ExpectedRangeIssue::InvalidRange);
    }
    for suspension in request.suspensions {
        if !is_safe_integer(suspension.from_ms)
            || !is_safe_integer(suspension.to_ms_exclusive)
            || suspension.to_ms_exclusive <= suspension.from_ms
        {
            return none(ExpectedRangeIssue::InvalidSuspension);
        }
    }

    let start = request
        .listed_from_ms
        .map_or(request.from_ms, |listed| request.from_ms.max(listed));
    let end = request
        .delisted_at_ms
        .map_or(request.to_ms_exclusive, |delisted| {
            request.to_ms_exclusive.min(delisted)
        });
    if end <= start {
        return ExpectedRangeResult {
            timestamps: Vec::new(),
            issue: None,
        };
    }

    let mut timestamps = Vec::new();
    let mut timestamp = ceil_to(start, cadence);
    while timestamp < end {
        if timestamps.len() >= MAX_EXPECTED_BARS {
            return none(ExpectedRangeIssue::RangeTooLarge);
        }
        let mut expected = true;
        if request.calendar.kind == CalendarKind::TradingDays {
            let weekday = iso_weekday_of_utc_ms(timestamp);
            // `||` short-circuits, so a non-trading weekday never pays for
            // the date formatting the holiday lookup needs.
            if !request.calendar.trading_weekdays.contains(&weekday)
                || request.calendar.holidays.contains(&utc_date_of(timestamp))
            {
                expected = false;
            }
        }
        if expected
            && request
                .suspensions
                .iter()
                .any(|period| timestamp >= period.from_ms && timestamp < period.to_ms_exclusive)
        {
            expected = false;
        }
        if expected {
            timestamps.push(timestamp);
        }
        timestamp += cadence;
    }
    ExpectedRangeResult {
        timestamps,
        issue: None,
    }
}

// ----------------------------------------------------------- coverage audit

pub const SEVERITY_IDS: [&str; 3] = ["blocking", "degraded", "info"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Blocking,
    Degraded,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Blocking => SEVERITY_IDS[0],
            Severity::Degraded => SEVERITY_IDS[1],
            Severity::Info => SEVERITY_IDS[2],
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "blocking" => Some(Severity::Blocking),
            "degraded" => Some(Severity::Degraded),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }
}

/// Declared order; the coverage report sorts by this RANK, never by the code
/// text, because string collation is not guaranteed to agree between
/// JavaScript and Rust.
pub const COVERAGE_CODES: [&str; 6] = [
    "out_of_order",
    "duplicate_timestamp",
    "unaligned_timestamp",
    "unexpected_bar",
    "missing_bar",
    "unclosed_bar",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageCode {
    OutOfOrder,
    DuplicateTimestamp,
    UnalignedTimestamp,
    UnexpectedBar,
    MissingBar,
    UnclosedBar,
}

impl CoverageCode {
    fn rank(self) -> usize {
        match self {
            CoverageCode::OutOfOrder => 0,
            CoverageCode::DuplicateTimestamp => 1,
            CoverageCode::UnalignedTimestamp => 2,
            CoverageCode::UnexpectedBar => 3,
            CoverageCode::MissingBar => 4,
            CoverageCode::UnclosedBar => 5,
        }
    }

    pub fn as_str(self) -> &'static str {
        COVERAGE_CODES[self.rank()]
    }

    /// What a user can actually do about it; UI owns the wording.
    pub fn action(self) -> &'static str {
        match self {
            CoverageCode::OutOfOrder => "resort_source",
            CoverageCode::DuplicateTimestamp => "deduplicate_source",
            CoverageCode::UnalignedTimestamp => "verify_time_unit",
            CoverageCode::UnexpectedBar => "review_calendar",
            CoverageCode::MissingBar => "refetch_range",
            CoverageCode::UnclosedBar => "wait_for_close",
        }
    }
}

/// Every action code this contract can ask for, including the snapshot-level
/// ones the storage layer attaches (`docs/market-foundation-v1.md` §4).
pub const ACTION_CODES: [&str; 11] = [
    "refetch_range",
    "deduplicate_source",
    "resort_source",
    "verify_time_unit",
    "wait_for_close",
    "review_calendar",
    "separate_sources",
    "confirm_costs",
    "verify_corporate_actions",
    "rebuild_from_revision",
    "record_availability",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageEvent {
    #[serde(serialize_with = "serialize_coverage_code")]
    pub code: CoverageCode,
    pub severity: Severity,
    /// Inclusive first offending bar start.
    pub range_start: i64,
    /// Inclusive last offending bar start (equal to `range_start` alone).
    pub range_end: i64,
    /// Bars covered by this event (a merged gap counts every missing bar).
    pub count: i64,
    /// The actionable next step (`ACTION_CODES`); UI owns the wording.
    pub action: &'static str,
}

fn serialize_coverage_code<S: serde::Serializer>(
    code: &CoverageCode,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(code.as_str())
}

#[derive(Clone, Debug)]
pub struct CoverageRequest<'a> {
    pub interval: &'a str,
    /// Ascending and unique — `expected_bar_starts` output.
    pub expected: &'a [i64],
    /// As retrieved or stored, in any order.
    pub observed: &'a [i64],
    /// The data cut: a bar closing after this is not due yet.
    pub as_of_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReport {
    pub version: String,
    pub events: Vec<CoverageEvent>,
    pub blocking: bool,
    pub expected_count: i64,
    /// Expected bars whose close is after `as_of_ms`; not due, never missing.
    pub not_due_count: i64,
    pub observed_count: i64,
    pub matched_count: i64,
    #[serde(serialize_with = "serialize_range_issue")]
    pub issue: Option<ExpectedRangeIssue>,
}

fn serialize_range_issue<S: serde::Serializer>(
    issue: &Option<ExpectedRangeIssue>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match issue {
        Some(issue) => serializer.serialize_str(issue.as_str()),
        None => serializer.serialize_none(),
    }
}

fn coverage_event(code: CoverageCode, range_start: i64, range_end: i64, count: i64) -> CoverageEvent {
    // Every coverage defect blocks: a series is admitted whole or not at all,
    // exactly as `market-data-quality-v1` admits a dataset whole.
    CoverageEvent {
        code,
        severity: Severity::Blocking,
        range_start,
        range_end,
        count,
        action: code.action(),
    }
}

/// Compare what the calendar expects with what a source actually delivered
/// (`docs/market-contract.md` §6 step 2). Nothing is repaired, dropped, or
/// forward-filled: the report is evidence, and a blocking report keeps the
/// series out of a snapshot until a human or an adapter resolves it.
pub fn audit_coverage(request: &CoverageRequest<'_>) -> CoverageReport {
    let mut report = CoverageReport {
        version: MARKET_FOUNDATION_VERSION.to_string(),
        events: Vec::new(),
        blocking: false,
        expected_count: request.expected.len() as i64,
        not_due_count: 0,
        observed_count: request.observed.len() as i64,
        matched_count: 0,
        issue: None,
    };
    let Some(cadence) = interval_ms(request.interval) else {
        report.issue = Some(ExpectedRangeIssue::UnknownInterval);
        report.blocking = true;
        return report;
    };

    let mut events: Vec<CoverageEvent> = Vec::new();
    let mut observed = request.observed.to_vec();
    // Non-decreasing is "ordered": a repeated timestamp is a duplicate, which
    // has its own code, so it must not also be reported as bad ordering.
    let ordered = observed.windows(2).all(|pair| pair[1] >= pair[0]);
    if !ordered {
        observed.sort_unstable();
        events.push(coverage_event(
            CoverageCode::OutOfOrder,
            observed[0],
            observed[observed.len() - 1],
            observed.len() as i64,
        ));
    }

    // Membership is answered by binary search over sorted copies, which is
    // what the TypeScript mirror's Set does, without an O(n^2) scan.
    let mut expected_sorted = request.expected.to_vec();
    expected_sorted.sort_unstable();

    // Occurrences per timestamp, ascending (`observed` is sorted by now).
    let mut counts: Vec<(i64, i64)> = Vec::new();
    for timestamp in observed {
        match counts.last_mut() {
            Some((last, count)) if *last == timestamp => *count += 1,
            _ => counts.push((timestamp, 1)),
        }
    }

    let mut matched = 0i64;
    for (timestamp, count) in &counts {
        let (timestamp, count) = (*timestamp, *count);
        if count > 1 {
            events.push(coverage_event(
                CoverageCode::DuplicateTimestamp,
                timestamp,
                timestamp,
                count,
            ));
        }
        if timestamp.rem_euclid(cadence) != 0 {
            events.push(coverage_event(
                CoverageCode::UnalignedTimestamp,
                timestamp,
                timestamp,
                1,
            ));
            continue;
        }
        if expected_sorted.binary_search(&timestamp).is_err() {
            events.push(coverage_event(
                CoverageCode::UnexpectedBar,
                timestamp,
                timestamp,
                1,
            ));
            continue;
        }
        matched += 1;
        if timestamp + cadence > request.as_of_ms {
            events.push(coverage_event(
                CoverageCode::UnclosedBar,
                timestamp,
                timestamp,
                1,
            ));
        }
    }

    // Merge contiguous missing bars (contiguous in the EXPECTED grid, so a
    // weekend inside a gap does not split the report into two rows).
    let mut not_due = 0i64;
    let mut gap: Option<(i64, i64, i64)> = None;
    let flush = |gap: &mut Option<(i64, i64, i64)>, events: &mut Vec<CoverageEvent>| {
        if let Some((start, end, count)) = gap.take() {
            events.push(coverage_event(CoverageCode::MissingBar, start, end, count));
        }
    };
    for timestamp in request.expected {
        let timestamp = *timestamp;
        if timestamp + cadence > request.as_of_ms {
            not_due += 1;
            flush(&mut gap, &mut events);
            continue;
        }
        if counts.binary_search_by_key(&timestamp, |(seen, _)| *seen).is_ok() {
            flush(&mut gap, &mut events);
            continue;
        }
        gap = Some(match gap {
            Some((start, _, count)) => (start, timestamp, count + 1),
            None => (timestamp, timestamp, 1),
        });
    }
    flush(&mut gap, &mut events);

    events.sort_by(|a, b| {
        a.range_start
            .cmp(&b.range_start)
            .then(a.code.rank().cmp(&b.code.rank()))
    });
    report.blocking = events
        .iter()
        .any(|event| event.severity == Severity::Blocking);
    report.events = events;
    report.not_due_count = not_due;
    report.matched_count = matched;
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;
    const DAY: i64 = 86_400_000;
    /// 2024-07-15T00:00:00Z, a Monday.
    const T0: i64 = 1_721_001_600_000;

    fn crypto_247() -> SessionCalendar {
        SessionCalendar {
            calendar_id: "crypto-24x7-v1".into(),
            kind: CalendarKind::Continuous,
            timezone: "UTC".into(),
            trading_weekdays: Vec::new(),
            holidays: Vec::new(),
            early_closes: Vec::new(),
        }
    }

    fn weekdays() -> SessionCalendar {
        SessionCalendar {
            calendar_id: "test-weekdays-v1".into(),
            kind: CalendarKind::TradingDays,
            timezone: "America/New_York".into(),
            trading_weekdays: vec![1, 2, 3, 4, 5],
            holidays: vec!["2024-07-18".into()],
            early_closes: Vec::new(),
        }
    }

    #[test]
    fn intervals_are_strict_and_instrument_ids_report_the_first_failing_rule() {
        assert_eq!(interval_ms("1h"), Some(HOUR));
        assert_eq!(interval_ms("2h"), None);
        assert_eq!(
            parse_instrument_id("crypto:binance:BTCUSDT").unwrap(),
            InstrumentId {
                market: Market::Crypto,
                venue: "binance".into(),
                symbol: "BTCUSDT".into()
            }
        );
        assert_eq!(
            parse_instrument_id("tw-etf:twse:0050").unwrap().symbol,
            "0050"
        );
        assert_eq!(
            parse_instrument_id("crypto:binance").unwrap_err(),
            InstrumentIdRule::Shape
        );
        assert_eq!(
            parse_instrument_id("crypto::BTCUSDT").unwrap_err(),
            InstrumentIdRule::Shape
        );
        assert_eq!(
            parse_instrument_id("fx:oanda:EURUSD").unwrap_err(),
            InstrumentIdRule::UnknownMarket
        );
        assert_eq!(
            parse_instrument_id("crypto:Binance:BTCUSDT").unwrap_err(),
            InstrumentIdRule::VenueNotNormalized
        );
        assert_eq!(
            parse_instrument_id("crypto:binance:BTC USDT").unwrap_err(),
            InstrumentIdRule::SymbolNotSupported
        );
    }

    #[test]
    fn a_second_venue_or_a_comparison_source_is_never_the_same_series() {
        let binance = SeriesIdentity {
            instrument_id: "crypto:binance:BTCUSDT".into(),
            quote: "USDT".into(),
            interval: "1h".into(),
            source: "binance-archive".into(),
            role: SeriesRole::Primary,
            price_basis: PriceBasis::Raw,
        };
        assert!(series_conflicts(&binance, &binance.clone()).is_empty());
        let coinbase = SeriesIdentity {
            instrument_id: "crypto:coinbase:BTC-USD".into(),
            quote: "USD".into(),
            source: "coinbase-rest".into(),
            ..binance.clone()
        };
        assert_eq!(
            series_conflicts(&binance, &coinbase),
            vec![
                "quote_mismatch",
                "source_mismatch",
                "symbol_mismatch",
                "venue_mismatch"
            ]
        );
        let comparison = SeriesIdentity {
            role: SeriesRole::Comparison,
            ..binance.clone()
        };
        assert_eq!(
            series_conflicts(&binance, &comparison),
            vec!["comparison_role_not_combinable"]
        );
        let broken = SeriesIdentity {
            instrument_id: "binance-BTCUSDT".into(),
            ..binance.clone()
        };
        assert_eq!(
            series_conflicts(&binance, &broken),
            vec!["invalid_instrument_id"]
        );

        // Two endpoints of one publisher: the mismatch is still REPORTED —
        // it is a fact about the retrieval — and `sources_share_origin` is
        // how a composition decides it may be tolerated.
        let rest = SeriesIdentity {
            source: "binance-rest".into(),
            ..binance.clone()
        };
        assert_eq!(series_conflicts(&binance, &rest), vec!["source_mismatch"]);
        assert!(sources_share_origin("binance-archive", "binance-rest"));
        assert!(!sources_share_origin("binance-archive", "coinbase-rest"));
        assert_eq!(source_origin("binance-archive"), "binance");
        assert_eq!(source_origin("tiingo"), "tiingo");
        assert!(!sources_share_origin("", ""));
    }

    #[test]
    fn a_unit_change_inside_one_batch_is_reported_at_its_index() {
        assert_eq!(
            detect_time_unit(&[T0 as f64, (T0 + HOUR) as f64]),
            TimeUnitVerdict {
                unit: Some(TimeUnit::Milliseconds),
                issue: None,
                index: None
            }
        );
        assert_eq!(
            detect_time_unit(&[T0 as f64, (T0 + HOUR) as f64, (T0 + 2 * HOUR) as f64 * 1000.0]),
            TimeUnitVerdict {
                unit: None,
                issue: Some(TimeUnitIssue::Mixed),
                index: Some(2)
            }
        );
        assert_eq!(
            detect_time_unit(&[]).issue,
            Some(TimeUnitIssue::NoTimestamps)
        );
        assert_eq!(detect_time_unit(&[0.0]).issue, Some(TimeUnitIssue::Unknown));
        assert_eq!(
            detect_time_unit(&[T0 as f64, f64::NAN]).index,
            Some(1),
            "a non-finite value is in no band"
        );
    }

    #[test]
    fn calendars_are_validated_by_rule_and_dates_need_no_timezone_database() {
        assert_eq!(validate_calendar(&crypto_247()), None);
        assert_eq!(validate_calendar(&weekdays()), None);
        let bad_id = SessionCalendar {
            calendar_id: "crypto-24x7".into(),
            ..crypto_247()
        };
        assert_eq!(validate_calendar(&bad_id), Some(CalendarRule::IdShape));
        let carries = SessionCalendar {
            holidays: vec!["2024-07-18".into()],
            ..crypto_247()
        };
        assert_eq!(
            validate_calendar(&carries),
            Some(CalendarRule::ContinuousCarriesTradingDays)
        );
        let empty = SessionCalendar {
            trading_weekdays: Vec::new(),
            ..weekdays()
        };
        assert_eq!(
            validate_calendar(&empty),
            Some(CalendarRule::TradingDaysWithoutWeekdays)
        );
        let impossible = SessionCalendar {
            holidays: vec!["2024-02-30".into()],
            ..weekdays()
        };
        assert_eq!(validate_calendar(&impossible), Some(CalendarRule::InvalidDate));
        assert_eq!(utc_date_to_ms("2024-07-15"), Some(T0));
        assert_eq!(utc_date_to_ms("2024-13-01"), None);
        assert_eq!(utc_date_of(T0), "2024-07-15");
        assert_eq!(iso_weekday_of_utc_ms(T0), 1);
        assert_eq!(iso_weekday_of_utc_ms(T0 + 5 * DAY), 6);
        assert_eq!(iso_weekday_of_utc_ms(0), 4);
    }

    #[test]
    fn expected_bars_skip_weekends_holidays_halts_and_unlisted_time() {
        let calendar = crypto_247();
        let request = ExpectedRangeRequest {
            calendar: &calendar,
            interval: "1h",
            from_ms: T0,
            to_ms_exclusive: T0 + 3 * HOUR,
            listed_from_ms: None,
            delisted_at_ms: None,
            suspensions: &[],
        };
        assert_eq!(
            expected_bar_starts(&request).timestamps,
            vec![T0, T0 + HOUR, T0 + 2 * HOUR]
        );

        let calendar = weekdays();
        let halted = [Suspension {
            from_ms: T0 + DAY,
            to_ms_exclusive: T0 + 2 * DAY,
        }];
        let request = ExpectedRangeRequest {
            calendar: &calendar,
            interval: "1d",
            from_ms: T0,
            to_ms_exclusive: T0 + 7 * DAY,
            listed_from_ms: None,
            delisted_at_ms: None,
            suspensions: &halted,
        };
        assert_eq!(
            expected_bar_starts(&request).timestamps,
            vec![T0, T0 + 2 * DAY, T0 + 4 * DAY],
            "Tuesday halted, Thursday a holiday, the weekend not a gap"
        );

        let listed = ExpectedRangeRequest {
            listed_from_ms: Some(T0 + 2 * DAY),
            delisted_at_ms: Some(T0 + 4 * DAY),
            suspensions: &[],
            ..request.clone()
        };
        assert_eq!(expected_bar_starts(&listed).timestamps, vec![T0 + 2 * DAY]);
    }

    #[test]
    fn an_impossible_expected_range_is_refused_rather_than_guessed() {
        let calendar = crypto_247();
        let base = ExpectedRangeRequest {
            calendar: &calendar,
            interval: "1h",
            from_ms: T0,
            to_ms_exclusive: T0 + DAY,
            listed_from_ms: None,
            delisted_at_ms: None,
            suspensions: &[],
        };
        assert_eq!(
            expected_bar_starts(&ExpectedRangeRequest {
                interval: "2h",
                ..base.clone()
            })
            .issue,
            Some(ExpectedRangeIssue::UnknownInterval)
        );
        let trading_days = weekdays();
        assert_eq!(
            expected_bar_starts(&ExpectedRangeRequest {
                calendar: &trading_days,
                ..base.clone()
            })
            .issue,
            Some(ExpectedRangeIssue::UnsupportedIntervalForCalendar)
        );
        assert_eq!(
            expected_bar_starts(&ExpectedRangeRequest {
                to_ms_exclusive: T0,
                ..base.clone()
            })
            .issue,
            Some(ExpectedRangeIssue::InvalidRange)
        );
        let backwards = [Suspension {
            from_ms: T0 + DAY,
            to_ms_exclusive: T0,
        }];
        assert_eq!(
            expected_bar_starts(&ExpectedRangeRequest {
                suspensions: &backwards,
                ..base.clone()
            })
            .issue,
            Some(ExpectedRangeIssue::InvalidSuspension)
        );
        assert_eq!(
            expected_bar_starts(&ExpectedRangeRequest {
                interval: "1m",
                to_ms_exclusive: T0 + (MAX_EXPECTED_BARS as i64 + 1) * 60_000,
                ..base.clone()
            }),
            ExpectedRangeResult {
                timestamps: Vec::new(),
                issue: Some(ExpectedRangeIssue::RangeTooLarge)
            }
        );
        // No overlap between the listing and the range: empty, but no defect.
        assert_eq!(
            expected_bar_starts(&ExpectedRangeRequest {
                listed_from_ms: Some(T0 + 10 * DAY),
                ..base.clone()
            }),
            ExpectedRangeResult {
                timestamps: Vec::new(),
                issue: None
            }
        );
    }

    #[test]
    fn coverage_separates_every_defect_and_merges_contiguous_gaps() {
        let expected = [T0, T0 + HOUR, T0 + 2 * HOUR, T0 + 3 * HOUR];
        let as_of_ms = T0 + 4 * HOUR;
        let clean = audit_coverage(&CoverageRequest {
            interval: "1h",
            expected: &expected,
            observed: &expected,
            as_of_ms,
        });
        assert!(clean.events.is_empty() && !clean.blocking);
        assert_eq!((clean.matched_count, clean.not_due_count), (4, 0));

        let gap = audit_coverage(&CoverageRequest {
            interval: "1h",
            expected: &expected,
            observed: &[T0, T0 + 3 * HOUR],
            as_of_ms,
        });
        assert_eq!(gap.events.len(), 1);
        assert_eq!(gap.events[0].code, CoverageCode::MissingBar);
        assert_eq!(
            (gap.events[0].range_start, gap.events[0].range_end, gap.events[0].count),
            (T0 + HOUR, T0 + 2 * HOUR, 2)
        );
        assert_eq!(gap.events[0].action, "refetch_range");

        let messy = audit_coverage(&CoverageRequest {
            interval: "1h",
            expected: &expected,
            observed: &[
                T0 + HOUR,
                T0,
                T0,
                T0 + 2 * HOUR,
                T0 + 3 * HOUR + 1,
                T0 + 4 * HOUR,
                T0 + 3 * HOUR,
            ],
            as_of_ms,
        });
        assert_eq!(
            messy
                .events
                .iter()
                .map(|event| (event.code.as_str(), event.count))
                .collect::<Vec<_>>(),
            vec![
                ("out_of_order", 7),
                ("duplicate_timestamp", 2),
                ("unaligned_timestamp", 1),
                ("unexpected_bar", 1),
            ],
            "ordered by first offending bar, then by declared code rank"
        );
    }

    #[test]
    fn a_bar_that_has_not_closed_is_never_missing_and_one_that_arrived_early_is_reported() {
        let expected = [T0, T0 + HOUR, T0 + 2 * HOUR, T0 + 3 * HOUR];
        let early = audit_coverage(&CoverageRequest {
            interval: "1h",
            expected: &expected,
            observed: &expected,
            as_of_ms: T0 + 3 * HOUR + 1,
        });
        assert_eq!(early.not_due_count, 1);
        assert_eq!(early.events.len(), 1);
        assert_eq!(early.events[0].code, CoverageCode::UnclosedBar);
        assert_eq!(early.events[0].action, "wait_for_close");

        let waiting = audit_coverage(&CoverageRequest {
            interval: "1h",
            expected: &expected,
            observed: &expected[..3],
            as_of_ms: T0 + 3 * HOUR + 1,
        });
        assert!(waiting.events.is_empty() && !waiting.blocking);
        assert_eq!(waiting.not_due_count, 1);

        let unknown = audit_coverage(&CoverageRequest {
            interval: "2h",
            expected: &expected,
            observed: &expected,
            as_of_ms: T0 + 4 * HOUR,
        });
        assert_eq!(unknown.issue, Some(ExpectedRangeIssue::UnknownInterval));
        assert!(unknown.blocking && unknown.events.is_empty());
    }
}
