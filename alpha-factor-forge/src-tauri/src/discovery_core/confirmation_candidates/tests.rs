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
    let inputs: Vec<CandidateSeries<'_>> = series
        .iter()
        .map(|(index, returns, benchmark)| CandidateSeries {
            candidate_index: *index,
            returns,
            benchmark_returns: benchmark,
        })
        .collect();
    evaluate(&declaration, case["familyTests"].as_u64().unwrap(), &inputs)
}

fn extremes(report: &ConfirmationReport) -> Vec<u64> {
    report.tests.iter().map(|test| test.extreme_count).collect()
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
    assert_eq!(cases.len(), 8);
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
            block_variance(&series, sum, example["block"].as_u64().unwrap() as usize),
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
        let narrow = block_variance(&series, sum, block);
        let wide = block_variance(&series, sum, 2 * block);
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
    assert!(
        2.0 * block_variance(&series, sum, 6) - block_variance(&series, sum, 3)
            > block_variance(&series, sum, 3)
    );
}

#[test]
fn the_resampled_variance_uses_the_drawn_blocks_not_a_recomputed_b() {
    // PR #136 review: x = [0, 1, 2, 3], L = 2, blocks drawn at 0 and 2
    // reproduce x. B(2) is 1, and so would be B recomputed on the resample;
    // the declared v* is 2.
    let x = [0.0, 1.0, 2.0, 3.0];
    assert_eq!(block_variance(&x, 6.0, 2), 1.0);
    let drawn = [([1.0, 1.0], 2usize), ([5.0, 5.0], 2)];
    assert_eq!(drawn_block_variance(&drawn, 6.0, 4, 0), 2.0);
    // A resample made of one block repeated has no spread between blocks.
    let repeated = [([1.0, 1.0], 2usize), ([1.0, 1.0], 2)];
    assert_eq!(drawn_block_variance(&repeated, 2.0, 4, 0), 0.0);
    // A partial last block is centred on its own length.
    let partial = [([4.0, 0.0], 2usize), ([2.0, 0.0], 1)];
    assert_eq!(drawn_block_variance(&partial, 6.0, 3, 0), 0.0);
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
                        scaled_at_least(a, p, s, q),
                        left >= right,
                        "{a}·sqrt({p}) >= {s}·sqrt({q})"
                    );
                }
            }
        }
    }
    // The four sign cases of the draft contract, at their edges.
    assert!(scaled_at_least(0.0, 1.0, 0.0, 1.0));
    assert!(scaled_at_least(2.0, 1.0, 2.0, 1.0));
    assert!(!scaled_at_least(1.0, 1.0, 2.0, 1.0));
    assert!(scaled_at_least(0.0, 1.0, -1.0, 1.0));
    assert!(!scaled_at_least(-1.0, 1.0, 0.0, 1.0));
    assert!(scaled_at_least(-1.0, 0.0, 1.0, 0.0));
    assert!(scaled_at_least(-1.0, 1.0, -1.0, 1.0));
    assert!(!scaled_at_least(-2.0, 1.0, -1.0, 1.0));
}

#[test]
fn the_documented_edge_cases_hold() {
    // A constant positive series: every resample is extreme under S1, so the
    // raw p-value is 1 and nothing is rejected; v1 rejects it outright.
    let constant = case("s1-constant-series-never-rejects");
    let s1 = run(&constant, evaluate_confirmation_candidate_s1).unwrap();
    assert_eq!(extremes(&s1), [9, 9]);
    assert!(s1.tests.iter().all(|test| !test.rejects_null));
    assert!(s1
        .tests
        .iter()
        .all(|test| test.raw_p.numerator == test.raw_p.denominator));
    let v1 = run(&constant, evaluate_confirmation).unwrap();
    assert_eq!(extremes(&v1), [0, 0]);

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
fn candidates_keep_v1s_resamples_report_and_scale_invariance() {
    for case in fixture()["cases"].as_array().unwrap() {
        let evaluate = statistic(&case["statistic"]);
        let report = run(case, evaluate).unwrap();
        let v1 = run(case, evaluate_confirmation);
        // Same report shape and the same Holm arithmetic as v1.
        for test in &report.tests {
            assert_eq!(test.raw_p.numerator, test.extreme_count + 1);
            assert!(test.adjusted_p.numerator >= test.raw_p.numerator);
            assert!(test.adjusted_p.numerator <= test.adjusted_p.denominator);
        }
        if let Ok(v1) = v1 {
            assert_eq!(v1.tests.len(), report.tests.len());
            for (candidate, plain) in report.tests.iter().zip(&v1.tests) {
                assert_eq!(candidate.observed_mean, plain.observed_mean);
                assert_eq!(candidate.observations, plain.observations);
            }
        }
        // Doubling every return scales every sum and variance exactly, so no
        // decision moves.
        let mut doubled = case.clone();
        for candidate in doubled["candidates"].as_array_mut().unwrap() {
            for field in ["returns", "benchmarkReturns"] {
                let scaled: Vec<f64> = floats(&candidate[field]).iter().map(|x| x * 2.0).collect();
                candidate[field] = json!(scaled);
            }
        }
        assert_eq!(
            extremes(&run(&doubled, evaluate).unwrap()),
            extremes(&report),
            "{}",
            case["id"]
        );
        // A different seed draws different blocks.
        let mut reseeded = case.clone();
        reseeded["declaration"]["seed"] = json!(case["declaration"]["seed"].as_u64().unwrap() + 1);
        assert_eq!(
            run(&reseeded, evaluate).unwrap().tests.len(),
            report.tests.len()
        );
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

    for evaluate in [
        evaluate_confirmation_candidate_s1 as Evaluate,
        evaluate_confirmation_candidate_s2,
    ] {
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
    let broken = [CandidateSeries {
        candidate_index: 0,
        returns: &returns,
        benchmark_returns: &benchmark,
    }];
    assert_eq!(
        evaluate_confirmation_candidate_s1(&declaration, 2, &broken)
            .unwrap_err()
            .0,
        "confirmation: candidate 0 has a non-finite return or sum"
    );
    // Finite returns whose block variance overflows are refused too.
    let huge = vec![
        1e160, -1e160, 1e160, -1e160, 1e160, 1e160, -1e160, 1e160, 1e160,
    ];
    let zeros = vec![0.0; 9];
    let mut small = declaration.clone();
    small.block_length = 1;
    let overflow = [CandidateSeries {
        candidate_index: 0,
        returns: &huge,
        benchmark_returns: &zeros,
    }];
    assert_eq!(
        evaluate_confirmation_candidate_s1(&small, 2, &overflow)
            .unwrap_err()
            .0,
        "confirmation: candidate 0 has a non-finite block variance"
    );
}
