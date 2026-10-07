use super::*;
use crate::db::repositories::{self, StrategyDef};
use crate::identity::{canonical_bytes, strategy_hash_from_definition_json};

const AUDIT_INPUT: &str = include_str!("../../../../fixtures/research/numeric-json-audit-v1.json");
const LEGACY_REPORT: &str =
    include_str!("../../../../fixtures/research/numeric-json-default-output.json");
const MANUAL_FIXTURE: &str =
    include_str!("../../../../fixtures/research/numeric-json-manual-v1.json");

fn marked(text: &str) -> String {
    format!(
        "{{\"definitionVersion\":\"{MANUAL_DEFINITION_VERSION}\",\"numericPolicy\":\"{ROUNDED_NUMERIC_POLICY}\",{}",
        &text[1..]
    )
}

fn row(text: &str) -> StrategyDef {
    StrategyDef {
        id: None,
        name: "numeric policy fixture".into(),
        kind: "params".into(),
        dsl_json: None,
        original_definition_json: text.into(),
        param_schema_json: None,
        source: "manual".into(),
        ai_prompt_hash: None,
        strategy_hash: strategy_hash_from_definition_json(text).unwrap(),
        lifecycle: "candidate".into(),
        parent_strategy_id: None,
    }
}

fn database() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::apply_migrations(&conn).unwrap();
    conn
}

#[test]
fn both_policies_replay_all_six_independent_numeric_controls() {
    let fixture: Value = serde_json::from_str(AUDIT_INPUT).unwrap();
    let legacy: Value = serde_json::from_str(LEGACY_REPORT).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for (case, prior) in cases.iter().zip(legacy["cases"].as_array().unwrap()) {
        let text = case["definitionJson"].as_str().unwrap();
        let (policy, old) = parse_definition_json(text).unwrap();
        assert_eq!(policy, DefinitionPolicy::Legacy);
        assert_eq!(
            format!("{:016x}", old["feePct"].as_f64().unwrap().to_bits()),
            prior["parsedBits"]
        );
        assert_eq!(
            strategy_hash_from_definition_json(text).unwrap(),
            prior["strategyHash"]
        );

        let new_text = marked(text);
        let (policy, mut new) = parse_definition_json(&new_text).unwrap();
        assert_eq!(policy, DefinitionPolicy::Rounded);
        assert_eq!(
            format!("{:016x}", new["feePct"].as_f64().unwrap().to_bits()),
            case["expectedJsBits"]
        );
        new.as_object_mut().unwrap().remove("definitionVersion");
        new.as_object_mut().unwrap().remove("numericPolicy");
        assert_eq!(
            hex::encode(canonical_bytes(&new).unwrap()),
            case["expectedJsCanonical"]
        );
        assert_ne!(
            strategy_hash_from_definition_json(&new_text).unwrap(),
            prior["strategyHash"],
            "policy markers are part of the frozen identity, even when the number agrees"
        );
    }
}

#[test]
fn marked_numbers_preserve_nested_values_signed_zero_and_json_strings() {
    let text = marked(
        r#"{"mode":"params","feePct":0,"slipPct":0,"nested":[{"x":0.0036944444444444438},-0,true,null,"0.0036944444444444438", "\u4f60\""],"integer":9007199254740991}"#,
    );
    let (_, value) = parse_definition_json(&text).unwrap();
    assert_eq!(
        value["nested"][0]["x"].as_f64().unwrap().to_bits(),
        0x3f6e43cfc21ad9fe
    );
    assert_eq!(
        value["nested"][1].as_f64().unwrap().to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_eq!(value["nested"][4], "0.0036944444444444438");
    assert_eq!(value["nested"][5], "你\"");
    assert_eq!(value["integer"].as_u64(), Some(9_007_199_254_740_991));
}

#[test]
fn unknown_partial_or_malformed_markers_never_guess_a_policy() {
    for text in [
        r#"{"definitionVersion":"manual-strategy-definition-v1"}"#,
        r#"{"numericPolicy":"json-f64-roundtrip-v1"}"#,
        r#"{"definitionVersion":"unknown","numericPolicy":"json-f64-roundtrip-v1"}"#,
        r#"{"definitionVersion":"manual-strategy-definition-v1","numericPolicy":"unknown"}"#,
        r#"{"definitionVersion":null,"numericPolicy":"json-f64-roundtrip-v1"}"#,
        r#"{"definitionVersion":"manual-strategy-definition-v1","numericPolicy":42}"#,
    ] {
        assert!(parse_definition_json(text).is_err(), "{text}");
    }
}

#[test]
fn new_policy_refuses_ambiguous_keys_nonfinite_numbers_and_excessive_depth() {
    let legacy = r#"{"mode":"params","feePct":1,"feePct":2,"slipPct":0}"#;
    assert_eq!(parse_definition_json(legacy).unwrap().1["feePct"], 2);
    for text in [
        marked(legacy),
        marked(r#"{"mode":"params","feePct":0,"slipPct":0,"nested":{"x":1,"x":2}}"#),
        marked(r#"{"mode":"params","feePct":1e400,"slipPct":0}"#),
        marked(
            r#"{"mode":"params","feePct":0,"slipPct":0,"definitionVersion":"manual-strategy-definition-v1"}"#,
        ),
    ] {
        assert!(parse_definition_json(&text).is_err(), "{text}");
    }
    let nested = format!(
        "{}0{}",
        "[".repeat(MAX_DEPTH + 2),
        "]".repeat(MAX_DEPTH + 2)
    );
    assert!(parse_definition_json(&marked(&format!("{{\"nested\":{nested}}}"))).is_err());
}

#[test]
fn saves_and_prepares_both_policies_without_rewriting_source_rows() {
    let conn = database();
    let fixture: Value = serde_json::from_str(AUDIT_INPUT).unwrap();
    let legacy: Value = serde_json::from_str(LEGACY_REPORT).unwrap();
    for (case, prior) in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(legacy["cases"].as_array().unwrap())
    {
        for (text, policy, bits) in [
            (
                case["definitionJson"].as_str().unwrap().to_string(),
                LEGACY_NUMERIC_POLICY,
                &prior["parsedBits"],
            ),
            (
                marked(case["definitionJson"].as_str().unwrap()),
                ROUNDED_NUMERIC_POLICY,
                &case["expectedJsBits"],
            ),
        ] {
            let source = row(&text);
            let id = repositories::insert_verified_strategy(&conn, &source).unwrap();
            let prepared = repositories::prepare_saved_strategy(&conn, id).unwrap();
            assert_eq!(prepared.source_strategy_id, id);
            assert_eq!(prepared.source_strategy_hash, source.strategy_hash);
            assert_eq!(prepared.numeric_policy, policy);
            // JS reads serialized binary64 with correctly rounded JSON.parse.
            let interpreted = parse_rounded(&prepared.interpreted_definition_json, 0).unwrap();
            assert_eq!(
                format!("{:016x}", interpreted["feePct"].as_f64().unwrap().to_bits()),
                *bits
            );
            let saved = repositories::get_strategy_by_id(&conn, id).unwrap();
            assert_eq!(saved.original_definition_json, text);
            assert_eq!(saved.strategy_hash, source.strategy_hash);
        }
    }
}

#[test]
fn frontend_marked_hashes_save_and_load_exactly_in_all_manual_modes() {
    let conn = database();
    let fixture: Value = serde_json::from_str(MANUAL_FIXTURE).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 18);
    let mut inventory = std::collections::BTreeSet::new();
    for case in cases {
        assert!(inventory.insert(case["id"].as_str().unwrap()));
        let text = case["definitionJson"].as_str().unwrap();
        let (_, definition) = parse_definition_json(text).unwrap();
        assert_eq!(
            hex::encode(canonical_bytes(&definition).unwrap()),
            case["expectedJsCanonical"]
        );
        assert_eq!(
            format!("{:016x}", definition["feePct"].as_f64().unwrap().to_bits()),
            case["expectedJsBits"]
        );
        let mut strategy = row(text);
        strategy.kind = definition["mode"].as_str().unwrap().into();
        strategy.strategy_hash = case["expectedJsStrategyHash"].as_str().unwrap().into();
        let id = repositories::insert_verified_strategy(&conn, &strategy).unwrap();
        let prepared = repositories::prepare_saved_strategy(&conn, id).unwrap();
        assert_eq!(prepared.source_strategy_hash, strategy.strategy_hash);
        assert_eq!(prepared.numeric_policy, ROUNDED_NUMERIC_POLICY);
        assert_eq!(
            parse_definition_json(&prepared.interpreted_definition_json)
                .unwrap()
                .1,
            definition
        );
        assert_eq!(
            repositories::get_strategy_by_id(&conn, id)
                .unwrap()
                .original_definition_json,
            text
        );
    }
    assert_eq!(repositories::list_strategies(&conn).unwrap().len(), 18);
}

#[test]
fn copy_lineage_is_verified_and_conflicting_existing_sources_are_refused() {
    let conn = database();
    let original =
        r#"{"mode":"params","feePct":0.0036944444444444438,"slipPct":0,"fastMA":5,"slowMA":20}"#;
    let source = row(original);
    let source_id = repositories::insert_verified_strategy(&conn, &source).unwrap();
    let second = row(r#"{"mode":"params","feePct":0.1,"slipPct":0,"fastMA":5,"slowMA":20}"#);
    let second_id = repositories::insert_verified_strategy(&conn, &second).unwrap();
    let mut copy = row(&marked(original));
    copy.parent_strategy_id = Some(source_id);
    let child_id = repositories::insert_verified_strategy(&conn, &copy).unwrap();
    assert_eq!(
        repositories::get_strategy_by_id(&conn, child_id)
            .unwrap()
            .parent_strategy_id,
        Some(source_id)
    );
    assert_eq!(
        repositories::insert_verified_strategy(&conn, &copy).unwrap(),
        child_id
    );

    copy.parent_strategy_id = Some(second_id);
    copy.name = "should not overwrite".into();
    assert!(repositories::insert_verified_strategy(&conn, &copy)
        .unwrap_err()
        .to_string()
        .contains("different source lineage"));
    copy.parent_strategy_id = Some(child_id);
    assert!(repositories::insert_verified_strategy(&conn, &copy)
        .unwrap_err()
        .to_string()
        .contains("itself"));
    assert_eq!(
        repositories::get_strategy_by_id(&conn, child_id)
            .unwrap()
            .name,
        "numeric policy fixture"
    );
    assert_eq!(
        repositories::get_strategy_by_id(&conn, source_id)
            .unwrap()
            .original_definition_json,
        original
    );

    // An ordinary marked save already occupies the same identity with no
    // source. A later copy cannot silently attach a different lineage to it.
    let mut independent = row(&marked(&second.original_definition_json));
    let independent_id = repositories::insert_verified_strategy(&conn, &independent).unwrap();
    independent.parent_strategy_id = Some(second_id);
    assert!(repositories::insert_verified_strategy(&conn, &independent).is_err());
    assert_eq!(
        repositories::get_strategy_by_id(&conn, independent_id)
            .unwrap()
            .parent_strategy_id,
        None
    );
}

#[test]
fn unresolved_sources_cannot_be_prepared_or_used_to_authorize_a_copy() {
    let conn = database();
    let source = row(r#"{"mode":"params","feePct":0.1,"slipPct":0}"#);
    let id = repositories::insert_verified_strategy(&conn, &source).unwrap();
    conn.execute("UPDATE strategy_def SET original_definition_json = '{\"mode\":\"params\",\"feePct\":0.2,\"slipPct\":0}' WHERE id = ?1", [id]).unwrap();
    assert!(repositories::prepare_saved_strategy(&conn, id).is_err());
    assert_eq!(
        repositories::list_strategies(&conn).unwrap().len(),
        1,
        "unresolved history stays visible"
    );
    let mut child = row(&marked(&source.original_definition_json));
    child.parent_strategy_id = Some(id);
    assert!(repositories::insert_verified_strategy(&conn, &child).is_err());
    assert_eq!(
        repositories::list_strategies(&conn).unwrap().len(),
        1,
        "refused copy writes nothing"
    );
    assert!(repositories::prepare_saved_strategy(&conn, 404)
        .unwrap_err()
        .to_string()
        .contains("not found"));
}
