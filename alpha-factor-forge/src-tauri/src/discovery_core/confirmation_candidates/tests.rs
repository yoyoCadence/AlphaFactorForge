use serde_json::{json, Value};

use super::*;
use crate::discovery_core::confirmation::{evaluate_confirmation, parse_confirmation_declaration};

const FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-confirmation-candidates-draft-v1.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

fn floats(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|number| number.as_f64().unwrap())
        .collect()
}

type Evaluate = fn(
    &ConfirmationDeclaration,
    u64,
    &[CandidateSeries<'_>],
) -> Result<ConfirmationReport, ConfirmationError>;

const CANDIDATES: [(&str, Evaluate); 2] = [
    ("S1", evaluate_confirmation_candidate_s1),
    ("S2", evaluate_confirmation_candidate_s2),
];

fn statistic(name: &Value) -> Evaluate {
    match name.as_str().unwrap() {
        CONFIRMATION_CANDIDATE_S1 => evaluate_confirmation_candidate_s1,
        CONFIRMATION_CANDIDATE_S2 => evaluate_confirmation_candidate_s2,
        other => panic!("unknown statistic {other}"),
    }
}

fn case(id: &str) -> Value {
    fixture()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("no case {id}"))
        .clone()
}

/// Runs a fixture case, optionally with another statistic or other series.
fn run(case: &Value, evaluate: Evaluate) -> Result<ConfirmationReport, ConfirmationError> {
    let declaration = parse_confirmation_declaration(&case["declaration"]).unwrap();
    let series: Vec<(u64, Vec<f64>, Vec<f64>)> = case["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| {
            (
                candidate["candidateIndex"].as_u64().unwrap(),
                floats(&candidate["returns"]),
                floats(&candidate["benchmarkReturns"]),
            )
        })
        .collect();
    run_series(
        &declaration,
        case["familyTests"].as_u64().unwrap(),
        &series,
        evaluate,
    )
}

fn run_series(
    declaration: &ConfirmationDeclaration,
    family_tests: u64,
    series: &[(u64, Vec<f64>, Vec<f64>)],
    evaluate: Evaluate,
) -> Result<ConfirmationReport, ConfirmationError> {
    let inputs: Vec<CandidateSeries<'_>> = series
        .iter()
        .map(|(index, returns, benchmark)| CandidateSeries {
            candidate_index: *index,
            returns,
            benchmark_returns: benchmark,
        })
        .collect();
    evaluate(declaration, family_tests, &inputs)
}

fn extremes(report: &ConfirmationReport) -> Vec<u64> {
    report.tests.iter().map(|test| test.extreme_count).collect()
}

fn decisions(report: &ConfirmationReport) -> Vec<(u64, bool)> {
    report
        .tests
        .iter()
        .map(|test| (test.extreme_count, test.rejects_null))
        .collect()
}

/// The PR #138 review's probe: n = 16, L = 2, B = 799, seed 17, 2.5%, a
/// family of two tests, a zero benchmark.
fn review_declaration() -> ConfirmationDeclaration {
    ConfirmationDeclaration {
        alpha_ppm: 25_000,
        block_length: 2,
        bootstrap_samples: 799,
        seed: 17,
        block_length_rationale: "PR #138 review probe".into(),
    }
}

fn review_series(returns: Vec<f64>) -> Vec<(u64, Vec<f64>, Vec<f64>)> {
    let n = returns.len();
    vec![(0, returns, vec![0.0; n])]
}

/// The circular autocovariance at lag `h`.
fn autocovariance(series: &[f64], lag: usize) -> f64 {
    let n = series.len();
    let mean = series.iter().sum::<f64>() / n as f64;
    (0..n)
        .map(|t| (series[t] - mean) * (series[(t + lag) % n] - mean))
        .sum::<f64>()
        / n as f64
}

#[test]
fn fixture_cases_reproduce_the_independent_reference() {
    let fixture = fixture();
    assert_eq!(
        fixture["contractVersion"],
        "research-confirmation-candidates-draft-v1"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 11);
    let mut informative = 0;
    for case in cases {
        let report = run(case, statistic(&case["statistic"])).unwrap();
        assert_eq!(report.contract_version, case["statistic"].as_str().unwrap());
        let expected = case["expected"]["tests"].as_array().unwrap();
        assert_eq!(report.tests.len(), expected.len(), "{}", case["id"]);
        for (actual, expected) in report.tests.iter().zip(expected) {
            let label = format!(
                "{} candidate {} {:?}",
                case["id"], actual.candidate_index, actual.test
            );
            let json = serde_json::to_value(actual).unwrap();
            // Everything a decision depends on is an integer or a boolean and
            // must match exactly.
            for key in [
                "candidateIndex",
                "test",
                "observations",
                "extremeCount",
                "rawP",
                "holmRank",
                "adjustedP",
                "rejectsNull",
            ] {
                assert_eq!(json[key], expected[key], "{label} {key}");
            }
            // The three floats are derived for display. serde_json reads a
            // 17-digit literal only to within one unit in the last place
            // unless its float_roundtrip feature is on, so they are compared
            // to that precision here; the Vitest pins them exactly.
            for key in ["observedMean", "rawPValue", "adjustedPValue"] {
                let ours = json[key].as_f64().unwrap();
                let theirs = expected[key].as_f64().unwrap();
                assert!(
                    (ours - theirs).abs() <= f64::EPSILON * theirs.abs(),
                    "{label} {key}: {ours:e} vs {theirs:e}"
                );
            }
        }
        let samples = case["declaration"]["bootstrapSamples"].as_u64().unwrap();
        informative += extremes(&report)
            .iter()
            .filter(|&&count| count > 0 && count < samples)
            .count();
    }
    // Enough tests sit strictly between "never" and "always" extreme for the
    // parity to mean something.
    assert!(informative >= 10, "{informative}");
}

#[test]
fn the_block_variance_is_the_bartlett_long_run_variance() {
    // Worked by hand in the fixture.
    for example in fixture()["blockVariances"].as_array().unwrap() {
        let series = floats(&example["series"]);
        let sum = series.iter().sum::<f64>();
        assert_eq!(
            block_variance(&series, sum, example["block"].as_u64().unwrap() as usize).unwrap(),
            example["expected"].as_f64().unwrap(),
            "{}",
            example["id"]
        );
    }
    // B(l) = sum over |h| < l of (1 - |h|/l) x the circular autocovariance,
    // and 2B(2l) - B(l) has the flat-top weights: 1 up to lag l, then
    // falling linearly to 0 at 2l.
    let series = floats(&case("s2-smooth-series-stretches")["candidates"][0]["returns"]);
    let sum = series.iter().sum::<f64>();
    for block in [1usize, 2, 3] {
        let lag_window = |weight: &dyn Fn(usize) -> f64, reach: usize| {
            autocovariance(&series, 0)
                + 2.0
                    * (1..reach)
                        .map(|lag| weight(lag) * autocovariance(&series, lag))
                        .sum::<f64>()
        };
        let bartlett = |length: usize| lag_window(&|lag| 1.0 - lag as f64 / length as f64, length);
        let narrow = block_variance(&series, sum, block).unwrap();
        let wide = block_variance(&series, sum, 2 * block).unwrap();
        assert!((narrow - bartlett(block)).abs() < 1e-15, "{block}");
        assert!((wide - bartlett(2 * block)).abs() < 1e-15, "{block}");
        let flat_top = lag_window(
            &|lag| {
                if lag <= block {
                    1.0
                } else {
                    2.0 - lag as f64 / block as f64
                }
            },
            2 * block,
        );
        assert!((2.0 * wide - narrow - flat_top).abs() < 1e-15, "{block}");
    }
    // On this smooth series the correction is active: the flat-top estimate
    // is above the Bartlett one.
    let narrow = block_variance(&series, sum, 3).unwrap();
    assert!(2.0 * block_variance(&series, sum, 6).unwrap() - narrow > narrow);
}

#[test]
fn the_resampled_variance_uses_the_drawn_blocks_not_a_recomputed_b() {
    // PR #136 review: x = [0, 1, 2, 3], L = 2, blocks drawn at 0 and 2
    // reproduce x. B(2) is 1, and so would be B recomputed on the resample;
    // the declared v* is 2.
    let x = [0.0, 1.0, 2.0, 3.0];
    assert_eq!(block_variance(&x, 6.0, 2).unwrap(), 1.0);
    let drawn = [([1.0, 1.0], 2usize), ([5.0, 5.0], 2)];
    assert_eq!(drawn_block_variance(&drawn, 6.0, 4, 0).unwrap(), 2.0);
    // A resample made of one block repeated has no spread between blocks.
    let repeated = [([1.0, 1.0], 2usize), ([1.0, 1.0], 2)];
    assert_eq!(drawn_block_variance(&repeated, 2.0, 4, 0).unwrap(), 0.0);
    // A partial last block is centred on its own length.
    let partial = [([4.0, 0.0], 2usize), ([2.0, 0.0], 1)];
    assert_eq!(drawn_block_variance(&partial, 6.0, 3, 0).unwrap(), 0.0);
}

#[test]
fn the_comparison_needs_no_square_root_and_matches_one() {
    // a·sqrt(p) >= s·sqrt(q), against the same statement with square roots,
    // away from ties.
    let values = [-3.0f64, -1.5, -0.25, 0.0, 0.25, 1.5, 3.0];
    let scales = [0.0f64, 0.0625, 0.25, 1.0, 4.0];
    for &a in &values {
        for &s in &values {
            for &p in &scales {
                for &q in &scales {
                    let (left, right) = (a * p.sqrt(), s * q.sqrt());
                    assert_eq!(
                        scaled_at_least(a, p, s, q).unwrap(),
                        left >= right,
                        "{a}·sqrt({p}) >= {s}·sqrt({q})"
                    );
                }
            }
        }
    }
    // The four sign cases of the draft contract, at their edges.
    let decide = |a, p, s, q| scaled_at_least(a, p, s, q).unwrap();
    assert!(decide(0.0, 1.0, 0.0, 1.0));
    assert!(decide(2.0, 1.0, 2.0, 1.0));
    assert!(!decide(1.0, 1.0, 2.0, 1.0));
    assert!(decide(0.0, 1.0, -1.0, 1.0));
    assert!(!decide(-1.0, 1.0, 0.0, 1.0));
    assert!(decide(-1.0, 0.0, 1.0, 0.0));
    assert!(decide(-1.0, 1.0, -1.0, 1.0));
    assert!(!decide(-2.0, 1.0, -1.0, 1.0));

    // PR #138 review R1: squares that leave the double range are refused, not
    // turned into Inf >= Inf or 0 >= 0. (After normalization the candidates
    // never produce such operands; this is the guard if they did.)
    let huge = f64::from_bits((1023 + 400) << 52);
    let tiny = f64::from_bits((1023 - 400) << 52);
    assert_eq!(
        scaled_at_least(huge, huge * huge, 2.0 * huge, huge * huge),
        Err(OutOfRange)
    );
    assert_eq!(
        scaled_at_least(tiny, tiny * tiny, 2.0 * tiny, tiny * tiny),
        Err(OutOfRange)
    );
    // A zero factor is a genuine zero, not an underflow.
    assert_eq!(scaled_at_least(0.0, tiny, 0.0, 0.0), Ok(true));
}

#[test]
fn a_decision_does_not_depend_on_the_overall_scale() {
    // PR #138 review R1, through the public evaluators: the same binary-exact
    // series scaled by 2^k must give the same counts and rejections. Before
    // the fix S1 gave 0, 384 and 799 extreme resamples at 1, 2^400, 2^-400.
    let base: Vec<f64> = [0.75, 0.375, 1.0, 0.25, 0.625, 0.875, 0.375, 0.5]
        .repeat(2)
        .to_vec();
    let declaration = review_declaration();
    for (name, evaluate) in CANDIDATES {
        let reference = decisions(
            &run_series(&declaration, 2, &review_series(base.clone()), evaluate).unwrap(),
        );
        assert_eq!(reference[0], (0, true), "{name} at scale 1");
        for exponent in [400i32, -400, 1000, -1000, 52, -52] {
            let scaled: Vec<f64> = base
                .iter()
                .map(|&x| x * f64::from_bits(((exponent / 2 + 1023) as u64) << 52))
                .map(|x| x * f64::from_bits(((exponent - exponent / 2 + 1023) as u64) << 52))
                .collect();
            let report = run_series(&declaration, 2, &review_series(scaled), evaluate).unwrap();
            assert_eq!(decisions(&report), reference, "{name} at 2^{exponent}");
        }
    }
    // Every fixture case keeps its counts under exact scaling too, the
    // smooth S2 case with its correction active included.
    for case in fixture()["cases"].as_array().unwrap() {
        let evaluate = statistic(&case["statistic"]);
        let reference = extremes(&run(case, evaluate).unwrap());
        for exponent in [400i32, -400] {
            let mut scaled = case.clone();
            let factor = f64::from_bits(((exponent + 1023) as u64) << 52);
            for candidate in scaled["candidates"].as_array_mut().unwrap() {
                for field in ["returns", "benchmarkReturns"] {
                    let values: Vec<f64> = floats(&candidate[field])
                        .iter()
                        .map(|x| x * factor)
                        .collect();
                    candidate[field] = json!(values);
                }
            }
            // JSON could round the scaled literals, so evaluate the values
            // directly.
            let declaration = parse_confirmation_declaration(&case["declaration"]).unwrap();
            let series: Vec<(u64, Vec<f64>, Vec<f64>)> = case["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .map(|candidate| {
                    (
                        candidate["candidateIndex"].as_u64().unwrap(),
                        floats(&candidate["returns"])
                            .iter()
                            .map(|x| x * factor)
                            .collect(),
                        floats(&candidate["benchmarkReturns"])
                            .iter()
                            .map(|x| x * factor)
                            .collect(),
                    )
                })
                .collect();
            let report = run_series(
                &declaration,
                case["familyTests"].as_u64().unwrap(),
                &series,
                evaluate,
            )
            .unwrap();
            assert_eq!(
                extremes(&report),
                reference,
                "{} at 2^{exponent}",
                case["id"]
            );
        }
    }
}

#[test]
fn a_constant_series_is_decided_by_its_sign_not_by_rounding() {
    // PR #138 review R2: sixteen bars of 0.1 used to be rejected by rounding
    // residue while sixteen bars of 0.125 were never rejected. Equal bars now
    // read no variance at all: a positive constant is rejected (every
    // resample ties the observed sum; the studentized statistic's limit is
    // +infinity), zero and negative constants never are.
    let declaration = review_declaration();
    let all = declaration.bootstrap_samples;
    for (name, evaluate) in CANDIDATES {
        for (value, expected) in [
            (0.1, (0, true)),
            (0.125, (0, true)),
            (1e-300, (0, true)),
            (0.0, (all, false)),
            (-0.0, (all, false)),
            (-0.1, (all, false)),
            (-0.125, (all, false)),
        ] {
            let report =
                run_series(&declaration, 2, &review_series(vec![value; 16]), evaluate).unwrap();
            // Net return and excess over a zero benchmark are the same series.
            assert_eq!(
                decisions(&report),
                [expected, expected],
                "{name} with sixteen bars of {value}"
            );
        }
    }
    // The rule applies per test: zero net returns against a moving benchmark
    // leave the excess test to the statistic (fixture case).
    let zero = case("s1-all-zero-net-returns-are-never-rejected");
    let report = run(&zero, evaluate_confirmation_candidate_s1).unwrap();
    assert_eq!(report.tests[0].extreme_count, 39);
    assert!(report.tests[1].extreme_count < 39);
    // v1 decides a positive constant the same way.
    let v1 = run_series(
        &declaration,
        2,
        &review_series(vec![0.1; 16]),
        evaluate_confirmation,
    )
    .unwrap();
    assert_eq!(decisions(&v1), [(0, true), (0, true)]);
}

#[test]
fn the_documented_edge_cases_hold() {
    // A clearly negative mean is never rejected by either candidate.
    for (id, evaluate) in [
        (
            "s1-negative-mean",
            evaluate_confirmation_candidate_s1 as Evaluate,
        ),
        ("s2-negative-mean", evaluate_confirmation_candidate_s2),
    ] {
        let report = run(&case(id), evaluate).unwrap();
        assert!(report.tests.iter().all(|test| test.observed_mean < 0.0));
        assert!(report.tests.iter().all(|test| !test.rejects_null), "{id}");
    }

    // S2 never shrinks v1's deviations: with a positive observed sum it
    // counts at least as many extreme resamples as v1 on the same draws.
    for id in [
        "s2-two-candidates",
        "s2-partial-last-block",
        "s2-smooth-series-stretches",
    ] {
        let case = case(id);
        let s2 = run(&case, evaluate_confirmation_candidate_s2).unwrap();
        let v1 = run(&case, evaluate_confirmation).unwrap();
        for (corrected, plain) in s2.tests.iter().zip(&v1.tests) {
            assert!(corrected.observed_mean > 0.0);
            assert_eq!(corrected.observed_mean, plain.observed_mean);
            assert!(corrected.extreme_count >= plain.extreme_count, "{id}");
        }
    }
    // Where the correction is active it changes the count.
    let smooth = case("s2-smooth-series-stretches");
    assert_ne!(
        extremes(&run(&smooth, evaluate_confirmation_candidate_s2).unwrap()),
        extremes(&run(&smooth, evaluate_confirmation).unwrap())
    );
}

#[test]
fn candidates_keep_v1s_resamples_report_and_holm() {
    for case in fixture()["cases"].as_array().unwrap() {
        let evaluate = statistic(&case["statistic"]);
        let report = run(case, evaluate).unwrap();
        // Same report shape and the same Holm arithmetic as v1.
        for test in &report.tests {
            assert_eq!(test.raw_p.numerator, test.extreme_count + 1);
            assert!(test.adjusted_p.numerator >= test.raw_p.numerator);
            assert!(test.adjusted_p.numerator <= test.adjusted_p.denominator);
        }
        if let Ok(v1) = run(case, evaluate_confirmation) {
            assert_eq!(v1.tests.len(), report.tests.len());
            for (candidate, plain) in report.tests.iter().zip(&v1.tests) {
                assert_eq!(candidate.observed_mean, plain.observed_mean);
                assert_eq!(candidate.observations, plain.observations);
            }
        }
    }
    // The two candidates are different statistics on the same inputs.
    let shared = case("s2-two-candidates");
    assert_ne!(
        extremes(&run(&shared, evaluate_confirmation_candidate_s1).unwrap()),
        extremes(&run(&shared, evaluate_confirmation_candidate_s2).unwrap())
    );
}

#[test]
fn inputs_the_draft_contract_refuses() {
    let base = case("s1-two-candidates");
    let error = |case: &Value, evaluate: Evaluate| run(case, evaluate).unwrap_err().0;

    // S1 needs L^2 <= n like v1; S2 needs (2L)^2 <= n. Sixteen bars.
    let with_block = |block: u64| {
        let mut case = base.clone();
        case["declaration"]["blockLength"] = json!(block);
        case
    };
    assert!(run(&with_block(4), evaluate_confirmation_candidate_s1).is_ok());
    assert_eq!(
        error(&with_block(5), evaluate_confirmation_candidate_s1),
        "confirmation: blockLength 5 squared exceeds candidate 0's 16 bars"
    );
    assert!(run(&with_block(2), evaluate_confirmation_candidate_s2).is_ok());
    assert_eq!(
        error(&with_block(3), evaluate_confirmation_candidate_s2),
        "confirmation: twice blockLength 3 squared exceeds candidate 0's 16 bars"
    );

    for (_, evaluate) in CANDIDATES {
        let mut short = base.clone();
        short["candidates"][0]["benchmarkReturns"] = json!([0.0]);
        assert_eq!(
            error(&short, evaluate),
            "confirmation: candidate 0 has 16 returns but 1 benchmark returns"
        );
        let mut repeated = base.clone();
        repeated["candidates"][1]["candidateIndex"] = json!(0);
        assert_eq!(
            error(&repeated, evaluate),
            "confirmation: candidate index 0 is repeated or out of range"
        );
        let mut family = base.clone();
        family["familyTests"] = json!(2);
        assert!(error(&family, evaluate).starts_with("confirmation: familyTests must be"));
    }
    // Non-finite values cannot come from JSON; build them directly.
    let declaration = parse_confirmation_declaration(&base["declaration"]).unwrap();
    let mut returns = floats(&base["candidates"][0]["returns"]);
    let benchmark = floats(&base["candidates"][0]["benchmarkReturns"]);
    returns[3] = f64::NAN;
    assert_eq!(
        run_series(
            &declaration,
            2,
            &[(0, returns, benchmark.clone())],
            evaluate_confirmation_candidate_s1
        )
        .unwrap_err()
        .0,
        "confirmation: candidate 0 has a non-finite return or sum"
    );
    // PR #138 review R1: a value so much smaller than the largest that it
    // would underflow when the series is normalized is refused, not rounded
    // to zero. The same values without the outlier are fine.
    let out_of_range = "confirmation: candidate 0 is outside the candidates' numeric range (an intermediate overflowed or underflowed)";
    let mut wide_range = vec![1e-300; 16];
    wide_range[5] = 1e300;
    for (name, evaluate) in CANDIDATES {
        assert_eq!(
            run_series(
                &declaration,
                2,
                &review_series(wide_range.clone()),
                evaluate
            )
            .unwrap_err()
            .0,
            out_of_range,
            "{name}"
        );
        let mut ordinary = vec![1e-300; 16];
        ordinary[5] = 3e-300;
        assert!(run_series(&declaration, 2, &review_series(ordinary), evaluate).is_ok());
    }
    // Very large but finite inputs are normalized instead of overflowing.
    let huge = vec![
        1e300, -1e300, 1e300, -1e300, 1e300, 1e300, -1e300, 1e300, 1e300,
    ];
    let mut small = declaration.clone();
    small.block_length = 1;
    assert!(run_series(
        &small,
        2,
        &[(0, huge, vec![0.0; 9])],
        evaluate_confirmation_candidate_s1
    )
    .is_ok());
}

#[test]
fn normalization_is_exact_and_lands_in_one_to_two() {
    assert_eq!(binary_exponent(1.0), 0);
    assert_eq!(binary_exponent(1.999), 0);
    assert_eq!(binary_exponent(2.0), 1);
    assert_eq!(binary_exponent(0.1), -4);
    assert_eq!(binary_exponent(f64::MAX), 1023);
    assert_eq!(binary_exponent(f64::MIN_POSITIVE), -1022);
    assert_eq!(binary_exponent(f64::from_bits(1)), -1074);
    assert_eq!(binary_exponent(f64::from_bits(1 << 51)), -1023);
    for (series, expected) in [
        (vec![0.75, -0.375, 0.0], vec![1.5, -0.75, 0.0]),
        (vec![3.0, 1.0], vec![1.5, 0.5]),
        (vec![0.0, 0.0], vec![0.0, 0.0]),
        (
            vec![f64::MAX, 2.0],
            vec![
                f64::from_bits(f64::MAX.to_bits() - (1023u64 << 52)),
                f64::MIN_POSITIVE,
            ],
        ),
    ] {
        assert_eq!(normalized(&series), Ok(expected), "{series:?}");
    }
    // One step further apart and the small value would become subnormal: it
    // is refused rather than rounded (draft §2).
    assert_eq!(normalized(&[f64::MAX, 1.0]), Err(OutOfRange));
    // The smallest subnormal scales up exactly to 1.
    assert_eq!(normalized(&[f64::from_bits(1)]), Ok(vec![1.0]));
    for exponent in [-1074, -1000, -400, 0, 400, 1023] {
        assert_eq!(times_power_of_two(1.0, exponent).log2(), exponent as f64);
    }
}
