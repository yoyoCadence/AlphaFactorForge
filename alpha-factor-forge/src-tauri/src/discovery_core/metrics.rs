//! `metrics-v1`: pure Rust parity port of `src/core/metrics`.
//!
//! Semantics mirror the TypeScript reference exactly, including the
//! METRIC-001 decisions: downside deviation over ALL bar returns, legitimate
//! `+Infinity` Sortino/Calmar/profit-factor, and every other zero-denominator
//! case resolving to 0. Monthly returns group by UTC calendar month.

use std::collections::BTreeMap;

use chrono::{Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};

pub const METRICS_CONTRACT_VERSION: &str = "metrics-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TradeSide {
    #[serde(rename = "LONG")]
    Long,
    #[serde(rename = "SHORT")]
    Short,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedTrade {
    pub entry_time: i64,
    pub exit_time: i64,
    pub side: TradeSide,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub bars: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EquityPoint {
    pub time: i64,
    pub equity: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub net_return: f64,
    pub cagr: f64,
    pub max_drawdown: f64,
    pub sharpe: f64,
    pub sortino: f64,
    pub calmar: f64,
    pub win_rate: f64,
    pub trade_count: i64,
    pub profit_factor: f64,
    pub avg_trade_return: f64,
    pub median_trade_return: f64,
    pub avg_holding_bars: f64,
    pub exposure: f64,
    pub turnover: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub consecutive_losses: i64,
    pub monthly_returns: BTreeMap<String, f64>,
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).expect("finite trade returns"));
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[middle]
    } else {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    }
}

/// Population standard deviation; fewer than two samples yield 0.
fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let center = mean(values);
    let squared: Vec<f64> = values
        .iter()
        .map(|value| (value - center) * (value - center))
        .collect();
    mean(&squared).sqrt()
}

/// METRIC-001: sqrt(mean(min(0, x)^2)) over ALL samples, so one downside
/// observation still yields a positive value.
fn downside_deviation(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let squared: Vec<f64> = values
        .iter()
        .map(|value| {
            let clipped = value.min(0.0);
            clipped * clipped
        })
        .collect();
    mean(&squared).sqrt()
}

/// Max peak-to-trough drawdown of an equity curve, as a positive fraction.
pub fn max_drawdown(equity: &[EquityPoint]) -> f64 {
    let mut peak = f64::NEG_INFINITY;
    let mut drawdown = 0.0f64;
    for point in equity {
        if point.equity > peak {
            peak = point.equity;
        }
        if peak > 0.0 {
            drawdown = drawdown.max((peak - point.equity) / peak);
        }
    }
    drawdown
}

pub struct MetricsInput<'a> {
    pub trades: &'a [ClosedTrade],
    pub equity: &'a [EquityPoint],
    /// Account equity immediately before the tested segment's first bar.
    pub start_equity: Option<f64>,
    /// Total bars in the tested segment, for exposure/turnover.
    pub total_bars: i64,
    /// Bars per year for the interval, for CAGR/annualization.
    pub bars_per_year: f64,
    /// Risk-free per-bar return, default 0.
    pub risk_free_per_bar: Option<f64>,
}

/// Compute the full metric set. Pure; mirrors `computeMetrics` exactly.
pub fn compute_metrics(input: &MetricsInput<'_>) -> Metrics {
    let trades = input.trades;
    let equity = input.equity;
    let total_bars = input.total_bars;
    let bars_per_year = input.bars_per_year;
    let risk_free = input.risk_free_per_bar.unwrap_or(0.0);

    let start = input
        .start_equity
        .unwrap_or_else(|| equity.first().map_or(1.0, |point| point.equity));
    let end = equity.last().map_or(start, |point| point.equity);
    let net_return = if start > 0.0 { end / start - 1.0 } else { 0.0 };

    let years = if bars_per_year > 0.0 {
        total_bars as f64 / bars_per_year
    } else {
        0.0
    };
    let cagr = if years > 0.0 && start > 0.0 {
        (end / start).powf(1.0 / years) - 1.0
    } else {
        0.0
    };

    // per-bar equity returns for Sharpe/Sortino
    let mut returns: Vec<f64> = Vec::with_capacity(equity.len());
    let mut previous = input.start_equity;
    for point in equity {
        if let Some(previous_equity) = previous {
            if previous_equity > 0.0 {
                returns.push(point.equity / previous_equity - 1.0);
            }
        }
        previous = Some(point.equity);
    }
    let excess: Vec<f64> = returns.iter().map(|value| value - risk_free).collect();
    let deviation = std_dev(&excess);
    let downside = downside_deviation(&excess);
    let mean_excess = mean(&excess);
    let annualize = if bars_per_year > 0.0 {
        bars_per_year.sqrt()
    } else {
        1.0
    };
    let sharpe = if deviation > 0.0 {
        (mean_excess / deviation) * annualize
    } else {
        0.0
    };
    // METRIC-001: no downside with positive mean excess is legitimately
    // infinite; every other zero-denominator case stays 0.
    let sortino = if downside > 0.0 {
        (mean_excess / downside) * annualize
    } else if mean_excess > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };

    let drawdown_equity: Vec<EquityPoint> = match (input.start_equity, equity.first()) {
        (Some(start_equity), Some(first)) => {
            let mut curve = Vec::with_capacity(equity.len() + 1);
            curve.push(EquityPoint {
                time: first.time,
                equity: start_equity,
            });
            curve.extend(equity.iter().cloned());
            curve
        }
        _ => equity.to_vec(),
    };
    let mdd = max_drawdown(&drawdown_equity);
    // METRIC-001: positive CAGR with zero drawdown is infinite Calmar, not 0.
    let calmar = if mdd > 0.0 {
        cagr / mdd
    } else if cagr > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };

    let return_pcts: Vec<f64> = trades.iter().map(|trade| trade.pnl_pct).collect();
    let wins: Vec<&ClosedTrade> = trades.iter().filter(|trade| trade.pnl > 0.0).collect();
    let losses: Vec<&ClosedTrade> = trades.iter().filter(|trade| trade.pnl < 0.0).collect();
    let gross_win: f64 = wins.iter().map(|trade| trade.pnl).sum();
    let gross_loss: f64 = -losses.iter().map(|trade| trade.pnl).sum::<f64>();

    let mut current_streak = 0i64;
    let mut max_streak = 0i64;
    for trade in trades {
        if trade.pnl < 0.0 {
            current_streak += 1;
            max_streak = max_streak.max(current_streak);
        } else {
            current_streak = 0;
        }
    }

    let bars_in_market: i64 = trades.iter().map(|trade| trade.bars).sum();

    Metrics {
        net_return,
        cagr,
        max_drawdown: mdd,
        sharpe,
        sortino,
        calmar,
        win_rate: if trades.is_empty() {
            0.0
        } else {
            wins.len() as f64 / trades.len() as f64
        },
        trade_count: trades.len() as i64,
        profit_factor: if gross_loss > 0.0 {
            gross_win / gross_loss
        } else if gross_win > 0.0 {
            f64::INFINITY
        } else {
            0.0
        },
        avg_trade_return: mean(&return_pcts),
        median_trade_return: median(&return_pcts),
        avg_holding_bars: if trades.is_empty() {
            0.0
        } else {
            bars_in_market as f64 / trades.len() as f64
        },
        exposure: if total_bars > 0 {
            bars_in_market as f64 / total_bars as f64
        } else {
            0.0
        },
        turnover: trades.len() as f64 / total_bars.max(1) as f64,
        largest_win: if wins.is_empty() {
            0.0
        } else {
            wins.iter()
                .map(|trade| trade.pnl_pct)
                .fold(f64::NEG_INFINITY, f64::max)
        },
        largest_loss: if losses.is_empty() {
            0.0
        } else {
            losses
                .iter()
                .map(|trade| trade.pnl_pct)
                .fold(f64::INFINITY, f64::min)
        },
        consecutive_losses: max_streak,
        monthly_returns: monthly_returns(equity),
    }
}

/// Equity grouped into per-calendar-month returns (`YYYY-MM` -> fraction).
pub fn monthly_returns(equity: &[EquityPoint]) -> BTreeMap<String, f64> {
    let mut by_month: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for point in equity {
        let stamp = Utc
            .timestamp_millis_opt(point.time)
            .single()
            .expect("equity timestamps are valid epoch milliseconds");
        let key = format!("{}-{:02}", stamp.year(), stamp.month());
        by_month
            .entry(key)
            .and_modify(|entry| entry.1 = point.equity)
            .or_insert((point.equity, point.equity));
    }
    by_month
        .into_iter()
        .map(|(key, (first, last))| {
            let value = if first > 0.0 { last / first - 1.0 } else { 0.0 };
            (key, value)
        })
        .collect()
}
