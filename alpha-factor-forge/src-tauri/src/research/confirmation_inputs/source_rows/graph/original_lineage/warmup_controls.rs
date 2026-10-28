use super::super::{semantics, warmup};
use super::semantic_controls::observation;
use super::*;
use crate::discovery_runner::execution::{DENY_ORIGINAL_WARMUP, ORIGINAL_WARMUP_ENTRIES};
use semantics::{DENY_PAYLOAD, PAYLOAD_ENTRIES};

const WARMUP: &str =
    include_str!("../../../../../../../fixtures/research/original-warmup-consumer-v1.json");
const MODES: &str =
    include_str!("../../../../../../../fixtures/research/original-producer-mode-coverage-v1.json");
pub(super) fn oracle() -> Value {
    assert_eq!(
        crate::research::sha256_hex(WARMUP.replace("\r\n", "\n").as_bytes()),
        "b0da3af1b12fe35dcb58c3622f1abf5dff45211eac8c9489ce7ccfa550307578"
    );
    let v: Value = serde_json::from_str(WARMUP).unwrap();
    assert_eq!(v["version"], "original-warmup-consumer-v1");
    assert_eq!(v["cases"].as_array().unwrap().len(), 2);
    assert_eq!(v["derivationRefusalCases"].as_array().unwrap().len(), 1);
    assert_eq!(v["existingFamilies"].as_array().unwrap().len(), 8);
    let pins = v["sourceFixtures"].as_array().unwrap();
    assert_eq!(pins.len(), 4);
    for (name, text) in [
        ("original-producer-projections-v1.json", PRODUCERS),
        ("original-producer-mode-coverage-v1.json", MODES),
        ("original-warmup-producer-projections-v1.json", include_str!("../../../../../../../fixtures/research/original-warmup-producer-projections-v1.json")),
        ("original-lineage-semantics-v1.json", include_str!("../../../../../../../fixtures/research/original-lineage-semantics-v1.json")),
    ] {
        let matches: Vec<_> = pins.iter().filter(|p| p["path"] == name).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(crate::research::sha256_hex(text.replace("\r\n", "\n").as_bytes()), matches[0]["lfPackagingSha256"]);
    }
    v
}
pub(super) fn literal(id: &str) -> Value {
    let v = oracle();
    v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(v["derivationRefusalCases"].as_array().unwrap())
        .find(|c| c["id"] == id)
        .unwrap()
        .clone()
}
pub(super) fn warm_entries() -> (usize, usize) {
    ORIGINAL_WARMUP_ENTRIES.with(std::cell::Cell::get)
}
pub(super) fn refused<T>(result: AppResult<T>) {
    match result {
        Err(AppError::Other(code)) => assert_eq!(code, "original_warmup_invalid"),
        Err(e) => panic!("wrong warmup error: {e}"),
        Ok(_) => panic!("expected whole warmup refusal"),
    }
}
pub(super) struct DenyWork {
    prior: (bool, bool),
    payload: bool,
    warmup: bool,
    before: (usize, (usize, usize)),
}
impl DenyWork {
    pub(super) fn new(payload: bool, warmup: bool) -> Self {
        let prior = (
            DENY_PAYLOAD.with(|v| v.replace(payload)),
            DENY_ORIGINAL_WARMUP.with(|v| v.replace(warmup)),
        );
        Self {
            prior,
            payload,
            warmup,
            before: (PAYLOAD_ENTRIES.with(std::cell::Cell::get), warm_entries()),
        }
    }
}
impl Drop for DenyWork {
    fn drop(&mut self) {
        DENY_PAYLOAD.with(|v| v.set(self.prior.0));
        DENY_ORIGINAL_WARMUP.with(|v| v.set(self.prior.1));
        if !std::thread::panicking() {
            if self.payload {
                assert_eq!(PAYLOAD_ENTRIES.with(std::cell::Cell::get), self.before.0);
            }
            if self.warmup {
                assert_eq!(warm_entries(), self.before.1);
            }
        }
    }
}
pub(super) fn prior(p: &PreparedOriginalLineage<'_>, w: &Value) {
    let n = |k: &str| w[k].as_u64().unwrap() as usize;
    assert_eq!(p.parse_work(), (n("parseBytes"), n("parseNodes")));
    assert_eq!(
        p.expansion_work(),
        (n("expansionBytes"), n("expansionNodes"))
    );
    assert_eq!(p.combined_work(), (n("scratchBytes"), n("scratchNodes")));
}
pub(super) fn work(p: &PreparedOriginalLineage<'_>, expected: &Value) {
    let n = |k: &str| expected[k].as_u64().unwrap() as usize;
    let actual = warmup::planned_work(p).unwrap();
    assert_eq!(actual.0, n("hashBytes"));
    assert_eq!(actual.1, n("copyBytes"));
    assert_eq!(actual.2, n("analysisUnits"));
    if expected["cloneNodes"].is_number() {
        assert_eq!(actual.3, n("cloneNodes"));
    }
    assert_eq!(actual.4, n("outputBytes"));
    assert!(actual.3 <= p.combined_work().1);
    let old = semantics::planned_work(p).unwrap();
    assert_eq!(
        actual.1,
        old.1 + n("additionalCopyBytes") + 24 * p.plan().candidates.len()
    );
    assert_eq!(actual.3, old.2 + n("additionalCloneNodes"));
    if let Some(parts) = expected["components"].as_array() {
        assert_eq!(parts.len(), p.plan().candidates.len());
        for (i, part) in parts.iter().enumerate() {
            assert_eq!(part["index"], i);
            let id = p.rows().attempts[i].strategy_id;
            let si = p.rows().strategies.iter().position(|s| s.id == id).unwrap();
            let d = p.strategies()[si].definition();
            let n = |k: &str| part[k].as_u64().unwrap_or(0) as usize;
            assert_eq!(d["mode"], part["mode"]);
            assert_eq!(
                warmup::planned_candidate(d).unwrap(),
                (
                    n("jsonBytes"),
                    n("valueNodes"),
                    n("wrapperTextBytes"),
                    n("copyBytes"),
                    n("analysisUnits"),
                    n("cloneNodes")
                )
            );
        }
    }
}
fn manifest(p: &PreparedOriginalLineage<'_>, expected: &Value) {
    for entry in expected["manifest"].as_array().unwrap() {
        if let Some(path) = entry["path"].as_str() {
            let parts: Vec<_> = path.split('/').collect();
            let i = parts
                .get(1)
                .and_then(|i| i.parse::<usize>().ok())
                .unwrap_or(0);
            let doc = match parts[0] {
                "configJson" => &p.rows().roots.run.config_json,
                "engineFingerprintJson" => &p.rows().attempts[i].engine_fingerprint_json,
                "candidates" => match *parts.last().unwrap() {
                    "original_definition_json" => &p.rows().strategies[i].original_definition_json,
                    "dsl_json" => p.rows().strategies[i].dsl_json.as_ref().unwrap(),
                    "inputFingerprintJson" => &p.rows().attempts[i].input_fingerprint_json,
                    "engineFingerprintJson" => &p.rows().attempts[i].engine_fingerprint_json,
                    other => panic!("unexpected original path: {other}"),
                },
                "hypotheses" => {
                    if parts[2] == "documentJson" {
                        &p.rows().hypotheses[i].content_json
                    } else {
                        &p.rows().hypotheses[i].applicability_json
                    }
                }
                other => panic!("unexpected original manifest: {other}"),
            };
            assert_eq!(entry["bytes"], doc.as_bytes().len());
            assert_eq!(entry["nodes"], doc.node_count());
            assert_eq!(entry["sha256"], crate::research::sha256_hex(doc.as_bytes()));
            continue;
        }
        let i = entry["index"].as_u64().unwrap_or(0) as usize;
        let (doc, weight) = match entry["kind"].as_str().unwrap() {
            "config" => (&p.rows().roots.run.config_json, 2),
            "definition" => (&p.rows().strategies[i].original_definition_json, 1),
            "dsl" => (p.rows().strategies[i].dsl_json.as_ref().unwrap(), 1),
            "input" => (&p.rows().attempts[i].input_fingerprint_json, 1),
            "engine" => (&p.rows().attempts[i].engine_fingerprint_json, 1),
            "content" => (&p.rows().hypotheses[i].content_json, 1),
            "applicability" => (&p.rows().hypotheses[i].applicability_json, 1),
            other => panic!("unexpected manifest: {other}"),
        };
        assert_eq!(entry["weight"], weight);
        assert_eq!(entry["bytes"], doc.as_bytes().len());
        assert_eq!(entry["nodes"], doc.node_count());
        assert_eq!(entry["sha256"], crate::research::sha256_hex(doc.as_bytes()));
    }
}
fn conform(c: &Value, expected: &Value, warm: &Value) {
    let mut f = Fixture::new(&tables_case(c));
    let rows = f.capture();
    let saved = rows.clone();
    let before = entries();
    let wb = warm_entries();
    let result = observe_original_warmup(&rows).unwrap();
    let n = c["candidates"].as_array().unwrap().len();
    let dsl = c["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["freshStrategy"]["type"] == "dsl")
        .count();
    assert_eq!(warm_entries(), (wb.0 + n, wb.1 + dsl));
    assert_eq!(
        entries(),
        (
            before.0 + c["work"]["manifest"].as_array().unwrap().len(),
            before.1 + 1
        )
    );
    assert_eq!(observation(result.lineage()), c["expectedObservation"]);
    assert_eq!(
        serde_json::to_value(result.derivations()).unwrap(),
        *expected
    );
    assert!(result.clone() == result);
    assert!(observe_original_warmup(&rows).unwrap() == result);
    let p = prepare_original_lineage(&rows).unwrap();
    prior(&p, &c["work"]);
    manifest(&p, &c["work"]);
    work(&p, warm);
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
fn warmup_varied_params_use_each_candidate_not_base() {
    let c = literal("v1-varied-slow");
    assert_eq!(c["baseLookbacks"]["varied-slow"], 22);
    conform(&c, &c["expectedWarmup"], &c["warmupWork"]);
}
#[test]
fn warmup_mixed_config_and_candidate_order_complete_literals() {
    let c = literal("v3-mixed-varied-dsl-first");
    assert_eq!(c["configBaseIds"][0], "dsl-cross");
    assert_eq!(c["candidates"][0]["baseId"], "varied-slow");
    assert_ne!(c["configHash"], c["configRawSha256"]);
    conform(&c, &c["expectedWarmup"], &c["warmupWork"]);
}
#[test]
fn warmup_all_eight_prior_families_keep_all_23_candidates() {
    let v = oracle();
    let mut count = 0;
    for f in v["existingFamilies"].as_array().unwrap() {
        let source: Value = serde_json::from_str(
            if f["sourceFixture"] == "original-producer-projections-v1.json" {
                PRODUCERS
            } else {
                MODES
            },
        )
        .unwrap();
        let c = source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == f["caseId"])
            .unwrap();
        // Earlier four producer cases keep work in their original preparation /
        // semantic fixtures rather than rewriting any old literal file.
        let mut c = c.clone();
        if c.get("work").is_none() {
            let prep = counts(c["id"].as_str().unwrap());
            let sem = super::semantic_controls::oracle();
            let sem = sem["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == c["id"])
                .unwrap();
            c["work"] = prep;
            for k in ["hashBytes", "copyBytes", "cloneNodes", "outputBytes"] {
                c["work"][k] = sem[k].clone();
            }
            c["expectedObservation"] = sem["expectedObservation"].clone();
        }
        conform(&c, &f["expectedWarmup"], &f["warmupWork"]);
        count += c["candidates"].as_array().unwrap().len();
    }
    assert_eq!(count, 23);
}

pub(super) fn fail_members(t: &mut Value, indexes: &[usize]) {
    for &i in indexes {
        let a = &mut table(t, "research_attempts")["rows"][i];
        a[7] = json!("failed");
        a[10] = json!("{}");
        for j in [2 * i, 2 * i + 1] {
            table(t, "discovery_jobs")["rows"][j][6] = json!("failed");
        }
    }
}
pub(super) fn change(
    t: &mut Value,
    tab: &str,
    i: usize,
    col: usize,
    edit: impl FnOnce(&mut Value),
) {
    let v = &mut table(t, tab)["rows"][i][col];
    let mut d: Value = serde_json::from_str(v.as_str().unwrap()).unwrap();
    edit(&mut d);
    *v = json!(serde_json::to_string(&d).unwrap());
}
#[test]
fn warmup_nonmonotonic_actual_ids_and_failed_members_are_not_positional() {
    let c = literal("v3-mixed-varied-dsl-first");
    let v = oracle();
    let id = &v["idOrderControl"];
    let mut t = tables_case(&c);
    for i in 0..4 {
        table(&mut t, "strategy_def")["rows"][i][0] = id["strategyIdsByCandidateIndex"][i].clone();
    }
    for i in 0..2 {
        table(&mut t, "hypotheses")["rows"][i][0] =
            id["hypothesisIdsByOriginalEncounter"][i].clone();
    }
    for i in 0..4 {
        let a = &mut table(&mut t, "research_attempts")["rows"][i];
        a[2] =
            id["hypothesisIdsByOriginalEncounter"][(a[2].as_u64().unwrap() - 200) as usize].clone();
        a[3] = id["strategyIdsByCandidateIndex"][i].clone();
        a[8] = id["candidates"][i]["inputFingerprintJson"].clone();
        assert_eq!(
            crate::research::sha256_hex(a[8].as_str().unwrap().as_bytes()),
            id["candidates"][i]["inputSha256"]
        );
        for j in [2 * i, 2 * i + 1] {
            table(&mut t, "discovery_jobs")["rows"][j][3] =
                id["strategyIdsByCandidateIndex"][i].clone();
        }
    }
    fail_members(&mut t, &[2, 3]);
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    assert_eq!(
        rows.strategies.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![11, 21, 31, 41]
    );
    let result = preserves(&mut f, &rows, true).unwrap();
    let obs = observation(result.lineage());
    for i in 0..4 {
        assert_eq!(obs["candidates"][i], id["candidates"][i]["expectedLineage"]);
    }
    assert_eq!(
        serde_json::to_value(result.derivations()).unwrap(),
        v["failedMemberControl"]["expectedWarmup"]
    );
    let p = prepare_original_lineage(&rows).unwrap();
    assert_eq!(
        p.parse_work().0,
        c["work"]["parseBytes"].as_u64().unwrap() as usize + 4
    );
    work(&p, &c["warmupWork"]);
}

/// The observer must not disturb caller SQL, savepoints, lease or reservation.
pub(super) fn preserves(
    f: &mut Fixture,
    rows: &SourceRowsObservation,
    success: bool,
) -> Option<OriginalWarmupObservation> {
    let saved = rows.clone();
    let before = snapshot(f.conn());
    let observer = Connection::open(f.root.join(db::DB_FILE_NAME)).unwrap();
    observer.busy_timeout(Duration::ZERO).unwrap();
    assert!(lease::try_lock_workspace(&f.root).is_err());
    let tx = f
        .conn()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute_batch("SAVEPOINT caller_guard; UPDATE strategy_def SET name='caller pending' WHERE id=(SELECT MIN(id) FROM strategy_def);").unwrap();
    writer_busy(&observer);
    let pending = snapshot(&tx);
    let result = unchanged(&tx, || observe_original_warmup(rows));
    let result = if success {
        Some(result.unwrap())
    } else {
        refused(result);
        None
    };
    assert_eq!(snapshot(&tx), pending);
    assert!(!tx.is_autocommit());
    writer_busy(&observer);
    assert!(*rows == saved);
    tx.execute_batch("ROLLBACK TO caller_guard; RELEASE caller_guard;")
        .unwrap();
    tx.rollback().unwrap();
    assert_eq!(snapshot(f.conn()), before);
    observer
        .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        .unwrap();
    assert!(lease::try_lock_workspace(&f.root).is_err());
    result
}
#[test]
fn warmup_failed_params_and_dsl_preserve_complete_positive_caller_state() {
    let c = literal("v3-mixed-varied-dsl-first");
    let mut t = tables_case(&c);
    fail_members(&mut t, &[2, 3]);
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    let before = warm_entries();
    let r = preserves(&mut f, &rows, true).unwrap();
    assert_eq!(warm_entries(), (before.0 + 4, before.1 + 1));
    assert_eq!(observation(r.lineage()), c["expectedObservation"]);
    assert_eq!(
        serde_json::to_value(r.derivations()).unwrap(),
        c["expectedWarmup"]
    );
}
#[test]
fn warmup_late_failed_embargo_refusal_has_no_partial_result_or_caller_change() {
    let c = literal("v3-mixed-failed-embargo-overflow");
    let mut t = tables_case(&c);
    fail_members(&mut t, &[2]);
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    assert_eq!(
        observation(&observe_original_lineage(&rows).unwrap()),
        c["expectedObservation"]
    );
    let p = prepare_original_lineage(&rows).unwrap();
    prior(&p, &c["work"]);
    manifest(&p, &c["work"]);
    work(&p, &c["warmupWork"]);
    // Freeze even the successful prefix and the later DSL that the whole
    // observer must never publish after candidate2 refuses.
    assert_eq!(c["expectedFirstDerivationRefusalIndex"], 2);
    for (i, expected) in c["expectedPerCandidate"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let id = rows.attempts[i].strategy_id;
        let si = rows.strategies.iter().position(|s| s.id == id).unwrap();
        let actual = crate::discovery_runner::execution::original_candidate_embargo(
            p.strategies()[si].definition(),
            p.config().embargo.holding_allowance_bars,
        );
        if expected["status"] == "ok" {
            assert_eq!(
                serde_json::to_value(actual.unwrap()).unwrap(),
                expected["derivation"]
            );
        } else {
            assert_eq!(expected["phase"], "embargo");
            assert!(actual.is_err());
        }
    }
    let before = warm_entries();
    preserves(&mut f, &rows, false);
    assert_eq!(warm_entries(), (before.0 + 3, before.1));
}
#[test]
fn warmup_late_lineage_and_key_resource_refusals_precede_analysis() {
    let c = literal("v3-mixed-varied-dsl-first");
    for kind in 0..5 {
        let mut t = tables_case(&c);
        match kind {
            0 => {
                for col in [2, 3] {
                    change(&mut t, "strategy_def", 3, col, |d| {
                        let d = if col == 2 { &mut d["dsl"] } else { d };
                        d["entry"] = json!({"ind":"CLOSE"});
                    });
                }
            }
            1 => {
                fail_members(&mut t, &[2]);
                change(&mut t, "strategy_def", 2, 2, |d| d["feePct"] = json!(0.051));
            }
            2 => change(&mut t, "research_attempts", 3, 9, |d| {
                d["configContracts"]["strategyDsl"] = json!("unsupported-v9")
            }),
            3 => change(&mut t, "research_attempts", 3, 8, |d| {
                d["strategyId"] = json!(1)
            }),
            _ => {
                for col in [2, 3] {
                    change(&mut t, "strategy_def", 3, col, |d| {
                        let d = if col == 2 { &mut d["dsl"] } else { d };
                        d["params"]["x".repeat(65)] = json!(0);
                    });
                }
            }
        }
        let mut f = Fixture::new(&t);
        let rows = f.capture();
        let _guard = DenyWork::new(kind == 4, true);
        preserves(&mut f, &rows, false);
    }
}
#[test]
fn warmup_compatible_original_format_preserves_bytes_and_semantic_results() {
    let c = literal("v3-mixed-varied-dsl-first");
    let mut t = tables_case(&c);
    for (tab, col) in [
        ("discovery_runs", 2),
        ("strategy_def", 2),
        ("strategy_def", 3),
        ("hypotheses", 5),
        ("hypotheses", 11),
    ] {
        for row in table(&mut t, tab)["rows"].as_array_mut().unwrap() {
            if let Some(s) = row[col].as_str() {
                let d: Value = serde_json::from_str(s).unwrap();
                row[col] = json!(serde_json::to_string_pretty(&d).unwrap());
            }
        }
    }
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    let saved = rows.clone();
    let r = preserves(&mut f, &rows, true).unwrap();
    assert_eq!(observation(r.lineage()), c["expectedObservation"]);
    assert_eq!(
        serde_json::to_value(r.derivations()).unwrap(),
        c["expectedWarmup"]
    );
    assert!(rows == saved);
}
