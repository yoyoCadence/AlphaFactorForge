use super::*;
use rusqlite::TransactionBehavior;

fn source() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;
      CREATE TABLE sample(id INTEGER PRIMARY KEY AUTOINCREMENT, text TEXT, bytes BLOB, number REAL, optional TEXT);
      INSERT INTO sample VALUES (7,'original',X'0001FF',1.2345678901234567,NULL);
      INSERT INTO sample(id) VALUES (21); DELETE FROM sample WHERE id=21;
      CREATE TABLE audit(id INTEGER PRIMARY KEY, note TEXT);
      INSERT INTO audit VALUES (9,'first-audit');
      CREATE TABLE sqlitexstat_data(id INTEGER PRIMARY KEY); INSERT INTO sqlitexstat_data VALUES (17);
      CREATE TABLE implicit(value TEXT); INSERT INTO implicit(rowid,value) VALUES (5,'hole'),(42,'later');
      CREATE INDEX sample_text ON sample(text);
      CREATE TABLE quoted(\"a\"\"key\" TEXT PRIMARY KEY, value TEXT) WITHOUT ROWID;
      INSERT INTO quoted VALUES ('b','second'),('a','first');
      CREATE VIEW sample_view AS SELECT id,text FROM sample;
      CREATE TRIGGER view_insert INSTEAD OF INSERT ON sample_view BEGIN INSERT INTO sample(id,text) VALUES (NEW.id,NEW.text); END;
      CREATE TRIGGER sample_audit AFTER INSERT ON sample BEGIN INSERT INTO audit(note) VALUES ('inserted'); END;
      CREATE TRIGGER sample_immutable BEFORE DELETE ON sample BEGIN SELECT RAISE(ABORT,'immutable'); END;").unwrap();
    conn
}
fn snapshot(conn: &Connection) -> String {
    let mut out = String::new();
    let definitions:Vec<(String,String,Option<String>)>=conn.prepare("SELECT type,name,sql FROM sqlite_schema WHERE name NOT GLOB 'sqlite_stat[0-9]*' ORDER BY type,name").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<Result<_,_>>().unwrap();
    for (kind, name, sql) in definitions {
        out.push_str(&format!("{kind}:{name}:{sql:?}\n"));
        if kind != "table" {
            continue;
        }
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM {}", quoted(&name)))
            .unwrap();
        let n = stmt.column_count();
        let mut rows: Vec<Vec<Value>> = stmt
            .query_map([], |r| (0..n).map(|i| r.get(i)).collect())
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        rows.sort_by_cached_key(|r| format!("{r:?}"));
        out.push_str(&format!("{rows:?}\n"));
    }
    out
}
fn changes(conn: &Connection) -> u64 {
    conn.query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap()
}
fn directory(conn: &Connection) -> PathBuf {
    Path::new(conn.path().unwrap()).parent().unwrap().to_owned()
}

#[test]
fn exact_schema_typed_rows_original_audits_rowids_and_sequence_are_copied_without_source_writes() {
    let mut conn = source();
    let before = snapshot(&conn);
    let changed = changes(&conn);
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let mut root = None;
    let id = with_snapshot(&tx, |private| {
        root = Some(directory(private));
        assert_eq!(snapshot(private), before);
        assert_eq!(
            private.query_row("SELECT rowid FROM implicit WHERE value='later'", [], |r| {
                r.get::<_, i64>(0)
            })?,
            42
        );
        assert_eq!(
            private.query_row("SELECT rowid FROM sample", [], |r| r.get::<_, i64>(0))?,
            7
        );
        assert!(private.execute("DELETE FROM sample", []).is_err());
        private.execute("INSERT INTO sample(text) VALUES ('proposal')", [])?;
        let id = private.last_insert_rowid();
        assert_eq!(id, 22);
        assert_eq!(
            private.query_row("SELECT COUNT(*) FROM audit", [], |r| r.get::<_, i64>(0))?,
            2
        );
        Ok(id)
    })
    .unwrap();
    assert_eq!(id, 22);
    assert!(!root.unwrap().exists());
    assert_eq!(snapshot(&tx), before);
    assert_eq!(changes(&tx), changed);
    assert!(!tx.is_autocommit());
    tx.execute(
        "INSERT INTO sample(text) VALUES ('caller still owns transaction')",
        [],
    )
    .unwrap();
    tx.rollback().unwrap();
    assert_eq!(snapshot(&conn), before);
}
#[test]
fn callback_failure_and_panic_drop_private_writes_and_owned_files() {
    let mut conn = source();
    let before = snapshot(&conn);
    let changed = changes(&conn);
    let tx = conn.transaction().unwrap();
    let owned = Temporary::new().unwrap();
    let root = owned.path.clone().unwrap();
    let result: Result<(), _> = with_owned(&tx, owned, |private| {
        private.execute("INSERT INTO sample(text) VALUES ('private failure')", [])?;
        Err(bad("injected callback failure"))
    });
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("injected callback failure"));
    assert!(!root.exists());
    let owned = Temporary::new().unwrap();
    let root = owned.path.clone().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), LedgerError> = with_owned(&tx, owned, |private| {
            private.execute("INSERT INTO sample(text) VALUES ('private panic')", [])?;
            panic!("injected private callback panic")
        });
    }));
    assert!(result.is_err());
    assert!(!root.exists());
    assert_eq!(snapshot(&tx), before);
    assert_eq!(changes(&tx), changed);
    assert!(!tx.is_autocommit());
}
#[test]
fn missing_source_foreign_key_is_refused_before_proposal_and_cleanup_completes() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=OFF; CREATE TABLE p(id INTEGER PRIMARY KEY); CREATE TABLE c(id INTEGER PRIMARY KEY,parent INTEGER REFERENCES p(id)); INSERT INTO c VALUES (1,99); PRAGMA foreign_keys=ON").unwrap();
    let before = snapshot(&conn);
    let tx = conn.transaction().unwrap();
    let owned = Temporary::new().unwrap();
    let root = owned.path.clone().unwrap();
    let mut called = false;
    let result = with_owned(&tx, owned, |_| {
        called = true;
        Ok(())
    });
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("foreign-key dependency missing"));
    assert!(!called);
    assert!(!root.exists());
    assert_eq!(snapshot(&tx), before);
}

#[test]
fn deferred_proposed_foreign_key_failure_is_checked_before_any_value_can_escape() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE p(id INTEGER PRIMARY KEY); CREATE TABLE c(id INTEGER PRIMARY KEY,parent INTEGER REFERENCES p(id) DEFERRABLE INITIALLY DEFERRED)").unwrap();
    let before = snapshot(&conn);
    let tx = conn.transaction().unwrap();
    let owned = Temporary::new().unwrap();
    let root = owned.path.clone().unwrap();
    let result = with_owned(&tx, owned, |private| {
        private.execute("INSERT INTO c VALUES (1,99)", [])?;
        Ok("must not return a proposed state that cannot commit")
    });
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("proposed foreign-key dependency missing"));
    assert!(!root.exists());
    assert_eq!(snapshot(&tx), before);
}

#[test]
fn accidental_private_commit_is_refused_and_never_commits_the_callers_transaction() {
    let mut conn = source();
    let before = snapshot(&conn);
    let tx = conn.transaction().unwrap();
    let owned = Temporary::new().unwrap();
    let root = owned.path.clone().unwrap();
    let result = with_owned(&tx, owned, |private| {
        private.execute(
            "INSERT INTO sample(text) VALUES ('private premature commit')",
            [],
        )?;
        private.execute_batch("COMMIT")?;
        Ok(())
    });
    assert!(result.is_err());
    assert!(!root.exists());
    assert_eq!(snapshot(&tx), before);
    assert!(!tx.is_autocommit());
}
#[test]
fn failed_clone_and_ended_source_transaction_never_invoke_proposal_or_keep_temp() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE shadowed(rowid TEXT,_rowid_ TEXT,oid TEXT); INSERT INTO shadowed VALUES ('a','b','c')").unwrap();
    let before = snapshot(&conn);
    let tx = conn.transaction().unwrap();
    let mut called = false;
    let owned = Temporary::new().unwrap();
    let root = owned.path.clone().unwrap();
    assert!(with_owned(&tx, owned, |_| {
        called = true;
        Ok(())
    })
    .is_err());
    assert!(!called);
    assert!(!root.exists());
    assert_eq!(snapshot(&tx), before);
    tx.execute_batch("COMMIT").unwrap();
    let owned = Temporary::new().unwrap();
    let root = owned.path.clone().unwrap();
    assert!(with_owned(&tx, owned, |_| {
        called = true;
        Ok(())
    })
    .unwrap_err()
    .to_string()
    .contains("not active"));
    assert!(!called);
    assert!(!root.exists());
}
#[test]
fn wal_snapshot_uses_actual_borrowed_transaction_and_keeps_independent_writers_blocked() {
    let owned = Temporary::new().unwrap();
    let path = owned.database();
    let mut conn = Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE t(id INTEGER PRIMARY KEY,value TEXT); INSERT INTO t VALUES (1,'committed')").unwrap();
    let second = Connection::open(&path).unwrap();
    second.busy_timeout(std::time::Duration::ZERO).unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute("INSERT INTO t VALUES (2,'caller-owned uncommitted')", [])
        .unwrap();
    let before = changes(&tx);
    let root = with_snapshot(&tx, |private| {
        assert_eq!(
            private.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0))?,
            2
        );
        assert_eq!(
            second.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0))?,
            1
        );
        assert!(second
            .execute("INSERT INTO t VALUES (3,'independent writer')", [])
            .is_err());
        private.execute("INSERT INTO t VALUES (3,'private proposal')", [])?;
        Ok(directory(private))
    })
    .unwrap();
    assert!(!root.exists());
    assert_eq!(changes(&tx), before);
    assert_eq!(
        tx.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert!(second
        .execute("INSERT INTO t VALUES (3,'still blocked')", [])
        .is_err());
    tx.commit().unwrap();
    second
        .execute("INSERT INTO t VALUES (3,'after caller commit')", [])
        .unwrap();
    second.close().unwrap();
    conn.close().unwrap();
    owned.remove().unwrap();
}
#[test]
fn recursive_trigger_setting_and_views_apply_only_to_private_proposal() {
    for recursive in [false, true] {
        let mut conn = source();
        conn.pragma_update(None, "recursive_triggers", recursive)
            .unwrap();
        conn.execute_batch("CREATE TRIGGER recurse AFTER INSERT ON sample WHEN NEW.id BETWEEN 100 AND 102 BEGIN INSERT INTO sample(id,text) VALUES (NEW.id+1,'recursive'); END").unwrap();
        let before = snapshot(&conn);
        let tx = conn.transaction().unwrap();
        with_snapshot(&tx, |private| {
            assert_eq!(
                private.query_row("PRAGMA recursive_triggers", [], |r| r.get::<_, bool>(0))?,
                recursive
            );
            private.execute("INSERT INTO sample_view VALUES (100,'view insertion')", [])?;
            assert_eq!(
                private.query_row("SELECT COUNT(*) FROM sample WHERE id>=100", [], |r| r
                    .get::<_, i64>(0))?,
                if recursive { 4 } else { 2 }
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(snapshot(&tx), before);
    }
}
#[test]
fn optimizer_statistics_do_not_replace_registry_rows_and_private_permissions_are_restrictive() {
    let mut conn = source();
    conn.execute_batch("ANALYZE").unwrap();
    let before = snapshot(&conn);
    let tx = conn.transaction().unwrap();
    with_snapshot(&tx, |private| {
        assert_eq!(snapshot(private), before);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file = Path::new(private.path().unwrap());
            assert_eq!(fs::metadata(file)?.permissions().mode() & 0o777, 0o600);
            assert_eq!(
                fs::metadata(file.parent().unwrap())?.permissions().mode() & 0o777,
                0o700
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(snapshot(&tx), before);
}

#[cfg(windows)]
#[test]
fn failed_owned_cleanup_returns_error_and_source_is_unchanged() {
    use std::os::windows::fs::OpenOptionsExt;
    let mut conn = source();
    let before = snapshot(&conn);
    let tx = conn.transaction().unwrap();
    let owned = Temporary::new().unwrap();
    let root = owned.path.clone().unwrap();
    let mut held = None;
    let result = with_owned(&tx, owned, |private| {
        let marker = directory(private).join("held-cleanup-probe");
        held = Some(
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .share_mode(0)
                .open(marker)?,
        );
        private.execute(
            "INSERT INTO sample(text) VALUES ('must not escape failed cleanup')",
            [],
        )?;
        Ok(())
    });
    assert_eq!(result.unwrap_err().code(), "io_error");
    assert_eq!(snapshot(&tx), before);
    drop(held);
    assert!(root.parent().unwrap() == fs::canonicalize(std::env::temp_dir()).unwrap());
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("aff-registry-preflight-"));
    fs::remove_dir_all(root).unwrap();
}
