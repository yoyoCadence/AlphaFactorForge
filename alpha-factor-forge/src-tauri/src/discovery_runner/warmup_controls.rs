//! Frozen producer controls call the same private parse/embargo boundary as
//! execution, without candles, signal series, a database or a source capability.

use super::*;
use crate::research::sha256_hex;
use serde_json::json;

const WARMUP: &str =
    include_str!("../../../fixtures/research/original-warmup-producer-projections-v1.json");
const ORIGINAL: &str =
    include_str!("../../../fixtures/research/original-producer-projections-v1.json");
const MODES: &str =
    include_str!("../../../fixtures/research/original-producer-mode-coverage-v1.json");

fn oracle() -> Value {
    assert_eq!(
        sha256_hex(WARMUP.replace("\r\n", "\n").as_bytes()),
        "b67268643162eca6da5c97b056b61d5461c1a31ae0ea02372d8b395a8f31ee06"
    );
    let v: Value = serde_json::from_str(WARMUP).unwrap();
    assert_eq!(v["version"], "original-warmup-producer-projections-v1");
    assert_eq!(v["maxSafeInteger"], 9_007_199_254_740_991_i64);
    assert_eq!(v["cases"].as_array().unwrap().len(), 47);
    assert_eq!(v["families"].as_array().unwrap().len(), 8);
    assert_eq!(v["sourceFixtures"].as_array().unwrap().len(), 2);
    v
}

fn check(id: &str, text: &str, holding: i64, expected: &Value) {
    let bytes = text.as_bytes().to_vec();
    let raw: Value = serde_json::from_str(text).unwrap();
    let saved = raw.clone();
    let parsed = CandidateStrategy::parse(&raw);
    if expected["phase"] == "parse" {
        assert_eq!(expected["status"], "refused");
        assert!(parsed.is_err(), "{id}: expected parse refusal");
    } else {
        assert!(parsed.is_ok(), "{id}: expected parse success: {parsed:?}");
        let first = parsed.unwrap().embargo(holding);
        if expected["phase"] == "embargo" {
            assert_eq!(expected["status"], "refused");
            assert!(first.is_err(), "{id}: expected embargo refusal");
        } else {
            assert_eq!(expected["status"], "ok");
            assert!(first.is_ok(), "{id}: expected embargo success: {first:?}");
            let first = first.unwrap();
            assert_eq!(
                serde_json::to_value(first).unwrap(),
                expected["derivation"],
                "{id}: all three original fields"
            );
            let repeated = CandidateStrategy::parse(&raw)
                .unwrap()
                .embargo(holding)
                .unwrap();
            assert_eq!(repeated, first, "{id}: repeated actual producer");
        }
    }
    assert_eq!(raw, saved, "{id}: borrowed input Value");
    assert_eq!(text.as_bytes(), bytes, "{id}: original input bytes");
}

fn cohort(mode: &str, status: &str, phase: Option<&str>, expected_count: usize) {
    let v = oracle();
    let mut count = 0;
    for c in v["cases"].as_array().unwrap() {
        let text = c["strategyJson"].as_str().unwrap();
        let raw: Value = serde_json::from_str(text).unwrap();
        let matches_mode = if mode == "non-dsl" {
            raw["mode"] != "dsl"
        } else {
            raw["mode"] == mode
        };
        if matches_mode
            && c["expected"]["status"] == status
            && c["expected"]["phase"].as_str() == phase
        {
            assert_eq!(sha256_hex(text.as_bytes()), c["strategySha256"]);
            check(
                c["id"].as_str().unwrap(),
                text,
                c["holdingAllowanceBars"].as_i64().unwrap(),
                &c["expected"],
            );
            count += 1;
        }
    }
    assert_eq!(count, expected_count, "complete frozen cohort");
}

#[test]
fn warmup_internal_bridge_matches_all_47_original_executor_controls() {
    let v = oracle();
    for c in v["cases"].as_array().unwrap() {
        let text = c["strategyJson"].as_str().unwrap();
        assert_eq!(sha256_hex(text.as_bytes()), c["strategySha256"]);
        let d: Value = serde_json::from_str(text).unwrap();
        let saved = d.clone();
        let holding = c["holdingAllowanceBars"].as_i64().unwrap();
        let actual = original_candidate_embargo(&d, holding);
        let direct = CandidateStrategy::parse(&d).and_then(|p| p.embargo(holding));
        assert_eq!(actual, direct, "{}", c["id"]);
        if c["expected"]["status"] == "ok" {
            assert_eq!(
                serde_json::to_value(actual.unwrap()).unwrap(),
                c["expected"]["derivation"]
            );
        } else {
            assert!(actual.is_err());
        }
        assert_eq!(d, saved);
    }
}

#[test]
fn warmup_params_successes_pin_usage_cross_exit_and_safe_boundaries() {
    cohort("params", "ok", None, 12);
}
#[test]
fn warmup_dsl_successes_pin_all_operators_temporal_depth_and_safe_sum() {
    cohort("dsl", "ok", None, 10);
}
#[test]
fn warmup_params_and_unsupported_modes_refuse_during_parse() {
    cohort("non-dsl", "refused", Some("parse"), 8);
}
#[test]
fn warmup_dsl_invalid_documents_refuse_during_parse() {
    cohort("dsl", "refused", Some("parse"), 9);
}
#[test]
fn warmup_params_invalid_signal_and_unsafe_arithmetic_refuse_after_parse() {
    cohort("params", "refused", Some("embargo"), 6);
}
#[test]
fn warmup_dsl_invalid_holding_and_one_over_sum_refuse_after_parse() {
    cohort("dsl", "refused", Some("embargo"), 2);
}

fn family_controls(name: &str, text: &str, expected_members: usize) {
    let v = oracle();
    let pins: Vec<_> = v["sourceFixtures"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["path"] == name)
        .collect();
    assert_eq!(pins.len(), 1);
    // Only the outer fixture packaging follows checkout line endings.
    // Decoded original JSON strings below go to the producer byte-for-byte.
    assert_eq!(
        sha256_hex(text.replace("\r\n", "\n").as_bytes()),
        pins[0]["lfPackagingSha256"]
    );
    let source: Value = serde_json::from_str(text).unwrap();
    let families: Vec<_> = v["families"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["sourceFixture"] == name)
        .collect();
    assert_eq!(families.len(), 4);
    assert_eq!(source["cases"].as_array().unwrap().len(), families.len());
    let mut members = 0;
    let mut ids = std::collections::BTreeSet::new();
    for family in families {
        let id = family["caseId"].as_str().unwrap();
        assert!(ids.insert(id));
        let case = source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        let config: Value = serde_json::from_str(case["configJson"].as_str().unwrap()).unwrap();
        assert_eq!(config["envelopeVersion"], family["envelopeVersion"]);
        let holding = config["embargo"]["holdingAllowanceBars"].as_i64().unwrap();
        assert_eq!(holding, 10);
        let candidates = case["candidates"].as_array().unwrap();
        let expected = family["candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), expected.len());
        for (i, (candidate, expected)) in candidates.iter().zip(expected).enumerate() {
            assert_eq!(candidate["index"], i);
            assert_eq!(expected["candidateIndex"], candidate["index"]);
            assert_eq!(expected["strategyHash"], candidate["strategyHash"]);
            let s = &candidate["freshStrategy"];
            assert_eq!(s["strategy_hash"], expected["strategyHash"]);
            assert_eq!(s["type"], expected["mode"]);
            let raw = s["original_definition_json"].as_str().unwrap();
            let definition: Value = serde_json::from_str(raw).unwrap();
            assert_eq!(definition["mode"], expected["mode"]);
            check(
                id,
                raw,
                holding,
                &json!({"status":"ok", "derivation":expected["expected"]}),
            );
            members += 1;
        }
    }
    assert_eq!(members, expected_members, "complete referenced families");
}
#[test]
fn warmup_all_original_envelope_families_match_literal_projections() {
    family_controls("original-producer-projections-v1.json", ORIGINAL, 9);
}
#[test]
fn warmup_all_params_and_mixed_mode_families_match_literal_projections() {
    family_controls("original-producer-mode-coverage-v1.json", MODES, 14);
}
