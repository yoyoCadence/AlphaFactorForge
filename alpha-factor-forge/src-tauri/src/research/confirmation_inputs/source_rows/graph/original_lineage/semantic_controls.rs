use super::super::semantics;
use super::*;
const SEMANTICS: &str =
    include_str!("../../../../../../../fixtures/research/original-lineage-semantics-v1.json");
pub(super) fn oracle() -> Value {
    serde_json::from_str(SEMANTICS).unwrap()
}
pub(super) fn observation(v: &OriginalLineageObservation) -> Value {
    json!({"configHash":v.config_hash(),"candidates":v.candidates().iter().map(|c|json!({
        "index":c.index(),"strategyId":c.strategy_id(),"strategyHash":c.strategy_hash(),
        "hypothesisId":c.hypothesis_id(),"hypothesisHash":c.hypothesis_hash(),"baseId":c.base_id()
    })).collect::<Vec<_>>()})
}
pub(super) fn work(p: &PreparedOriginalLineage<'_>, v: &Value) {
    let n = |k: &str| v[k].as_u64().unwrap() as usize;
    assert_eq!(
        semantics::planned_work(p).unwrap(),
        (
            n("hashBytes"),
            n("copyBytes"),
            n("cloneNodes"),
            n("outputBytes")
        )
    );
}
#[test]
fn semantic_four_frozen_whole_family_results_and_work() {
    for v in oracle()["cases"].as_array().unwrap() {
        let mut f = Fixture::new(&tables(v["id"].as_str().unwrap()));
        let rows = f.capture();
        let saved = rows.clone();
        let before = entries();
        let result = observe_original_lineage(&rows).unwrap();
        let after = entries();
        assert_eq!(after.1, before.1 + 1);
        assert_eq!(observation(&result), v["expectedObservation"]);
        let copy = result.clone();
        assert!(copy == result);
        work(&prepare_original_lineage(&rows).unwrap(), v);
        assert!(rows == saved);
    }
}
#[test]
fn semantic_nonmonotonic_library_ids_resolve_actual_references() {
    let v = &oracle()["idOrderControl"];
    let mut t = tables("v1-shared-dedup");
    let sid = |i: usize| v["strategyIdsByCandidateIndex"][i].clone();
    let hid = |i: usize| v["hypothesisIdsByProducerOrder"][i].clone();
    for (i, r) in table(&mut t, "strategy_def")["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        r[0] = sid(i);
    }
    for (i, r) in table(&mut t, "hypotheses")["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        r[0] = hid(i);
    }
    for (i, r) in table(&mut t, "research_attempts")["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        r[2] = hid((r[2].as_u64().unwrap() - 200) as usize);
        r[3] = sid(i);
        r[8] = v["inputs"][i]["inputFingerprintJson"].clone();
        assert_eq!(
            crate::research::sha256_hex(r[8].as_str().unwrap().as_bytes()),
            v["inputs"][i]["sha256"]
        );
    }
    for r in table(&mut t, "discovery_jobs")["rows"]
        .as_array_mut()
        .unwrap()
    {
        r[3] = sid(r[2].as_u64().unwrap() as usize);
    }
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    assert_eq!(
        rows.strategies.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![11, 21, 31, 41]
    );
    assert_eq!(
        rows.hypotheses.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![51, 91]
    );
    let p = prepare_original_lineage(&rows).unwrap();
    work(&p, v);
    assert_eq!(p.parse_work(), (13396, 628));
    assert_eq!(p.combined_work(), (16705, 856));
    assert_eq!(
        observation(&observe_original_lineage(&rows).unwrap()),
        v["expectedObservation"]
    );
}
// Explicit forged-history setup only in an owned disposable DB. Original
// writer controls and the positive output boundary retain all immutable guards.
fn claims_fixture(id: &str) -> Fixture {
    let mut f = Fixture::new(&tables(id));
    forged_history(&mut f);
    f
}
fn forged_history(f: &mut Fixture) {
    f.conn().execute_batch("DROP TRIGGER IF EXISTS research_attempts_are_frozen; DROP TRIGGER IF EXISTS hypotheses_are_immutable; DROP TRIGGER IF EXISTS campaign_run_admissions_no_update;").unwrap();
}
fn json_change(
    f: &mut Fixture,
    table: &str,
    column: &str,
    id: i64,
    change: impl FnOnce(&mut Value),
) {
    forged_history(f);
    let sql = format!("SELECT {column} FROM {table} WHERE id=?");
    let original: String = f.conn().query_row(&sql, [id], |r| r.get(0)).unwrap();
    let mut v: Value = serde_json::from_str(&original).unwrap();
    change(&mut v);
    f.conn()
        .execute(
            &format!("UPDATE {table} SET {column}=? WHERE id=?"),
            rusqlite::params![serde_json::to_string(&v).unwrap(), id],
        )
        .unwrap();
}
#[test]
fn semantic_every_input_field_key_type_axis_seed_and_late_member_refuses() {
    let fields = [
        "envelopeVersion",
        "configHash",
        "rootSeed",
        "datasetId",
        "datasetHash",
        "interval",
        "strategyId",
        "strategyHash",
        "baseId",
        "candidateIndex",
        "appliedAxes",
        "seeds",
    ];
    for field in fields {
        for mutation in 0..3 {
            let mut f = claims_fixture("v1-ordered-axes");
            json_change(
                &mut f,
                "research_attempts",
                "input_fingerprint_json",
                402,
                |v| match mutation {
                    0 => {
                        v.as_object_mut().unwrap().remove(field);
                    }
                    1 => v[field] = json!(null),
                    _ => v[field] = json!("changed"),
                },
            );
            refused(observe_original_lineage(&f.capture()));
        }
    }
    for (field, value) in [
        ("datasetId", json!(7.0)),
        ("strategyId", json!(3.0)),
        ("candidateIndex", json!(2.0)),
        ("rootSeed", json!(20260726.0)),
        ("seeds", json!({"randomEntry":0.0})),
        ("appliedAxes", json!({"fastMA":11.0})),
        ("extra", json!(true)),
    ] {
        let mut f = claims_fixture("v1-ordered-axes");
        json_change(
            &mut f,
            "research_attempts",
            "input_fingerprint_json",
            402,
            |v| v[field] = value,
        );
        refused(observe_original_lineage(&f.capture()));
    }
}
#[test]
fn semantic_every_engine_and_contract_member_is_original_and_exact() {
    for field in [
        "package",
        "configContracts",
        "execution",
        "metrics",
        "benchmarks",
        "gate",
        "score",
        "validationRecord",
        "benchmarkRecord",
        "extra",
    ] {
        let mut f = claims_fixture("v1-ordered-axes");
        json_change(
            &mut f,
            "research_attempts",
            "engine_fingerprint_json",
            402,
            |v| v[field] = json!("changed"),
        );
        refused(observe_original_lineage(&f.capture()));
    }
    for id in ["v1-ordered-axes", "v2-fixed-dsl", "v3-walk-forward"] {
        let c = case(id);
        let engine: Value =
            serde_json::from_str(c["engineFingerprintJson"].as_str().unwrap()).unwrap();
        for field in engine["configContracts"]
            .as_object()
            .unwrap()
            .keys()
            .chain(std::iter::once(&"extra".to_owned()))
        {
            let mut f = claims_fixture(id);
            json_change(
                &mut f,
                "research_attempts",
                "engine_fingerprint_json",
                400,
                |v| v["configContracts"][field] = Value::Null,
            );
            refused(observe_original_lineage(&f.capture()));
        }
    }
}
#[test]
fn semantic_complete_hypothesis_scalar_content_applicability_and_attribution_refuse() {
    for field in [
        "version",
        "source",
        "mechanism",
        "applicability",
        "failureModes",
        "strategyHash",
        "strategyId",
        "parentStrategyId",
        "variationKind",
        "extra",
    ] {
        let mut f = claims_fixture("v1-shared-dedup");
        json_change(&mut f, "hypotheses", "content_json", 201, |v| {
            v[field] = json!("changed")
        });
        refused(observe_original_lineage(&f.capture()));
    }
    for field in [
        "datasetId",
        "datasetHash",
        "interval",
        "split",
        "embargo",
        "axes",
        "extra",
    ] {
        let mut f = claims_fixture("v1-shared-dedup");
        json_change(&mut f, "hypotheses", "applicability_json", 201, |v| {
            v[field] = json!("changed")
        });
        refused(observe_original_lineage(&f.capture()));
    }
    for field in [
        "hypothesis_hash",
        "version",
        "source",
        "mechanism",
        "failure_modes",
        "strategy_hash",
        "variation_kind",
    ] {
        let mut f = claims_fixture("v1-shared-dedup");
        let changed = match field {
            "hypothesis_hash" => "0".repeat(64),
            "strategy_hash" => format!("strategy-doc-v1:{}", "0".repeat(64)),
            "source" => "manual".to_owned(),
            _ => "changed".to_owned(),
        };
        f.conn()
            .execute(
                &format!("UPDATE hypotheses SET {field}=? WHERE id=201"),
                [changed],
            )
            .unwrap();
        refused(observe_original_lineage(&f.capture()));
    }
    let mut f = claims_fixture("v1-shared-dedup");
    f.conn()
        .execute(
            "UPDATE research_attempts SET hypothesis_id=200 WHERE id=403",
            [],
        )
        .unwrap();
    // Both bases have the same strategy-doc hash; original attribution still differs.
    refused(observe_original_lineage(&f.capture()));
}
#[test]
fn semantic_original_strategy_costs_modes_markers_and_saved_dsl_refuse() {
    for (field, value) in [
        ("mode", json!("code")),
        ("definitionVersion", json!(null)),
        ("numericPolicy", json!("strict")),
        ("feePct", json!(0.002)),
        ("slipPct", json!(-0.0)),
        ("slowMA", json!(999)),
        ("entryRules", json!([true])),
    ] {
        let mut f = claims_fixture("v1-ordered-axes");
        json_change(&mut f, "strategy_def", "original_definition_json", 3, |v| {
            v[field] = value
        });
        refused(observe_original_lineage(&f.capture()));
    }
    let mut f = claims_fixture("v2-fixed-dsl");
    json_change(&mut f, "strategy_def", "dsl_json", 1, |v| {
        v["name"] = json!("contradiction retained by repository")
    });
    refused(observe_original_lineage(&f.capture()));
    let mut f = claims_fixture("v2-fixed-dsl");
    f.conn()
        .execute("UPDATE strategy_def SET dsl_json=NULL", [])
        .unwrap();
    refused(observe_original_lineage(&f.capture()));
    let mut f = claims_fixture("v1-ordered-axes");
    f.conn()
        .execute("UPDATE strategy_def SET dsl_json='{}' WHERE id=3", [])
        .unwrap();
    refused(observe_original_lineage(&f.capture()));
    let mut f = claims_fixture("v1-ordered-axes");
    f.conn()
        .execute(
            "UPDATE strategy_def SET parent_strategy_id=1 WHERE id=3",
            [],
        )
        .unwrap();
    refused(observe_original_lineage(&f.capture()));
}
#[test]
fn semantic_legacy_numeric_spelling_library_format_and_failed_members_are_preserved() {
    let mut f = claims_fixture("v1-ordered-axes");
    json_change(&mut f, "strategy_def", "original_definition_json", 3, |v| {
        v["slowMA"] = json!(21.0)
    });
    f.conn()
        .execute(
            "UPDATE research_attempts SET status='failed',outcome_json='{}' WHERE id=402",
            [],
        )
        .unwrap();
    f.conn()
        .execute(
            "UPDATE discovery_jobs SET status='failed' WHERE candidate_index=2",
            [],
        )
        .unwrap();
    let rows = f.capture();
    let actual = observe_original_lineage(&rows).unwrap();
    assert_eq!(actual.candidates().len(), 3);
    json_change(&mut f, "strategy_def", "original_definition_json", 3, |v| {
        v["slowMA"] = json!(999)
    });
    refused(observe_original_lineage(&f.capture()));
}
#[test]
fn semantic_success_and_refusal_preserve_caller_transaction_and_workspace() {
    let mut f = claims_fixture("v1-ordered-axes");
    let expected = f.expected.clone().unwrap();
    let tx = f
        .conn()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let graph: Value = serde_json::from_str(GRAPH).unwrap();
    let locator = SourceLocator::new(graph["campaignId"].as_str().unwrap(), 1).unwrap();
    let rows = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator)).unwrap();
    let before = snapshot(&tx);
    unchanged(&tx, || observe_original_lineage(&rows)).unwrap();
    tx.execute("UPDATE strategy_def SET dsl_json='{}' WHERE id=3", [])
        .unwrap();
    let bad = capture_source_rows_in(&tx, &expected, &locator).unwrap();
    refused(unchanged(&tx, || observe_original_lineage(&bad)));
    assert!(!tx.is_autocommit());
    tx.rollback().unwrap();
    assert_eq!(snapshot(f.conn()), before);
}

#[test]
fn semantic_unsupported_original_envelope_modes_are_not_promoted() {
    use super::semantic_boundaries::produced;
    for version in ["v2-fixed-dsl", "v3-walk-forward"] {
        let mut raw: Value =
            serde_json::from_str(case(version)["configJson"].as_str().unwrap()).unwrap();
        let param: Value =
            serde_json::from_str(case("v1-ordered-axes")["configJson"].as_str().unwrap()).unwrap();
        raw["bases"] = param["bases"].clone();
        let mut f = produced(&raw);
        let rows = f.capture();
        assert!(prepare_original_lineage(&rows).is_ok());
        refused(observe_original_lineage(&rows));
        let dsl: Value =
            serde_json::from_str(case(version)["configJson"].as_str().unwrap()).unwrap();
        raw["bases"]
            .as_array_mut()
            .unwrap()
            .push(dsl["bases"][0].clone());
        let mut f = produced(&raw);
        let rows = f.capture();
        assert!(prepare_original_lineage(&rows).is_ok());
        refused(observe_original_lineage(&rows));
    }
}
#[test]
fn semantic_original_zero_bits_and_admission_cost_mismatch_refuse() {
    use super::semantic_boundaries::produced;
    let mut raw: Value =
        serde_json::from_str(case("v1-ordered-axes")["configJson"].as_str().unwrap()).unwrap();
    raw["benchmarkCosts"]["feePct"] = json!(0.0);
    raw["benchmarkCosts"]["slipPct"] = json!(0.0);
    raw["bases"][0]["strategy"]["feePct"] = json!(0.0);
    raw["bases"][0]["strategy"]["slipPct"] = json!(0.0);
    let mut f = produced(&raw);
    forged_history(&mut f);
    f.conn()
        .execute(
            "UPDATE campaign_run_admissions SET fee_pct=0.0,slip_pct=0.0",
            [],
        )
        .unwrap();
    observe_original_lineage(&f.capture()).unwrap();
    json_change(&mut f, "strategy_def", "original_definition_json", 3, |d| {
        d["feePct"] = json!(-0.0)
    });
    refused(observe_original_lineage(&f.capture()));
    let mut f = claims_fixture("v1-ordered-axes");
    f.conn()
        .execute("UPDATE campaign_run_admissions SET fee_pct=0.123", [])
        .unwrap();
    refused(observe_original_lineage(&f.capture()));
}
#[test]
fn semantic_compatible_saved_json_bytes_use_legacy_identity_and_canonical_config() {
    for id in ["v1-ordered-axes", "v2-fixed-dsl", "v3-walk-forward"] {
        let mut f = claims_fixture(id);
        for (tab, col) in [
            ("discovery_runs", "config_json"),
            ("strategy_def", "original_definition_json"),
            ("strategy_def", "dsl_json"),
            ("hypotheses", "content_json"),
            ("hypotheses", "applicability_json"),
        ] {
            let values: Vec<(i64, Option<String>)> = f
                .conn()
                .prepare(&format!("SELECT id,{col} FROM {tab}"))
                .unwrap()
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            for (row_id, s) in values {
                if let Some(s) = s {
                    let v: Value = serde_json::from_str(&s).unwrap();
                    f.conn()
                        .execute(
                            &format!("UPDATE {tab} SET {col}=? WHERE id=?"),
                            rusqlite::params![serde_json::to_string_pretty(&v).unwrap(), row_id],
                        )
                        .unwrap();
                }
            }
        }
        let rows = f.capture();
        let result = observe_original_lineage(&rows).unwrap();
        let expected = oracle()["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == id)
            .unwrap()
            .clone();
        assert_eq!(observation(&result), expected["expectedObservation"]);
        assert_ne!(
            result.config_hash(),
            crate::research::sha256_hex(rows.roots.run.config_json.as_bytes())
        );
    }
}
#[test]
fn semantic_null_parent_hash_and_order_projection_refusals() {
    for column in ["strategy_id", "parent_strategy_id"] {
        let mut f = claims_fixture("v2-fixed-dsl");
        f.conn()
            .execute(&format!("UPDATE hypotheses SET {column}=1"), [])
            .unwrap();
        refused(observe_original_lineage(&f.capture()));
    }
    let mut f = claims_fixture("v1-ordered-axes");
    for col in ["content_json", "applicability_json"] {
        json_change(&mut f, "hypotheses", col, 200, |v| {
            let a = if col == "content_json" {
                &mut v["applicability"]
            } else {
                v
            };
            a["axes"].as_array_mut().unwrap().reverse();
        });
    }
    refused(observe_original_lineage(&f.capture()));
    let mut f = claims_fixture("v1-ordered-axes");
    f.conn()
        .execute("UPDATE hypotheses SET hypothesis_hash=?", ["0".repeat(64)])
        .unwrap();
    refused(observe_original_lineage(&f.capture()));
    let mut f = claims_fixture("v1-ordered-axes");
    f.conn()
        .execute("UPDATE strategy_def SET type='manual' WHERE id=3", [])
        .unwrap();
    refused(observe_original_lineage(&f.capture()));
}

#[test]
fn semantic_defensive_complete_reference_identity_and_extra_member_checks() {
    let mut f = Fixture::new(&tables("v1-shared-dedup"));
    let rows = f.capture();
    for mutation in 0..6 {
        // Descendant-only defensive tests start from a real sealed capture;
        // no unchecked constructor or mutation API exists outside this module.
        let mut bad = rows.clone();
        match mutation {
            0 => bad.hypotheses[1].hypothesis_hash = bad.hypotheses[0].hypothesis_hash.clone(),
            1 => bad.hypotheses.push(bad.hypotheses[0].clone()),
            2 => bad.strategies.push(bad.strategies[0].clone()),
            3 => bad.attempts[3].strategy_id = 99999,
            4 => bad.attempts[3].hypothesis_id = 99999,
            _ => bad.attempts[3].candidate_index = Some(0),
        }
        refused(observe_original_lineage(&bad));
    }
    let mut f = Fixture::new(&serde_json::from_str::<Value>(GRAPH).unwrap()["tables"]);
    // Historical raw fixture deliberately contains 1e999, never repaired.
    refused(observe_original_lineage(&f.capture()));
}
