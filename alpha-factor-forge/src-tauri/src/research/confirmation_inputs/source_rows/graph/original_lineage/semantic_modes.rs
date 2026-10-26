use super::semantic_controls::{observation, work};
use super::*;

const MODES: &str =
    include_str!("../../../../../../../fixtures/research/original-producer-mode-coverage-v1.json");

fn mode_case(id: &str) -> Value {
    serde_json::from_str::<Value>(MODES).unwrap()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap()
        .clone()
}
const MIXED: [&str; 2] = ["v2-mixed-params-first", "v3-mixed-dsl-first"];

fn conforms(id: &str) {
    let c = mode_case(id);
    let mut f = Fixture::new(&tables_case(&c));
    let rows = f.capture();
    let saved = rows.clone();
    let expected = &c["work"];
    let manifest = expected["manifest"].as_array().unwrap();
    let before = entries();
    let result = observe_original_lineage(&rows);
    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(entries(), (before.0 + manifest.len(), before.1 + 1));
    assert_eq!(observation(&result), c["expectedObservation"]);
    assert!(result.clone() == result);
    let p = prepare_original_lineage(&rows).unwrap();
    assert!(std::ptr::eq(p.rows(), &rows));
    let n = |k: &str| expected[k].as_u64().unwrap() as usize;
    assert_eq!(p.parse_work(), (n("parseBytes"), n("parseNodes")));
    assert_eq!(
        p.expansion_work(),
        (n("expansionBytes"), n("expansionNodes"))
    );
    assert_eq!(p.combined_work(), (n("scratchBytes"), n("scratchNodes")));
    work(&p, expected);
    let mut bytes = 0;
    let mut nodes = 0;
    for entry in manifest {
        let i = entry["index"].as_u64().unwrap_or(0) as usize;
        let (doc, weight) = match entry["kind"].as_str().unwrap() {
            "config" => (&rows.roots.run.config_json, 2),
            "definition" => (&rows.strategies[i].original_definition_json, 1),
            "dsl" => (rows.strategies[i].dsl_json.as_ref().unwrap(), 1),
            "input" => (&rows.attempts[i].input_fingerprint_json, 1),
            "engine" => (&rows.attempts[i].engine_fingerprint_json, 1),
            "content" => (&rows.hypotheses[i].content_json, 1),
            "applicability" => (&rows.hypotheses[i].applicability_json, 1),
            other => panic!("unknown literal document {other}"),
        };
        assert_eq!(entry["weight"], weight);
        assert_eq!(entry["bytes"], doc.as_bytes().len());
        assert_eq!(entry["nodes"], doc.node_count());
        assert_eq!(entry["sha256"], crate::research::sha256_hex(doc.as_bytes()));
        bytes += weight * doc.as_bytes().len();
        nodes += weight * doc.node_count();
    }
    assert_eq!((bytes, nodes), p.parse_work());
    assert_eq!(
        rows.roots.run.config_json.as_bytes(),
        c["configJson"].as_str().unwrap().as_bytes()
    );
    if id == "v3-mixed-dsl-first" {
        assert_eq!(p.config().bases[0].id, "dsl-cross");
        assert_eq!(result.candidates()[0].base_id(), "ma-cross");
        assert_ne!(result.config_hash(), c["configRawSha256"].as_str().unwrap());
    }
    assert!(rows == saved);
    for table in [
        "candles",
        "research_artifacts",
        "backtest_summary",
        "trades",
        "validation_records",
    ] {
        assert_eq!(
            f.conn()
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
#[test]
fn semantic_v2_params_complete_graph_and_literal_work() {
    conforms("v2-params");
}
#[test]
fn semantic_v3_params_complete_graph_and_literal_work() {
    conforms("v3-params");
}
#[test]
fn semantic_v2_mixed_complete_graph_and_literal_work() {
    conforms("v2-mixed-params-first");
}
#[test]
fn semantic_v3_mixed_config_order_differs_from_winning_order() {
    conforms("v3-mixed-dsl-first");
}

// All negative claims still enter through a captured graph. Altering literal
// seed rows here never changes the immutable writer or exposes a parts factory.
fn mixed_refusal(id: &str, change: impl FnOnce(&mut Value)) {
    let mut t = tables_case(&mode_case(id));
    change(&mut t);
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    let saved = rows.clone();
    let observer = Connection::open(f.root.join(db::DB_FILE_NAME)).unwrap();
    observer.busy_timeout(Duration::ZERO).unwrap();
    let before = snapshot(f.conn());
    let tx = f
        .conn()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute(
        "UPDATE strategy_def SET name='caller pending' WHERE id=1",
        [],
    )
    .unwrap();
    writer_busy(&observer);
    let result = unchanged(&tx, || observe_original_lineage(&rows));
    assert!(result.is_err());
    refused(result);
    writer_busy(&observer);
    assert!(rows == saved);
    tx.rollback().unwrap();
    assert_eq!(snapshot(f.conn()), before);
}
fn change_json(
    t: &mut Value,
    name: &str,
    row: usize,
    column: usize,
    edit: impl FnOnce(&mut Value),
) {
    let cell = &mut table(t, name)["rows"][row][column];
    let mut v: Value = serde_json::from_str(cell.as_str().unwrap()).unwrap();
    edit(&mut v);
    *cell = json!(serde_json::to_string(&v).unwrap());
}
#[test]
fn semantic_mixed_late_dsl_contradiction_and_params_unexpected_dsl_refuse() {
    for id in MIXED {
        mixed_refusal(id, |t| {
            change_json(t, "strategy_def", 3, 3, |d| {
                d["name"] = json!("saved contradiction")
            })
        });
        mixed_refusal(id, |t| table(t, "strategy_def")["rows"][2][3] = json!("{}"));
    }
}
#[test]
fn semantic_mixed_types_markers_parents_costs_and_unsupported_modes_refuse() {
    for id in MIXED {
        for row in [2, 3] {
            for mode in ["code", "blocks", "manual"] {
                mixed_refusal(id, |t| {
                    table(t, "strategy_def")["rows"][row][1] = json!(mode);
                    change_json(t, "strategy_def", row, 2, |d| d["mode"] = json!(mode));
                });
            }
            for (field, value) in [
                ("definitionVersion", Value::Null),
                ("numericPolicy", json!("strict")),
                ("feePct", json!(0.051)),
                ("slipPct", json!(-0.0)),
            ] {
                mixed_refusal(id, |t| {
                    change_json(t, "strategy_def", row, 2, |d| d[field] = value)
                });
            }
            mixed_refusal(id, |t| table(t, "strategy_def")["rows"][row][5] = json!(1));
            mixed_refusal(id, |t| {
                table(t, "strategy_def")["rows"][row][1] =
                    json!(if row == 2 { "dsl" } else { "params" })
            });
        }
    }
}
#[test]
fn semantic_mixed_wrong_base_and_hypothesis_attribution_refuse() {
    for id in MIXED {
        mixed_refusal(id, |t| {
            change_json(t, "research_attempts", 3, 8, |v| {
                v["baseId"] = json!("ma-cross")
            })
        });
        // Both referenced hypotheses remain in the complete graph, preventing
        // missing-row closure from masking per-base attribution checks.
        mixed_refusal(id, |t| {
            table(t, "research_attempts")["rows"][2][2] = json!(201);
            table(t, "research_attempts")["rows"][3][2] = json!(200);
        });
        for column in [8, 9] {
            mixed_refusal(id, |t| table(t, "hypotheses")["rows"][1][column] = json!(4));
        }
    }
}
#[test]
fn semantic_mixed_late_original_engine_contract_refuses() {
    for id in MIXED {
        for key in ["enumeration", "strategyDsl"] {
            mixed_refusal(id, |t| {
                change_json(t, "research_attempts", 3, 9, |v| {
                    v["configContracts"][key] = json!("changed")
                })
            });
        }
    }
}
#[test]
fn semantic_mixed_failed_params_and_dsl_still_observe_or_refuse_as_a_whole() {
    for id in MIXED {
        for bad_row in [None, Some(2), Some(3)] {
            let c = mode_case(id);
            let mut t = tables_case(&c);
            for row in [2, 3] {
                let a = &mut table(&mut t, "research_attempts")["rows"][row];
                a[7] = json!("failed");
                a[10] = json!("{}");
                for job in [2 * row, 2 * row + 1] {
                    table(&mut t, "discovery_jobs")["rows"][job][6] = json!("failed");
                }
            }
            if let Some(row) = bad_row {
                if row == 2 {
                    change_json(&mut t, "strategy_def", row, 2, |d| d["slowMA"] = json!(999));
                } else {
                    change_json(&mut t, "strategy_def", row, 3, |d| {
                        d["name"] = json!("failed contradiction")
                    });
                }
            }
            let mut f = Fixture::new(&t);
            let rows = f.capture();
            assert_eq!(rows.attempts.len(), 4);
            assert_eq!(rows.jobs.len(), 8);
            let result = observe_original_lineage(&rows);
            if bad_row.is_some() {
                refused(result);
            } else {
                assert_eq!(observation(&result.unwrap()), c["expectedObservation"]);
            }
        }
    }
}
#[test]
fn semantic_mode_success_preserves_pending_caller_rows_owner_and_write_reservation() {
    for id in ["v2-params", "v3-params", MIXED[0], MIXED[1]] {
        let c = mode_case(id);
        let mut f = Fixture::new(&tables_case(&c));
        let expected = f.expected.clone().unwrap();
        let observer = Connection::open(f.root.join(db::DB_FILE_NAME)).unwrap();
        observer.busy_timeout(Duration::ZERO).unwrap();
        let before = snapshot(f.conn());
        let graph: Value = serde_json::from_str(GRAPH).unwrap();
        let locator = SourceLocator::new(graph["campaignId"].as_str().unwrap(), 1).unwrap();
        let tx = f
            .conn()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        tx.execute("UPDATE strategy_def SET name='caller pending',source='manual',param_schema_json='{}' WHERE id=1", []).unwrap();
        writer_busy(&observer);
        let rows = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator)).unwrap();
        let saved = rows.clone();
        let result = unchanged(&tx, || observe_original_lineage(&rows)).unwrap();
        assert_eq!(observation(&result), c["expectedObservation"]);
        assert!(rows == saved);
        writer_busy(&observer);
        tx.rollback().unwrap();
        assert_eq!(snapshot(f.conn()), before);
    }
}
