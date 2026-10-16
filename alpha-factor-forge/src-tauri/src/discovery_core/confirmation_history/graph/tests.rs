use super::*;
use crate::discovery_core::alpha_reservations::parse_alpha_reservation;
use serde_json::json;
mod capacity;
mod overlap;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../fixtures/rs-core/research-confirmation-graph-v1.json"
    ))
    .unwrap()
}

#[test]
fn admission_original_nested_json_is_closed_and_static_after_routing() {
    let (i, c) = val_only();
    for path in [
        "workspaceObservation",
        "registryObservation",
        "precisionPlan",
        "precisionProof",
    ] {
        let mut x = i.clone();
        change_admission(&mut x, |a| {
            a[path]["unknownSecret"] = json!("PROTECTED_SENTINEL")
        });
        let err = complete_confirmation_graph(&x, &c).unwrap_err();
        assert!(!err.to_string().contains("PROTECTED"));
    }
    let mut x = i.clone();
    x.admissions[0] =
        x.admissions[0].replace("\"familyTests\":8", "\"familyTests\":8,\"familyTests\":8");
    assert_ne!(x.admissions, i.admissions);
    assert_bad(&x, &c);
    let mut x = i.clone();
    let mut a = doc(&x.admissions[0]);
    a["body"]["precisionProof"]
        .as_object_mut()
        .unwrap()
        .remove("requiredSamplesForPrecision");
    rehash(&mut a);
    x.admissions[0] = a.to_string();
    assert_bad(&x, &c);
}
fn setup() -> (ConfirmationInventory, GraphContext) {
    let f = fixture();
    let r = &f["records"];
    let mut i = ConfirmationInventory::default();
    for (names, section) in [
        (vec!["validation", "test"], &mut i.batches),
        (vec!["selection"], &mut i.selections),
        (
            vec!["validationOperation", "testOperation"],
            &mut i.operations,
        ),
        (vec!["validationExposure", "testExposure"], &mut i.exposures),
        (
            vec!["validationAdmission", "testAdmission"],
            &mut i.admissions,
        ),
        (
            vec!["validationStart", "validationSeal", "testStart", "testSeal"],
            &mut i.events,
        ),
    ] {
        for name in names {
            section.push(r[name].to_string());
        }
    }
    for b in f["blobs"].as_array().unwrap() {
        let kind = if b["kind"] == "binding" {
            BlobKind::Binding
        } else {
            BlobKind::Statistics
        };
        let blob = HistoryBlob::new(kind, b["text"].as_str().unwrap()).unwrap();
        assert_eq!(blob.sha256(), b["sha256"].as_str().unwrap());
        i.blobs.push(blob);
    }
    let s = &f["registeredSource"];
    let t = &f["trial"];
    let c = GraphContext {
        registered_sources: vec![RegisteredSource {
            source: serde_json::from_value(s["source"].clone()).unwrap(),
            candidates: serde_json::from_value(s["candidates"].clone()).unwrap(),
        }],
        allocations: vec![HostAllocation {
            family_id: t["familyId"].as_str().unwrap().into(),
            declaration: parse_alpha_allocation(&f["allocationDeclaration"]).unwrap(),
        }],
        reservations: ["validation", "test"]
            .iter()
            .map(|k| parse_alpha_reservation(&f["reservations"][k].to_string()).unwrap())
            .collect(),
        historical_prefixes: vec![HistoricalPrefix {
            registry_id: t["registryId"].as_str().unwrap().into(),
            seq: 0,
            chain_head: t["chainHead"].as_str().unwrap().into(),
        }],
        historical_trials: vec![HistoricalTrialObservation {
            registry_id: t["registryId"].as_str().unwrap().into(),
            seq: 0,
            chain_head: t["chainHead"].as_str().unwrap().into(),
            trial_batch_id: t["trialBatchId"].as_str().unwrap().into(),
            family_id: t["familyId"].as_str().unwrap().into(),
            family_effective_trials: 4,
            registered_batch_effective_trials: 2,
            tests_per_trial: 2,
            complete_candidates: 2,
        }],
        family_tests: BTreeMap::from([(t["familyId"].as_str().unwrap().into(), 2)]),
        ..GraphContext::default()
    };
    (i, c)
}
fn doc(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap()
}
fn rehash(v: &mut Value) {
    v["id"] = json!(identity(v["body"]["contractVersion"].as_str().unwrap(), &v["body"]).unwrap());
}
fn val_only() -> (ConfirmationInventory, GraphContext) {
    let (mut i, c) = setup();
    i.batches.truncate(1);
    i.selections.clear();
    i.operations.truncate(1);
    i.exposures.truncate(1);
    i.admissions.truncate(1);
    i.events.truncate(2);
    i.blobs.pop();
    (i, c)
}
fn assert_bad(i: &ConfirmationInventory, c: &GraphContext) {
    assert!(complete_confirmation_graph(i, c).is_err());
}
fn change_admission(i: &mut ConfirmationInventory, mutate: impl FnOnce(&mut Value)) {
    let mut a = doc(&i.admissions[0]);
    let old = a["id"].as_str().unwrap().to_string();
    mutate(&mut a["body"]);
    rehash(&mut a);
    let new = a["id"].as_str().unwrap();
    i.admissions[0] = a.to_string();
    let mut old_start = String::new();
    let mut new_start = String::new();
    for raw in &mut i.events {
        let mut v = doc(raw);
        if v["body"]["admissionId"] == old {
            v["body"]["admissionId"] = json!(new);
            if v["body"]["event"]["kind"] == "execution_started" {
                old_start = v["id"].as_str().unwrap().into();
                rehash(&mut v);
                new_start = v["id"].as_str().unwrap().into();
            } else {
                if v["body"]["event"]["executionStartedId"] == old_start {
                    v["body"]["event"]["executionStartedId"] = json!(new_start);
                }
                rehash(&mut v);
            }
            *raw = v.to_string();
        }
    }
}
fn change_report(i: &mut ConfirmationInventory, mutate: impl FnOnce(&mut Value)) {
    let index = i
        .blobs
        .iter()
        .position(|b| b.kind() == BlobKind::Statistics)
        .unwrap();
    let old = i.blobs[index].sha256().to_string();
    let mut r = doc(i.blobs[index].text());
    mutate(&mut r);
    let blob = HistoryBlob::new(BlobKind::Statistics, &r.to_string()).unwrap();
    for raw in &mut i.events {
        let mut v = doc(raw);
        if v["body"]["event"]["reportSha256"] == old {
            v["body"]["event"]["reportSha256"] = json!(blob.sha256());
            rehash(&mut v);
            *raw = v.to_string();
        }
    }
    i.blobs[index] = blob;
}
fn manual(
    i: &mut ConfirmationInventory,
    keychar: char,
    from: u64,
    to: u64,
    seal: Option<String>,
) -> String {
    let body = json!({"contractVersion":EXPOSURE_VERSION,"originRegistryId":"2".repeat(32),"reservationKey":keychar.to_string().repeat(32),
        "instrumentId":"crypto:binance:BTCUSDT","intervals":[{"fromMs":from,"toExclusiveMs":to}],"claim":{"kind":"manual","accessKind":"raw","afterSealId":seal}});
    let r = freeze_manual_exposure(&body.to_string()).unwrap();
    i.exposures.push(r.document().to_string());
    r.id().into()
}
#[test]
fn independent_full_chain_roundtrips_both_stages_and_preserves_original_bytes() {
    let (i, c) = setup();
    let checked = validate_confirmation_graph(&i, &c).unwrap();
    assert!(!checked.is_restricted());
    assert_eq!(checked.inventory().admissions.len(), 2);
    assert_eq!(checked.inventory().blobs.len(), 3);
    assert!(checked
        .inventory()
        .blobs
        .iter()
        .any(|b| b.text().contains("1e-1")));
    assert_eq!(
        complete_confirmation_graph(checked.inventory(), &c)
            .unwrap()
            .inventory(),
        checked.inventory()
    );
    let mut reversed = i.clone();
    for v in [
        &mut reversed.batches,
        &mut reversed.events,
        &mut reversed.admissions,
    ] {
        v.reverse();
    }
    reversed.blobs.reverse();
    assert_eq!(
        validate_confirmation_graph(&reversed, &c)
            .unwrap()
            .inventory(),
        checked.inventory()
    );
}
#[test]
fn missing_wrong_kind_duplicate_and_orphan_records_are_refused() {
    let (i, c) = setup();
    for section in 0..8 {
        let mut x = i.clone();
        match section {
            0 => {
                x.batches.pop();
            }
            1 => {
                x.selections.pop();
            }
            2 => {
                x.operations.pop();
            }
            3 => {
                x.exposures.pop();
            }
            4 => {
                x.admissions.pop();
            }
            5 => {
                x.events.pop();
            }
            6 => {
                x.blobs.pop();
            }
            _ => {
                x.events[1] = x.exposures[0].clone();
            }
        }
        assert_bad(&x, &c);
    }
    let mut x = i.clone();
    x.events.push(x.events[0].clone());
    assert_bad(&x, &c);
    let (mut x, c) = val_only();
    x.events.clear();
    x.admissions.clear();
    assert_bad(&x, &c);
    let (mut x, c) = val_only();
    x.blobs.push(
        HistoryBlob::new(
            BlobKind::Statistics,
            fixture()["blobs"][2]["text"].as_str().unwrap(),
        )
        .unwrap(),
    );
    assert_bad(&x, &c);
}
#[test]
fn every_plan_and_proof_field_is_recomputed_after_valid_rehash() {
    for (object, field, value) in [
        ("precisionPlan", "contractVersion", json!("next")),
        ("precisionPlan", "correction", json!("bonferroni")),
        ("precisionPlan", "alphaPpm", json!(50000)),
        ("precisionPlan", "priorTrials", json!(1)),
        ("precisionPlan", "plannedTrials", json!(1)),
        ("precisionPlan", "testsPerTrial", json!(1)),
        ("precisionPlan", "bootstrapSamples", json!(800)),
        ("precisionPlan", "maxBootstrapSamples", json!(800)),
        (
            "precisionPlan",
            "maxRelativeStandardErrorPpm",
            json!(999998),
        ),
        ("precisionProof", "contractVersion", json!("next")),
        ("precisionProof", "status", json!("NOT_ELIGIBLE")),
        (
            "precisionProof",
            "reasons",
            json!(["bootstrap_resolution_insufficient"]),
        ),
        ("precisionProof", "familyTrials", json!(3)),
        ("precisionProof", "familyTests", json!(4)),
        (
            "precisionProof",
            "minAttainableRawP",
            json!({"numerator":2,"denominator":800}),
        ),
        (
            "precisionProof",
            "bestAdjustedP",
            json!({"numerator":7,"denominator":800}),
        ),
        (
            "precisionProof",
            "requiredSamplesForResolution",
            Value::Null,
        ),
        ("precisionProof", "requiredSamplesForPrecision", json!(319)),
    ] {
        let (mut i, c) = val_only();
        change_admission(&mut i, |v| v[object][field] = value);
        assert_bad(&i, &c);
    }
}
#[test]
fn admission_requires_exact_original_counts_share_prefix_source_and_workspace() {
    for (object, field, value) in [
        ("registryObservation", "registryId", json!("3".repeat(32))),
        ("registryObservation", "seq", json!(1)),
        ("registryObservation", "chainHead", json!("a".repeat(64))),
        (
            "registryObservation",
            "protocolSha256",
            json!("b".repeat(64)),
        ),
        ("registryObservation", "familyEffectiveTrials", json!(2)),
        (
            "registryObservation",
            "registeredBatchEffectiveTrials",
            json!(1),
        ),
        ("registryObservation", "testsPerTrial", json!(1)),
        ("workspaceObservation", "workspaceId", json!("3".repeat(32))),
        ("workspaceObservation", "ownershipEpoch", json!(0)),
        (
            "workspaceObservation",
            "mutationSeq",
            json!(9007199254740992u64),
        ),
    ] {
        let (mut i, c) = val_only();
        change_admission(&mut i, |v| v[object][field] = value);
        assert_bad(&i, &c);
    }
    let (i, mut c) = setup();
    c.historical_trials[0].registered_batch_effective_trials = 1;
    assert_bad(&i, &c);
    let (i, mut c) = setup();
    c.historical_trials[0].complete_candidates = 1;
    assert_bad(&i, &c);
    let (i, mut c) = setup();
    c.registered_sources[0].candidates.pop();
    assert_bad(&i, &c);
    let (i, mut c) = setup();
    c.historical_prefixes.clear();
    assert_bad(&i, &c);
    let (i, mut c) = setup();
    c.registered_sources.push(c.registered_sources[0].clone());
    assert_bad(&i, &c);
    let (i, mut c) = setup();
    c.historical_trials.push(c.historical_trials[0].clone());
    assert_bad(&i, &c);
    let (i, mut c) = setup();
    c.allocations[0].declaration.schedule = vec![20000, 30000];
    assert_bad(&i, &c);
}
#[test]
fn test_subset_keeps_whole_family_m_and_original_batch_count() {
    let (mut i, c) = setup();
    assert_eq!(
        doc(&i.batches[1])["body"]["candidates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut t = doc(&i.admissions[1]);
    t["body"]["precisionPlan"]["plannedTrials"] = json!(1);
    rehash(&mut t);
    i.admissions[1] = t.to_string();
    assert_bad(&i, &c);
    let (mut i, c) = setup();
    let idx = i
        .blobs
        .iter()
        .position(|b| b.kind() == BlobKind::Statistics && doc(b.text())["seed"] == 8)
        .unwrap();
    let old = i.blobs[idx].sha256().to_string();
    let mut report = doc(i.blobs[idx].text());
    report["familyTests"] = json!(4);
    // Recompute valid Holm for the smaller m: report consistency alone cannot
    // replace the admission's m=8, even though Test only has one candidate.
    for (j, n) in [4, 6].into_iter().enumerate() {
        report["tests"][j]["adjustedP"]["numerator"] = json!(n);
    }
    let blob = HistoryBlob::new(BlobKind::Statistics, &report.to_string()).unwrap();
    for raw in &mut i.events {
        let mut v = doc(raw);
        if v["body"]["event"]["reportSha256"] == old {
            v["body"]["event"]["reportSha256"] = json!(blob.sha256());
            rehash(&mut v);
            *raw = v.to_string();
        }
    }
    i.blobs[idx] = blob;
    assert_bad(&i, &c);
}
#[test]
fn report_settings_counts_decisions_and_seal_engine_are_checked_without_bootstrap() {
    for (field, value) in [
        ("seed", json!(8)),
        ("familyTests", json!(10)),
        ("alphaPpm", json!(50000)),
        ("blockLength", json!(8)),
        ("bootstrapSamples", json!(800)),
    ] {
        let (mut i, c) = val_only();
        change_report(&mut i, |r| r[field] = value);
        assert_bad(&i, &c);
    }
    for (field, value) in [
        ("rejectsNull", json!(false)),
        ("observations", json!(255)),
        ("candidateIndex", json!(1)),
        ("holmRank", json!(2)),
        ("rawP", json!({"numerator":2,"denominator":800})),
        ("adjustedP", json!({"numerator":7,"denominator":800})),
    ] {
        let (mut i, c) = val_only();
        change_report(&mut i, |r| r["tests"][0][field] = value);
        assert_bad(&i, &c);
    }
    for field in ["executionStartedId", "engineManifestHash"] {
        let (mut i, c) = val_only();
        let mut e = doc(&i.events[1]);
        e["body"]["event"][field] = json!("f".repeat(64));
        rehash(&mut e);
        i.events[1] = e.to_string();
        assert_bad(&i, &c);
    }
}
fn change_binding(i: &mut ConfirmationInventory, mutate: impl FnOnce(&mut Value)) {
    let idx = i
        .blobs
        .iter()
        .position(|b| b.kind() == BlobKind::Binding)
        .unwrap();
    let mut b = doc(i.blobs[idx].text());
    mutate(&mut b);
    let blob = HistoryBlob::new(BlobKind::Binding, &b.to_string()).unwrap();
    change_admission(i, |a| {
        a["registryObservation"]["bindingSha256"] = json!(blob.sha256())
    });
    i.blobs[idx] = blob;
}
#[test]
fn pre_spend_binding_requires_covered_typed_history_and_restrictive_evidence() {
    for (section, field, value) in [
        ("confirmation", "events", json!(["f".repeat(64)])),
        ("alpha", "reservations", json!(["f".repeat(64)])),
        ("alpha", "budgets", json!([])),
        (
            "evidence",
            "quarantinedFamilies",
            json!([fixture()["trial"]["familyId"]]),
        ),
        ("evidence", "conflictedOrigins", json!(["2".repeat(32)])),
        ("confirmation", "unknownOrigins", json!(["3".repeat(32)])),
    ] {
        let (mut i, c) = val_only();
        change_binding(&mut i, |b| b[section][field] = value);
        assert_bad(&i, &c);
    }
    let (mut i, c) = val_only();
    let exposure = doc(&i.exposures[0])["id"].clone();
    change_binding(&mut i, |b| {
        b["confirmation"]["exposures"] = json!([exposure])
    });
    assert_bad(&i, &c);
    let (mut i, c) = val_only();
    let alpha = c.reservations[0].reservation_id().to_string();
    change_binding(&mut i, |b| b["alpha"]["reservations"] = json!([alpha]));
    assert_bad(&i, &c);
    let (mut i, c) = val_only();
    let op = doc(&i.operations[0])["id"].clone();
    change_binding(&mut i, |b| b["confirmation"]["operations"] = json!([op]));
    validate_confirmation_graph(&i, &c).unwrap();
}

#[test]
fn coverage_only_blob_reference_does_not_launder_an_orphan_report() {
    let (mut i, c) = val_only();
    let raw = fixture()["blobs"][2]["text"].as_str().unwrap().to_string();
    let blob = HistoryBlob::new(BlobKind::Statistics, &raw).unwrap();
    let orphan = blob.sha256().to_string();
    i.blobs.push(blob);
    change_binding(&mut i, |b| b["confirmation"]["blobs"] = json!([orphan]));
    assert_bad(&i, &c);
}
#[test]
fn original_blob_duplicate_dynamic_keys_closed_fields_and_exact_raw_caps() {
    let f = fixture();
    let b = f["blobs"][0]["text"].as_str().unwrap();
    let family = f["trial"]["familyId"].as_str().unwrap();
    let duplicate = b.replace(
        &format!("\"{family}\": 2"),
        &format!("\"{family}\": 2,\"{family}\": 2"),
    );
    assert_eq!(
        HistoryBlob::new(BlobKind::Binding, &duplicate).unwrap_err(),
        HistoryRecordError::Blob
    );
    assert!(HistoryBlob::new(
        BlobKind::Binding,
        &b.replacen("\"seq\": 0", "\"seq\": 0,\"seq\": 0", 1)
    )
    .is_err());
    let mut value = doc(b);
    value["confirmation"]["unexpected"] = json!("PRIVATE_PROTECTED_SENTINEL");
    let err = HistoryBlob::new(BlobKind::Binding, &value.to_string()).unwrap_err();
    assert!(!err.to_string().contains("PRIVATE"));
    for (kind, raw, cap) in [
        (BlobKind::Binding, b, admission::MAX_BINDING_BYTES),
        (
            BlobKind::Statistics,
            f["blobs"][1]["text"].as_str().unwrap(),
            MAX_DOCUMENT_BYTES,
        ),
    ] {
        let mut text = raw.to_string();
        text.extend(std::iter::repeat_n(' ', cap - raw.len()));
        assert_eq!(HistoryBlob::new(kind, &text).unwrap().text().len(), cap);
        text.push(' ');
        assert_eq!(
            HistoryBlob::new(kind, &text).unwrap_err(),
            HistoryRecordError::TooLarge
        );
    }
}
#[test]
fn iterative_graph_refuses_cycles_without_recursive_stack_growth() {
    let mut nodes = BTreeMap::new();
    for j in 0..10000 {
        let id = format!("{j:064x}");
        let deps = if j == 0 {
            BTreeSet::new()
        } else {
            BTreeSet::from([key(Kind::Event, &format!("{:064x}", j - 1))])
        };
        nodes.insert(
            key(Kind::Event, &id),
            Node {
                raw: String::new(),
                body: Value::Null,
                deps,
            },
        );
    }
    assert_eq!(topological(&nodes).unwrap().len(), 10000);
    nodes
        .get_mut(&key(Kind::Event, &format!("{:064x}", 0)))
        .unwrap()
        .deps
        .insert(key(Kind::Event, &format!("{:064x}", 9999)));
    assert_eq!(topological(&nodes).unwrap_err(), HistoryRecordError::Cycle);
}
#[test]
fn manual_after_seal_is_exempt_only_for_exact_containing_exposure() {
    let (mut i, c) = val_only();
    let e = doc(&i.exposures[0]);
    let start = e["body"]["intervals"][0]["fromMs"].as_u64().unwrap();
    let seal = doc(&i.events[1])["id"].as_str().unwrap().into();
    manual(&mut i, '7', start, start + 100, Some(seal));
    assert!(!complete_confirmation_graph(&i, &c).unwrap().is_restricted());
    let mut m = doc(i.exposures.last().unwrap());
    m["body"]["claim"]["afterSealId"] = Value::Null;
    rehash(&mut m);
    *i.exposures.last_mut().unwrap() = m.to_string();
    assert_eq!(
        validate_confirmation_graph(&i, &c).unwrap_err(),
        HistoryRecordError::Conflict
    );
    let completed = complete_confirmation_graph(&i, &c).unwrap();
    assert!(completed.is_restricted());
    assert_eq!(completed.inventory().conflicts.len(), 1);
    validate_confirmation_graph(completed.inventory(), &c).unwrap();
    let (mut i, c) = val_only();
    let start = 946684800000;
    let seal = doc(&i.events[1])["id"].as_str().unwrap().into();
    manual(&mut i, '7', start, start + 100, Some(seal));
    assert_bad(&i, &c);
}
#[test]
fn contradictory_starts_seals_admissions_and_terminals_are_retained_with_witnesses() {
    let (mut i, c) = val_only();
    let mut start = doc(&i.events[0]);
    start["body"]["event"]["executionKey"] = json!("c".repeat(32));
    rehash(&mut start);
    i.events.push(start.to_string());
    let mut seal = doc(&i.events[1]);
    seal["body"]["event"]["resultArtifactSha256"] = json!("d".repeat(64));
    rehash(&mut seal);
    i.events.push(seal.to_string());
    let a = doc(&i.admissions[0]);
    let mut terminal = json!({"contractVersion":EVENT_VERSION,"operationId":a["body"]["operationId"],"admissionId":a["id"],"event":{"kind":"failed_spent","executionStartedId":null,"reason":"input_missing"}});
    let op = parse_operation(
        &i.operations[0],
        &parse_validation_batch(&i.batches[0], &c.registered_sources[0]).unwrap(),
    )
    .unwrap();
    let b = parse_validation_batch(&i.batches[0], &c.registered_sources[0]).unwrap();
    i.events.push(
        freeze_confirmation_event(&terminal.to_string(), &b, &op)
            .unwrap()
            .document()
            .to_string(),
    );
    terminal["event"]["reason"] = json!("owner_lost");
    i.events.push(
        freeze_confirmation_event(&terminal.to_string(), &b, &op)
            .unwrap()
            .document()
            .to_string(),
    );
    let completed = complete_confirmation_graph(&i, &c).unwrap();
    let kinds: BTreeSet<_> = completed
        .inventory()
        .conflicts
        .iter()
        .map(|s| doc(s)["body"]["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        kinds,
        BTreeSet::from(["execution_started".into(), "seal".into(), "terminal".into()])
    );
    assert_eq!(completed.inventory().events.len(), 6);
    validate_confirmation_graph(completed.inventory(), &c).unwrap();
    let mut other = i.clone();
    change_admission(&mut other, |v| {
        v["workspaceObservation"]["mutationSeq"] = json!(13)
    });
    i.admissions.push(other.admissions[0].clone());
    let completed = complete_confirmation_graph(&i, &c).unwrap();
    assert!(completed
        .inventory()
        .conflicts
        .iter()
        .any(|s| doc(s)["body"]["kind"] == "admission"));
    assert_eq!(completed.inventory().admissions.len(), 2);
}
#[test]
fn false_conflict_relations_and_inexact_targets_are_refused_after_rehash() {
    let (mut i, c) = val_only();
    let start = doc(&i.exposures[0])["body"]["intervals"][0]["fromMs"]
        .as_u64()
        .unwrap();
    manual(&mut i, '7', start, start + 1, None);
    let mut i = complete_confirmation_graph(&i, &c)
        .unwrap()
        .inventory()
        .clone();
    let good = i.conflicts[0].clone();
    for (field, value) in [
        ("kind", json!("exposure_overlap")),
        ("affectedFamilyIds", json!([])),
        ("affectedInstrumentIds", json!(["crypto:binance:ETHUSDT"])),
    ] {
        let mut v = doc(&good);
        v["body"][field] = value;
        rehash(&mut v);
        i.conflicts[0] = v.to_string();
        assert_bad(&i, &c);
    }
}
#[test]
fn same_origin_key_excludes_linked_confirmation_exposure_but_includes_manual() {
    let (mut i, c) = val_only();
    assert!(complete_confirmation_graph(&i, &c)
        .unwrap()
        .inventory()
        .conflicts
        .is_empty());
    manual(&mut i, '4', 946684800000, 946684800001, None);
    let result = complete_confirmation_graph(&i, &c).unwrap();
    assert_eq!(result.inventory().conflicts.len(), 1);
    let conflict = doc(&result.inventory().conflicts[0]);
    assert_eq!(conflict["body"]["kind"], "origin_key");
    assert!(conflict["body"]["claimIds"]
        .as_array()
        .unwrap()
        .contains(&doc(&i.operations[0])["id"]));
}
#[test]
fn unknown_provenance_is_readable_and_permanently_restrictive() {
    let (mut i, c) = setup();
    i.unknown_origins.push("3".repeat(32));
    assert!(validate_confirmation_graph(&i, &c).unwrap().is_restricted());
    let mut c = c;
    c.alpha_unknown_origins.push("4".repeat(32));
    assert!(validate_confirmation_graph(&i, &c).unwrap().is_restricted());
    i.unknown_origins.push("3".repeat(32));
    assert_bad(&i, &c);
}
#[test]
fn archival_receipts_use_historical_counts_and_cannot_authenticate_consistent_host_inventions() {
    let (i, mut c) = setup();
    c.family_tests.values_mut().for_each(|v| *v = 4);
    validate_confirmation_graph(&i, &c).unwrap();
    // These observations are deliberately synthetic. Validation establishes
    // internal consistency and returns no runtime/admission permission API.
    assert_eq!(c.historical_trials[0].family_effective_trials, 4);
    assert!(!validate_confirmation_graph(&i, &c).unwrap().is_restricted());
    c.historical_trials[0].family_effective_trials = 5;
    assert_bad(&i, &c);
}
#[test]
fn row_byte_interval_unknown_capacities_do_not_truncate() {
    let mut i = ConfirmationInventory {
        batches: vec![String::new(); MAX_HISTORY_ROWS],
        ..ConfirmationInventory::default()
    };
    i.capacity().unwrap();
    i.batches.push(String::new());
    assert_eq!(i.capacity().unwrap_err(), HistoryRecordError::Capacity);
    let mut i = ConfirmationInventory {
        batches: vec![" ".repeat(MAX_DOCUMENT_BYTES); 16],
        ..ConfirmationInventory::default()
    };
    i.capacity().unwrap();
    i.batches.push(" ".into());
    assert_eq!(i.capacity().unwrap_err(), HistoryRecordError::Capacity);
    let mut i = ConfirmationInventory {
        unknown_origins: (0..10000).map(|v| format!("{v:032x}")).collect(),
        ..ConfirmationInventory::default()
    };
    i.capacity().unwrap();
    i.unknown_origins.push(format!("{:032x}", 10000));
    assert_eq!(i.capacity().unwrap_err(), HistoryRecordError::Capacity);
}
#[test]
fn active_sweep_exclusions_are_exact_and_do_not_scan_same_operation_members() {
    let mut a = Active::default();
    for n in 0..10000 {
        a.insert("same", &format!("{n:064x}"));
    }
    a.insert("other", &"f".repeat(64));
    assert_eq!(a.excluding_group("same"), Some("f".repeat(64)));
    assert_eq!(
        a.excluding_member(&format!("{:064x}", 0)),
        Some(format!("{:064x}", 1))
    );
    a.remove("same", &format!("{:064x}", 0));
    assert_eq!(a.excluding_group("other"), Some(format!("{:064x}", 1)));
}
