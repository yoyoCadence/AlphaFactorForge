use serde::Deserialize;

use super::backtest::{run_backtest, BacktestConfig, Signals, EXECUTION_CONTRACT_VERSION};
use super::metrics::METRICS_CONTRACT_VERSION;
use super::parity_support::{
    assert_equity, assert_metrics, assert_trades, ExpectedEquity, ExpectedMetrics, ExpectedTrade,
    TolerancePolicy,
};
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

#[test]
fn rust_backtest_engine_matches_the_committed_typescript_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE_JSON).expect("parse backtest fixture");
    assert_eq!(fixture.schema_version, "rs-core-parity-fixture-v1");
    assert_eq!(fixture.fixture_version, "backtest-parity-v1");
    assert_eq!(fixture.contracts.candle, CANDLE_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.execution, EXECUTION_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.metrics, METRICS_CONTRACT_VERSION);
    // Exact inventory (PR #69 review): an accidental case deletion must fail.
    let expected_ids = [
        "long-close-two-roundtrips",
        "long-close-costs-partial-sizing",
        "short-close-win-and-loss",
        "both-close-reversals",
        "long-nextopen-pending-and-final-bar",
        "both-nextopen-reversal",
        "long-stoploss-gap-through",
        "long-takeprofit-then-gap-up",
        "short-stoploss-and-takeprofit",
        "stoploss-wins-ambiguous-bar",
        "full-sizing-budgets-entry-fee",
        "eod-settles-open-position",
        "from-to-subrange",
        "single-bar-from-equals-to",
        "no-trades-zero-metrics",
        "empty-candles-boundary",
        "inverted-range-empty-evaluation",
        "rising-no-downside-infinite-ratios",
        "sample-daily-long-nextopen-risk",
        "sample-daily-both-close",
    ];
    let actual_ids: Vec<&str> = fixture.cases.iter().map(|case| case.id.as_str()).collect();
    assert_eq!(
        actual_ids, expected_ids,
        "case inventory must match exactly"
    );

    let tolerance = fixture.tolerance.default;
    for case in &fixture.cases {
        let result = run_backtest(&case.input.candles, &case.input.signals, &case.input.config)
            .unwrap_or_else(|error| panic!("{}: engine failed: {error}", case.id));

        assert_trades(&case.id, &result.trades, &case.expected.trades, tolerance);
        assert_equity(&case.id, &result.equity, &case.expected.equity, tolerance);
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
