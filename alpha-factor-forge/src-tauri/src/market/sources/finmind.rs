//! FinMind's public Taiwan-market datasets used by P10.
//!
//! This module is deliberately pure: published JSON bytes in, validated rows
//! out. The adapter keeps `TaiwanStockPrice` raw and rejects zero/placeholder
//! prices instead of forward-filling them or switching to an adjusted feed.
use crate::db::repositories::Candle;
use alpha_factor_forge::discovery_core::{
    etf::{DividendAction, NativeCurrency},
    market_foundation::{utc_date_of, utc_date_to_ms},
};
use serde::Deserialize;
use serde_json::Value;

pub const SOURCE: &str = "finmind";
pub const VERSION: &str = "finmind-tw-etf-v1";
pub const SYMBOLS: [&str; 5] = ["0050", "006208", "0056", "00878", "00713"];
pub type ParseResult<T> = Result<T, &'static str>;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PriceRow {
    pub date: String,
    pub stock_id: String,
    #[serde(rename = "Trading_Volume")]
    pub trading_volume: u64,
    #[serde(rename = "Trading_money")]
    pub trading_money: u64,
    pub open: f64,
    pub max: f64,
    pub min: f64,
    pub close: f64,
    pub spread: f64,
    #[serde(rename = "Trading_turnover")]
    pub trading_turnover: u64,
}

impl PriceRow {
    pub fn candle(&self) -> Candle {
        Candle {
            timestamp: utc_date_to_ms(&self.date).expect("validated FinMind date"),
            open: self.open,
            high: self.max,
            low: self.min,
            close: self.close,
            volume: self.trading_volume as f64,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SplitObservation {
    pub date: String,
    pub kind: String,
    pub before_price: f64,
    pub after_price: f64,
}

fn rows(bytes: &[u8]) -> ParseResult<Vec<Value>> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid_finmind_response")?;
    if value.get("status").and_then(Value::as_i64) != Some(200) {
        return Err("finmind_status_error");
    }
    value
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .ok_or("invalid_finmind_response")
}

fn in_range(date: &str, from: i64, to: i64) -> ParseResult<Option<i64>> {
    let timestamp = utc_date_to_ms(date).ok_or("invalid_source_date")?;
    Ok(((from..to).contains(&timestamp)).then_some(timestamp))
}

fn valid_ohlc(row: &PriceRow) -> bool {
    [row.open, row.max, row.min, row.close, row.spread]
        .iter()
        .all(|value| value.is_finite())
        && row.open > 0.0
        && row.max > 0.0
        && row.min > 0.0
        && row.close > 0.0
        && row.min <= row.open
        && row.min <= row.close
        && row.max >= row.open
        && row.max >= row.close
}

pub fn parse_prices(bytes: &[u8], symbol: &str, from: i64, to: i64) -> ParseResult<Vec<PriceRow>> {
    let mut parsed = Vec::new();
    let mut previous = None;
    for value in rows(bytes)? {
        let row: PriceRow =
            serde_json::from_value(value).map_err(|_| "invalid_finmind_price_schema")?;
        if row.stock_id != symbol {
            return Err("price_identity_mismatch");
        }
        let timestamp = in_range(&row.date, from, to)?.ok_or("price_out_of_range")?;
        if previous.is_some_and(|old| timestamp <= old) {
            return Err("duplicate_or_unsorted_price");
        }
        if !valid_ohlc(&row) {
            // FinMind uses zero OHLC when the upstream row has no published
            // price. It is evidence of an unusable row, never permission to
            // repeat the prior close.
            return Err("unpublished_or_invalid_price");
        }
        if row.trading_volume > 9_007_199_254_740_991 {
            return Err("volume_exceeds_exact_range");
        }
        previous = Some(timestamp);
        parsed.push(row);
    }
    Ok(parsed)
}

pub fn parse_trading_dates(bytes: &[u8], from: i64, to: i64) -> ParseResult<Vec<i64>> {
    let mut dates = Vec::new();
    for value in rows(bytes)? {
        let object = value.as_object().ok_or("invalid_trading_date_schema")?;
        let date = object
            .get("date")
            .and_then(Value::as_str)
            .ok_or("invalid_trading_date_schema")?;
        if let Some(timestamp) = in_range(date, from, to)? {
            dates.push(timestamp);
        }
    }
    dates.sort_unstable();
    if dates.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("duplicate_trading_date");
    }
    Ok(dates)
}

fn text<'a>(row: &'a Value, key: &str) -> ParseResult<&'a str> {
    row.get(key)
        .and_then(Value::as_str)
        .ok_or("invalid_finmind_event_schema")
}

fn optional_text(row: &Value, key: &str) -> ParseResult<Option<String>> {
    match row.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err("invalid_finmind_event_schema"),
    }
}

fn number(row: &Value, key: &str) -> ParseResult<f64> {
    row.get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or("invalid_finmind_event_schema")
}

pub fn parse_dividends(
    bytes: &[u8],
    symbol: &str,
    instrument_id: &str,
    from: i64,
    to: i64,
) -> ParseResult<Vec<DividendAction>> {
    let mut actions = Vec::new();
    for row in rows(bytes)? {
        if text(&row, "stock_id")? != symbol {
            return Err("dividend_identity_mismatch");
        }
        let stock =
            number(&row, "StockEarningsDistribution")? + number(&row, "StockStatutorySurplus")?;
        if stock > 0.0 {
            return Err("unsupported_stock_distribution");
        }
        let amount =
            number(&row, "CashEarningsDistribution")? + number(&row, "CashStatutorySurplus")?;
        if amount == 0.0 {
            continue;
        }
        let ex_date = text(&row, "CashExDividendTradingDate")?;
        if in_range(ex_date, from, to)?.is_none() {
            continue;
        }
        let payment_date = optional_text(&row, "CashDividendPaymentDate")?;
        if payment_date
            .as_deref()
            .is_some_and(|date| utc_date_to_ms(date).is_none())
        {
            return Err("invalid_payment_date");
        }
        if payment_date.as_deref().is_some_and(|date| date < ex_date) {
            return Err("invalid_payment_date");
        }
        actions.push(DividendAction {
            id: format!("finmind:{symbol}:dividend:{ex_date}"),
            instrument_id: instrument_id.into(),
            currency: NativeCurrency::TWD,
            ex_date: ex_date.into(),
            payment_date,
            amount_per_share: amount,
        });
    }
    actions.sort_by(|left, right| left.ex_date.cmp(&right.ex_date));
    if actions
        .windows(2)
        .any(|pair| pair[0].ex_date == pair[1].ex_date)
    {
        return Err("duplicate_dividend");
    }
    Ok(actions)
}

pub fn parse_splits(
    bytes: &[u8],
    symbol: &str,
    from: i64,
    to: i64,
) -> ParseResult<Vec<SplitObservation>> {
    let mut splits = Vec::new();
    for row in rows(bytes)? {
        if text(&row, "stock_id")? != symbol {
            return Err("split_identity_mismatch");
        }
        let date = text(&row, "date")?;
        if in_range(date, from, to)?.is_none() {
            continue;
        }
        let kind = text(&row, "type")?;
        if kind != "分割" && kind != "反分割" {
            return Err("unsupported_split_type");
        }
        let before_price = number(&row, "before_price")?;
        let after_price = number(&row, "after_price")?;
        if before_price <= 0.0 || after_price <= 0.0 {
            return Err("invalid_split_price");
        }
        splits.push(SplitObservation {
            date: date.into(),
            kind: kind.into(),
            before_price,
            after_price,
        });
    }
    splits.sort_by(|left, right| left.date.cmp(&right.date));
    if splits.windows(2).any(|pair| pair[0].date == pair[1].date) {
        return Err("duplicate_split");
    }
    Ok(splits)
}

pub fn urls(
    symbol: &str,
    from: &str,
    final_date: &str,
    event_from: &str,
    event_to: &str,
) -> [String; 4] {
    let root = "https://api.finmindtrade.com/api/v4/data";
    [
        format!("{root}?dataset=TaiwanStockTradingDate&start_date={from}&end_date={final_date}"),
        format!("{root}?dataset=TaiwanStockPrice&data_id={symbol}&start_date={from}&end_date={final_date}"),
        format!("{root}?dataset=TaiwanStockDividend&data_id={symbol}&start_date={event_from}&end_date={event_to}"),
        format!("{root}?dataset=TaiwanStockSplitPrice&data_id={symbol}&start_date={event_from}&end_date={event_to}"),
    ]
}

pub fn date(ms: i64) -> String {
    utc_date_of(ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(data: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"msg":"success","status":200,"data":data})).unwrap()
    }

    #[test]
    fn raw_prices_keep_share_volume_and_reject_zero_markers() {
        let bytes = envelope(serde_json::json!([{
            "date":"2025-06-18","stock_id":"0050","Trading_Volume":291042332,
            "Trading_money":13878058322u64,"open":47.0,"max":48.25,"min":46.95,
            "close":47.65,"spread":0.49,"Trading_turnover":242995
        }]));
        let from = utc_date_to_ms("2025-06-18").unwrap();
        let to = utc_date_to_ms("2025-06-19").unwrap();
        let rows = parse_prices(&bytes, "0050", from, to).unwrap();
        assert_eq!(rows[0].trading_volume, 291_042_332);
        assert_eq!(rows[0].candle().volume, 291_042_332.0);
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        value["data"][0]["open"] = serde_json::json!(0);
        assert_eq!(
            parse_prices(&serde_json::to_vec(&value).unwrap(), "0050", from, to),
            Err("unpublished_or_invalid_price")
        );
    }

    #[test]
    fn dividend_payment_and_split_observation_are_explicit() {
        let from = utc_date_to_ms("2025-01-01").unwrap();
        let to = utc_date_to_ms("2026-01-01").unwrap();
        let dividends = envelope(serde_json::json!([{
            "stock_id":"0050","StockEarningsDistribution":0,"StockStatutorySurplus":0,
            "CashEarningsDistribution":2.7,"CashStatutorySurplus":0,
            "CashExDividendTradingDate":"2025-01-17","CashDividendPaymentDate":"2025-02-20"
        }]));
        let actions = parse_dividends(&dividends, "0050", "tw-etf:twse:0050", from, to).unwrap();
        assert_eq!(actions[0].payment_date.as_deref(), Some("2025-02-20"));
        let splits = envelope(serde_json::json!([{
            "date":"2025-06-18","stock_id":"0050","type":"分割",
            "before_price":188.65,"after_price":47.16
        }]));
        let observed = parse_splits(&splits, "0050", from, to).unwrap();
        assert!(((observed[0].before_price / observed[0].after_price) - 4.0).abs() < 0.01);
    }
}
