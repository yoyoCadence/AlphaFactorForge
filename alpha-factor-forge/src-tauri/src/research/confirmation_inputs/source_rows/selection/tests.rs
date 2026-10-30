use super::super::super::tests::{snapshot, Fixture};
use super::super::{capture_root_rows_in, SourceLocator};
use super::*;
use alpha_factor_forge::discovery_core::config::{AxisKey, DiscoveryAxis};
use rusqlite::params;
use serde_json::json;

fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../../fixtures/research/source-selection-v1.json"
    ))
    .unwrap()
}

fn parity() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../../fixtures/rs-core/runner-config-v1.json"
    ))
    .unwrap()
}

fn captured(raw: &str, declaration: &Value, version: &str) -> (Fixture, RootRowsObservation) {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let id = declaration["campaignId"].as_str().unwrap();
    let document = declaration["documentJson"].as_str().unwrap();
    let instrument = declaration["instrumentId"].as_str().unwrap();
    let conn = fixture.connection();
    conn.execute("INSERT INTO discovery_runs(id,name,status,config_json) VALUES(1,'selection','completed',?1)",[raw]).unwrap();
    conn.execute(
        "INSERT INTO research_campaigns(campaign_id,version,document_json) VALUES(?1,?2,?3)",
        params![id, version, document],
    )
    .unwrap();
    conn.execute("INSERT INTO campaign_run_admissions(id,discovery_run_id,campaign_id,instrument_id,batch_id,status,report_version,report_json,fee_pct,slip_pct) VALUES(1,1,?1,?2,?3,'NOT_ELIGIBLE','uninterpreted-report','null',0.05,0.02)", params![id,instrument,format!("trial-batch-v1:{}","b".repeat(64))]).unwrap();
    let locator = SourceLocator::new(id, 1).unwrap();
    let tx = conn.transaction().unwrap();
    let roots = capture_root_rows_in(&tx, &expected, &locator).unwrap();
    tx.rollback().unwrap();
    (fixture, roots)
}

fn source(raw: &str) -> (Fixture, RootRowsObservation) {
    captured(
        raw,
        &oracle()["declarations"]["datasetA"],
        CAMPAIGN_DECLARATION_VERSION,
    )
}

fn refused<T>(result: AppResult<T>) {
    match result {
        Err(crate::error::AppError::Other(code)) => assert_eq!(code, "source_rows_invalid"),
        Err(other) => panic!("wrong refusal {other:?}"),
        Ok(_) => panic!("expected static source_rows_invalid"),
    }
}

fn assert_manifest(selection: &SourceSelection, expected: &Value) {
    let expected = expected.as_array().unwrap();
    assert_eq!(
        selection.candidates().len(),
        expected.len(),
        "complete manifest length"
    );
    for (actual, expected) in selection.candidates().iter().zip(expected) {
        assert_eq!(actual.index(), expected["index"].as_i64().unwrap());
        assert_eq!(
            actual.strategy_hash(),
            expected["strategyHash"].as_str().unwrap()
        );
    }
    assert_eq!(selection.dataset_id(), 7);
    assert_eq!(
        selection.dataset_hash(),
        format!("dataset-content-v2:{}", "a".repeat(64))
    );
    assert_eq!(selection.interval(), "1h");
    assert_eq!(selection.campaign_from_ms(), 1_735_689_600_000);
    assert_eq!(selection.campaign_to_ms(), 1_767_222_000_000);
}

#[test]
fn all_six_independent_saved_manifests_preserve_order_pruning_and_deduplication() {
    let fixtures = parity();
    let sizes = [3, 9, 1, 4, 4, 4];
    for (case, size) in fixtures["enumerationCases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(sizes)
    {
        let raw = serde_json::to_string(&case["input"]).unwrap();
        let (mut fixture, roots) = source(&raw);
        let before = roots.clone();
        let snapshot_before = snapshot(fixture.connection());
        let entries = ENUM_ENTRIES.with(std::cell::Cell::get);
        let selection = derive_selection(&roots).unwrap();
        assert_manifest(&selection, &case["expected"]["candidates"]);
        assert_eq!(selection.candidates().len(), size);
        assert_eq!(selection.snapshot_id(), "a".repeat(64));
        assert_eq!(ENUM_ENTRIES.with(std::cell::Cell::get), entries + 1);
        assert!(roots == before, "original roots are immutable");
        assert_eq!(
            snapshot(fixture.connection()),
            snapshot_before,
            "pure adapter changes no SQL state"
        );
    }
}

#[test]
fn independently_recorded_v2_v3_and_characterized_legacy_float_manifests_match() {
    let fixtures = oracle();
    for case in fixtures["cases"].as_array().unwrap() {
        let raw = case["configJson"].as_str().unwrap();
        let (_fixture, roots) = source(raw);
        let selection = derive_selection(&roots).unwrap();
        assert_manifest(&selection, &case["expected"]);
        assert_eq!(roots.run.config_json.as_bytes(), raw.as_bytes());
        if case["id"] == "legacy-long-float" {
            assert!(raw.contains("0.0036944444444444438"));
            let parsed: Value = serde_json::from_slice(roots.run.config_json.as_bytes()).unwrap();
            assert_eq!(
                parsed["bases"][0]["strategy"]["feePct"]
                    .as_f64()
                    .unwrap()
                    .to_bits(),
                0x3f6e43cfc21ad9ff
            );
            assert_eq!(case["characterizedLegacyFeeBits"], "3f6e43cfc21ad9ff");
        }
    }
}

#[test]
fn exact_matching_instrument_is_selected_instead_of_the_first_binding() {
    let fixtures = parity();
    let raw = serde_json::to_string(&fixtures["enumerationCases"][0]["input"]).unwrap();
    let declaration = &oracle()["declarations"]["secondInstrument"];
    assert_eq!(
        declaration["campaignId"],
        "9b5a546295b5abe09581319fb3360fbd1e0ab1fbe4680cf5ea692a07e782792b"
    );
    let (_fixture, roots) = captured(&raw, declaration, CAMPAIGN_DECLARATION_VERSION);
    let selection = derive_selection(&roots).unwrap();
    assert_eq!(selection.snapshot_id(), "c".repeat(64));
    assert_manifest(
        &selection,
        &fixtures["enumerationCases"][0]["expected"]["candidates"],
    );
}

#[test]
fn selected_campaign_range_retains_exact_safe_integers_not_first_binding_bounds() {
    let raw = serde_json::to_string(&parity()["enumerationCases"][0]["input"]).unwrap();
    for (from, to) in [(0, 0), (9_007_199_254_740_990i64, 9_007_199_254_740_991i64)] {
        let mut declaration = oracle()["declarations"]["secondInstrument"].clone();
        let mut document: Value =
            serde_json::from_str(declaration["documentJson"].as_str().unwrap()).unwrap();
        let selected = &mut document["instruments"][1];
        assert_eq!(selected["instrumentId"], declaration["instrumentId"]);
        selected["listedAtMs"] = json!(0);
        selected["fromMs"] = json!(from);
        selected["toMs"] = json!(to);
        // The original campaign writer supplies setup identity only. Expected
        // bounds are the independent integer literals above, not writer output.
        let frozen = freeze_campaign(&document).unwrap();
        declaration["campaignId"] = json!(frozen.campaign_id());
        declaration["documentJson"] = json!(serde_json::to_string_pretty(&document).unwrap());
        let (mut fixture, roots) = captured(&raw, &declaration, CAMPAIGN_DECLARATION_VERSION);
        let saved = roots.clone();
        let before = snapshot(fixture.connection());
        let parse = PARSE_ENTRIES.with(std::cell::Cell::get);
        let enumeration = ENUM_ENTRIES.with(std::cell::Cell::get);
        let selection = derive_selection(&roots).unwrap();
        assert_eq!(
            (selection.campaign_from_ms(), selection.campaign_to_ms()),
            (from, to)
        );
        assert_eq!(selection.snapshot_id(), "c".repeat(64));
        assert_ne!(selection.campaign_from_ms(), 1_735_689_600_000);
        assert_eq!(PARSE_ENTRIES.with(std::cell::Cell::get), parse + 1);
        assert_eq!(ENUM_ENTRIES.with(std::cell::Cell::get), enumeration + 1);
        assert!(roots == saved);
        assert_eq!(snapshot(fixture.connection()), before);
    }
}

#[test]
fn campaign_version_identity_membership_and_dataset_mismatches_refuse() {
    let raw = serde_json::to_string(&parity()["enumerationCases"][0]["input"]).unwrap();
    let declarations = oracle()["declarations"].clone();
    let (_fixture, roots) = captured(&raw, &declarations["datasetA"], "unsupported-v9");
    refused(derive_selection(&roots));
    let (_fixture, roots) = captured(
        &raw,
        &declarations["original"],
        CAMPAIGN_DECLARATION_VERSION,
    );
    refused(derive_selection(&roots)); // valid original campaign, wrong dataset
    for change in [
        "wrong-id",
        "missing-instrument",
        "duplicate-instrument",
        "invalid-version",
    ] {
        let mut declaration = declarations["datasetA"].clone();
        match change {
            "wrong-id" => declaration["campaignId"] = json!("b".repeat(64)),
            "missing-instrument" => {
                declaration["instrumentId"] = json!("crypto:binance:NOTDECLARED")
            }
            "duplicate-instrument" => {
                let mut document: Value =
                    serde_json::from_str(declaration["documentJson"].as_str().unwrap()).unwrap();
                let instrument = document["instruments"][0].clone();
                document["instruments"]
                    .as_array_mut()
                    .unwrap()
                    .push(instrument);
                declaration["documentJson"] = json!(serde_json::to_string(&document).unwrap());
            }
            _ => {
                let mut document: Value =
                    serde_json::from_str(declaration["documentJson"].as_str().unwrap()).unwrap();
                document["contractVersion"] = json!("unknown-v9");
                declaration["documentJson"] = json!(serde_json::to_string(&document).unwrap());
            }
        }
        let (_fixture, roots) = captured(&raw, &declaration, CAMPAIGN_DECLARATION_VERSION);
        refused(derive_selection(&roots));
    }
}

#[test]
fn unsupported_config_contracts_and_invalid_numeric_interpretation_refuse_statically() {
    let valid = parity()["configCases"][0]["input"].clone();
    for value in [
        json!(null),
        json!([]),
        json!({}),
        {
            let mut v = valid.clone();
            v["envelopeVersion"] = json!("unknown-v9");
            v
        },
        {
            let mut v = valid.clone();
            v["contracts"]["seed"] = json!("unknown-v9");
            v
        },
        {
            let mut v = valid;
            v["maxConcurrency"] = json!(-1);
            v
        },
    ] {
        let raw = serde_json::to_string(&value).unwrap();
        let (_fixture, roots) = source(&raw);
        refused(derive_selection(&roots));
    }
    let raw = oracle()["cases"][2]["configJson"]
        .as_str()
        .unwrap()
        .replace("0.0036944444444444438", "1e999");
    let (_fixture, roots) = source(&raw); // opaque valid JSON, unsupported Value number
    refused(derive_selection(&roots));
}

#[test]
fn parser_cores_are_deterministic_and_do_not_change_candidate_identity() {
    let case = &parity()["enumerationCases"][0];
    for concurrency in [
        Value::Null,
        json!(1),
        json!(4),
        json!(1024),
        json!(9_007_199_254_740_991_i64),
    ] {
        let mut input = case["input"].clone();
        input["maxConcurrency"] = concurrency;
        let raw = serde_json::to_string(&input).unwrap();
        let (_fixture, roots) = source(&raw);
        assert_manifest(
            &derive_selection(&roots).unwrap(),
            &case["expected"]["candidates"],
        );
        let small = parse_discovery_config(&input, parser_cores(&input).unwrap()).unwrap();
        let large = parse_discovery_config(&input, JS_MAX_SAFE_INTEGER).unwrap();
        let a = enumerate_candidates(&small).unwrap();
        let b = enumerate_candidates(&large).unwrap();
        assert_eq!(a, b, "core choice changes performance only");
    }
    for raw in [
        json!({}),
        json!({"maxConcurrency":0}),
        json!({"maxConcurrency":1.5}),
        json!({"maxConcurrency":"4"}),
        json!({"maxConcurrency":9_007_199_254_740_992_i64}),
    ] {
        refused(parser_cores(&raw));
    }
}

#[test]
fn all_pruned_and_raw_over_cap_configs_refuse_without_returning_a_subset() {
    for case in parity()["enumerationErrorCases"].as_array().unwrap() {
        let raw = serde_json::to_string(&case["input"]).unwrap();
        let (_fixture, roots) = source(&raw);
        refused(derive_selection(&roots));
    }
}

fn minimal_config() -> ResolvedDiscoveryConfig {
    let input = parity()["configCases"][0]["input"].clone();
    let mut config = parse_discovery_config(&input, 8.0).unwrap();
    config.bases.truncate(1);
    config.bases[0].id = "a".into();
    config.bases[0].strategy = json!({"x":1});
    config.bases[0].axes.clear();
    config
}

#[test]
fn scratch_formula_has_independent_exact_bytes_nodes_and_nonempty_base_totals() {
    let mut config = minimal_config();
    let usage = scratch_preflight(&config).unwrap();
    assert_eq!((usage.bytes, usage.nodes), (12, 4)); // S=7,T=2,I=3,A=0,p=1
    config.bases[0].axes.push(DiscoveryAxis {
        key: AxisKey::SlPct,
        min: 1.0,
        max: 3.0,
        step: 1.0,
    });
    let usage = scratch_preflight(&config).unwrap();
    assert_eq!((usage.bytes, usage.nodes), (114, 18)); // K=7,V=3,A=13,B=38,Q=6,p=3
    let mut another = config.bases[0].clone();
    another.axes.clear();
    config.bases.push(another);
    let usage = scratch_preflight(&config).unwrap();
    assert_eq!((usage.bytes, usage.nodes), (126, 22));
}

#[test]
fn scratch_exact_one_over_and_overflow_are_atomic_with_nonempty_prior_usage() {
    let mut usage = ScratchUsage {
        bytes: 128,
        nodes: 17,
    };
    usage
        .add_product(1, 33_554_432 - 128, 262_144 - 17)
        .unwrap();
    let before = usage.clone();
    for (p, b, n) in [
        (1, 1, 0),
        (1, 0, 1),
        (usize::MAX, 2, 0),
        (usize::MAX, 0, 2),
        (1, usize::MAX, 0),
        (1, 0, usize::MAX),
    ] {
        refused(usage.add_product(p, b, n));
        assert!(usage == before);
    }
    assert_eq!((usage.bytes, usage.nodes), (33_554_432, 262_144));
}

#[test]
fn serializer_counts_escaped_utf8_and_floats_without_retaining_output() {
    assert_eq!(serialized_len(&"é").unwrap(), 4);
    assert_eq!(serialized_len(&1.0_f64).unwrap(), 3);
    let value = json!({"x":null,"y":[true,false]});
    assert_eq!(
        serialized_len(&value).unwrap(),
        br#"{"x":null,"y":[true,false]}"#.len()
    );
    assert_eq!(value_nodes(&value).unwrap(), 5);
    let mut count = ByteCount(33_554_431);
    assert_eq!(count.write(b"a").unwrap(), 1);
    assert!(count.write(b"a").is_err());
    assert_eq!(count.0, 33_554_432);
    let mut count = ByteCount(usize::MAX);
    assert!(count.write(b"a").is_err());
    assert_eq!(count.0, usize::MAX);
}

struct NoEnumeration;
impl NoEnumeration {
    fn new() -> Self {
        DENY_ENUM.with(|flag| flag.set(true));
        Self
    }
}
impl Drop for NoEnumeration {
    fn drop(&mut self) {
        DENY_ENUM.with(|flag| flag.set(false));
    }
}

#[test]
fn oversized_raw_payload_is_refused_before_cartesian_entry_even_if_many_candidates_prune() {
    let mut input = parity()["configCases"][0]["input"].clone();
    input["caps"]["candidates"] = json!(4096);
    input["bases"][0]["id"] = json!("a".repeat(100_000));
    input["bases"][0]["axes"] = json!([
        {"key":"fastMA","min":2,"max":65,"step":1},
        {"key":"slowMA","min":3,"max":66,"step":1}
    ]);
    let raw = serde_json::to_string(&input).unwrap();
    let (_fixture, roots) = source(&raw);
    let entries = ENUM_ENTRIES.with(std::cell::Cell::get);
    let _guard = NoEnumeration::new();
    refused(derive_selection(&roots));
    assert_eq!(
        ENUM_ENTRIES.with(std::cell::Cell::get),
        entries,
        "no Cartesian call before byte refusal"
    );
}

#[test]
fn scratch_raw_product_and_node_caps_refuse_before_deduplication() {
    let mut config = minimal_config();
    config.caps.candidates = 1;
    config.bases[0].axes.push(DiscoveryAxis {
        key: AxisKey::SlPct,
        min: 1.0,
        max: 2.0,
        step: 1.0,
    });
    refused(scratch_preflight(&config));
    let mut config = minimal_config();
    config.caps.candidates = 4096;
    config.bases[0].strategy = Value::Array(vec![Value::Null; 65_535]);
    config.bases[0].axes.push(DiscoveryAxis {
        key: AxisKey::SlPct,
        min: 1.0,
        max: 4.0,
        step: 1.0,
    });
    refused(scratch_preflight(&config)); // 4*(65,536+4) > joint nodes
}

#[test]
fn exact_raw_product_4096_and_one_additional_base_use_checked_whole_config_count() {
    let mut config = minimal_config();
    config.caps.candidates = 4096;
    config.bases[0].axes = vec![
        DiscoveryAxis {
            key: AxisKey::SlPct,
            min: 1.0,
            max: 64.0,
            step: 1.0,
        },
        DiscoveryAxis {
            key: AxisKey::TpPct,
            min: 1.0,
            max: 64.0,
            step: 1.0,
        },
    ];
    scratch_preflight(&config).unwrap(); // independent 64*64=4096
    let mut extra = config.bases[0].clone();
    extra.axes.clear();
    config.bases.push(extra);
    refused(scratch_preflight(&config)); // 4096+1, never deduplicate first
}
