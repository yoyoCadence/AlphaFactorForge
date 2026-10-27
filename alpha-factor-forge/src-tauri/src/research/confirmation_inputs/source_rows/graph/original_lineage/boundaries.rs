use super::*;
use alpha_factor_forge::discovery_core::{
    config::parse_discovery_config, enumerate::enumerate_candidates,
};

// Test-owned input shaping, not the adapter's accounting or expected output.
fn selected_documents(t: &mut Value, mut edit: impl FnMut(&mut Value)) {
    for (name, key) in [
        ("strategy_def", "original_definition_json"),
        ("strategy_def", "dsl_json"),
        ("research_attempts", "input_fingerprint_json"),
        ("research_attempts", "engine_fingerprint_json"),
        ("hypotheses", "content_json"),
        ("hypotheses", "applicability_json"),
    ] {
        let tab = table(t, name);
        let index = tab["columns"]
            .as_array()
            .unwrap()
            .iter()
            .position(|c| c == key)
            .unwrap();
        for row in tab["rows"].as_array_mut().unwrap() {
            if !row[index].is_null() {
                edit(&mut row[index]);
            }
        }
    }
}
pub(super) fn byte_tables(target: usize) -> Value {
    let mut t = tables("v1-ordered-axes");
    let config = &mut table(&mut t, "discovery_runs")["rows"][0][2];
    let mut raw = config.as_str().unwrap().to_string();
    let old = raw.len();
    assert_eq!(old, 1928);
    raw.extend(std::iter::repeat_n(' ', 1024 * 1024 - old));
    *config = json!(raw);
    let mut remaining = target - (9704 + 2 * (1024 * 1024 - old));
    selected_documents(&mut t, |v| {
        let mut raw = v.as_str().unwrap().to_string();
        let add = remaining.min(4 * 1024 * 1024 - raw.len());
        raw.extend(std::iter::repeat_n(' ', add));
        *v = json!(raw);
        remaining -= add;
    });
    assert_eq!(remaining, 0);
    t
}
// Original entryRules has 5 nodes (array, object, three strings); new has 4097.
const RULE_NODE_DELTA: usize = 4092;

pub(super) fn node_tables(target: usize) -> Value {
    let mut t = tables("v1-ordered-axes");
    let config = &mut table(&mut t, "discovery_runs")["rows"][0][2];
    let mut raw: Value = serde_json::from_str(config.as_str().unwrap()).unwrap();
    raw["bases"][0]["strategy"]["entryRules"] = json!(vec![Value::Null; 4096]);
    *config = json!(serde_json::to_string(&raw).unwrap());
    // Boundary input remains a capturable family. New hashes are fixture setup
    // only; no semantic conformance is inferred for these padded raw claims.
    let resolved = parse_discovery_config(&raw, 64.0).unwrap();
    let plan = enumerate_candidates(&resolved).unwrap();
    for (row, candidate) in table(&mut t, "strategy_def")["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(plan.candidates)
    {
        row[4] = json!(candidate.strategy_hash);
    }
    let mut remaining = target - (461 + 2 * RULE_NODE_DELTA);
    selected_documents(&mut t, |v| {
        if remaining == 0 {
            return;
        }
        let old = OriginalJson::parse(v.as_str().unwrap().as_bytes())
            .unwrap()
            .node_count();
        let add = remaining.min(65536 - old);
        *v = json!(serde_json::to_string(&vec![Value::Null; old + add - 1]).unwrap());
        remaining -= add;
    });
    assert_eq!(remaining, 0);
    t
}
#[test]
fn real_captured_parse_bytes_exact_and_one_over_precede_materialization() {
    for delta in [0, 1] {
        let mut f = Fixture::new(&byte_tables(MAX_TOTAL_TEXT_BYTES + delta));
        let rows = f.capture();
        let before = entries();
        let _deny = Deny::new(true, true);
        if delta == 0 {
            let w = parse_work(&rows).unwrap();
            assert_eq!(w.bytes, MAX_TOTAL_TEXT_BYTES);
        } else {
            refused(prepare_original_lineage(&rows));
        }
        assert_eq!(entries(), before);
    }
}
#[test]
fn real_captured_combined_bytes_exact_and_one_over_precede_enumeration() {
    for delta in [0, 1] {
        let mut f = Fixture::new(&byte_tables(MAX_TOTAL_TEXT_BYTES - 1743 + delta));
        let rows = f.capture();
        let before = entries();
        if delta == 0 {
            let p = prepare_original_lineage(&rows).unwrap();
            assert_eq!(p.expansion_work().0, 1743);
            assert_eq!(p.combined_work().0, MAX_TOTAL_TEXT_BYTES);
            assert_eq!(entries(), (before.0 + 12, before.1 + 1));
        } else {
            let _deny = Deny::new(false, true);
            refused(prepare_original_lineage(&rows));
            assert_eq!(entries(), (before.0 + 1, before.1));
        }
    }
}
#[test]
fn real_captured_parse_nodes_exact_and_one_over_precede_materialization() {
    for delta in [0, 1] {
        let mut f = Fixture::new(&node_tables(MAX_TOTAL_JSON_NODES + delta));
        let rows = f.capture();
        let before = entries();
        let _deny = Deny::new(true, true);
        if delta == 0 {
            let w = parse_work(&rows).unwrap();
            assert_eq!(w.nodes, MAX_TOTAL_JSON_NODES);
        } else {
            refused(prepare_original_lineage(&rows));
        }
        assert_eq!(entries(), before);
    }
}
#[test]
fn real_captured_combined_nodes_exact_and_one_over_precede_enumeration() {
    for delta in [0, 1] {
        let mut f = Fixture::new(&node_tables(
            MAX_TOTAL_JSON_NODES - (120 + 3 * RULE_NODE_DELTA) + delta,
        ));
        let rows = f.capture();
        let before = entries();
        if delta == 0 {
            let p = prepare_original_lineage(&rows).unwrap();
            assert_eq!(p.expansion_work().1, 12396);
            assert_eq!(p.combined_work().1, MAX_TOTAL_JSON_NODES);
            assert_eq!(entries(), (before.0 + 12, before.1 + 1));
        } else {
            let _deny = Deny::new(false, true);
            refused(prepare_original_lineage(&rows));
            assert_eq!(entries(), (before.0 + 1, before.1));
        }
    }
}
#[test]
fn whole_work_checked_addition_is_atomic_with_nonempty_prior_usage() {
    let mut work = Work {
        bytes: 128,
        nodes: 17,
    };
    work.add(1, MAX_TOTAL_TEXT_BYTES - 128, MAX_TOTAL_JSON_NODES - 17)
        .unwrap();
    let before = work;
    for (weight, bytes, nodes) in [
        (1, 1, 0),
        (1, 0, 1),
        (usize::MAX, 2, 0),
        (usize::MAX, 0, 2),
        (1, usize::MAX, 0),
        (1, 0, usize::MAX),
    ] {
        refused(work.add(weight, bytes, nodes));
        assert!(work == before);
    }
}
#[test]
fn initial_and_dataset_refusal_stay_before_parse_or_enumeration() {
    let mut f = Fixture::new(&tables("v1-ordered-axes"));
    let rows = f.capture();
    let before = entries();
    {
        let _deny = Deny::new(true, true);
        assert!(selection::prepare_run_plan(
            &rows.roots.run.config_json,
            &rows.dataset.dataset_hash,
            MAX_TOTAL_TEXT_BYTES + 1,
            0
        )
        .is_err());
        assert!(selection::prepare_run_plan(
            &rows.roots.run.config_json,
            &rows.dataset.dataset_hash,
            0,
            MAX_TOTAL_JSON_NODES + 1
        )
        .is_err());
        assert_eq!(entries(), before);
    }
    let _deny = Deny::new(false, true);
    assert!(selection::prepare_run_plan(
        &rows.roots.run.config_json,
        "dataset-content-v2:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        0,
        0
    )
    .is_err());
    assert_eq!(entries(), (before.0 + 1, before.1));
}
