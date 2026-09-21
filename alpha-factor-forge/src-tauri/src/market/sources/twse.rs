//! Pure parser/reconciler for TWSE's official monthly daily-quote response.
//! TWSE is comparison evidence only; its rows never fill a FinMind gap.
use super::finmind::PriceRow;
use alpha_factor_forge::discovery_core::market_foundation::utc_date_to_ms;
use serde_json::Value;
use std::collections::BTreeMap;

pub const SOURCE: &str = "twse-comparison";

#[derive(Clone, Debug, PartialEq)]
pub struct QuoteRow {
    pub date: String,
    pub volume: u64,
    pub money: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub spread: f64,
    pub turnover: u64,
}

fn roc_date(value: &str) -> Result<String, &'static str> {
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() != 3 {
        return Err("invalid_twse_roc_date");
    }
    let year: i32 = parts[0].parse().map_err(|_| "invalid_twse_roc_date")?;
    let month: u32 = parts[1].parse().map_err(|_| "invalid_twse_roc_date")?;
    let day: u32 = parts[2].parse().map_err(|_| "invalid_twse_roc_date")?;
    let date =
        chrono::NaiveDate::from_ymd_opt(year + 1911, month, day).ok_or("invalid_twse_roc_date")?;
    Ok(date.format("%Y-%m-%d").to_string())
}

fn integer(value: &str) -> Result<u64, &'static str> {
    value
        .replace(',', "")
        .parse()
        .map_err(|_| "invalid_twse_number")
}

fn decimal(value: &str) -> Result<f64, &'static str> {
    let cleaned = value.replace(',', "").replace('＋', "+").replace('－', "-");
    // TWSE right-aligns an unchanged value as ` 0.00`; `X0.00` marks an
    // ex-right/ex-dividend reference adjustment. The numeric part remains
    // the published change and the original marker stays in raw evidence.
    let cleaned = cleaned
        .trim()
        .trim_start_matches('X')
        .trim_start_matches('x');
    cleaned
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or("invalid_twse_number")
}

pub fn parse_month(bytes: &[u8], symbol: &str) -> Result<Vec<QuoteRow>, &'static str> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "invalid_twse_response")?;
    if value.get("stat").and_then(Value::as_str) != Some("OK") {
        return Err("twse_status_error");
    }
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .ok_or("invalid_twse_response")?;
    if !title.split_whitespace().any(|token| token == symbol) {
        return Err("twse_identity_mismatch");
    }
    let fields = value
        .get("fields")
        .and_then(Value::as_array)
        .ok_or("invalid_twse_response")?;
    let names: Vec<&str> = fields
        .iter()
        .map(|field| field.as_str().ok_or("invalid_twse_response"))
        .collect::<Result<_, _>>()?;
    let index = |name: &str| {
        names
            .iter()
            .position(|field| *field == name)
            .ok_or("invalid_twse_fields")
    };
    let indices = [
        index("日期")?,
        index("成交股數")?,
        index("成交金額")?,
        index("開盤價")?,
        index("最高價")?,
        index("最低價")?,
        index("收盤價")?,
        index("漲跌價差")?,
        index("成交筆數")?,
    ];
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("invalid_twse_response")?;
    let mut result = Vec::new();
    for raw in data {
        let row = raw.as_array().ok_or("invalid_twse_response")?;
        let get = |position: usize| {
            row.get(position)
                .and_then(Value::as_str)
                .ok_or("invalid_twse_response")
        };
        let quote = QuoteRow {
            date: roc_date(get(indices[0])?)?,
            volume: integer(get(indices[1])?)?,
            money: integer(get(indices[2])?)?,
            open: decimal(get(indices[3])?)?,
            high: decimal(get(indices[4])?)?,
            low: decimal(get(indices[5])?)?,
            close: decimal(get(indices[6])?)?,
            spread: decimal(get(indices[7])?)?,
            turnover: integer(get(indices[8])?)?,
        };
        if [quote.open, quote.high, quote.low, quote.close]
            .iter()
            .any(|price| *price <= 0.0)
        {
            return Err("unpublished_or_invalid_twse_price");
        }
        result.push(quote);
    }
    result.sort_by(|left, right| left.date.cmp(&right.date));
    if result.windows(2).any(|pair| pair[0].date == pair[1].date) {
        return Err("duplicate_twse_date");
    }
    Ok(result)
}

fn same_decimal(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

pub fn reconcile(
    primary: &[PriceRow],
    comparison: &[QuoteRow],
    from: i64,
    to: i64,
) -> Result<(), &'static str> {
    let filtered: BTreeMap<&str, &QuoteRow> = comparison
        .iter()
        .filter(|row| {
            utc_date_to_ms(&row.date).is_some_and(|timestamp| (from..to).contains(&timestamp))
        })
        .map(|row| (row.date.as_str(), row))
        .collect();
    if filtered.len() != primary.len() {
        return Err("twse_date_set_mismatch");
    }
    for row in primary {
        let official = filtered
            .get(row.date.as_str())
            .ok_or("twse_date_set_mismatch")?;
        if row.trading_volume != official.volume
            || row.trading_money != official.money
            || row.trading_turnover != official.turnover
            || !same_decimal(row.open, official.open)
            || !same_decimal(row.max, official.high)
            || !same_decimal(row.min, official.low)
            || !same_decimal(row.close, official.close)
            || !same_decimal(row.spread, official.spread)
        {
            return Err("twse_value_mismatch");
        }
    }
    Ok(())
}

pub fn month_url(symbol: &str, year: i32, month: u32) -> String {
    format!("https://www.twse.com.tw/rwd/zh/afterTrading/STOCK_DAY?date={year:04}{month:02}01&stockNo={symbol}&response=json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(volume: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "stat":"OK","date":"20250601","title":"114年06月 0050 元大台灣50 各日成交資訊",
            "fields":["日期","成交股數","成交金額","開盤價","最高價","最低價","收盤價","漲跌價差","成交筆數","註記"],
            "data":[["114/06/18",volume,"13,878,058,322","47.00","48.25","46.95","47.65","+0.49","242,995",""]]
        })).unwrap()
    }

    #[test]
    fn roc_date_and_share_volume_reconcile_without_unit_conversion() {
        let official = parse_month(&response("291,042,332"), "0050").unwrap();
        assert_eq!(official[0].date, "2025-06-18");
        assert_eq!(official[0].volume, 291_042_332);
        let primary = PriceRow {
            date: "2025-06-18".into(),
            stock_id: "0050".into(),
            trading_volume: 291_042_332,
            trading_money: 13_878_058_322,
            open: 47.0,
            max: 48.25,
            min: 46.95,
            close: 47.65,
            spread: 0.49,
            trading_turnover: 242_995,
        };
        let from = utc_date_to_ms("2025-06-18").unwrap();
        let to = utc_date_to_ms("2025-06-19").unwrap();
        assert_eq!(
            reconcile(std::slice::from_ref(&primary), &official, from, to),
            Ok(())
        );
        assert_eq!(
            reconcile(
                &[primary],
                &parse_month(&response("291,042"), "0050").unwrap(),
                from,
                to
            ),
            Err("twse_value_mismatch")
        );
    }

    #[test]
    fn unchanged_and_ex_dividend_change_markers_are_numeric() {
        assert_eq!(decimal(" 0.00"), Ok(0.0));
        assert_eq!(decimal("X0.00"), Ok(0.0));
    }
}
