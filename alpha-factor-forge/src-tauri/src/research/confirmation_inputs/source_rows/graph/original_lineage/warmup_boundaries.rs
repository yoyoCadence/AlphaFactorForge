use super::super::warmup;
use super::semantic_boundaries::produced;
use super::warmup_controls::{self as controls, DenyWork};
use super::*;

fn n(v: &Value, k: &str) -> usize {
    v[k].as_u64().unwrap() as usize
}
fn raw_template(id: &str) -> Value {
    serde_json::from_str(case(id)["configJson"].as_str().unwrap()).unwrap()
}
fn resource(kind: &str, over: bool) -> (Fixture, Value) {
    let r = controls::oracle()["resourceBoundaryRecipes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == kind)
        .unwrap()
        .clone();
    let mut raw = raw_template("v2-fixed-dsl");
    raw["caps"]["candidates"] = json!(4096);
    raw["bases"][0]["id"] = r["baseId"].clone();
    raw["bases"][0]["strategy"]["dsl"]["name"] = r["baseDslName"].clone();
    if kind == "analysis" {
        let c = controls::literal("v1-varied-slow");
        let cfg: Value = serde_json::from_str(c["configJson"].as_str().unwrap()).unwrap();
        let mut base = cfg["bases"][0].clone();
        base["id"] = r["paramsSidecar"]["id"].clone();
        base["axes"] = json!([]);
        raw["bases"].as_array_mut().unwrap().push(base);
    }
    let mut f = produced(&raw);
    let definitions: Vec<(i64, String)> = f
        .conn()
        .prepare("SELECT id,original_definition_json FROM strategy_def ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(definitions.len(), n(&r, "familyCandidates"));
    for (id, text) in definitions {
        let mut d: Value = serde_json::from_str(&text).unwrap();
        if d["mode"] == "dsl" {
            d["dsl"]["name"] = json!("x".repeat(n(&r, "asciiStoredDslNameBytes")));
            if kind == "copy" {
                for i in 0..n(&r, "numericParams") {
                    d["dsl"]["params"][format!("p{i:05}")] =
                        json!(if i < n(&r, "tenReplacements") { 10 } else { 0 });
                }
                d["resourceCounter"] = json!(if over { 10 } else { 0 });
            }
            f.conn()
                .execute(
                    "UPDATE strategy_def SET dsl_json=? WHERE id=?",
                    rusqlite::params![serde_json::to_string(&d["dsl"]).unwrap(), id],
                )
                .unwrap();
        } else {
            let sig = d["entrySig"].as_str().unwrap().to_owned()
                + &"x".repeat(n(&r, "paramsStoredEntrySigSuffixBytes") + usize::from(over));
            d["entrySig"] = json!(sig);
        }
        f.conn()
            .execute(
                "UPDATE strategy_def SET original_definition_json=? WHERE id=?",
                rusqlite::params![serde_json::to_string(&d).unwrap(), id],
            )
            .unwrap();
    }
    (f, r[if over { "over" } else { "exact" }].clone())
}
fn resource_boundary(kind: &str) {
    for over in [false, true] {
        let (mut f, v) = resource(kind, over);
        let rows = f.capture();
        let saved = rows.clone();
        let p = prepare_original_lineage(&rows).unwrap();
        controls::prior(&p, &v["preparation"]);
        let before = controls::warm_entries();
        {
            let _guard = DenyWork::new(true, true);
            if over {
                controls::refused(warmup::planned_work(&p));
                controls::preserves(&mut f, &rows, false);
            } else {
                controls::work(&p, &v["warmup"]);
            }
        }
        if !over {
            // Resource-only identity poison: exact whole reserve, then refusal
            // at lineage, with no actual candidate analysis or partial result.
            let _guard = DenyWork::new(false, true);
            controls::preserves(&mut f, &rows, false);
        }
        assert_eq!(controls::warm_entries(), before);
        assert!(rows == saved);
    }
}
#[test]
fn warmup_real_analysis_exact_and_one_over_precede_payload_and_analysis() {
    resource_boundary("analysis");
}
#[test]
fn warmup_real_copy_exact_and_one_over_compose_nonempty_lineage_reserve() {
    resource_boundary("copy");
}
#[test]
fn warmup_real_hash_exact_and_one_over_preserve_prior_entry_capacity() {
    let v = controls::oracle();
    let r = &v["hashBoundaryReference"];
    for over in [false, true] {
        let (mut f, prior) = super::semantic_boundaries::resource("hash", over);
        let rows = f.capture();
        let p = prepare_original_lineage(&rows).unwrap();
        controls::prior(&p, &prior);
        {
            let _guard = DenyWork::new(true, true);
            if over {
                controls::refused(warmup::planned_work(&p));
                controls::preserves(&mut f, &rows, false);
            } else {
                controls::work(&p, &r["exact"]);
                super::semantic_controls::work(
                    &p,
                    &json!({"hashBytes":prior["hashBytes"],"copyBytes":prior["copyBytes"],"cloneNodes":p.rows().strategies.iter().map(|s|s.original_definition_json.node_count()+4).sum::<usize>(),"outputBytes":prior["outputBytes"]}),
                );
            }
        }
        if !over {
            let _guard = DenyWork::new(false, true);
            controls::preserves(&mut f, &rows, false);
        }
    }
}
fn output_raw(r: &Value, over: bool) -> Value {
    let mut raw = raw_template("v1-ordered-axes");
    raw["caps"]["candidates"] = json!(4096);
    let mut main = raw["bases"][0].clone();
    main["id"] = json!("m".repeat(n(r, "mainIdBytes")));
    main["strategy"]["slowMA"] = r["mainSlowMA"].clone();
    main["axes"] = r["mainAxes"].clone();
    let mut other = main.clone();
    other["id"] = json!("o".repeat(n(r, "otherIdBytes") + usize::from(over)));
    other["strategy"]["emaPeriod"] = r["otherEmaPeriod"].clone();
    other["axes"] = json!([]);
    raw["bases"] = json!([main, other]);
    raw
}
#[test]
fn warmup_real_output_exact_1101_whole_positive_and_one_over_pre_copy() {
    let v = controls::oracle();
    let r = &v["outputBoundaryRecipe"];
    for over in [false, true] {
        let mut f = produced(&output_raw(r, over));
        let rows = f.capture();
        let p = prepare_original_lineage(&rows).unwrap();
        let e = &r[if over { "oneOver" } else { "exact" }];
        controls::prior(&p, &e["preparation"]);
        if over {
            let _guard = DenyWork::new(true, true);
            controls::refused(warmup::planned_work(&p));
            controls::preserves(&mut f, &rows, false);
        } else {
            controls::work(&p, &e["warmup"]);
            let before = controls::warm_entries();
            let result = controls::preserves(&mut f, &rows, true).unwrap();
            assert_eq!(controls::warm_entries(), (before.0 + 1101, before.1));
            assert_eq!(result.lineage().candidates().len(), 1101);
            assert_eq!(result.derivations().len(), 1101);
            for d in result.derivations() {
                assert_eq!(
                    serde_json::to_value(d).unwrap(),
                    r["expectedEveryDerivation"]
                );
            }
            let out = result.lineage().config_hash().len()
                + result
                    .lineage()
                    .candidates()
                    .iter()
                    .map(|c| {
                        c.strategy_hash().len() + c.hypothesis_hash().len() + c.base_id().len()
                    })
                    .sum::<usize>()
                + 24 * result.derivations().len();
            assert_eq!(out, 4_194_304);
        }
    }
}
#[test]
fn warmup_initial_parse_refusal_preserves_static_error_and_deny_gates() {
    let mut f = Fixture::new(&tables("v1-ordered-axes"));
    let mut rows = f.capture();
    rows.attempts.clear();
    let before = entries();
    let _guard = DenyWork::new(true, true);
    let _deny = Deny::new(true, true);
    controls::preserves(&mut f, &rows, false);
    assert_eq!(entries(), before);
}

#[test]
fn warmup_real_prior_parse_and_expansion_overages_keep_original_gates() {
    for (by_nodes, combined) in [(false, false), (true, false), (false, true), (true, true)] {
        let target = if by_nodes {
            MAX_TOTAL_JSON_NODES - if combined { 12396 } else { 0 } + 1
        } else {
            MAX_TOTAL_TEXT_BYTES - if combined { 1743 } else { 0 } + 1
        };
        let t = if by_nodes {
            super::boundaries::node_tables(target)
        } else {
            super::boundaries::byte_tables(target)
        };
        let mut f = Fixture::new(&t);
        let rows = f.capture();
        let before = entries();
        let _work = DenyWork::new(true, true);
        let _deny = Deny::new(!combined, true);
        controls::preserves(&mut f, &rows, false);
        assert_eq!(entries(), (before.0 + usize::from(combined), before.1));
    }
}
