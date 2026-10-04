//! NUMERIC-JSON-001: characterize the current parser at real product boundaries.
//! These are audit regressions, not approval of lossy parsing. A parser repair
//! must replace the legacy expectation explicitly and preserve old identities.
use super::*;
use crate::db::repositories::{insert_verified_strategy, list_strategies};
use crate::research::{canonical_json, sha256_hex};
use rusqlite::Connection;
use serde_json::json;

const INPUT: &str = include_str!("../../../fixtures/research/numeric-json-audit-v1.json");
const DEFAULT: &str = include_str!("../../../fixtures/research/numeric-json-default-output.json");

#[test]
fn numeric_json_audit_replays_the_default_parser_and_product_hashes() {
    let input: Value = serde_json::from_str(INPUT).unwrap();
    let evidence: Value = serde_json::from_str(DEFAULT).unwrap();
    for (case, report) in input["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(evidence["cases"].as_array().unwrap())
    {
        assert_eq!(case["id"], report["id"]);
        let literal = case["literal"].as_str().unwrap();
        let parsed: f64 = serde_json::from_str(literal).unwrap();
        assert_eq!(format!("{:016x}", parsed.to_bits()), report["parsedBits"]);
        let standard: f64 = literal.parse().unwrap();
        assert_eq!(
            format!("{:016x}", standard.to_bits()),
            case["expectedJsBits"]
        );
        let definition = case["definitionJson"].as_str().unwrap();
        assert_eq!(
            strategy_hash_from_definition_json(definition).unwrap(),
            report["strategyHash"]
        );
        let value: Value = serde_json::from_str(definition).unwrap();
        assert_eq!(
            hex::encode(canonical_bytes(&value).unwrap()),
            report["canonical"]
        );
    }
}

#[test]
fn numeric_json_audit_save_refuses_the_frontend_hash_and_preserves_legacy_text() {
    let input: Value = serde_json::from_str(INPUT).unwrap();
    let evidence: Value = serde_json::from_str(DEFAULT).unwrap();
    let conn = Connection::open_in_memory().unwrap();
    crate::db::apply_migrations(&conn).unwrap();
    for (case, report) in input["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(evidence["cases"].as_array().unwrap())
    {
        let mut strategy = StrategyDef {
            id: None,
            name: case["id"].as_str().unwrap().into(),
            kind: "params".into(),
            dsl_json: None,
            original_definition_json: case["definitionJson"].as_str().unwrap().into(),
            param_schema_json: None,
            source: "manual".into(),
            ai_prompt_hash: None,
            strategy_hash: case["expectedJsStrategyHash"].as_str().unwrap().into(),
            lifecycle: "candidate".into(),
            parent_strategy_id: None,
        };
        let differs = case["expectedJsStrategyHash"] != report["strategyHash"];
        let before = list_strategies(&conn).unwrap().len();
        let result = insert_verified_strategy(&conn, &strategy);
        assert_eq!(result.is_err(), differs, "{}", case["id"]);
        if differs {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("identity mismatch"));
            assert_eq!(
                list_strategies(&conn).unwrap().len(),
                before,
                "failed save writes nothing"
            );
        }
        strategy.strategy_hash = report["strategyHash"].as_str().unwrap().into();
        let id = insert_verified_strategy(&conn, &strategy).unwrap();
        let saved = list_strategies(&conn)
            .unwrap()
            .into_iter()
            .find(|row| row.id == Some(id))
            .unwrap();
        assert_eq!(
            saved.original_definition_json,
            strategy.original_definition_json
        );
        verify_strategy_identity(&saved).unwrap();
        // Same legacy row would fail under the correctly rounded interpretation.
        assert_eq!(
            saved.strategy_hash != case["expectedJsStrategyHash"],
            differs
        );
    }
}

#[test]
fn numeric_json_audit_rehashing_research_json_changes_the_content_identity() {
    let standard: f64 = "0.0036944444444444438".parse().unwrap();
    let original = json!({"cost": standard});
    let bytes = canonical_json(&original).unwrap();
    let reread: Value = serde_json::from_slice(&bytes).unwrap();
    assert_ne!(
        original["cost"].as_f64().unwrap().to_bits(),
        reread["cost"].as_f64().unwrap().to_bits()
    );
    assert_ne!(
        canonical_bytes(&original).unwrap(),
        canonical_bytes(&reread).unwrap()
    );
    assert_ne!(
        sha256_hex(&bytes),
        sha256_hex(&canonical_json(&reread).unwrap())
    );
    // Exercise the actual byte store: reading proves the original checksum
    // before parsing, even though reconstructing canonical JSON changes it.
    let temporary = std::env::temp_dir().join(format!(
        "aff-numeric-audit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let store = crate::research::artifacts::ArtifactStore::in_data_dir(&temporary);
    let reference = store.put("candidate-result-v1", &bytes).unwrap();
    let verified_bytes = store.read(&reference).unwrap();
    assert_eq!(verified_bytes, bytes);
    assert_ne!(
        reference.sha256,
        sha256_hex(&canonical_json(&reread).unwrap())
    );
    std::fs::remove_dir_all(&temporary).unwrap();
}
