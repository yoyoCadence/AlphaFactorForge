//! `research-precision-v1`: the P12a statistical precision precheck.
//!
//! Before a confirmation batch spends compute, this decides whether its
//! declared Monte Carlo/bootstrap budget can even resolve the Holm-corrected
//! threshold implied by the whole trial family. It answers the AlphaBTC
//! regression (`docs/research-precision-v1.md` §4): 73 family trials x 2 tests
//! with 1,000 samples can never adjust below `146/1001 ~= 0.145854`, so such a
//! plan is `NOT_ELIGIBLE` before it runs instead of being read as "no edge".
//!
//! All decisions use exact integer arithmetic (`u128`); alpha and the relative
//! standard-error limit are integer parts-per-million so no float rounding can
//! move a boundary. The plan is parsed by hand like `discovery-config-v1`: the
//! rejection ORDER is part of the contract.
//!
//! Pure: no Tauri, rusqlite, threads, events, UI, or IO. Not yet called by any
//! runtime path — the trial ledger (a later P12 sub-item) is the intended sole
//! producer of `priorTrials`.

use serde::Serialize;
use serde_json::{Map, Value};

pub const RESEARCH_PRECISION_VERSION: &str = "research-precision-v1";
pub const PRECISION_CORRECTION_HOLM: &str = "holm";

/// Every count and derived requirement stays representable by a JavaScript
/// number so a future TypeScript reader sees the same integers.
pub const PRECISION_MAX_COUNT: u64 = 9_007_199_254_740_991;
const PPM: u128 = 1_000_000;
const PPM_SQUARED: u128 = PPM * PPM;

const FIELDS: [&str; 9] = [
    "contractVersion",
    "correction",
    "alphaPpm",
    "maxRelativeStandardErrorPpm",
    "priorTrials",
    "plannedTrials",
    "testsPerTrial",
    "bootstrapSamples",
    "maxBootstrapSamples",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrecisionPlan {
    /// Family-wise alpha in parts per million, `[1, 999_999]`.
    pub alpha_ppm: u64,
    /// Largest accepted relative Monte Carlo standard error of the p-value
    /// estimate at the strictest Holm threshold, ppm, `[1, 999_999]`.
    pub max_relative_standard_error_ppm: u64,
    /// Effective trials already recorded in this family; never reset.
    pub prior_trials: u64,
    pub planned_trials: u64,
    pub tests_per_trial: u64,
    pub bootstrap_samples: u64,
    pub max_bootstrap_samples: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrecisionStatus {
    Eligible,
    NotEligible,
}

/// Reported in this fixed order; every failing check is listed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrecisionReason {
    BootstrapResolutionInsufficient,
    MonteCarloPrecisionInsufficient,
    BootstrapBudgetInsufficient,
}

/// An exact, unreduced rational `numerator / denominator`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Ratio {
    pub numerator: u64,
    pub denominator: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrecisionReport {
    pub contract_version: &'static str,
    pub status: PrecisionStatus,
    pub reasons: Vec<PrecisionReason>,
    pub family_trials: u64,
    pub family_tests: u64,
    /// `1 / (B + 1)`: the smallest p-value the `(1 + extreme) / (B + 1)`
    /// Monte Carlo estimator can return.
    pub min_attainable_raw_p: Ratio,
    /// Holm's smallest adjusted p-value, `min(m, B + 1) / (B + 1)`.
    pub best_adjusted_p: Ratio,
    pub best_adjusted_p_value: f64,
    /// Smallest `B` with `m / (B + 1) <= alpha`; `None` above
    /// [`PRECISION_MAX_COUNT`].
    pub required_samples_for_resolution: Option<u64>,
    /// Smallest `B` whose relative standard error at `p = alpha / m` is within
    /// the declared limit; `None` above [`PRECISION_MAX_COUNT`].
    pub required_samples_for_precision: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrecisionPlanError(pub String);

impl std::fmt::Display for PrecisionPlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PrecisionPlanError {}

fn fail<T>(message: String) -> Result<T, PrecisionPlanError> {
    Err(PrecisionPlanError(message))
}

fn read_count(
    object: &Map<String, Value>,
    field: &str,
    min: u64,
    max: u64,
) -> Result<u64, PrecisionPlanError> {
    let Some(value) = object.get(field) else {
        return fail(format!("precision.{field}: required"));
    };
    // `as_u64` is `None` for negatives and for any float literal (even `1.0`).
    match value.as_u64() {
        Some(count) if (min..=max).contains(&count) => Ok(count),
        _ => fail(format!(
            "precision.{field}: must be an integer in [{min}, {max}]"
        )),
    }
}

fn read_literal(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), PrecisionPlanError> {
    match object.get(field) {
        None => fail(format!("precision.{field}: required")),
        Some(Value::String(text)) if text == expected => Ok(()),
        Some(_) => fail(format!("precision.{field}: must be \"{expected}\"")),
    }
}

/// Strictly parses a `research-precision-v1` plan. Rejection order: not an
/// object, unknown fields (sorted), then [`FIELDS`] in order, then the
/// cross-field checks (samples within budget, family size in range).
pub fn parse_precision_plan(raw: &Value) -> Result<PrecisionPlan, PrecisionPlanError> {
    let Some(object) = raw.as_object() else {
        return fail("precision: must be an object".into());
    };
    let mut unknown: Vec<&str> = object
        .keys()
        .map(String::as_str)
        .filter(|key| !FIELDS.contains(key))
        .collect();
    unknown.sort_unstable();
    if let Some(first) = unknown.first() {
        return fail(format!("precision.{first}: unknown field"));
    }
    read_literal(object, "contractVersion", RESEARCH_PRECISION_VERSION)?;
    read_literal(object, "correction", PRECISION_CORRECTION_HOLM)?;
    let plan = PrecisionPlan {
        alpha_ppm: read_count(object, "alphaPpm", 1, 999_999)?,
        max_relative_standard_error_ppm: read_count(
            object,
            "maxRelativeStandardErrorPpm",
            1,
            999_999,
        )?,
        prior_trials: read_count(object, "priorTrials", 0, PRECISION_MAX_COUNT)?,
        planned_trials: read_count(object, "plannedTrials", 1, PRECISION_MAX_COUNT)?,
        tests_per_trial: read_count(object, "testsPerTrial", 1, PRECISION_MAX_COUNT)?,
        bootstrap_samples: read_count(object, "bootstrapSamples", 1, PRECISION_MAX_COUNT)?,
        max_bootstrap_samples: read_count(object, "maxBootstrapSamples", 1, PRECISION_MAX_COUNT)?,
    };
    if plan.bootstrap_samples > plan.max_bootstrap_samples {
        return fail("precision.bootstrapSamples: exceeds maxBootstrapSamples".into());
    }
    family_tests(&plan)?;
    Ok(plan)
}

/// `(prior + planned) * testsPerTrial`, bounded by [`PRECISION_MAX_COUNT`].
/// Checked arithmetic: a directly constructed plan with arbitrary `u64`
/// counts must return an error, never overflow (PR #115 review R1).
fn family_tests(plan: &PrecisionPlan) -> Result<(u64, u64), PrecisionPlanError> {
    let tests = u128::from(plan.prior_trials)
        .checked_add(u128::from(plan.planned_trials))
        .and_then(|trials| {
            trials
                .checked_mul(u128::from(plan.tests_per_trial))
                .map(|tests| (trials, tests))
        })
        .filter(|(_, tests)| *tests <= u128::from(PRECISION_MAX_COUNT));
    match tests {
        Some((trials, tests)) => Ok((trials as u64, tests as u64)),
        None => fail(format!(
            "precision: family tests (priorTrials + plannedTrials) * testsPerTrial must not exceed {PRECISION_MAX_COUNT}"
        )),
    }
}

fn ceil_div(numerator: u128, denominator: u128) -> u128 {
    numerator.div_ceil(denominator)
}

fn bounded(value: u128) -> Option<u64> {
    (value <= u128::from(PRECISION_MAX_COUNT)).then_some(value as u64)
}

/// Evaluates an already-parsed plan. Fails closed only on a plan that could
/// not have come from [`parse_precision_plan`]: every field is re-checked
/// against the same domain before any arithmetic.
pub fn evaluate_precision_plan(
    plan: &PrecisionPlan,
) -> Result<PrecisionReport, PrecisionPlanError> {
    let count = |value: u64, min: u64| (min..=PRECISION_MAX_COUNT).contains(&value);
    if !(1..=999_999).contains(&plan.alpha_ppm)
        || !(1..=999_999).contains(&plan.max_relative_standard_error_ppm)
        || !count(plan.prior_trials, 0)
        || !count(plan.planned_trials, 1)
        || !count(plan.tests_per_trial, 1)
        || !count(plan.bootstrap_samples, 1)
        || !count(plan.max_bootstrap_samples, 1)
        || plan.bootstrap_samples > plan.max_bootstrap_samples
    {
        return fail("precision: plan is outside the research-precision-v1 domain".into());
    }
    let (family_trials, family_tests) = family_tests(plan)?;
    let m = u128::from(family_tests);
    let a = u128::from(plan.alpha_ppm);
    let s = u128::from(plan.max_relative_standard_error_ppm);
    let samples = u128::from(plan.bootstrap_samples);

    // m / (B + 1) <= a / 1e6  <=>  B >= ceil(1e6 * m / a) - 1  (>= 1: a < 1e6).
    let resolution = ceil_div(PPM * m, a) - 1;
    // With p = a / (1e6 m) and r = s / 1e6, sqrt((1 - p) / (p B)) <= r
    // <=> B >= (1e6 m - a) * 1e12 / (a s^2).  a < 1e6 <= 1e6 m, so p < 1.
    let precision = ceil_div((PPM * m - a) * PPM_SQUARED, a * s * s);

    let mut reasons = Vec::new();
    if samples < resolution {
        reasons.push(PrecisionReason::BootstrapResolutionInsufficient);
    }
    if samples < precision {
        reasons.push(PrecisionReason::MonteCarloPrecisionInsufficient);
    }
    if resolution.max(precision) > u128::from(plan.max_bootstrap_samples) {
        reasons.push(PrecisionReason::BootstrapBudgetInsufficient);
    }

    let denominator = plan.bootstrap_samples + 1;
    let best_numerator = family_tests.min(denominator);
    Ok(PrecisionReport {
        contract_version: RESEARCH_PRECISION_VERSION,
        status: if reasons.is_empty() {
            PrecisionStatus::Eligible
        } else {
            PrecisionStatus::NotEligible
        },
        reasons,
        family_trials,
        family_tests,
        min_attainable_raw_p: Ratio {
            numerator: 1,
            denominator,
        },
        best_adjusted_p: Ratio {
            numerator: best_numerator,
            denominator,
        },
        // Both operands are exact integers below 2^53; one IEEE division.
        best_adjusted_p_value: best_numerator as f64 / denominator as f64,
        required_samples_for_resolution: bounded(resolution),
        required_samples_for_precision: bounded(precision),
    })
}

/// Parse then evaluate; the entry point for raw JSON plans.
pub fn precheck_precision(raw: &Value) -> Result<PrecisionReport, PrecisionPlanError> {
    evaluate_precision_plan(&parse_precision_plan(raw)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The AlphaBTC next-run counts (`docs/research-precision-v1.md` §4):
    /// 49 prior + 24 planned trials, two tests each, 1,000 samples, alpha 0.05.
    fn alphabtc() -> Value {
        json!({
            "contractVersion": "research-precision-v1",
            "correction": "holm",
            "alphaPpm": 50_000,
            "maxRelativeStandardErrorPpm": 200_000,
            "priorTrials": 49,
            "plannedTrials": 24,
            "testsPerTrial": 2,
            "bootstrapSamples": 1000,
            "maxBootstrapSamples": 100_000
        })
    }

    fn with(field: &str, value: Value) -> Value {
        let mut plan = alphabtc();
        plan[field] = value;
        plan
    }

    fn report(plan: &Value) -> PrecisionReport {
        precheck_precision(plan).unwrap()
    }

    fn error(plan: &Value) -> String {
        precheck_precision(plan).unwrap_err().0
    }

    #[test]
    fn alphabtc_counterexample_is_not_eligible_at_146_over_1001() {
        let report = report(&alphabtc());
        assert_eq!(report.status, PrecisionStatus::NotEligible);
        assert_eq!(
            report.reasons,
            vec![
                PrecisionReason::BootstrapResolutionInsufficient,
                PrecisionReason::MonteCarloPrecisionInsufficient,
            ]
        );
        assert_eq!(report.family_trials, 73);
        assert_eq!(report.family_tests, 146);
        assert_eq!(
            report.min_attainable_raw_p,
            Ratio {
                numerator: 1,
                denominator: 1001
            }
        );
        assert_eq!(
            report.best_adjusted_p,
            Ratio {
                numerator: 146,
                denominator: 1001
            }
        );
        assert_eq!(report.best_adjusted_p_value, 146.0 / 1001.0);
        assert_eq!(format!("{:.6}", report.best_adjusted_p_value), "0.145854");
        // AlphaBTC's own note: 2,919 samples only just reach 0.05.
        assert_eq!(report.required_samples_for_resolution, Some(2919));
        assert_eq!(report.required_samples_for_precision, Some(72_975));
    }

    #[test]
    fn resolution_boundary_is_exact_at_2919() {
        let below = report(&with("bootstrapSamples", json!(2918)));
        assert!(below
            .reasons
            .contains(&PrecisionReason::BootstrapResolutionInsufficient));
        assert!(below.best_adjusted_p_value > 0.05);

        let at = report(&with("bootstrapSamples", json!(2919)));
        assert!(!at
            .reasons
            .contains(&PrecisionReason::BootstrapResolutionInsufficient));
        assert_eq!(
            at.best_adjusted_p,
            Ratio {
                numerator: 146,
                denominator: 2920
            }
        );
        assert_eq!(at.best_adjusted_p_value, 0.05);
        // Reaching the threshold is not precision: still NOT_ELIGIBLE.
        assert_eq!(at.status, PrecisionStatus::NotEligible);
        assert_eq!(
            at.reasons,
            vec![PrecisionReason::MonteCarloPrecisionInsufficient]
        );
    }

    #[test]
    fn precision_boundary_is_exact_at_72975() {
        let below = report(&with("bootstrapSamples", json!(72_974)));
        assert_eq!(
            below.reasons,
            vec![PrecisionReason::MonteCarloPrecisionInsufficient]
        );
        let at = report(&with("bootstrapSamples", json!(72_975)));
        assert_eq!(at.status, PrecisionStatus::Eligible);
        assert!(at.reasons.is_empty());
    }

    #[test]
    fn budget_boundary_blocks_a_plan_the_cap_can_never_satisfy() {
        let mut plan = with("maxBootstrapSamples", json!(72_974));
        let below = report(&plan);
        assert_eq!(
            below.reasons,
            vec![
                PrecisionReason::BootstrapResolutionInsufficient,
                PrecisionReason::MonteCarloPrecisionInsufficient,
                PrecisionReason::BootstrapBudgetInsufficient,
            ]
        );
        plan["maxBootstrapSamples"] = json!(72_975);
        assert!(!report(&plan)
            .reasons
            .contains(&PrecisionReason::BootstrapBudgetInsufficient));
    }

    #[test]
    fn growing_the_family_can_only_raise_requirements() {
        // A trial count cannot be "reset" by this contract: more prior trials
        // never lowers a requirement or improves the best adjusted p-value.
        let mut previous = report(&with("priorTrials", json!(0)));
        for prior in [1_u64, 49, 50, 1_000, 1_000_000] {
            let next = report(&with("priorTrials", json!(prior)));
            assert!(
                next.required_samples_for_resolution >= previous.required_samples_for_resolution
            );
            assert!(next.required_samples_for_precision >= previous.required_samples_for_precision);
            assert!(next.best_adjusted_p_value >= previous.best_adjusted_p_value);
            previous = next;
        }
    }

    #[test]
    fn best_adjusted_p_caps_at_one() {
        let plan = with("priorTrials", json!(10_000));
        let report = report(&plan);
        assert_eq!(report.family_tests, 20_048);
        assert_eq!(
            report.best_adjusted_p,
            Ratio {
                numerator: 1001,
                denominator: 1001
            }
        );
        assert_eq!(report.best_adjusted_p_value, 1.0);
    }

    #[test]
    fn huge_requirements_saturate_to_none_and_stay_budget_blocked() {
        let mut plan = with("alphaPpm", json!(1));
        plan["maxRelativeStandardErrorPpm"] = json!(1);
        plan["priorTrials"] = json!(1_000_000_000);
        let report = report(&plan);
        // m = (1e9 + 24) * 2; 1e6 * m - 1 still fits; the precision term does not.
        assert_eq!(
            report.required_samples_for_resolution,
            Some(2_000_000_047_999_999)
        );
        assert_eq!(report.required_samples_for_precision, None);
        assert!(report
            .reasons
            .contains(&PrecisionReason::BootstrapBudgetInsufficient));
    }

    #[test]
    fn smallest_family_reaches_resolution_with_one_sample() {
        let plan = json!({
            "contractVersion": "research-precision-v1",
            "correction": "holm",
            "alphaPpm": 999_999,
            "maxRelativeStandardErrorPpm": 999_999,
            "priorTrials": 0,
            "plannedTrials": 1,
            "testsPerTrial": 1,
            "bootstrapSamples": 1,
            "maxBootstrapSamples": 1
        });
        let report = report(&plan);
        assert_eq!(report.required_samples_for_resolution, Some(1));
        assert_eq!(report.required_samples_for_precision, Some(1));
        assert_eq!(report.status, PrecisionStatus::Eligible);
    }

    #[test]
    fn integer_domains_reject_both_sides_of_each_bound() {
        for (field, below, min, max, above) in [
            ("alphaPpm", json!(0), 1_u64, 999_999_u64, json!(1_000_000)),
            (
                "maxRelativeStandardErrorPpm",
                json!(0),
                1,
                999_999,
                json!(1_000_000),
            ),
            (
                "plannedTrials",
                json!(0),
                1,
                PRECISION_MAX_COUNT,
                json!(PRECISION_MAX_COUNT + 1),
            ),
            (
                "testsPerTrial",
                json!(0),
                1,
                PRECISION_MAX_COUNT,
                json!(PRECISION_MAX_COUNT + 1),
            ),
        ] {
            let message = format!("precision.{field}: must be an integer in [{min}, {max}]");
            assert_eq!(error(&with(field, below)), message);
            assert_eq!(error(&with(field, above)), message);
        }
        for field in ["priorTrials", "bootstrapSamples", "maxBootstrapSamples"] {
            let min = if field == "priorTrials" { 0 } else { 1 };
            let message =
                format!("precision.{field}: must be an integer in [{min}, {PRECISION_MAX_COUNT}]");
            assert_eq!(error(&with(field, json!(-1))), message);
            assert_eq!(error(&with(field, json!(PRECISION_MAX_COUNT + 1))), message);
        }
        assert!(precheck_precision(&with("priorTrials", json!(0))).is_ok());
        assert_eq!(
            error(&with("bootstrapSamples", json!(1000.0))),
            "precision.bootstrapSamples: must be an integer in [1, 9007199254740991]"
        );
        assert_eq!(
            error(&with("alphaPpm", json!("50000"))),
            "precision.alphaPpm: must be an integer in [1, 999999]"
        );
    }

    #[test]
    fn rejection_order_is_fixed() {
        assert_eq!(error(&json!([])), "precision: must be an object");
        let mut plan = alphabtc();
        plan["zeta"] = json!(1);
        plan["alpha"] = json!(0.05);
        plan["contractVersion"] = json!("research-precision-v2");
        assert_eq!(error(&plan), "precision.alpha: unknown field");

        let mut plan = alphabtc();
        plan.as_object_mut().unwrap().remove("contractVersion");
        plan["correction"] = json!("bonferroni");
        assert_eq!(error(&plan), "precision.contractVersion: required");
        assert_eq!(
            error(&with("correction", json!("bonferroni"))),
            "precision.correction: must be \"holm\""
        );

        let mut plan = with("alphaPpm", json!(0));
        plan["bootstrapSamples"] = json!(0);
        assert_eq!(
            error(&plan),
            "precision.alphaPpm: must be an integer in [1, 999999]"
        );
    }

    #[test]
    fn cross_field_checks_fail_closed() {
        assert_eq!(
            error(&with("bootstrapSamples", json!(100_001))),
            "precision.bootstrapSamples: exceeds maxBootstrapSamples"
        );
        let mut plan = with("testsPerTrial", json!(PRECISION_MAX_COUNT));
        plan["priorTrials"] = json!(1);
        assert!(error(&plan).starts_with("precision: family tests"));
        let direct = PrecisionPlan {
            alpha_ppm: 0,
            max_relative_standard_error_ppm: 1,
            prior_trials: 0,
            planned_trials: 1,
            tests_per_trial: 1,
            bootstrap_samples: 1,
            max_bootstrap_samples: 1,
        };
        assert!(evaluate_precision_plan(&direct).is_err());
    }

    /// PR #115 review R1: the public typed API must reject out-of-domain
    /// counts with `Err` before any arithmetic, never panic on overflow.
    #[test]
    fn direct_api_rejects_out_of_domain_counts_without_panicking() {
        let legal = PrecisionPlan {
            alpha_ppm: 50_000,
            max_relative_standard_error_ppm: 200_000,
            prior_trials: 49,
            planned_trials: 24,
            tests_per_trial: 2,
            bootstrap_samples: 72_975,
            max_bootstrap_samples: 100_000,
        };
        let evaluate = |plan: PrecisionPlan| {
            std::panic::catch_unwind(|| evaluate_precision_plan(&plan))
                .expect("evaluate_precision_plan must not panic")
        };
        assert!(evaluate(legal).is_ok());

        // The reviewer's reproduction: three u64::MAX counts.
        let reproduction = PrecisionPlan {
            prior_trials: u64::MAX,
            planned_trials: u64::MAX,
            tests_per_trial: u64::MAX,
            ..legal
        };
        assert!(evaluate(reproduction).is_err());

        type Setter = fn(&mut PrecisionPlan, u64);
        let fields: [(&str, Setter); 5] = [
            ("priorTrials", |plan, value| plan.prior_trials = value),
            ("plannedTrials", |plan, value| plan.planned_trials = value),
            ("testsPerTrial", |plan, value| plan.tests_per_trial = value),
            ("bootstrapSamples", |plan, value| {
                plan.bootstrap_samples = value;
                plan.max_bootstrap_samples = value;
            }),
            ("maxBootstrapSamples", |plan, value| {
                plan.max_bootstrap_samples = value
            }),
        ];
        for (name, set) in fields {
            for value in [PRECISION_MAX_COUNT + 1, u64::MAX] {
                let mut plan = legal;
                set(&mut plan, value);
                assert!(evaluate(plan).is_err(), "{name} = {value}");
            }
        }

        // Legal maxima stay accepted where the family still fits.
        let mut plan = legal;
        plan.bootstrap_samples = PRECISION_MAX_COUNT;
        plan.max_bootstrap_samples = PRECISION_MAX_COUNT;
        let report = evaluate(plan).unwrap();
        assert_eq!(report.status, PrecisionStatus::Eligible);
        assert_eq!(report.best_adjusted_p.denominator, PRECISION_MAX_COUNT + 1);

        let mut plan = legal;
        plan.prior_trials = PRECISION_MAX_COUNT - 1;
        plan.planned_trials = 1;
        plan.tests_per_trial = 1;
        let report = evaluate(plan).unwrap();
        assert_eq!(report.family_tests, PRECISION_MAX_COUNT);
        assert_eq!(report.status, PrecisionStatus::NotEligible);
        plan.prior_trials = PRECISION_MAX_COUNT;
        assert!(evaluate(plan)
            .unwrap_err()
            .0
            .starts_with("precision: family tests"));

        let mut plan = legal;
        plan.tests_per_trial = PRECISION_MAX_COUNT;
        plan.prior_trials = 0;
        plan.planned_trials = 1;
        assert_eq!(evaluate(plan).unwrap().family_tests, PRECISION_MAX_COUNT);

        // The checked family product itself also fails closed on raw u64s.
        assert!(family_tests(&reproduction).is_err());
    }

    #[test]
    fn authored_fixture_locks_the_contract() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../fixtures/rs-core/research-precision-v1.json"
        ))
        .unwrap();
        assert_eq!(fixture["contractVersion"], RESEARCH_PRECISION_VERSION);
        let cases = fixture["cases"].as_array().unwrap();
        assert!(!cases.is_empty());
        for case in cases {
            let id = &case["id"];
            match (&case["expected"], &case["error"]) {
                (expected, Value::Null) => {
                    let actual = serde_json::to_value(report(&case["plan"])).unwrap();
                    assert_eq!(&actual, expected, "{id}");
                }
                (Value::Null, Value::String(message)) => {
                    assert_eq!(&error(&case["plan"]), message, "{id}");
                }
                _ => panic!("{id}: exactly one of expected/error"),
            }
        }
    }
}
