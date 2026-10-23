use super::super::tests::{snapshot, unchanged, writer_busy, Fixture};
use super::*;
use crate::db::ownership;
use crate::db::ownership::HolderKind;
use rusqlite::TransactionBehavior;

const CAMPAIGN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_CAMPAIGN: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const DOCUMENT: &str = " \n{\"n\":0.0036944444444444438,\"huge\":1e999}\t";
const REPORT: &str = "[null,true,{\"cost\":-0}]";
const CONFIG: &str = " {\"opaque\":[1.00,2.2250738585072014e-308]} ";

fn locator() -> SourceLocator {
    SourceLocator::new(CAMPAIGN, 1).unwrap()
}

fn fixture() -> Fixture {
    let mut fixture = Fixture::new();
    let conn = fixture.connection();
    conn.execute("INSERT INTO discovery_runs(id,name,status,config_json) VALUES(1,'root test','completed',?1)", [CONFIG]).unwrap();
    conn.execute("INSERT INTO research_campaigns(campaign_id,version,document_json) VALUES(?1,'unknown-original-v1',?2)", params![CAMPAIGN, DOCUMENT]).unwrap();
    conn.execute("INSERT INTO campaign_run_admissions(id,discovery_run_id,campaign_id,instrument_id,batch_id,status,report_version,report_json,fee_pct,slip_pct,epoch) VALUES(1,1,?1,'crypto:venue:BTCUSDT',?2,'ELIGIBLE','unknown-report-v1',?3,?4,?5,99)", params![CAMPAIGN,format!("trial-batch-v1:{OTHER_CAMPAIGN}"),REPORT,0.0036944444444444438_f64,-0.25_f64]).unwrap();
    fixture
}

fn capture(fixture: &mut Fixture) -> RootRowsObservation {
    let expected = fixture.expected.clone();
    let tx = fixture.connection().transaction().unwrap();
    let saved = unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())).unwrap();
    tx.rollback().unwrap();
    saved
}

fn refused<T>(result: AppResult<T>, code: &str) {
    match result {
        Err(AppError::Other(text)) => assert_eq!(text, code),
        Err(other) => panic!("wrong refusal: {other:?}"),
        Ok(_) => panic!("expected {code}"),
    }
}

fn editable(tx: &Transaction<'_>) {
    tx.execute_batch("DROP TRIGGER research_campaigns_no_update; DROP TRIGGER research_campaigns_no_delete; DROP TRIGGER campaign_run_admissions_no_update; DROP TRIGGER campaign_run_admissions_no_delete;").unwrap();
}

// Test-owned damaged schema only: remove types/keys/checks to expose raw cells
// and duplicate roots that the ordinary migrations prohibit.
fn unconstrained(tx: &Transaction<'_>, table: &str) {
    let columns: Vec<String> = tx
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .map(|v| format!("\"{}\"", v.unwrap()))
        .collect();
    tx.execute_batch(&format!("CREATE TABLE damaged_root ({}); INSERT INTO damaged_root SELECT * FROM {table}; DROP TABLE {table}; ALTER TABLE damaged_root RENAME TO {table};", columns.join(","))).unwrap();
}

#[test]
fn exact_capture_preserves_every_root_cell_bits_bytes_and_joint_usage() {
    let mut fixture = fixture();
    let saved = capture(&mut fixture);
    assert_eq!(saved.workspace.workspace_id, fixture.expected.workspace_id);
    assert_eq!(saved.locator.campaign_id, CAMPAIGN);
    assert_eq!(saved.locator.run_id, 1);
    assert_eq!(saved.campaign.campaign_id, CAMPAIGN);
    assert_eq!(saved.campaign.version, "unknown-original-v1");
    assert_eq!(saved.campaign.document_json.as_bytes(), DOCUMENT.as_bytes());
    assert_eq!(saved.admission.id, 1);
    assert_eq!(saved.admission.discovery_run_id, 1);
    assert_eq!(saved.admission.campaign_id, CAMPAIGN);
    assert_eq!(saved.admission.instrument_id, "crypto:venue:BTCUSDT");
    assert_eq!(
        saved.admission.batch_id,
        format!("trial-batch-v1:{OTHER_CAMPAIGN}")
    );
    assert_eq!(saved.admission.status, "ELIGIBLE");
    assert_eq!(saved.admission.report_version, "unknown-report-v1");
    assert_eq!(saved.admission.report_json.as_bytes(), REPORT.as_bytes());
    assert_eq!(
        saved.admission.fee_pct_bits,
        0.0036944444444444438_f64.to_bits()
    );
    assert_eq!(saved.admission.slip_pct_bits, (-0.25_f64).to_bits());
    assert_eq!(saved.run.id, 1);
    assert_eq!(saved.run.status, "completed");
    assert_eq!(saved.run.config_json.as_bytes(), CONFIG.as_bytes());
    let row_bytes = [
        CAMPAIGN,
        "unknown-original-v1",
        DOCUMENT,
        CAMPAIGN,
        "crypto:venue:BTCUSDT",
        &format!("trial-batch-v1:{OTHER_CAMPAIGN}"),
        "ELIGIBLE",
        "unknown-report-v1",
        REPORT,
        "completed",
        CONFIG,
    ]
    .iter()
    .map(|v| v.len())
    .sum::<usize>();
    assert_eq!(saved.usage.text_bytes, 128 + row_bytes);
    assert_eq!(saved.usage.json_nodes, 3 + 5 + 4);
    let expected = fixture.expected.clone();
    let tx = fixture.connection().transaction().unwrap();
    unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)).unwrap();
    assert_eq!(saved.workspace.mutation_seq, 0);
    assert_eq!(ownership::read(&tx).unwrap().unwrap().epoch, expected.epoch);
    tx.rollback().unwrap();
}

#[test]
fn diagnostic_only_fields_and_historical_epoch_do_not_stale_roots() {
    let mut fixture = fixture();
    let saved = capture(&mut fixture);
    let expected = fixture.expected.clone();
    let tx = fixture.connection().transaction().unwrap();
    editable(&tx);
    tx.execute_batch("UPDATE research_campaigns SET created_at='old'; UPDATE campaign_run_admissions SET epoch=-77,created_at='display'; UPDATE discovery_runs SET name='renamed',progress_json='not-json',error_message='diagnostic',started_at='old',updated_at='new',completed_at='later'; UPDATE workspace_ownership SET heartbeat_seq=heartbeat_seq+1,pid=pid+1,heartbeat_at='display';").unwrap();
    assert!(unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())).unwrap() == saved);
    unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
}

#[test]
fn pending_roots_are_visible_only_inside_and_full_rollback_restores_schema_rows() {
    let mut fixture = fixture();
    let saved = capture(&mut fixture);
    let expected = fixture.expected.clone();
    let mut observer = fixture.independent();
    let baseline = snapshot(fixture.connection());
    let tx = fixture
        .connection()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    editable(&tx);
    tx.execute_batch("UPDATE research_campaigns SET version='pending'; UPDATE campaign_run_admissions SET report_json='null'; UPDATE discovery_runs SET config_json='[1e999]';").unwrap();
    let pending = unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())).unwrap();
    assert!(pending != saved);
    let other = observer.transaction().unwrap();
    assert!(capture_root_rows_in(&other, &expected, &locator()).unwrap() == saved);
    assert_eq!(snapshot(&other), baseline);
    other.rollback().unwrap();
    refused(
        unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)),
        "source_rows_changed",
    );
    tx.rollback().unwrap();
    assert_eq!(snapshot(fixture.connection()), baseline);
    assert!(capture(&mut fixture) == saved);
}

#[test]
fn success_refusal_and_sql_error_never_release_callers_writer_reservation() {
    let mut fixture = fixture();
    let saved = capture(&mut fixture);
    let expected = fixture.expected.clone();
    let writer = fixture.independent();
    let tx = fixture
        .connection()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    writer_busy(&writer);
    unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)).unwrap();
    writer_busy(&writer);
    tx.execute("UPDATE discovery_runs SET config_json='null'", [])
        .unwrap();
    refused(
        unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)),
        "source_rows_changed",
    );
    writer_busy(&writer);
    tx.execute_batch("ALTER TABLE discovery_runs RENAME TO pending_missing_runs")
        .unwrap();
    assert!(matches!(
        unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())),
        Err(AppError::Db(_))
    ));
    writer_busy(&writer);
    tx.rollback().unwrap();
    writer.execute_batch("BEGIN IMMEDIATE; ROLLBACK").unwrap();
    assert!(capture(&mut fixture) == saved);
}

#[test]
fn deferred_equality_is_historical_and_fresh_root_change_refuses() {
    let mut fixture = fixture();
    let saved = capture(&mut fixture);
    let expected = fixture.expected.clone();
    let writer = fixture.independent();
    let tx = fixture.connection().transaction().unwrap();
    assert!(capture_root_rows_in(&tx, &expected, &locator()).unwrap() == saved);
    writer
        .execute("UPDATE discovery_runs SET config_json='null'", [])
        .unwrap();
    unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
    let tx = fixture.connection().transaction().unwrap();
    refused(
        unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)),
        "source_rows_changed",
    );
    tx.rollback().unwrap();
}

#[test]
fn every_retained_field_change_is_detected_without_adopting_saved_locator() {
    let mut fixture = fixture();
    let saved = capture(&mut fixture);
    let expected = fixture.expected.clone();
    for sql in [
        "UPDATE research_campaigns SET version='other'",
        "UPDATE research_campaigns SET document_json='null'",
        "UPDATE campaign_run_admissions SET id=2",
        "UPDATE campaign_run_admissions SET instrument_id='other:instrument'",
        "UPDATE campaign_run_admissions SET batch_id='trial-batch-v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
        "UPDATE campaign_run_admissions SET status='NOT_ELIGIBLE'",
        "UPDATE campaign_run_admissions SET report_version='other'",
        "UPDATE campaign_run_admissions SET report_json='null'",
        "UPDATE campaign_run_admissions SET fee_pct=0.0036944444444444442",
        "UPDATE campaign_run_admissions SET slip_pct=0.25",
        "UPDATE discovery_runs SET status='paused'",
        "UPDATE discovery_runs SET config_json='null'",
    ] {
        let tx = fixture.connection().transaction().unwrap();
        editable(&tx);
        tx.execute_batch(sql).unwrap();
        refused(unchanged(&tx, || recheck_root_rows_in(&tx,&expected,&saved)),"source_rows_changed");
        tx.rollback().unwrap();
        assert!(capture(&mut fixture) == saved);
    }
}

#[test]
fn missing_each_root_and_substituted_campaign_or_run_never_fall_back() {
    let mut fixture = fixture();
    let expected = fixture.expected.clone();
    fixture
        .connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    for sql in [
        "DELETE FROM research_campaigns",
        "DELETE FROM campaign_run_admissions",
        "DELETE FROM discovery_runs",
        "UPDATE discovery_runs SET id=2",
        "UPDATE campaign_run_admissions SET discovery_run_id=2",
        "UPDATE campaign_run_admissions SET campaign_id='bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'",
    ] {
        let tx = fixture.connection().transaction().unwrap();
        editable(&tx);
        tx.execute_batch(sql).unwrap();
        refused(unchanged(&tx, || capture_root_rows_in(&tx,&expected,&locator())),"source_rows_invalid");
        tx.rollback().unwrap();
    }
}

#[test]
fn duplicate_each_root_refuses_without_inspecting_or_retaining_sentinel_payload() {
    let mut fixture = fixture();
    let expected = fixture.expected.clone();
    fixture
        .connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    for table in [
        "research_campaigns",
        "campaign_run_admissions",
        "discovery_runs",
    ] {
        let tx = fixture.connection().transaction().unwrap();
        unconstrained(&tx, table);
        tx.execute_batch(&format!("INSERT INTO {table} SELECT * FROM {table}"))
            .unwrap();
        refused(
            unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())),
            "source_rows_invalid",
        );
        tx.rollback().unwrap();
    }
    let tx = fixture.connection().transaction().unwrap();
    unconstrained(&tx, "campaign_run_admissions");
    tx.execute_batch("INSERT INTO campaign_run_admissions SELECT * FROM campaign_run_admissions; UPDATE campaign_run_admissions SET report_json=zeroblob(4194305) WHERE rowid=2;").unwrap();
    refused(
        unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())),
        "source_rows_invalid",
    );
    tx.rollback().unwrap();
}

#[test]
fn raw_cell_types_null_utf8_domains_and_nonfinite_reals_refuse_statically() {
    let mut fixture = fixture();
    let expected = fixture.expected.clone();
    fixture
        .connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    for (table, assignments) in [
        (
            "research_campaigns",
            vec![
                "version=NULL",
                "version=x'ff'",
                "version=CAST(x'ff' AS TEXT)",
                "version=1",
                "version=''",
                "document_json=x'6e756c6c'",
                "document_json=NULL",
                "document_json=CAST(x'ff' AS TEXT)",
                "document_json='{\"a\":1,\"a\":2}'",
            ],
        ),
        (
            "campaign_run_admissions",
            vec![
                "id=0",
                "id=-1",
                "id=1.0",
                "id='1'",
                "id=NULL",
                "discovery_run_id=1.0",
                "campaign_id=upper(campaign_id)",
                "campaign_id=x'aa'",
                "instrument_id=''",
                "instrument_id='a b'",
                "instrument_id=char(127)",
                "instrument_id=CAST(x'ff' AS TEXT)",
                "batch_id='trial-batch-v1:bad'",
                "batch_id=upper(batch_id)",
                "status='eligible'",
                "report_version=''",
                "report_json=1",
                "report_json=NULL",
                "fee_pct=1",
                "fee_pct='1.0'",
                "fee_pct=NULL",
                "fee_pct=1e999",
                "slip_pct=-1e999",
                "slip_pct=x'00'",
            ],
        ),
        (
            "discovery_runs",
            vec![
                "id=1.0",
                "status='unknown'",
                "status=NULL",
                "status=CAST(x'ff' AS TEXT)",
                "config_json=NULL",
                "config_json='[1,]'",
                "config_json=CAST(x'ff' AS TEXT)",
            ],
        ),
    ] {
        for assignment in assignments {
            let tx = fixture.connection().transaction().unwrap();
            unconstrained(&tx, table);
            tx.execute_batch(&format!("UPDATE {table} SET {assignment}"))
                .unwrap();
            refused(
                unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())),
                "source_rows_invalid",
            );
            tx.rollback().unwrap();
        }
    }
}

#[test]
fn campaign_selector_collation_never_replaces_exact_domain_check() {
    let mut fixture = fixture();
    let expected = fixture.expected.clone();
    fixture
        .connection()
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    let tx = fixture.connection().transaction().unwrap();
    tx.execute_batch("CREATE TABLE damaged_root(campaign_id COLLATE NOCASE,version,document_json,created_at); INSERT INTO damaged_root SELECT upper(campaign_id),version,document_json,created_at FROM research_campaigns; DROP TABLE research_campaigns; ALTER TABLE damaged_root RENAME TO research_campaigns;").unwrap();
    refused(
        unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())),
        "source_rows_invalid",
    );
    tx.rollback().unwrap();
}

#[test]
fn locator_validates_domains_only_including_full_positive_i64_range() {
    assert!(SourceLocator::new(CAMPAIGN, i64::MAX).is_ok());
    for id in [0, -1, i64::MIN] {
        refused(SourceLocator::new(CAMPAIGN, id), "source_rows_invalid");
    }
    for value in [
        "".to_owned(),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
        format!("{} ", "a".repeat(63)),
    ] {
        refused(SourceLocator::new(&value, 1), "source_rows_invalid");
    }
}

#[test]
fn text_byte_and_instrument_boundaries_use_raw_utf8_without_trimming() {
    let mut fixture = fixture();
    let expected = fixture.expected.clone();
    for (column, exact, over) in [
        (
            "version",
            "é".repeat(2048),
            format!("{}a", "é".repeat(2048)),
        ),
        ("report_version", "a".repeat(4096), "a".repeat(4097)),
        ("instrument_id", "!".repeat(256), "!".repeat(257)),
    ] {
        let table = if column == "version" {
            "research_campaigns"
        } else {
            "campaign_run_admissions"
        };
        let tx = fixture.connection().transaction().unwrap();
        editable(&tx);
        tx.execute(&format!("UPDATE {table} SET {column}=?1"), [exact])
            .unwrap();
        unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())).unwrap();
        tx.execute(&format!("UPDATE {table} SET {column}=?1"), [over])
            .unwrap();
        refused(
            unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())),
            "source_rows_invalid",
        );
        tx.rollback().unwrap();
    }
}

#[test]
fn every_json_column_enforces_exact_byte_node_depth_and_key_limits() {
    let mut fixture = fixture();
    let expected = fixture.expected.clone();
    let exact_bytes = format!("null{}", " ".repeat(4_194_300));
    let exact_nodes = format!("[{}]", vec!["null"; 65_535].join(","));
    let exact_depth = format!("{}null{}", "[".repeat(63), "]".repeat(63));
    let exact_key = format!("{{\"{}\":null}}", "a".repeat(1024));
    for (table, column) in [
        ("research_campaigns", "document_json"),
        ("campaign_run_admissions", "report_json"),
        ("discovery_runs", "config_json"),
    ] {
        for (exact, over) in [
            (exact_bytes.clone(), format!("{exact_bytes} ")),
            (
                exact_nodes.clone(),
                format!("[{},null]", &exact_nodes[1..exact_nodes.len() - 1]),
            ),
            (exact_depth.clone(), format!("[{exact_depth}]")),
            (
                exact_key.clone(),
                format!("{{\"{}\":null}}", "a".repeat(1025)),
            ),
        ] {
            let tx = fixture.connection().transaction().unwrap();
            editable(&tx);
            tx.execute(&format!("UPDATE {table} SET {column}=?1"), [exact])
                .unwrap();
            unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())).unwrap();
            tx.execute(&format!("UPDATE {table} SET {column}=?1"), [over])
                .unwrap();
            refused(
                unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())),
                "source_rows_invalid",
            );
            tx.rollback().unwrap();
        }
    }
}

#[test]
fn shared_budget_exact_one_over_nonempty_and_checked_overflow() {
    let mut budget = ResourceBudget::new().unwrap();
    budget
        .charge(MAX_TOTAL_TEXT_BYTES - 128, MAX_TOTAL_JSON_NODES - 1)
        .unwrap();
    let before = budget.used.clone();
    refused(budget.charge(1, 0), "source_rows_invalid");
    refused(budget.charge(0, 2), "source_rows_invalid");
    refused(budget.charge(usize::MAX, 0), "source_rows_invalid");
    refused(budget.charge(0, usize::MAX), "source_rows_invalid");
    assert!(budget.used == before);
    budget.charge(0, 1).unwrap();
    assert_eq!(budget.used.text_bytes, 33_554_432);
    assert_eq!(budget.used.json_nodes, 262_144);
    refused(budget.charge(0, 1), "source_rows_invalid");
}

#[test]
fn shared_json_budget_checks_remaining_nodes_and_bytes_before_retention() {
    let mut budget = ResourceBudget::new().unwrap();
    budget.charge(7, MAX_TOTAL_JSON_NODES - 2).unwrap();
    let before = budget.used.clone();
    let copies = super::super::original_json::owned_copy_count();
    refused(budget.json(b"[null,true]"), "source_rows_invalid");
    assert_eq!(
        super::super::original_json::owned_copy_count(),
        copies,
        "joint node refusal must precede original byte ownership"
    );
    assert!(budget.used == before);
    let json = budget.json(b"[null]").unwrap();
    assert_eq!(json.as_bytes(), b"[null]");
    assert_eq!(budget.used.json_nodes, 262_144);
    let before = budget.used.clone();
    refused(budget.json(b"null"), "source_rows_invalid");
    assert!(budget.used == before);
    let mut budget = ResourceBudget::new().unwrap();
    budget.charge(MAX_TOTAL_TEXT_BYTES - 128 - 4, 17).unwrap();
    let before = budget.used.clone();
    refused(budget.json(b"null "), "source_rows_invalid");
    assert!(budget.used == before);
    budget.json(b"null").unwrap();
    assert_eq!(budget.used.text_bytes, 33_554_432);
    assert_eq!(budget.used.json_nodes, 18);
}

#[test]
fn workspace_changes_keep_priority_over_root_corruption_and_old_owner_is_not_adopted() {
    let mut fixture = fixture();
    fixture
        .connection()
        .execute("UPDATE runtime_state SET mutation_seq=1", [])
        .unwrap();
    let saved = capture(&mut fixture);
    let expected = fixture.expected.clone();
    for sql in [
        "UPDATE runtime_state SET mutation_seq=0",
        "UPDATE runtime_state SET mutation_seq=2",
        "UPDATE app_settings SET value_json='\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"' WHERE key='workspace_id'",
    ] {
        let tx = fixture.connection().transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        tx.execute("UPDATE discovery_runs SET config_json='broken'",[]).unwrap();
        refused(unchanged(&tx, || recheck_root_rows_in(&tx,&expected,&saved)),"workspace_observation_changed");
        tx.rollback().unwrap();
    }
    for sql in [
        "UPDATE workspace_ownership SET epoch=epoch+1",
        "UPDATE workspace_ownership SET holder_kind='service'",
        "UPDATE workspace_ownership SET holder_instance_id='bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'",
    ] {
        let tx = fixture.connection().transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        assert!(matches!(
            unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)),
            Err(AppError::StaleOwner(_))
        ));
        let actual = ownership::read(&tx).unwrap().unwrap();
        let current = ExpectedWorkspace::new(
            &expected.workspace_id,
            actual.epoch,
            if actual.holder_kind == "service" {
                HolderKind::Service
            } else {
                HolderKind::DesktopEmbedded
            },
            &actual.holder_instance_id,
        )
        .unwrap();
        assert!(matches!(
            unchanged(&tx, || recheck_root_rows_in(&tx, &current, &saved)),
            Err(AppError::StaleOwner(_))
        ));
        tx.rollback().unwrap();
    }
}

#[test]
fn inactive_transaction_and_missing_sql_keep_workspace_first_error_order() {
    let mut fixture = fixture();
    let saved = capture(&mut fixture);
    let expected = fixture.expected.clone();
    let tx = fixture.connection().transaction().unwrap();
    tx.execute_batch("ROLLBACK").unwrap();
    refused(
        unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())),
        "workspace_observation_invalid",
    );
    refused(
        unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)),
        "workspace_observation_invalid",
    );
}

#[test]
fn identity_matching_copy_remains_only_consistency_evidence() {
    let mut fixture = fixture();
    let saved = capture(&mut fixture);
    let expected = fixture.expected.clone();
    let mut copy = self::fixture();
    assert_ne!(fixture.root, copy.root);
    let tx = copy.connection().transaction().unwrap();
    tx.execute(
        "UPDATE app_settings SET value_json=?1 WHERE key='workspace_id'",
        [serde_json::to_string(&expected.workspace_id).unwrap()],
    )
    .unwrap();
    tx.execute(
        "UPDATE workspace_ownership SET epoch=?1,holder_kind=?2,holder_instance_id=?3",
        params![
            expected.epoch,
            expected.holder_kind.as_str(),
            expected.holder_instance_id
        ],
    )
    .unwrap();
    assert!(unchanged(&tx, || capture_root_rows_in(&tx, &expected, &locator())).unwrap() == saved);
    unchanged(&tx, || recheck_root_rows_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
}

#[test]
fn sql_projection_marker_order_and_guarded_values_are_exact() {
    let mut fixture = fixture();
    let tx = fixture.connection().transaction().unwrap();
    for (sql, selector, names) in [
        (
            CAMPAIGN_SQL,
            rusqlite::types::Value::Text(CAMPAIGN.into()),
            vec!["campaign_id", "version", "document_json"],
        ),
        (
            ADMISSION_SQL,
            rusqlite::types::Value::Integer(1),
            vec![
                "id",
                "discovery_run_id",
                "campaign_id",
                "instrument_id",
                "batch_id",
                "status",
                "report_version",
                "report_json",
                "fee_pct",
                "slip_pct",
            ],
        ),
        (
            RUN_SQL,
            rusqlite::types::Value::Integer(1),
            vec!["id", "status", "config_json"],
        ),
    ] {
        let mut statement = tx.prepare(sql).unwrap();
        assert_eq!(statement.column_count(), 2 * names.len());
        let mut rows = statement.query(params![selector, 4096, 4_194_304]).unwrap();
        let row = rows.next().unwrap().unwrap();
        for (i, name) in names.iter().enumerate() {
            assert!(
                matches!(row.get_ref(i * 2).unwrap(), ValueRef::Integer(1)),
                "marker for {name}"
            );
        }
        assert!(rows.next().unwrap().is_none());
    }
    editable(&tx);
    tx.execute(
        "UPDATE research_campaigns SET version=?1,document_json=?2",
        params!["x".repeat(4097), " ".repeat(4_194_305)],
    )
    .unwrap();
    let mut statement = tx.prepare(CAMPAIGN_SQL).unwrap();
    let mut rows = statement.query(params![CAMPAIGN, 4096, 4_194_304]).unwrap();
    let row = rows.next().unwrap().unwrap();
    for (marker, value) in [(2, 3), (4, 5)] {
        assert!(matches!(row.get_ref(marker).unwrap(), ValueRef::Integer(0)));
        assert!(matches!(row.get_ref(value).unwrap(), ValueRef::Null));
    }
}
