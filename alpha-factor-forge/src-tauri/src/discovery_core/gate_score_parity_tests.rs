use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use super::gate::{
    encode_gate_verdict, evaluate_gate, EncodedGateVerdict, EvaluateGateArgs, GateBenchmarkView,
    GateCandidateView, GateConfigOverrides, GateError, GateRandomEntryView, GateTradeEvidence,
    GATE_CONTRACT_VERSION,
};
use super::metrics::{EquityPoint, METRICS_CONTRACT_VERSION};
use super::parity_support::{assert_close, NumericTolerance};
use super::score::{
    complexity_units, score_candidate, ParamsStrategyProjection, ScoreCandidateArgs,
    ScoreCandidateView, ScoreCapsOverride, ScoreConfigOverride, ScoreError, ScoreWeightsOverride,
    SCORE_FORMULA_VERSION,
};

const FIXTURE_JSON: &str = include_str!("../../../fixtures/rs-core/gate-score-v1.json");

const COMPLEXITY_IDS: [&str; 6] = [
    "complexity-ma-family",
    "complexity-ema-family",
    "complexity-price-slow-family-one-risk",
    "complexity-rsi-family-two-risks",
    "complexity-macd-family",
    "complexity-bollinger-family",
];

const GATE_IDS: [&str; 22] = [
    "gate-default-pass",
    "gate-full-config-boundary-pass",
    "gate-partial-config-pass",
    "gate-min-trades-fail",
    "gate-unsafe-trade-count-fails-closed",
    "gate-nonfinite-trade-count-fails-closed",
    "gate-fractional-trade-count-fails-closed",
    "gate-negative-trade-count-fails-closed",
    "gate-avg-return-strict-tie",
    "gate-rolling-consistency-fail",
    "gate-max-drawdown-fail",
    "gate-monthly-concentration-fail",
    "gate-trade-concentration-fail",
    "gate-benchmark-strict-tie",
    "gate-random-percentile-fail",
    "gate-short-equity-fails-closed",
    "gate-nonpositive-profit-fails-closed",
    "gate-utc-month-boundary-pass",
    "gate-invalid-date-evidence-fails-closed",
    "gate-finite-ratio-overflow-fails-closed",
    "gate-nonfinite-statuses-fail-closed",
    "gate-nonfinite-derived-evidence-fails-closed",
];

const SCORE_IDS: [&str; 4] = [
    "score-default-baseline",
    "score-partial-config-population-sigma-max-safe-n",
    "score-nonfinite-statuses-negative-zero-insufficient",
    "score-extreme-finite-months-and-clamps",
];

const GATE_ERROR_IDS: [&str; 16] = [
    "gate-duplicate-benchmark",
    "gate-missing-benchmark",
    "gate-invalid-min-trades",
    "gate-fractional-min-trades",
    "gate-min-trades-above-safe-range",
    "gate-nonfinite-min-avg-return",
    "gate-invalid-rolling-window",
    "gate-fractional-rolling-window",
    "gate-rolling-window-above-safe-range",
    "gate-invalid-min-rolling-ratio",
    "gate-invalid-max-drawdown",
    "gate-invalid-monthly-contribution",
    "gate-invalid-single-trade-contribution",
    "gate-negative-percentile",
    "gate-invalid-percentile",
    "gate-nonfinite-percentile",
];

const SCORE_ERROR_IDS: [&str; 11] = [
    "score-invalid-cap-zero",
    "score-invalid-profit-factor-cap",
    "score-invalid-negative-weight",
    "score-invalid-nonfinite-weight",
    "score-invalid-nonfinite-cap",
    "score-deferred-regime-weight",
    "score-tested-combinations-zero",
    "score-tested-combinations-fractional",
    "score-tested-combinations-above-safe-range",
    "score-unsupported-stoch-signal",
    "score-resolved-weight-aggregate-overflow",
];

const EXACT_POLICY: [&str; 6] = [
    "schema, fixture, contract, provenance, and numeric-encoding versions",
    "case ids, inventory order, input keys, and input integers",
    "JSON object keys (order-insensitive), array order/length, booleans, strings, and nulls",
    "Gate criterion ids/order/pass/detail/valueStatus and discrete count values",
    "Score formula/segment/entry ids/order/rawStatus and evidence strings/counts",
    "complexity units/counts, testedCombinations integers, and error fragments",
];

const APPROXIMATE_POLICY: [&str; 2] = [
    "finite non-integer Gate values/config thresholds",
    "finite non-integer Score raw/normalized/weight/contribution/config/evidence values and score",
];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    fixture_version: String,
    contracts: Contracts,
    generator: Generator,
    numeric_encoding: NumericEncoding,
    tolerance: FixtureTolerance,
    complexity_cases: Vec<ComplexityCase>,
    gate_cases: Vec<GateCase>,
    score_cases: Vec<ScoreCase>,
    gate_error_cases: Vec<GateErrorCase>,
    score_error_cases: Vec<ScoreErrorCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Contracts {
    metrics: String,
    gate: String,
    score: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Generator {
    command: String,
    reference_runtime: String,
    source_hash_encoding: String,
    source_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NumericEncoding {
    special_input_numbers: String,
    expected_tolerant_floats: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct Tolerance {
    absolute: f64,
    relative: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureTolerance {
    default: Tolerance,
    exact: Vec<String>,
    approximate: Vec<String>,
}

impl From<Tolerance> for NumericTolerance {
    fn from(value: Tolerance) -> Self {
        Self {
            absolute: value.absolute,
            relative: value.relative,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum FixtureNumber {
    Number(f64),
    Tag(String),
}

impl FixtureNumber {
    fn decode(&self, path: &str) -> f64 {
        match self {
            Self::Number(value) => *value,
            Self::Tag(tag) => match tag.as_str() {
                "positive_infinity" => f64::INFINITY,
                "negative_infinity" => f64::NEG_INFINITY,
                "nan" => f64::NAN,
                "negative_zero" => -0.0,
                other => panic!("{path}: unknown fixture numeric tag {other}"),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComplexityCase {
    id: String,
    input: ComplexityInput,
    expected: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComplexityInput {
    strategy: StrategyInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrategyInput {
    entry_sig: String,
    exit_sig: String,
    sl_pct: FixtureNumber,
    tp_pct: FixtureNumber,
}

impl StrategyInput {
    fn decode(&self, case_id: &str) -> ParamsStrategyProjection {
        ParamsStrategyProjection {
            entry_sig: self.entry_sig.clone(),
            exit_sig: self.exit_sig.clone(),
            sl_pct: self.sl_pct.decode(&format!("{case_id}.strategy.slPct")),
            tp_pct: self.tp_pct.decode(&format!("{case_id}.strategy.tpPct")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GateCase {
    id: String,
    input: GateInput,
    expected: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateErrorCase {
    id: String,
    expected_error_includes: String,
    input: GateInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateInput {
    candidate: GateCandidateInput,
    benchmarks: Vec<GateBenchmarkInput>,
    random_entry_percentile: FixtureNumber,
    config: Option<GateConfigInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GateCandidateInput {
    metrics: GateMetricsInput,
    equity: Vec<FixtureNumber>,
    trades: Vec<GateTradeInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateMetricsInput {
    net_return: FixtureNumber,
    trade_count: FixtureNumber,
    avg_trade_return: FixtureNumber,
    max_drawdown: FixtureNumber,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateTradeInput {
    pnl: FixtureNumber,
    exit_time: FixtureNumber,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateBenchmarkInput {
    id: String,
    net_return: FixtureNumber,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateConfigInput {
    min_trades: Option<FixtureNumber>,
    min_avg_trade_return: Option<FixtureNumber>,
    rolling_window_bars: Option<FixtureNumber>,
    min_rolling_positive_ratio: Option<FixtureNumber>,
    max_drawdown: Option<FixtureNumber>,
    max_monthly_contribution: Option<FixtureNumber>,
    max_single_trade_contribution: Option<FixtureNumber>,
    min_random_entry_percentile: Option<FixtureNumber>,
}

impl GateConfigInput {
    fn decode(&self, case_id: &str) -> GateConfigOverrides {
        let number = |field: &str, value: &Option<FixtureNumber>| {
            value
                .as_ref()
                .map(|value| value.decode(&format!("{case_id}.config.{field}")))
        };
        GateConfigOverrides {
            min_trades: number("minTrades", &self.min_trades),
            min_avg_trade_return: number("minAvgTradeReturn", &self.min_avg_trade_return),
            rolling_window_bars: number("rollingWindowBars", &self.rolling_window_bars),
            min_rolling_positive_ratio: number(
                "minRollingPositiveRatio",
                &self.min_rolling_positive_ratio,
            ),
            max_drawdown: number("maxDrawdown", &self.max_drawdown),
            max_monthly_contribution: number(
                "maxMonthlyContribution",
                &self.max_monthly_contribution,
            ),
            max_single_trade_contribution: number(
                "maxSingleTradeContribution",
                &self.max_single_trade_contribution,
            ),
            min_random_entry_percentile: number(
                "minRandomEntryPercentile",
                &self.min_random_entry_percentile,
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreCase {
    id: String,
    input: ScoreInput,
    expected: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScoreErrorCase {
    id: String,
    expected_error_includes: String,
    input: ScoreInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScoreInput {
    validation: ScoreValidationInput,
    strategy: StrategyInput,
    tested_combinations: FixtureNumber,
    config: Option<ScoreConfigInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScoreValidationInput {
    metrics: ScoreMetricsInput,
    closed_trade_count: usize,
    total_bars: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScoreMetricsInput {
    cagr: FixtureNumber,
    sortino: FixtureNumber,
    calmar: FixtureNumber,
    profit_factor: FixtureNumber,
    turnover: FixtureNumber,
    monthly_returns: BTreeMap<String, FixtureNumber>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreConfigInput {
    caps: Option<BTreeMap<String, FixtureNumber>>,
    weights: Option<BTreeMap<String, FixtureNumber>>,
}

impl ScoreConfigInput {
    fn decode(&self, case_id: &str) -> ScoreConfigOverride {
        let mut caps = ScoreCapsOverride::default();
        if let Some(values) = &self.caps {
            for (key, encoded) in values {
                let value = encoded.decode(&format!("{case_id}.config.caps.{key}"));
                match key.as_str() {
                    "cagr" => caps.cagr = Some(value),
                    "sortino" => caps.sortino = Some(value),
                    "calmar" => caps.calmar = Some(value),
                    "profitFactor" => caps.profit_factor = Some(value),
                    "consistencySigmaScale" => caps.consistency_sigma_scale = Some(value),
                    "complexityUnits" => caps.complexity_units = Some(value),
                    "turnover" => caps.turnover = Some(value),
                    "dataMiningLog10" => caps.data_mining_log10 = Some(value),
                    other => panic!("{case_id}.config.caps: unknown key {other}"),
                }
            }
        }

        let mut weights = ScoreWeightsOverride::default();
        if let Some(values) = &self.weights {
            for (key, encoded) in values {
                let value = encoded.decode(&format!("{case_id}.config.weights.{key}"));
                match key.as_str() {
                    "cagr" => weights.cagr = Some(value),
                    "sortino" => weights.sortino = Some(value),
                    "calmar" => weights.calmar = Some(value),
                    "regime" => weights.regime = Some(value),
                    "profitFactor" => weights.profit_factor = Some(value),
                    "consistency" => weights.consistency = Some(value),
                    "complexity" => weights.complexity = Some(value),
                    "turnover" => weights.turnover = Some(value),
                    "dataMining" => weights.data_mining = Some(value),
                    other => panic!("{case_id}.config.weights: unknown key {other}"),
                }
            }
        }

        ScoreConfigOverride {
            caps: self.caps.as_ref().map(|_| caps),
            weights: self.weights.as_ref().map(|_| weights),
        }
    }
}

fn parse_fixture() -> Fixture {
    serde_json::from_str(FIXTURE_JSON).expect("parse Gate + Score parity fixture")
}

fn evaluate_gate_fixture(
    case_id: &str,
    input: &GateInput,
) -> Result<EncodedGateVerdict, GateError> {
    let equity: Vec<EquityPoint> = input
        .candidate
        .equity
        .iter()
        .enumerate()
        .map(|(time, value)| EquityPoint {
            time: time as i64,
            equity: value.decode(&format!("{case_id}.candidate.equity[{time}]")),
        })
        .collect();
    let trades: Vec<GateTradeEvidence> = input
        .candidate
        .trades
        .iter()
        .enumerate()
        .map(|(index, trade)| GateTradeEvidence {
            exit_time: trade
                .exit_time
                .decode(&format!("{case_id}.candidate.trades[{index}].exitTime")),
            pnl: trade
                .pnl
                .decode(&format!("{case_id}.candidate.trades[{index}].pnl")),
        })
        .collect();
    let benchmarks: Vec<GateBenchmarkView<'_>> = input
        .benchmarks
        .iter()
        .enumerate()
        .map(|(index, benchmark)| GateBenchmarkView {
            id: &benchmark.id,
            net_return: benchmark
                .net_return
                .decode(&format!("{case_id}.benchmarks[{index}].netReturn")),
        })
        .collect();
    let config = input.config.as_ref().map(|config| config.decode(case_id));
    let metrics = &input.candidate.metrics;
    let args = EvaluateGateArgs {
        candidate: GateCandidateView {
            trades,
            equity: &equity,
            net_return: metrics
                .net_return
                .decode(&format!("{case_id}.candidate.metrics.netReturn")),
            trade_count: metrics
                .trade_count
                .decode(&format!("{case_id}.candidate.metrics.tradeCount")),
            avg_trade_return: metrics
                .avg_trade_return
                .decode(&format!("{case_id}.candidate.metrics.avgTradeReturn")),
            max_drawdown: metrics
                .max_drawdown
                .decode(&format!("{case_id}.candidate.metrics.maxDrawdown")),
        },
        benchmarks: &benchmarks,
        random_entry: GateRandomEntryView {
            candidate_percentile: input
                .random_entry_percentile
                .decode(&format!("{case_id}.randomEntryPercentile")),
        },
        config,
    };
    evaluate_gate(&args).map(|verdict| encode_gate_verdict(&verdict))
}

fn decode_monthly_returns(case_id: &str, metrics: &ScoreMetricsInput) -> BTreeMap<String, f64> {
    metrics
        .monthly_returns
        .iter()
        .map(|(month, value)| {
            (
                month.clone(),
                value.decode(&format!(
                    "{case_id}.validation.metrics.monthlyReturns[{month}]"
                )),
            )
        })
        .collect()
}

fn score_fixture(
    case_id: &str,
    input: &ScoreInput,
) -> Result<super::score::ScoreBreakdown, ScoreError> {
    let monthly_returns = decode_monthly_returns(case_id, &input.validation.metrics);
    let metrics = &input.validation.metrics;
    let strategy = input.strategy.decode(case_id);
    let config = input.config.as_ref().map(|config| config.decode(case_id));
    let args = ScoreCandidateArgs {
        candidate: ScoreCandidateView {
            cagr: metrics
                .cagr
                .decode(&format!("{case_id}.validation.metrics.cagr")),
            sortino: metrics
                .sortino
                .decode(&format!("{case_id}.validation.metrics.sortino")),
            calmar: metrics
                .calmar
                .decode(&format!("{case_id}.validation.metrics.calmar")),
            profit_factor: metrics
                .profit_factor
                .decode(&format!("{case_id}.validation.metrics.profitFactor")),
            turnover: metrics
                .turnover
                .decode(&format!("{case_id}.validation.metrics.turnover")),
            monthly_returns: &monthly_returns,
            closed_trade_count: input.validation.closed_trade_count,
            total_bars: input.validation.total_bars,
        },
        strategy: &strategy,
        tested_combinations: input
            .tested_combinations
            .decode(&format!("{case_id}.testedCombinations")),
        config: config.as_ref(),
    };
    score_candidate(&args)
}

/// JSON structure is exact (object key sets, array order/length, scalar kind,
/// strings, booleans, and null). Expected integer tokens compare exactly,
/// including MAX_SAFE boundaries. Only finite non-integer tokens use the
/// fixture's reviewed abs/relative tolerance.
fn assert_json_parity(path: &str, actual: &Value, expected: &Value, tolerance: NumericTolerance) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual
                .as_f64()
                .unwrap_or_else(|| panic!("{path}: actual JSON number is not f64-compatible"));
            let expected_value = expected
                .as_f64()
                .unwrap_or_else(|| panic!("{path}: expected JSON number is not f64-compatible"));
            if expected.is_i64() || expected.is_u64() {
                assert_eq!(actual, expected_value, "{path}: exact integer leaf");
                if expected_value == 0.0 {
                    assert!(
                        !actual.is_sign_negative(),
                        "{path}: expected canonical positive zero, got negative zero"
                    );
                }
            } else {
                assert_close(path, actual, expected_value, tolerance);
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}: array length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_json_parity(&format!("{path}[{index}]"), actual, expected, tolerance);
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}: object key count");
            for (key, expected) in expected {
                let actual = actual
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}: missing object key {key}"));
                assert_json_parity(&format!("{path}.{key}"), actual, expected, tolerance);
            }
        }
        _ => assert_eq!(actual, expected, "{path}: exact structural leaf"),
    }
}

fn ids<'a, T>(cases: &'a [T], id: impl Fn(&'a T) -> &'a str) -> Vec<&'a str> {
    cases.iter().map(id).collect()
}

#[test]
fn gate_score_fixture_metadata_and_inventories_are_exact() {
    let fixture = parse_fixture();
    assert_eq!(fixture.schema_version, "rs-core-parity-fixture-v1");
    assert_eq!(fixture.fixture_version, "gate-score-parity-v1");
    assert_eq!(fixture.contracts.metrics, METRICS_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.gate, GATE_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.score, SCORE_FORMULA_VERSION);
    assert_eq!(fixture.generator.command, "npm run fixtures:gate-score");
    assert_eq!(fixture.generator.reference_runtime, "typescript");
    assert_eq!(fixture.generator.source_hash_encoding, "utf8-lf-v1");
    let source_keys: Vec<&str> = fixture
        .generator
        .source_hashes
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        source_keys,
        [
            "benchmarks",
            "gate",
            "generator",
            "metricsCodec",
            "nonFinite",
            "score",
            "strategy",
            "validationRecord",
        ]
    );
    for (name, hash) in &fixture.generator.source_hashes {
        assert!(
            hash.starts_with("sha256:") && hash.len() == 71,
            "source hash {name} must be sha256:<64 hex>"
        );
        assert!(
            hash[7..].bytes().all(|byte| byte.is_ascii_hexdigit()),
            "source hash {name} must be hexadecimal"
        );
    }
    assert_eq!(
        fixture.numeric_encoding.special_input_numbers,
        "explicit-numeric-status-v1"
    );
    assert_eq!(
        fixture.numeric_encoding.expected_tolerant_floats,
        "decimal-significant-15-v1"
    );
    assert_eq!(fixture.tolerance.default.absolute, 1e-12);
    assert_eq!(fixture.tolerance.default.relative, 1e-10);
    assert_eq!(fixture.tolerance.exact, EXACT_POLICY.map(str::to_string));
    assert_eq!(
        fixture.tolerance.approximate,
        APPROXIMATE_POLICY.map(str::to_string)
    );

    assert_eq!(
        ids(&fixture.complexity_cases, |case| &case.id),
        COMPLEXITY_IDS
    );
    assert_eq!(ids(&fixture.gate_cases, |case| &case.id), GATE_IDS);
    assert_eq!(ids(&fixture.score_cases, |case| &case.id), SCORE_IDS);
    assert_eq!(
        ids(&fixture.gate_error_cases, |case| &case.id),
        GATE_ERROR_IDS
    );
    assert_eq!(
        ids(&fixture.score_error_cases, |case| &case.id),
        SCORE_ERROR_IDS
    );
}

#[test]
fn rust_params_complexity_matches_the_typescript_fixture() {
    let fixture = parse_fixture();
    let tolerance = fixture.tolerance.default.into();
    for case in &fixture.complexity_cases {
        let strategy = case.input.strategy.decode(&case.id);
        let actual = complexity_units(&strategy)
            .unwrap_or_else(|error| panic!("{}: complexity failed: {error}", case.id));
        let actual = serde_json::to_value(actual).expect("serialize complexity result");
        assert_json_parity(&case.id, &actual, &case.expected, tolerance);
    }
}

#[test]
fn rust_encoded_gate_verdicts_match_the_typescript_fixture() {
    let fixture = parse_fixture();
    let tolerance = fixture.tolerance.default.into();
    for case in &fixture.gate_cases {
        let actual = evaluate_gate_fixture(&case.id, &case.input)
            .unwrap_or_else(|error| panic!("{}: Gate failed: {error}", case.id));
        let actual = serde_json::to_value(actual).expect("serialize encoded Gate verdict");
        assert_json_parity(&case.id, &actual, &case.expected, tolerance);
    }
}

#[test]
fn rust_score_breakdowns_match_the_typescript_fixture() {
    let fixture = parse_fixture();
    let tolerance = fixture.tolerance.default.into();
    for case in &fixture.score_cases {
        let actual = score_fixture(&case.id, &case.input)
            .unwrap_or_else(|error| panic!("{}: Score failed: {error}", case.id));
        let actual = serde_json::to_value(actual).expect("serialize Score breakdown");
        assert_json_parity(&case.id, &actual, &case.expected, tolerance);
    }
}

#[test]
fn rust_gate_and_score_reject_every_typescript_held_error_case() {
    let fixture = parse_fixture();

    for case in &fixture.gate_error_cases {
        let error = evaluate_gate_fixture(&case.id, &case.input)
            .err()
            .unwrap_or_else(|| panic!("{} must fail closed", case.id));
        assert!(
            error.to_string().contains(&case.expected_error_includes),
            "{}: error {error} must mention {}",
            case.id,
            case.expected_error_includes
        );
    }

    for case in &fixture.score_error_cases {
        let error = score_fixture(&case.id, &case.input)
            .err()
            .unwrap_or_else(|| panic!("{} must fail closed", case.id));
        assert!(
            error.to_string().contains(&case.expected_error_includes),
            "{}: error {error} must mention {}",
            case.id,
            case.expected_error_includes
        );
    }
}
