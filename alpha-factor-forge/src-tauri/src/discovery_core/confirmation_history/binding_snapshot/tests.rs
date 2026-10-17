use super::*;
use crate::discovery_core::{
    confirmation_history::{BlobKind, HistoryBlob},
    precision::PRECISION_MAX_COUNT,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../fixtures/rs-core/research-binding-snapshot-v5.json"
    ))
    .unwrap()
}
fn binding() -> Value {
    fixture()["binding"].clone()
}
fn parse(value: &Value) -> BindingSnapshot {
    parse_binding_snapshot(&value.to_string()).unwrap()
}
fn bad(raw: &str) {
    let error = parse_binding_snapshot(raw).unwrap_err();
    assert!(!error.to_string().contains("PROTECTED_SENTINEL"));
    assert!(HistoryBlob::new(BlobKind::Binding, raw).is_err());
}
const NAMES: [&str; 9] = [
    "batches",
    "selections",
    "operations",
    "exposures",
    "admissions",
    "events",
    "blobs",
    "conflicts",
    "unknownOrigins",
];

#[test]
fn independent_node_json_bytes_and_all_readonly_fields_roundtrip() {
    let f = fixture();
    let raw = f["raw"].as_str().unwrap();
    let snapshot = parse_binding_snapshot(raw).unwrap();
    let text = snapshot.to_canonical_json().unwrap();
    assert_eq!(text, f["canonicalJson"]);
    assert_eq!(
        hex::encode(Sha256::digest(text.as_bytes())),
        f["canonicalSha256"]
    );
    assert_eq!(parse_binding_snapshot(&text).unwrap(), snapshot);
    assert_eq!(snapshot.registry_id(), f["binding"]["registryId"]);
    assert_eq!(snapshot.seq(), 0);
    assert_eq!(snapshot.chain_head(), f["binding"]["chainHead"]);
    let b = &f["binding"];
    assert_eq!(
        json!(snapshot.quarantined_families()),
        b["evidence"]["quarantinedFamilies"]
    );
    assert_eq!(
        json!(snapshot.conflicted_origins()),
        b["evidence"]["conflictedOrigins"]
    );
    assert_eq!(json!(snapshot.family_tests()), b["evidence"]["familyTests"]);
    assert_eq!(
        snapshot.alpha_budgets().collect::<Vec<_>>(),
        vec![(
            b["alpha"]["budgets"][0]["familyId"].as_str().unwrap(),
            b["alpha"]["budgets"][0]["allocationId"].as_str().unwrap(),
        )]
    );
    assert_eq!(
        json!(snapshot.alpha_reservations()),
        b["alpha"]["reservations"]
    );
    assert_eq!(json!(snapshot.alpha_conflicts()), b["alpha"]["conflicts"]);
    assert_eq!(
        json!(snapshot.alpha_unknown_origins()),
        b["alphaUnknownOrigins"]
    );
    for (section, name) in SETS.into_iter().zip(NAMES) {
        assert_eq!(
            json!(snapshot.confirmation_ids(section)),
            b["confirmation"][name]
        );
    }
    // Canonicalizing a new workspace binding must not rewrite an old blob.
    let blob = HistoryBlob::new(BlobKind::Binding, raw).unwrap();
    assert_eq!(blob.sha256(), f["rawSha256"]);
    assert_eq!(blob.text(), raw);
    assert_ne!(blob.sha256(), f["canonicalSha256"]);
}

#[test]
fn every_confirmation_set_detects_loss_and_equal_count_substitution() {
    let saved = parse(&binding());
    assert!(saved.covers_confirmation(&saved));
    for name in NAMES {
        let mut current = binding();
        current["confirmation"][name] = json!([]);
        assert!(!parse(&current).covers_confirmation(&saved), "lost {name}");
        current["confirmation"][name] =
            json!(["f".repeat(if name == "unknownOrigins" { 32 } else { 64 })]);
        assert!(
            !parse(&current).covers_confirmation(&saved),
            "substituted {name}"
        );
        current["confirmation"][name]
            .as_array_mut()
            .unwrap()
            .insert(0, binding()["confirmation"][name][0].clone());
        assert!(parse(&current).covers_confirmation(&saved), "growth {name}");
        assert!(
            !saved.covers_confirmation(&parse(&current)),
            "reverse {name}"
        );
    }
}

#[test]
fn coverage_is_per_section_and_handles_skipped_prefix_infix_and_suffix() {
    let mut old = binding();
    let mut now = binding();
    old["confirmation"]["batches"] = json!(["2".repeat(64), "4".repeat(64)]);
    now["confirmation"]["batches"] = json!([
        "1".repeat(64),
        "2".repeat(64),
        "3".repeat(64),
        "4".repeat(64),
        "5".repeat(64)
    ]);
    assert!(parse(&now).covers_confirmation(&parse(&old)));
    now["confirmation"]["batches"] = json!(["1".repeat(64), "2".repeat(64), "3".repeat(64)]);
    now["confirmation"]["events"] = old["confirmation"]["batches"].clone();
    assert!(!parse(&now).covers_confirmation(&parse(&old)));
    // Coverage never decides replacement-prefix/trial/alpha acceptance.
    let mut replacement = old.clone();
    replacement["registryId"] = json!("9".repeat(32));
    replacement["seq"] = json!(1);
    replacement["chainHead"] = json!("f".repeat(64));
    replacement["alpha"]["reservations"] = json!([]);
    assert!(parse(&replacement).covers_confirmation(&parse(&old)));
}

#[test]
fn every_level_is_closed_and_requires_fields_in_original_text() {
    for path in [
        "",
        "/evidence",
        "/alpha",
        "/confirmation",
        "/alpha/budgets/0",
    ] {
        let original = binding();
        let object = original.pointer(path).unwrap();
        for name in object.as_object().unwrap().keys() {
            let mut v = original.clone();
            let target = v.pointer_mut(path).unwrap();
            target.as_object_mut().unwrap().remove(name);
            bad(&v.to_string());
            let mut v = original.clone();
            let target = v.pointer_mut(path).unwrap();
            target[name] = Value::Null;
            bad(&v.to_string());
            let needle = format!("{}:{}", serde_json::to_string(name).unwrap(), object[name]);
            let raw = original
                .to_string()
                .replacen(&needle, &format!("{needle},{needle}"), 1);
            assert_ne!(raw, original.to_string());
            bad(&raw);
        }
        let mut v = original;
        let target = v.pointer_mut(path).unwrap();
        target["PROTECTED_SENTINEL"] = json!("secret");
        bad(&v.to_string());
    }
}

#[test]
fn sorted_sets_and_dynamic_family_keys_refuse_duplicate_normalization() {
    for (parent, name) in NAMES.into_iter().map(|n| ("confirmation", n)).chain([
        ("evidence", "quarantinedFamilies"),
        ("evidence", "conflictedOrigins"),
        ("alpha", "budgets"),
        ("alpha", "reservations"),
        ("alpha", "conflicts"),
    ]) {
        let mut v = binding();
        let item = v[parent][name][0].clone();
        v[parent][name] = json!([item.clone(), item]);
        bad(&v.to_string());
    }
    let mut v = binding();
    v["alphaUnknownOrigins"] = json!(["3".repeat(32), "2".repeat(32)]);
    bad(&v.to_string());
    let family = binding()["evidence"]["quarantinedFamilies"][0]
        .as_str()
        .unwrap()
        .to_string();
    let needle = format!("\"{family}\":2");
    let raw = binding()
        .to_string()
        .replace(&needle, &format!("{needle},{needle}"));
    bad(&raw);
    let reversed = format!("\"trial-family-v1:{}\":1,{needle}", "f".repeat(64));
    bad(&binding().to_string().replace(&needle, &reversed));
}

#[test]
fn version_safe_integer_and_genesis_limits_match_history_blob_codec() {
    for version in [
        "trial-ledger-binding-v1",
        "trial-ledger-binding-v2",
        "trial-ledger-binding-v3",
        "trial-ledger-binding-v4",
        "trial-ledger-binding-v6",
    ] {
        let mut v = binding();
        v["version"] = json!(version);
        bad(&v.to_string());
    }
    let mut v = binding();
    v["seq"] = json!(PRECISION_MAX_COUNT);
    assert_eq!(parse(&v).seq(), PRECISION_MAX_COUNT);
    v["seq"] = json!(PRECISION_MAX_COUNT + 1);
    bad(&v.to_string());
    for invalid in [json!(-1), json!(1.5), json!("1"), Value::Null] {
        let mut v = binding();
        v["seq"] = invalid;
        bad(&v.to_string());
    }
    let mut v = binding();
    v["chainHead"] = json!("0".repeat(64));
    bad(&v.to_string());
    v = binding();
    let family = v["evidence"]["quarantinedFamilies"][0]
        .as_str()
        .unwrap()
        .to_string();
    v["evidence"]["familyTests"][&family] = json!(PRECISION_MAX_COUNT);
    parse(&v);
    for count in [0, PRECISION_MAX_COUNT + 1] {
        v["evidence"]["familyTests"][&family] = json!(count);
        bad(&v.to_string());
    }
}

#[test]
fn exact_eight_mib_original_byte_boundary_is_not_canonical_size() {
    let raw = binding().to_string();
    let at_limit = format!("{raw}{}", " ".repeat(MAX_BINDING_BYTES - raw.len()));
    let snapshot = parse_binding_snapshot(&at_limit).unwrap();
    assert_eq!(snapshot, parse(&binding()));
    assert_eq!(
        HistoryBlob::new(BlobKind::Binding, &at_limit)
            .unwrap()
            .text()
            .len(),
        MAX_BINDING_BYTES
    );
    bad(&(at_limit + " "));
    let larger_than_legacy = format!("{raw}{}", " ".repeat(4 * 1024 * 1024));
    assert!(parse_binding_snapshot(&larger_than_legacy).is_ok());
}

#[test]
fn joint_confirmation_capacity_cannot_be_reset_per_section() {
    let mut v = binding();
    for name in NAMES {
        v["confirmation"][name] = json!([]);
    }
    let ids: Vec<_> = (0..10000).map(|n| format!("{n:064x}")).collect();
    v["confirmation"]["blobs"] = json!(ids);
    assert_eq!(
        parse(&v)
            .confirmation_ids(ConfirmationBindingSet::Blobs)
            .len(),
        10000
    );
    v["confirmation"]["events"] = json!(["f".repeat(64)]);
    bad(&v.to_string());
    v["confirmation"]["events"] = json!([]);
    v["confirmation"]["unknownOrigins"] =
        json!((0..10000).map(|n| format!("{n:032x}")).collect::<Vec<_>>());
    parse(&v); // Unknown origins have their own permanent provenance bound.
    v["confirmation"]["unknownOrigins"]
        .as_array_mut()
        .unwrap()
        .push(json!("f".repeat(32)));
    bad(&v.to_string());
}

#[test]
fn all_sections_match_an_independent_exhaustive_subset_oracle() {
    for name in NAMES {
        let width = if name == "unknownOrigins" { 32 } else { 64 };
        let snapshots: Vec<_> = (0u8..32)
            .map(|mask| {
                let mut v = binding();
                let ids: Vec<_> = (0..5)
                    .filter(|bit| mask & (1 << bit) != 0)
                    .map(|bit| format!("{:0width$x}", bit + 1, width = width))
                    .collect();
                v["confirmation"][name] = json!(ids);
                parse(&v)
            })
            .collect();
        for now in 0usize..32 {
            for old in 0usize..32 {
                assert_eq!(
                    snapshots[now].covers_confirmation(&snapshots[old]),
                    old & !now == 0,
                    "{name}: {now}/{old}"
                );
            }
        }
    }
}

#[test]
fn alpha_and_provenance_caps_remain_separate_from_confirmation_ids() {
    let mut v = binding();
    v["alpha"]["budgets"] = json!([]);
    v["alpha"]["conflicts"] = json!([]);
    v["alpha"]["reservations"] = json!((0..30000).map(|n| format!("{n:064x}")).collect::<Vec<_>>());
    parse(&v);
    v["alpha"]["conflicts"] = json!(["f".repeat(64)]);
    bad(&v.to_string());
    v = binding();
    v["alphaUnknownOrigins"] = json!((0..10000).map(|n| format!("{n:032x}")).collect::<Vec<_>>());
    parse(&v);
    v["alphaUnknownOrigins"]
        .as_array_mut()
        .unwrap()
        .push(json!("f".repeat(32)));
    bad(&v.to_string());
}

#[test]
fn all_confirmation_id_domains_are_checked_before_coverage() {
    for name in NAMES {
        let width = if name == "unknownOrigins" { 32 } else { 64 };
        for invalid in [
            "A".repeat(width),
            "g".repeat(width),
            "a".repeat(width - 1),
            format!("{}\0", "a".repeat(width - 1)),
            "PROTECTED_SENTINEL".into(),
        ] {
            let mut v = binding();
            v["confirmation"][name] = json!([invalid]);
            bad(&v.to_string());
        }
        let mut v = binding();
        v["confirmation"][name] = json!(["f".repeat(width), "a".repeat(width)]);
        bad(&v.to_string());
    }
}
