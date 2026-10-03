use serde_json::{json, Value};

use super::*;
use crate::discovery_core::noise_simulation::{parse_noise_simulation, simulate_noise};

const FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-noise-simulation-v2.json");
const V1_FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-noise-simulation-v1.json");
/// P12e-6b: the recalibration plan's diagnostic declarations and reports.
const DIAGNOSTICS: &str =
    include_str!("../../../../fixtures/research/recalibration-plan-v1-diagnostics.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

/// Eight simulations, two confirmations of two candidates, a point screen and
/// two checkpoints.
fn declaration_json() -> Value {
    fixture()["cases"][0]["declaration"].clone()
}

fn declaration() -> NoiseSimulationV2Declaration {
    parse_noise_simulation_v2(&declaration_json()).unwrap()
}

fn parse_error(raw: &Value) -> String {
    parse_noise_simulation_v2(raw).unwrap_err().0
}

fn with(field: &str, value: Value) -> Value {
    let mut raw = declaration_json();
    raw[field] = value;
    raw
}

fn report_json(raw: &Value) -> Value {
    let declaration = parse_noise_simulation_v2(raw).unwrap();
    serde_json::to_value(simulate_noise_v2(&declaration).unwrap()).unwrap()
}

fn rule(rule: NoiseCheckRule, limit_multiplier_ppm: Option<u64>) -> NoiseCheck {
    NoiseCheck {
        rule,
        limit_multiplier_ppm,
    }
}

/// The floating-point Wilson interval, for comparison only.
fn wilson_f64(count: u64, simulations: u64) -> (f64, f64) {
    let (x, n, z) = (count as f64, simulations as f64, 1.96f64);
    let p = x / n;
    let centre = p + z * z / (2.0 * n);
    let spread = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt();
    let scale = 1.0 + z * z / n;
    ((centre - spread) / scale, (centre + spread) / scale)
}

#[test]
fn fixture_cases_reproduce_the_independent_reference() {
    let fixture = fixture();
    assert_eq!(fixture["contractVersion"], NOISE_SIMULATION_V2_VERSION);
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 7);
    let mut statuses = std::collections::BTreeSet::new();
    for case in cases {
        let actual = report_json(&case["declaration"]);
        assert_eq!(actual, case["expected"], "{}", case["id"]);
        statuses.insert(actual["status"].as_str().unwrap().to_string());
        // The engine fixture must never run a cell of the recalibration plan.
        let seed = case["declaration"]["seed"].as_u64().unwrap();
        assert!(seed != 20_261_005 && seed != 20_261_117, "{}", case["id"]);
        assert!(case["declaration"]["simulations"].as_u64().unwrap() < 100);
    }
    assert_eq!(
        statuses.into_iter().collect::<Vec<_>>(),
        ["EXCEEDS_LIMITS", "MEASURED", "WITHIN_LIMITS"]
    );
}

#[test]
fn the_null_scenario_is_the_v1_simulation_with_more_reported() {
    let v1: Value = serde_json::from_str(V1_FIXTURE).unwrap();
    for case in v1["cases"].as_array().unwrap() {
        let old = parse_noise_simulation(&case["declaration"]).unwrap();
        let old_report = simulate_noise(&old).unwrap();
        let new = NoiseSimulationV2Declaration {
            statistic: SimulatedStatistic::ConfirmationV1,
            autocorrelation_ppm: old.autocorrelation_ppm,
            effect_millionths: 0,
            bars: old.bars,
            candidates_per_confirmation: old.candidates_per_confirmation,
            prior_trials: old.prior_trials,
            block_length: old.block_length,
            bootstrap_samples: old.bootstrap_samples,
            simulations: old.simulations,
            seed: old.seed,
            check: rule(NoiseCheckRule::None, None),
            checkpoints: Vec::new(),
            allocation: old.allocation.clone(),
        };
        let new_report = simulate_noise_v2(&new).unwrap();
        let id = &case["id"];
        assert_eq!(new_report.allocation_id, old_report.allocation_id, "{id}");
        assert_eq!(
            new_report.family.count, old_report.family_false_positives,
            "{id}"
        );
        assert_eq!(new_report.family.nominal_ppm, old_report.nominal_alpha_ppm);
        for (new_row, old_row) in new_report
            .confirmations
            .iter()
            .zip(&old_report.confirmations)
        {
            assert_eq!(new_row.alpha_ppm, old_row.alpha_ppm, "{id}");
            assert_eq!(new_row.family_tests, old_row.family_tests, "{id}");
            assert_eq!(new_row.check.count, old_row.false_positives, "{id}");
            assert_eq!(
                new_row.counts.rejecting_simulations,
                old_row.false_positives
            );
            // v1 counts rejected tests; with one candidate that is the number
            // of simulations in which the test rejected.
            if old.candidates_per_confirmation == 1 {
                assert_eq!(
                    new_row.counts.net_return_rejecting_simulations, old_row.net_return_rejections,
                    "{id}"
                );
                assert_eq!(
                    new_row.counts.benchmark_excess_rejecting_simulations,
                    old_row.benchmark_excess_rejections,
                    "{id}"
                );
            } else {
                assert!(
                    new_row.counts.net_return_rejecting_simulations
                        <= old_row.net_return_rejections
                );
            }
        }
    }
}

#[test]
fn a_run_cut_at_a_checkpoint_reproduces_that_checkpoint() {
    let mut declaration = declaration();
    declaration.simulations = 30;
    declaration.checkpoints = vec![1, 4, 9, 20, 29];
    declaration.check = rule(NoiseCheckRule::None, None);
    let full = simulate_noise_v2(&declaration).unwrap();
    assert_eq!(
        full.checkpoints
            .iter()
            .map(|checkpoint| checkpoint.simulations)
            .collect::<Vec<_>>(),
        declaration.checkpoints
    );
    for checkpoint in &full.checkpoints {
        let mut prefix = declaration.clone();
        prefix.simulations = checkpoint.simulations;
        prefix.checkpoints.clear();
        let cut = simulate_noise_v2(&prefix).unwrap();
        assert_eq!(cut.family.count, checkpoint.family_rejecting_simulations);
        let counts: Vec<NoiseConfirmationCounts> = cut
            .confirmations
            .into_iter()
            .map(|row| row.counts)
            .collect();
        assert_eq!(
            counts, checkpoint.confirmations,
            "{}",
            checkpoint.simulations
        );
    }
    // Running counts never fall, and the last checkpoint is within the totals.
    for pair in full.checkpoints.windows(2) {
        assert!(pair[0].family_rejecting_simulations <= pair[1].family_rejecting_simulations);
        for (earlier, later) in pair[0].confirmations.iter().zip(&pair[1].confirmations) {
            assert!(earlier.net_return_extreme_total <= later.net_return_extreme_total);
            assert!(earlier.rejecting_simulations <= later.rejecting_simulations);
        }
    }
    let last = full.checkpoints.last().unwrap();
    assert!(last.family_rejecting_simulations <= full.family.count);
    // Declaring checkpoints changes nothing else.
    let mut plain = declaration.clone();
    plain.checkpoints.clear();
    let plain = simulate_noise_v2(&plain).unwrap();
    assert_eq!(plain.family, full.family);
    assert_eq!(plain.confirmations, full.confirmations);
    assert!(plain.checkpoints.is_empty());
    // The digest moves with the seed even where no count does.
    let mut other = declaration.clone();
    other.seed += 1;
    let other = simulate_noise_v2(&other).unwrap();
    assert_ne!(
        other.confirmations[0].counts.net_return_extreme_total,
        full.confirmations[0].counts.net_return_extreme_total
    );
}

#[test]
fn the_wilson_rule_is_the_plans_and_agrees_with_the_reported_bound() {
    // Recalibration plan §7: 20,000 simulations, limits 3% and 6%.
    assert!(wilson_upper_within(552, 20_000, 30_000));
    assert!(!wilson_upper_within(553, 20_000, 30_000));
    assert!(wilson_upper_within(1_134, 20_000, 60_000));
    assert!(!wilson_upper_within(1_135, 20_000, 60_000));
    assert_eq!(wilson_bounds_ppm(552, 20_000).1, 29_963);
    assert_eq!(wilson_bounds_ppm(553, 20_000).1, 30_015);
    assert_eq!(wilson_bounds_ppm(1_134, 20_000).1, 59_992);
    assert_eq!(wilson_bounds_ppm(1_135, 20_000).1, 60_043);
    // The v1 acceptance results, as the PR #135 review computed them.
    assert_eq!(wilson_bounds_ppm(129, 2_000), (54_547, 76_123));
    assert_eq!(wilson_bounds_ppm(97, 2_000), (39_919, 58_812));

    // The decision without a square root and the bound with one are the same
    // statement, for every count and a spread of limits.
    for simulations in [1u64, 2, 5, 7, 100, 999, 4_000] {
        let mut previous = (0u64, 0u64);
        for count in 0..=simulations {
            let (lower, upper) = wilson_bounds_ppm(count, simulations);
            let rate_ppm = count * 1_000_000 / simulations;
            assert!(lower <= rate_ppm && rate_ppm <= upper && upper <= 1_000_000);
            assert!(
                lower >= previous.0 && upper >= previous.1,
                "monotone in the count"
            );
            previous = (lower, upper);
            let (exact_lower, exact_upper) = wilson_f64(count, simulations);
            assert!(
                (lower as f64 - exact_lower * 1e6).abs() <= 1.0,
                "{count}/{simulations}"
            );
            assert!(
                (upper as f64 - exact_upper * 1e6).abs() <= 1.0,
                "{count}/{simulations}"
            );
            for limit in [1u64, 2_999, 30_000, 60_000, 250_000, 500_000, 999_999] {
                assert_eq!(
                    wilson_upper_within(count, simulations, limit),
                    upper <= limit,
                    "{count}/{simulations} against {limit}"
                );
            }
        }
        assert_eq!(wilson_bounds_ppm(0, simulations).0, 0);
        assert_eq!(wilson_bounds_ppm(simulations, simulations).1, 1_000_000);
    }
    // An upper bound that equals the limit exactly is within it: at 4902 of
    // 10,000 the bound is 50% to the last digit (N(L - p)^2 = z^2 L(1 - L) =
    // 0.9604).
    assert_eq!(wilson_bounds_ppm(4_902, 10_000).1, 500_000);
    assert!(wilson_upper_within(4_902, 10_000, 500_000));
    assert!(!wilson_upper_within(4_902, 10_000, 499_999));
    assert!(!wilson_upper_within(4_903, 10_000, 500_000));
    // A limit of 100% or more is not a limit, and the largest size is exact.
    assert!(!wilson_upper_within(0, 1_000_000, 1_000_000));
    let (lower, upper) = wilson_bounds_ppm(500_000, 1_000_000);
    assert_eq!((lower, upper), (499_020, 500_980));
    assert_eq!(
        wilson_bounds_ppm(1_000_000, 1_000_000),
        (999_996, 1_000_000)
    );
}

#[test]
fn each_rule_is_an_exact_boundary_on_every_check() {
    // The plan's diagnostic screen: 3.0% of 4,000 is 120.
    let screen = rule(NoiseCheckRule::PointScreen, Some(1_200_000));
    let check = |count| rate_check(25_000, count, 4_000, &screen).unwrap();
    assert_eq!(check(120).limit_ppm, Some(30_000));
    assert_eq!(check(120).within_limit, Some(true));
    assert_eq!(check(121).within_limit, Some(false));
    assert_eq!(
        rate_check(50_000, 240, 4_000, &screen)
            .unwrap()
            .within_limit,
        Some(true)
    );
    assert_eq!(
        rate_check(50_000, 241, 4_000, &screen)
            .unwrap()
            .within_limit,
        Some(false)
    );
    // The acceptance rule on the same sizes is stricter than the screen.
    let bound = rule(NoiseCheckRule::WilsonUpperBound, Some(1_200_000));
    assert_eq!(
        rate_check(25_000, 98, 4_000, &bound).unwrap().within_limit,
        Some(true)
    );
    assert_eq!(
        rate_check(25_000, 99, 4_000, &bound).unwrap().within_limit,
        Some(false)
    );
    let measured = rate_check(25_000, 99, 4_000, &rule(NoiseCheckRule::None, None)).unwrap();
    assert_eq!((measured.limit_ppm, measured.within_limit), (None, None));
    assert_eq!(measured.rate_ppm, 24_750);

    // One confirmation above its share fails the run even when the family is
    // within its own limit (fixture case 0).
    let report = simulate_noise_v2(&declaration()).unwrap();
    assert_eq!(report.family.within_limit, Some(true));
    assert_eq!(report.confirmations[0].check.within_limit, Some(false));
    assert_eq!(report.status, NoiseSimulationV2Status::ExceedsLimits);
    // And a looser multiplier that admits every check is within limits.
    let mut loose = declaration();
    loose.check = rule(NoiseCheckRule::PointScreen, Some(1_500_000));
    let loose = simulate_noise_v2(&loose).unwrap();
    assert_eq!(loose.confirmations[0].check.limit_ppm, Some(450_000));
    assert_eq!(loose.status, NoiseSimulationV2Status::WithinLimits);
    // The rule never changes what is counted.
    assert_eq!(loose.family.count, report.family.count);
    assert_eq!(
        loose.confirmations[0].counts,
        report.confirmations[0].counts
    );
}

#[test]
fn the_shifted_scenario_measures_power_and_leaves_the_benchmark_alone() {
    let raw = fixture()["cases"][2]["declaration"].clone();
    let shifted = parse_noise_simulation_v2(&raw).unwrap();
    assert_eq!(shifted.effect_millionths, 400_000);
    let power = simulate_noise_v2(&shifted).unwrap();
    assert_eq!(power.status, NoiseSimulationV2Status::Measured);
    let mut null = shifted.clone();
    null.effect_millionths = 0;
    let size = simulate_noise_v2(&null).unwrap();
    // The plan's power figure: simulations in which the first confirmation
    // rejects the net-return null.
    assert!(
        power.confirmations[0]
            .counts
            .net_return_rejecting_simulations
            > size.confirmations[0]
                .counts
                .net_return_rejecting_simulations
    );
    assert_eq!(
        power.confirmations[0]
            .counts
            .net_return_rejecting_simulations,
        shifted.simulations
    );
    // A positive shift can only lower an extreme count of the net-return
    // test, and it moves the excess test the same way.
    for (with_effect, without) in power.confirmations.iter().zip(&size.confirmations) {
        assert!(
            with_effect.counts.net_return_extreme_total < without.counts.net_return_extreme_total
        );
        assert!(
            with_effect.counts.benchmark_excess_extreme_total
                < without.counts.benchmark_excess_extreme_total
        );
    }
    // A rate check needs true nulls.
    let mut checked = shifted.clone();
    checked.check = rule(NoiseCheckRule::PointScreen, Some(1_200_000));
    assert_eq!(
        simulate_noise_v2(&checked).unwrap_err().0,
        "simulation.check.rule: must be \"none\" when effectMillionths is not 0"
    );
}

#[test]
fn the_declaration_is_parsed_strictly_in_a_fixed_rejection_order() {
    assert_eq!(parse_error(&json!([])), "simulation: must be an object");

    let mut raw = with("statistic", json!("made-up"));
    raw["zeta"] = json!(1);
    raw["adaptiveLimit"] = json!(true);
    assert_eq!(parse_error(&raw), "simulation.adaptiveLimit: unknown field");

    let complete = declaration_json();
    let mut raw = json!({});
    for field in FIELDS {
        assert_eq!(parse_error(&raw), format!("simulation.{field}: required"));
        raw[field] = complete[field].clone();
    }
    assert_eq!(parse_noise_simulation_v2(&raw).unwrap(), declaration());

    assert_eq!(
        parse_error(&with(
            "contractVersion",
            json!("research-noise-simulation-v1")
        )),
        "simulation.contractVersion: must be \"research-noise-simulation-v2\""
    );
    // The baseline and the two draft candidates are selectable, nothing else.
    for unknown in [
        json!("research-confirmation-statistics-v2"),
        json!("S1"),
        json!(1),
    ] {
        assert_eq!(
            parse_error(&with("statistic", unknown)),
            "simulation.statistic: must be one of \"research-confirmation-statistics-v1\", \"research-confirmation-candidate-s1-v1\", \"research-confirmation-candidate-s2-v1\""
        );
    }
    assert_eq!(
        parse_error(&with("noiseModel", json!("garch"))),
        "simulation.noiseModel: must be \"ar1-uniform-sum\""
    );

    // The check object, in its own order.
    for (check, message) in [
        (json!("point-screen"), "simulation.check: must be an object"),
        (
            json!({"rule": "none", "tolerancePpm": 1}),
            "simulation.check.tolerancePpm: unknown field",
        ),
        (json!({}), "simulation.check.rule: required"),
        (
            json!({"rule": "one-sided"}),
            "simulation.check.rule: must be \"none\", \"point-screen\" or \"wilson-upper-bound\"",
        ),
        (
            json!({"rule": "none", "limitMultiplierPpm": 1_200_000}),
            "simulation.check.limitMultiplierPpm: not allowed with rule \"none\"",
        ),
        (
            json!({"rule": "point-screen"}),
            "simulation.check.limitMultiplierPpm: required",
        ),
        (
            json!({"rule": "wilson-upper-bound", "limitMultiplierPpm": 999_999}),
            "simulation.check.limitMultiplierPpm: must be an integer in [1000000, 10000000]",
        ),
        (
            json!({"rule": "wilson-upper-bound", "limitMultiplierPpm": 1.2}),
            "simulation.check.limitMultiplierPpm: must be an integer in [1000000, 10000000]",
        ),
    ] {
        assert_eq!(parse_error(&with("check", check)), message);
    }

    // Checkpoints: an array of rising counts below the simulation count (8).
    for (checkpoints, message) in [
        (json!(3), "simulation.checkpoints: must be an array"),
        (
            json!([0]),
            "simulation.checkpoints[0]: must be above the previous entry and below simulations",
        ),
        (
            json!([3, 3]),
            "simulation.checkpoints[1]: must be above the previous entry and below simulations",
        ),
        (
            json!([5, 3]),
            "simulation.checkpoints[1]: must be above the previous entry and below simulations",
        ),
        (
            json!([3, 8]),
            "simulation.checkpoints[1]: must be above the previous entry and below simulations",
        ),
        (
            json!([2.0]),
            "simulation.checkpoints[0]: must be above the previous entry and below simulations",
        ),
    ] {
        assert_eq!(parse_error(&with("checkpoints", checkpoints)), message);
    }
    assert!(parse_noise_simulation_v2(&with("checkpoints", json!([1, 7]))).is_ok());
    let mut many = with("simulations", json!(100));
    many["checkpoints"] = json!((1..=17).collect::<Vec<u64>>());
    assert_eq!(
        parse_error(&many),
        "simulation.checkpoints: at most 16 entries"
    );

    // An earlier field wins, and the nested budget keeps its own messages.
    let mut raw = with("bars", json!(1));
    raw["check"] = json!({});
    assert_eq!(
        parse_error(&raw),
        "simulation.bars: must be an integer in [2, 1000000]"
    );
    let mut raw = declaration_json();
    raw["allocation"]["scope"] = json!("campaign");
    assert_eq!(
        parse_error(&raw),
        "simulation.allocation.scope: must be \"trial-family\""
    );
}

#[test]
fn every_count_is_inside_its_domain_and_every_limit_is_a_whole_ppm() {
    for (field, min, limit) in [
        ("autocorrelationPpm", 0u64, 900_000u64),
        ("effectMillionths", 0, NOISE_V2_MAX_EFFECT_MILLIONTHS),
        ("bootstrapSamples", 1, NOISE_V2_MAX_BOOTSTRAP_SAMPLES),
        ("simulations", 1, 1_000_000),
        ("seed", 0, PRECISION_MAX_COUNT),
    ] {
        let message = format!("simulation.{field}: must be an integer in [{min}, {limit}]");
        for value in [
            json!(limit + 1),
            json!(-1),
            json!(2.5),
            json!("4"),
            json!(null),
        ] {
            assert_eq!(parse_error(&with(field, value)), message);
        }
    }
    assert_eq!(
        parse_error(&with("blockLength", json!(5))),
        "simulation.blockLength: 5 squared exceeds bars 16"
    );

    // 300,000 x 1.2 = 360,000 ppm: a whole number. A multiplier that leaves a
    // fraction of a ppm, or reaches 100%, is refused rather than rounded.
    let limited = |multiplier: u64, schedule: Value, total: u64| {
        let mut raw = declaration_json();
        raw["check"] = json!({"rule": "point-screen", "limitMultiplierPpm": multiplier});
        raw["allocation"]["schedule"] = schedule;
        raw["allocation"]["totalAlphaPpm"] = json!(total);
        parse_noise_simulation_v2(&raw)
            .map(|_| ())
            .map_err(|error| error.0)
    };
    assert_eq!(
        limited(1_200_000, json!([300_000, 300_000]), 600_000),
        Ok(())
    );
    assert_eq!(limited(1_000_000, json!([16_666, 16_666]), 50_000), Ok(()));
    assert_eq!(
        limited(1_200_000, json!([16_666, 16_666]), 50_000),
        Err("simulation.check.limitMultiplierPpm: the limit of a nominal 16666 ppm must be a whole number of ppm below 1000000".into())
    );
    // Each share's limit fits, the family's does not: 900,000 x 1.2 >= 100%.
    assert_eq!(
        limited(1_200_000, json!([450_000, 450_000]), 900_000),
        Err("simulation.check.limitMultiplierPpm: the limit of a nominal 900000 ppm must be a whole number of ppm below 1000000".into())
    );
    assert_eq!(limit_ppm(25_000, 1_200_000), Ok(30_000));
    assert_eq!(limit_ppm(50_000, 1_200_000), Ok(60_000));
}

#[test]
fn a_directly_constructed_declaration_cannot_bypass_the_domain() {
    let refused = |change: fn(&mut NoiseSimulationV2Declaration)| {
        let mut declaration = declaration();
        change(&mut declaration);
        simulate_noise_v2(&declaration).unwrap_err().0
    };
    assert_eq!(
        refused(|declaration| declaration.simulations = 0),
        "simulation.simulations: must be an integer in [1, 1000000]"
    );
    assert_eq!(
        refused(|declaration| declaration.bootstrap_samples = 1_000_001),
        "simulation.bootstrapSamples: must be an integer in [1, 1000000]"
    );
    assert_eq!(
        refused(|declaration| declaration.check.limit_multiplier_ppm = None),
        "simulation.check.limitMultiplierPpm: required"
    );
    assert_eq!(
        refused(|declaration| declaration.check.rule = NoiseCheckRule::None),
        "simulation.check.limitMultiplierPpm: not allowed with rule \"none\""
    );
    assert_eq!(
        refused(|declaration| declaration.checkpoints = vec![5, 3]),
        "simulation.checkpoints[1]: must be above the previous entry and below simulations"
    );
    assert_eq!(
        refused(|declaration| declaration.checkpoints = vec![8]),
        "simulation.checkpoints[0]: must be above the previous entry and below simulations"
    );
    assert_eq!(
        refused(|declaration| declaration.effect_millionths = 1),
        "simulation.check.rule: must be \"none\" when effectMillionths is not 0"
    );
    assert_eq!(
        refused(|declaration| declaration.allocation.schedule = vec![600_000, 1]),
        "simulation.allocation.schedule: allocates 600001 ppm, above totalAlphaPpm 600000"
    );
}

/// P12e-6a: the draft candidates are reachable only through this engine.
#[test]
fn the_draft_candidates_are_selectable_and_differ_from_the_baseline() {
    let s1 = fixture()["cases"][5]["declaration"].clone();
    let s2 = fixture()["cases"][6]["declaration"].clone();
    assert_eq!(s1["statistic"], "research-confirmation-candidate-s1-v1");
    assert_eq!(s2["statistic"], "research-confirmation-candidate-s2-v1");
    assert_eq!(
        parse_noise_simulation_v2(&s1).unwrap().statistic,
        SimulatedStatistic::CandidateS1
    );
    assert_eq!(
        report_json(&s2)["statistic"],
        "research-confirmation-candidate-s2-v1"
    );

    // Same noise and same block draws: only the statistic differs, and the
    // extreme-count digest shows it.
    let digests: Vec<Value> = [
        "research-confirmation-statistics-v1",
        "research-confirmation-candidate-s1-v1",
        "research-confirmation-candidate-s2-v1",
    ]
    .into_iter()
    .map(|statistic| {
        let mut raw = s2.clone();
        raw["statistic"] = json!(statistic);
        report_json(&raw)["confirmations"][0]["counts"]["netReturnExtremeTotal"].clone()
    })
    .collect();
    assert_ne!(digests[0], digests[1]);
    assert_ne!(digests[0], digests[2]);
    assert_ne!(digests[1], digests[2]);

    // S2 reads blocks of twice the length, so its block rule is stricter:
    // 36 bars admit L = 3 for S2 and up to L = 6 for the others.
    let mut wide = s2.clone();
    wide["blockLength"] = json!(4);
    assert_eq!(
        parse_error(&wide),
        "simulation.blockLength: twice 4 squared exceeds bars 36"
    );
    wide["statistic"] = json!("research-confirmation-candidate-s1-v1");
    assert!(parse_noise_simulation_v2(&wide).is_ok());
    wide["blockLength"] = json!(7);
    assert_eq!(
        parse_error(&wide),
        "simulation.blockLength: 7 squared exceeds bars 36"
    );
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
            "checkRule",
            "checkpoints",
            "confirmations",
            "contractVersion",
            "effectMillionths",
            "family",
            "limitMultiplierPpm",
            "simulations",
            "statistic",
            "status",
        ]
    );
    assert_eq!(report["statistic"], "research-confirmation-statistics-v1");
    assert_eq!(report["checkRule"], "point-screen");
    for case in fixture()["cases"].as_array().unwrap() {
        let report = report_json(&case["declaration"]);
        assert!(!report.to_string().contains("PASS"));
        assert!(["MEASURED", "WITHIN_LIMITS", "EXCEEDS_LIMITS"]
            .contains(&report["status"].as_str().unwrap()));
    }
}

/// Plan §8: every committed diagnostic report is re-checked by re-running its
/// declaration cut at its one declared checkpoint (`4096 / bars` simulations)
/// and comparing the counts. The full runs stay outside the suite.
#[test]
fn every_committed_diagnostic_report_reproduces_its_declared_prefix() {
    let diagnostics: Value = serde_json::from_str(DIAGNOSTICS).unwrap();
    let mut checked = Vec::new();
    for kind in ["size", "power"] {
        let reports = diagnostics["reports"][kind].as_object().unwrap();
        let declared: Vec<&Value> = diagnostics[kind].as_array().unwrap().iter().collect();
        for id in reports.keys() {
            assert!(
                declared.iter().any(|run| run["id"] == id.as_str()),
                "{id} is not a declared {kind} run"
            );
        }
        for run in declared {
            let id = run["id"].as_str().unwrap();
            if let Some(report) = reports.get(id) {
                checked.push((id, &run["declaration"], report));
            }
        }
    }
    // 36 size reports and the power runs the plan required.
    let required = diagnostics["requiredPowerRuns"].as_array().unwrap().len();
    assert_eq!(checked.len(), 36 + required);

    let check = |(id, raw, report): &(&str, &Value, &Value)| {
        let declaration = parse_noise_simulation_v2(raw).unwrap();
        // The diagnostic seed, never the acceptance one.
        assert_eq!(declaration.seed, 20_261_005, "{id}");
        assert_eq!(declaration.simulations, 4000, "{id}");
        assert_eq!(declaration.checkpoints, [4096 / declaration.bars], "{id}");
        let committed = &report["checkpoints"];
        assert_eq!(committed.as_array().unwrap().len(), 1, "{id}");
        assert_eq!(
            committed[0]["simulations"],
            json!(declaration.checkpoints[0])
        );
        let mut prefix = declaration.clone();
        prefix.simulations = declaration.checkpoints[0];
        prefix.checkpoints.clear();
        let cut = serde_json::to_value(simulate_noise_v2(&prefix).unwrap()).unwrap();
        assert_eq!(
            cut["family"]["count"], committed[0]["familyRejectingSimulations"],
            "{id}"
        );
        let counts: Vec<&Value> = cut["confirmations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| &row["counts"])
            .collect();
        assert_eq!(
            counts,
            committed[0]["confirmations"]
                .as_array()
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            "{id}"
        );
        // What the report echoes from its declaration.
        for field in [
            "contractVersion",
            "statistic",
            "allocationId",
            "effectMillionths",
            "checkRule",
            "limitMultiplierPpm",
        ] {
            assert_eq!(cut[field], report[field], "{id} {field}");
        }
        assert_eq!(report["simulations"], json!(4000), "{id}");
    };
    // About 4,096 bar-simulations per report; spread over the cores so the
    // debug-build total stays within the plan's budget.
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    let chunk = checked.len().div_ceil(workers);
    std::thread::scope(|scope| {
        for part in checked.chunks(chunk) {
            scope.spawn(move || part.iter().for_each(check));
        }
    });
}
