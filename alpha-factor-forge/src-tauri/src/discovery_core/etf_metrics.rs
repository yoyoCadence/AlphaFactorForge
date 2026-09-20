//! Opt-in `etf-metrics-v1`; reuse metrics-v2 except elapsed-time CAGR/Calmar.
use super::etf::{date_ms, validate_etf_market, EtfMarket, EtfResult, NativeCurrency};
use super::metrics::{compute_metrics, Metrics, MetricsInput, TradeSide};
pub const ETF_METRICS_VERSION: &str = "etf-metrics-v1";
pub const ELAPSED_YEAR_MS: f64 = 365.25 * 86_400_000.0;
pub struct EtfMetrics {
    pub version: &'static str,
    pub currency: NativeCurrency,
    pub calendar_id: String,
    pub sessions_per_year: f64,
    pub elapsed_years: f64,
    pub annualized_volatility: f64,
    pub metrics: Metrics,
}
/// bars_per_year in the legacy input is ignored; the frozen market setting owns it.
pub fn compute_etf_metrics(
    input: &MetricsInput<'_>,
    market: &EtfMarket,
    start: i64,
    end: i64,
) -> EtfResult<EtfMetrics> {
    validate_etf_market(market)?;
    let initial = input.start_equity.ok_or("invalid_metrics_input")?;
    if end <= start
        || start < date_ms(&market.calendar_from)?
        || end > date_ms(&market.calendar_to_exclusive)?
        || !initial.is_finite()
        || initial <= 0.0
        || !input.risk_free_per_bar.unwrap_or(0.0).is_finite()
        || input.equity.is_empty()
        || input.total_bars != input.equity.len() as i64
    {
        return Err("invalid_metrics_input");
    }
    let (mut previous, mut previous_time) = (initial, start);
    let mut returns = Vec::new();
    for (index, point) in input.equity.iter().enumerate() {
        if point.time <= previous_time
            || point.time > end
            || !point.equity.is_finite()
            || point.equity < 0.0
            || (point.equity == 0.0 && index < input.equity.len() - 1)
        {
            return Err("invalid_equity");
        }
        returns.push(point.equity / previous - 1.0);
        previous = point.equity;
        previous_time = point.time;
    }
    if previous_time != end {
        return Err("missing_final_valuation");
    }
    for t in input.trades {
        if t.entry_time < start
            || t.exit_time > end
            || t.exit_time < t.entry_time
            || t.side != TradeSide::Long
            || t.bars < 0
            || t.bars > input.total_bars
            || ![t.entry_price, t.exit_price]
                .iter()
                .all(|v| v.is_finite() && *v > 0.0)
            || ![t.pnl, t.pnl_pct].iter().all(|v| v.is_finite())
        {
            return Err("invalid_trade");
        }
    }
    let mut metrics = compute_metrics(&MetricsInput {
        bars_per_year: market.sessions_per_year,
        ..*input
    });
    let cagr = (previous / initial).powf(ELAPSED_YEAR_MS / (end - start) as f64) - 1.0;
    if !cagr.is_finite() || returns.iter().any(|v| !v.is_finite()) {
        return Err("numeric_overflow");
    }
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / returns.len() as f64;
    let annualized_volatility = (variance * market.sessions_per_year).sqrt();
    if !annualized_volatility.is_finite() {
        return Err("numeric_overflow");
    }
    metrics.cagr = cagr;
    metrics.calmar = if metrics.max_drawdown > 0.0 {
        cagr / metrics.max_drawdown
    } else if cagr > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };
    Ok(EtfMetrics {
        version: ETF_METRICS_VERSION,
        currency: market.currency,
        calendar_id: market.calendar.calendar_id.clone(),
        sessions_per_year: market.sessions_per_year,
        elapsed_years: (end - start) as f64 / ELAPSED_YEAR_MS,
        annualized_volatility,
        metrics,
    })
}
