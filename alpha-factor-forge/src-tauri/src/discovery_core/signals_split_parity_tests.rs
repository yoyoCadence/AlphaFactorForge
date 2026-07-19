use serde::Deserialize;

use super::embargo::{derive_embargo_bars, EmbargoDerivation, EMBARGO_CONTRACT_VERSION};
use super::signals::{build_params_signals, ParamsSignalConfig, PARAMS_SIGNALS_CONTRACT_VERSION};
use super::split::{plan_validation_split, ValidationSplitPlan, SPLIT_CONTRACT_VERSION};
use super::types::{Candle, CANDLE_CONTRACT_VERSION};

const FIXTURE_JSON: &str = include_str!("../../../fixtures/rs-core/signals-split-v1.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    schema_version: String,
    fixture_version: String,
    contracts: Contracts,
    signal_cases: Vec<SignalCase>,
    split_cases: Vec<SplitCase>,
    embargo_cases: Vec<EmbargoCase>,
    signal_error_cases: Vec<SignalErrorCase>,
    split_error_cases: Vec<SplitErrorCase>,
    embargo_error_cases: Vec<EmbargoErrorCase>,
}

#[derive(Debug, Deserialize)]
struct Contracts {
    candle: String,
    signals: String,
    split: String,
    embargo: String,
}

#[derive(Debug, Deserialize)]
struct SignalCase {
    id: String,
    input: SignalInput,
    expected: ExpectedSignals,
}

#[derive(Debug, Deserialize)]
struct SignalInput {
    candles: Vec<Candle>,
    config: ParamsSignalConfig,
}

#[derive(Debug, Deserialize)]
struct ExpectedSignals {
    entry: Vec<bool>,
    exit: Vec<bool>,
}

#[derive(Debug, Deserialize)]
struct SplitCase {
    id: String,
    input: SplitInput,
    expected: ValidationSplitPlan,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SplitInput {
    total_bars: i64,
    embargo_bars: i64,
}

#[derive(Debug, Deserialize)]
struct EmbargoCase {
    id: String,
    input: EmbargoInput,
    expected: EmbargoDerivation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbargoInput {
    config: ParamsSignalConfig,
    holding_allowance_bars: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalErrorCase {
    id: String,
    input: SignalInput,
    expected_error_includes: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SplitErrorCase {
    id: String,
    input: SplitInput,
    expected_error_includes: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbargoErrorCase {
    id: String,
    input: EmbargoInput,
    expected_error_includes: String,
}

fn parse_fixture() -> Fixture {
    let fixture: Fixture = serde_json::from_str(FIXTURE_JSON).expect("parse signals/split fixture");
    assert_eq!(fixture.schema_version, "rs-core-parity-fixture-v1");
    assert_eq!(fixture.fixture_version, "signals-split-parity-v1");
    assert_eq!(fixture.contracts.candle, CANDLE_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.signals, PARAMS_SIGNALS_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.split, SPLIT_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.embargo, EMBARGO_CONTRACT_VERSION);
    fixture
}

#[test]
fn rust_params_signals_match_the_committed_typescript_fixture() {
    let fixture = parse_fixture();
    let expected_ids = [
        "hand-ma-cross-exact-index",
        "sample-ma-cross",
        "sample-ema-cross",
        "sample-price-vs-slow",
        "sample-rsi-thresholds",
        "sample-macd-cross",
        "sample-bollinger-touch",
    ];
    let actual_ids: Vec<&str> = fixture
        .signal_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(actual_ids, expected_ids, "signal case inventory");

    for case in &fixture.signal_cases {
        let signals = build_params_signals(&case.input.candles, &case.input.config)
            .unwrap_or_else(|error| panic!("{}: signal build failed: {error}", case.id));
        assert_eq!(signals.entry, case.expected.entry, "{}: entry", case.id);
        assert_eq!(signals.exit, case.expected.exit, "{}: exit", case.id);
    }
}

#[test]
fn rust_split_plans_match_the_committed_typescript_fixture() {
    let fixture = parse_fixture();
    let expected_ids = [
        "split-minimal-five",
        "split-residue-0",
        "split-residue-1",
        "split-residue-2",
        "split-residue-3",
        "split-residue-4",
        "split-with-embargo",
        "split-embargo-residue",
        "split-max-safe-integer",
    ];
    let actual_ids: Vec<&str> = fixture
        .split_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(actual_ids, expected_ids, "split case inventory");

    for case in &fixture.split_cases {
        let plan = plan_validation_split(case.input.total_bars, case.input.embargo_bars)
            .unwrap_or_else(|error| panic!("{}: split failed: {error}", case.id));
        assert_eq!(plan, case.expected, "{}", case.id);
    }
}

#[test]
fn rust_embargo_derivations_match_the_committed_typescript_fixture() {
    let fixture = parse_fixture();
    let expected_ids = [
        "embargo-default-ma-cross",
        "embargo-macd-exit",
        "embargo-rsi-pair",
        "embargo-bollinger-pair",
        "embargo-price-vs-slow",
        "embargo-with-allowance",
        "embargo-unused-period-ignored",
    ];
    let actual_ids: Vec<&str> = fixture
        .embargo_cases
        .iter()
        .map(|case| case.id.as_str())
        .collect();
    assert_eq!(actual_ids, expected_ids, "embargo case inventory");

    for case in &fixture.embargo_cases {
        let derivation = derive_embargo_bars(&case.input.config, case.input.holding_allowance_bars)
            .unwrap_or_else(|error| panic!("{}: derivation failed: {error}", case.id));
        assert_eq!(derivation, case.expected, "{}", case.id);
    }
}

#[test]
fn rust_rejects_every_committed_error_case() {
    let fixture = parse_fixture();

    assert_eq!(fixture.signal_error_cases.len(), 1);
    for case in &fixture.signal_error_cases {
        let error = build_params_signals(&case.input.candles, &case.input.config)
            .expect_err(&format!("{} must fail closed", case.id));
        assert!(
            error.to_string().contains(&case.expected_error_includes),
            "{}: error {error} must mention {}",
            case.id,
            case.expected_error_includes
        );
    }

    assert_eq!(fixture.split_error_cases.len(), 4);
    for case in &fixture.split_error_cases {
        let error = plan_validation_split(case.input.total_bars, case.input.embargo_bars)
            .expect_err(&format!("{} must fail closed", case.id));
        assert!(
            error.to_string().contains(&case.expected_error_includes),
            "{}: error {error} must mention {}",
            case.id,
            case.expected_error_includes
        );
    }

    assert_eq!(fixture.embargo_error_cases.len(), 3);
    for case in &fixture.embargo_error_cases {
        let error = derive_embargo_bars(&case.input.config, case.input.holding_allowance_bars)
            .expect_err(&format!("{} must fail closed", case.id));
        assert!(
            error.to_string().contains(&case.expected_error_includes),
            "{}: error {error} must mention {}",
            case.id,
            case.expected_error_includes
        );
    }
}
