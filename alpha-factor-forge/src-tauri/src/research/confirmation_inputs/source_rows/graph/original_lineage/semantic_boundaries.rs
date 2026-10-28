use super::super::semantics::{self, DENY_PAYLOAD, PAYLOAD_ENTRIES};
use super::semantic_controls::{observation, oracle, work};
use super::*;
use crate::{db::repositories, research::history};
use alpha_factor_forge::discovery_core::config::parse_discovery_config;

// Actual original candidate/repository/history setup only, no candles, jobs run,
// registry expenditure or trial execution. Only the tests' syntactic job IDs.
pub(super) fn produced(raw: &Value) -> Fixture {
    let mut t = tables("v1-ordered-axes");
    table(&mut t, "discovery_runs")["rows"][0][2] = json!(serde_json::to_string(raw).unwrap());
    for name in [
        "strategy_def",
        "hypotheses",
        "discovery_jobs",
        "research_attempts",
    ] {
        table(&mut t, name)["rows"] = json!([]);
    }
    let mut f = Fixture::new(&t);
    let cfg = parse_discovery_config(raw, 64.0).unwrap();
    let expected = f.expected.clone().unwrap();
    let defs = crate::discovery_runner::source_row_strategies_for_test(&cfg).unwrap();
    let ids: Vec<_> = defs
        .iter()
        .map(|s| {
            repositories::get_or_insert_verified_runner_strategy(f.conn(), Some(expected.epoch), s)
                .unwrap()
        })
        .collect();
    let lineage =
        crate::discovery_runner::source_row_lineage_for_test(&cfg, raw, 1, &ids, "1h").unwrap();
    let events: Vec<_> = (0..ids.len())
        .map(|i| format!("trial-event-v1:{i:064x}"))
        .collect();
    let tx = f.conn().transaction().unwrap();
    let recorded = history::register_run_lineage_with_events(&tx, &lineage, Some(&events)).unwrap();
    assert_eq!(recorded.len(), ids.len());
    for (i, id) in ids.iter().enumerate() {
        for segment in ["train", "validation"] {
            tx.execute("INSERT INTO discovery_jobs(discovery_run_id,candidate_index,strategy_id,dataset_id,segment,status) VALUES(1,?,?,7,?,'queued')",rusqlite::params![i as i64,id,segment]).unwrap();
        }
    }
    tx.commit().unwrap();
    f
}
fn template(id: &str) -> Value {
    serde_json::from_str(case(id)["configJson"].as_str().unwrap()).unwrap()
}
struct NoPayload(usize);
impl NoPayload {
    fn new() -> Self {
        DENY_PAYLOAD.with(|v| v.set(true));
        Self(PAYLOAD_ENTRIES.with(std::cell::Cell::get))
    }
}
impl Drop for NoPayload {
    fn drop(&mut self) {
        DENY_PAYLOAD.with(|v| v.set(false));
        if !std::thread::panicking() {
            assert_eq!(PAYLOAD_ENTRIES.with(std::cell::Cell::get), self.0);
        }
    }
}
fn prior(p: &PreparedOriginalLineage<'_>, v: &Value) {
    let n = |k: &str| v[k].as_u64().unwrap() as usize;
    assert_eq!(p.parse_work(), (n("parseBytes"), n("parseNodes")));
    assert_eq!(
        p.expansion_work(),
        (n("expansionBytes"), n("expansionNodes"))
    );
    assert_eq!(p.combined_work(), (n("scratchBytes"), n("scratchNodes")));
}
pub(super) fn resource(kind: &str, over: bool) -> (Fixture, Value) {
    let r = oracle()["resourceBoundaryRecipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == kind)
        .unwrap()
        .clone();
    let mut raw = template(r["producerTemplate"].as_str().unwrap());
    raw["caps"]["candidates"] = json!(4096);
    if kind == "hash" {
        raw["bases"][0]["id"] = json!("resource-main");
        raw["bases"][0]["axes"][0] = json!({"key":"fastMA","min":1,"max":8,"step":1});
    } else {
        let base = raw["bases"][0].clone();
        raw["bases"] = json!((0..5)
            .map(|i| {
                let mut b = base.clone();
                b["id"] = json!(format!("resource-dsl-{i}"));
                b["strategy"]["dsl"]["name"] = json!(format!("Resource {i}"));
                b
            })
            .collect::<Vec<_>>());
    }
    let mut f = produced(&raw);
    let claims: Vec<(i64, String)> = f
        .conn()
        .prepare("SELECT id,original_definition_json FROM strategy_def ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        claims.len(),
        r["familyCandidates"].as_u64().unwrap() as usize
    );
    for (i, (id, text)) in claims.into_iter().enumerate() {
        let mut d: Value = serde_json::from_str(&text).unwrap();
        let padding = r["asciiPaddingBytesByCandidateIndex"][i].as_u64().unwrap() as usize;
        if kind == "hash" {
            d["entryRules"] = json!(vec![0; 32000]);
            d["entryCode"] = json!("x".repeat(padding + usize::from(over && i == 0)));
        } else {
            d["dsl"]["name"] = json!("x".repeat(padding));
            d["dsl"]["params"]["noise"] = json!(vec![0; 24000]);
            if i == 0 {
                for j in 0..(1 + usize::from(over)) {
                    d["dsl"]["params"]["noise"][j] = json!(10);
                }
            }
            f.conn()
                .execute(
                    "UPDATE strategy_def SET dsl_json=? WHERE id=?",
                    rusqlite::params![serde_json::to_string(&d["dsl"]).unwrap(), id],
                )
                .unwrap();
        }
        f.conn()
            .execute(
                "UPDATE strategy_def SET original_definition_json=? WHERE id=?",
                rusqlite::params![serde_json::to_string(&d).unwrap(), id],
            )
            .unwrap();
    }
    (f, r[if over { "oneOver" } else { "exact" }].clone())
}
#[test]
fn semantic_real_hash_whole_preflight_exact_and_one_over() {
    resource_boundary("hash");
}
#[test]
fn semantic_real_copy_whole_preflight_exact_and_one_over() {
    resource_boundary("copy");
}
fn resource_boundary(kind: &str) {
    for over in [false, true] {
        let (mut f, v) = resource(kind, over);
        let rows = f.capture();
        let p = prepare_original_lineage(&rows).unwrap();
        prior(&p, &v);
        let guard = NoPayload::new();
        if over {
            refused(semantics::planned_work(&p));
            refused(observe_original_lineage(&rows));
        } else {
            // Poisoned definitions are deliberately invalid after preflight;
            // this proves only exact resource acceptance, never semantic PASS.
            let actual = semantics::planned_work(&p).unwrap();
            assert_eq!(actual.0, v["hashBytes"].as_u64().unwrap() as usize);
            assert_eq!(actual.1, v["copyBytes"].as_u64().unwrap() as usize);
            assert_eq!(actual.3, v["outputBytes"].as_u64().unwrap() as usize);
        }
        drop(guard);
        if !over {
            refused(observe_original_lineage(&rows));
        }
    }
}
fn output_raw(over: bool) -> Value {
    let mut raw = template("v1-ordered-axes");
    raw["caps"]["candidates"] = json!(4096);
    let mut main = raw["bases"][0].clone();
    main["id"] = json!("m".repeat(3672));
    main["strategy"]["slowMA"] = json!(2000);
    main["axes"] = json!([{"key":"fastMA","min":1,"max":44,"step":1},{"key":"emaPeriod","min":1,"max":25,"step":1}]);
    let mut other = main.clone();
    other["id"] = json!("o".repeat(900 + usize::from(over)));
    other["strategy"]["emaPeriod"] = json!(51);
    other["axes"] = json!([]);
    raw["bases"] = json!([main, other]);
    raw
}
#[test]
fn semantic_real_output_boundary_is_whole_positive_and_one_over_is_pre_copy() {
    for over in [false, true] {
        let raw = output_raw(over);
        let mut f = produced(&raw);
        let rows = f.capture();
        let p = prepare_original_lineage(&rows).unwrap();
        let v =
            &oracle()["outputBoundaryRecipe"][if over { "oneOverTotals" } else { "exactTotals" }];
        prior(&p, v);
        if over {
            let _guard = NoPayload::new();
            refused(observe_original_lineage(&rows));
        } else {
            work(&p, v);
            let result = observe_original_lineage(&rows).unwrap();
            assert_eq!(result.candidates().len(), 1101);
            let actual = observation(&result);
            assert_eq!(
                actual["configHash"],
                crate::research::sha256_hex(&crate::research::canonical_json(&raw).unwrap())
            );
            let bytes = result.config_hash().len()
                + result
                    .candidates()
                    .iter()
                    .map(|c| {
                        c.strategy_hash().len() + c.hypothesis_hash().len() + c.base_id().len()
                    })
                    .sum::<usize>();
            assert_eq!(bytes, 4194304);
        }
    }
}
