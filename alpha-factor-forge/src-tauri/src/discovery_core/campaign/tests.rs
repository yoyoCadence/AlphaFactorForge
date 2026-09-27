use super::*;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-campaign-declaration-v1.json"
    ))
    .unwrap()
}

#[test]
fn freeze_round_trip_is_immutable_and_has_no_admission_claim() {
    let mut raw = fixture();
    let frozen = freeze_campaign(&raw).unwrap();
    assert_eq!(freeze_campaign(frozen.document()).unwrap(), frozen);
    assert_eq!(frozen.campaign_id().len(), 64);
    // Independently derived with Node crypto + an authored binary encoder.
    assert_eq!(
        frozen.campaign_id(),
        "2ffaed4e40e8cd81c1d8631f802c8b744876f83cfef29e2d7db01db5e5f26c45"
    );
    assert!(frozen.document().get("status").is_none());
    assert!(frozen.document().get("priorTrials").is_none());
    raw["sampling"]["alphaPpm"] = json!(10000);
    assert_eq!(frozen.document()["sampling"]["alphaPpm"], 50000);
    assert_ne!(
        freeze_campaign(&raw).unwrap().campaign_id(),
        frozen.campaign_id()
    );
}

#[test]
fn instrument_order_is_not_identity_but_membership_is() {
    let mut raw = fixture();
    let single = freeze_campaign(&raw).unwrap();
    let mut eth = raw["instruments"][0].clone();
    eth["instrumentId"] = json!("crypto:binance:ETHUSDT");
    eth["snapshotId"] = json!("c".repeat(64));
    eth["datasetHash"] = json!(format!("dataset-content-v2:{}", "d".repeat(64)));
    raw["instruments"].as_array_mut().unwrap().push(eth);
    let forward = freeze_campaign(&raw).unwrap();
    raw["instruments"].as_array_mut().unwrap().reverse();
    assert_eq!(freeze_campaign(&raw).unwrap(), forward);
    assert_ne!(single.campaign_id(), forward.campaign_id());
    let reordered: Value =
        serde_json::from_str(&serde_json::to_string_pretty(&raw).unwrap()).unwrap();
    assert_eq!(freeze_campaign(&reordered).unwrap(), forward);
}

// Every nested key is required; every object rejects additional data. This
// includes delistedAtMs:null and prevents counts/results sneaking into input.
fn assert_strict_objects(value: &Value) {
    fn walk(root: &Value, value: &Value, pointer: &str) {
        match value {
            Value::Object(fields) => {
                let mut added = root.clone();
                added
                    .pointer_mut(pointer)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("unexpected".into(), json!(1));
                assert!(
                    freeze_campaign(&added).is_err(),
                    "accepted extra field at {pointer}"
                );
                for (key, child) in fields {
                    let mut missing = root.clone();
                    missing
                        .pointer_mut(pointer)
                        .unwrap()
                        .as_object_mut()
                        .unwrap()
                        .remove(key);
                    assert!(
                        freeze_campaign(&missing).is_err(),
                        "accepted missing {pointer}/{key}"
                    );
                    walk(root, child, &format!("{pointer}/{key}"));
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    walk(root, child, &format!("{pointer}/{index}"));
                }
            }
            _ => {}
        }
    }
    walk(value, value, "");
}

#[test]
fn missing_unknown_and_unpinned_contracts_fail_closed() {
    let raw = fixture();
    assert_strict_objects(&raw);
    for key in raw["contracts"].as_object().unwrap().keys() {
        let mut changed = raw.clone();
        changed["contracts"][key] = json!("unknown-v9");
        assert!(
            freeze_campaign(&changed).is_err(),
            "accepted contracts.{key}"
        );
    }
    let mut changed = raw;
    changed["contractVersion"] = json!("research-campaign-declaration-v2");
    assert!(freeze_campaign(&changed).is_err());
    for invalid in [Value::Null, json!([]), json!(1), json!("campaign")] {
        assert!(freeze_campaign(&invalid).is_err());
    }
}

#[test]
fn invalid_bindings_and_listing_ranges_are_rejected() {
    for (field, value) in [
        ("instrumentId", json!("BTCUSDT")),
        ("interval", json!("unknown")),
        ("snapshotId", json!("A".repeat(64))),
        ("datasetHash", json!("b".repeat(63))),
        ("datasetHash", json!("b".repeat(64))),
        (
            "datasetHash",
            json!(format!("dataset-content-v1:{}", "b".repeat(64))),
        ),
        ("listedAtMs", json!(1735689600001u64)),
        ("fromMs", json!(1767222000001u64)),
        ("toMs", json!(PRECISION_MAX_COUNT + 1)),
        ("delistedAtMs", json!(1767222000000u64)),
        ("delistedAtMs", json!(PRECISION_MAX_COUNT + 1)),
        ("fromMs", json!(-1)),
        ("fromMs", json!(1.0)),
    ] {
        let mut raw = fixture();
        raw["instruments"][0][field] = value;
        assert!(freeze_campaign(&raw).is_err(), "accepted {field}");
    }
    let mut raw = fixture();
    raw["instruments"][0]["delistedAtMs"] = json!(1767222000001u64);
    assert!(freeze_campaign(&raw).is_ok(), "exclusive endpoint");
}

#[test]
fn instrument_set_is_bounded_and_duplicates_cannot_hide_behind_other_fields() {
    let raw = fixture();
    let mut duplicate = raw.clone();
    let mut other = raw["instruments"][0].clone();
    other["interval"] = json!("4h");
    other["snapshotId"] = json!("c".repeat(64));
    duplicate["instruments"].as_array_mut().unwrap().push(other);
    assert!(freeze_campaign(&duplicate)
        .unwrap_err()
        .0
        .contains("duplicate"));
    let mut empty = raw.clone();
    empty["instruments"] = json!([]);
    assert!(freeze_campaign(&empty).is_err());
    let mut bounded = raw.clone();
    bounded["instruments"] = Value::Array(
        (0..CAMPAIGN_MAX_INSTRUMENTS)
            .map(|index| {
                let mut instrument = raw["instruments"][0].clone();
                instrument["instrumentId"] = json!(format!("crypto:binance:ASSET{index}"));
                instrument
            })
            .collect(),
    );
    assert!(freeze_campaign(&bounded).is_ok());
    bounded["instruments"]
        .as_array_mut()
        .unwrap()
        .push(raw["instruments"][0].clone());
    assert!(freeze_campaign(&bounded).is_err());
}

#[test]
fn policy_domains_reuse_precision_and_fold_contracts_without_claiming_feasibility() {
    for (pointer, value) in [
        ("/sampling/alphaPpm", json!(0)),
        ("/sampling/alphaPpm", json!(1000000)),
        ("/sampling/maxRelativeStandardErrorPpm", json!(1000000)),
        ("/sampling/bootstrapSamples", json!(200001)),
        (
            "/sampling/maxBootstrapSamples",
            json!(PRECISION_MAX_COUNT + 1),
        ),
        ("/instruments/0/samplePolicy/minimumTotalBars", json!(0)),
        ("/instruments/0/samplePolicy/minimumTrainBars", json!(0)),
        (
            "/instruments/0/samplePolicy/foldValidationBars",
            json!(PRECISION_MAX_COUNT + 1),
        ),
        ("/instruments/0/samplePolicy/foldCount", json!(1)),
        ("/instruments/0/samplePolicy/foldCount", json!(129)),
        ("/instruments/0/samplePolicy/foldCount", json!(3.0)),
        ("/instruments/0/samplePolicy/rationale", json!(" ")),
        ("/instruments/0/samplePolicy/rationale", json!("untrimmed ")),
        (
            "/instruments/0/samplePolicy/rationale",
            json!("x".repeat(1025)),
        ),
    ] {
        let mut raw = fixture();
        *raw.pointer_mut(pointer).unwrap() = value;
        assert!(freeze_campaign(&raw).is_err(), "accepted {pointer}");
    }
    let mut small = fixture();
    small["sampling"]["bootstrapSamples"] = json!(1);
    small["instruments"][0]["samplePolicy"]["minimumTotalBars"] = json!(1);
    assert!(
        freeze_campaign(&small).is_ok(),
        "a valid declaration is not an admission decision"
    );
}

#[test]
fn every_mutable_policy_and_binding_changes_the_identity() {
    let raw = fixture();
    let original = freeze_campaign(&raw).unwrap();
    for (pointer, value) in [
        ("/sampling/alphaPpm", json!(49999)),
        ("/sampling/maxRelativeStandardErrorPpm", json!(199999)),
        ("/sampling/bootstrapSamples", json!(99999)),
        ("/sampling/maxBootstrapSamples", json!(199999)),
        (
            "/instruments/0/instrumentId",
            json!("crypto:binance:ETHUSDT"),
        ),
        ("/instruments/0/listedAtMs", json!(1502928000001u64)),
        ("/instruments/0/delistedAtMs", json!(1767222000001u64)),
        ("/instruments/0/snapshotId", json!("c".repeat(64))),
        (
            "/instruments/0/datasetHash",
            json!(format!("dataset-content-v2:{}", "d".repeat(64))),
        ),
        ("/instruments/0/interval", json!("4h")),
        ("/instruments/0/fromMs", json!(1735689600001u64)),
        ("/instruments/0/toMs", json!(1767221999999u64)),
        ("/instruments/0/samplePolicy/minimumTotalBars", json!(8759)),
        ("/instruments/0/samplePolicy/minimumTrainBars", json!(999)),
        ("/instruments/0/samplePolicy/foldValidationBars", json!(499)),
        ("/instruments/0/samplePolicy/foldCount", json!(4)),
        (
            "/instruments/0/samplePolicy/rationale",
            json!("A different predeclared rationale."),
        ),
    ] {
        let mut changed = raw.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        assert_ne!(
            freeze_campaign(&changed).unwrap().campaign_id(),
            original.campaign_id(),
            "unbound {pointer}"
        );
    }
}
