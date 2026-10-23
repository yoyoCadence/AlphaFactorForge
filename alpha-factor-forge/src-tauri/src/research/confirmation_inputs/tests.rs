use super::*;
use crate::db::{self, ownership, runtime_ledger};
use crate::runtime::lease::{self, OsLock};
use rusqlite::{params, types::ValueRef, Connection, TransactionBehavior};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) struct Fixture {
    pub(super) root: PathBuf,
    conn: Option<Connection>,
    lease: Option<OsLock>,
    pub(super) expected: ExpectedWorkspace,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "aff-workspace-observation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let lease = lease::try_lock_workspace(&root).unwrap();
        let mut conn = db::open_at(&root.join(db::DB_FILE_NAME)).unwrap();
        let acquired =
            ownership::acquire(&mut conn, HolderKind::DesktopEmbedded, std::process::id()).unwrap();
        let expected = ExpectedWorkspace::new(
            &runtime_ledger::workspace_id(&conn).unwrap(),
            acquired.epoch,
            HolderKind::DesktopEmbedded,
            &acquired.instance_id,
        )
        .unwrap();
        Self {
            root,
            conn: Some(conn),
            lease: Some(lease),
            expected,
        }
    }

    pub(super) fn connection(&mut self) -> &mut Connection {
        self.conn.as_mut().unwrap()
    }

    pub(super) fn independent(&self) -> Connection {
        let conn = Connection::open(self.root.join(db::DB_FILE_NAME)).unwrap();
        conn.busy_timeout(Duration::ZERO).unwrap();
        conn
    }

    fn capture(&mut self) -> WorkspaceObservation {
        let expected = self.expected.clone();
        let tx = self.connection().transaction().unwrap();
        let observed = unchanged(&tx, || capture_workspace_in(&tx, &expected)).unwrap();
        tx.rollback().unwrap();
        observed
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Windows must release both DB and lock-file handles before removal.
        drop(self.conn.take());
        drop(self.lease.take());
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, PartialEq)]
enum Cell {
    Null,
    Integer(i64),
    Real(u64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

type Rows = Vec<Vec<Cell>>;

fn query_rows(conn: &Connection, sql: &str) -> Rows {
    let mut statement = conn.prepare(sql).unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| {
            (0..columns)
                .map(|index| {
                    Ok(match row.get_ref(index)? {
                        ValueRef::Null => Cell::Null,
                        ValueRef::Integer(value) => Cell::Integer(value),
                        ValueRef::Real(value) => Cell::Real(value.to_bits()),
                        ValueRef::Text(value) => Cell::Text(value.to_vec()),
                        ValueRef::Blob(value) => Cell::Blob(value.to_vec()),
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

#[derive(Debug, PartialEq)]
pub(super) struct Snapshot {
    schema: Rows,
    tables: Vec<(String, Rows)>,
}

pub(super) fn snapshot(conn: &Connection) -> Snapshot {
    let names = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    Snapshot {
        schema: query_rows(
            conn,
            "SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name",
        ),
        tables: names
            .into_iter()
            .map(|name| {
                let values = query_rows(
                    conn,
                    &format!(
                        "SELECT rowid,* FROM \"{}\" ORDER BY rowid",
                        name.replace('"', "\"\"")
                    ),
                );
                (name, values)
            })
            .collect(),
    }
}

fn changes(conn: &Connection) -> u64 {
    conn.query_row("SELECT total_changes()", [], |row| row.get(0))
        .unwrap()
}

pub(super) fn unchanged<T>(
    tx: &Transaction<'_>,
    read: impl FnOnce() -> AppResult<T>,
) -> AppResult<T> {
    let before = snapshot(tx);
    let writes = changes(tx);
    let was_inactive = tx.is_autocommit();
    let result = read();
    assert_eq!(
        tx.is_autocommit(),
        was_inactive,
        "caller owns transaction lifetime"
    );
    assert_eq!(changes(tx), writes, "observation must not write");
    assert_eq!(
        snapshot(tx),
        before,
        "pending rows/schema must be unchanged"
    );
    result
}

fn invalid_error<T: std::fmt::Debug>(result: AppResult<T>) {
    assert!(
        matches!(result, Err(AppError::Other(ref text)) if text == "workspace_observation_invalid"),
        "expected static invalid refusal, got {result:?}"
    );
}

fn changed_error<T: std::fmt::Debug>(result: AppResult<T>) {
    assert!(
        matches!(result, Err(AppError::Other(ref text)) if text == "workspace_observation_changed"),
        "expected changed refusal, got {result:?}"
    );
}

#[test]
fn actual_capture_and_unchanged_recheck_do_not_advance_any_workspace_state() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let saved = fixture.capture();
    assert_eq!(saved.workspace_id, expected.workspace_id);
    assert_eq!(saved.epoch, expected.epoch);
    assert_eq!(saved.holder_kind, HolderKind::DesktopEmbedded);
    assert_eq!(saved.holder_instance_id, expected.holder_instance_id);
    assert_eq!(saved.mutation_seq, 0);
    let tx = fixture
        .connection()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)).unwrap();
    tx.commit().unwrap();
    assert_eq!(fixture.capture(), saved);
}

#[test]
fn pending_domain_mutation_is_visible_only_inside_and_rollback_restores_every_row() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let saved = fixture.capture();
    let mut observer = fixture.independent();
    let baseline = snapshot(fixture.connection());
    let tx = fixture
        .connection()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute(
        "UPDATE runtime_state SET mutation_seq=mutation_seq+1 WHERE id=1",
        [],
    )
    .unwrap();
    let pending = unchanged(&tx, || capture_workspace_in(&tx, &expected)).unwrap();
    assert_eq!(pending.mutation_seq, saved.mutation_seq + 1);
    let other = observer.transaction().unwrap();
    assert_eq!(capture_workspace_in(&other, &expected).unwrap(), saved);
    assert_eq!(snapshot(&other), baseline);
    other.rollback().unwrap();
    changed_error(unchanged(&tx, || {
        recheck_workspace_in(&tx, &expected, &saved)
    }));
    tx.rollback().unwrap();
    assert_eq!(snapshot(fixture.connection()), baseline);
    assert_eq!(fixture.capture(), saved);
}

pub(super) fn writer_busy(conn: &Connection) {
    let error = conn.execute_batch("BEGIN IMMEDIATE").unwrap_err();
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy)
    );
}

#[test]
fn all_outcomes_retain_callers_write_reservation_until_its_own_commit() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let saved = fixture.capture();
    let writer = fixture.independent();
    let tx = fixture
        .connection()
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    writer_busy(&writer);
    unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)).unwrap();
    writer_busy(&writer);
    tx.execute("UPDATE runtime_state SET mutation_seq=1 WHERE id=1", [])
        .unwrap();
    changed_error(unchanged(&tx, || {
        recheck_workspace_in(&tx, &expected, &saved)
    }));
    writer_busy(&writer);
    tx.execute_batch("ALTER TABLE workspace_ownership RENAME TO pending_unavailable_owner")
        .unwrap();
    assert!(matches!(
        unchanged(&tx, || capture_workspace_in(&tx, &expected)),
        Err(AppError::Db(_))
    ));
    writer_busy(&writer);
    tx.execute_batch("ALTER TABLE pending_unavailable_owner RENAME TO workspace_ownership")
        .unwrap();
    tx.commit().unwrap();
    writer.execute_batch("BEGIN IMMEDIATE; ROLLBACK").unwrap();
    assert_eq!(fixture.capture().mutation_seq, 1);
}

#[test]
fn deferred_equality_is_historical_while_fresh_recheck_detects_committed_growth() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let saved = fixture.capture();
    let writer = fixture.independent();
    let tx = fixture.connection().transaction().unwrap();
    assert_eq!(capture_workspace_in(&tx, &expected).unwrap(), saved);
    writer
        .execute("UPDATE runtime_state SET mutation_seq=1 WHERE id=1", [])
        .unwrap();
    unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
    let tx = fixture.connection().transaction().unwrap();
    changed_error(unchanged(&tx, || {
        recheck_workspace_in(&tx, &expected, &saved)
    }));
    tx.rollback().unwrap();
}

#[test]
fn every_positive_owner_tuple_change_refuses_even_with_matching_current_expectation() {
    for sql in [
        "UPDATE workspace_ownership SET epoch=epoch+1 WHERE id=1",
        "UPDATE workspace_ownership SET holder_kind='service' WHERE id=1",
        "UPDATE workspace_ownership SET holder_instance_id=lower(hex(randomblob(16))) WHERE id=1",
    ] {
        let mut fixture = Fixture::new();
        let expected = fixture.expected.clone();
        let saved = fixture.capture();
        let tx = fixture.connection().transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        let result = unchanged(&tx, || capture_workspace_in(&tx, &expected));
        assert!(
            matches!(result, Err(AppError::StaleOwner(_))),
            "changed owner must refuse"
        );
        let actual = ownership::read(&tx).unwrap().unwrap();
        let kind = if actual.holder_kind == "service" {
            HolderKind::Service
        } else {
            HolderKind::DesktopEmbedded
        };
        let current = ExpectedWorkspace::new(
            &expected.workspace_id,
            actual.epoch,
            kind,
            &actual.holder_instance_id,
        )
        .unwrap();
        assert!(
            matches!(
                unchanged(&tx, || recheck_workspace_in(&tx, &current, &saved)),
                Err(AppError::StaleOwner(_))
            ),
            "old saved owner must not be adopted"
        );
        tx.rollback().unwrap();
    }
}

#[test]
fn changed_workspace_and_both_version_directions_refuse_without_replacing_preparation() {
    for sql in [
        "UPDATE app_settings SET value_json=json_quote(lower(hex(randomblob(16)))) WHERE key='workspace_id'",
        "UPDATE runtime_state SET mutation_seq=2 WHERE id=1",
        "UPDATE runtime_state SET mutation_seq=0 WHERE id=1",
    ] {
        let mut fixture = Fixture::new();
        fixture.connection().execute("UPDATE runtime_state SET mutation_seq=1 WHERE id=1", []).unwrap();
        let expected = fixture.expected.clone();
        let saved = fixture.capture();
        let tx = fixture.connection().transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        changed_error(unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)));
        tx.rollback().unwrap();
        assert_eq!(fixture.capture(), saved);
    }
}

#[test]
fn heartbeat_pid_and_display_timestamps_do_not_stale_the_domain_observation() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let saved = fixture.capture();
    let tx = fixture.connection().transaction().unwrap();
    tx.execute_batch("UPDATE workspace_ownership SET heartbeat_seq=heartbeat_seq+1,pid=pid+1,acquired_at='display-only',heartbeat_at='also-display-only' WHERE id=1").unwrap();
    assert_eq!(
        unchanged(&tx, || capture_workspace_in(&tx, &expected)).unwrap(),
        saved
    );
    unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)).unwrap();
    tx.rollback().unwrap();
}

#[test]
fn different_workspace_and_inactive_transaction_refuse_without_any_repair() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let saved = fixture.capture();
    let mut other = Fixture::new();
    let tx = other.connection().transaction().unwrap();
    changed_error(unchanged(&tx, || capture_workspace_in(&tx, &expected)));
    changed_error(unchanged(&tx, || {
        recheck_workspace_in(&tx, &expected, &saved)
    }));
    tx.rollback().unwrap();
    let tx = fixture.connection().transaction().unwrap();
    tx.execute_batch("ROLLBACK").unwrap();
    let result = unchanged(&tx, || capture_workspace_in(&tx, &expected));
    assert!(
        matches!(result, Err(AppError::Other(ref text)) if text == "workspace_observation_invalid"),
        "inactive observation must refuse"
    );
    invalid_error(unchanged(&tx, || {
        recheck_workspace_in(&tx, &expected, &saved)
    }));
}

#[test]
fn metadata_matching_copy_can_compare_but_is_not_physical_ownership_authority() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let saved = fixture.capture();
    let mut copy = Fixture::new();
    assert_ne!(fixture.root, copy.root);
    let tx = copy.connection().transaction().unwrap();
    tx.execute(
        "UPDATE app_settings SET value_json=?1 WHERE key='workspace_id'",
        [serde_json::to_string(&expected.workspace_id).unwrap()],
    )
    .unwrap();
    tx.execute(
        "UPDATE workspace_ownership SET epoch=?1,holder_kind=?2,holder_instance_id=?3 WHERE id=1",
        params![
            expected.epoch,
            expected.holder_kind.as_str(),
            expected.holder_instance_id
        ],
    )
    .unwrap();
    assert_eq!(
        unchanged(&tx, || capture_workspace_in(&tx, &expected)).unwrap(),
        saved
    );
    unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)).unwrap();
    // Separate actual OS locks/paths; matching metadata minted no live guard.
    tx.rollback().unwrap();
}

#[test]
fn constructor_checks_domains_only_and_never_defaults_an_unowned_acquisition() {
    let valid = "a".repeat(32);
    for id in [
        "".to_string(),
        "a".repeat(31),
        "a".repeat(33),
        "g".repeat(32),
        "é".repeat(16),
    ] {
        invalid_error(ExpectedWorkspace::new(&id, 1, HolderKind::Service, &valid));
        invalid_error(ExpectedWorkspace::new(&valid, 1, HolderKind::Service, &id));
    }
    for epoch in [i64::MIN, -1, 0] {
        invalid_error(ExpectedWorkspace::new(
            &valid,
            epoch,
            HolderKind::Service,
            &valid,
        ));
    }
    assert!(ExpectedWorkspace::new(&valid, i64::MAX, HolderKind::Service, &valid).is_ok());
}

#[test]
fn malformed_missing_or_sql_type_substituted_metadata_never_defaults_or_writes() {
    for sql in [
        "DELETE FROM app_settings WHERE key='workspace_id'",
        "UPDATE app_settings SET value_json='{' WHERE key='workspace_id'",
        "UPDATE app_settings SET value_json='{}' WHERE key='workspace_id'",
        "UPDATE app_settings SET value_json='null' WHERE key='workspace_id'",
        "UPDATE app_settings SET value_json='123' WHERE key='workspace_id'",
        "UPDATE app_settings SET value_json='\"bad\"' WHERE key='workspace_id'",
        "UPDATE app_settings SET value_json=CAST(value_json AS BLOB) WHERE key='workspace_id'",
        "UPDATE app_settings SET value_json=CAST(X'FF' AS TEXT) WHERE key='workspace_id'",
        "DELETE FROM workspace_ownership WHERE id=1",
        "UPDATE workspace_ownership SET epoch=0 WHERE id=1",
        "UPDATE workspace_ownership SET epoch=-1 WHERE id=1",
        "UPDATE workspace_ownership SET epoch=1.5 WHERE id=1",
        "UPDATE workspace_ownership SET epoch=X'31' WHERE id=1",
        "UPDATE workspace_ownership SET holder_kind='none' WHERE id=1",
        "UPDATE workspace_ownership SET holder_kind='desktop-embedded-extra' WHERE id=1",
        "UPDATE workspace_ownership SET holder_kind=CAST(holder_kind AS BLOB) WHERE id=1",
        "UPDATE workspace_ownership SET holder_kind=CAST(X'FF' AS TEXT) WHERE id=1",
        "UPDATE workspace_ownership SET holder_instance_id='' WHERE id=1",
        "UPDATE workspace_ownership SET holder_instance_id='gggggggggggggggggggggggggggggggg' WHERE id=1",
        "UPDATE workspace_ownership SET holder_instance_id=holder_instance_id||'a' WHERE id=1",
        "UPDATE workspace_ownership SET holder_instance_id=CAST(holder_instance_id AS BLOB) WHERE id=1",
        "UPDATE workspace_ownership SET holder_instance_id=CAST(X'FF' AS TEXT) WHERE id=1",
        "DELETE FROM runtime_state WHERE id=1",
        "UPDATE runtime_state SET mutation_seq=-1 WHERE id=1",
        "UPDATE runtime_state SET mutation_seq=0.5 WHERE id=1",
        "UPDATE runtime_state SET mutation_seq=X'30' WHERE id=1",
    ] {
        let mut fixture = Fixture::new();
        let expected = fixture.expected.clone();
        let saved = fixture.capture();
        fixture.connection().execute_batch("PRAGMA ignore_check_constraints=ON").unwrap();
        let tx = fixture.connection().transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        invalid_error(unchanged(&tx, || capture_workspace_in(&tx, &expected)));
        invalid_error(unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)));
        tx.rollback().unwrap();
    }
}

#[test]
fn exact_raw_json_byte_boundary_escaping_and_case_are_preserved_without_normalization() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let saved = fixture.capture();
    let raw = serde_json::to_string(&expected.workspace_id).unwrap();
    let exact = format!(
        "{raw}{}",
        " ".repeat(MAX_WORKSPACE_ID_JSON_BYTES - raw.len())
    );
    let tx = fixture.connection().transaction().unwrap();
    tx.execute(
        "UPDATE app_settings SET value_json=?1 WHERE key='workspace_id'",
        [&exact],
    )
    .unwrap();
    unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)).unwrap();
    tx.execute(
        "UPDATE app_settings SET value_json=?1 WHERE key='workspace_id'",
        [format!("{exact} ")],
    )
    .unwrap();
    invalid_error(unchanged(&tx, || capture_workspace_in(&tx, &expected)));
    assert_eq!(
        runtime_ledger::workspace_id(&tx).unwrap(),
        expected.workspace_id,
        "old public reader keeps its own behavior"
    );
    let escaped = format!(
        "\"{}\"",
        expected
            .workspace_id
            .bytes()
            .map(|b| format!("\\u{b:04x}"))
            .collect::<String>()
    );
    assert_eq!(escaped.len(), 194);
    tx.execute(
        "UPDATE app_settings SET value_json=?1 WHERE key='workspace_id'",
        [&escaped],
    )
    .unwrap();
    unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)).unwrap();
    let upper = "ABCDEF0123456789ABCDEF0123456789";
    tx.execute(
        "UPDATE app_settings SET value_json=?1 WHERE key='workspace_id'",
        [serde_json::to_string(upper).unwrap()],
    )
    .unwrap();
    let upper_expected = ExpectedWorkspace::new(
        upper,
        expected.epoch,
        expected.holder_kind,
        &expected.holder_instance_id,
    )
    .unwrap();
    assert_eq!(
        unchanged(&tx, || capture_workspace_in(&tx, &upper_expected))
            .unwrap()
            .workspace_id,
        upper
    );
    changed_error(unchanged(&tx, || {
        recheck_workspace_in(&tx, &upper_expected, &saved)
    }));
    tx.rollback().unwrap();
}

#[test]
fn invalid_metadata_precedes_comparison_and_missing_tables_propagate_sql_error() {
    let mut fixture = Fixture::new();
    let expected = fixture.expected.clone();
    let wrong = ExpectedWorkspace::new(
        &"f".repeat(32),
        expected.epoch,
        expected.holder_kind,
        &expected.holder_instance_id,
    )
    .unwrap();
    let saved = fixture.capture();
    let baseline = snapshot(fixture.connection());
    let tx = fixture.connection().transaction().unwrap();
    tx.execute_batch("DELETE FROM runtime_state WHERE id=1")
        .unwrap();
    invalid_error(unchanged(&tx, || capture_workspace_in(&tx, &wrong)));
    tx.execute_batch("ALTER TABLE runtime_state RENAME TO unavailable_state")
        .unwrap();
    assert!(matches!(
        unchanged(&tx, || capture_workspace_in(&tx, &expected)),
        Err(AppError::Db(_))
    ));
    assert!(matches!(
        unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)),
        Err(AppError::Db(_))
    ));
    tx.rollback().unwrap();
    assert_eq!(snapshot(fixture.connection()), baseline);
}

#[test]
fn nulls_in_damaged_metadata_schema_are_invalid_without_repair_or_defaults() {
    for sql in [
        "ALTER TABLE app_settings RENAME TO original_settings;
         CREATE TABLE app_settings(key TEXT PRIMARY KEY,value_json TEXT);
         INSERT INTO app_settings VALUES ('workspace_id',NULL)",
        "ALTER TABLE workspace_ownership RENAME TO original_owner;
         CREATE TABLE workspace_ownership(id INTEGER,epoch,holder_kind,holder_instance_id);
         INSERT INTO workspace_ownership SELECT 1,NULL,holder_kind,holder_instance_id FROM original_owner",
        "ALTER TABLE runtime_state RENAME TO original_state;
         CREATE TABLE runtime_state(id INTEGER,mutation_seq);
         INSERT INTO runtime_state VALUES (1,NULL)",
    ] {
        let mut fixture = Fixture::new();
        let expected = fixture.expected.clone();
        let saved = fixture.capture();
        let baseline = snapshot(fixture.connection());
        let tx = fixture.connection().transaction().unwrap();
        tx.execute_batch(sql).unwrap();
        invalid_error(unchanged(&tx, || capture_workspace_in(&tx, &expected)));
        invalid_error(unchanged(&tx, || recheck_workspace_in(&tx, &expected, &saved)));
        tx.rollback().unwrap();
        assert_eq!(snapshot(fixture.connection()), baseline);
    }
}
