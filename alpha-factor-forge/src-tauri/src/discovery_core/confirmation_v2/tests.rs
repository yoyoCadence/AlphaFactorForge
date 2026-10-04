use serde_json::{json, Value};

use super::*;
use crate::discovery_core::confirmation::parse_confirmation_declaration;
use crate::discovery_core::confirmation_candidates::{
    evaluate_confirmation_candidate_s2, CONFIRMATION_CANDIDATE_S2,
};

const FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-confirmation-statistics-v2.json");

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

type Series = Vec<(u64, Vec<f64>, Vec<f64>)>;

fn case_series(case: &Value) -> Series {
    case["candidates"]
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
        .collect()
}

fn evaluate(
    evaluate: fn(
        &ConfirmationDeclaration,
        u64,
        &[CandidateSeries<'_>],
    ) -> Result<ConfirmationReport, ConfirmationError>,
    declaration: &ConfirmationDeclaration,
    family_tests: u64,
    series: &Series,
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

fn declaration(block_length: u64) -> ConfirmationDeclaration {
    ConfirmationDeclaration {
        alpha_ppm: 25_000,
        block_length,
        bootstrap_samples: 19,
        seed: 3,
        block_length_rationale: "block rule R3".into(),
    }
}

/// A non-constant series of `n` bars with a zero benchmark.
fn plain_series(n: usize) -> Series {
    let returns = (0..n)
        .map(|k| ((37 * k + 11) % 61) as f64 / 10_000.0 - 0.0029)
        .collect();
    vec![(0, returns, vec![0.0; n])]
}

/// `round(n^(1/3))` straight from its definition: `(2L − 1)^3 < 8n < (2L + 1)^3`.
fn satisfies_r3(n: u64, length: u64) -> bool {
    let (eight_n, odd) = (8 * u128::from(n), 2 * u128::from(length));
    length >= 1 && (odd - 1).pow(3) < eight_n && eight_n < (odd + 1).pow(3)
}

#[test]
fn fixture_cases_reproduce_the_independent_reference() {
    let fixture = fixture();
    assert_eq!(
        fixture["contractVersion"],
        CONFIRMATION_STATISTICS_V2_VERSION
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    let (mut informative, mut rejected, mut kept) = (0, 0, 0);
    for case in cases {
        let declaration = parse_confirmation_declaration_v2(&case["declaration"]).unwrap();
        let report = evaluate(
            evaluate_confirmation_v2,
            &declaration,
            case["familyTests"].as_u64().unwrap(),
            &case_series(case),
        )
        .unwrap();
        assert_eq!(report.contract_version, CONFIRMATION_STATISTICS_V2_VERSION);
        let expected = case["expected"]["tests"].as_array().unwrap();
        assert_eq!(report.tests.len(), expected.len(), "{}", case["id"]);
        for (actual, expected) in report.tests.iter().zip(expected) {
            let label = format!(
                "{} candidate {} {:?}",
                case["id"], actual.candidate_index, actual.test
            );
            let json = serde_json::to_value(actual).unwrap();
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
            // Display floats to one unit in the last place: serde_json reads a
            // 17-digit literal only that closely (NUMERIC-JSON-001).
            for key in ["observedMean", "rawPValue", "adjustedPValue"] {
                let ours = json[key].as_f64().unwrap();
                let theirs = expected[key].as_f64().unwrap();
                assert!(
                    (ours - theirs).abs() <= f64::EPSILON * theirs.abs(),
                    "{label} {key}: {ours:e} vs {theirs:e}"
                );
            }
            if actual.extreme_count > 0 && actual.extreme_count < declaration.bootstrap_samples {
                informative += 1;
            }
            if actual.rejects_null {
                rejected += 1;
            } else {
                kept += 1;
            }
        }
    }
    assert!(informative >= 10, "{informative}");
    assert!(rejected >= 3 && kept >= 3, "{rejected} {kept}");
}

#[test]
fn v2_is_draft_s2_with_its_block_rule() {
    // On every fixture input the frozen statistic reports exactly what the
    // draft candidate S2 reports, apart from the contract's name.
    for case in fixture()["cases"].as_array().unwrap() {
        let declaration = parse_confirmation_declaration_v2(&case["declaration"]).unwrap();
        let family_tests = case["familyTests"].as_u64().unwrap();
        let series = case_series(case);
        let v2 = evaluate(
            evaluate_confirmation_v2,
            &declaration,
            family_tests,
            &series,
        )
        .unwrap();
        let mut s2 = evaluate(
            evaluate_confirmation_candidate_s2,
            &declaration,
            family_tests,
            &series,
        )
        .unwrap();
        assert_eq!(s2.contract_version, CONFIRMATION_CANDIDATE_S2);
        s2.contract_version = CONFIRMATION_STATISTICS_V2_VERSION;
        assert_eq!(v2, s2, "{}", case["id"]);
    }
}

#[test]
fn r3_is_the_rounded_cube_root_in_exact_integers() {
    // The plan's table (§3).
    assert_eq!(
        [256, 512, 1024].map(r3_block_length),
        [6, 8, 10],
        "plan §3, rule R3"
    );
    // Every n up to 200,000 against the definition, and every boundary
    // around the half-integer cubes `(L ± 1/2)^3` up to L = 2,000.
    for n in 1..=200_000 {
        assert!(satisfies_r3(n, r3_block_length(n)), "{n}");
    }
    for length in 1u64..=2_000 {
        // The largest n with round(cbrt n) = length: 8n < (2L + 1)^3.
        let last = ((2 * u128::from(length) + 1).pow(3) / 8) as u64;
        assert_eq!(r3_block_length(last), length, "{last}");
        assert_eq!(r3_block_length(last + 1), length + 1, "{}", last + 1);
        assert_eq!(r3_block_length(length.pow(3)), length, "cube {length}");
    }
    // The top of the domain: no overflow, still the definition.
    for n in [u64::MAX, u64::MAX - 1, 1 << 63, (1 << 53) - 1] {
        assert!(satisfies_r3(n, r3_block_length(n)), "{n}");
    }
    assert_eq!(r3_block_length(u64::MAX), 2_642_246);
}

#[test]
fn the_block_length_must_be_r3_for_every_series() {
    // 64 bars: R3 = 4. One more or one less is refused, with the reason.
    let series = plain_series(64);
    assert!(evaluate(evaluate_confirmation_v2, &declaration(4), 2, &series).is_ok());
    for wrong in [3, 5] {
        assert_eq!(
            evaluate(evaluate_confirmation_v2, &declaration(wrong), 2, &series)
                .unwrap_err()
                .0,
            format!(
                "confirmation: research-confirmation-statistics-v2 requires blockLength 4 (round of the cube root of candidate 0's 64 bars), not {wrong}"
            )
        );
        // The draft S2 has no such rule.
        if wrong == 3 {
            assert!(evaluate(
                evaluate_confirmation_candidate_s2,
                &declaration(wrong),
                2,
                &series
            )
            .is_ok());
        }
    }
    // Every candidate is checked: 64 and 70 bars share R3 = 4; 100 bars has 5.
    let mut mixed = plain_series(64);
    mixed.push((1, plain_series(70).remove(0).1, vec![0.0; 70]));
    assert!(evaluate(evaluate_confirmation_v2, &declaration(4), 4, &mixed).is_ok());
    mixed.push((2, plain_series(100).remove(0).1, vec![0.0; 100]));
    assert_eq!(
        evaluate(evaluate_confirmation_v2, &declaration(4), 6, &mixed)
            .unwrap_err()
            .0,
        "confirmation: research-confirmation-statistics-v2 requires blockLength 5 (round of the cube root of candidate 2's 100 bars), not 4"
    );
    // v1's own input checks come first and keep their messages.
    let mut uneven = plain_series(64);
    uneven[0].2.pop();
    assert_eq!(
        evaluate(evaluate_confirmation_v2, &declaration(9), 2, &uneven)
            .unwrap_err()
            .0,
        "confirmation: candidate 0 has 64 returns but 63 benchmark returns"
    );
    assert_eq!(
        evaluate(
            evaluate_confirmation_v2,
            &declaration(9),
            2,
            &plain_series(1)
        )
        .unwrap_err()
        .0,
        "confirmation: candidate 0 needs at least 2 bars"
    );
}

#[test]
fn the_accepted_lengths_are_those_where_twice_r3_fits() {
    // With L = R3(n), S2's rule (2L)^2 <= n leaves holes below 100 bars.
    let mut accepted = Vec::new();
    for n in 2..=400u64 {
        let length = r3_block_length(n);
        let outcome = evaluate(
            evaluate_confirmation_v2,
            &declaration(length),
            2,
            &plain_series(n as usize),
        );
        assert_eq!(outcome.is_ok(), 4 * length * length <= n, "{n}");
        if let Err(error) = outcome {
            assert_eq!(
                error.0,
                format!(
                    "confirmation: twice blockLength {length} squared exceeds candidate 0's {n} bars"
                )
            );
        } else {
            accepted.push(n);
        }
    }
    let expected: Vec<u64> = (36..=42).chain(64..=91).chain(100..=400).collect();
    assert_eq!(accepted, expected);
    // From L = 6 on, the first n with that R3 already fits twice the block.
    for length in 6u64..=2_000 {
        let first = ((2 * u128::from(length) - 1).pow(3) / 8 + 1) as u64;
        assert_eq!(r3_block_length(first), length);
        assert!(4 * length * length <= first, "{length}");
    }
}

#[test]
fn the_declaration_is_v1s_under_its_own_name() {
    let raw = fixture()["cases"][0]["declaration"].clone();
    let parsed = parse_confirmation_declaration_v2(&raw).unwrap();
    assert_eq!(parsed.block_length, 4);
    // Each parser accepts only its own contract.
    assert_eq!(
        parse_confirmation_declaration(&raw).unwrap_err().0,
        "confirmation.contractVersion: must be \"research-confirmation-statistics-v1\""
    );
    let mut v1 = raw.clone();
    v1["contractVersion"] = json!("research-confirmation-statistics-v1");
    assert!(parse_confirmation_declaration(&v1).is_ok());
    assert_eq!(
        parse_confirmation_declaration_v2(&v1).unwrap_err().0,
        "confirmation.contractVersion: must be \"research-confirmation-statistics-v2\""
    );
    // v1's field rules and order, unchanged.
    let mut extra = raw.clone();
    extra["blockRule"] = json!("R3");
    assert_eq!(
        parse_confirmation_declaration_v2(&extra).unwrap_err().0,
        "confirmation.blockRule: unknown field"
    );
    let mut missing = raw;
    missing
        .as_object_mut()
        .unwrap()
        .remove("blockLengthRationale");
    assert_eq!(
        parse_confirmation_declaration_v2(&missing).unwrap_err().0,
        "confirmation.blockLengthRationale: required"
    );
}
