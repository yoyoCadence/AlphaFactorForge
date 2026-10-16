use super::*;
use crate::discovery_core::{
    alpha_reservations::{freeze_alpha_reservation, parse_alpha_reservation},
    confirmation_contracts::{
        freeze_operation, freeze_validation_batch, parse_operation, parse_selection,
        parse_test_batch, parse_validation_batch, RegisteredSource, SealedValidationInput,
    },
};
use serde_json::json;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-confirmation-history-records-v1.json"
    ))
    .unwrap()
}
fn base() -> Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-confirmation-contracts-v1.json"
    ))
    .unwrap()
}
fn registered(body: &Value) -> RegisteredSource {
    RegisteredSource {
        source: serde_json::from_value(body["source"].clone()).unwrap(),
        candidates: serde_json::from_value(body["candidates"].clone()).unwrap(),
    }
}
fn batch(stage: &str) -> FrozenBatch {
    let f = base();
    let v = parse_validation_batch(
        &f["validation"].to_string(),
        &registered(&f["validation"]["body"]),
    )
    .unwrap();
    if stage == "validation" {
        return v;
    }
    let r = parse_alpha_reservation(&f["alphaReservation"].to_string()).unwrap();
    let s = parse_selection(
        &f["selection"].to_string(),
        &v,
        &SealedValidationInput {
            receipt_id: f["sealReceiptId"].as_str().unwrap(),
            reservation: &r,
            report_json: f["reportJson"].as_str().unwrap(),
        },
    )
    .unwrap();
    parse_test_batch(&f["test"].to_string(), &v, &s).unwrap()
}
fn setup(stage: &str) -> (FrozenBatch, FrozenOperation, AlphaReservation) {
    let b = batch(stage);
    let f = base();
    let o = parse_operation(&f[format!("{stage}Operation")].to_string(), &b).unwrap();
    let a = parse_alpha_reservation(&fixture()["reservations"][stage].to_string()).unwrap();
    (b, o, a)
}
fn body(name: &str) -> Value {
    fixture()["records"][name]["body"].clone()
}

#[test]
fn all_independent_identities_roundtrip_without_changing_old_binding_ids() {
    let f = fixture();
    let (b, o, a) = setup("validation");
    assert_eq!(b.allocation_id(), a.allocation_id());
    assert_eq!(o.workspace_id(), b.source().workspace_id);
    assert_eq!(o.confirmation_batch_id(), b.id());
    for stage in ["validation", "test"] {
        let (b, o, a) = setup(stage);
        let stored = &f["records"][format!("{stage}Exposure")];
        let e = parse_confirmation_exposure(&stored.to_string(), &b, &o, &a).unwrap();
        assert_eq!(e.id(), stored["id"]);
        assert_eq!(e.document(), *stored);
        assert_eq!(e.footprint(), b.footprint());
        assert_eq!(e.operation_id(), Some(o.id()));
        assert_eq!(e.origin_registry_id(), o.origin_registry_id());
        assert_eq!(e.reservation_key(), o.reservation_key());
        assert_eq!(
            freeze_confirmation_exposure(&stored["body"].to_string(), &b, &o, &a).unwrap(),
            e
        );
    }
    for name in ["manual", "legacy"] {
        let r = parse_manual_exposure(&f["records"][name].to_string()).unwrap();
        assert_eq!(r.id(), f["records"][name]["id"]);
        assert_eq!(r.operation_id(), None);
        assert_eq!(freeze_manual_exposure(&body(name).to_string()).unwrap(), r);
    }
    for name in ["started", "sealed", "interrupted", "failed", "cancelled"] {
        let e = parse_confirmation_event(&f["records"][name].to_string(), &b, &o).unwrap();
        assert_eq!(e.id(), f["records"][name]["id"]);
        assert_eq!(e.operation_id(), o.id());
        assert_eq!(e.admission_id(), "9".repeat(64));
        assert_eq!(
            freeze_confirmation_event(&body(name).to_string(), &b, &o).unwrap(),
            e
        );
    }
    for (name, stored) in f["records"].as_object().unwrap() {
        if !name.starts_with("conflict_") {
            continue;
        }
        let d = parse_conflict_declaration(&stored.to_string()).unwrap();
        assert_eq!(d.id(), stored["id"]);
        assert_eq!(
            freeze_conflict_declaration(&stored["body"].to_string()).unwrap(),
            d
        );
    }
}

#[test]
fn managed_exposure_rejects_each_changed_bound_reference_and_original_operation_key() {
    let (b, o, a) = setup("validation");
    for (pointer, value) in [
        ("/originRegistryId", json!("a".repeat(32))),
        ("/reservationKey", json!("a".repeat(32))),
        ("/instrumentId", json!("crypto:binance:ETHUSDT")),
        ("/claim/operationId", json!("a".repeat(64))),
        ("/claim/alphaReservationId", json!("a".repeat(64))),
    ] {
        let mut raw = body("validationExposure");
        *raw.pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            freeze_confirmation_exposure(&raw.to_string(), &b, &o, &a).unwrap_err(),
            HistoryRecordError::Binding,
            "{pointer}"
        );
    }
    for (field, value) in [
        ("originRegistryId", json!("a".repeat(32))),
        ("reservationKey", json!("a".repeat(32))),
        ("confirmationBatchId", json!("a".repeat(64))),
        ("allocationId", json!("a".repeat(64))),
        (
            "familyId",
            json!(confirmation_family_id("crypto:binance:ETHUSDT").unwrap()),
        ),
    ] {
        let mut raw = fixture()["reservations"]["validation"]["body"].clone();
        raw[field] = value;
        let other = freeze_alpha_reservation(&raw.to_string()).unwrap();
        let mut e = body("validationExposure");
        e["claim"]["alphaReservationId"] = json!(other.reservation_id());
        assert_eq!(
            freeze_confirmation_exposure(&e.to_string(), &b, &o, &other).unwrap_err(),
            HistoryRecordError::Binding,
            "{field}"
        );
    }
}

#[test]
fn managed_support_is_exact_and_never_shortened_extended_or_replaced_by_another_stage() {
    let (v, vo, va) = setup("validation");
    let (t, to, ta) = setup("test");
    let from = v.footprint().intervals()[0].from_ms();
    let end = v.footprint().intervals()[0].to_exclusive_ms();
    for intervals in [
        json!([{"fromMs":from+1,"toExclusiveMs":end}]),
        json!([{"fromMs":from-1,"toExclusiveMs":end}]),
        json!([{"fromMs":from,"toExclusiveMs":end+1}]),
        json!([{"fromMs":end,"toExclusiveMs":end+1}]),
        base()["test"]["body"]["split"]["testSupport"].clone(),
    ] {
        let mut e = body("validationExposure");
        e["intervals"] = intervals;
        assert_eq!(
            freeze_confirmation_exposure(&e.to_string(), &v, &vo, &va).unwrap_err(),
            HistoryRecordError::Binding
        );
    }
    assert_eq!(
        freeze_confirmation_exposure(&body("validationExposure").to_string(), &t, &to, &ta)
            .unwrap_err(),
        HistoryRecordError::Binding
    );
    assert_eq!(
        freeze_confirmation_exposure(&body("testExposure").to_string(), &t, &to, &va).unwrap_err(),
        HistoryRecordError::Binding
    );
    assert_eq!(
        freeze_confirmation_exposure(&body("validationExposure").to_string(), &t, &vo, &va)
            .unwrap_err(),
        HistoryRecordError::Binding
    );
    assert!(!super::super::confirmation_contracts::footprints_overlap(
        &v.footprint(),
        &t.footprint()
    ));
}

#[test]
fn a_replaced_operation_cannot_keep_the_old_exposure_identity_even_for_the_same_batch() {
    let (b, o, a) = setup("validation");
    let mut raw = base()["validationOperation"]["body"].clone();
    raw["requestId"] = json!("a".repeat(32));
    let changed = freeze_operation(&raw.to_string(), &b).unwrap();
    assert_ne!(changed.id(), o.id());
    assert_eq!(
        freeze_confirmation_exposure(&body("validationExposure").to_string(), &b, &changed, &a)
            .unwrap_err(),
        HistoryRecordError::Binding
    );
    let mut updated = body("validationExposure");
    updated["claim"]["operationId"] = json!(changed.id());
    let e = freeze_confirmation_exposure(&updated.to_string(), &b, &changed, &a).unwrap();
    assert_ne!(e.id(), fixture()["records"]["validationExposure"]["id"]);
}

#[test]
fn manual_and_legacy_variants_retain_all_closed_codes_and_nullable_references() {
    let (b, o, a) = setup("validation");
    for access in ["raw", "chart", "holdout", "worker", "export", "history"] {
        for seal in [Value::Null, json!("a".repeat(64))] {
            let mut e = body("manual");
            e["claim"]["accessKind"] = json!(access);
            e["claim"]["afterSealId"] = seal;
            let r = freeze_manual_exposure(&e.to_string()).unwrap();
            assert_eq!(parse_manual_exposure(&r.document().to_string()).unwrap(), r);
            assert_eq!(
                freeze_confirmation_exposure(&e.to_string(), &b, &o, &a).unwrap_err(),
                HistoryRecordError::Variant
            );
        }
    }
    for reason in ["migrated_workspace", "legacy_import", "unmediated_access"] {
        for evidence in [Value::Null, json!("a".repeat(64))] {
            let mut e = body("legacy");
            e["claim"]["reason"] = json!(reason);
            e["claim"]["sourceEvidenceSha256"] = evidence;
            assert!(freeze_manual_exposure(&e.to_string()).is_ok());
        }
    }
    assert_eq!(
        freeze_manual_exposure(&body("validationExposure").to_string()).unwrap_err(),
        HistoryRecordError::Variant
    );
    let mut e = body("manual");
    e["claim"]["kind"] = json!("known_unseen");
    assert_eq!(
        freeze_manual_exposure(&e.to_string()).unwrap_err(),
        HistoryRecordError::Malformed
    );
}

#[test]
fn event_binding_checks_batch_operation_started_workspace_and_sealed_engine() {
    let (b, o, _) = setup("validation");
    let (t, to, _) = setup("test");
    for (name, pointer, value) in [
        ("started", "/operationId", json!("a".repeat(64))),
        ("started", "/event/workspaceId", json!("a".repeat(32))),
        ("sealed", "/event/engineManifestHash", json!("a".repeat(64))),
    ] {
        let mut e = body(name);
        *e.pointer_mut(pointer).unwrap() = value;
        assert_eq!(
            freeze_confirmation_event(&e.to_string(), &b, &o).unwrap_err(),
            HistoryRecordError::Binding
        );
    }
    assert_eq!(
        freeze_confirmation_event(&body("started").to_string(), &t, &o).unwrap_err(),
        HistoryRecordError::Binding
    );
    assert_eq!(
        freeze_confirmation_event(&body("started").to_string(), &t, &to).unwrap_err(),
        HistoryRecordError::Binding
    );
    let mut body_b = b.document()["body"].clone();
    body_b["statistics"]["blockLengthRationale"] = json!("Changed frozen rationale");
    let newer = freeze_validation_batch(&body_b.to_string(), &registered(&body_b)).unwrap();
    assert_eq!(
        freeze_confirmation_event(&body("sealed").to_string(), &newer, &o).unwrap_err(),
        HistoryRecordError::Binding
    );
}

#[test]
fn every_terminal_variant_and_reason_retains_spend_without_resolving_references() {
    let (b, o, _) = setup("validation");
    for kind in ["interrupted_spent", "failed_spent", "cancelled_spent"] {
        for reason in [
            "owner_lost",
            "cancelled",
            "engine_missing",
            "input_missing",
            "artifact_missing",
            "execution_failed",
            "seal_missing",
        ] {
            for started in [Value::Null, json!("a".repeat(64))] {
                let mut e = body("failed");
                e["event"] = json!({"kind":kind,"executionStartedId":started,"reason":reason});
                let r = freeze_confirmation_event(&e.to_string(), &b, &o).unwrap();
                assert_eq!(
                    parse_confirmation_event(&r.document().to_string(), &b, &o).unwrap(),
                    r
                );
            }
        }
    }
}

#[test]
fn references_counts_and_original_json_numeric_domains_are_strict() {
    let (b, o, a) = setup("validation");
    for (name, pointer, value) in [
        ("started", "/admissionId", json!("a".repeat(63))),
        ("started", "/event/ownershipEpoch", json!(0)),
        (
            "started",
            "/event/ownershipEpoch",
            json!(PRECISION_MAX_COUNT + 1),
        ),
        ("started", "/event/executionKey", json!("A".repeat(32))),
        ("sealed", "/event/resultArtifactBytes", json!(0)),
        (
            "sealed",
            "/event/resultArtifactBytes",
            json!(PRECISION_MAX_COUNT + 1),
        ),
        ("sealed", "/event/executionStartedId", json!("a".repeat(63))),
        ("sealed", "/event/reportSha256", json!("a".repeat(63))),
        (
            "sealed",
            "/event/resultArtifactSha256",
            json!("A".repeat(64)),
        ),
        ("failed", "/event/executionStartedId", json!("wrong")),
    ] {
        let mut e = body(name);
        *e.pointer_mut(pointer).unwrap() = value;
        assert!(
            freeze_confirmation_event(&e.to_string(), &b, &o).is_err(),
            "{pointer}"
        );
    }
    for name in ["started", "sealed"] {
        let mut e = body(name);
        e["event"][if name == "started" {
            "ownershipEpoch"
        } else {
            "resultArtifactBytes"
        }] = json!(PRECISION_MAX_COUNT);
        assert!(freeze_confirmation_event(&e.to_string(), &b, &o).is_ok());
    }
    for (pointer, value) in [
        ("/originRegistryId", json!("A".repeat(32))),
        ("/claim/operationId", json!("not-a-hash")),
        ("/intervals/0/fromMs", json!(0)),
        ("/intervals/0/toExclusiveMs", json!(u64::MAX)),
    ] {
        let mut e = body("validationExposure");
        *e.pointer_mut(pointer).unwrap() = value;
        assert!(freeze_confirmation_exposure(&e.to_string(), &b, &o, &a).is_err());
    }
    for (old, new) in [
        ("\"ownershipEpoch\":7", "\"ownershipEpoch\":7.0"),
        ("\"ownershipEpoch\":7", "\"ownershipEpoch\":7e0"),
        (
            "\"ownershipEpoch\":7",
            "\"ownershipEpoch\":7,\"ownershipEpoch\":7",
        ),
    ] {
        let raw = body("started").to_string();
        assert!(raw.contains(old));
        assert_eq!(
            freeze_confirmation_event(&raw.replacen(old, new, 1), &b, &o).unwrap_err(),
            HistoryRecordError::Malformed
        );
    }
}

#[test]
fn missing_nullables_unknown_nested_fields_and_duplicate_tags_are_refused() {
    let (b, o, a) = setup("validation");
    for (name, field) in [
        ("manual", "afterSealId"),
        ("legacy", "sourceEvidenceSha256"),
    ] {
        let mut e = body(name);
        e["claim"].as_object_mut().unwrap().remove(field);
        assert_eq!(
            freeze_manual_exposure(&e.to_string()).unwrap_err(),
            HistoryRecordError::Malformed
        );
    }
    for name in ["failed", "cancelled", "interrupted"] {
        let mut e = body(name);
        e["event"]
            .as_object_mut()
            .unwrap()
            .remove("executionStartedId");
        assert_eq!(
            freeze_confirmation_event(&e.to_string(), &b, &o).unwrap_err(),
            HistoryRecordError::Malformed
        );
    }
    for pointer in ["", "/claim", "/intervals/0"] {
        let mut e = body("validationExposure");
        e.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!("PROTECTED_SENTINEL"));
        assert_eq!(
            freeze_confirmation_exposure(&e.to_string(), &b, &o, &a).unwrap_err(),
            HistoryRecordError::Malformed
        );
    }
    for name in ["started", "sealed", "failed", "interrupted", "cancelled"] {
        for pointer in ["", "/event"] {
            let mut e = body(name);
            e.pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("afterSealId".into(), Value::Null);
            assert_eq!(
                freeze_confirmation_event(&e.to_string(), &b, &o).unwrap_err(),
                HistoryRecordError::Malformed
            );
        }
    }
    for name in ["manual", "legacy"] {
        let mut e = body(name);
        e["claim"]["operationId"] = json!("a".repeat(64));
        assert_eq!(
            freeze_manual_exposure(&e.to_string()).unwrap_err(),
            HistoryRecordError::Malformed
        );
    }
    let raw = body("manual").to_string();
    let needle = "\"kind\":\"manual\"";
    assert_eq!(
        freeze_manual_exposure(&raw.replacen(needle, &format!("{needle},{needle}"), 1))
            .unwrap_err(),
        HistoryRecordError::Malformed
    );
}

#[test]
fn conflict_declarations_bound_shape_canonical_targets_and_family_instrument_derivation() {
    let original = body("conflict_origin_key");
    for (field, value) in [
        ("claimIds", json!([])),
        ("claimIds", json!(["a".repeat(64)])),
        (
            "claimIds",
            json!(["a".repeat(64), "b".repeat(64), "c".repeat(64)]),
        ),
        ("claimIds", json!(["d".repeat(64), "c".repeat(64)])),
        ("claimIds", json!(["c".repeat(64), "c".repeat(64)])),
        ("affectedInstrumentIds", json!([])),
        ("affectedInstrumentIds", json!(["crypto:BINANCE:BTCUSDT"])),
        (
            "affectedInstrumentIds",
            json!(["crypto:binance:ETHUSDT", "crypto:binance:BTCUSDT"]),
        ),
        (
            "affectedInstrumentIds",
            json!([
                "crypto:binance:BTCUSDT",
                "crypto:binance:ETHUSDT",
                "crypto:binance:XRPUSDT"
            ]),
        ),
        (
            "affectedFamilyIds",
            json!([confirmation_family_id("crypto:binance:ETHUSDT").unwrap()]),
        ),
        (
            "affectedFamilyIds",
            json!([
                original["affectedFamilyIds"][0].clone(),
                original["affectedFamilyIds"][0].clone()
            ]),
        ),
        ("kind", json!("automatic_refund")),
    ] {
        let mut c = original.clone();
        c[field] = value;
        assert!(
            freeze_conflict_declaration(&c.to_string()).is_err(),
            "{field}"
        );
    }
    let mut c = original.clone();
    c["affectedFamilyIds"] = json!([]);
    assert!(freeze_conflict_declaration(&c.to_string()).is_ok());
    c["affectedInstrumentIds"] = json!(["crypto:binance:BTCUSDT", "crypto:binance:ETHUSDT"]);
    let mut families = vec![
        confirmation_family_id("crypto:binance:BTCUSDT").unwrap(),
        confirmation_family_id("crypto:binance:ETHUSDT").unwrap(),
    ];
    families.sort();
    c["affectedFamilyIds"] = json!(families);
    let d = freeze_conflict_declaration(&c.to_string()).unwrap();
    assert_eq!(
        parse_conflict_declaration(&d.document().to_string()).unwrap(),
        d
    );
    let mut c = original;
    c["notes"] = json!("PROTECTED_SENTINEL");
    assert_eq!(
        freeze_conflict_declaration(&c.to_string()).unwrap_err(),
        HistoryRecordError::Malformed
    );
}

#[test]
fn stored_ids_and_closed_wrappers_cannot_be_trusted_after_rehash_or_field_substitution() {
    let f = fixture();
    let (b, o, a) = setup("validation");
    for field in ["id", "unexpected"] {
        for name in [
            "validationExposure",
            "manual",
            "started",
            "conflict_origin_key",
        ] {
            let mut r = f["records"][name].clone();
            r[field] = json!("a".repeat(64));
            let error = match name {
                "validationExposure" => {
                    parse_confirmation_exposure(&r.to_string(), &b, &o, &a).unwrap_err()
                }
                "manual" => parse_manual_exposure(&r.to_string()).unwrap_err(),
                "started" => parse_confirmation_event(&r.to_string(), &b, &o).unwrap_err(),
                _ => parse_conflict_declaration(&r.to_string()).unwrap_err(),
            };
            assert_eq!(
                error,
                if field == "id" {
                    HistoryRecordError::Identity
                } else {
                    HistoryRecordError::Malformed
                }
            );
        }
    }
    let mut r = f["records"]["started"].clone();
    r["body"]["event"]["ownershipEpoch"] = json!(8);
    assert_eq!(
        parse_confirmation_event(&r.to_string(), &b, &o).unwrap_err(),
        HistoryRecordError::Identity
    );
    let mut e = body("manual");
    e["claim"]["accessKind"] = json!("export");
    let changed = freeze_manual_exposure(&e.to_string()).unwrap();
    assert_ne!(changed.id(), f["records"]["manual"]["id"]);
    let raw = f["records"]["manual"].to_string();
    let needle = format!(
        "\"id\":\"{}\"",
        f["records"]["manual"]["id"].as_str().unwrap()
    );
    assert_eq!(
        parse_manual_exposure(&raw.replacen(&needle, &format!("{needle},{needle}"), 1))
            .unwrap_err(),
        HistoryRecordError::Malformed
    );
}

#[test]
fn manual_interval_cap_and_market_endpoints_use_existing_half_open_contract() {
    let mut e = body("manual");
    let t = 946684800000u64;
    let last = 4102444800000u64;
    let intervals: Vec<Value> = (0..128)
        .map(|i| json!({"fromMs":t+i*2,"toExclusiveMs":t+i*2+1}))
        .collect();
    e["intervals"] = json!(intervals);
    let r = freeze_manual_exposure(&e.to_string()).unwrap();
    assert_eq!(r.footprint().intervals().len(), 128);
    e["intervals"]
        .as_array_mut()
        .unwrap()
        .push(json!({"fromMs":t+256,"toExclusiveMs":t+257}));
    assert_eq!(
        freeze_manual_exposure(&e.to_string()).unwrap_err(),
        HistoryRecordError::Intervals
    );
    for intervals in [
        json!([]),
        json!([{"fromMs":t,"toExclusiveMs":t}]),
        json!([{"fromMs":t-1,"toExclusiveMs":t+1}]),
        json!([{"fromMs":last-1,"toExclusiveMs":last+1}]),
        json!([{"fromMs":t,"toExclusiveMs":t+2},{"fromMs":t+2,"toExclusiveMs":t+3}]),
        json!([{"fromMs":t+2,"toExclusiveMs":t+3},{"fromMs":t,"toExclusiveMs":t+1}]),
    ] {
        e["intervals"] = intervals;
        assert_eq!(
            freeze_manual_exposure(&e.to_string()).unwrap_err(),
            HistoryRecordError::Intervals
        );
    }
    e["intervals"] = json!([{"fromMs":last-1,"toExclusiveMs":last}]);
    assert!(freeze_manual_exposure(&e.to_string()).is_ok());
}

#[test]
fn exact_raw_byte_capacity_and_one_over_apply_to_every_constructor_and_stored_parser() {
    let f = fixture();
    let (b, o, a) = setup("validation");
    fn padded(v: &Value, n: usize) -> String {
        let mut raw = v.to_string();
        raw.extend(std::iter::repeat_n(' ', n - raw.len()));
        raw
    }
    for n in [MAX_DOCUMENT_BYTES, MAX_DOCUMENT_BYTES + 1] {
        let results = [
            freeze_confirmation_exposure(&padded(&body("validationExposure"), n), &b, &o, &a)
                .map(|_| ()),
            freeze_manual_exposure(&padded(&body("manual"), n)).map(|_| ()),
            freeze_confirmation_event(&padded(&body("started"), n), &b, &o).map(|_| ()),
            freeze_conflict_declaration(&padded(&body("conflict_origin_key"), n)).map(|_| ()),
            parse_confirmation_exposure(
                &padded(&f["records"]["validationExposure"], n),
                &b,
                &o,
                &a,
            )
            .map(|_| ()),
            parse_manual_exposure(&padded(&f["records"]["manual"], n)).map(|_| ()),
            parse_confirmation_event(&padded(&f["records"]["started"], n), &b, &o).map(|_| ()),
            parse_conflict_declaration(&padded(&f["records"]["conflict_origin_key"], n))
                .map(|_| ()),
        ];
        for result in results {
            if n == MAX_DOCUMENT_BYTES {
                result.unwrap();
            } else {
                assert_eq!(result.unwrap_err(), HistoryRecordError::TooLarge);
            }
        }
    }
}

#[test]
fn unsupported_versions_and_diagnostics_never_echo_protected_input() {
    let (b, o, a) = setup("validation");
    let mut e = body("manual");
    e["contractVersion"] = json!("future");
    assert_eq!(
        freeze_manual_exposure(&e.to_string()).unwrap_err(),
        HistoryRecordError::Version
    );
    let mut e = body("started");
    e["contractVersion"] = json!("future");
    assert_eq!(
        freeze_confirmation_event(&e.to_string(), &b, &o).unwrap_err(),
        HistoryRecordError::Version
    );
    let mut e = body("conflict_seal");
    e["contractVersion"] = json!("future");
    assert_eq!(
        freeze_conflict_declaration(&e.to_string()).unwrap_err(),
        HistoryRecordError::Version
    );
    for raw in [
        "{\"PROTECTED_SENTINEL\":42}",
        "{invalid:PROTECTED_SENTINEL}",
        "\"PROTECTED_SENTINEL\"",
    ] {
        for error in [
            freeze_confirmation_exposure(raw, &b, &o, &a).unwrap_err(),
            freeze_manual_exposure(raw).unwrap_err(),
            freeze_confirmation_event(raw, &b, &o).unwrap_err(),
            freeze_conflict_declaration(raw).unwrap_err(),
        ] {
            assert_eq!(error, HistoryRecordError::Malformed);
            assert!(!format!("{error:?} {error}").contains("PROTECTED_SENTINEL"));
        }
    }
}

#[test]
fn partial_records_deliberately_do_not_validate_graph_existence_or_create_authority() {
    let (b, o, _) = setup("validation");
    // These are declarations, not verified conflict evidence or valid seals.
    // The next slice must resolve every actual admission/start/report/claim.
    let mut e = body("sealed");
    e["admissionId"] = json!("a".repeat(64));
    e["event"]["executionStartedId"] = json!("b".repeat(64));
    e["event"]["reportSha256"] = json!("c".repeat(64));
    assert!(freeze_confirmation_event(&e.to_string(), &b, &o).is_ok());
    let mut c = body("conflict_origin_key");
    c["claimIds"] = json!(["a".repeat(64), "b".repeat(64)]);
    assert!(freeze_conflict_declaration(&c.to_string()).is_ok());
    let mut m = body("manual");
    m["claim"]["afterSealId"] = json!("a".repeat(64));
    assert!(freeze_manual_exposure(&m.to_string()).is_ok());
}
