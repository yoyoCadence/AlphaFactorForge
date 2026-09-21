//! Tiingo daily parser. Raw and adjusted prices stay separate; daily dates
//! are venue date labels, never exchange-open instants. Unknown fields are
//! retained in originals, while all consumed fields are validated strictly.
use crate::db::repositories::Candle;
use alpha_factor_forge::discovery_core::{
    etf::{DividendAction, NativeCurrency, SplitAction},
    market_foundation::utc_date_to_ms,
};
use chrono::{DateTime, Timelike};
use serde::{Deserialize, Serialize};

pub const SOURCE: &str = "tiingo-eod";
pub const VERSION: &str = "tiingo-eod-v1";
pub const SYMBOLS: [&str; 5] = ["SPY", "QQQ", "VTI", "TLT", "GLD"];
pub type ParseResult<T> = Result<T, &'static str>;

/// Date-only or the documented midnight UTC timestamp. No lossy slicing of
/// arbitrary timestamps (which could silently move an action to another day).
pub fn date_label(value: &str) -> ParseResult<String> {
    if utc_date_to_ms(value).is_some() {
        return Ok(value.to_string());
    }
    let time = DateTime::parse_from_rfc3339(value).map_err(|_| "invalid_source_date")?;
    if time.offset().local_minus_utc() != 0
        || time.time().num_seconds_from_midnight() != 0
        || time.nanosecond() != 0
    {
        return Err("invalid_source_date");
    }
    let date = time.format("%Y-%m-%d").to_string();
    utc_date_to_ms(&date).ok_or("invalid_source_date")?;
    Ok(date)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EodRow {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub adj_open: f64,
    pub adj_high: f64,
    pub adj_low: f64,
    pub adj_close: f64,
    pub adj_volume: f64,
    pub div_cash: f64,
    pub split_factor: f64,
}
fn ohlcv(o: f64, h: f64, l: f64, c: f64, v: f64) -> bool {
    [o, h, l, c].iter().all(|x| x.is_finite() && *x > 0.0)
        && h >= o.max(c)
        && l <= o.min(c)
        && h >= l
        && v.is_finite()
        && v >= 0.0
}
pub fn parse_eod(bytes: &[u8], from: i64, to: i64) -> ParseResult<Vec<EodRow>> {
    let mut rows: Vec<EodRow> =
        serde_json::from_slice(bytes).map_err(|_| "invalid_eod_response")?;
    let mut previous = None;
    for row in &mut rows {
        row.date = date_label(&row.date)?;
        let ts = utc_date_to_ms(&row.date).ok_or("invalid_source_date")?;
        if ts < from || ts >= to {
            return Err("eod_out_of_range");
        }
        if previous.is_some_and(|p| ts <= p) {
            return Err("duplicate_or_unsorted_eod");
        }
        previous = Some(ts);
        if !ohlcv(row.open, row.high, row.low, row.close, row.volume)
            || row.volume.fract() != 0.0
            || row.volume > 9_007_199_254_740_991.0
            || !ohlcv(
                row.adj_open,
                row.adj_high,
                row.adj_low,
                row.adj_close,
                row.adj_volume,
            )
            || !row.div_cash.is_finite()
            || row.div_cash < 0.0
            || !row.split_factor.is_finite()
            || row.split_factor <= 0.0
        {
            return Err("invalid_eod_values");
        }
    }
    Ok(rows)
}
impl EodRow {
    pub fn candle(&self) -> Candle {
        Candle {
            timestamp: utc_date_to_ms(&self.date).expect("validated date"),
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
        }
    }
    pub fn dividend(&self, id: &str) -> DividendAction {
        DividendAction {
            id: format!("tiingo:{id}:dividend:{}", self.date),
            instrument_id: id.into(),
            currency: NativeCurrency::USD,
            ex_date: self.date.clone(),
            payment_date: None,
            amount_per_share: self.div_cash,
        }
    }
    pub fn split(&self, id: &str, observed_at: i64) -> SplitAction {
        SplitAction {
            id: format!("tiingo:{id}:split:{}", self.date),
            instrument_id: id.into(),
            currency: NativeCurrency::USD,
            effective_date: self.date.clone(),
            ratio: self.split_factor,
            available_at_ms: observed_at,
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    ticker: String,
    exchange_code: String,
    start_date: Option<String>,
    end_date: Option<String>,
}
/// Coverage start is NOT an inception/listing date. Compare with the configured
/// listing evidence; never shrink the requested period to hide missing history.
pub fn parse_metadata(
    bytes: &[u8],
    ticker: &str,
    exchange: &str,
    from: i64,
    last: i64,
) -> ParseResult<()> {
    let m: Metadata = serde_json::from_slice(bytes).map_err(|_| "invalid_metadata")?;
    if !m.ticker.eq_ignore_ascii_case(ticker) || m.exchange_code != exchange {
        return Err("metadata_identity_mismatch");
    }
    let start = m
        .start_date
        .as_deref()
        .and_then(utc_date_to_ms)
        .ok_or("metadata_coverage_unknown")?;
    let end = m
        .end_date
        .as_deref()
        .and_then(utc_date_to_ms)
        .ok_or("metadata_coverage_unknown")?;
    if start > from || end < last || end < start {
        return Err("metadata_range_incomplete");
    }
    Ok(())
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Distribution {
    ticker: String,
    ex_date: String,
    payment_date: Option<String>,
    distribution: f64,
    distribution_frequency: String,
}
/// Reconcile in BOTH directions; an extra event and a missing event both block.
/// Unknown payment remains None. No use of today's adjusted prices for signals.
pub fn reconcile_dividends(
    bytes: &[u8],
    ticker: &str,
    id: &str,
    rows: &[EodRow],
    from: i64,
    to: i64,
) -> ParseResult<Vec<DividendAction>> {
    let events: Vec<Distribution> =
        serde_json::from_slice(bytes).map_err(|_| "invalid_distributions")?;
    let mut matched = std::collections::BTreeMap::new();
    for e in events {
        if !e.ticker.eq_ignore_ascii_case(ticker) {
            return Err("distribution_identity_mismatch");
        }
        let ex = date_label(&e.ex_date)?;
        let ts = utc_date_to_ms(&ex).ok_or("invalid_source_date")?;
        if ts < from || ts >= to {
            return Err("distribution_out_of_range");
        }
        if !e.distribution.is_finite() || e.distribution <= 0.0 || e.distribution_frequency == "c" {
            return Err("invalid_or_cancelled_distribution");
        }
        let payment = e.payment_date.as_deref().map(date_label).transpose()?;
        if payment.as_ref().is_some_and(|p| p < &ex) {
            return Err("invalid_payment_date");
        }
        if matched.insert(ex, (e.distribution, payment)).is_some() {
            return Err("duplicate_distribution");
        }
    }
    let mut dividends = Vec::new();
    for row in rows.iter().filter(|r| r.div_cash > 0.0) {
        let (amount, payment) = matched.remove(&row.date).ok_or("distribution_missing")?;
        if (amount - row.div_cash).abs() > 1e-10 * amount.max(1.0) {
            return Err("distribution_amount_mismatch");
        }
        let mut action = row.dividend(id);
        action.payment_date = payment;
        dividends.push(action);
    }
    if !matched.is_empty() {
        return Err("distribution_without_eod");
    }
    Ok(dividends)
}

pub fn urls(ticker: &str, from: &str, last: &str) -> [String; 3] {
    // Callers validate ticker against SYMBOLS and dates before this function.
    let root = "https://api.tiingo.com/tiingo";
    [format!("{root}/daily/{ticker}"),
     format!("{root}/daily/{ticker}/prices?startDate={from}&endDate={last}&format=json&resampleFreq=daily"),
     format!("{root}/corporate-actions/{ticker}/distributions?startExDate={from}&endExDate={last}")]
}
