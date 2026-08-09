//! DATA-QUALITY-001 parity: the Rust market-data admission validator is checked
//! against the same committed accept/reject matrix the vitest suite reads.
//!
//! The fixture is authored from the adjudicated specification rather than
//! recorded from either runtime, so agreeing with it is a real constraint on
//! both sides and not a restatement of whichever validator was written first.
//! Every leaf compares exactly (`expectedNumericPolicy: exact-v1`): admission is
//! a classification, so there is no tolerance to spend.

use serde_json::Value;

use super::market_data::{
    first_issue, CandleFields, MARKET_DATA_QUALITY_VERSION, MARKET_DATA_RULE_IDS,
    MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE, MIN_MARKET_TIMESTAMP_MS,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/rs-core/market-data-quality-v1.json"
    ))
    .expect("market-data quality fixture parses")
}

/// Decode one input leaf, honouring `explicit-numeric-status-v1` tags for the
/// non-finite values JSON cannot hold.
fn decode_number(value: &Value, path: &str) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().unwrap_or_else(|| panic!("{path} is a f64")),
        Value::String(tag) => match tag.as_str() {
            "nan" => f64::NAN,
            "positive_infinity" => f64::INFINITY,
            "negative_infinity" => f64::NEG_INFINITY,
            other => panic!("{path}: unknown numeric tag {other}"),
        },
        other => panic!("{path}: unsupported numeric leaf {other}"),
    }
}

fn decode_candle(value: &Value, path: &str) -> CandleFields {
    let field = |name: &str| decode_number(&value[name], &format!("{path}.{name}"));
    CandleFields {
        timestamp: field("timestamp"),
        open: field("open"),
        high: field("high"),
        low: field("low"),
        close: field("close"),
        volume: field("volume"),
    }
}

#[test]
fn envelope_constants_and_rule_inventory_match_the_fixture() {
    let fixture = fixture();
    assert_eq!(fixture["schemaVersion"], "rs-core-parity-fixture-v1");
    assert_eq!(fixture["fixtureVersion"], "market-data-quality-parity-v1");
    assert_eq!(
        fixture["contracts"]["marketDataQuality"],
        MARKET_DATA_QUALITY_VERSION
    );
    assert_eq!(fixture["contracts"]["candle"], super::types::CANDLE_CONTRACT_VERSION);
    assert_eq!(
        fixture["constants"]["minTimestampMs"].as_i64(),
        Some(MIN_MARKET_TIMESTAMP_MS)
    );
    assert_eq!(
        fixture["constants"]["maxTimestampMsExclusive"].as_i64(),
        Some(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE)
    );

    let rule_ids: Vec<&str> = fixture["ruleIds"]
        .as_array()
        .expect("ruleIds array")
        .iter()
        .map(|value| value.as_str().expect("rule id string"))
        .collect();
    assert_eq!(rule_ids, MARKET_DATA_RULE_IDS.to_vec());
    assert_eq!(
        fixture["unreachableRuleIds"]
            .as_array()
            .expect("unreachableRuleIds array")
            .len(),
        1,
        "rule 3 is the only unreachable rule"
    );
}

#[test]
fn every_fixture_row_is_classified_exactly_as_specified() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().expect("cases array");
    assert!(!cases.is_empty(), "fixture must hold cases");

    for case in cases {
        let id = case["id"].as_str().expect("case id");
        let rows: Vec<CandleFields> = case["candles"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: candles array"))
            .iter()
            .enumerate()
            .map(|(index, candle)| decode_candle(candle, &format!("{id}.candles[{index}]")))
            .collect();
        let issue = first_issue(rows);
        let expected = &case["expected"];

        if expected["accepted"] == Value::Bool(true) {
            assert!(
                issue.is_none(),
                "{id} must be admitted, got {:?}",
                issue.map(|issue| issue.to_string())
            );
            continue;
        }

        let issue = issue.unwrap_or_else(|| panic!("{id} must be rejected"));
        assert_eq!(
            issue.rule.as_str(),
            expected["rule"].as_str().expect("expected rule id"),
            "{id}: rule id"
        );
        assert_eq!(
            issue.index as u64,
            expected["index"].as_u64().expect("expected index"),
            "{id}: failing candle index"
        );
    }
}

#[test]
fn every_reachable_rule_id_has_a_rejection_row() {
    let fixture = fixture();
    let mut observed: Vec<String> = Vec::new();
    for case in fixture["cases"].as_array().expect("cases array") {
        if case["expected"]["accepted"] == Value::Bool(true) {
            continue;
        }
        let rows: Vec<CandleFields> = case["candles"]
            .as_array()
            .expect("candles array")
            .iter()
            .enumerate()
            .map(|(index, candle)| decode_candle(candle, &format!("case.candles[{index}]")))
            .collect();
        let issue = first_issue(rows).expect("rejected case must produce an issue");
        let rule = issue.rule.as_str().to_string();
        if !observed.contains(&rule) {
            observed.push(rule);
        }
    }
    for rule in fixture["reachableRuleIds"]
        .as_array()
        .expect("reachableRuleIds array")
    {
        let rule = rule.as_str().expect("reachable rule id");
        assert!(
            observed.iter().any(|seen| seen == rule),
            "no rejection row produced {rule}"
        );
    }
    assert!(
        !observed.iter().any(|seen| seen == "timestamp_not_representable"),
        "rule 3 is unreachable and must not be produced by any fixture row"
    );
}
