//! RUNNER-CONFIG-001 parity: the pure Rust config/enumeration/seed port is
//! checked against the same committed TypeScript-reference fixture the vitest
//! freshness test rebuilds.
//!
//! Every expected leaf compares EXACTLY (`expectedNumericPolicy: exact-v1`):
//! this slice produces identifiers, counters, indexes, and axis values that
//! both languages derive with identical IEEE-754 operations, so no tolerance
//! is admissible here. Error cases are held by the TypeScript reference; Rust
//! must reject the same input with a message containing the recorded fragment.

use serde_json::Value;

use super::config::{
    axis_values, discovery_axis_keys, parse_discovery_config, resolve_concurrency, AxisKey,
    DiscoveryAxis, DISCOVERY_HARD_CANDIDATE_CAP, DISCOVERY_MAX_AXIS_VALUES,
    DISCOVERY_SUPPORTED_SIGNAL_IDS,
};
use super::enumerate::{enumerate_candidates, DISCOVERY_VALIDITY_RULE_IDS};
use super::seed::{derive_discovery_seed, discovery_seed_preimage, DeriveSeedArgs};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/rs-core/runner-config-v1.json"
    ))
    .expect("runner-config fixture parses")
}

fn cases<'a>(fixture: &'a Value, group: &str) -> &'a Vec<Value> {
    fixture[group]
        .as_array()
        .unwrap_or_else(|| panic!("fixture group {group} must be an array"))
}

fn ids(fixture: &Value, group: &str) -> Vec<String> {
    cases(fixture, group)
        .iter()
        .map(|case| case["id"].as_str().expect("case id").to_string())
        .collect()
}

/// Numeric leaves compare by exact f64 value, so `5` and `5.0` agree while
/// `0.3` and `0.30000000000000004` do not.
///
/// The ONE numeric-leaf rule for `exact-v1`. Every comparison in this file
/// routes through it, so no call site can silently fall back to a bare `==`
/// that would accept a sign flip (IEEE-754 defines `-0.0 == 0.0`).
fn assert_exact_leaf(path: &str, actual: f64, expected: f64) {
    for (label, value) in [("actual", actual), ("expected", expected)] {
        assert!(value.is_finite(), "{path} {label} leaf must be finite");
        assert!(
            !(value == 0.0 && value.is_sign_negative()),
            "{path} {label} leaf is negative zero, which exact-v1 forbids"
        );
    }
    assert!(
        actual == expected,
        "{path} differs: actual={actual}, expected={expected}"
    );
}

/// Structural JSON equality whose numeric leaves obey `assert_exact_leaf`.
fn json_exact_eq(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(left), Value::Number(right)) => {
            let left = left.as_f64().expect("actual number is representable");
            let right = right.as_f64().expect("expected number is representable");
            assert_exact_leaf(path, left, right);
        }
        (Value::Array(left), Value::Array(right)) => {
            assert_eq!(left.len(), right.len(), "{path} length differs");
            for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
                json_exact_eq(left, right, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(left), Value::Object(right)) => {
            let mut left_keys: Vec<&String> = left.keys().collect();
            let mut right_keys: Vec<&String> = right.keys().collect();
            left_keys.sort();
            right_keys.sort();
            assert_eq!(left_keys, right_keys, "{path} key set differs");
            for key in left_keys {
                json_exact_eq(&left[key], &right[key], &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path} differs"),
    }
}

fn axis_from(value: &Value) -> DiscoveryAxis {
    let key = value["key"].as_str().expect("axis key");
    DiscoveryAxis {
        key: AxisKey::parse(key).expect("fixture axis key is whitelisted"),
        min: value["min"].as_f64().expect("axis min"),
        max: value["max"].as_f64().expect("axis max"),
        step: value["step"].as_f64().expect("axis step"),
    }
}

fn seed_args<'a>(input: &'a Value) -> DeriveSeedArgs<'a> {
    DeriveSeedArgs {
        root_seed: input["rootSeed"].as_f64().expect("rootSeed"),
        dataset_content_hash: input["datasetContentHash"].as_str().expect("dataset hash"),
        strategy_hash: input["strategyHash"].as_str().expect("strategy hash"),
        purpose: input["purpose"].as_str().expect("purpose"),
    }
}

#[test]
fn envelope_caps_and_whitelists_match_the_reference() {
    let fixture = fixture();
    assert_eq!(fixture["schemaVersion"], "rs-core-parity-fixture-v1");
    assert_eq!(fixture["fixtureVersion"], "runner-config-parity-v1");
    assert_eq!(fixture["expectedNumericPolicy"], "exact-v1");
    assert_eq!(fixture["contracts"]["config"], "discovery-config-v1");
    assert_eq!(fixture["contracts"]["preset"], "discovery-preset-v1");
    assert_eq!(
        fixture["contracts"]["enumeration"],
        "discovery-enumeration-v1"
    );
    assert_eq!(fixture["contracts"]["seed"], "seed-v1");

    assert_eq!(
        fixture["caps"]["hardCandidateCap"].as_i64().unwrap(),
        DISCOVERY_HARD_CANDIDATE_CAP
    );
    assert_eq!(
        fixture["caps"]["maxAxisValues"].as_u64().unwrap() as usize,
        DISCOVERY_MAX_AXIS_VALUES
    );

    let axis_keys: Vec<&str> = fixture["axisKeys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(axis_keys, discovery_axis_keys().to_vec());
    // The string whitelist is derived from the enum, so they cannot drift.
    assert_eq!(
        AxisKey::ALL.map(AxisKey::as_str).to_vec(),
        discovery_axis_keys().to_vec()
    );
    for excluded in ["feePct", "slipPct", "sizePct"] {
        assert!(
            !axis_keys.contains(&excluded),
            "{excluded} must not be an axis"
        );
    }

    let signal_ids: Vec<&str> = fixture["supportedSignalIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(signal_ids, DISCOVERY_SUPPORTED_SIGNAL_IDS.to_vec());

    let rule_ids: Vec<&str> = fixture["validityRuleIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(rule_ids, DISCOVERY_VALIDITY_RULE_IDS.to_vec());
}

#[test]
fn seed_preimages_and_derived_values_match_exactly() {
    let fixture = fixture();
    assert_eq!(
        ids(&fixture, "seedCases"),
        vec![
            "seed-root-zero",
            "seed-root-max-u32",
            "seed-root-mid",
            "seed-other-strategy",
            "seed-other-dataset",
        ]
    );

    for case in cases(&fixture, "seedCases") {
        let id = case["id"].as_str().unwrap();
        let args = seed_args(&case["input"]);
        let preimage = discovery_seed_preimage(&args)
            .unwrap_or_else(|error| panic!("{id} preimage failed: {error}"));
        assert_eq!(
            hex::encode(&preimage),
            case["expected"]["preimageHex"].as_str().unwrap(),
            "{id} preimage bytes differ"
        );
        assert_eq!(
            derive_discovery_seed(&args).unwrap(),
            case["expected"]["seed"].as_u64().unwrap() as u32,
            "{id} derived seed differs"
        );
    }

    assert_eq!(
        ids(&fixture, "seedErrorCases"),
        vec![
            "seed-negative-root",
            "seed-root-above-u32",
            "seed-fractional-root",
            "seed-legacy-dataset-hash",
            "seed-ephemeral-strategy-hash",
            "seed-empty-dataset-digest",
            "seed-truncated-strategy-digest",
            "seed-uppercase-strategy-digest",
            "seed-non-hex-strategy-digest",
            "seed-unknown-purpose",
        ]
    );
    for case in cases(&fixture, "seedErrorCases") {
        let id = case["id"].as_str().unwrap();
        let fragment = case["expectedErrorIncludes"].as_str().unwrap();
        let error = discovery_seed_preimage(&seed_args(&case["input"]))
            .expect_err(&format!("{id} must fail closed"));
        assert!(
            error.0.contains(fragment),
            "{id} error \"{}\" does not contain \"{fragment}\"",
            error.0
        );
    }
}

#[test]
fn axis_values_and_concurrency_match_exactly() {
    let fixture = fixture();
    for case in cases(&fixture, "axisCases") {
        let id = case["id"].as_str().unwrap();
        let values = axis_values(&axis_from(&case["input"]))
            .unwrap_or_else(|error| panic!("{id} failed: {error}"));
        let expected: Vec<f64> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap())
            .collect();
        assert_eq!(values.len(), expected.len(), "{id} value count differs");
        for (index, (actual, expected)) in values.iter().zip(expected.iter()).enumerate() {
            // Use the shared helper, not a bare `==`: axis values are exactly
            // where a -0 could appear (e.g. a `min: -0` boundary).
            assert_exact_leaf(&format!("{id}[{index}]"), *actual, *expected);
        }
    }

    for case in cases(&fixture, "axisErrorCases") {
        let id = case["id"].as_str().unwrap();
        let fragment = case["expectedErrorIncludes"].as_str().unwrap();
        let error =
            axis_values(&axis_from(&case["input"])).expect_err(&format!("{id} must fail closed"));
        assert!(
            error.0.contains(fragment),
            "{id} error \"{}\" does not contain \"{fragment}\"",
            error.0
        );
    }

    for case in cases(&fixture, "concurrencyCases") {
        let id = case["id"].as_str().unwrap();
        let requested = case["input"]["requested"].as_f64();
        let logical_cores = case["input"]["logicalCores"].as_f64().unwrap();
        assert_eq!(
            resolve_concurrency(requested, logical_cores).unwrap(),
            case["expected"].as_i64().unwrap(),
            "{id} resolved concurrency differs"
        );
    }

    for case in cases(&fixture, "concurrencyErrorCases") {
        let id = case["id"].as_str().unwrap();
        let fragment = case["expectedErrorIncludes"].as_str().unwrap();
        let error = resolve_concurrency(
            case["input"]["requested"].as_f64(),
            case["input"]["logicalCores"].as_f64().unwrap(),
        )
        .expect_err(&format!("{id} must fail closed"));
        assert!(
            error.0.contains(fragment),
            "{id} error \"{}\" does not contain \"{fragment}\"",
            error.0
        );
    }
}

#[test]
fn resolved_configs_match_the_reference_structure() {
    let fixture = fixture();
    assert_eq!(
        ids(&fixture, "configCases"),
        vec!["config-default-single-base", "config-multi-base-overrides"]
    );

    for case in cases(&fixture, "configCases") {
        let id = case["id"].as_str().unwrap();
        let logical_cores = case["logicalCores"].as_f64().unwrap();
        let resolved = parse_discovery_config(&case["input"], logical_cores)
            .unwrap_or_else(|error| panic!("{id} failed: {error}"));
        let actual = serde_json::to_value(&resolved).expect("resolved config serializes");
        json_exact_eq(&actual, &case["expected"], id);
    }
}

#[test]
fn every_typescript_held_config_rejection_is_reproduced() {
    let fixture = fixture();
    // The EXACT ordered inventory, not just its size: a deleted case must fail
    // here rather than be masked by a new case that keeps the count.
    assert_eq!(
        ids(&fixture, "configErrorCases"),
        vec![
            "config-unknown-envelope-key",
            "config-unknown-key-utf8-order",
            "config-missing-envelope-key",
            "config-envelope-version-mismatch",
            "config-contract-version-mismatch",
            "config-preset-version-mismatch",
            "config-dataset-legacy-hash",
            "config-dataset-empty-digest",
            "config-dataset-uppercase-digest",
            "config-dataset-id-zero",
            "config-blocks-mode-rejected",
            "config-code-mode-rejected",
            "config-unsupported-signal",
            "config-unknown-fill-mode",
            "config-strategy-unknown-key",
            "config-strategy-missing-key",
            "config-period-below-one",
            "config-period-fractional",
            "config-multiplier-not-positive",
            "config-size-out-of-range",
            "config-fee-percent-above-range",
            "config-slippage-percent-above-range",
            "config-stop-loss-percent-above-range",
            "config-take-profit-percent-negative",
            "config-axis-generates-percent-above-range",
            "config-level-out-of-range",
            "config-axis-key-not-whitelisted",
            "config-axis-step-not-positive",
            "config-axis-inverted-range",
            "config-axis-fractional-integer-bound",
            "config-axis-repeated-key",
            "config-axis-above-value-cap",
            "config-axis-generates-invalid-value",
            "config-empty-bases",
            "config-duplicate-base-id",
            "config-invalid-base-id",
            "config-benchmark-costs-mismatch",
            "config-benchmark-costs-percent-above-range",
            "config-benchmark-slippage-percent-negative",
            "config-random-entry-runs-above-cap",
            "config-negative-holding-allowance",
            "config-start-equity-zero",
            "config-candidate-cap-above-hard-cap",
            "config-root-seed-above-u32",
            "config-max-concurrency-string",
            "config-max-concurrency-above-cores",
            "config-gate-min-trades-invalid",
            "config-gate-fraction-invalid",
            "config-gate-percentile-invalid",
            "config-score-cap-invalid",
            "config-score-profit-factor-cap-invalid",
            "config-score-negative-weight",
            "config-score-regime-weight-deferred",
        ]
    );
    let error_cases = cases(&fixture, "configErrorCases");

    let mut mode_rejections = 0;
    for case in error_cases {
        let id = case["id"].as_str().unwrap();
        let fragment = case["expectedErrorIncludes"].as_str().unwrap();
        let logical_cores = case["logicalCores"].as_f64().unwrap();
        let error = parse_discovery_config(&case["input"], logical_cores)
            .expect_err(&format!("{id} must fail closed"));
        assert!(
            error.0.contains(fragment),
            "{id} error \"{}\" does not contain \"{fragment}\"",
            error.0
        );
        if fragment.contains("mode must be \"params\"") {
            mode_rejections += 1;
        }
    }
    // Blocks and code candidates are rejected at admission, not deeper in the
    // engine: the Rust pipeline has no non-params path at all.
    assert_eq!(mode_rejections, 2);

    // The full held-rejection total the docs quote.
    let held: usize = [
        "seedErrorCases",
        "axisErrorCases",
        "concurrencyErrorCases",
        "configErrorCases",
        "enumerationErrorCases",
    ]
    .iter()
    .map(|group| cases(&fixture, group).len())
    .sum();
    assert_eq!(held, 70, "held-error inventory total changed");
}

#[test]
fn candidate_plans_match_counts_order_indexes_and_seeds() {
    let fixture = fixture();
    assert_eq!(
        ids(&fixture, "enumerationCases"),
        vec![
            "enumerate-single-axis",
            "enumerate-multi-base-product",
            "enumerate-cross-field-prune",
            "enumerate-cross-base-duplicates",
            "enumerate-disjoint-bases",
            "enumerate-disjoint-bases-reversed",
        ]
    );

    for case in cases(&fixture, "enumerationCases") {
        let id = case["id"].as_str().unwrap();
        let logical_cores = case["logicalCores"].as_f64().unwrap();
        let config = parse_discovery_config(&case["input"], logical_cores)
            .unwrap_or_else(|error| panic!("{id} config failed: {error}"));
        let plan = enumerate_candidates(&config)
            .unwrap_or_else(|error| panic!("{id} enumeration failed: {error}"));
        let actual = serde_json::to_value(&plan).expect("plan serializes");
        json_exact_eq(&actual, &case["expected"], id);

        // Independent invariants, so a fixture regenerated from a broken
        // reference cannot quietly ship an inconsistent plan.
        let counts = &plan.counts;
        assert_eq!(
            counts.pruned_invalid + counts.duplicates + counts.final_unique,
            counts.raw,
            "{id} counters do not reconcile"
        );
        assert_eq!(plan.candidates.len() as i64, counts.final_unique);
        assert_eq!(plan.tested_combinations.n, counts.final_unique);
        assert_eq!(plan.tested_combinations.basis, "lineage-final-unique");
        let mut previous: Option<&str> = None;
        for (index, candidate) in plan.candidates.iter().enumerate() {
            assert_eq!(candidate.index, index as i64, "{id} index is not stable");
            assert!(candidate.strategy_hash.starts_with("strategy-v2:"));
            if let Some(previous) = previous {
                assert!(
                    previous < candidate.strategy_hash.as_str(),
                    "{id} candidates are not strictly hash-ordered"
                );
            }
            previous = Some(candidate.strategy_hash.as_str());
            assert_eq!(candidate.strategy["mode"], "params");
        }
    }

    for case in cases(&fixture, "enumerationErrorCases") {
        let id = case["id"].as_str().unwrap();
        let fragment = case["expectedErrorIncludes"].as_str().unwrap();
        let logical_cores = case["logicalCores"].as_f64().unwrap();
        let config = parse_discovery_config(&case["input"], logical_cores)
            .unwrap_or_else(|error| panic!("{id} config failed: {error}"));
        let error = enumerate_candidates(&config).expect_err(&format!("{id} must fail closed"));
        assert!(
            error.0.contains(fragment),
            "{id} error \"{}\" does not contain \"{fragment}\"",
            error.0
        );
    }
}

#[test]
fn base_declaration_order_never_changes_candidate_identity() {
    let fixture = fixture();
    let find = |id: &str| -> Value {
        cases(&fixture, "enumerationCases")
            .iter()
            .find(|case| case["id"] == id)
            .unwrap_or_else(|| panic!("missing case {id}"))
            .clone()
    };

    let forward = find("enumerate-disjoint-bases");
    let reversed = find("enumerate-disjoint-bases-reversed");
    let plan_for = |case: &Value| {
        let config = parse_discovery_config(&case["input"], case["logicalCores"].as_f64().unwrap())
            .expect("config parses");
        enumerate_candidates(&config).expect("enumeration succeeds")
    };

    let forward_plan = plan_for(&forward);
    let reversed_plan = plan_for(&reversed);
    assert_eq!(forward_plan.counts, reversed_plan.counts);
    assert_eq!(forward_plan.candidates, reversed_plan.candidates);
}

/// The `exact-v1` guard must actually FAIL on the values it claims to forbid.
/// Without these, an assertion that only ever sees clean data proves nothing.
#[test]
fn exact_leaf_helper_rejects_negative_zero_and_non_finite_values() {
    // Sanity: the helper accepts ordinary equal leaves, including +0.
    assert_exact_leaf("ok", 0.0, 0.0);
    assert_exact_leaf("ok", 0.30000000000000004, 0.30000000000000004);

    let rejected = [
        ("actual -0", -0.0_f64, 0.0_f64),
        ("expected -0", 0.0, -0.0),
        ("both -0", -0.0, -0.0),
        ("actual NaN", f64::NAN, 0.0),
        ("expected inf", 1.0, f64::INFINITY),
        ("actual -inf", f64::NEG_INFINITY, 1.0),
        // A genuine mismatch must still fail.
        ("drifted value", 0.3, 0.30000000000000004),
    ];
    // These panics are expected; keep the test output readable.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcomes: Vec<bool> = rejected
        .iter()
        .map(|(_, actual, expected)| {
            std::panic::catch_unwind(|| assert_exact_leaf("case", *actual, *expected)).is_err()
        })
        .collect();
    std::panic::set_hook(previous);

    for ((label, _, _), rejected) in rejected.iter().zip(outcomes) {
        assert!(rejected, "{label} must be rejected by exact-v1");
    }
}

#[test]
fn axis_keys_outside_the_whitelist_cannot_be_constructed() {
    // `AxisKey::parse` is the only way to build a key, so an axis naming a
    // non-whitelisted field is unrepresentable rather than merely rejected.
    for rejected in [
        "feePct", "slipPct", "sizePct", "mode", "", "FASTMA", "bogus",
    ] {
        assert!(
            AxisKey::parse(rejected).is_none(),
            "{rejected} must not parse as an axis key"
        );
    }
    for key in AxisKey::ALL {
        assert_eq!(AxisKey::parse(key.as_str()), Some(key));
    }
    assert_eq!(AxisKey::ALL.len(), discovery_axis_keys().len());
}
