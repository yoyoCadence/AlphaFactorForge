use super::super::super::tests::{snapshot, unchanged, writer_busy, Fixture};
use super::*;
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection, TransactionBehavior};
use serde_json::{json, Value};

#[path = "compare_tests.rs"]
mod compare_tests;

fn oracle() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../../fixtures/research/source-row-graph-v1.json"
    ))
    .unwrap()
}

pub(super) fn locator() -> SourceLocator {
    SourceLocator::new(oracle()["campaignId"].as_str().unwrap(), 1).unwrap()
}

fn sql_value(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Number(n) => SqlValue::Integer(n.as_i64().unwrap()),
        Value::String(s) => SqlValue::Text(s.clone()),
        Value::Object(_) => SqlValue::Real(f64::from_bits(
            u64::from_str_radix(value["realBits"].as_str().unwrap(), 16).unwrap(),
        )),
        _ => panic!("invalid fixture cell"),
    }
}

pub(super) fn fixture() -> Fixture {
    fixture_with_lineage(true)
}

fn fixture_with_lineage(include_lineage: bool) -> Fixture {
    let mut fixture = Fixture::new();
    let conn = fixture.connection();
    let expected = oracle();
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
        if !include_lineage && matches!(name, "hypotheses" | "research_attempts") {
            continue;
        }
        let table = expected["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["table"] == name)
            .unwrap();
        let mut columns: Vec<String> = table["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap().into())
            .collect();
        let extras = match name {
            "discovery_runs" => vec![("name", "graph fixture")],
            "strategy_def" => vec![("name", "graph claim"), ("source", "manual")],
            _ => vec![],
        };
        for (column, _) in &extras {
            columns.push((*column).into());
        }
        let sql = format!(
            "INSERT INTO {name} ({}) VALUES({})",
            columns.join(","),
            vec!["?"; columns.len()].join(",")
        );
        for row in table["rows"].as_array().unwrap() {
            let mut cells: Vec<SqlValue> = row.as_array().unwrap().iter().map(sql_value).collect();
            cells.extend(extras.iter().map(|(_, v)| SqlValue::Text((*v).into())));
            conn.execute(&sql, params_from_iter(cells)).unwrap();
        }
    }
    fixture
}

pub(super) fn capture(f: &mut Fixture) -> SourceRowsObservation {
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    let observed = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator())).unwrap();
    tx.rollback().unwrap();
    observed
}

fn refused<T>(result: AppResult<T>, code: &str) {
    match result {
        Err(AppError::Other(actual)) => assert_eq!(actual, code),
        Err(other) => panic!("wrong error: {other:?}"),
        Ok(_) => panic!("expected {code}"),
    }
}

fn invalid<T>(result: AppResult<T>) {
    refused(result, "source_rows_invalid");
}

// Test-owned damage only. Remove all constraints/triggers in one table so the
// production projection can be challenged with raw types and duplicate keys.
fn unconstrained(conn: &Connection, table: &str) {
    let columns: Vec<String> = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .map(|c| format!("\"{}\"", c.unwrap()))
        .collect();
    conn.execute_batch(&format!("CREATE TABLE damaged_graph ({}); INSERT INTO damaged_graph SELECT * FROM {table}; DROP TABLE {table}; ALTER TABLE damaged_graph RENAME TO {table}",columns.join(","))).unwrap();
}

pub(super) fn damage(f: &mut Fixture) {
    f.connection()
        .execute_batch("PRAGMA foreign_keys=OFF")
        .unwrap();
    for name in [
        "discovery_jobs",
        "research_attempts",
        "strategy_def",
        "hypotheses",
        "research_artifacts",
        "market_snapshots",
        "market_instruments",
        "market_calendars",
        "datasets",
        "market_snapshot_sources",
        "market_provenance",
    ] {
        unconstrained(f.connection(), name);
    }
}

#[derive(Debug, PartialEq)]
enum Cell {
    Null,
    Integer(i64),
    Real(u64),
    Text(Vec<u8>),
}
trait Observed {
    fn cell(&self) -> Cell;
}
impl Observed for i64 {
    fn cell(&self) -> Cell {
        Cell::Integer(*self)
    }
}
impl Observed for u64 {
    fn cell(&self) -> Cell {
        Cell::Real(*self)
    }
}
impl Observed for String {
    fn cell(&self) -> Cell {
        Cell::Text(self.as_bytes().to_vec())
    }
}
impl Observed for OriginalJson {
    fn cell(&self) -> Cell {
        Cell::Text(self.as_bytes().to_vec())
    }
}
impl<T: Observed> Observed for Option<T> {
    fn cell(&self) -> Cell {
        self.as_ref().map_or(Cell::Null, Observed::cell)
    }
}
macro_rules! cells { ($row:expr; $($field:ident),+ $(,)?) => { vec![$(($row).$field.cell()),+] }; }

fn expected_cell(value: &Value) -> Cell {
    match sql_value(value) {
        SqlValue::Null => Cell::Null,
        SqlValue::Integer(n) => Cell::Integer(n),
        SqlValue::Real(n) => Cell::Real(n.to_bits()),
        SqlValue::Text(s) => Cell::Text(s.into_bytes()),
        _ => panic!("fixture cell"),
    }
}

fn observed_tables(s: &SourceRowsObservation) -> Vec<Vec<Vec<Cell>>> {
    vec![
        vec![cells!(&s.roots.campaign;campaign_id,version,document_json)],
        vec![cells!(&s.roots.admission;id,discovery_run_id,campaign_id,instrument_id,batch_id,status,report_version,report_json,fee_pct_bits,slip_pct_bits)],
        vec![cells!(&s.roots.run;id,status,config_json)],
        s.jobs.iter().map(|r|cells!(r;id,discovery_run_id,candidate_index,strategy_id,dataset_id,segment,status,result_id)).collect(),
        s.attempts.iter().map(|r|cells!(r;id,attempt_key,hypothesis_id,strategy_id,dataset_id,discovery_run_id,candidate_index,status,input_fingerprint_json,engine_fingerprint_json,outcome_json,result_artifact_id,trial_event_id)).collect(),
        s.strategies.iter().map(|r|cells!(r;id,r#type,original_definition_json,dsl_json,strategy_hash,parent_strategy_id)).collect(),
        s.hypotheses.iter().map(|r|cells!(r;id,hypothesis_hash,version,source,mechanism,applicability_json,failure_modes,strategy_hash,strategy_id,parent_strategy_id,variation_kind,content_json)).collect(),
        s.artifacts.iter().map(|r|cells!(r;id,sha256,kind,byte_len,relative_path)).collect(),
        vec![cells!(&s.snapshot;id,snapshot_id,version,instrument_row_id,instrument_id,interval,dataset_id,dataset_hash,price_basis,calendar_id,corporate_action_version,cost_profile_version,kind,status,as_of,coverage_json,content_json)],
        vec![cells!(&s.instrument;id,instrument_id,revision,content_hash,version,market,venue,symbol,base,quote,asset_type,session_calendar_id,timezone,lot_size_bits,price_step_bits,min_notional_bits,listed_from,delisted_at,suspensions_json,source_capabilities_json,content_json)],
        vec![cells!(&s.calendar;id,calendar_id,version,kind,timezone,content_hash,definition_json)],
        vec![cells!(&s.dataset;id,exchange,symbol,interval,start_time,end_time,candle_count,source,dataset_hash)],
        s.sources.iter().map(|r|cells!(r;snapshot_row_id,provenance_id)).collect(),
        s.provenance.iter().map(|r|cells!(r;id,record_hash,version,instrument_id,interval,source,role,request_scope_json,raw_sha256,raw_byte_len,raw_artifact_path,raw_media_type,retrieved_at,available_at,accepted,rejection_reason,revision_of)).collect(),
    ]
}

#[test]
fn complete_graph_matches_all_independent_ordered_cells_and_usage() {
    let mut f = fixture();
    let saved = capture(&mut f);
    let expected = oracle();
    let actual = observed_tables(&saved);
    assert_eq!(actual.len(), 14);
    for (rows, table) in actual.iter().zip(expected["tables"].as_array().unwrap()) {
        let expected_rows: Vec<Vec<Cell>> = table["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_array().unwrap().iter().map(expected_cell).collect())
            .collect();
        assert_eq!(*rows, expected_rows, "{}", table["table"]);
    }
    assert_eq!(saved.usage.text_bytes, 6102);
    assert_eq!(saved.usage.json_nodes, 184);
    assert_eq!(
        expected["expectedUsage"],
        json!({"textBytes":6102,"jsonNodes":184})
    );
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)).unwrap();
    assert!(!tx.is_autocommit());
    tx.rollback().unwrap();
}

#[test]
fn actual_unchanged_writer_shared_base_hypothesis_captures_and_rechecks() {
    use crate::research::history;
    use alpha_factor_forge::discovery_core::config::parse_discovery_config;
    let mut f = fixture_with_lineage(false);
    let oracle = oracle();
    let raw: Value =
        serde_json::from_str(oracle["tables"][2]["rows"][0][2].as_str().unwrap()).unwrap();
    let config = parse_discovery_config(&raw, 64.0).unwrap();
    let lineage = crate::discovery_runner::source_row_lineage_for_test(
        &config,
        &raw,
        1,
        &[100, 101, 102],
        "1h",
    )
    .unwrap();
    assert_eq!(lineage.len(), 3);
    for (hypothesis, _) in &lineage {
        assert_eq!(
            hypothesis.strategy_hash,
            oracle["baseStrategyDocumentHash"].as_str().unwrap()
        );
        assert!(hypothesis.strategy_id.is_none());
    }
    let events: Vec<String> = oracle["tables"][4]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[12].as_str().unwrap().into())
        .collect();
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    let registered =
        history::register_run_lineage_with_events(&tx, &lineage, Some(&events)).unwrap();
    assert_eq!(registered.len(), 3);
    assert!(registered
        .iter()
        .all(|(hypothesis, _)| *hypothesis == registered[0].0));
    let saved = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator())).unwrap();
    assert_eq!(saved.hypotheses.len(), 1);
    assert_eq!(saved.attempts.len(), 3);
    assert_eq!(saved.jobs.len(), 6);
    assert!(saved.hypotheses[0].strategy_id.is_none());
    assert_eq!(
        saved.hypotheses[0].strategy_hash,
        oracle["baseStrategyDocumentHash"].as_str().unwrap()
    );
    for (i, attempt) in saved.attempts.iter().enumerate() {
        assert_eq!(attempt.hypothesis_id, registered[0].0);
        assert_eq!(attempt.strategy_id, 100 + i as i64);
        assert_eq!(attempt.trial_event_id.as_ref(), Some(&events[i]));
    }
    let digests = unchanged(&tx, || original_digests::observe_raw_digests(&saved)).unwrap();
    assert_eq!(digests.candidates().len(), 3);
    assert_eq!(digests.engine_manifest_hash().len(), 64);
    for (i, candidate) in digests.candidates().iter().enumerate() {
        assert_eq!(candidate.index(), i as i64);
        assert_eq!(candidate.definition_hash().len(), 64);
        assert_eq!(candidate.execution_hash().len(), 64);
        assert_eq!(candidate.lineage_hash().len(), 64);
    }
    unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
}

#[test]
fn candidate_bound_nonnull_hypotheses_remain_exact_references() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    tx.execute_batch("UPDATE hypotheses SET strategy_id=100,strategy_hash=(SELECT strategy_hash FROM strategy_def WHERE id=100),source='manual'; INSERT INTO hypotheses SELECT 201,hypothesis_hash,version,source,mechanism,applicability_json,failure_modes,(SELECT strategy_hash FROM strategy_def WHERE id=101),101,parent_strategy_id,variation_kind,content_json,created_at FROM hypotheses WHERE id=200; INSERT INTO hypotheses SELECT 202,hypothesis_hash,version,source,mechanism,applicability_json,failure_modes,(SELECT strategy_hash FROM strategy_def WHERE id=102),102,parent_strategy_id,variation_kind,content_json,created_at FROM hypotheses WHERE id=200; UPDATE research_attempts SET hypothesis_id=200+candidate_index").unwrap();
    let saved = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
    assert_eq!(saved.hypotheses.len(), 3);
    for (i, hypothesis) in saved.hypotheses.iter().enumerate() {
        assert_eq!(hypothesis.strategy_id, Some(100 + i as i64));
        assert_eq!(hypothesis.strategy_hash, saved.strategies[i].strategy_hash);
    }
    unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)).unwrap();
    tx.execute_batch("UPDATE hypotheses SET strategy_id=999 WHERE id=200")
        .unwrap();
    invalid(capture_source_rows_in(&tx, &expected, &locator()));
    tx.rollback().unwrap();
}

#[test]
fn original_manifest_refuses_consistent_deletion_extra_members_and_broken_links() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    for sql in [
        "DELETE FROM discovery_jobs WHERE candidate_index=1; DELETE FROM research_attempts WHERE candidate_index=1",
        "DELETE FROM discovery_jobs; DELETE FROM research_attempts",
        "DELETE FROM discovery_jobs WHERE segment='validation' AND candidate_index=1",
        "DELETE FROM research_attempts WHERE candidate_index=1",
        "INSERT INTO discovery_jobs SELECT * FROM discovery_jobs WHERE id=500",
        "INSERT INTO research_attempts SELECT * FROM research_attempts WHERE id=400",
        "UPDATE discovery_jobs SET segment='test' WHERE id=500",
        "UPDATE discovery_jobs SET candidate_index=3 WHERE id=500",
        "UPDATE research_attempts SET candidate_index=NULL WHERE id=400",
        "UPDATE research_attempts SET discovery_run_id=NULL WHERE id=400",
        "UPDATE research_attempts SET trial_event_id=NULL WHERE id=400",
        "UPDATE research_attempts SET trial_event_id=(SELECT trial_event_id FROM research_attempts WHERE id=401) WHERE id=400",
        "UPDATE research_attempts SET attempt_key='run:2:candidate:0' WHERE id=400",
        "UPDATE research_attempts SET discovery_run_id=2 WHERE id=400",
        "UPDATE discovery_jobs SET dataset_id=8 WHERE id=500",
        "UPDATE research_attempts SET dataset_id=8 WHERE id=400",
        "UPDATE discovery_jobs SET strategy_id=101 WHERE id=500",
        "UPDATE research_attempts SET strategy_id=101 WHERE id=400",
        "UPDATE research_attempts SET result_artifact_id=NULL WHERE id=400",
        "DELETE FROM research_artifacts WHERE id=300",
        "DELETE FROM hypotheses WHERE id=200",
        "DELETE FROM strategy_def WHERE id=100",
        "UPDATE hypotheses SET strategy_id=999 WHERE id=200",
        "UPDATE hypotheses SET strategy_id=101 WHERE id=200",
        "UPDATE strategy_def SET strategy_hash='strategy-v2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' WHERE id=100",
        "UPDATE strategy_def SET parent_strategy_id=999 WHERE id=100",
        "UPDATE hypotheses SET parent_strategy_id=999 WHERE id=200",
        "DELETE FROM market_snapshots",
        "DELETE FROM market_instruments",
        "DELETE FROM market_calendars",
        "DELETE FROM datasets",
        "DELETE FROM market_provenance WHERE id=14",
        "INSERT INTO market_snapshot_sources SELECT * FROM market_snapshot_sources WHERE provenance_id=14",
        "UPDATE market_snapshots SET instrument_id='other'",
        "UPDATE market_snapshots SET interval='4h'",
        "UPDATE market_snapshots SET dataset_id=8",
        "UPDATE market_snapshots SET dataset_hash='dataset-content-v2:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'",
        "UPDATE market_instruments SET instrument_id='other'",
        "UPDATE market_instruments SET session_calendar_id='other'",
        "UPDATE datasets SET interval='4h'",
        "UPDATE datasets SET dataset_hash='dataset-content-v2:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'",
        "UPDATE market_provenance SET instrument_id='other' WHERE id=14",
        "UPDATE market_provenance SET interval='4h' WHERE id=14",
        "UPDATE market_provenance SET rejection_reason='' WHERE id=14",
        "UPDATE market_provenance SET rejection_reason=NULL WHERE id=15",
    ] {
        let tx=f.connection().transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        invalid(unchanged(&tx,||capture_source_rows_in(&tx,&expected,&locator())));
        tx.rollback().unwrap();
    }
}

#[test]
fn every_referenced_singleton_refuses_damaged_schema_duplicates() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    for (table, predicate) in [
        ("strategy_def", "id=100"),
        ("hypotheses", "id=200"),
        ("research_artifacts", "id=300"),
        ("market_snapshots", "id=13"),
        ("market_instruments", "id=12"),
        ("market_calendars", "id=11"),
        ("datasets", "id=7"),
        ("market_provenance", "id=14"),
    ] {
        let tx = f.connection().transaction().unwrap();
        tx.execute_batch(&format!(
            "INSERT INTO {table} SELECT * FROM {table} WHERE {predicate}"
        ))
        .unwrap();
        invalid(unchanged(&tx, || {
            capture_source_rows_in(&tx, &expected, &locator())
        }));
        tx.rollback().unwrap();
    }
}

#[test]
fn shared_artifact_storage_is_charged_once_and_raw_unsuitable_claims_are_preserved() {
    let mut f = fixture();
    let before = capture(&mut f);
    damage(&mut f);
    f.connection().execute_batch("UPDATE research_attempts SET result_artifact_id=300; UPDATE strategy_def SET parent_strategy_id=100; UPDATE hypotheses SET parent_strategy_id=100; UPDATE research_attempts SET status='failed',result_artifact_id=NULL WHERE id=401").unwrap();
    let after = capture(&mut f);
    assert_eq!(after.jobs.len(), 6);
    assert_eq!(after.attempts.len(), 3);
    assert_eq!(after.artifacts.len(), 1);
    assert_eq!(
        after.usage.text_bytes,
        before.usage.text_bytes - 2 * (64 + 14 + 22) - 3
    ); // two artifact payloads; completed -> failed
    assert_eq!(after.usage.json_nodes, before.usage.json_nodes);
    assert_eq!(after.provenance[1].role, "comparison");
    assert_eq!(after.provenance[1].accepted, 0);
    assert!(after.snapshot.cost_profile_version.is_none());
    assert!(after.snapshot.corporate_action_version.is_none());
}

#[test]
fn diagnostic_changes_and_unrelated_rows_stay_outside_the_closure() {
    let mut f = fixture();
    let saved = capture(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    tx.execute_batch("UPDATE discovery_jobs SET error_message='display',updated_at='later'; UPDATE strategy_def SET name='display',source='other',param_schema_json='not json',lifecycle='rejected'; UPDATE datasets SET created_at='other',updated_at='later'; UPDATE research_attempts SET epoch=-10,finished_at='later' WHERE 0; INSERT INTO discovery_runs(id,name,status,config_json) VALUES(2,'unrelated','completed','not-json'); INSERT INTO datasets(id,exchange,symbol,interval,start_time,end_time,source,dataset_hash) VALUES(8,'other','other','other',0,0,'other','unrelated')").unwrap();
    unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
}

#[test]
fn pending_graph_visibility_full_rollback_and_writer_reservation_are_caller_owned() {
    let mut f = fixture();
    let saved = capture(&mut f);
    damage(&mut f);
    let expected = f.expected.clone();
    let mut observer = f.independent();
    let baseline = snapshot(f.connection());
    let tx = f
        .connection()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    writer_busy(&observer);
    tx.execute_batch("UPDATE market_instruments SET venue='pending'")
        .unwrap();
    let pending = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator())).unwrap();
    assert!(pending != saved);
    let other = observer.transaction().unwrap();
    assert!(capture_source_rows_in(&other, &expected, &locator()).unwrap() == saved);
    assert_eq!(snapshot(&other), baseline);
    other.rollback().unwrap();
    refused(
        unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)),
        "source_rows_changed",
    );
    writer_busy(&observer);
    tx.execute_batch("DELETE FROM research_artifacts WHERE id=300")
        .unwrap();
    invalid(unchanged(&tx, || {
        capture_source_rows_in(&tx, &expected, &locator())
    }));
    writer_busy(&observer);
    tx.execute_batch("ALTER TABLE hypotheses RENAME TO pending_missing_hypotheses")
        .unwrap();
    assert!(matches!(
        unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator())),
        Err(AppError::Db(_))
    ));
    writer_busy(&observer);
    tx.rollback().unwrap();
    assert_eq!(snapshot(f.connection()), baseline);
    observer.execute_batch("BEGIN IMMEDIATE; ROLLBACK").unwrap();
    assert!(capture(&mut f) == saved);
}

#[test]
fn deferred_equality_is_historical_and_fresh_graph_change_refuses() {
    let mut f = fixture();
    let saved = capture(&mut f);
    damage(&mut f);
    let expected = f.expected.clone();
    let writer = f.independent();
    let tx = f.connection().transaction().unwrap();
    assert!(capture_source_rows_in(&tx, &expected, &locator()).unwrap() == saved);
    writer
        .execute_batch("UPDATE market_instruments SET venue='new' ")
        .unwrap();
    unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
    let tx = f.connection().transaction().unwrap();
    refused(
        unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)),
        "source_rows_changed",
    );
    tx.rollback().unwrap();
}

#[test]
fn full_fresh_closure_validation_precedes_comparison_but_workspace_errors_are_first() {
    let mut f = fixture();
    let saved = capture(&mut f);
    damage(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    tx.execute_batch("DROP TRIGGER campaign_run_admissions_no_update; UPDATE campaign_run_admissions SET report_json='{}'; DELETE FROM research_artifacts WHERE id=300").unwrap();
    invalid(unchanged(&tx, || {
        recheck_source_rows_in(&tx, &expected, &saved)
    }));
    tx.execute_batch("UPDATE runtime_state SET mutation_seq=mutation_seq+1")
        .unwrap();
    refused(
        unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)),
        "workspace_observation_changed",
    );
    tx.execute_batch("UPDATE workspace_ownership SET epoch=epoch+1")
        .unwrap();
    assert!(matches!(
        unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)),
        Err(AppError::StaleOwner(_))
    ));
    tx.rollback().unwrap();
}

#[test]
fn every_graph_column_refuses_blob_and_every_text_refuses_invalid_utf8() {
    let mut f = fixture();
    let saved = capture(&mut f);
    damage(&mut f);
    let expected = f.expected.clone();
    let oracle = oracle();
    for table in oracle["tables"].as_array().unwrap().iter().skip(3) {
        let name = table["table"].as_str().unwrap();
        let first = &table["rows"][0];
        for (i, column) in table["columns"].as_array().unwrap().iter().enumerate() {
            let column = column.as_str().unwrap();
            for substitute in ["x'ff'", "CAST(x'ff' AS TEXT)"] {
                // Invalid TEXT may coerce only in the intentionally untyped fixture;
                // production SQL always checks storage class before typed reads.
                let tx = f.connection().transaction().unwrap();
                tx.execute_batch(&format!("UPDATE {name} SET \"{column}\"={substitute} WHERE rowid=(SELECT min(rowid) FROM {name})")).unwrap();
                let result = capture_source_rows_in(&tx, &expected, &locator());
                if name == "market_snapshot_sources" && column == "snapshot_row_id" {
                    // The changed link leaves this exact WHERE scope. A fresh
                    // SQL closure has one source; saved membership must stale.
                    assert_eq!(result.unwrap().sources.len(), 1);
                    refused(
                        recheck_source_rows_in(&tx, &expected, &saved),
                        "source_rows_changed",
                    );
                } else {
                    assert!(
                        matches!(result,Err(AppError::Other(ref code)) if code=="source_rows_invalid"),
                        "{name}.{column}: {substitute}"
                    );
                }
                tx.rollback().unwrap();
            }
            if !first[i].is_null() {
                // Required NULL domains, except schema-optional columns populated
                // in the base fixture (those have their own optionality test).
                let nullable = matches!(
                    (name, column),
                    (
                        "research_attempts",
                        "discovery_run_id"
                            | "candidate_index"
                            | "outcome_json"
                            | "result_artifact_id"
                            | "trial_event_id"
                    ) | ("hypotheses", "strategy_id")
                        | ("market_instruments", "price_step" | "listed_from")
                );
                if !(nullable || name == "market_snapshot_sources" && column == "snapshot_row_id") {
                    let tx = f.connection().transaction().unwrap();
                    tx.execute_batch(&format!("UPDATE {name} SET \"{column}\"=NULL WHERE rowid=(SELECT min(rowid) FROM {name})")).unwrap();
                    invalid(capture_source_rows_in(&tx, &expected, &locator()));
                    tx.rollback().unwrap();
                }
            }
        }
    }
}

#[test]
fn nullable_cells_preserve_null_empty_and_original_json_null_without_coercion() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    tx.execute_batch("UPDATE discovery_jobs SET result_id=77; UPDATE research_attempts SET outcome_json=NULL,status='failed',result_artifact_id=NULL WHERE id=400; UPDATE strategy_def SET dsl_json='null',parent_strategy_id=100; UPDATE hypotheses SET parent_strategy_id=100,variation_kind=''; UPDATE market_snapshots SET corporate_action_version='',cost_profile_version=''; UPDATE market_instruments SET lot_size=0.0,price_step=NULL,min_notional=-0.0,listed_from=NULL,delisted_at=-9223372036854775808; UPDATE market_provenance SET available_at='',revision_of=999 WHERE id=14").unwrap();
    let rows = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
    assert_eq!(rows.jobs[0].result_id, Some(77));
    assert!(rows.attempts[0].outcome_json.is_none());
    assert_eq!(
        rows.attempts[1].outcome_json.as_ref().unwrap().as_bytes(),
        b"null"
    );
    assert_eq!(
        rows.strategies[0].dsl_json.as_ref().unwrap().as_bytes(),
        b"null"
    );
    assert_eq!(rows.hypotheses[0].variation_kind.as_deref(), Some(""));
    assert_eq!(rows.snapshot.corporate_action_version.as_deref(), Some(""));
    assert_eq!(rows.snapshot.cost_profile_version.as_deref(), Some(""));
    assert_eq!(rows.instrument.lot_size_bits, Some(0.0f64.to_bits()));
    assert_eq!(rows.instrument.min_notional_bits, Some((-0.0f64).to_bits()));
    assert!(rows.instrument.price_step_bits.is_none());
    assert!(rows.instrument.listed_from.is_none());
    assert_eq!(rows.instrument.delisted_at, Some(i64::MIN));
    assert_eq!(rows.provenance[0].available_at.as_deref(), Some(""));
    assert_eq!(rows.provenance[0].revision_of, Some(999)); // historical metadata, no expansion
    tx.rollback().unwrap();
    for (table, column) in [
        ("discovery_jobs", "result_id"),
        ("research_attempts", "discovery_run_id"),
        ("research_attempts", "candidate_index"),
        ("research_attempts", "outcome_json"),
        ("research_attempts", "result_artifact_id"),
        ("research_attempts", "trial_event_id"),
        ("strategy_def", "dsl_json"),
        ("strategy_def", "parent_strategy_id"),
        ("hypotheses", "strategy_id"),
        ("hypotheses", "parent_strategy_id"),
        ("hypotheses", "variation_kind"),
        ("market_snapshots", "corporate_action_version"),
        ("market_snapshots", "cost_profile_version"),
        ("market_instruments", "lot_size"),
        ("market_instruments", "price_step"),
        ("market_instruments", "min_notional"),
        ("market_instruments", "listed_from"),
        ("market_instruments", "delisted_at"),
        ("market_provenance", "available_at"),
        ("market_provenance", "rejection_reason"),
        ("market_provenance", "revision_of"),
    ] {
        let tx = f.connection().transaction().unwrap();
        tx.execute_batch(&format!(
            "UPDATE {table} SET {column}=x'00' WHERE rowid=(SELECT min(rowid) FROM {table})"
        ))
        .unwrap();
        invalid(capture_source_rows_in(&tx, &expected, &locator()));
        tx.rollback().unwrap();
    }
}

#[test]
fn finite_real_only_and_all_closed_vocabularies_and_integer_domains_are_checked() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    for (table, column, value) in [
        ("market_instruments", "lot_size", "1"),
        ("market_instruments", "price_step", "'1.0'"),
        ("market_instruments", "min_notional", "1e999"),
        ("market_instruments", "lot_size", "-1e999"),
        ("discovery_jobs", "candidate_index", "-1"),
        ("research_attempts", "candidate_index", "-1"),
        ("research_artifacts", "byte_len", "-1"),
        ("datasets", "candle_count", "-1"),
        ("market_provenance", "raw_byte_len", "-1"),
        ("market_provenance", "accepted", "2"),
        ("market_instruments", "revision", "0"),
        ("research_attempts", "id", "0"),
        ("discovery_jobs", "status", "'unknown'"),
        ("research_attempts", "status", "'done'"),
        ("hypotheses", "source", "'unknown'"),
        ("market_calendars", "kind", "'unknown'"),
        ("market_instruments", "market", "'unknown'"),
        ("market_instruments", "asset_type", "'unknown'"),
        ("market_snapshots", "price_basis", "'unknown'"),
        ("market_snapshots", "kind", "'unknown'"),
        ("market_snapshots", "status", "'unknown'"),
        ("market_provenance", "role", "'unknown'"),
        (
            "strategy_def",
            "strategy_hash",
            "'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
        ),
        (
            "datasets",
            "dataset_hash",
            "'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
        ),
        (
            "research_artifacts",
            "sha256",
            "'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'",
        ),
        ("market_calendars", "version", "''"),
        (
            "hypotheses",
            "strategy_hash",
            "'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
        ),
        (
            "hypotheses",
            "strategy_hash",
            "'strategy-doc-v1:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'",
        ),
        (
            "hypotheses",
            "strategy_hash",
            "'strategy-doc-v2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
        ),
    ] {
        let tx = f.connection().transaction().unwrap();
        tx.execute_batch(&format!(
            "UPDATE {table} SET {column}={value} WHERE rowid=(SELECT min(rowid) FROM {table})"
        ))
        .unwrap();
        invalid(capture_source_rows_in(&tx, &expected, &locator()));
        tx.rollback().unwrap();
    }
}

const PAD_SLOTS: [(&str, &str, i64, usize, usize); 9] = [
    ("strategy_def", "original_definition_json", 100, 43, 3),
    ("strategy_def", "original_definition_json", 101, 43, 3),
    ("strategy_def", "original_definition_json", 102, 43, 3),
    ("strategy_def", "dsl_json", 100, 0, 0),
    ("strategy_def", "dsl_json", 101, 0, 0),
    ("strategy_def", "dsl_json", 102, 0, 0),
    ("hypotheses", "content_json", 200, 43, 3),
    ("market_snapshots", "content_json", 13, 43, 3),
    ("market_instruments", "content_json", 12, 43, 3),
];

#[test]
fn actual_rows_enforce_exact_whole_text_budget_and_one_over_without_table_resets() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    let mut remaining = 33_554_432 - 6102;
    for (table, column, id, old_bytes, _) in PAD_SLOTS {
        let added = remaining.min(4_194_304 - old_bytes);
        let bytes = old_bytes + added;
        let raw = format!("\"{}\"", "x".repeat(bytes - 2));
        tx.execute(
            &format!("UPDATE {table} SET {column}=?1 WHERE id=?2"),
            params![raw, id],
        )
        .unwrap();
        remaining -= added;
        if remaining == 0 {
            break;
        }
    }
    assert_eq!(remaining, 0);
    let exact = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
    assert_eq!(exact.usage.text_bytes, 33_554_432);
    tx.execute_batch("UPDATE market_provenance SET retrieved_at=retrieved_at||'x' WHERE id=15")
        .unwrap();
    invalid(capture_source_rows_in(&tx, &expected, &locator()));
    tx.rollback().unwrap();
}

#[test]
fn actual_rows_enforce_exact_whole_node_budget_before_copy_and_one_over() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    let mut remaining = 262_144 - 184;
    for (table, column, id, _, old_nodes) in PAD_SLOTS {
        let added = remaining.min(65_536 - old_nodes);
        let nodes = old_nodes + added;
        let raw = format!("[{}]", vec!["null"; nodes - 1].join(","));
        tx.execute(
            &format!("UPDATE {table} SET {column}=?1 WHERE id=?2"),
            params![raw, id],
        )
        .unwrap();
        remaining -= added;
        if remaining == 0 {
            break;
        }
    }
    assert_eq!(remaining, 0);
    assert_eq!(
        capture_source_rows_in(&tx, &expected, &locator())
            .unwrap()
            .usage
            .json_nodes,
        262_144
    );
    // The final retained JSON column adds exactly one node, after a nonempty
    // prior budget. Its parser must refuse before owning these original bytes.
    tx.execute_batch(
        "UPDATE market_provenance SET request_scope_json='[null,null,null]' WHERE id=15",
    )
    .unwrap();
    let copies = super::super::super::original_json::owned_copy_count();
    invalid(capture_source_rows_in(&tx, &expected, &locator()));
    assert_eq!(
        super::super::super::original_json::owned_copy_count() - copies,
        25 // 26 retained documents after padding, final refusal before its copy
    );
    tx.rollback().unwrap();
}

pub(super) fn large_family(f: &mut Fixture) {
    use alpha_factor_forge::discovery_core::{
        config::parse_discovery_config, enumerate::enumerate_candidates,
    };
    let expected = oracle();
    let mut config: Value =
        serde_json::from_str(expected["tables"][2]["rows"][0][2].as_str().unwrap()).unwrap();
    config["maxConcurrency"] = json!(1);
    config["caps"]["candidates"] = json!(4096);
    config["bases"][0]["axes"] = json!([
        {"key":"slPct","min":1,"max":64,"step":1},
        {"key":"tpPct","min":1,"max":64,"step":1}]);
    // Pure enumerator is setup only, never the complete-graph oracle. The
    // independent 133-column fixture remains unchanged. Count is 64*64.
    let plan = enumerate_candidates(&parse_discovery_config(&config, 1.0).unwrap()).unwrap();
    assert_eq!(plan.candidates.len(), 4096);
    let tx = f.connection().transaction().unwrap();
    tx.execute(
        "UPDATE discovery_runs SET config_json=?1",
        [serde_json::to_string(&config).unwrap()],
    )
    .unwrap();
    tx.execute_batch("DELETE FROM discovery_jobs; DELETE FROM research_attempts; DELETE FROM strategy_def; DELETE FROM hypotheses;").unwrap();
    for c in plan.candidates {
        let i = c.index;
        tx.execute("INSERT INTO strategy_def(id,type,original_definition_json,strategy_hash) VALUES(?1,'params','null',?2)",params![100+i,c.strategy_hash]).unwrap();
        tx.execute("INSERT INTO hypotheses(id,hypothesis_hash,version,source,mechanism,applicability_json,failure_modes,strategy_hash,strategy_id,content_json) VALUES(?1,?2,'v1','discovery','','null','',?3,?4,'null')",params![200+i,format!("{:064x}",i+10),c.strategy_hash,100+i]).unwrap();
        for (s, segment) in [(0, "train"), (1, "validation")] {
            tx.execute("INSERT INTO discovery_jobs(id,discovery_run_id,candidate_index,strategy_id,dataset_id,segment,status) VALUES(?1,1,?2,?3,7,?4,'done')",params![500+2*i+s,i,100+i,segment]).unwrap();
        }
        tx.execute("INSERT INTO research_attempts(id,attempt_key,hypothesis_id,strategy_id,dataset_id,discovery_run_id,candidate_index,status,input_fingerprint_json,engine_fingerprint_json,result_artifact_id,trial_event_id) VALUES(?1,?2,?3,?4,7,1,?5,'completed','null','null',300,?6)",params![400+i,format!("run:1:candidate:{i}"),200+i,100+i,i,format!("trial-event-v1:{:064x}",i+1)]).unwrap();
    }
    tx.commit().unwrap();
}

#[test]
fn actual_full_4096_candidate_8192_job_graph_and_cap_plus_one_sentinels() {
    let mut f = fixture();
    damage(&mut f);
    large_family(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    let rows = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
    assert_eq!(rows.jobs.len(), 8192);
    assert_eq!(rows.attempts.len(), 4096);
    assert_eq!(rows.strategies.len(), 4096);
    assert_eq!(rows.hypotheses.len(), 4096);
    assert_eq!(rows.artifacts.len(), 1);
    tx.execute_batch("INSERT INTO discovery_jobs(id,discovery_run_id,candidate_index,strategy_id,dataset_id,segment,status) VALUES(99999,1,4096,100,7,'train',x'ff')").unwrap();
    invalid(capture_source_rows_in(&tx, &expected, &locator()));
    // The cap+1 marker is not read: its bad status must not yield a Db error.
    tx.execute_batch("DELETE FROM discovery_jobs WHERE id=99999; INSERT INTO research_attempts(id,attempt_key,hypothesis_id,strategy_id,dataset_id,discovery_run_id,candidate_index,status) VALUES(99999,x'ff',200,100,7,1,4096,'completed')").unwrap();
    invalid(capture_source_rows_in(&tx, &expected, &locator()));
    tx.rollback().unwrap();
}

#[test]
fn actual_source_cap_and_reference_sets_are_inclusive_and_never_truncate() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    let tx = f.connection().transaction().unwrap();
    for id in 16..1038 {
        tx.execute("INSERT INTO market_provenance SELECT ?1,record_hash,version,instrument_id,interval,source,role,request_scope_json,raw_sha256,raw_byte_len,raw_artifact_path,raw_media_type,retrieved_at,available_at,accepted,rejection_reason,revision_of,created_at FROM market_provenance WHERE id=14",[id]).unwrap();
        tx.execute("INSERT INTO market_snapshot_sources VALUES(13,?1)", [id])
            .unwrap();
    }
    let rows = capture_source_rows_in(&tx, &expected, &locator()).unwrap();
    assert_eq!(rows.sources.len(), 1024);
    assert_eq!(rows.provenance.len(), 1024);
    tx.execute_batch("INSERT INTO market_snapshot_sources VALUES(13,1038)")
        .unwrap();
    invalid(capture_source_rows_in(&tx, &expected, &locator()));
    tx.rollback().unwrap();
    let mut ids = BTreeSet::new();
    for id in 1..=4096 {
        reference(&mut ids, id, 4096).unwrap();
    }
    reference(&mut ids, 4096, 4096).unwrap();
    invalid(reference(&mut ids, 4097, 4096));
    assert_eq!(ids.len(), 4096);
}

#[test]
fn graph_field_limits_exact_one_over_and_json_depth_key_nodes_are_not_coerced() {
    let mut f = fixture();
    damage(&mut f);
    let expected = f.expected.clone();
    let mut cases: Vec<(String, bool)> = vec![];
    cases.push((format!("\"{}\"", "x".repeat(4_194_302)), true));
    cases.push((format!("\"{}\"", "x".repeat(4_194_303)), false));
    cases.push((format!("[{}]", vec!["null"; 65535].join(",")), true));
    cases.push((format!("[{}]", vec!["null"; 65536].join(",")), false));
    cases.push((format!("{}null{}", "[".repeat(63), "]".repeat(63)), true));
    cases.push((format!("{}null{}", "[".repeat(64), "]".repeat(64)), false));
    cases.push((format!("{{\"{}\":null}}", "x".repeat(1024)), true));
    cases.push((format!("{{\"{}\":null}}", "x".repeat(1025)), false));
    cases.push(("{\"x\":0,\"x\":1}".into(), false));
    for (raw, valid) in cases {
        let tx = f.connection().transaction().unwrap();
        tx.execute("UPDATE strategy_def SET dsl_json=?1 WHERE id=100", [&raw])
            .unwrap();
        let result = capture_source_rows_in(&tx, &expected, &locator());
        if valid {
            assert_eq!(
                result.unwrap().strategies[0]
                    .dsl_json
                    .as_ref()
                    .unwrap()
                    .as_bytes(),
                raw.as_bytes()
            );
        } else {
            invalid(result);
        }
        tx.rollback().unwrap();
    }
    for (bytes, valid) in [(4096, true), (4097, false)] {
        let tx = f.connection().transaction().unwrap();
        tx.execute(
            "UPDATE market_snapshots SET cost_profile_version=?1",
            ["x".repeat(bytes)],
        )
        .unwrap();
        let result = capture_source_rows_in(&tx, &expected, &locator());
        if valid {
            assert_eq!(
                result.unwrap().snapshot.cost_profile_version.unwrap().len(),
                bytes
            );
        } else {
            invalid(result);
        }
        tx.rollback().unwrap();
    }
}

#[test]
fn matching_copied_database_proves_consistency_only_and_cap_sentinel_has_no_payload_charge() {
    let mut f = fixture();
    let saved = capture(&mut f);
    let expected = f.expected.clone();
    let copy = f.root.join("graph-consistency-copy.sqlite");
    f.connection()
        .execute("VACUUM INTO ?1", [copy.to_str().unwrap()])
        .unwrap();
    let mut copied = Connection::open(&copy).unwrap();
    let tx = copied.transaction().unwrap();
    let equal = unchanged(&tx, || capture_source_rows_in(&tx, &expected, &locator())).unwrap();
    assert!(equal == saved); // no API here returns or tests a physical-owner guard
    unchanged(&tx, || recheck_source_rows_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
    drop(copied);
    damage(&mut f);
    large_family(&mut f);
    let tx = f.connection().transaction().unwrap();
    tx.execute_batch("INSERT INTO discovery_jobs(id,discovery_run_id,candidate_index,strategy_id,dataset_id,segment,status) VALUES(99999,1,4096,100,7,'train',x'ff')").unwrap();
    let mut budget = ResourceBudget::new().unwrap();
    invalid(read_jobs(&tx, 1, &mut budget));
    assert_eq!(budget.used.text_bytes, 128 + 4096 * (5 + 4 + 10 + 4));
    assert_eq!(budget.used.json_nodes, 0);
    tx.rollback().unwrap();
}
