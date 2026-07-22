//! Shared expected-output types and comparison helpers for the RS-CORE
//! parity tests. One copy so the tolerance policy and metric-leaf handling
//! cannot drift between the backtest and benchmark suites.

use serde::Deserialize;

use super::metrics::{Metrics, TradeSide};

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct NumericTolerance {
    pub absolute: f64,
    pub relative: f64,
}

#[derive(Debug, Deserialize)]
pub struct TolerancePolicy {
    pub default: NumericTolerance,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedTrade {
    pub entry_time: i64,
    pub exit_time: i64,
    pub side: TradeSide,
    pub entry_price: f64,
    pub exit_price: f64,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub bars: i64,
}

#[derive(Debug, Deserialize)]
pub struct ExpectedEquity {
    pub time: i64,
    pub equity: f64,
}

/// A finite number, or a METRIC-001 non-finite status compared exactly.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum MetricLeaf {
    Number(f64),
    Status(String),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedMetrics {
    pub net_return: MetricLeaf,
    pub cagr: MetricLeaf,
    pub max_drawdown: MetricLeaf,
    pub sharpe: MetricLeaf,
    pub sortino: MetricLeaf,
    pub calmar: MetricLeaf,
    pub win_rate: MetricLeaf,
    pub trade_count: f64,
    pub profit_factor: MetricLeaf,
    pub avg_trade_return: MetricLeaf,
    pub median_trade_return: MetricLeaf,
    pub avg_holding_bars: MetricLeaf,
    pub exposure: MetricLeaf,
    pub turnover: MetricLeaf,
    pub largest_win: MetricLeaf,
    pub largest_loss: MetricLeaf,
    pub consecutive_losses: f64,
    pub monthly_returns: std::collections::BTreeMap<String, f64>,
}

pub fn assert_close(path: &str, actual: f64, expected: f64, tolerance: NumericTolerance) {
    assert!(actual.is_finite(), "{path} must be finite, got {actual}");
    let difference = (actual - expected).abs();
    let relative_scale = actual.abs().max(expected.abs());
    assert!(
        difference <= tolerance.absolute || difference <= tolerance.relative * relative_scale,
        "{path} differs: actual={actual}, expected={expected}, diff={difference}"
    );
}

pub fn assert_leaf(path: &str, actual: f64, expected: &MetricLeaf, tolerance: NumericTolerance) {
    match expected {
        MetricLeaf::Number(value) => assert_close(path, actual, *value, tolerance),
        MetricLeaf::Status(status) => match status.as_str() {
            "positive_infinity" => assert!(
                actual.is_infinite() && actual > 0.0,
                "{path} must be +Infinity, got {actual}"
            ),
            "negative_infinity" => assert!(
                actual.is_infinite() && actual < 0.0,
                "{path} must be -Infinity, got {actual}"
            ),
            "nan" => assert!(actual.is_nan(), "{path} must be NaN, got {actual}"),
            other => panic!("{path}: unknown non-finite status {other}"),
        },
    }
}

pub fn assert_metrics(
    case_id: &str,
    actual: &Metrics,
    expected: &ExpectedMetrics,
    tol: NumericTolerance,
) {
    let p = |field: &str| format!("{case_id}.metrics.{field}");
    assert_leaf(
        &p("netReturn"),
        actual.net_return,
        &expected.net_return,
        tol,
    );
    assert_leaf(&p("cagr"), actual.cagr, &expected.cagr, tol);
    assert_leaf(
        &p("maxDrawdown"),
        actual.max_drawdown,
        &expected.max_drawdown,
        tol,
    );
    assert_leaf(&p("sharpe"), actual.sharpe, &expected.sharpe, tol);
    assert_leaf(&p("sortino"), actual.sortino, &expected.sortino, tol);
    assert_leaf(&p("calmar"), actual.calmar, &expected.calmar, tol);
    assert_leaf(&p("winRate"), actual.win_rate, &expected.win_rate, tol);
    assert_eq!(
        actual.trade_count as f64,
        expected.trade_count,
        "{}",
        p("tradeCount")
    );
    assert_leaf(
        &p("profitFactor"),
        actual.profit_factor,
        &expected.profit_factor,
        tol,
    );
    assert_leaf(
        &p("avgTradeReturn"),
        actual.avg_trade_return,
        &expected.avg_trade_return,
        tol,
    );
    assert_leaf(
        &p("medianTradeReturn"),
        actual.median_trade_return,
        &expected.median_trade_return,
        tol,
    );
    assert_leaf(
        &p("avgHoldingBars"),
        actual.avg_holding_bars,
        &expected.avg_holding_bars,
        tol,
    );
    assert_leaf(&p("exposure"), actual.exposure, &expected.exposure, tol);
    assert_leaf(&p("turnover"), actual.turnover, &expected.turnover, tol);
    assert_leaf(
        &p("largestWin"),
        actual.largest_win,
        &expected.largest_win,
        tol,
    );
    assert_leaf(
        &p("largestLoss"),
        actual.largest_loss,
        &expected.largest_loss,
        tol,
    );
    assert_eq!(
        actual.consecutive_losses as f64,
        expected.consecutive_losses,
        "{}",
        p("consecutiveLosses")
    );

    let actual_keys: Vec<&String> = actual.monthly_returns.keys().collect();
    let expected_keys: Vec<&String> = expected.monthly_returns.keys().collect();
    assert_eq!(actual_keys, expected_keys, "{}", p("monthlyReturns keys"));
    for (key, expected_value) in &expected.monthly_returns {
        assert_close(
            &p(&format!("monthlyReturns[{key}]")),
            actual.monthly_returns[key],
            *expected_value,
            tol,
        );
    }
}

pub fn assert_trades(
    case_id: &str,
    actual: &[super::metrics::ClosedTrade],
    expected: &[ExpectedTrade],
    tol: NumericTolerance,
) {
    assert_eq!(actual.len(), expected.len(), "{case_id}: trade count");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let path = format!("{case_id}.trades[{index}]");
        assert_eq!(actual.entry_time, expected.entry_time, "{path}.entryTime");
        assert_eq!(actual.exit_time, expected.exit_time, "{path}.exitTime");
        assert_eq!(actual.side, expected.side, "{path}.side");
        assert_eq!(actual.bars, expected.bars, "{path}.bars");
        assert_close(
            &format!("{path}.entryPrice"),
            actual.entry_price,
            expected.entry_price,
            tol,
        );
        assert_close(
            &format!("{path}.exitPrice"),
            actual.exit_price,
            expected.exit_price,
            tol,
        );
        assert_close(&format!("{path}.pnl"), actual.pnl, expected.pnl, tol);
        assert_close(
            &format!("{path}.pnlPct"),
            actual.pnl_pct,
            expected.pnl_pct,
            tol,
        );
    }
}

pub fn assert_equity(
    case_id: &str,
    actual: &[super::metrics::EquityPoint],
    expected: &[ExpectedEquity],
    tol: NumericTolerance,
) {
    assert_eq!(actual.len(), expected.len(), "{case_id}: equity length");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let path = format!("{case_id}.equity[{index}]");
        assert_eq!(actual.time, expected.time, "{path}.time");
        assert_close(
            &format!("{path}.equity"),
            actual.equity,
            expected.equity,
            tol,
        );
    }
}
