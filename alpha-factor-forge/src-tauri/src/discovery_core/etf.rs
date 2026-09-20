//! P08 opt-in daily ETF semantics; mirrors core/market-data/etf.ts.
//! Pure primitives for P09/P10 and P18, not a second fill engine or live ledger.
use super::market_data::{
    MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE as MAX, MIN_MARKET_TIMESTAMP_MS as MIN,
};
use super::market_foundation::{
    expected_bar_starts, parse_instrument_id, utc_date_to_ms, validate_calendar, CalendarKind,
    ExpectedRangeRequest, Market, SessionCalendar, Suspension,
};
use super::types::Candle;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const ETF_SEMANTICS_VERSION: &str = "etf-semantics-v1";
pub type EtfResult<T> = Result<T, &'static str>;
fn require(ok: bool, code: &'static str) -> EtfResult<()> {
    if ok {
        Ok(())
    } else {
        Err(code)
    }
}
fn nonnegative(v: f64) -> bool {
    v.is_finite() && v >= 0.0
}
fn positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}
pub(super) fn date_ms(date: &str) -> EtfResult<i64> {
    utc_date_to_ms(date)
        .filter(|v| (MIN..MAX).contains(v))
        .ok_or("invalid_date")
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum NativeCurrency {
    USD,
    TWD,
    USDT,
}
fn currency_for(id: &str) -> EtfResult<NativeCurrency> {
    match parse_instrument_id(id).map(|v| v.market) {
        Ok(Market::UsEtf) => Ok(NativeCurrency::USD),
        Ok(Market::TwEtf) => Ok(NativeCurrency::TWD),
        _ => Err("invalid_etf_instrument"),
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EtfMarket {
    pub instrument_id: String,
    pub currency: NativeCurrency,
    pub calendar: SessionCalendar,
    pub calendar_from: String,
    pub calendar_to_exclusive: String,
    pub sessions_per_year: f64,
}
pub fn validate_etf_market(m: &EtfMarket) -> EtfResult<()> {
    require(
        currency_for(&m.instrument_id)? == m.currency,
        "currency_mismatch",
    )?;
    require(
        validate_calendar(&m.calendar).is_none() && m.calendar.kind == CalendarKind::TradingDays,
        "invalid_calendar",
    )?;
    require(
        m.calendar.timezone
            == if m.currency == NativeCurrency::USD {
                "America/New_York"
            } else {
                "Asia/Taipei"
            },
        "timezone_mismatch",
    )?;
    require(
        date_ms(&m.calendar_to_exclusive)? > date_ms(&m.calendar_from)?,
        "invalid_calendar_range",
    )?;
    require(
        m.sessions_per_year.is_finite()
            && m.sessions_per_year.fract() == 0.0
            && m.sessions_per_year > 0.0
            && m.sessions_per_year <= 366.0,
        "invalid_annualization",
    )
}
/// UTC midnight session date labels, NOT venue opening instants.
pub fn etf_session_dates(
    m: &EtfMarket,
    from: &str,
    to: &str,
    suspensions: &[Suspension],
) -> EtfResult<Vec<i64>> {
    validate_etf_market(m)?;
    let (start, end) = (date_ms(from)?, date_ms(to)?);
    require(
        start >= date_ms(&m.calendar_from)? && end <= date_ms(&m.calendar_to_exclusive)?,
        "calendar_out_of_range",
    )?;
    let result = expected_bar_starts(&ExpectedRangeRequest {
        calendar: &m.calendar,
        interval: "1d",
        from_ms: start,
        to_ms_exclusive: end,
        listed_from_ms: None,
        delisted_at_ms: None,
        suspensions,
    });
    if let Some(issue) = result.issue {
        return Err(issue.as_str());
    }
    Ok(result.timestamps)
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EtfCostProfile {
    pub version: String,
    pub currency: NativeCurrency,
    pub confirmed: bool,
    pub commission_rate: f64,
    pub minimum_commission: f64,
    pub slippage_rate: f64,
    pub buy_tax_rate: f64,
    pub sell_tax_rate: f64,
}
pub fn validate_etf_costs(c: &EtfCostProfile, currency: NativeCurrency) -> EtfResult<()> {
    require(
        currency != NativeCurrency::USDT && c.currency == currency,
        "currency_mismatch",
    )?;
    require(!c.version.trim().is_empty(), "missing_cost_version")?;
    require(
        [
            c.commission_rate,
            c.slippage_rate,
            c.buy_tax_rate,
            c.sell_tax_rate,
        ]
        .iter()
        .all(|v| nonnegative(*v) && *v < 1.0)
            && nonnegative(c.minimum_commission),
        "invalid_cost",
    )
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostEstimate {
    pub version: &'static str,
    pub currency: NativeCurrency,
    pub execution_price: f64,
    pub commission: f64,
    pub transaction_tax: f64,
    pub slippage: f64,
    pub cash_delta: f64,
    pub personal_income_tax_included: bool,
}
pub fn estimate_etf_costs(
    c: &EtfCostProfile,
    currency: NativeCurrency,
    side: &str,
    quantity: f64,
    price: f64,
) -> EtfResult<CostEstimate> {
    validate_etf_costs(c, currency)?;
    require(side == "buy" || side == "sell", "invalid_side")?;
    require(positive(quantity) && positive(price), "invalid_order")?;
    let execution_price = price
        * (1.0
            + if side == "buy" {
                c.slippage_rate
            } else {
                -c.slippage_rate
            });
    let notional = quantity * execution_price;
    let commission = c.minimum_commission.max(notional * c.commission_rate);
    let transaction_tax = notional
        * if side == "buy" {
            c.buy_tax_rate
        } else {
            c.sell_tax_rate
        };
    let cash_delta = if side == "buy" {
        -(notional + commission + transaction_tax)
    } else {
        notional - commission - transaction_tax
    };
    require(
        [
            execution_price,
            notional,
            commission,
            transaction_tax,
            cash_delta,
        ]
        .iter()
        .all(|v| v.is_finite()),
        "numeric_overflow",
    )?;
    Ok(CostEstimate {
        version: ETF_SEMANTICS_VERSION,
        currency,
        execution_price,
        commission,
        transaction_tax,
        slippage: quantity * (execution_price - price).abs(),
        cash_delta,
        personal_income_tax_included: false,
    })
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SplitAction {
    pub id: String,
    pub instrument_id: String,
    pub currency: NativeCurrency,
    pub effective_date: String,
    pub ratio: f64,
    pub available_at_ms: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DividendAction {
    pub id: String,
    pub instrument_id: String,
    pub currency: NativeCurrency,
    pub ex_date: String,
    pub payment_date: Option<String>,
    pub amount_per_share: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DividendReceivable {
    pub action_id: String,
    pub ex_date: String,
    pub payment_date: Option<String>,
    pub amount: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingOrder {
    pub id: String,
    pub quantity: f64,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EtfPosition {
    pub instrument_id: String,
    pub currency: NativeCurrency,
    pub cash: f64,
    pub quantity: f64,
    pub cost_basis: f64,
    pub orders: Vec<PendingOrder>,
    pub receivables: Vec<DividendReceivable>,
    pub applied_action_ids: Vec<String>,
    pub last_event_date: String,
}
fn unique<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = HashSet::new();
    ids.into_iter().all(|id| seen.insert(id))
}
fn validate_position(s: &EtfPosition) -> EtfResult<()> {
    require(
        currency_for(&s.instrument_id)? == s.currency,
        "currency_mismatch",
    )?;
    date_ms(&s.last_event_date)?;
    require(
        [s.cash, s.quantity, s.cost_basis]
            .iter()
            .all(|v| nonnegative(*v))
            && (s.quantity > 0.0 || s.cost_basis == 0.0),
        "invalid_position",
    )?;
    require(
        s.applied_action_ids.iter().all(|id| !id.trim().is_empty())
            && unique(s.applied_action_ids.iter().map(String::as_str)),
        "duplicate_action",
    )?;
    require(
        unique(s.orders.iter().map(|o| o.id.as_str())),
        "invalid_order",
    )?;
    for o in &s.orders {
        require(
            !o.id.trim().is_empty()
                && positive(o.quantity)
                && o.limit_price.is_none_or(positive)
                && o.stop_price.is_none_or(positive),
            "invalid_order",
        )?;
    }
    require(
        unique(s.receivables.iter().map(|r| r.action_id.as_str())),
        "invalid_receivable",
    )?;
    for r in &s.receivables {
        require(
            s.applied_action_ids.contains(&r.action_id)
                && nonnegative(r.amount)
                && date_ms(&r.ex_date)? <= date_ms(&s.last_event_date)?
                && match &r.payment_date {
                    None => true,
                    Some(d) => date_ms(d)? >= date_ms(&r.ex_date)?,
                },
            "invalid_receivable",
        )?;
    }
    Ok(())
}
fn action_boundary(
    s: &EtfPosition,
    id: &str,
    instrument: &str,
    currency: NativeCurrency,
    date: &str,
) -> EtfResult<()> {
    validate_position(s)?;
    require(
        instrument == s.instrument_id && currency == s.currency,
        "currency_or_instrument_mismatch",
    )?;
    require(!id.trim().is_empty(), "invalid_action_id")?;
    require(
        !s.applied_action_ids.iter().any(|v| v == id),
        "duplicate_action",
    )?;
    require(
        date_ms(date)? >= date_ms(&s.last_event_date)?,
        "out_of_order_action",
    )
}
pub fn accrue_dividend(s: &EtfPosition, a: &DividendAction) -> EtfResult<EtfPosition> {
    action_boundary(s, &a.id, &a.instrument_id, a.currency, &a.ex_date)?;
    require(nonnegative(a.amount_per_share), "invalid_dividend")?;
    if let Some(date) = &a.payment_date {
        require(
            date_ms(date)? >= date_ms(&a.ex_date)?,
            "invalid_payment_date",
        )?;
    }
    let amount = s.quantity * a.amount_per_share;
    require(amount.is_finite(), "numeric_overflow")?;
    let mut next = s.clone();
    next.last_event_date = a.ex_date.clone();
    next.applied_action_ids.push(a.id.clone());
    next.receivables.push(DividendReceivable {
        action_id: a.id.clone(),
        ex_date: a.ex_date.clone(),
        payment_date: a.payment_date.clone(),
        amount,
    });
    Ok(next)
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionValue {
    pub currency: NativeCurrency,
    pub cash: f64,
    pub market_value: f64,
    pub receivables: f64,
    pub equity: f64,
}
/// Receivables contribute to equity, never to spendable cash before payment.
pub fn value_etf_position(s: &EtfPosition, raw_price: f64) -> EtfResult<PositionValue> {
    validate_position(s)?;
    require(positive(raw_price), "invalid_raw_price")?;
    let receivables = s.receivables.iter().map(|r| r.amount).sum::<f64>();
    let market_value = s.quantity * raw_price;
    let equity = s.cash + market_value + receivables;
    require(equity.is_finite(), "numeric_overflow")?;
    Ok(PositionValue {
        currency: s.currency,
        cash: s.cash,
        market_value,
        receivables,
        equity,
    })
}
pub fn pay_dividend(
    s: &EtfPosition,
    action_id: &str,
    payment_date: &str,
) -> EtfResult<EtfPosition> {
    validate_position(s)?;
    let r = s
        .receivables
        .iter()
        .find(|r| r.action_id == action_id)
        .ok_or("missing_receivable")?;
    let due = r.payment_date.as_ref().ok_or("unknown_payment_date")?;
    require(payment_date == due, "invalid_payment_date")?;
    require(
        date_ms(payment_date)? >= date_ms(&s.last_event_date)?,
        "out_of_order_action",
    )?;
    require((s.cash + r.amount).is_finite(), "numeric_overflow")?;
    let mut next = s.clone();
    next.cash += r.amount;
    next.last_event_date = payment_date.to_owned();
    next.receivables.retain(|r| r.action_id != action_id);
    Ok(next)
}
fn validate_split(a: &SplitAction) -> EtfResult<()> {
    require(
        !a.id.trim().is_empty() && currency_for(&a.instrument_id)? == a.currency,
        "invalid_split",
    )?;
    date_ms(&a.effective_date)?;
    require(
        positive(a.ratio) && (MIN..MAX).contains(&a.available_at_ms),
        "invalid_split",
    )
}
pub fn apply_split(s: &EtfPosition, a: &SplitAction) -> EtfResult<EtfPosition> {
    validate_split(a)?;
    action_boundary(s, &a.id, &a.instrument_id, a.currency, &a.effective_date)?;
    let mut next = s.clone();
    next.quantity *= a.ratio;
    next.last_event_date = a.effective_date.clone();
    next.applied_action_ids.push(a.id.clone());
    for o in &mut next.orders {
        o.quantity *= a.ratio;
        o.limit_price = o.limit_price.map(|p| p / a.ratio);
        o.stop_price = o.stop_price.map(|p| p / a.ratio);
    }
    validate_position(&next)?;
    require(s.quantity == 0.0 || next.quantity > 0.0, "numeric_overflow")?;
    Ok(next)
}
pub fn split_adjusted_signals(
    raw: &[Candle],
    instrument: &str,
    splits: &[SplitAction],
    as_of: i64,
) -> EtfResult<Vec<Candle>> {
    currency_for(instrument)?;
    require((MIN..MAX).contains(&as_of), "invalid_as_of")?;
    require(
        unique(splits.iter().map(|s| s.id.as_str())),
        "duplicate_action",
    )?;
    for s in splits {
        validate_split(s)?;
        require(
            s.instrument_id == instrument,
            "currency_or_instrument_mismatch",
        )?;
    }
    let mut visible: Vec<_> = splits
        .iter()
        .filter(|s| date_ms(&s.effective_date).unwrap() <= as_of && s.available_at_ms <= as_of)
        .collect();
    visible.sort_by(|a, b| {
        a.effective_date
            .cmp(&b.effective_date)
            .then(a.id.cmp(&b.id))
    });
    raw.iter()
        .enumerate()
        .map(|(i, bar)| {
            require(
                bar.timestamp % 86_400_000 == 0
                    && bar.timestamp >= MIN
                    && bar.timestamp <= as_of
                    && (i == 0 || raw[i - 1].timestamp < bar.timestamp)
                    && [bar.open, bar.high, bar.low, bar.close]
                        .iter()
                        .all(|v| positive(*v))
                    && nonnegative(bar.volume)
                    && bar.low <= bar.open.min(bar.close)
                    && bar.high >= bar.open.max(bar.close),
                "invalid_raw_bar",
            )?;
            let factor = visible
                .iter()
                .filter(|s| bar.timestamp < date_ms(&s.effective_date).unwrap())
                .fold(1.0, |v, s| v * s.ratio);
            require(positive(factor), "numeric_overflow")?;
            let adjusted = Candle {
                timestamp: bar.timestamp,
                open: bar.open / factor,
                high: bar.high / factor,
                low: bar.low / factor,
                close: bar.close / factor,
                volume: bar.volume * factor,
            };
            require(
                [adjusted.open, adjusted.high, adjusted.low, adjusted.close]
                    .iter()
                    .all(|v| positive(*v))
                    && nonnegative(adjusted.volume),
                "numeric_overflow",
            )?;
            Ok(adjusted)
        })
        .collect()
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionEvidence {
    pub version: String,
    pub complete: bool,
    pub from: String,
    pub to_exclusive: String,
    pub dividends: Vec<DividendAction>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticsVerdict {
    pub version: &'static str,
    pub status: &'static str,
    pub semantics_eligible: bool,
    pub reasons: Vec<&'static str>,
}
pub fn assess_etf_semantics(
    m: &EtfMarket,
    c: &EtfCostProfile,
    e: &ActionEvidence,
    from: &str,
    to: &str,
) -> EtfResult<SemanticsVerdict> {
    etf_session_dates(m, from, to, &[])?;
    validate_etf_costs(c, m.currency)?;
    let mut reasons = Vec::new();
    let (start, end) = (date_ms(&e.from)?, date_ms(&e.to_exclusive)?);
    require(end > start, "invalid_action_range")?;
    if e.version.trim().is_empty() || !e.complete || start > date_ms(from)? || end < date_ms(to)? {
        reasons.push("corporate_actions_unconfirmed");
    }
    require(
        unique(e.dividends.iter().map(|d| d.id.as_str())),
        "duplicate_action",
    )?;
    for d in &e.dividends {
        require(
            !d.id.trim().is_empty()
                && d.instrument_id == m.instrument_id
                && d.currency == m.currency
                && nonnegative(d.amount_per_share),
            "invalid_dividend",
        )?;
        let ex = date_ms(&d.ex_date)?;
        if let Some(date) = &d.payment_date {
            require(date_ms(date)? >= ex, "invalid_payment_date")?;
        }
        if ex >= date_ms(from)?
            && ex < date_ms(to)?
            && d.payment_date.is_none()
            && !reasons.contains(&"unknown_payment_date")
        {
            reasons.push("unknown_payment_date");
        }
    }
    if !c.confirmed {
        reasons.push("costs_unconfirmed");
    }
    Ok(SemanticsVerdict {
        version: ETF_SEMANTICS_VERSION,
        status: if reasons.is_empty() { "OK" } else { "DEGRADED" },
        semantics_eligible: reasons.is_empty(),
        reasons,
    })
}
