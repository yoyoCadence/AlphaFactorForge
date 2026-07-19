use std::collections::BTreeMap;

use serde::Deserialize;

use super::backtest::{run_backtest, BacktestConfig, Signals, EXECUTION_CONTRACT_VERSION};
use super::metrics::{Metrics, TradeSide, METRICS_CONTRACT_VERSION};
use super::types::{Candle, CANDLE_CONTRACT_VERSION};

const FIXTURE_JSON: &str = include_str!("../../../fixtures/rs-core/backtest-v1.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    schema_version: String,
    fixture_version: String,
    contracts: Contracts,
    tolerance: TolerancePolicy,
    cases: Vec<ParityCase>,
    error_cases: Vec<ErrorCase>,
}

#[derive(Debug, Deserialize)]
struct Contracts {
    candle: String,
    execution: String,
    metrics: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct NumericTolerance {
    absolute: f64,
    relative: f64,
}

#[derive(Debug, Deserialize)]
struct TolerancePolicy {
    default: NumericTolerance,
}

#[derive(Debug, Deserialize)]
struct ParityCase {
    id: String,
    input: CaseInput,
    expected: ExpectedOutput,
}

#[derive(Debug, Deserialize)]
struct CaseInput {
    candles: Vec<Candle>,
    signals: Signals,
    config: BacktestConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedTrade {
    entry_time: i64,
    exit_time: i64,
    side: TradeSide,
    entry_price: f64,
    exit_price: f64,
    pnl: f64,
    pnl_pct: f64,
    bars: i64,
}

#[derive(Debug, Deserialize)]
struct ExpectedEquity {
    time: i64,
    equity: f64,
}

/// A finite number, or a METRIC-001 non-finite status compared exactly.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MetricLeaf {
    Number(f64),
    Status(String),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedMetrics {
    net_return: MetricLeaf,
    cagr: MetricLeaf,
    max_drawdown: MetricLeaf,
    sharpe: MetricLeaf,
    sortino: MetricLeaf,
    calmar: MetricLeaf,
    win_rate: MetricLeaf,
    trade_count: f64,
    profit_factor: MetricLeaf,
    avg_trade_return: MetricLeaf,
    median_trade_return: MetricLeaf,
    avg_holding_bars: MetricLeaf,
    exposure: MetricLeaf,
    turnover: MetricLeaf,
    largest_win: MetricLeaf,
    largest_loss: MetricLeaf,
    consecutive_losses: f64,
    monthly_returns: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedOutput {
    trades: Vec<ExpectedTrade>,
    equity: Vec<ExpectedEquity>,
    metrics: ExpectedMetrics,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorCase {
    id: String,
    input: CaseInput,
    expected_error_includes: String,
}

fn assert_close(path: &str, actual: f64, expected: f64, tolerance: NumericTolerance) {
    assert!(actual.is_finite(), "{path} must be finite, got {actual}");
    let difference = (actual - expected).abs();
    let relative_scale = actual.abs().max(expected.abs());
    assert!(
        difference <= tolerance.absolute || difference <= tolerance.relative * relative_scale,
        "{path} differs: actual={actual}, expected={expected}, diff={difference}"
    );
}

fn assert_leaf(path: &str, actual: f64, expected: &MetricLeaf, tolerance: NumericTolerance) {
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

fn assert_metrics(case_id: &str, actual: &Metrics, expected: &ExpectedMetrics, tol: NumericTolerance) {
    let p = |field: &str| format!("{case_id}.metrics.{field}");
    assert_leaf(&p("netReturn"), actual.net_return, &expected.net_return, tol);
    assert_leaf(&p("cagr"), actual.cagr, &expected.cagr, tol);
    assert_leaf(&p("maxDrawdown"), actual.max_drawdown, &expected.max_drawdown, tol);
    assert_leaf(&p("sharpe"), actual.sharpe, &expected.sharpe, tol);
    assert_leaf(&p("sortino"), actual.sortino, &expected.sortino, tol);
    assert_leaf(&p("calmar"), actual.calmar, &expected.calmar, tol);
    assert_leaf(&p("winRate"), actual.win_rate, &expected.win_rate, tol);
    assert_eq!(actual.trade_count as f64, expected.trade_count, "{}", p("tradeCount"));
    assert_leaf(&p("profitFactor"), actual.profit_factor, &expected.profit_factor, tol);
    assert_leaf(&p("avgTradeReturn"), actual.avg_trade_return, &expected.avg_trade_return, tol);
    assert_leaf(
        &p("medianTradeReturn"),
        actual.median_trade_return,
        &expected.median_trade_return,
        tol,
    );
    assert_leaf(&p("avgHoldingBars"), actual.avg_holding_bars, &expected.avg_holding_bars, tol);
    assert_leaf(&p("exposure"), actual.exposure, &expected.exposure, tol);
    assert_leaf(&p("turnover"), actual.turnover, &expected.turnover, tol);
    assert_leaf(&p("largestWin"), actual.largest_win, &expected.largest_win, tol);
    assert_leaf(&p("largestLoss"), actual.largest_loss, &expected.largest_loss, tol);
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

#[test]
fn rust_backtest_engine_matches_the_committed_typescript_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE_JSON).expect("parse backtest fixture");
    assert_eq!(fixture.schema_version, "rs-core-parity-fixture-v1");
    assert_eq!(fixture.fixture_version, "backtest-parity-v1");
    assert_eq!(fixture.contracts.candle, CANDLE_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.execution, EXECUTION_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.metrics, METRICS_CONTRACT_VERSION);
    assert!(fixture.cases.len() >= 17, "expected the full case set");

    let tolerance = fixture.tolerance.default;
    for case in &fixture.cases {
        let result = run_backtest(&case.input.candles, &case.input.signals, &case.input.config)
            .unwrap_or_else(|error| panic!("{}: engine failed: {error}", case.id));

        assert_eq!(
            result.trades.len(),
            case.expected.trades.len(),
            "{}: trade count",
            case.id
        );
        for (index, (actual, expected)) in
            result.trades.iter().zip(&case.expected.trades).enumerate()
        {
            let path = format!("{}.trades[{index}]", case.id);
            assert_eq!(actual.entry_time, expected.entry_time, "{path}.entryTime");
            assert_eq!(actual.exit_time, expected.exit_time, "{path}.exitTime");
            assert_eq!(actual.side, expected.side, "{path}.side");
            assert_eq!(actual.bars, expected.bars, "{path}.bars");
            assert_close(&format!("{path}.entryPrice"), actual.entry_price, expected.entry_price, tolerance);
            assert_close(&format!("{path}.exitPrice"), actual.exit_price, expected.exit_price, tolerance);
            assert_close(&format!("{path}.pnl"), actual.pnl, expected.pnl, tolerance);
            assert_close(&format!("{path}.pnlPct"), actual.pnl_pct, expected.pnl_pct, tolerance);
        }

        assert_eq!(
            result.equity.len(),
            case.expected.equity.len(),
            "{}: equity length",
            case.id
        );
        for (index, (actual, expected)) in
            result.equity.iter().zip(&case.expected.equity).enumerate()
        {
            let path = format!("{}.equity[{index}]", case.id);
            assert_eq!(actual.time, expected.time, "{path}.time");
            assert_close(&format!("{path}.equity"), actual.equity, expected.equity, tolerance);
        }

        assert_metrics(&case.id, &result.metrics, &case.expected.metrics, tolerance);
    }
}

#[test]
fn rust_backtest_engine_rejects_the_fixture_error_cases() {
    let fixture: Fixture = serde_json::from_str(FIXTURE_JSON).expect("parse backtest fixture");
    assert_eq!(fixture.error_cases.len(), 3);
    for case in &fixture.error_cases {
        let error = run_backtest(&case.input.candles, &case.input.signals, &case.input.config)
            .expect_err(&format!("{} must fail closed", case.id));
        assert!(
            error.to_string().contains(&case.expected_error_includes),
            "{}: error {error} must mention {}",
            case.id,
            case.expected_error_includes
        );
    }
}
