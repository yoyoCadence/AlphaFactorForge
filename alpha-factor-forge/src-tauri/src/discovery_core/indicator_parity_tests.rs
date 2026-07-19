use serde::Deserialize;

use super::{
    indicators::{
        atr, bbands, ema, highest, lowest, macd, roc, rsi, sma, stddev, true_range, wma,
        INDICATOR_CONTRACT_VERSION,
    },
    types::{Candle, CANDLE_CONTRACT_VERSION},
};

const FIXTURE_JSON: &str = include_str!("../../../fixtures/rs-core/indicators-v1.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    schema_version: String,
    fixture_version: String,
    contracts: Contracts,
    tolerance: TolerancePolicy,
    cases: Vec<ParityCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contracts {
    candle: String,
    indicators: String,
    sample_input: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct NumericTolerance {
    absolute: f64,
    relative: f64,
}

#[derive(Debug, Deserialize)]
struct TolerancePolicy {
    default: NumericTolerance,
    exact: Vec<String>,
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
    parameters: Parameters,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Parameters {
    sma_period: usize,
    ema_period: usize,
    wma_period: usize,
    rsi_period: usize,
    macd_fast: usize,
    macd_slow: usize,
    macd_signal: usize,
    atr_period: usize,
    bbands_period: usize,
    bbands_mult: f64,
    stddev_period: usize,
    extrema_period: usize,
    roc_period: usize,
}

type ExpectedSeries = Vec<Option<f64>>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedOutput {
    sma: ExpectedSeries,
    ema: ExpectedSeries,
    wma: ExpectedSeries,
    rsi: ExpectedSeries,
    macd: ExpectedMacd,
    true_range: ExpectedSeries,
    atr: ExpectedSeries,
    bbands: ExpectedBands,
    stddev: ExpectedSeries,
    highest: ExpectedSeries,
    lowest: ExpectedSeries,
    roc: ExpectedSeries,
}

#[derive(Debug, Deserialize)]
struct ExpectedMacd {
    macd: ExpectedSeries,
    signal: ExpectedSeries,
    hist: ExpectedSeries,
}

#[derive(Debug, Deserialize)]
struct ExpectedBands {
    middle: ExpectedSeries,
    upper: ExpectedSeries,
    lower: ExpectedSeries,
}

fn assert_series(
    path: &str,
    actual: &[f64],
    expected: &[Option<f64>],
    tolerance: NumericTolerance,
) {
    assert_eq!(actual.len(), expected.len(), "{path} length mismatch");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        match expected {
            None => assert!(
                actual.is_nan(),
                "{path}[{index}] must preserve the exact warm-up null/NaN position"
            ),
            Some(expected) => {
                assert!(actual.is_finite(), "{path}[{index}] must be finite");
                let difference = (actual - expected).abs();
                let relative_scale = actual.abs().max(expected.abs());
                assert!(
                    difference <= tolerance.absolute
                        || difference <= tolerance.relative * relative_scale,
                    "{path}[{index}] differs: actual={actual}, expected={expected}, diff={difference}"
                );
            }
        }
    }
}

#[test]
fn rust_indicators_match_the_committed_typescript_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE_JSON).expect("parse parity fixture");
    assert_eq!(fixture.schema_version, "rs-core-parity-fixture-v1");
    assert_eq!(fixture.fixture_version, "indicator-parity-v1");
    assert_eq!(fixture.contracts.candle, CANDLE_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.indicators, INDICATOR_CONTRACT_VERSION);
    assert_eq!(fixture.contracts.sample_input, "sample-candles-v1");
    assert!(fixture
        .tolerance
        .exact
        .iter()
        .any(|rule| rule.contains("warm-up null")));
    assert_eq!(fixture.cases.len(), 1);

    let parity_case = &fixture.cases[0];
    assert_eq!(parity_case.id, "sample-seed-42-48-bars");
    assert_eq!(parity_case.input.candles.len(), 48);
    for pair in parity_case.input.candles.windows(2) {
        assert_eq!(pair[1].timestamp - pair[0].timestamp, 3_600_000);
    }

    let close: Vec<_> = parity_case
        .input
        .candles
        .iter()
        .map(|candle| candle.close)
        .collect();
    let high: Vec<_> = parity_case
        .input
        .candles
        .iter()
        .map(|candle| candle.high)
        .collect();
    let low: Vec<_> = parity_case
        .input
        .candles
        .iter()
        .map(|candle| candle.low)
        .collect();
    let parameters = &parity_case.input.parameters;
    let expected = &parity_case.expected;
    let tolerance = fixture.tolerance.default;

    assert_series(
        "sma",
        &sma(&close, parameters.sma_period),
        &expected.sma,
        tolerance,
    );
    assert_series(
        "ema",
        &ema(&close, parameters.ema_period),
        &expected.ema,
        tolerance,
    );
    assert_series(
        "wma",
        &wma(&close, parameters.wma_period),
        &expected.wma,
        tolerance,
    );
    assert_series(
        "rsi",
        &rsi(&close, parameters.rsi_period),
        &expected.rsi,
        tolerance,
    );

    let macd_output = macd(
        &close,
        parameters.macd_fast,
        parameters.macd_slow,
        parameters.macd_signal,
    );
    assert_series(
        "macd.macd",
        &macd_output.macd,
        &expected.macd.macd,
        tolerance,
    );
    assert_series(
        "macd.signal",
        &macd_output.signal,
        &expected.macd.signal,
        tolerance,
    );
    assert_series(
        "macd.hist",
        &macd_output.hist,
        &expected.macd.hist,
        tolerance,
    );

    assert_series(
        "trueRange",
        &true_range(&high, &low, &close).expect("aligned fixture OHLC"),
        &expected.true_range,
        tolerance,
    );
    assert_series(
        "atr",
        &atr(&high, &low, &close, parameters.atr_period).expect("aligned fixture OHLC"),
        &expected.atr,
        tolerance,
    );

    let bands = bbands(&close, parameters.bbands_period, parameters.bbands_mult);
    assert_series(
        "bbands.middle",
        &bands.middle,
        &expected.bbands.middle,
        tolerance,
    );
    assert_series(
        "bbands.upper",
        &bands.upper,
        &expected.bbands.upper,
        tolerance,
    );
    assert_series(
        "bbands.lower",
        &bands.lower,
        &expected.bbands.lower,
        tolerance,
    );

    assert_series(
        "stddev",
        &stddev(&close, parameters.stddev_period),
        &expected.stddev,
        tolerance,
    );
    assert_series(
        "highest",
        &highest(&high, parameters.extrema_period),
        &expected.highest,
        tolerance,
    );
    assert_series(
        "lowest",
        &lowest(&low, parameters.extrema_period),
        &expected.lowest,
        tolerance,
    );
    assert_series(
        "roc",
        &roc(&close, parameters.roc_period),
        &expected.roc,
        tolerance,
    );
}
