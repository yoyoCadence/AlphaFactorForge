use super::*;
use crate::discovery_core::alpha_allocation::{allocate_confirmation_alpha, AlphaAllocationStatus};
use serde_json::json;

fn family() -> String {
    format!("trial-family-v1:{}", "a".repeat(64))
}

fn budget() -> AlphaAllocationDeclaration {
    AlphaAllocationDeclaration {
        total_alpha_ppm: 50_000,
        schedule: vec![25_000, 25_000],
    }
}

fn body(number: u64, key: char) -> Value {
    json!({
        "contractVersion": ALPHA_RESERVATION_VERSION,
        "familyId": family(),
        "allocationId": alpha_allocation_id(&budget()).unwrap(),
        "originRegistryId": "b".repeat(32),
        "reservationKey": key.to_string().repeat(32),
        "confirmationBatchId": "c".repeat(64),
        "confirmationNumber": number,
        "alphaPpm": 25_000,
    })
}

fn freeze(body: &Value) -> AlphaReservation {
    freeze_alpha_reservation(&body.to_string()).unwrap()
}

fn reservation(number: u64, key: char) -> AlphaReservation {
    freeze(&body(number, key))
}

fn review(history: &[AlphaReservation]) -> AlphaReservationHistoryReport {
    review_alpha_reservations(&family(), &budget(), history).unwrap()
}

#[test]
fn empty_replay_and_exhausted_histories_keep_exact_numbering() {
    let first = reservation(1, 'd');
    let second = reservation(2, 'e');
    let empty = review(&[]);
    assert_eq!(empty.status, AlphaReservationHistoryStatus::Consistent);
    assert_eq!(empty.allocation.unwrap().confirmation_number, Some(1));

    let replay = review(&[first.clone(), first.clone()]);
    assert_eq!(replay.duplicate_replays, 1);
    assert_eq!(replay.evidence.as_slice(), std::slice::from_ref(&first));
    let next = replay.allocation.unwrap();
    assert_eq!(next.confirmation_number, Some(2));
    assert_eq!(next.alpha_ppm, Some(25_000));
    assert_eq!(next.spent_alpha_ppm, 25_000);

    // There is no lifecycle/status filter: failed or unfinished reservations
    // must be included by the caller and consume their shares equally.
    let exhausted = review(&[second, first]);
    assert_eq!(exhausted.status, AlphaReservationHistoryStatus::Consistent);
    assert!(exhausted.conflicts.is_empty());
    let allocation = exhausted.allocation.unwrap();
    assert_eq!(allocation.status, AlphaAllocationStatus::NotEligible);
    assert_eq!(allocation.spent_alpha_ppm, 50_000);
    assert_eq!(allocation.confirmation_number, None);
    assert_eq!(allocation.alpha_ppm, None);
}

#[test]
fn unequal_schedule_uses_explicit_numbers_before_projecting_amounts() {
    let declaration = AlphaAllocationDeclaration {
        total_alpha_ppm: 50_000,
        schedule: vec![30_000, 20_000],
    };
    let mut first_body = body(1, 'd');
    first_body["allocationId"] = json!(alpha_allocation_id(&declaration).unwrap());
    first_body["alphaPpm"] = json!(30_000);
    let first = freeze(&first_body);
    let next = review_alpha_reservations(&family(), &declaration, std::slice::from_ref(&first))
        .unwrap()
        .allocation
        .unwrap();
    assert_eq!(next.confirmation_number, Some(2));
    assert_eq!(next.alpha_ppm, Some(20_000));
    let mut second_body = first_body.clone();
    second_body["confirmationNumber"] = json!(2);
    second_body["reservationKey"] = json!("e".repeat(32));
    second_body["alphaPpm"] = json!(20_000);
    let second = freeze(&second_body);
    let exhausted = review_alpha_reservations(&family(), &declaration, &[second, first.clone()])
        .unwrap()
        .allocation
        .unwrap();
    assert_eq!(exhausted.spent_alpha_ppm, 50_000);
    assert_eq!(exhausted.status, AlphaAllocationStatus::NotEligible);
    second_body["alphaPpm"] = json!(30_000);
    assert!(
        review_alpha_reservations(&family(), &declaration, &[first, freeze(&second_body)]).is_err()
    );
}

#[test]
fn equal_shares_do_not_hide_independent_confirmation_one_reservations() {
    // This is the PR #134 counterexample: amounts alone wrongly look like
    // a complete schedule. Distinct operation keys are distinct reservations.
    let amount_only = allocate_confirmation_alpha(&budget(), &[25_000, 25_000]).unwrap();
    assert_eq!(amount_only.spent_alpha_ppm, 50_000);
    let first = reservation(1, 'd');
    let other = reservation(1, 'e');
    let report = review(&[first.clone(), other.clone(), first]);
    assert_eq!(report.status, AlphaReservationHistoryStatus::Blocked);
    assert_eq!(report.duplicate_replays, 1);
    assert_eq!(report.evidence.len(), 2);
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(
        report.conflicts[0].kind,
        AlphaReservationConflictKind::ConfirmationNumber
    );
    assert_eq!(report.conflicts[0].reservation_ids.len(), 2);
    assert_eq!(report.allocation, None);
    assert!(report.evidence.contains(&other));
}

#[test]
fn origin_keys_are_scoped_to_registry_but_cannot_be_reassigned() {
    let first = reservation(1, 'd');
    let reused = reservation(2, 'd');
    let report = review(&[first.clone(), reused]);
    assert_eq!(report.status, AlphaReservationHistoryStatus::Blocked);
    assert_eq!(
        report.conflicts[0].kind,
        AlphaReservationConflictKind::OriginKey
    );
    assert_eq!(report.allocation, None);

    let mut another_registry = body(2, 'd');
    another_registry["originRegistryId"] = json!("f".repeat(32));
    assert_eq!(
        review(&[first, freeze(&another_registry)]).status,
        AlphaReservationHistoryStatus::Consistent
    );
}

#[test]
fn conflict_evidence_is_deterministic_and_never_renumbered_or_discarded() {
    let a = reservation(1, 'd');
    let b = reservation(1, 'e');
    let c = reservation(2, 'd');
    let expected = review(&[a.clone(), b.clone(), c.clone(), a.clone()]);
    for records in [
        vec![c.clone(), a.clone(), b.clone(), a.clone()],
        vec![a.clone(), c, a.clone(), b],
    ] {
        assert_eq!(review(&records), expected);
    }
    assert_eq!(expected.evidence.len(), 3);
    assert_eq!(expected.conflicts.len(), 2);
    assert_eq!(expected.evidence[0].body.confirmation_number, 1);
    assert_eq!(expected.evidence[1].body.confirmation_number, 1);
    assert_eq!(expected.evidence[2].body.confirmation_number, 2);
    assert!(expected.allocation.is_none());
}

#[test]
fn binding_share_and_prefix_errors_never_produce_an_allocation() {
    let first = reservation(1, 'd');
    let mut other_family = body(2, 'e');
    other_family["familyId"] = json!(format!("trial-family-v1:{}", "f".repeat(64)));
    let mut other_allocation = body(2, 'e');
    other_allocation["allocationId"] = json!("f".repeat(64));
    let mut wrong_share = body(2, 'e');
    wrong_share["alphaPpm"] = json!(24_999);
    let overrun = body(3, 'e');
    for (value, expected) in [
        (other_family, "another family"),
        (other_allocation, "another allocation"),
        (wrong_share, "contradicts"),
        (overrun, "contradicts"),
    ] {
        let record = freeze(&value);
        let error =
            review_alpha_reservations(&family(), &budget(), &[first.clone(), record]).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
    assert!(
        review_alpha_reservations(&family(), &budget(), &[reservation(2, 'e')])
            .unwrap_err()
            .to_string()
            .contains("contiguous prefix")
    );

    let invalid = AlphaAllocationDeclaration {
        total_alpha_ppm: 1,
        schedule: vec![2],
    };
    assert!(review_alpha_reservations(&family(), &invalid, &[]).is_err());
    assert!(review_alpha_reservations("not-a-family", &budget(), &[]).is_err());
}

#[test]
fn every_body_field_is_hash_covered_and_stored_tampering_is_refused() {
    let original = reservation(1, 'd');
    let document = original.document();
    assert_eq!(
        parse_alpha_reservation(&document.to_string()).unwrap(),
        original
    );
    for (field, changed) in [
        (
            "familyId",
            json!(format!("trial-family-v1:{}", "f".repeat(64))),
        ),
        ("allocationId", json!("f".repeat(64))),
        ("originRegistryId", json!("f".repeat(32))),
        ("reservationKey", json!("e".repeat(32))),
        ("confirmationBatchId", json!("f".repeat(64))),
        ("confirmationNumber", json!(2)),
        ("alphaPpm", json!(24_999)),
    ] {
        let mut tampered = document.clone();
        tampered["body"][field] = changed;
        assert!(
            parse_alpha_reservation(&tampered.to_string()).is_err(),
            "{field}"
        );
        assert_ne!(
            freeze(&tampered["body"]).reservation_id(),
            original.reservation_id(),
            "{field}"
        );
    }
    let mut tampered_id = document;
    tampered_id["reservationId"] = json!("0".repeat(64));
    assert!(parse_alpha_reservation(&tampered_id.to_string()).is_err());
}

#[test]
fn strict_text_parsers_reject_duplicates_unknown_fields_and_invalid_domains() {
    let valid = body(1, 'd');
    let raw = valid.to_string();
    let duplicated = raw.replacen('{', "{\"alphaPpm\":25000,", 1);
    assert!(freeze_alpha_reservation(&duplicated)
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
    for (field, invalid) in [
        ("contractVersion", json!("research-alpha-reservation-v99")),
        ("familyId", json!("a".repeat(64))),
        ("allocationId", json!("F".repeat(64))),
        ("originRegistryId", json!("b".repeat(31))),
        ("reservationKey", json!("d".repeat(33))),
        ("confirmationBatchId", json!("c".repeat(63))),
        ("confirmationNumber", json!(0)),
        ("confirmationNumber", json!(1_000_000)),
        ("confirmationNumber", json!(1.0)),
        ("confirmationNumber", json!(u64::MAX)),
        ("alphaPpm", json!(0)),
        ("alphaPpm", json!(-1)),
        ("alphaPpm", json!(1_000_000)),
        ("unknown", json!(true)),
    ] {
        let mut changed = valid.clone();
        changed[field] = invalid;
        assert!(
            freeze_alpha_reservation(&changed.to_string()).is_err(),
            "{field}"
        );
    }
    for field in valid.as_object().unwrap().keys() {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            freeze_alpha_reservation(&missing.to_string()).is_err(),
            "{field}"
        );
    }
    let stored = reservation(1, 'd').document().to_string();
    let duplicate_id = stored.replacen('{', "{\"reservationId\":\"extra\",", 1);
    assert!(parse_alpha_reservation(&duplicate_id).is_err());
    let nested_duplicate = stored.replacen("\"body\":{", "\"body\":{\"alphaPpm\":25000,", 1);
    assert!(parse_alpha_reservation(&nested_duplicate).is_err());
    let mut unknown = reservation(1, 'd').document();
    unknown["status"] = json!("completed");
    assert!(parse_alpha_reservation(&unknown.to_string()).is_err());
}

#[test]
fn identity_matches_the_independent_typescript_golden_record() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-alpha-reservation-v1.json"
    ))
    .unwrap();
    let record = freeze(&fixture["body"]);
    assert_eq!(
        record.reservation_id(),
        fixture["reservationId"].as_str().unwrap()
    );
    let stored = json!({"reservationId": fixture["reservationId"], "body": fixture["body"]});
    assert_eq!(
        parse_alpha_reservation(&stored.to_string()).unwrap(),
        record
    );
}

#[test]
fn a_valid_truncated_prefix_cannot_prove_history_completeness() {
    // Document the pure boundary's limit, so callers cannot mistake its
    // result for a registry watermark, atomic reservation or replay permit.
    let first = reservation(1, 'd');
    let second = reservation(2, 'e');
    assert_eq!(
        review(&[first.clone(), second])
            .allocation
            .unwrap()
            .confirmation_number,
        None
    );
    assert_eq!(
        review(&[first]).allocation.unwrap().confirmation_number,
        Some(2)
    );
}
