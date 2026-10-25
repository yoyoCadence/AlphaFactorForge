use super::super::super::selection::{self, DENY_ENUM, DENY_PARSE, ENUM_ENTRIES, PARSE_ENTRIES};
use super::super::*;
use super::*;
use crate::db::{self, ownership, runtime_ledger};
use crate::research::confirmation_inputs::tests::{snapshot, unchanged, writer_busy};
use crate::runtime::lease::{self, OsLock};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection, TransactionBehavior};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

mod semantic_boundaries;
mod semantic_controls;

#[path = "boundaries.rs"]
mod boundaries;

const PRODUCERS: &str =
    include_str!("../../../../../../../fixtures/research/original-producer-projections-v1.json");
const COUNTS: &str =
    include_str!("../../../../../../../fixtures/research/original-lineage-preparation-v1.json");
const GRAPH: &str = include_str!("../../../../../../../fixtures/research/source-row-graph-v1.json");

fn case(id: &str) -> Value {
    serde_json::from_str::<Value>(PRODUCERS).unwrap()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap()
        .clone()
}
fn counts(id: &str) -> Value {
    serde_json::from_str::<Value>(COUNTS).unwrap()["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap()
        .clone()
}
fn table<'a>(tables: &'a mut Value, name: &str) -> &'a mut Value {
    tables
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|t| t["table"] == name)
        .unwrap()
}

// Literal rows are input controls, never expected values from the new adapter.
fn tables(id: &str) -> Value {
    let c = case(id);
    let mut tables = serde_json::from_str::<Value>(GRAPH).unwrap()["tables"].clone();
    table(&mut tables, "discovery_runs")["rows"][0][2] = c["configJson"].clone();
    table(&mut tables, "research_artifacts")["rows"] = json!([]);
    let hypotheses = c["hypotheses"].as_array().unwrap();
    let mut hs = Vec::new();
    for (i, h) in hypotheses.iter().enumerate() {
        let d: Value = serde_json::from_str(h["documentJson"].as_str().unwrap()).unwrap();
        hs.push(json!([
            200 + i,
            h["hash"],
            d["version"],
            d["source"],
            d["mechanism"],
            h["applicabilityJson"],
            d["failureModes"],
            d["strategyHash"],
            d["strategyId"],
            d["parentStrategyId"],
            d["variationKind"],
            h["documentJson"]
        ]));
    }
    table(&mut tables, "hypotheses")["rows"] = json!(hs);
    let mut strategies = Vec::new();
    let mut attempts = Vec::new();
    let mut jobs = Vec::new();
    for (i, candidate) in c["candidates"].as_array().unwrap().iter().enumerate() {
        let s = &candidate["freshStrategy"];
        strategies.push(json!([
            i + 1,
            s["type"],
            s["original_definition_json"],
            s["dsl_json"],
            s["strategy_hash"],
            s["parent_strategy_id"]
        ]));
        let h = hypotheses
            .iter()
            .position(|h| h["hash"] == candidate["hypothesisHash"])
            .unwrap();
        attempts.push(json!([
            400 + i,
            candidate["attemptKey"],
            200 + h,
            i + 1,
            7,
            1,
            i,
            "submitted",
            candidate["inputFingerprintJson"],
            c["engineFingerprintJson"],
            null,
            null,
            format!("trial-event-v1:{i:064x}")
        ]));
        for (j, segment) in ["train", "validation"].iter().enumerate() {
            jobs.push(json!([
                500 + 2 * i + j,
                1,
                i,
                i + 1,
                7,
                segment,
                "queued",
                null
            ]));
        }
    }
    table(&mut tables, "strategy_def")["rows"] = json!(strategies);
    table(&mut tables, "research_attempts")["rows"] = json!(attempts);
    table(&mut tables, "discovery_jobs")["rows"] = json!(jobs);
    tables
}

fn cell(v: &Value) -> SqlValue {
    match v {
        Value::Null => SqlValue::Null,
        Value::String(s) => SqlValue::Text(s.clone()),
        Value::Number(n) => SqlValue::Integer(n.as_i64().unwrap()),
        Value::Object(_) => SqlValue::Real(f64::from_bits(
            u64::from_str_radix(v["realBits"].as_str().unwrap(), 16).unwrap(),
        )),
        _ => panic!("unsupported fixture cell"),
    }
}
fn fresh_root() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..1024 {
        let root = std::env::temp_dir().join(format!(
            "aff-lineage-preparation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        match fs::create_dir(&root) {
            Ok(()) => return root,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("owned preparation root: {e}"),
        }
    }
    panic!("bounded unused preparation root allocation failed");
}
struct Fixture {
    root: PathBuf,
    conn: Option<Connection>,
    lease: Option<OsLock>,
    expected: Option<ExpectedWorkspace>,
}
impl Fixture {
    fn new(tables: &Value) -> Self {
        let mut f = Self {
            root: fresh_root(),
            conn: None,
            lease: None,
            expected: None,
        };
        f.lease = Some(lease::try_lock_workspace(&f.root).unwrap());
        f.conn = Some(db::open_at(&f.root.join(db::DB_FILE_NAME)).unwrap());
        let acquired = ownership::acquire(
            f.conn.as_mut().unwrap(),
            crate::db::ownership::HolderKind::DesktopEmbedded,
            std::process::id(),
        )
        .unwrap();
        f.expected = Some(
            ExpectedWorkspace::new(
                &runtime_ledger::workspace_id(f.conn.as_ref().unwrap()).unwrap(),
                acquired.epoch,
                crate::db::ownership::HolderKind::DesktopEmbedded,
                &acquired.instance_id,
            )
            .unwrap(),
        );
        for name in [
            "datasets",
            "strategy_def",
            "discovery_runs",
            "research_campaigns",
            "campaign_run_admissions",
            "market_calendars",
            "market_instruments",
            "market_provenance",
            "market_snapshots",
            "market_snapshot_sources",
            "hypotheses",
            "research_artifacts",
            "discovery_jobs",
            "research_attempts",
        ] {
            let t = tables
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["table"] == name)
                .unwrap();
            let mut columns: Vec<String> = t["columns"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().into())
                .collect();
            let extras = match name {
                "discovery_runs" => vec![("name", "preparation control")],
                "strategy_def" => vec![("name", "literal claim"), ("source", "manual")],
                _ => vec![],
            };
            columns.extend(extras.iter().map(|(c, _)| c.to_string()));
            let sql = format!(
                "INSERT INTO {name} ({}) VALUES({})",
                columns.join(","),
                vec!["?"; columns.len()].join(",")
            );
            for row in t["rows"].as_array().unwrap() {
                let mut values: Vec<_> = row.as_array().unwrap().iter().map(cell).collect();
                values.extend(extras.iter().map(|(_, v)| SqlValue::Text(v.to_string())));
                f.conn
                    .as_ref()
                    .unwrap()
                    .execute(&sql, params_from_iter(values))
                    .unwrap();
            }
        }
        f
    }
    fn conn(&mut self) -> &mut Connection {
        self.conn.as_mut().unwrap()
    }
    fn capture(&mut self) -> SourceRowsObservation {
        let expected = self.expected.as_ref().unwrap().clone();
        let graph: Value = serde_json::from_str(GRAPH).unwrap();
        let locator = SourceLocator::new(graph["campaignId"].as_str().unwrap(), 1).unwrap();
        let tx = self.conn().transaction().unwrap();
        let rows = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator)).unwrap();
        tx.rollback().unwrap();
        rows
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.conn.take());
        drop(self.lease.take());
        if let Err(e) = fs::remove_dir_all(&self.root) {
            if std::thread::panicking() {
                eprintln!("owned preparation cleanup: {e}");
            } else {
                panic!("owned preparation cleanup: {e}");
            }
        }
    }
}
fn refused<T>(result: AppResult<T>) {
    match result {
        Err(AppError::Other(code)) => assert_eq!(code, "original_lineage_invalid"),
        Err(e) => panic!("wrong refusal: {e}"),
        Ok(_) => panic!("expected whole preparation refusal"),
    }
}
fn entries() -> (usize, usize) {
    (
        PARSE_ENTRIES.with(std::cell::Cell::get),
        ENUM_ENTRIES.with(std::cell::Cell::get),
    )
}
struct Deny {
    parse: bool,
    enumeration: bool,
}
impl Deny {
    fn new(parse: bool, enumeration: bool) -> Self {
        DENY_PARSE.with(|v| v.set(parse));
        DENY_ENUM.with(|v| v.set(enumeration));
        Self { parse, enumeration }
    }
}
impl Drop for Deny {
    fn drop(&mut self) {
        if self.parse {
            DENY_PARSE.with(|v| v.set(false));
        }
        if self.enumeration {
            DENY_ENUM.with(|v| v.set(false));
        }
    }
}
fn conforms(id: &str) {
    let c = case(id);
    let expected = counts(id);
    let mut f = Fixture::new(&tables(id));
    let rows = f.capture();
    let saved = rows.clone();
    let before = entries();
    let prepared = prepare_original_lineage(&rows).unwrap();
    let after = entries();
    assert!(std::ptr::eq(prepared.rows(), &rows));
    assert_eq!(
        after,
        (
            before.0 + expected["manifest"].as_array().unwrap().len(),
            before.1 + 1
        )
    );
    assert_eq!(
        prepared.parse_work(),
        (
            expected["parseBytes"].as_u64().unwrap() as usize,
            expected["parseNodes"].as_u64().unwrap() as usize
        )
    );
    assert_eq!(
        prepared.expansion_work(),
        (
            expected["expansionBytes"].as_u64().unwrap() as usize,
            expected["expansionNodes"].as_u64().unwrap() as usize
        )
    );
    assert_eq!(
        prepared.combined_work(),
        (
            expected["scratchBytes"].as_u64().unwrap() as usize,
            expected["scratchNodes"].as_u64().unwrap() as usize
        )
    );
    assert_eq!(
        prepared.raw_config(),
        &serde_json::from_str::<Value>(c["configJson"].as_str().unwrap()).unwrap()
    );
    assert_eq!(prepared.config().dataset.id, 7);
    assert_eq!(
        prepared.plan().counts.raw,
        expected["rawCandidates"].as_i64().unwrap()
    );
    assert_eq!(
        prepared.plan().candidates.len(),
        expected["finalCandidates"].as_u64().unwrap() as usize
    );
    for ((p, candidate), expected) in prepared
        .strategies()
        .iter()
        .zip(&prepared.plan().candidates)
        .zip(c["candidates"].as_array().unwrap())
    {
        assert_eq!(
            p.definition(),
            &serde_json::from_str::<Value>(
                expected["freshStrategy"]["original_definition_json"]
                    .as_str()
                    .unwrap()
            )
            .unwrap()
        );
        assert_eq!(candidate.strategy_hash, expected["strategyHash"]);
        assert_eq!(candidate.base_id, expected["baseId"]);
        assert_eq!(
            serde_json::to_value(candidate.seeds).unwrap(),
            expected["seeds"]
        );
        assert_eq!(
            serde_json::to_value(&candidate.applied_axes).unwrap(),
            expected["appliedAxes"]
        );
        assert_eq!(
            p.dsl().cloned(),
            expected["freshStrategy"]["dsl_json"]
                .as_str()
                .map(|s| serde_json::from_str::<Value>(s).unwrap())
        );
    }
    for (p, a) in prepared
        .attempts()
        .iter()
        .zip(c["candidates"].as_array().unwrap())
    {
        assert_eq!(
            p.input(),
            &serde_json::from_str::<Value>(a["inputFingerprintJson"].as_str().unwrap()).unwrap()
        );
        assert_eq!(
            p.engine(),
            &serde_json::from_str::<Value>(c["engineFingerprintJson"].as_str().unwrap()).unwrap()
        );
    }
    for (p, h) in prepared
        .hypotheses()
        .iter()
        .zip(c["hypotheses"].as_array().unwrap())
    {
        assert_eq!(
            p.content(),
            &serde_json::from_str::<Value>(h["documentJson"].as_str().unwrap()).unwrap()
        );
        assert_eq!(
            p.applicability(),
            &serde_json::from_str::<Value>(h["applicabilityJson"].as_str().unwrap()).unwrap()
        );
    }
    assert!(rows == saved);
    for table in [
        "candles",
        "research_artifacts",
        "backtest_summary",
        "trades",
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
fn ordered_axes_literal_graph_prepares_once() {
    conforms("v1-ordered-axes");
}
#[test]
fn shared_hypotheses_and_per_attempt_engines_have_exact_work() {
    conforms("v1-shared-dedup");
}
#[test]
fn fixed_dsl_literal_graph_prepares_once() {
    conforms("v2-fixed-dsl");
}
#[test]
fn walk_forward_literal_graph_prepares_once() {
    conforms("v3-walk-forward");
}

#[test]
fn selected_nonrepresentable_numbers_refuse_and_excluded_claims_stay_unparsed() {
    let mut t = tables("v1-ordered-axes");
    table(&mut t, "campaign_run_admissions")["rows"][0][7] = json!("{\"uninterpreted\":1e9999}");
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    prepare_original_lineage(&rows).unwrap();
    table(&mut t, "strategy_def")["rows"][0][2] = json!("{\"uninterpreted\":1e9999}");
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    refused(prepare_original_lineage(&rows));
}
#[test]
fn legacy_numeric_claim_is_prepared_without_repair_or_semantic_acceptance() {
    let audit: Value = serde_json::from_str(include_str!(
        "../../../../../../../fixtures/research/numeric-json-audit-v1.json"
    ))
    .unwrap();
    let numeric = audit["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "number-3")
        .unwrap();
    let mut t = tables("v1-ordered-axes");
    table(&mut t, "strategy_def")["rows"][0][2] = numeric["definitionJson"].clone();
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    let p = prepare_original_lineage(&rows).unwrap();
    assert_eq!(
        p.strategies()[0].definition()["feePct"]
            .as_f64()
            .unwrap()
            .to_bits(),
        0x3f6e43cfc21ad9ff
    );
    assert_eq!(
        rows.strategies[0].original_definition_json.as_bytes(),
        numeric["definitionJson"].as_str().unwrap().as_bytes()
    );
}
#[test]
fn original_synthetic_nonrepresentable_claims_refuse_without_repair() {
    let t = serde_json::from_str::<Value>(GRAPH).unwrap()["tables"].clone();
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    refused(prepare_original_lineage(&rows));
}
#[test]
fn finite_uninterpreted_claim_is_only_syntactic_preparation() {
    let mut t = tables("v1-ordered-axes");
    table(&mut t, "strategy_def")["rows"][0][2] = json!("{\"claimOnly\":true}");
    let mut f = Fixture::new(&t);
    let rows = f.capture();
    let p = prepare_original_lineage(&rows).unwrap();
    assert_eq!(p.strategies()[0].definition(), &json!({"claimOnly":true}));
    assert_ne!(
        p.strategies()[0].definition(),
        &p.plan().candidates[0].strategy
    );
}
#[test]
fn caller_transaction_owner_and_mutation_stay_unchanged() {
    for reject in [false, true] {
        let mut t = tables("v2-fixed-dsl");
        if reject {
            table(&mut t, "strategy_def")["rows"][0][2] = json!("{\"unrepresentable\":1e9999}");
        }
        let mut f = Fixture::new(&t);
        let rows = f.capture();
        let observer = Connection::open(f.root.join(db::DB_FILE_NAME)).unwrap();
        observer.busy_timeout(Duration::ZERO).unwrap();
        let saved = snapshot(f.conn());
        let tx = f
            .conn()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        writer_busy(&observer);
        let result = unchanged(&tx, || prepare_original_lineage(&rows));
        if reject {
            refused(result);
        } else {
            result.unwrap();
        }
        writer_busy(&observer);
        tx.rollback().unwrap();
        assert_eq!(snapshot(f.conn()), saved);
    }
}
