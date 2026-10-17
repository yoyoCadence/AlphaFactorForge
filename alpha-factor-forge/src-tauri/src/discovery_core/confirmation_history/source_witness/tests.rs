use super::*;
use crate::discovery_core::confirmation_history::parse_binding_snapshot;
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../fixtures/rs-core/research-source-witness-v1.json"
    ))
    .unwrap()
}
fn source() -> RegisteredSource {
    serde_json::from_value(fixture()["registeredSource"].clone()).unwrap()
}
fn valid() -> Value {
    serde_json::from_str(fixture()["cases"][0]["witnessText"].as_str().unwrap()).unwrap()
}
fn bad(raw: &str) {
    let err = parse_source_witness(raw).unwrap_err();
    assert!(!err.to_string().contains("PROTECTED_SENTINEL"));
}
fn anchor(case: &Value) -> SourceTrialAnchor {
    let a = &case["anchor"];
    SourceTrialAnchor::from_host_facts(
        HistoricalPrefix {
            registry_id: a["registryId"].as_str().unwrap().into(),
            seq: a["seq"].as_u64().unwrap(),
            chain_head: a["chainHead"].as_str().unwrap().into(),
        },
        serde_json::from_value(a["evidence"]["quarantinedFamilies"].clone()).unwrap(),
        serde_json::from_value(a["evidence"]["conflictedOrigins"].clone()).unwrap(),
        serde_json::from_value(a["evidence"]["familyTests"].clone()).unwrap(),
    )
    .unwrap()
}

#[test]
fn independent_node_original_bytes_hashes_and_freeze_roundtrip() {
    let f = fixture();
    for case in f["cases"].as_array().unwrap() {
        let text = case["witnessText"].as_str().unwrap();
        let w = parse_source_witness(text).unwrap();
        assert_eq!(w.text(), text);
        assert_eq!(w.sha256(), case["witnessSha256"]);
        assert_eq!(w.trial_export_text(), case["trialExportText"]);
        assert_eq!(w.source_claims(), &source());
        assert_eq!(
            freeze_source_witness(w.trial_export_text(), w.source_claims()).unwrap(),
            w
        );
        assert_eq!(
            hex::encode(Sha256::digest(w.trial_export_text().as_bytes())),
            case["archiveSha256"]
        );
        assert!(anchor(case).matches_binding(
            &parse_binding_snapshot(case["bindingJson"].as_str().unwrap()).unwrap()
        ));
    }
    assert_ne!(
        f["cases"][0]["witnessSha256"],
        f["cases"][2]["witnessSha256"]
    );
    assert_eq!(anchor(&f["cases"][0]), anchor(&f["cases"][2]));
}

#[test]
fn every_original_nested_object_is_closed_required_and_duplicate_sensitive() {
    for path in [
        "",
        "/registeredSource",
        "/registeredSource/source",
        "/registeredSource/candidates/0",
    ] {
        let original = valid();
        let object = original.pointer(path).unwrap();
        for name in object.as_object().unwrap().keys() {
            let mut v = original.clone();
            v.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(name);
            bad(&canonical(&v).unwrap());
            let mut v = original.clone();
            v.pointer_mut(path).unwrap()[name] = Value::Null;
            bad(&canonical(&v).unwrap());
            let needle = format!(
                "{}:{}",
                serde_json::to_string(name).unwrap(),
                canonical(&object[name]).unwrap()
            );
            let text = canonical(&original).unwrap();
            let duplicate = text.replacen(&needle, &format!("{needle},{needle}"), 1);
            assert_ne!(text, duplicate);
            bad(&duplicate);
        }
        let mut v = original;
        v.pointer_mut(path).unwrap()["PROTECTED_SENTINEL"] = json!(true);
        bad(&canonical(&v).unwrap());
    }
}

#[test]
fn outer_canonical_requirement_never_normalizes_nested_original_archive() {
    let w = freeze_source_witness("opaque original\r\n", &source()).unwrap();
    assert_eq!(
        parse_source_witness(w.text()).unwrap().trial_export_text(),
        "opaque original\r\n"
    );
    assert_ne!(
        w.sha256(),
        freeze_source_witness("opaque original\n", &source())
            .unwrap()
            .sha256()
    );
    bad(&(w.text().to_string() + "\n"));
    bad(&(" ".to_string() + w.text()));
    bad(&serde_json::to_string_pretty(&valid()).unwrap());
    let raw = canonical(&valid()).unwrap();
    bad(&raw.replacen(
        "research-confirmation-source-witness-v1",
        "research-confirmation-source-witness-v2",
        1,
    ));
    for inner in ["", "missing final newline"] {
        assert!(freeze_source_witness(inner, &source()).is_err());
    }
}

#[test]
fn opaque_archive_shape_is_not_a_complete_trial_proof() {
    let w = freeze_source_witness("PROTECTED_SENTINEL\n", &source()).unwrap();
    assert!(parse_source_witness(w.text()).is_ok());
    // Only backend reconstruction may validate its contents. This snapshot
    // deliberately cannot enter existing stored graph HistoryBlob inventory.
    assert_eq!(w.trial_export_text(), "PROTECTED_SENTINEL\n");
}

#[test]
fn exact_outer_byte_limit_and_escaping_overhead_are_enforced() {
    let overhead = freeze_source_witness("\n", &source()).unwrap().text().len();
    let inner = "x".repeat(MAX_SOURCE_WITNESS_BYTES - overhead) + "\n";
    let w = freeze_source_witness(&inner, &source()).unwrap();
    assert_eq!(w.text().len(), MAX_SOURCE_WITNESS_BYTES);
    assert_eq!(parse_source_witness(w.text()).unwrap(), w);
    assert_eq!(
        freeze_source_witness(&("x".to_string() + &inner), &source()).unwrap_err(),
        HistoryRecordError::TooLarge
    );
    bad(&(w.text().to_string() + " "));
    let escaped = "\\".repeat(MAX_SOURCE_WITNESS_BYTES / 2) + "\n";
    assert!(escaped.len() < MAX_SOURCE_WITNESS_BYTES);
    assert_eq!(
        freeze_source_witness(&escaped, &source()).unwrap_err(),
        HistoryRecordError::TooLarge
    );
    assert_eq!(
        freeze_source_witness(&"x".repeat(MAX_SOURCE_WITNESS_BYTES + 1), &source()).unwrap_err(),
        HistoryRecordError::TooLarge
    );
}

#[test]
fn source_domains_order_duplicates_and_safe_integers_are_checked() {
    for path in [
        "/registeredSource/source/runId",
        "/registeredSource/candidates/0/candidateIndex",
        "/registeredSource/candidates/0/minimumWarmupBars",
    ] {
        let mut v = valid();
        *v.pointer_mut(path).unwrap() = json!(PRECISION_MAX_COUNT + 1);
        bad(&canonical(&v).unwrap());
    }
    let mut v = valid();
    v["registeredSource"]["candidates"]
        .as_array_mut()
        .unwrap()
        .reverse();
    bad(&canonical(&v).unwrap());
    let mut v = valid();
    v["registeredSource"]["candidates"][1] = v["registeredSource"]["candidates"][0].clone();
    bad(&canonical(&v).unwrap());
    for value in [json!([]), json!([{"PROTECTED_SENTINEL":true}])] {
        let mut v = valid();
        v["registeredSource"]["candidates"] = value;
        bad(&canonical(&v).unwrap());
    }
    for value in ["F".repeat(64), "g".repeat(64), "PROTECTED_SENTINEL".into()] {
        let mut v = valid();
        v["registeredSource"]["candidates"][0]["resultArtifactHash"] = json!(value);
        bad(&canonical(&v).unwrap());
    }
}

#[test]
fn existing_candidate_limit_is_preserved_without_an_arbitrary_anchor_family_limit() {
    let mut s = source();
    let template = s.candidates[0].clone();
    s.candidates = (0..4096)
        .map(|i| {
            let mut c = template.clone();
            c.candidate_index = i;
            c
        })
        .collect();
    let w = freeze_source_witness("opaque\n", &s).unwrap();
    assert_eq!(
        parse_source_witness(w.text())
            .unwrap()
            .source_claims()
            .candidates
            .len(),
        4096
    );
    let mut c = template;
    c.candidate_index = 4096;
    s.candidates.push(c);
    assert!(freeze_source_witness("opaque\n", &s).is_err());
    let a = anchor(&fixture()["cases"][0]);
    let map = (0..15_000)
        .map(|i| (format!("trial-family-v1:{i:064x}"), 2))
        .collect();
    assert_eq!(
        SourceTrialAnchor::from_host_facts(a.prefix().clone(), vec![], vec![], map)
            .unwrap()
            .family_tests()
            .len(),
        15_000
    );
}

#[test]
fn original_anchor_matches_every_trial_field_exactly_and_ignores_other_scopes() {
    let f = fixture();
    let a = anchor(&f["cases"][0]);
    let original: Value =
        serde_json::from_str(f["cases"][0]["bindingJson"].as_str().unwrap()).unwrap();
    for field in [
        "registryId",
        "seq",
        "chainHead",
        "quarantinedFamilies",
        "conflictedOrigins",
        "familyTests",
    ] {
        let mut v = original.clone();
        match field {
            "registryId" => v[field] = json!("f".repeat(32)),
            "seq" => v[field] = json!(5),
            "chainHead" => v[field] = json!("f".repeat(64)),
            "quarantinedFamilies" => v["evidence"][field] = json!([source().source.family_id]),
            "conflictedOrigins" => v["evidence"][field] = json!(["f".repeat(32)]),
            _ => v["evidence"][field][source().source.family_id] = json!(3),
        }
        assert!(
            !a.matches_binding(&parse_binding_snapshot(&v.to_string()).unwrap()),
            "{field}"
        );
    }
    let mut v = original;
    v["alpha"]["reservations"] = json!(["a".repeat(64)]);
    v["confirmation"]["events"] = json!(["b".repeat(64)]);
    assert!(a.matches_binding(&parse_binding_snapshot(&v.to_string()).unwrap()));
    assert_eq!(a.prefix().seq, 4);
    assert!(a.quarantined_families().is_empty());
    assert!(a.conflicted_origins().is_empty());
    assert_eq!(a.family_tests().get(&source().source.family_id), Some(&2));
}

#[test]
fn same_event_head_does_not_merge_different_original_protocol_anchors() {
    let f = fixture();
    let a = anchor(&f["cases"][0]);
    let upgraded = anchor(&f["cases"][1]);
    assert_eq!(a.prefix(), upgraded.prefix());
    assert_ne!(a, upgraded);
    assert!(!a.matches_binding(
        &parse_binding_snapshot(f["cases"][1]["bindingJson"].as_str().unwrap()).unwrap()
    ));
    assert!(!upgraded.matches_binding(
        &parse_binding_snapshot(f["cases"][0]["bindingJson"].as_str().unwrap()).unwrap()
    ));
}

#[test]
fn anchor_constructor_checks_genesis_domains_order_and_safe_ranges() {
    let f = fixture();
    let a = anchor(&f["cases"][0]);
    let make = |p, q, c, t| SourceTrialAnchor::from_host_facts(p, q, c, t);
    let mut p = a.prefix().clone();
    p.seq = 0;
    assert!(make(p.clone(), vec![], vec![], BTreeMap::new()).is_err());
    p.chain_head = admission::genesis(&p.registry_id);
    assert!(make(p, vec![], vec![], BTreeMap::new()).is_ok());
    let mut p = a.prefix().clone();
    p.seq = PRECISION_MAX_COUNT + 1;
    assert!(make(p, vec![], vec![], BTreeMap::new()).is_err());
    let id = source().source.family_id;
    assert!(make(
        a.prefix().clone(),
        vec![id.clone(), id.clone()],
        vec![],
        BTreeMap::new()
    )
    .is_err());
    assert!(make(
        a.prefix().clone(),
        vec![],
        vec!["f".repeat(32), "a".repeat(32)],
        BTreeMap::new()
    )
    .is_err());
    for n in [0, PRECISION_MAX_COUNT + 1] {
        assert!(make(
            a.prefix().clone(),
            vec![],
            vec![],
            BTreeMap::from([(id.clone(), n)])
        )
        .is_err());
    }
}
