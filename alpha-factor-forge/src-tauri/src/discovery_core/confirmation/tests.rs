use serde_json::{json, Value};

use super::*;
use crate::discovery_core::precision::{evaluate_precision_plan, PrecisionPlan};

const FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-confirmation-statistics-v1.json");

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

fn declaration_json() -> Value {
    fixture()["cases"][1]["declaration"].clone()
}

fn declaration() -> ConfirmationDeclaration {
    parse_confirmation_declaration(&declaration_json()).unwrap()
}

fn error(raw: &Value) -> String {
    parse_confirmation_declaration(raw).unwrap_err().0
}

/// Every return in `[0.004, 0.013]`: no resample can reach twice the observed
/// sum, so both tests have `extremeCount = 0` whatever the seed.
fn steady() -> (Vec<f64>, Vec<f64>) {
    let case = &fixture()["cases"][2]["candidates"][0];
    (floats(&case["returns"]), floats(&case["benchmarkReturns"]))
}

fn evaluate_steady(
    alpha_ppm: u64,
    bootstrap_samples: u64,
    family_tests: u64,
) -> ConfirmationReport {
    let (returns, benchmark) = steady();
    let declaration = ConfirmationDeclaration {
        alpha_ppm,
        bootstrap_samples,
        block_length: 3,
        ..declaration()
    };
    evaluate_confirmation(
        &declaration,
        family_tests,
        &[CandidateSeries {
            candidate_index: 5,
            returns: &returns,
            benchmark_returns: &benchmark,
        }],
    )
    .unwrap()
}

#[test]
fn splitmix64_matches_the_published_reference_vectors() {
    for vector in fixture()["prngVectors"].as_array().unwrap() {
        let state: u64 = vector["state"].as_str().unwrap().parse().unwrap();
        let mut rng = SplitMix64::new(state);
        for expected in vector["outputs"].as_array().unwrap() {
            assert_eq!(rng.next_u64().to_string(), expected.as_str().unwrap());
        }
    }
    // The first output for state 0, as published with the reference code.
    assert_eq!(SplitMix64::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
}

#[test]
fn unbiased_index_draws_stay_in_range_and_discard_the_biased_prefix() {
    let mut rng = SplitMix64::new(42);
    for n in [1u64, 2, 3, 7, 1 << 20, u64::MAX] {
        for _ in 0..64 {
            assert!(rng.below(n) < n);
        }
    }
    // 2^64 mod 3 = 1: exactly one raw value (0) is rejected.
    assert_eq!(3u64.wrapping_neg() % 3, 1);
    assert_eq!(
        (1u64 << 63).wrapping_neg() % (1u64 << 63),
        0,
        "powers of two reject nothing"
    );
}

#[test]
fn fixture_cases_reproduce_the_independent_reference() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 5);
    for case in cases {
        let id = case["id"].as_str().unwrap();
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
        let candidates: Vec<CandidateSeries<'_>> = series
            .iter()
            .map(|(candidate_index, returns, benchmark)| CandidateSeries {
                candidate_index: *candidate_index,
                returns,
                benchmark_returns: benchmark,
            })
            .collect();
        let report = evaluate_confirmation(
            &declaration,
            case["familyTests"].as_u64().unwrap(),
            &candidates,
        )
        .unwrap();
        let expected = case["expected"]["tests"].as_array().unwrap();
        assert_eq!(report.tests.len(), expected.len(), "{id}");
        for (actual, expected) in report.tests.iter().zip(expected) {
            let label = format!(
                "{id} candidate {} {:?}",
                actual.candidate_index, actual.test
            );
            let actual = serde_json::to_value(actual).unwrap();
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
                assert_eq!(actual[key], expected[key], "{label} {key}");
            }
            for key in ["observedMean", "rawPValue", "adjustedPValue"] {
                assert_eq!(
                    actual[key].as_f64().unwrap().to_bits(),
                    expected[key].as_f64().unwrap().to_bits(),
                    "{label} {key}"
                );
            }
        }
    }
}

#[test]
fn the_report_states_statistics_and_never_a_pass() {
    let report = evaluate_steady(50_000, 999, 2);
    let json = serde_json::to_value(&report).unwrap();
    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "alphaPpm",
            "blockLength",
            "bootstrapSamples",
            "contractVersion",
            "correction",
            "familyTests",
            "prng",
            "scheme",
            "seed",
            "tests"
        ]
    );
    assert_eq!(
        json["contractVersion"],
        "research-confirmation-statistics-v1"
    );
    assert_eq!(json["tests"][0]["test"], "net_return");
    assert_eq!(json["tests"][1]["test"], "benchmark_excess");
    assert!(!json.to_string().to_ascii_uppercase().contains("PASS"));
}

#[test]
fn the_declaration_is_parsed_strictly_in_a_fixed_rejection_order() {
    assert_eq!(declaration().block_length, 4);
    assert_eq!(error(&json!([])), "confirmation: must be an object");

    let mut two_unknown = declaration_json();
    two_unknown["zeta"] = json!(1);
    two_unknown["alphaBeta"] = json!(1);
    two_unknown["contractVersion"] = json!("other");
    assert_eq!(error(&two_unknown), "confirmation.alphaBeta: unknown field");

    // Each field alone, then the ORDER: every later field is also broken.
    let invalid: [(&str, Value, &str); 9] = [
        (
            "contractVersion",
            json!("research-confirmation-statistics-v2"),
            "must be \"research-confirmation-statistics-v1\"",
        ),
        ("correction", json!("bonferroni"), "must be \"holm\""),
        ("scheme", json!("stationary"), "must be \"circular-block\""),
        ("prng", json!("mulberry32"), "must be \"splitmix64\""),
        ("alphaPpm", json!(0), "must be an integer in [1, 999999]"),
        (
            "blockLength",
            json!(0),
            "must be an integer in [1, 9007199254740991]",
        ),
        (
            "bootstrapSamples",
            json!(1000.0),
            "must be an integer in [1, 9007199254740991]",
        ),
        (
            "seed",
            json!(-1),
            "must be an integer in [0, 9007199254740991]",
        ),
        (
            "blockLengthRationale",
            json!(" "),
            "must be non-empty trimmed text of at most 1024 bytes",
        ),
    ];
    for (position, (field, value, message)) in invalid.iter().enumerate() {
        let mut one = declaration_json();
        one[*field] = value.clone();
        assert_eq!(error(&one), format!("confirmation.{field}: {message}"));

        let mut missing = declaration_json();
        missing.as_object_mut().unwrap().remove(*field);
        assert_eq!(error(&missing), format!("confirmation.{field}: required"));

        let mut rest = declaration_json();
        for (later, value, _) in &invalid[position..] {
            rest[*later] = value.clone();
        }
        assert_eq!(
            error(&rest),
            format!("confirmation.{field}: {message}"),
            "order at {field}"
        );
    }

    for (label, value) in [
        ("alpha at the top of its range", json!(1_000_000)),
        ("string alpha", json!("50000")),
    ] {
        let mut raw = declaration_json();
        raw["alphaPpm"] = value;
        assert!(error(&raw).starts_with("confirmation.alphaPpm"), "{label}");
    }
    for (label, text, ok) in [
        ("exactly 1024 bytes", "x".repeat(1024), true),
        ("1025 bytes", "x".repeat(1025), false),
        ("trailing space", "reason ".to_string(), false),
        ("empty", String::new(), false),
    ] {
        let mut raw = declaration_json();
        raw["blockLengthRationale"] = json!(text);
        assert_eq!(parse_confirmation_declaration(&raw).is_ok(), ok, "{label}");
    }
    let mut max_seed = declaration_json();
    max_seed["seed"] = json!(PRECISION_MAX_COUNT);
    assert!(parse_confirmation_declaration(&max_seed).is_ok());
    max_seed["seed"] = json!(PRECISION_MAX_COUNT + 1);
    assert!(parse_confirmation_declaration(&max_seed).is_err());
}

#[test]
fn inputs_that_cannot_support_the_declared_bootstrap_are_refused() {
    let declaration = declaration(); // blockLength 4
    let good: Vec<f64> = (0..16)
        .map(|bar| 0.001 * f64::from(bar % 5) - 0.001)
        .collect();
    let run = |family_tests: u64, candidates: &[CandidateSeries<'_>]| {
        evaluate_confirmation(&declaration, family_tests, candidates).map_err(|error| error.0)
    };
    let one = |returns: &[f64], benchmark: &[f64]| {
        run(
            2,
            &[CandidateSeries {
                candidate_index: 0,
                returns,
                benchmark_returns: benchmark,
            }],
        )
    };
    assert!(one(&good, &good).is_ok());
    assert!(run(2, &[]).unwrap_err().contains("no candidates"));
    assert!(one(&good, &good[..15])
        .unwrap_err()
        .contains("benchmark returns"));
    assert!(one(&good[..1], &good[..1])
        .unwrap_err()
        .contains("at least 2 bars"));

    // Block length boundary: L^2 <= n (16 bars allow 4, 15 do not).
    assert!(one(&good[..15], &good[..15])
        .unwrap_err()
        .contains("squared exceeds"));
    let three = ConfirmationDeclaration {
        block_length: 3,
        ..declaration.clone()
    };
    for (bars, ok) in [(8usize, false), (9, true), (10, true)] {
        let result = evaluate_confirmation(
            &three,
            2,
            &[CandidateSeries {
                candidate_index: 0,
                returns: &good[..bars],
                benchmark_returns: &good[..bars],
            }],
        );
        assert_eq!(result.is_ok(), ok, "{bars} bars with blockLength 3");
    }

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut returns = good.clone();
        returns[7] = bad;
        assert!(one(&returns, &good).unwrap_err().contains("non-finite"));
        assert!(one(&good, &returns).unwrap_err().contains("non-finite"));
    }
    let huge = vec![f64::MAX; 16];
    assert!(
        one(&huge, &good).unwrap_err().contains("non-finite"),
        "an overflowing sum"
    );

    let pair = [
        CandidateSeries {
            candidate_index: 4,
            returns: &good,
            benchmark_returns: &good,
        },
        CandidateSeries {
            candidate_index: 4,
            returns: &good,
            benchmark_returns: &good,
        },
    ];
    assert!(run(4, &pair).unwrap_err().contains("repeated"));

    // familyTests: a multiple of two, at least this batch, at most MAX.
    let single = [pair[0]];
    for (family_tests, ok) in [
        (0u64, false),
        (1, false),
        (2, true),
        (3, false),
        (4, true),
        (PRECISION_MAX_COUNT - 1, true),
        (PRECISION_MAX_COUNT, false),
        (PRECISION_MAX_COUNT + 1, false),
    ] {
        assert_eq!(
            run(family_tests, &single).is_ok(),
            ok,
            "familyTests {family_tests}"
        );
    }

    for out_of_domain in [
        ConfirmationDeclaration {
            alpha_ppm: 0,
            ..declaration.clone()
        },
        ConfirmationDeclaration {
            alpha_ppm: 1_000_000,
            ..declaration.clone()
        },
        ConfirmationDeclaration {
            block_length: 0,
            ..declaration.clone()
        },
        ConfirmationDeclaration {
            bootstrap_samples: 0,
            ..declaration.clone()
        },
        ConfirmationDeclaration {
            seed: PRECISION_MAX_COUNT + 1,
            ..declaration.clone()
        },
        ConfirmationDeclaration {
            block_length_rationale: String::new(),
            ..declaration.clone()
        },
    ] {
        let error = evaluate_confirmation(&out_of_domain, 2, &single).unwrap_err();
        assert!(error.0.contains("outside the"), "{error}");
    }
}

#[test]
fn a_candidate_is_resampled_the_same_way_in_any_batch() {
    let fixture = fixture();
    let case = &fixture["cases"][1];
    let declaration = parse_confirmation_declaration(&case["declaration"]).unwrap();
    let returns: Vec<Vec<f64>> = case["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| floats(&candidate["returns"]))
        .collect();
    let benchmark = floats(&case["candidates"][0]["benchmarkReturns"]);
    let series = |index: u64, which: usize| CandidateSeries {
        candidate_index: index,
        returns: &returns[which],
        benchmark_returns: &benchmark,
    };
    let together = evaluate_confirmation(&declaration, 8, &[series(0, 0), series(3, 1)]).unwrap();
    let reversed = evaluate_confirmation(&declaration, 8, &[series(3, 1), series(0, 0)]).unwrap();
    assert_eq!(together, reversed, "batch order does not matter");
    let alone = evaluate_confirmation(&declaration, 8, &[series(3, 1)]).unwrap();
    for (single, batched) in alone.tests.iter().zip(&together.tests[2..]) {
        assert_eq!(
            (single.extreme_count, single.raw_p),
            (batched.extreme_count, batched.raw_p),
            "the bootstrap itself ignores other candidates"
        );
    }
    // The stream is keyed by candidate index, not by position.
    let renamed = evaluate_confirmation(&declaration, 8, &[series(4, 1)]).unwrap();
    assert_ne!(
        renamed.tests[1].extreme_count, alone.tests[1].extreme_count,
        "fixture choice: this series resamples differently under index 4"
    );
    assert_eq!(
        evaluate_confirmation(&declaration, 8, &[series(3, 1)]).unwrap(),
        alone,
        "deterministic"
    );
}

#[test]
fn holm_uses_the_whole_family_and_never_loosens_as_it_grows() {
    // extremeCount = 0 for both tests, so everything below is exact.
    let batch = 2u64;
    let mut previous = 0u64;
    for family_tests in [2u64, 4, 10, 200, 1_000, 2_000, 4_000] {
        let report = evaluate_steady(50_000, 999, family_tests);
        let [net, excess] = [&report.tests[0], &report.tests[1]];
        assert_eq!((net.extreme_count, excess.extreme_count), (0, 0));
        assert_eq!(
            (net.holm_rank, excess.holm_rank),
            (1, 2),
            "ties break by test order"
        );
        let expected = family_tests.min(1_000);
        assert_eq!(
            net.adjusted_p,
            Ratio {
                numerator: expected,
                denominator: 1_000
            }
        );
        assert_eq!(
            excess.adjusted_p, net.adjusted_p,
            "the running maximum keeps ties equal"
        );
        assert!(
            net.adjusted_p.numerator >= previous && net.adjusted_p.numerator >= net.raw_p.numerator
        );
        previous = net.adjusted_p.numerator;

        // P12a's planning bound is exactly this calculation's best case.
        let plan = PrecisionPlan {
            alpha_ppm: 50_000,
            max_relative_standard_error_ppm: 200_000,
            prior_trials: family_tests / 2 - batch / 2,
            planned_trials: batch / 2,
            tests_per_trial: CONFIRMATION_TESTS_PER_TRIAL,
            bootstrap_samples: 999,
            max_bootstrap_samples: 999,
        };
        assert_eq!(
            evaluate_precision_plan(&plan).unwrap().best_adjusted_p,
            net.adjusted_p,
            "familyTests {family_tests}"
        );
    }
}

#[test]
fn rejection_is_an_exact_comparison_with_alpha() {
    // m = 10, B = 199: adjusted p = 10/200 = 0.05 exactly.
    for (alpha_ppm, rejects) in [(50_001u64, true), (50_000, true), (49_999, false)] {
        let report = evaluate_steady(alpha_ppm, 199, 10);
        assert_eq!(
            report.tests[0].adjusted_p,
            Ratio {
                numerator: 10,
                denominator: 200
            }
        );
        assert_eq!(
            report.tests[0].rejects_null, rejects,
            "alpha {alpha_ppm} ppm"
        );
        assert_eq!(report.tests[1].rejects_null, rejects);
    }
    // One fewer sample and the same family can no longer reach 0.05.
    let report = evaluate_steady(50_000, 198, 10);
    assert_eq!(
        report.tests[0].adjusted_p,
        Ratio {
            numerator: 10,
            denominator: 199
        }
    );
    assert!(!report.tests[0].rejects_null);
}

#[test]
fn a_block_as_long_as_the_series_cannot_manufacture_significance() {
    // With L = n every resample is a rotation with the observed sum, so the
    // centered statistic would never reach it and p would be 1/(B + 1).
    let (returns, benchmark) = steady();
    let degenerate = ConfirmationDeclaration {
        block_length: returns.len() as u64,
        ..declaration()
    };
    let error = evaluate_confirmation(
        &degenerate,
        2,
        &[CandidateSeries {
            candidate_index: 0,
            returns: &returns,
            benchmark_returns: &benchmark,
        }],
    )
    .unwrap_err();
    assert!(error.0.contains("squared exceeds"), "{error}");
}
