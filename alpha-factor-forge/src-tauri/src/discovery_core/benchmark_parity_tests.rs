use serde::Deserialize;

use super::backtest::{BacktestError, EXECUTION_CONTRACT_VERSION};
use super::benchmarks::{
    run_deterministic_benchmarks, BenchmarkCosts, RunBenchmarksArgs, BENCHMARK_CONTRACT_VERSION,
    DETERMINISTIC_BENCHMARK_IDS,
};
use super::metrics::METRICS_CONTRACT_VERSION;
use super::parity_support::{
    assert_close, assert_equity, assert_metrics, assert_trades, ExpectedEquity, ExpectedMetrics,
    ExpectedTrade, TolerancePolicy,
};
use super::prng::{Mulberry32, PRNG_CONTRACT_VERSION};
use super::random_entry::{
    plan_random_trades, run_random_entry_benchmark, PlannedRandomTrade, RandomEntryArgs,
    RandomEntryCandidate, RANDOM_ENTRY_CONTRACT_VERSION,
};
use super::signals::PARAMS_SIGNALS_CONTRACT_VERSION;
use super::types::{Candle, CANDLE_CONTRACT_VERSION};

const FIXTURE_JSON: &str = include_str!("../../../fixtures/rs-core/benchmark-v1.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    schema_version: String,
    fixture_version: String,
    contracts: Contracts,
    tolerance: TolerancePolicy,
    prng_cases: Vec<PrngCase>,
    suite_cases: Vec<SuiteCase>,
    planner_cases: Vec<PlannerCase>,
    random_entry_cases: Vec<RandomEntryCase>,
    benchmark_error_cases: Vec<BenchmarkErrorCase>,
    random_entry_error_cases: Vec<RandomEntryErrorCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contracts {
    candle: String,
    execution: String,
    metrics: String,
    signals: String,
    prng: String,
    benchmarks: String,
    random_entry: String,
}

#[derive(Debug, Deserialize)]
struct PrngCase {
    id: String,
    input: PrngInput,
    expected: ExpectedPrng,
}

#[derive(Debug, Deserialize)]
struct PrngInput {
    seed: i64,
    count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedPrng {
    raw_u32: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct SuiteCase {
    id: String,
    input: SuiteInput,
    expected: ExpectedSuite,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SuiteInput {
    candles: Vec<Candle>,
    interval: String,
    costs: BenchmarkCosts,
    start_equity: Option<f64>,
    from: Option<i64>,
    to: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ExpectedSuite {
    benchmarks: Vec<ExpectedBenchmark>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedBenchmark {
    id: String,
    strat_is_null: bool,
    strat: serde_json::Value,
    result: ExpectedResult,
}

#[derive(Debug, Deserialize)]
struct ExpectedResult {
    trades: Vec<ExpectedTrade>,
    equity: Vec<ExpectedEquity>,
    metrics: ExpectedMetrics,
}

#[derive(Debug, Deserialize)]
struct PlannerCase {
    id: String,
    input: PlannerInput,
    expected: ExpectedPlan,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlannerInput {
    seed: i64,
    from: i64,
    to: i64,
    holding_pool: Vec<i64>,
    trade_count: i64,
}

#[derive(Debug, Deserialize)]
struct ExpectedPlan {
    planned: Vec<PlannedRandomTrade>,
}

#[derive(Debug, Deserialize)]
struct RandomEntryCase {
    id: String,
    input: RandomEntryInput,
    expected: ExpectedRandomEntry,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RandomEntryInput {
    candles: Vec<Candle>,
    interval: String,
    costs: BenchmarkCosts,
    candidate: RandomEntryCandidate,
    seed: i64,
    runs: Option<i64>,
    #[serde(default)]
    start_equity: Option<f64>,
    #[serde(default)]
    from: Option<i64>,
    #[serde(default)]
    to: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedRandomEntry {
    runs: i64,
    seed: i64,
    net_returns: Vec<f64>,
    candidate_net_return: f64,
    candidate_percentile: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkErrorCase {
    id: String,
    input: SuiteInput,
    expected_error_includes: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RandomEntryErrorCase {
    id: String,
    input: RandomEntryInput,
    expected_error_includes: String,
}

fn parse_fixture() -> Fixture {
    let fixture: Fixture = serde_json::from_str(FIXTURE_JSON).expect("parse benchmark fixture");
    assert_eq!(fixture.schema_version, "rs-core-parity-fixture-v1");
    assert_eq!(fixture.fixture_version, "benchmark-parity-v1");
    assert_eq!(fixture.contracts.candle, CANDLE_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.execution, EXECUTION_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.metrics, METRICS_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.signals, PARAMS_SIGNALS_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.prng, PRNG_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.benchmarks, BENCHMARK_CONTRACT_VERSION);
    assert_eq!(
        fixture.contracts.random_entry,
        RANDOM_ENTRY_CONTRACT_VERSION
    );
    fixture
}

fn benchmark_args(input: &SuiteInput) -> RunBenchmarksArgs<'_> {
    RunBenchmarksArgs {
        candles: &input.candles,
        interval: &input.interval,
        costs: input.costs,
        start_equity: input.start_equity,
        from: input.from,
        to: input.to,
    }
}

fn random_entry_args(input: &RandomEntryInput) -> RandomEntryArgs<'_> {
    RandomEntryArgs {
        candles: &input.candles,
        interval: &input.interval,
        costs: input.costs,
        candidate: &input.candidate,
        seed: input.seed,
        runs: input.runs,
        start_equity: input.start_equity,
        from: input.from,
        to: input.to,
    }
}

fn require_error<T>(result: Result<T, BacktestError>, id: &str) -> BacktestError {
    match result {
        Ok(_) => panic!("{id} must fail closed"),
        Err(error) => error,
    }
}

fn assert_json_structure(
    path: &str,
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    tolerance: super::parity_support::NumericTolerance,
) {
    match (actual, expected) {
        (serde_json::Value::Number(actual), serde_json::Value::Number(expected)) => assert_close(
            path,
            actual.as_f64().expect("actual JSON number"),
            expected.as_f64().expect("expected JSON number"),
            tolerance,
        ),
        (serde_json::Value::Array(actual), serde_json::Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}: array length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_json_structure(&format!("{path}[{index}]"), actual, expected, tolerance);
            }
        }
        (serde_json::Value::Object(actual), serde_json::Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}: object key count");
            for (key, expected) in expected {
                let actual = actual
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}: missing key {key}"));
                assert_json_structure(&format!("{path}.{key}"), actual, expected, tolerance);
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn rust_mulberry32_matches_the_exact_raw_u32_fixture() {
    let fixture = parse_fixture();
    let expected_ids = [
        "prng-seed-42",
        "prng-seed-7",
        "prng-seed-u32-max",
        "prng-seed-123",
        "prng-seed-truncated-2pow32-plus-123",
    ];
    let actual_ids: Vec<&str> = fixture
        .prng_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(actual_ids, expected_ids, "PRNG case inventory");

    for case in &fixture.prng_cases {
        let mut rng = Mulberry32::from_truncated(case.input.seed);
        let actual: Vec<u32> = (0..case.input.count).map(|_| rng.next_u32()).collect();
        assert_eq!(actual, case.expected.raw_u32, "{}", case.id);
    }
}

#[test]
fn rust_deterministic_benchmarks_match_the_typescript_fixture() {
    let fixture = parse_fixture();
    let expected_ids = [
        "suite-small-no-cost",
        "suite-sample-daily-costs",
        "suite-sma-cross-trades",
        "suite-subrange-prototype-key-interval",
    ];
    let actual_ids: Vec<&str> = fixture
        .suite_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(actual_ids, expected_ids, "benchmark suite case inventory");

    let tolerance = fixture.tolerance.default;
    for case in &fixture.suite_cases {
        let runs = run_deterministic_benchmarks(&benchmark_args(&case.input))
            .unwrap_or_else(|error| panic!("{}: benchmark suite failed: {error}", case.id));
        let expected_run_ids: Vec<&str> = case
            .expected
            .benchmarks
            .iter()
            .map(|run| run.id.as_str())
            .collect();
        assert_eq!(expected_run_ids, DETERMINISTIC_BENCHMARK_IDS, "{}", case.id);
        assert_eq!(runs.len(), case.expected.benchmarks.len(), "{}", case.id);

        for (actual, expected) in runs.iter().zip(&case.expected.benchmarks) {
            let path = format!("{}.{}", case.id, expected.id);
            assert_eq!(actual.id, expected.id, "{path}.id");
            assert_eq!(
                actual.strat.is_none(),
                expected.strat_is_null,
                "{path}.strat"
            );
            assert_json_structure(
                &format!("{path}.strat"),
                &serde_json::to_value(&actual.strat).expect("serialize benchmark strategy"),
                &expected.strat,
                tolerance,
            );
            assert_trades(
                &path,
                &actual.result.trades,
                &expected.result.trades,
                tolerance,
            );
            assert_equity(
                &path,
                &actual.result.equity,
                &expected.result.equity,
                tolerance,
            );
            assert_metrics(
                &path,
                &actual.result.metrics,
                &expected.result.metrics,
                tolerance,
            );
        }
    }
}

#[test]
fn rust_random_entry_plans_and_distributions_match_the_typescript_fixture() {
    let fixture = parse_fixture();
    let planner_ids: Vec<&str> = fixture
        .planner_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(planner_ids, ["planner-basic", "planner-clip-and-drop"]);
    for case in &fixture.planner_cases {
        let mut rng = Mulberry32::from_truncated(case.input.seed);
        let actual = plan_random_trades(
            &mut rng,
            case.input.from,
            case.input.to,
            &case.input.holding_pool,
            case.input.trade_count,
        );
        assert_eq!(actual, case.expected.planned, "{}", case.id);
    }

    let random_ids: Vec<&str> = fixture
        .random_entry_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(
        random_ids,
        [
            "random-entry-fake-candidate",
            "random-entry-real-candidate",
            "random-entry-zero-bar-clamp-subrange",
            "random-entry-flat-tie-default-runs",
            "random-entry-min-seed-min-runs",
            "random-entry-max-seed-max-runs",
        ]
    );
    let tolerance = fixture.tolerance.default;
    for case in &fixture.random_entry_cases {
        let actual = run_random_entry_benchmark(&random_entry_args(&case.input))
            .unwrap_or_else(|error| panic!("{}: Random Entry failed: {error}", case.id));
        assert_eq!(actual.runs, case.expected.runs, "{}.runs", case.id);
        assert_eq!(actual.seed, case.expected.seed, "{}.seed", case.id);
        assert_eq!(
            actual.net_returns.len(),
            case.expected.net_returns.len(),
            "{}.netReturns length",
            case.id
        );
        for (index, (value, expected)) in actual
            .net_returns
            .iter()
            .zip(&case.expected.net_returns)
            .enumerate()
        {
            assert_close(
                &format!("{}.netReturns[{index}]", case.id),
                *value,
                *expected,
                tolerance,
            );
        }
        assert_close(
            &format!("{}.candidateNetReturn", case.id),
            actual.candidate_net_return,
            case.expected.candidate_net_return,
            tolerance,
        );
        assert_close(
            &format!("{}.candidatePercentile", case.id),
            actual.candidate_percentile,
            case.expected.candidate_percentile,
            tolerance,
        );
    }
}

#[test]
fn rust_benchmarks_reject_the_typescript_held_error_cases() {
    let fixture = parse_fixture();
    let benchmark_ids: Vec<&str> = fixture
        .benchmark_error_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(benchmark_ids, ["benchmarks-empty-candles"]);
    for case in &fixture.benchmark_error_cases {
        let error = require_error(
            run_deterministic_benchmarks(&benchmark_args(&case.input)),
            &case.id,
        );
        assert!(
            error.to_string().contains(&case.expected_error_includes),
            "{}: error {error} must mention {}",
            case.id,
            case.expected_error_includes
        );
    }

    let random_ids: Vec<&str> = fixture
        .random_entry_error_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(
        random_ids,
        [
            "random-entry-zero-runs",
            "random-entry-runs-above-cap",
            "random-entry-negative-seed",
            "random-entry-seed-above-safe-range",
            "random-entry-empty-candles",
            "random-entry-inverted-segment",
            "random-entry-no-candidate-trades",
        ]
    );
    for case in &fixture.random_entry_error_cases {
        let error = require_error(
            run_random_entry_benchmark(&random_entry_args(&case.input)),
            &case.id,
        );
        assert!(
            error.to_string().contains(&case.expected_error_includes),
            "{}: error {error} must mention {}",
            case.id,
            case.expected_error_includes
        );
    }
}
