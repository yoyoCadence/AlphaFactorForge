use super::*;
use crate::discovery_core::alpha_reservations::{
    freeze_alpha_reservation, parse_alpha_reservation, AlphaReservation,
};
use serde_json::json;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-confirmation-contracts-v1.json"
    ))
    .unwrap()
}
fn source(body: &Value) -> RegisteredSource {
    RegisteredSource {
        source: serde_json::from_value(body["source"].clone()).unwrap(),
        candidates: serde_json::from_value(body["candidates"].clone()).unwrap(),
    }
}
fn parent(f: &Value) -> FrozenBatch {
    parse_validation_batch(
        &f["validation"].to_string(),
        &source(&f["validation"]["body"]),
    )
    .unwrap()
}
fn reservation(parent: &FrozenBatch) -> AlphaReservation {
    let mut body = fixture()["alphaReservation"]["body"].clone();
    body["confirmationBatchId"] = json!(parent.id());
    freeze_alpha_reservation(&body.to_string()).unwrap()
}
fn select(
    parent: &FrozenBatch,
    report: &str,
    selected: &[u64],
) -> Result<FrozenSelection, ContractError> {
    let f = fixture();
    let reserve = reservation(parent);
    let mut body = f["selection"]["body"].clone();
    body["validationBatchId"] = json!(parent.id());
    body["alphaReservationId"] = json!(reserve.reservation_id());
    body["reportSha256"] = json!(hex::encode(Sha256::digest(report.as_bytes())));
    body["selectedCandidates"] = json!(selected);
    freeze_selection(
        &body.to_string(),
        parent,
        &SealedValidationInput {
            receipt_id: f["sealReceiptId"].as_str().unwrap(),
            reservation: &reserve,
            report_json: report,
        },
    )
}
fn report() -> Value {
    serde_json::from_str(fixture()["reportJson"].as_str().unwrap()).unwrap()
}
fn test_body(parent: &FrozenBatch, selection: &FrozenSelection) -> Value {
    let mut body = parent.document()["body"].clone();
    body["stage"] =
        json!({"kind":"test","validationBatchId":parent.id(),"selectionId":selection.id()});
    body["candidates"] = json!(parent
        .candidates()
        .iter()
        .filter(|c| selection.selected_candidates().contains(&c.candidate_index))
        .collect::<Vec<_>>());
    body
}
fn all_zero_report(parent: &FrozenBatch, family_tests: u64) -> String {
    let b = parent.body.statistics.bootstrap_samples;
    let denominator = b + 1;
    // With every raw numerator 1, the first multiplier dominates every later
    // one. The hand-calculated running adjusted numerator stays m throughout.
    let tests: Vec<Value> = parent
        .candidates()
        .iter()
        .flat_map(|c| {
            [
                (c.candidate_index, "net_return"),
                (c.candidate_index, "benchmark_excess"),
            ]
        })
        .enumerate()
        .map(|(i, (index, kind))| {
            let adjusted = family_tests.min(denominator);
            json!({
                "candidateIndex": index, "test": kind, "observations": 256,
                "observedMean": 0.1, "extremeCount": 0,
                "rawP": {"numerator": 1, "denominator": denominator},
                "rawPValue": 1.0 / denominator as f64, "holmRank": i + 1,
                "adjustedP": {"numerator": adjusted, "denominator": denominator},
                "adjustedPValue": adjusted as f64 / denominator as f64,
                "rejectsNull": u128::from(adjusted) * 1_000_000 <= 25_000 * u128::from(denominator)
            })
        })
        .collect();
    json!({
        "contractVersion": CONFIRMATION_STATISTICS_V2_VERSION,
        "correction": "holm", "scheme": "circular-block", "prng": "splitmix64",
        "alphaPpm": 25000, "blockLength": 6, "bootstrapSamples": b,
        "seed": 7, "familyTests": family_tests, "tests": tests
    })
    .to_string()
}

#[test]
fn independent_fixture_roundtrips_every_document_and_existing_family_budget_ids() {
    let f = fixture();
    let p = parent(&f);
    assert_eq!(p.id(), f["validation"]["id"]);
    assert_eq!(
        confirmation_family_id("crypto:binance:BTCUSDT").unwrap(),
        f["validation"]["body"]["source"]["familyId"]
    );
    let allocation = crate::discovery_core::alpha_allocation::parse_alpha_allocation(
        &f["allocationDeclaration"],
    )
    .unwrap();
    assert_eq!(
        crate::discovery_core::alpha_allocation::alpha_allocation_id(&allocation).unwrap(),
        p.body.statistics.allocation_id
    );
    let reserve = parse_alpha_reservation(&f["alphaReservation"].to_string()).unwrap();
    let input = SealedValidationInput {
        receipt_id: f["sealReceiptId"].as_str().unwrap(),
        reservation: &reserve,
        report_json: f["reportJson"].as_str().unwrap(),
    };
    let selection = parse_selection(&f["selection"].to_string(), &p, &input).unwrap();
    assert_eq!(selection.id(), f["selection"]["id"]);
    assert_eq!(selection.selected_candidates(), [0]);
    let t = parse_test_batch(&f["test"].to_string(), &p, &selection).unwrap();
    assert_eq!(t.id(), f["test"]["id"]);
    assert!(!t.is_validation());
    assert!(!footprints_overlap(&p.footprint(), &t.footprint()));
    for (name, batch) in [("validationOperation", &p), ("testOperation", &t)] {
        let operation = parse_operation(&f[name].to_string(), batch).unwrap();
        assert_eq!(operation.id(), f[name]["id"]);
        assert_eq!(
            freeze_operation(&f[name]["body"].to_string(), batch).unwrap(),
            operation
        );
    }
}

#[test]
fn validation_rejects_missing_extra_substituted_and_reordered_registered_members() {
    let f = fixture();
    let original = &f["validation"]["body"];
    let evidence = source(original);
    for kind in 0..5 {
        let mut body = original.clone();
        match kind {
            0 => {
                body["candidates"].as_array_mut().unwrap().pop();
            }
            1 => {
                let extra = body["candidates"][1].clone();
                body["candidates"].as_array_mut().unwrap().push(extra);
            }
            2 => {
                body["candidates"][0]["strategyHash"] =
                    json!(format!("strategy-v2:{}", "a".repeat(64)))
            }
            3 => body["candidates"].as_array_mut().unwrap().reverse(),
            _ => body["source"]["trialBatchId"] = json!("a".repeat(64)),
        }
        assert!(
            freeze_validation_batch(&body.to_string(), &evidence).is_err(),
            "case {kind}"
        );
    }
}

#[test]
fn consistent_truncated_context_is_not_a_completeness_or_authority_proof() {
    let mut body = fixture()["validation"]["body"].clone();
    body["candidates"].as_array_mut().unwrap().pop();
    let truncated = freeze_validation_batch(&body.to_string(), &source(&body)).unwrap();
    assert_eq!(truncated.candidates().len(), 1);
    assert_ne!(truncated.id(), fixture()["validation"]["id"]);
}

#[test]
fn every_test_common_binding_and_candidate_must_equal_its_parent() {
    let f = fixture();
    let p = parent(&f);
    let s = select(&p, f["reportJson"].as_str().unwrap(), &[0]).unwrap();
    let original = test_body(&p, &s);
    for pointer in [
        "/source/campaignId",
        "/source/engineManifestHash",
        "/data/snapshotId",
        "/data/instrumentRevisionHash",
        "/data/calendarHash",
        "/data/costProfileHash",
        "/data/numericCostsHash",
        "/data/sourceEvidenceHash",
        "/statistics/allocationId",
        "/stage/validationBatchId",
        "/stage/selectionId",
        "/candidates/0/definitionHash",
        "/candidates/0/executionHash",
        "/candidates/0/resultArtifactHash",
        "/candidates/0/lineageHash",
    ] {
        let mut body = original.clone();
        *body.pointer_mut(pointer).unwrap() = json!("a".repeat(64));
        assert!(
            freeze_test_batch(&body.to_string(), &p, &s).is_err(),
            "{pointer}"
        );
    }
    for (pointer, value) in [
        ("/statistics/test/seed", json!(9)),
        (
            "/split/testWindow/toExclusiveMs",
            json!(p.body.split.test_window.to_exclusive_ms + 1),
        ),
        ("/selectionRule/maxCandidates", json!(2)),
        ("/data/interval", json!("1d")),
    ] {
        let mut body = original.clone();
        *body.pointer_mut(pointer).unwrap() = value;
        assert!(
            freeze_test_batch(&body.to_string(), &p, &s).is_err(),
            "{pointer}"
        );
    }
    let mut extra = original;
    extra["candidates"] = f["validation"]["body"]["candidates"].clone();
    assert!(freeze_test_batch(&extra.to_string(), &p, &s).is_err());
}

#[test]
fn empty_selection_is_retained_but_cannot_create_a_test_batch() {
    let f = fixture();
    let p = parent(&f);
    let mut r = report();
    // Whole-family m=16 changes adjusted numerators to 16/30/140/390.
    r["familyTests"] = json!(16);
    for (i, n) in [16, 30, 140, 390].into_iter().enumerate() {
        r["tests"][i]["adjustedP"]["numerator"] = json!(n);
        r["tests"][i]["adjustedPValue"] = json!(n as f64 / 800.0);
        r["tests"][i]["rejectsNull"] = json!(i == 0);
    }
    let s = select(&p, &r.to_string(), &[]).unwrap();
    assert!(s.selected_candidates().is_empty());
    assert!(freeze_test_batch(&test_body(&p, &s).to_string(), &p, &s).is_err());
    assert!(select(&p, &r.to_string(), &[0]).is_err());
}

#[test]
fn frozen_rule_requires_both_tests_and_applies_index_limit_without_hand_picking() {
    let f = fixture();
    let p = parent(&f);
    let raw = all_zero_report(&p, 8);
    let s = select(&p, &raw, &[0]).unwrap();
    assert_eq!(s.selected_candidates(), [0]);
    for wrong in [&[][..], &[2][..], &[0, 2][..]] {
        assert!(select(&p, &raw, wrong).is_err());
    }
    let mut r = report();
    r["tests"][1]["extremeCount"] = json!(4);
    r["tests"][1]["rawP"]["numerator"] = json!(5);
    r["tests"][1]["rawPValue"] = json!(5.0 / 800.0);
    r["tests"][1]["adjustedP"]["numerator"] = json!(35);
    r["tests"][1]["adjustedPValue"] = json!(35.0 / 800.0);
    r["tests"][1]["rejectsNull"] = json!(false);
    assert!(select(&p, &r.to_string(), &[]).is_ok());
    assert!(select(&p, &r.to_string(), &[0]).is_err());
}

#[test]
fn whole_family_holm_and_every_report_binding_are_recomputed_after_rehash() {
    let f = fixture();
    let p = parent(&f);
    assert!(select(&p, f["reportJson"].as_str().unwrap(), &[0]).is_ok());
    let mut batch_only = report();
    // A fully self-consistent four-test adjustment must still be refused when
    // the original report declares eight whole-family tests.
    for (i, n) in [4, 6, 20, 30].into_iter().enumerate() {
        batch_only["tests"][i]["adjustedP"]["numerator"] = json!(n);
        batch_only["tests"][i]["adjustedPValue"] = json!(n as f64 / 800.0);
        batch_only["tests"][i]["rejectsNull"] = json!(n <= 20);
    }
    assert!(select(&p, &batch_only.to_string(), &[0]).is_err());
    for (pointer, value) in [
        ("/familyTests", json!(2)),
        ("/familyTests", json!(7)),
        ("/familyTests", json!(16)),
        (
            "/contractVersion",
            json!("research-confirmation-statistics-v1"),
        ),
        ("/correction", json!("none")),
        ("/scheme", json!("iid")),
        ("/prng", json!("other")),
        ("/alphaPpm", json!(50000)),
        ("/blockLength", json!(5)),
        ("/bootstrapSamples", json!(800)),
        ("/seed", json!(8)),
        ("/tests/0/candidateIndex", json!(1)),
        ("/tests/0/test", json!("benchmark_excess")),
        ("/tests/0/observations", json!(255)),
        ("/tests/0/extremeCount", json!(800)),
        ("/tests/0/rawP/numerator", json!(2)),
        ("/tests/0/rawP/denominator", json!(799)),
        ("/tests/0/holmRank", json!(2)),
        ("/tests/0/adjustedP/numerator", json!(1)),
        ("/tests/0/adjustedP/denominator", json!(799)),
        ("/tests/0/rejectsNull", json!(false)),
        ("/tests/0/rawPValue", json!(-0.1)),
        ("/tests/0/adjustedPValue", json!(1.1)),
    ] {
        let mut r = report();
        *r.pointer_mut(pointer).unwrap() = value;
        assert!(select(&p, &r.to_string(), &[0]).is_err(), "{pointer}");
    }
    let mut r = report();
    r["tests"].as_array_mut().unwrap().pop();
    assert!(select(&p, &r.to_string(), &[0]).is_err());
    let mut r = report();
    r["tests"].as_array_mut().unwrap().swap(0, 1);
    assert!(select(&p, &r.to_string(), &[0]).is_err());
}

#[test]
fn tied_holm_ranks_keep_existing_candidate_then_test_order() {
    let f = fixture();
    let p = parent(&f);
    let mut r: Value = serde_json::from_str(&all_zero_report(&p, 8)).unwrap();
    r["tests"][0]["holmRank"] = json!(2);
    r["tests"][1]["holmRank"] = json!(1);
    assert!(select(&p, &r.to_string(), &[0]).is_err());
}

#[test]
fn selection_binds_the_original_report_bytes_without_rewriting_display_floats() {
    let f = fixture();
    let p = parent(&f);
    let original = f["reportJson"].as_str().unwrap();
    let long_decimal = original.replace(
        "\"observedMean\":0.1",
        "\"observedMean\":0.0036944444444444438",
    );
    let selection = select(&p, &long_decimal, &[0]).unwrap();
    assert_eq!(
        selection.document()["body"]["reportSha256"],
        json!(hex::encode(Sha256::digest(long_decimal.as_bytes())))
    );
    assert_ne!(selection.id(), f["selection"]["id"]);
    let reserve = reservation(&p);
    assert!(parse_selection(
        &selection.document().to_string(),
        &p,
        &SealedValidationInput {
            receipt_id: f["sealReceiptId"].as_str().unwrap(),
            reservation: &reserve,
            report_json: original
        }
    )
    .is_err());
}

#[test]
fn reservation_and_seal_cannot_be_substituted_into_selection() {
    let f = fixture();
    let p = parent(&f);
    let r = f["reportJson"].as_str().unwrap();
    for field in ["familyId", "allocationId", "confirmationBatchId"] {
        let mut b = f["alphaReservation"]["body"].clone();
        b[field] = json!(if field == "familyId" {
            format!("trial-family-v1:{}", "a".repeat(64))
        } else {
            "a".repeat(64)
        });
        let reservation = freeze_alpha_reservation(&b.to_string()).unwrap();
        let mut selection = f["selection"]["body"].clone();
        selection["alphaReservationId"] = json!(reservation.reservation_id());
        assert!(freeze_selection(
            &selection.to_string(),
            &p,
            &SealedValidationInput {
                receipt_id: f["sealReceiptId"].as_str().unwrap(),
                reservation: &reservation,
                report_json: r
            }
        )
        .is_err());
    }
    let reserve = reservation(&p);
    assert!(freeze_selection(
        &f["selection"]["body"].to_string(),
        &p,
        &SealedValidationInput {
            receipt_id: &"a".repeat(64),
            reservation: &reserve,
            report_json: r
        }
    )
    .is_err());
}

#[test]
fn original_origin_key_replay_is_distinct_from_transport_or_registry_reassignment() {
    let f = fixture();
    let p = parent(&f);
    let original = freeze_operation(&f["validationOperation"]["body"].to_string(), &p).unwrap();
    assert_eq!(
        operation_relation(&original, &original.clone()),
        OperationRelation::ExactReplay
    );
    for field in ["requestId", "originRegistryId", "reservationKey"] {
        let mut body = f["validationOperation"]["body"].clone();
        body[field] = json!("a".repeat(32));
        let changed = freeze_operation(&body.to_string(), &p).unwrap();
        assert_ne!(original.id(), changed.id());
        assert_eq!(
            operation_relation(&original, &changed),
            if field == "requestId" {
                OperationRelation::OriginKeyConflict
            } else {
                OperationRelation::Distinct
            }
        );
    }
    let mut new_body = p.document()["body"].clone();
    new_body["data"]["sourceEvidenceHash"] = json!("a".repeat(64));
    let newer = freeze_validation_batch(&new_body.to_string(), &source(&new_body)).unwrap();
    let mut op = f["validationOperation"]["body"].clone();
    op["confirmationBatchId"] = json!(newer.id());
    assert_eq!(
        operation_relation(
            &original,
            &freeze_operation(&op.to_string(), &newer).unwrap()
        ),
        OperationRelation::OriginKeyConflict
    );
}

#[test]
fn operation_must_bind_stage_selection_workspace_family_allocation_and_batch() {
    let f = fixture();
    let p = parent(&f);
    for (field, value) in [
        ("stage", json!("test")),
        ("selectionId", json!("a".repeat(64))),
        ("workspaceId", json!("a".repeat(32))),
        (
            "familyId",
            json!(format!("trial-family-v1:{}", "a".repeat(64))),
        ),
        ("allocationId", json!("a".repeat(64))),
        ("confirmationBatchId", json!("a".repeat(64))),
    ] {
        let mut body = f["validationOperation"]["body"].clone();
        body[field] = value;
        assert!(freeze_operation(&body.to_string(), &p).is_err(), "{field}");
    }
    let mut body = f["validationOperation"]["body"].clone();
    body.as_object_mut().unwrap().remove("selectionId");
    assert!(freeze_operation(&body.to_string(), &p).is_err());
}

#[test]
fn strict_json_rejects_duplicate_unknown_missing_integer_and_nullable_fields() {
    let f = fixture();
    let body = &f["validation"]["body"];
    let ctx = source(body);
    let raw = body.to_string();
    for (old, new) in [
        ("\"runId\":42", "\"runId\":42,\"runId\":42"),
        ("\"runId\":42", "\"runId\":42.0"),
        ("\"runId\":42", "\"runId\":4.2e1"),
        (
            "\"kind\":\"validation\"",
            "\"kind\":\"validation\",\"selectionId\":null",
        ),
        (
            "\"kind\":\"validation\"",
            "\"kind\":\"validation\",\"kind\":\"validation\"",
        ),
    ] {
        assert!(raw.contains(old));
        assert!(
            freeze_validation_batch(&raw.replacen(old, new, 1), &ctx).is_err(),
            "{new}"
        );
    }
    for pointer in [
        "/data/corporateActionsHash",
        "/statistics/test/seed",
        "/source/campaignId",
    ] {
        let mut value = body.clone();
        let (path, key) = pointer.rsplit_once('/').unwrap();
        value
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            freeze_validation_batch(&value.to_string(), &ctx).is_err(),
            "{pointer}"
        );
    }
    for pointer in [
        "",
        "/source",
        "/data",
        "/split",
        "/split/trainWindow",
        "/statistics",
        "/statistics/validation",
        "/selectionRule",
        "/candidates/0",
    ] {
        let mut value = body.clone();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!(true));
        assert!(
            freeze_validation_batch(&value.to_string(), &ctx).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn selection_operation_and_original_report_keep_strict_nested_wire_fields() {
    let f = fixture();
    let p = parent(&f);
    let reserve = reservation(&p);
    let input = SealedValidationInput {
        receipt_id: f["sealReceiptId"].as_str().unwrap(),
        reservation: &reserve,
        report_json: f["reportJson"].as_str().unwrap(),
    };
    let raw_selection = f["selection"]["body"].to_string();
    let raw_operation = f["validationOperation"]["body"].to_string();
    for raw in [&raw_selection, &raw_operation] {
        let field = if raw == &raw_selection {
            "selectedCandidates"
        } else {
            "selectionId"
        };
        let value: Value = serde_json::from_str(raw).unwrap();
        let literal = format!("\"{field}\":{}", value[field]);
        let duplicate = raw.replacen(&literal, &format!("{literal},{literal}"), 1);
        let unknown = raw.replacen('{', "{\"unexpected\":true,", 1);
        for malformed in [duplicate, unknown] {
            if raw == &raw_selection {
                assert!(freeze_selection(&malformed, &p, &input).is_err());
            } else {
                assert!(freeze_operation(&malformed, &p).is_err());
            }
        }
    }
    for pointer in ["", "/tests/0", "/tests/0/rawP", "/tests/0/adjustedP"] {
        let mut value = report();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!(true));
        // Rehash the original report before the attempted selection, so this
        // exercises nested parsing rather than only the outer hash binding.
        assert!(select(&p, &value.to_string(), &[0]).is_err(), "{pointer}");
    }
    let raw_report = input.report_json;
    for (old, new) in [
        ("\"seed\":7", "\"seed\":7,\"seed\":7"),
        ("\"seed\":7", "\"seed\":7.0"),
        (
            "\"extremeCount\":0",
            "\"extremeCount\":0,\"extremeCount\":0",
        ),
        ("\"numerator\":1", "\"numerator\":1,\"numerator\":1"),
    ] {
        assert!(raw_report.contains(old));
        assert!(select(&p, &raw_report.replacen(old, new, 1), &[0]).is_err());
    }
}

#[test]
fn every_stored_record_recomputes_its_id_and_has_a_closed_wrapper() {
    let f = fixture();
    let p = parent(&f);
    let reserve = reservation(&p);
    let input = SealedValidationInput {
        receipt_id: f["sealReceiptId"].as_str().unwrap(),
        reservation: &reserve,
        report_json: f["reportJson"].as_str().unwrap(),
    };
    for field in ["id", "unexpected"] {
        let mut b = f["validation"].clone();
        b[field] = json!("a".repeat(64));
        assert!(parse_validation_batch(&b.to_string(), &source(&b["body"])).is_err());
        let mut s = f["selection"].clone();
        s[field] = json!("a".repeat(64));
        assert!(parse_selection(&s.to_string(), &p, &input).is_err());
        let mut o = f["validationOperation"].clone();
        o[field] = json!("a".repeat(64));
        assert!(parse_operation(&o.to_string(), &p).is_err());
    }
    let mut b = f["validation"].clone();
    b["body"]["statistics"]["blockLengthRationale"] = json!("Another frozen rationale");
    assert!(parse_validation_batch(&b.to_string(), &source(&b["body"]))
        .unwrap_err()
        .0
        .contains("ID"));
}

#[test]
fn split_versions_support_and_algorithm_domain_are_checked_before_freezing() {
    let original = fixture()["validation"]["body"].clone();
    for (pointer, value) in [
        ("/contractVersion", json!("v0")),
        ("/split/contractVersion", json!("v0")),
        (
            "/statistics/contractVersion",
            json!("research-confirmation-statistics-v1"),
        ),
        ("/selectionRule/version", json!("manual")),
        ("/selectionRule/maxCandidates", json!(0)),
        ("/selectionRule/maxCandidates", json!(3)),
        ("/statistics/bootstrapSamples", json!(800)),
        ("/statistics/maxRelativeStandardErrorPpm", json!(0)),
        ("/statistics/validation/blockLength", json!(5)),
        ("/statistics/test/blockLength", json!(5)),
        ("/statistics/test/seed", json!(PRECISION_MAX_COUNT + 1)),
        ("/statistics/blockLengthRationale", json!(" ")),
        ("/split/totalBars", json!(4)),
        ("/split/embargoBars", json!(4)),
        ("/data/interval", json!("2years")),
        ("/data/calendarId", json!(" invalid ")),
    ] {
        let mut b = original.clone();
        *b.pointer_mut(pointer).unwrap() = value;
        assert!(
            freeze_validation_batch(&b.to_string(), &source(&b)).is_err(),
            "{pointer}"
        );
    }
    let mut b = original.clone();
    b["split"]["validationSupport"] = json!([b["split"]["validationWindow"].clone()]);
    b["split"]["validationSupport"][0]["fromMs"] =
        json!(b["split"]["validationWindow"]["fromMs"].as_u64().unwrap() + 1);
    assert!(freeze_validation_batch(&b.to_string(), &source(&b)).is_err());
    let mut b = original.clone();
    b["split"]["testSupport"][0]["fromMs"] = b["split"]["validationWindow"]["fromMs"].clone();
    assert!(freeze_validation_batch(&b.to_string(), &source(&b)).is_err());
    let mut b = original;
    b["split"]["testWindow"] = b["split"]["validationWindow"].clone();
    assert!(freeze_validation_batch(&b.to_string(), &source(&b)).is_err());
}

#[test]
fn canonical_instrument_family_and_etf_corporate_action_evidence_are_required() {
    for bad in [
        "crypto:Binance:BTCUSDT",
        "crypto:binance:BTC USDT",
        "fx:oanda:EURUSD",
        &format!("crypto:binance:{}", "A".repeat(256)),
    ] {
        assert!(confirmation_family_id(bad).is_err());
    }
    let mut b = fixture()["validation"]["body"].clone();
    b["source"]["familyId"] = json!(format!("trial-family-v1:{}", "a".repeat(64)));
    assert!(freeze_validation_batch(&b.to_string(), &source(&b)).is_err());
    b["source"]["instrumentId"] = json!("tw-etf:twse:0050");
    b["source"]["familyId"] = json!(confirmation_family_id("tw-etf:twse:0050").unwrap());
    assert!(freeze_validation_batch(&b.to_string(), &source(&b)).is_err());
    b["data"]["corporateActionsHash"] = json!("a".repeat(64));
    assert!(freeze_validation_batch(&b.to_string(), &source(&b)).is_ok());
}

#[test]
fn each_original_artifact_reference_changes_identity_without_rehashing_its_contents() {
    let original = fixture()["validation"]["body"].clone();
    let p = parent(&fixture());
    for pointer in [
        "/source/campaignId",
        "/source/campaignDocumentHash",
        "/source/trialBatchId",
        "/source/engineManifestHash",
        "/data/snapshotId",
        "/data/instrumentRevisionHash",
        "/data/calendarHash",
        "/data/costProfileHash",
        "/data/numericCostsHash",
        "/data/sourceEvidenceHash",
        "/candidates/0/definitionHash",
        "/candidates/0/executionHash",
        "/candidates/0/resultArtifactHash",
        "/candidates/0/lineageHash",
        "/statistics/allocationId",
    ] {
        let mut b = original.clone();
        *b.pointer_mut(pointer).unwrap() = json!("a".repeat(64));
        let changed = freeze_validation_batch(&b.to_string(), &source(&b)).unwrap();
        assert_ne!(changed.id(), p.id(), "{pointer}");
    }
}

#[test]
fn overlap_uses_only_canonical_instrument_and_half_open_support() {
    let min = MIN_MARKET_TIMESTAMP_MS as u64;
    let make = |id: &str, ranges: &[(u64, u64)]| {
        ExposureFootprint::new(
            id,
            ranges
                .iter()
                .map(|&(a, b)| TimeInterval::new(min + a, min + b).unwrap())
                .collect(),
        )
        .unwrap()
    };
    let a = make("crypto:binance:BTCUSDT", &[(0, 10), (20, 30)]);
    for (ranges, wanted) in [
        (vec![(10, 20)], false),
        (vec![(9, 11)], true),
        (vec![(2, 3)], true),
        (vec![(0, 40)], true),
        (vec![(15, 20), (30, 35)], false),
        (vec![(15, 21)], true),
    ] {
        let b = make("crypto:binance:BTCUSDT", &ranges);
        assert_eq!(footprints_overlap(&a, &b), wanted);
        assert_eq!(footprints_overlap(&b, &a), wanted);
    }
    assert!(!footprints_overlap(
        &a,
        &make("crypto:coinbase:BTCUSDT", &[(0, 10)])
    ));
    let mut body = fixture()["validation"]["body"].clone();
    body["data"]["datasetHash"] = json!(format!("dataset-content-v2:{}", "a".repeat(64)));
    body["data"]["sourceEvidenceHash"] = json!("b".repeat(64));
    body["data"]["interval"] = json!("1d");
    let changed = freeze_validation_batch(&body.to_string(), &source(&body)).unwrap();
    assert!(footprints_overlap(
        &parent(&fixture()).footprint(),
        &changed.footprint()
    ));
}

#[test]
fn overlap_matches_an_independent_finite_set_intersection_oracle() {
    use std::collections::BTreeSet;
    let min = MIN_MARKET_TIMESTAMP_MS as u64;
    let id = "crypto:binance:BTCUSDT";
    let mut shapes = Vec::new();
    // Every nonempty subset of eight integer milliseconds, grouped into runs.
    for mask in 1..256u16 {
        let points: BTreeSet<u64> = (0..8).filter(|i| mask & (1 << i) != 0).collect();
        let mut intervals = Vec::new();
        let mut start = None;
        for i in 0..=8 {
            if points.contains(&i) {
                if start.is_none() {
                    start = Some(i);
                }
            } else if let Some(a) = start.take() {
                intervals.push(TimeInterval::new(min + a, min + i).unwrap());
            }
        }
        shapes.push((points, ExposureFootprint::new(id, intervals).unwrap()));
    }
    for (a, af) in &shapes {
        for (b, bf) in &shapes {
            assert_eq!(footprints_overlap(af, bf), !a.is_disjoint(b));
        }
    }
}

#[test]
fn interval_count_canonical_order_and_time_endpoints_are_bounded() {
    let min = MIN_MARKET_TIMESTAMP_MS as u64;
    let max = MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE as u64;
    let id = "crypto:binance:BTCUSDT";
    assert!(TimeInterval::new(max - 1, max).is_ok());
    for (a, b) in [(min - 1, min), (max, max + 1), (min, min), (min + 1, min)] {
        assert!(TimeInterval::new(a, b).is_err());
    }
    assert!(ExposureFootprint::new(id, vec![]).is_err());
    for ranges in [
        vec![(0, 2), (2, 4)],
        vec![(2, 4), (0, 1)],
        vec![(0, 3), (2, 4)],
    ] {
        assert!(ExposureFootprint::new(
            id,
            ranges
                .into_iter()
                .map(|(a, b)| TimeInterval::new(min + a, min + b).unwrap())
                .collect()
        )
        .is_err());
    }
    let ranges: Vec<_> = (0..128)
        .map(|i| TimeInterval::new(min + i * 2, min + i * 2 + 1).unwrap())
        .collect();
    assert!(ExposureFootprint::new(id, ranges.clone()).is_ok());
    let mut excess = ranges;
    excess.push(TimeInterval::new(min + 256, min + 257).unwrap());
    assert!(ExposureFootprint::new(id, excess).is_err());
    let untrusted: TimeInterval =
        serde_json::from_str("{\"fromMs\":0,\"toExclusiveMs\":1}").unwrap();
    assert!(ExposureFootprint::new(id, vec![untrusted]).is_err());
}

#[test]
fn raw_byte_caps_apply_before_parsing_to_bodies_records_and_reports() {
    let f = fixture();
    let p = parent(&f);
    let b = f["validation"]["body"].to_string();
    let ctx = source(&f["validation"]["body"]);
    let padded = format!("{b}{}", " ".repeat(MAX_DOCUMENT_BYTES - b.len()));
    assert!(freeze_validation_batch(&padded, &ctx).is_ok());
    let oversized = format!("{padded} ");
    assert!(freeze_validation_batch(&oversized, &ctx)
        .unwrap_err()
        .0
        .contains("4 MiB"));
    assert!(
        parse_validation_batch(&" ".repeat(MAX_DOCUMENT_BYTES + 1), &ctx)
            .unwrap_err()
            .0
            .contains("4 MiB")
    );
    assert!(freeze_operation(&oversized, &p)
        .unwrap_err()
        .0
        .contains("4 MiB"));
    assert!(parse_operation(&oversized, &p)
        .unwrap_err()
        .0
        .contains("4 MiB"));
    let raw = f["reportJson"].as_str().unwrap();
    let at_limit = format!("{raw}{}", " ".repeat(MAX_DOCUMENT_BYTES - raw.len()));
    assert!(select(&p, &at_limit, &[0]).is_ok());
    assert!(select(&p, &format!("{at_limit} "), &[0])
        .unwrap_err()
        .0
        .contains("4 MiB"));
}

#[test]
fn exact_candidate_and_report_capacity_roundtrips_and_one_over_is_refused() {
    let mut b = fixture()["validation"]["body"].clone();
    let candidates: Vec<Value> = (0..MAX_CANDIDATES)
        .map(|i| {
            let mut c = b["candidates"][0].clone();
            c["candidateIndex"] = json!(i);
            c
        })
        .collect();
    b["candidates"] = json!(candidates);
    b["selectionRule"]["maxCandidates"] = json!(MAX_CANDIDATES);
    b["statistics"]["bootstrapSamples"] = json!(655359);
    b["statistics"]["maxBootstrapSamples"] = json!(655359);
    let ctx = source(&b);
    let p = freeze_validation_batch(&b.to_string(), &ctx).unwrap();
    assert!(p.document().to_string().len() < MAX_DOCUMENT_BYTES);
    let raw = all_zero_report(&p, 8192);
    let indexes: Vec<u64> = (0..4096).collect();
    let selection = select(&p, &raw, &indexes).unwrap();
    assert_eq!(selection.selected_candidates().len(), 4096);
    let t = freeze_test_batch(&test_body(&p, &selection).to_string(), &p, &selection).unwrap();
    assert_eq!(t.candidates().len(), 4096);
    assert_eq!(
        parse_test_batch(&t.document().to_string(), &p, &selection).unwrap(),
        t
    );
    let mut extra = b["candidates"][0].clone();
    extra["candidateIndex"] = json!(4096);
    b["candidates"].as_array_mut().unwrap().push(extra);
    assert!(freeze_validation_batch(&b.to_string(), &source(&b))
        .unwrap_err()
        .0
        .contains("4096"));
}

#[test]
fn safe_integer_and_per_text_field_bounds_are_enforced() {
    let original = fixture()["validation"]["body"].clone();
    for (pointer, value) in [
        ("/source/runId", json!(0)),
        ("/source/runId", json!(PRECISION_MAX_COUNT + 1)),
        (
            "/candidates/1/candidateIndex",
            json!(PRECISION_MAX_COUNT + 1),
        ),
        ("/statistics/blockLengthRationale", json!("x".repeat(1025))),
        ("/data/calendarId", json!("a".repeat(129))),
        ("/source/workspaceId", json!("A".repeat(32))),
        ("/candidates/0/strategyHash", json!("a".repeat(64))),
    ] {
        let mut b = original.clone();
        *b.pointer_mut(pointer).unwrap() = value;
        assert!(
            freeze_validation_batch(&b.to_string(), &source(&b)).is_err(),
            "{pointer}"
        );
    }
    let mut b = original;
    b["source"]["runId"] = json!(PRECISION_MAX_COUNT);
    b["candidates"][1]["candidateIndex"] = json!(PRECISION_MAX_COUNT);
    b["statistics"]["blockLengthRationale"] = json!("x".repeat(1024));
    b["data"]["calendarId"] = json!("a".repeat(128));
    assert!(freeze_validation_batch(&b.to_string(), &source(&b)).is_ok());
}
