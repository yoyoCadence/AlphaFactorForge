use serde_json::{json, Value};

use super::*;

const FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-noise-simulation-v1.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

/// Eight simulations of two confirmations with two candidates each.
fn declaration_json() -> Value {
    fixture()["cases"][0]["declaration"].clone()
}

fn declaration() -> NoiseSimulationDeclaration {
    parse_noise_simulation(&declaration_json()).unwrap()
}

fn parse_error(raw: &Value) -> String {
    parse_noise_simulation(raw).unwrap_err().0
}

fn with(field: &str, value: Value) -> Value {
    let mut raw = declaration_json();
    raw[field] = value;
    raw
}

fn report_json(raw: &Value) -> Value {
    let declaration = parse_noise_simulation(raw).unwrap();
    serde_json::to_value(simulate_noise(&declaration).unwrap()).unwrap()
}

fn acceptance(id: &str) -> Value {
    fixture()["acceptance"]
        .as_array()
        .unwrap()
        .iter()
        .find(|run| run["id"] == id)
        .unwrap_or_else(|| panic!("no acceptance run {id}"))
        .clone()
}

/// The declaration of §7, written out so an edit to the fixture's seed,
/// size or tolerance cannot pass unnoticed.
fn declared_acceptance_run(autocorrelation_ppm: u64) -> NoiseSimulationDeclaration {
    NoiseSimulationDeclaration {
        autocorrelation_ppm,
        bars: 256,
        candidates_per_confirmation: 1,
        prior_trials: 0,
        block_length: 6,
        bootstrap_samples: 799,
        simulations: 2_000,
        seed: 20_261_002,
        tolerance_ppm: 10_000,
        allocation: AlphaAllocationDeclaration {
            total_alpha_ppm: 50_000,
            schedule: vec![25_000, 25_000],
        },
    }
}

fn assert_acceptance_run(id: &str, autocorrelation_ppm: u64) {
    let run = acceptance(id);
    let declaration = parse_noise_simulation(&run["declaration"]).unwrap();
    assert_eq!(declaration, declared_acceptance_run(autocorrelation_ppm));
    let report = simulate_noise(&declaration).unwrap();
    assert_eq!(report.nominal_alpha_ppm, 50_000);
    assert_eq!(report.nominal_standard_error_ppm, 4_873);
    assert_eq!(report.limit_false_positives, 120);
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        run["expected"],
        "{id}"
    );
}

#[test]
fn small_cases_reproduce_the_independent_reference() {
    let fixture = fixture();
    assert_eq!(fixture["contractVersion"], NOISE_SIMULATION_VERSION);
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    let mut statuses = Vec::new();
    for case in cases {
        let actual = report_json(&case["declaration"]);
        assert_eq!(actual, case["expected"], "{}", case["id"]);
        statuses.push(actual["status"].as_str().unwrap().to_string());
    }
    // Both outcomes and both kinds of noise are pinned by the reference.
    assert!(statuses.iter().any(|status| status == "WITHIN_TOLERANCE"));
    assert!(statuses.iter().any(|status| status == "EXCEEDS_TOLERANCE"));
    assert!(cases
        .iter()
        .any(|case| case["expected"]["familyFalsePositives"] != json!(0)));
}

#[test]
fn the_declared_independent_noise_run_reproduces_its_committed_report() {
    assert_acceptance_run("independent-noise", 0);
}

#[test]
fn the_declared_serially_correlated_run_reproduces_its_committed_report() {
    assert_acceptance_run("serially-correlated-noise", 300_000);
}

/// Prints the acceptance reports for the fixture (contract §8):
/// `cargo test --lib noise_simulation::tests::print_acceptance_reports -- --ignored --nocapture`.
#[test]
#[ignore = "prints the acceptance reports; run by hand when the contract version changes"]
fn print_acceptance_reports() {
    let mut reports = serde_json::Map::new();
    for run in fixture()["acceptance"].as_array().unwrap() {
        reports.insert(
            run["id"].as_str().unwrap().to_string(),
            report_json(&run["declaration"]),
        );
    }
    println!(
        "{}",
        serde_json::to_string(&Value::Object(reports)).unwrap()
    );
}

#[test]
fn the_declaration_is_parsed_strictly_in_a_fixed_rejection_order() {
    assert_eq!(parse_error(&json!([])), "simulation: must be an object");
    assert_eq!(parse_error(&json!(null)), "simulation: must be an object");

    // Unknown fields come first, in sorted order, even when others are wrong.
    let mut raw = with("noiseModel", json!("garch"));
    raw["zeta"] = json!(1);
    raw["adaptiveTolerance"] = json!(true);
    assert_eq!(
        parse_error(&raw),
        "simulation.adaptiveTolerance: unknown field"
    );

    // Then the fields in declared order.
    let complete = declaration_json();
    let mut raw = json!({});
    for field in FIELDS {
        assert_eq!(parse_error(&raw), format!("simulation.{field}: required"));
        raw[field] = complete[field].clone();
    }
    assert_eq!(parse_noise_simulation(&raw).unwrap(), declaration());

    assert_eq!(
        parse_error(&with(
            "contractVersion",
            json!("research-noise-simulation-v2")
        )),
        "simulation.contractVersion: must be \"research-noise-simulation-v1\""
    );
    assert_eq!(
        parse_error(&with("confirmationContract", json!("anything"))),
        "simulation.confirmationContract: must be \"research-confirmation-statistics-v1\""
    );
    assert_eq!(
        parse_error(&with("noiseModel", json!("iid-gaussian"))),
        "simulation.noiseModel: must be \"ar1-uniform-sum\""
    );

    // An earlier field wins over a later one.
    let mut raw = with("bars", json!(1));
    raw["simulations"] = json!(0);
    raw["allocation"] = json!({});
    assert_eq!(
        parse_error(&raw),
        "simulation.bars: must be an integer in [2, 1000000]"
    );

    // The nested budget is parsed by its own contract, in its order.
    assert_eq!(
        parse_error(&with("allocation", json!([]))),
        "simulation.allocation: must be an object"
    );
    let mut raw = declaration_json();
    raw["allocation"]["schedule"] = json!([300_000, 300_001]);
    assert_eq!(
        parse_error(&raw),
        "simulation.allocation.schedule: allocates 600001 ppm, above totalAlphaPpm 600000"
    );
    raw["allocation"]["scope"] = json!("campaign");
    assert_eq!(
        parse_error(&raw),
        "simulation.allocation.scope: must be \"trial-family\""
    );
}

#[test]
fn every_count_is_an_integer_inside_its_domain() {
    let max = PRECISION_MAX_COUNT;
    for (field, min, limit) in [
        ("autocorrelationPpm", 0u64, 999_999u64),
        ("bars", 2, NOISE_MAX_BARS),
        ("candidatesPerConfirmation", 1, NOISE_MAX_CANDIDATES),
        ("priorTrials", 0, max),
        ("blockLength", 1, max),
        ("bootstrapSamples", 1, max),
        ("simulations", 1, NOISE_MAX_SIMULATIONS),
        ("seed", 0, max),
        ("tolerancePpm", 0, 999_999),
    ] {
        let message = format!("simulation.{field}: must be an integer in [{min}, {limit}]");
        let mut bad = vec![
            json!(limit + 1),
            json!(-1),
            json!(2.5),
            json!(4.0),
            json!("4"),
            json!(null),
        ];
        if min > 0 {
            bad.push(json!(min - 1));
        }
        for value in bad {
            assert_eq!(parse_error(&with(field, value.clone())), message, "{value}");
        }
    }
    // Both ends of a cheap domain are accepted.
    for (field, value) in [
        ("autocorrelationPpm", 999_999u64),
        ("tolerancePpm", 999_999),
        ("seed", max),
        ("seed", 0),
        ("priorTrials", 0),
    ] {
        assert!(
            parse_noise_simulation(&with(field, json!(value))).is_ok(),
            "{field}"
        );
    }

    // The confirmation contract's block rule, on 16 bars: 4 fits, 5 does not.
    assert!(parse_noise_simulation(&with("blockLength", json!(4))).is_ok());
    assert_eq!(
        parse_error(&with("blockLength", json!(5))),
        "simulation.blockLength: 5 squared exceeds bars 16"
    );

    // The last confirmation's family count must stay a safe integer:
    // 2 x (priorTrials + 2 confirmations x 2 candidates).
    let most = max / 2 - 4;
    assert!(parse_noise_simulation(&with("priorTrials", json!(most))).is_ok());
    assert_eq!(
        parse_error(&with("priorTrials", json!(most + 1))),
        format!("simulation: family tests at confirmation 2 exceed {max}")
    );
}

#[test]
fn a_directly_constructed_declaration_cannot_bypass_the_domain() {
    let refused = |change: fn(&mut NoiseSimulationDeclaration)| {
        let mut declaration = declaration();
        change(&mut declaration);
        simulate_noise(&declaration).unwrap_err().0
    };
    assert_eq!(
        refused(|declaration| declaration.simulations = 0),
        "simulation.simulations: must be an integer in [1, 1000000]"
    );
    assert_eq!(
        refused(|declaration| declaration.autocorrelation_ppm = 1_000_000),
        "simulation.autocorrelationPpm: must be an integer in [0, 999999]"
    );
    assert_eq!(
        refused(|declaration| declaration.bars = 3),
        "simulation.blockLength: 2 squared exceeds bars 3"
    );
    assert_eq!(
        refused(|declaration| declaration.allocation.schedule.clear()),
        "simulation.allocation.schedule: must be a non-empty array"
    );
    assert_eq!(
        refused(|declaration| declaration.allocation.schedule = vec![600_000, 1]),
        "simulation.allocation.schedule: allocates 600001 ppm, above totalAlphaPpm 600000"
    );
}

#[test]
fn the_noise_is_zero_mean_with_the_declared_autocorrelation() {
    let moments = |phi_ppm: u64| {
        let phi = phi_ppm as f64 / 1_000_000.0;
        let series = noise_series(&mut stream(42, &[DOMAIN_DATA, phi_ppm]), phi, 200_000);
        assert_eq!(series.len(), 200_000);
        let n = series.len() as f64;
        let mean = series.iter().sum::<f64>() / n;
        let variance = series.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
        let lag_one = series
            .windows(2)
            .map(|pair| (pair[0] - mean) * (pair[1] - mean))
            .sum::<f64>()
            / n
            / variance;
        (mean, variance, lag_one, series)
    };

    // Independent: four uniforms minus two lie in [-2, 2) with variance 1/3.
    let (mean, variance, lag_one, series) = moments(0);
    assert!(series.iter().all(|value| (-2.0..2.0).contains(value)));
    assert!(mean.abs() < 0.01, "{mean}");
    assert!((variance - 1.0 / 3.0).abs() < 0.01, "{variance}");
    assert!(lag_one.abs() < 0.01, "{lag_one}");

    // AR(1) 0.3: lag-one correlation 0.3, variance (1/3) / (1 - 0.09).
    let (mean, variance, lag_one, _) = moments(300_000);
    assert!(mean.abs() < 0.01, "{mean}");
    assert!((variance - (1.0 / 3.0) / 0.91).abs() < 0.01, "{variance}");
    assert!((lag_one - 0.3).abs() < 0.01, "{lag_one}");

    // The warm-up is discarded: a series starts where a longer warm-up-free
    // series would be after 64 bars, not at zero.
    let mut rng = stream(7, &[DOMAIN_DATA]);
    let kept = noise_series(&mut rng, 0.9, 3);
    let mut rng = stream(7, &[DOMAIN_DATA]);
    let mut value = 0.0f64;
    let mut all = Vec::new();
    for _ in 0..NOISE_WARMUP_BARS + 3 {
        value = 0.9 * value
            + (uniform(&mut rng) + uniform(&mut rng) + uniform(&mut rng) + uniform(&mut rng) - 2.0);
        all.push(value);
    }
    assert_eq!(kept, all[NOISE_WARMUP_BARS..]);
}

#[test]
fn every_series_and_every_confirmation_has_its_own_stream() {
    let first = |parts: &[u64]| stream(20_261_002, parts).next_u64();
    let mut seen = std::collections::BTreeSet::new();
    for simulation in 0..4 {
        for confirmation in 1..=3 {
            assert!(seen.insert(first(&[DOMAIN_CONFIRMATION_SEED, simulation, confirmation])));
            for candidate in 0..3 {
                for role in [ROLE_RETURNS, ROLE_BENCHMARK] {
                    assert!(seen.insert(first(&[
                        DOMAIN_DATA,
                        simulation,
                        confirmation,
                        candidate,
                        role
                    ])));
                }
            }
        }
    }
    assert_eq!(seen.len(), 4 * 3 * (1 + 3 * 2));
    // A key is the list, not its sum or its order.
    assert_ne!(first(&[0, 1, 2]), first(&[0, 2, 1]));
    assert_ne!(first(&[1, 0]), first(&[0, 1]));
    // The uniform draw is exact and below one.
    let mut rng = stream(1, &[]);
    for _ in 0..10_000 {
        let value = uniform(&mut rng);
        assert!((0.0..1.0).contains(&value));
        assert_eq!(
            value * 9_007_199_254_740_992.0,
            (value * 9_007_199_254_740_992.0).trunc()
        );
    }
}

#[test]
fn a_simulation_is_reproducible_and_its_seed_matters() {
    let declaration = declaration();
    assert_eq!(
        simulate_noise(&declaration).unwrap(),
        simulate_noise(&declaration).unwrap()
    );
    // Over many seeds the counts cannot all coincide.
    let counts: std::collections::BTreeSet<u64> = (0..12)
        .map(|seed| {
            let mut other = declaration.clone();
            other.seed = seed;
            simulate_noise(&other).unwrap().family_false_positives
        })
        .collect();
    assert!(counts.len() > 1, "{counts:?}");
}

#[test]
fn the_whole_schedule_is_simulated_with_the_allocated_shares_and_a_growing_family() {
    let mut declaration = declaration();
    declaration.allocation = AlphaAllocationDeclaration {
        total_alpha_ppm: 900_000,
        schedule: vec![400_000, 300_000, 100_000, 50_000],
    };
    let report = simulate_noise(&declaration).unwrap();
    assert_eq!(report.nominal_alpha_ppm, 850_000);
    assert_eq!(
        report.allocation_id,
        alpha_allocation_id(&declaration.allocation).unwrap()
    );
    let rows: Vec<(u64, u64, u64)> = report
        .confirmations
        .iter()
        .map(|row| (row.confirmation_number, row.alpha_ppm, row.family_tests))
        .collect();
    // priorTrials 1, two candidates per confirmation, two tests per trial.
    assert_eq!(
        rows,
        [
            (1, 400_000, 6),
            (2, 300_000, 10),
            (3, 100_000, 14),
            (4, 50_000, 18)
        ]
    );
}

#[test]
fn a_family_false_positive_is_any_rejection_in_any_confirmation() {
    let mut reports = vec![simulate_noise(&declaration()).unwrap()];
    for case in fixture()["cases"].as_array().unwrap() {
        let declaration = parse_noise_simulation(&case["declaration"]).unwrap();
        reports.push(simulate_noise(&declaration).unwrap());
    }
    let mut wide = declaration();
    wide.simulations = 40;
    wide.prior_trials = 0;
    wide.candidates_per_confirmation = 1;
    wide.bootstrap_samples = 199;
    wide.allocation = AlphaAllocationDeclaration {
        total_alpha_ppm: 900_000,
        schedule: vec![300_000, 300_000, 300_000],
    };
    let wide = simulate_noise(&wide).unwrap();
    // The cross-batch effect is real: the family rate is above every single
    // confirmation's rate here.
    assert!(wide
        .confirmations
        .iter()
        .all(|row| row.false_positives < wide.family_false_positives));
    reports.push(wide);

    for report in &reports {
        let per_confirmation: Vec<u64> = report
            .confirmations
            .iter()
            .map(|row| row.false_positives)
            .collect();
        // At least the worst confirmation, at most all of them together.
        assert!(report.family_false_positives >= *per_confirmation.iter().max().unwrap());
        assert!(report.family_false_positives <= per_confirmation.iter().sum::<u64>());
        assert!(report.family_false_positives <= report.simulations);
        for row in &report.confirmations {
            // A confirmation that rejected rejected at least one test.
            assert!(
                row.false_positives <= row.net_return_rejections + row.benchmark_excess_rejections
            );
            assert_eq!(
                row.false_positive_rate_ppm,
                row.false_positives * 1_000_000 / report.simulations
            );
        }
        assert_eq!(
            report.family_false_positive_rate,
            Ratio {
                numerator: report.family_false_positives,
                denominator: report.simulations
            }
        );
    }
}

#[test]
fn untested_family_members_only_make_the_family_more_conservative() {
    let mut previous = u64::MAX;
    for prior_trials in [0u64, 1, 2, 5, 50] {
        let mut declaration = declaration();
        declaration.prior_trials = prior_trials;
        let report = simulate_noise(&declaration).unwrap();
        // The data and the bootstrap streams do not depend on priorTrials;
        // only the Holm family grows.
        assert!(report.family_false_positives <= previous, "{prior_trials}");
        previous = report.family_false_positives;
    }
    assert_eq!(
        previous, 0,
        "fifty untested trials leave nothing rejectable at 19 samples"
    );
}

#[test]
fn the_tolerance_is_an_exact_integer_boundary() {
    // One confirmation at 300,000 ppm; the reference counts 3 of 8.
    let raw = fixture()["cases"][3]["declaration"].clone();
    let run = |tolerance_ppm: u64| {
        let mut declaration = parse_noise_simulation(&raw).unwrap();
        declaration.tolerance_ppm = tolerance_ppm;
        simulate_noise(&declaration).unwrap()
    };
    // 3/8 = 0.375 = 0.300 + 0.075 exactly.
    let at = run(75_000);
    let below = run(74_999);
    assert_eq!(at.family_false_positives, 3);
    assert_eq!(
        below.family_false_positives, 3,
        "the tolerance never changes the counts"
    );
    assert_eq!(at.status, NoiseSimulationStatus::WithinTolerance);
    assert_eq!(below.status, NoiseSimulationStatus::ExceedsTolerance);
    assert_eq!(at.limit_false_positives, 3);
    assert_eq!(below.limit_false_positives, 2);
    // A limit above every simulation is capped at the simulation count.
    assert_eq!(run(999_999).limit_false_positives, 8);

    assert_eq!(nominal_standard_error_ppm(50_000, 2_000), 4_873);
    assert_eq!(nominal_standard_error_ppm(50_000, 1), 217_944);
    assert_eq!(nominal_standard_error_ppm(500_000, 1_000_000), 500);
    assert_eq!(nominal_standard_error_ppm(1, 1_000_000), 0);
}

#[test]
fn the_report_is_a_measurement_and_never_a_pass() {
    let report = report_json(&declaration_json());
    let mut keys: Vec<&str> = report
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "allocationId",
            "confirmationContract",
            "confirmations",
            "contractVersion",
            "familyFalsePositiveRate",
            "familyFalsePositiveRatePpm",
            "familyFalsePositives",
            "limitFalsePositives",
            "nominalAlphaPpm",
            "nominalStandardErrorPpm",
            "simulations",
            "status",
            "tolerancePpm",
        ]
    );
    for case in fixture()["cases"].as_array().unwrap() {
        let report = report_json(&case["declaration"]);
        assert!(!report.to_string().contains("PASS"));
        assert!(
            ["WITHIN_TOLERANCE", "EXCEEDS_TOLERANCE"].contains(&report["status"].as_str().unwrap())
        );
    }
}
