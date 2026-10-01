use serde_json::{json, Value};

use super::*;
use crate::discovery_core::confirmation::{
    evaluate_confirmation, parse_confirmation_declaration, CandidateSeries, ConfirmationDeclaration,
};
use crate::discovery_core::precision::{evaluate_precision_plan, PrecisionPlan};

const FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-alpha-allocation-v1.json");
const CONFIRMATION_FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-confirmation-statistics-v1.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap()
}

fn declaration_json() -> Value {
    fixture()["cases"][0]["declaration"].clone()
}

/// 50,000 ppm over four confirmations: 20,000 / 15,000 / 10,000 / 5,000.
fn declaration() -> AlphaAllocationDeclaration {
    parse_alpha_allocation(&declaration_json()).unwrap()
}

fn parse_error(raw: &Value) -> String {
    parse_alpha_allocation(raw).unwrap_err().0
}

fn with(field: &str, value: Value) -> Value {
    let mut raw = declaration_json();
    raw[field] = value;
    raw
}

fn counts(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|number| number.as_u64().unwrap())
        .collect()
}

fn budget(total_alpha_ppm: u64, schedule: &[u64]) -> AlphaAllocationDeclaration {
    AlphaAllocationDeclaration {
        total_alpha_ppm,
        schedule: schedule.to_vec(),
    }
}

#[test]
fn fixture_cases_reproduce_the_independent_reference() {
    let fixture = fixture();
    assert_eq!(fixture["contractVersion"], ALPHA_ALLOCATION_VERSION);
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 10);
    for case in cases {
        let id = &case["id"];
        let actual = parse_alpha_allocation(&case["declaration"]).and_then(|declaration| {
            allocate_confirmation_alpha(&declaration, &counts(&case["reserved"]))
        });
        match (&case["expected"], &case["error"]) {
            (expected, Value::Null) => {
                assert_eq!(
                    &serde_json::to_value(actual.unwrap()).unwrap(),
                    expected,
                    "{id}"
                );
            }
            (Value::Null, Value::String(message)) => {
                assert_eq!(&actual.unwrap_err().0, message, "{id}");
            }
            _ => panic!("{id}: exactly one of expected/error"),
        }
    }

    let splits = fixture["equalSchedules"].as_array().unwrap();
    assert_eq!(splits.len(), 5);
    for split in splits {
        let id = &split["id"];
        let actual = equal_alpha_schedule(
            split["totalAlphaPpm"].as_u64().unwrap(),
            split["confirmations"].as_u64().unwrap(),
        );
        match (&split["schedule"], &split["error"]) {
            (schedule, Value::Null) => assert_eq!(actual.unwrap(), counts(schedule), "{id}"),
            (Value::Null, Value::String(message)) => {
                assert_eq!(&actual.unwrap_err().0, message, "{id}");
            }
            _ => panic!("{id}: exactly one of schedule/error"),
        }
    }
}

#[test]
fn the_declaration_is_parsed_strictly_in_a_fixed_rejection_order() {
    assert_eq!(
        declaration(),
        budget(50_000, &[20_000, 15_000, 10_000, 5_000])
    );
    // The parsed declaration's document is the declaration that was parsed.
    assert_eq!(declaration().document(), declaration_json());

    assert_eq!(parse_error(&json!([])), "allocation: must be an object");
    assert_eq!(parse_error(&json!(null)), "allocation: must be an object");

    // Unknown fields come first, in sorted order, even when others are wrong.
    let mut raw = with("rule", json!("spend-it-all"));
    raw["zeta"] = json!(1);
    raw["carryOver"] = json!(true);
    assert_eq!(parse_error(&raw), "allocation.carryOver: unknown field");

    // Then the fields in declared order.
    let mut raw = json!({});
    for (field, value, missing) in [
        (
            "contractVersion",
            json!(ALPHA_ALLOCATION_VERSION),
            "allocation.contractVersion: required",
        ),
        (
            "rule",
            json!(ALPHA_ALLOCATION_RULE),
            "allocation.rule: required",
        ),
        (
            "scope",
            json!(ALPHA_ALLOCATION_SCOPE),
            "allocation.scope: required",
        ),
        (
            "totalAlphaPpm",
            json!(50_000),
            "allocation.totalAlphaPpm: required",
        ),
        ("schedule", json!([50_000]), "allocation.schedule: required"),
    ] {
        assert_eq!(parse_error(&raw), missing);
        raw[field] = value;
    }
    assert_eq!(
        parse_alpha_allocation(&raw).unwrap(),
        budget(50_000, &[50_000])
    );

    assert_eq!(
        parse_error(&with(
            "contractVersion",
            json!("research-alpha-allocation-v2")
        )),
        "allocation.contractVersion: must be \"research-alpha-allocation-v1\""
    );
    assert_eq!(
        parse_error(&with("rule", json!("equal-split"))),
        "allocation.rule: must be \"declared-schedule\""
    );
    // A per-campaign or per-workspace budget would be a reset.
    for scope in [json!("campaign"), json!("workspace"), json!(null)] {
        assert_eq!(
            parse_error(&with("scope", scope)),
            "allocation.scope: must be \"trial-family\""
        );
    }

    // An earlier field wins over a later one.
    let mut raw = with("totalAlphaPpm", json!(0));
    raw["schedule"] = json!([]);
    assert_eq!(
        parse_error(&raw),
        "allocation.totalAlphaPpm: must be an integer in [1, 999999]"
    );
    // Entries are checked in array order, and before the sum.
    assert_eq!(
        parse_error(&with("schedule", json!([40_000, "x", 0, 40_000]))),
        "allocation.schedule[1]: must be an integer in [1, 999999]"
    );
    assert_eq!(
        parse_error(&with("schedule", json!([40_000, 0, "x", 40_000]))),
        "allocation.schedule[1]: must be an integer in [1, 999999]"
    );
}

#[test]
fn alpha_is_integer_ppm_with_exact_boundaries() {
    let total_error = "allocation.totalAlphaPpm: must be an integer in [1, 999999]";
    for bad in [
        json!(0),
        json!(1_000_000),
        json!(-1),
        json!(50_000.0),
        json!(0.05),
        json!("50000"),
        json!(null),
    ] {
        let mut raw = with("totalAlphaPpm", bad);
        raw["schedule"] = json!([1]);
        assert_eq!(parse_error(&raw), total_error);
    }
    for (total, share) in [(1u64, 1u64), (999_999, 999_999)] {
        let mut raw = with("totalAlphaPpm", json!(total));
        raw["schedule"] = json!([share]);
        assert_eq!(
            parse_alpha_allocation(&raw).unwrap(),
            budget(total, &[share])
        );
    }

    for bad in [json!(null), json!({}), json!([]), json!(50_000), json!("x")] {
        assert_eq!(
            parse_error(&with("schedule", bad)),
            "allocation.schedule: must be a non-empty array"
        );
    }
    let share_error = "allocation.schedule[0]: must be an integer in [1, 999999]";
    for bad in [
        json!(0),
        json!(1_000_000),
        json!(-1),
        json!(16_666.5),
        json!(20_000.0),
        json!("20000"),
        json!(null),
        json!([20_000]),
    ] {
        assert_eq!(parse_error(&with("schedule", json!([bad]))), share_error);
    }

    // The budget bound is exact: equal to the total is allowed, one ppm over
    // is refused, and nothing is rounded.
    assert!(parse_alpha_allocation(&with("schedule", json!([25_000, 25_000]))).is_ok());
    assert!(parse_alpha_allocation(&with("schedule", json!([25_000, 24_999]))).is_ok());
    assert_eq!(
        parse_error(&with("schedule", json!([25_000, 25_001]))),
        "allocation.schedule: allocates 50001 ppm, above totalAlphaPpm 50000"
    );
    // Even a schedule of maximal entries cannot overflow the sum.
    let mut raw = with("totalAlphaPpm", json!(999_999));
    raw["schedule"] = json!(vec![999_999u64; 3]);
    assert_eq!(
        parse_error(&raw),
        "allocation.schedule: allocates 2999997 ppm, above totalAlphaPpm 999999"
    );
}

#[test]
fn a_directly_constructed_declaration_cannot_bypass_the_domain() {
    for (declaration, message) in [
        (
            budget(0, &[1]),
            "allocation.totalAlphaPpm: must be an integer in [1, 999999]",
        ),
        (
            budget(1_000_000, &[1]),
            "allocation.totalAlphaPpm: must be an integer in [1, 999999]",
        ),
        (
            budget(50_000, &[]),
            "allocation.schedule: must be a non-empty array",
        ),
        (
            budget(50_000, &[10_000, 0]),
            "allocation.schedule[1]: must be an integer in [1, 999999]",
        ),
        (
            budget(50_000, &[u64::MAX, u64::MAX]),
            "allocation.schedule[0]: must be an integer in [1, 999999]",
        ),
        (
            budget(50_000, &[50_000, 1]),
            "allocation.schedule: allocates 50001 ppm, above totalAlphaPpm 50000",
        ),
    ] {
        assert_eq!(
            allocate_confirmation_alpha(&declaration, &[])
                .unwrap_err()
                .0,
            message
        );
        assert_eq!(alpha_allocation_id(&declaration).unwrap_err().0, message);
    }
}

#[test]
fn a_family_spends_its_schedule_once_and_never_more_than_the_total() {
    for declaration in [
        declaration(),
        budget(50_000, &[16_666, 16_666, 16_666]),
        budget(50_000, &[50_000]),
        budget(999_999, &[1, 999_998]),
        budget(10, &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1]),
        budget(10, &[3]),
    ] {
        let scheduled: u64 = declaration.schedule.iter().sum();
        let mut reserved: Vec<u64> = Vec::new();
        loop {
            let report = allocate_confirmation_alpha(&declaration, &reserved).unwrap();
            let spent: u64 = reserved.iter().sum();
            assert_eq!(report.reserved_confirmations, reserved.len() as u64);
            assert_eq!(report.spent_alpha_ppm, spent);
            assert_eq!(
                report.unscheduled_alpha_ppm,
                declaration.total_alpha_ppm - scheduled
            );
            // Every ppm of the total is accounted for exactly once.
            assert_eq!(
                spent
                    + report.alpha_ppm.unwrap_or(0)
                    + report.remaining_scheduled_alpha_ppm
                    + report.unscheduled_alpha_ppm,
                declaration.total_alpha_ppm
            );
            let Some(alpha) = report.alpha_ppm else {
                assert_eq!(report.status, AlphaAllocationStatus::NotEligible);
                assert_eq!(
                    report.reasons,
                    vec![AlphaAllocationReason::AlphaBudgetExhausted]
                );
                assert_eq!(report.confirmation_number, None);
                assert_eq!(report.remaining_confirmations, 0);
                assert_eq!(report.remaining_scheduled_alpha_ppm, 0);
                break;
            };
            assert_eq!(report.status, AlphaAllocationStatus::Eligible);
            assert!(report.reasons.is_empty());
            assert_eq!(report.confirmation_number, Some(reserved.len() as u64 + 1));
            assert_eq!(alpha, declaration.schedule[reserved.len()]);
            assert_eq!(
                report.remaining_confirmations,
                (declaration.schedule.len() - reserved.len() - 1) as u64
            );
            reserved.push(alpha);
        }
        // The family got exactly its schedule, and that is within the total.
        assert_eq!(reserved, declaration.schedule);
        assert!(reserved.iter().sum::<u64>() <= declaration.total_alpha_ppm);
        // Asking again changes nothing: exhausted stays exhausted.
        assert_eq!(
            allocate_confirmation_alpha(&declaration, &reserved).unwrap(),
            allocate_confirmation_alpha(&declaration, &reserved).unwrap()
        );
    }
}

#[test]
fn a_history_that_contradicts_the_schedule_is_refused() {
    let declaration = declaration();
    let refused = |reserved: &[u64]| {
        allocate_confirmation_alpha(&declaration, reserved)
            .unwrap_err()
            .0
    };
    // A reservation made under some other alpha than the declared one.
    assert_eq!(
        refused(&[15_000]),
        "allocation: confirmation 1 reserved 15000 ppm but the schedule declares 20000"
    );
    // Reordered: the same amounts in another order are a different history.
    assert_eq!(
        refused(&[20_000, 10_000, 15_000]),
        "allocation: confirmation 2 reserved 10000 ppm but the schedule declares 15000"
    );
    // A smaller spend does not earn the difference back later.
    assert_eq!(
        refused(&[20_000, 15_000, 1]),
        "allocation: confirmation 3 reserved 1 ppm but the schedule declares 10000"
    );
    assert_eq!(
        refused(&[20_000, 15_000, 10_000, 5_000, 1]),
        "allocation: 5 confirmations are reserved but the schedule declares 4"
    );
    // Length is checked before content.
    assert_eq!(
        refused(&[1, 1, 1, 1, 1]),
        "allocation: 5 confirmations are reserved but the schedule declares 4"
    );

    // What the pure function cannot detect: a caller that hides history gets
    // confirmation 1 again. P13 must pass the registry's full list.
    let reset = allocate_confirmation_alpha(&declaration, &[]).unwrap();
    assert_eq!(reset.confirmation_number, Some(1));
    assert_eq!(reset.alpha_ppm, Some(20_000));
}

#[test]
fn an_equal_split_floors_and_never_rounds_up() {
    assert_eq!(equal_alpha_schedule(50_000, 3).unwrap(), vec![16_666; 3]);
    assert_eq!(equal_alpha_schedule(50_000, 1).unwrap(), vec![50_000]);
    assert_eq!(equal_alpha_schedule(999_999, 2).unwrap(), vec![499_999; 2]);
    // The smallest share is 1 ppm; one confirmation more is refused instead
    // of producing a zero share or borrowing from the others.
    assert_eq!(equal_alpha_schedule(7, 7).unwrap(), vec![1; 7]);
    assert_eq!(
        equal_alpha_schedule(7, 8).unwrap_err().0,
        "allocation: confirmations must be in [1, 7] so every share is at least 1 ppm"
    );
    assert_eq!(
        equal_alpha_schedule(50_000, 0).unwrap_err().0,
        "allocation: confirmations must be in [1, 50000] so every share is at least 1 ppm"
    );
    assert!(equal_alpha_schedule(50_000, u64::MAX).is_err());
    for total in [0, 1_000_000, u64::MAX] {
        assert_eq!(
            equal_alpha_schedule(total, 1).unwrap_err().0,
            "allocation.totalAlphaPpm: must be an integer in [1, 999999]"
        );
    }

    for total in 1..=60u64 {
        for confirmations in 1..=total {
            let schedule = equal_alpha_schedule(total, confirmations).unwrap();
            let share = schedule[0];
            assert_eq!(schedule.len() as u64, confirmations);
            assert!(schedule.iter().all(|&entry| entry == share));
            // The largest equal share that fits: one ppm more would overspend.
            assert!(share >= 1 && share * confirmations <= total);
            assert!((share + 1) * confirmations > total);
            // It is always a valid schedule for that total.
            let report = allocate_confirmation_alpha(&budget(total, &schedule), &[]).unwrap();
            assert_eq!(report.alpha_ppm, Some(share));
            assert_eq!(report.unscheduled_alpha_ppm, total % confirmations);
        }
        assert!(equal_alpha_schedule(total, total + 1).is_err());
    }
}

#[test]
fn the_identity_covers_every_declared_number_and_the_schedule_order() {
    let id = alpha_allocation_id(&declaration()).unwrap();
    assert_eq!(id.len(), 64);
    assert_eq!(id, fixture()["cases"][0]["expected"]["allocationId"]);
    // Derived by the TypeScript reference with its own canonical encoder.
    assert_eq!(
        id,
        "104705944f7da4b3bceb04e805f4fefa35db1930c332d820ec58fad94d1b8e3a"
    );
    assert_eq!(id, alpha_allocation_id(&declaration()).unwrap());
    // The reserved history is not part of the declaration's identity.
    for reserved in [vec![], vec![20_000], vec![20_000, 15_000, 10_000, 5_000]] {
        assert_eq!(
            allocate_confirmation_alpha(&declaration(), &reserved)
                .unwrap()
                .allocation_id,
            id
        );
    }
    for changed in [
        budget(50_001, &[20_000, 15_000, 10_000, 5_000]),
        budget(50_000, &[20_000, 15_000, 10_000, 4_999]),
        budget(50_000, &[20_000, 15_000, 10_000]),
        budget(50_000, &[5_000, 10_000, 15_000, 20_000]),
        budget(50_000, &[15_000, 20_000, 10_000, 5_000]),
    ] {
        assert_ne!(alpha_allocation_id(&changed).unwrap(), id);
    }
}

#[test]
fn the_report_is_an_allocation_and_never_a_pass() {
    let open =
        serde_json::to_value(allocate_confirmation_alpha(&declaration(), &[]).unwrap()).unwrap();
    let exhausted = serde_json::to_value(
        allocate_confirmation_alpha(&declaration(), &declaration().schedule).unwrap(),
    )
    .unwrap();
    assert_eq!(open["status"], "ELIGIBLE");
    assert_eq!(exhausted["status"], "NOT_ELIGIBLE");
    assert_eq!(exhausted["reasons"], json!(["alpha_budget_exhausted"]));
    assert_eq!(exhausted["alphaPpm"], Value::Null);
    assert_eq!(exhausted["confirmationNumber"], Value::Null);
    for report in [&open, &exhausted] {
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
                "alphaPpm",
                "confirmationNumber",
                "contractVersion",
                "reasons",
                "remainingConfirmations",
                "remainingScheduledAlphaPpm",
                "reservedConfirmations",
                "rule",
                "scheduledConfirmations",
                "scope",
                "spentAlphaPpm",
                "status",
                "totalAlphaPpm",
                "unscheduledAlphaPpm",
            ]
        );
        assert!(!report.to_string().contains("PASS"));
        assert_eq!(report["scope"], "trial-family");
    }
}

/// The steady positive series of the confirmation fixture: both tests have
/// `extremeCount = 0` whatever the seed, so only alpha decides the outcome.
fn steady_confirmation(alpha_ppm: u64) -> Vec<bool> {
    let fixture: Value = serde_json::from_str(CONFIRMATION_FIXTURE).unwrap();
    let case = &fixture["cases"][2];
    assert_eq!(case["id"], "steady-positive-returns");
    let floats = |value: &Value| -> Vec<f64> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|number| number.as_f64().unwrap())
            .collect()
    };
    let returns = floats(&case["candidates"][0]["returns"]);
    let benchmark = floats(&case["candidates"][0]["benchmarkReturns"]);
    let declaration = ConfirmationDeclaration {
        alpha_ppm,
        bootstrap_samples: 199,
        ..parse_confirmation_declaration(&case["declaration"]).unwrap()
    };
    evaluate_confirmation(
        &declaration,
        10,
        &[CandidateSeries {
            candidate_index: 5,
            returns: &returns,
            benchmark_returns: &benchmark,
        }],
    )
    .unwrap()
    .tests
    .iter()
    .map(|test| test.rejects_null)
    .collect()
}

#[test]
fn the_allocated_alpha_is_what_a_confirmation_is_judged_against() {
    // Ten family tests and 199 samples: the best adjusted p is 10/200 = 0.05.
    let two_shares = budget(99_999, &[50_000, 49_999]);
    let first = allocate_confirmation_alpha(&two_shares, &[]).unwrap();
    let second = allocate_confirmation_alpha(&two_shares, &[50_000]).unwrap();
    assert_eq!(first.alpha_ppm, Some(50_000));
    assert_eq!(second.alpha_ppm, Some(49_999));
    // The same data rejects under the first share and not under the second:
    // a later confirmation is judged by its own share, not the first one's.
    assert_eq!(steady_confirmation(first.alpha_ppm.unwrap()), [true, true]);
    assert_eq!(
        steady_confirmation(second.alpha_ppm.unwrap()),
        [false, false]
    );

    // A smaller share also needs more samples to be resolvable at all (P12a),
    // so the precheck has to be run with the allocated alpha.
    let required = |alpha_ppm: u64| {
        evaluate_precision_plan(&PrecisionPlan {
            alpha_ppm,
            max_relative_standard_error_ppm: 200_000,
            prior_trials: 0,
            planned_trials: 5,
            tests_per_trial: 2,
            bootstrap_samples: 199,
            max_bootstrap_samples: 1_000_000,
        })
        .unwrap()
        .required_samples_for_resolution
        .unwrap()
    };
    let mut previous = 0;
    for share in declaration().schedule {
        let needed = required(share);
        assert!(needed > previous);
        previous = needed;
    }
    assert_eq!(required(50_000), 199);
    assert_eq!(required(49_999), 200);
    assert_eq!(required(5_000), 1_999);
}
