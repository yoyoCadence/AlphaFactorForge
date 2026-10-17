//! SQL-shape fixtures intentionally are not checked confirmation graphs. The
//! graph writer/reader is the next slice; these tests exercise database fences.
use super::*;
use crate::research::trial_ledger::{
    migrate, TrialLedger, REGISTRY_FILE_NAME, REGISTRY_MIGRATIONS,
};
use alpha_factor_forge::discovery_core::{
    confirmation_history::MAX_DOCUMENT_BYTES,
    market_data::{MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE, MIN_MARKET_TIMESTAMP_MS},
};
use rusqlite::OptionalExtension;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
struct Dirs {
    root: PathBuf,
    registry: PathBuf,
    workspace: PathBuf,
}
impl Dirs {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "aff-confirmation-schema-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let registry = root.join("evidence");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        Self {
            root,
            registry,
            workspace,
        }
    }
    fn open(&self) -> TrialLedger {
        TrialLedger::open_with_sync_roots(&self.registry, &self.workspace, &[]).unwrap()
    }
}
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    migrate(&mut c).unwrap();
    c
}
fn old() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("CREATE TABLE registry_migrations(version TEXT PRIMARY KEY,applied_at TEXT NOT NULL DEFAULT(datetime('now')))").unwrap();
    for (version, sql) in REGISTRY_MIGRATIONS.iter().take(5) {
        c.execute_batch(sql).unwrap();
        if version.starts_with("0001") {
            c.execute(
                "INSERT INTO registry_meta VALUES ('registry_id',?1)",
                ["a".repeat(32)],
            )
            .unwrap();
        }
        c.execute(
            "INSERT INTO registry_migrations(version) VALUES (?1)",
            [version],
        )
        .unwrap();
    }
    c
}
fn id() -> String {
    "1".repeat(64)
}
fn family() -> String {
    format!("trial-family-v1:{}", id())
}
fn rows() -> Vec<(&'static str, Vec<&'static str>, Vec<rusqlite::types::Value>)> {
    use rusqlite::types::Value as V;
    let text = |s: &str| V::Text(s.into());
    let x = || text(&id());
    let a = || text("first");
    let d = || text("{}");
    vec![
        (
            "confirmation_batches",
            vec![
                "batch_id",
                "family_id",
                "stage",
                "parent_batch_id",
                "selection_id",
                "document_json",
                "recorded_at",
            ],
            vec![
                x(),
                text(&family()),
                text("validation"),
                V::Null,
                V::Null,
                d(),
                a(),
            ],
        ),
        (
            "confirmation_selections",
            vec![
                "selection_id",
                "validation_batch_id",
                "sealed_receipt_id",
                "alpha_reservation_id",
                "report_sha256",
                "document_json",
                "recorded_at",
            ],
            vec![x(), x(), x(), x(), x(), d(), a()],
        ),
        (
            "confirmation_operations",
            vec![
                "operation_id",
                "origin_registry_id",
                "reservation_key",
                "workspace_id",
                "batch_id",
                "family_id",
                "document_json",
                "recorded_at",
            ],
            vec![
                x(),
                text(&"2".repeat(32)),
                text(&"3".repeat(32)),
                text(&"4".repeat(32)),
                x(),
                text(&family()),
                d(),
                a(),
            ],
        ),
        (
            "confirmation_exposures",
            vec![
                "exposure_id",
                "instrument_id",
                "kind",
                "operation_id",
                "alpha_reservation_id",
                "document_json",
                "recorded_at",
            ],
            vec![
                x(),
                text("crypto:binance:BTCUSDT"),
                text("confirmation"),
                x(),
                x(),
                d(),
                a(),
            ],
        ),
        (
            "confirmation_intervals",
            vec![
                "exposure_id",
                "interval_index",
                "instrument_id",
                "from_ms",
                "to_exclusive_ms",
            ],
            vec![
                x(),
                V::Integer(0),
                text("crypto:binance:BTCUSDT"),
                V::Integer(946684800000),
                V::Integer(946684800010),
            ],
        ),
        (
            "confirmation_admissions",
            vec![
                "admission_id",
                "operation_id",
                "alpha_reservation_id",
                "exposure_id",
                "binding_sha256",
                "document_json",
                "recorded_at",
            ],
            vec![x(), x(), x(), x(), x(), d(), a()],
        ),
        (
            "confirmation_events",
            vec![
                "event_id",
                "operation_id",
                "admission_id",
                "kind",
                "document_json",
                "recorded_at",
            ],
            vec![x(), x(), x(), text("sealed"), d(), a()],
        ),
        (
            "confirmation_blobs",
            vec![
                "blob_sha256",
                "kind",
                "byte_length",
                "content_bytes",
                "recorded_at",
            ],
            vec![
                x(),
                text("statistics"),
                V::Integer(2),
                V::Blob(b"{}".to_vec()),
                a(),
            ],
        ),
        (
            "confirmation_conflicts",
            vec!["conflict_id", "document_json", "recorded_at"],
            vec![x(), d(), a()],
        ),
        (
            "confirmation_unknown_origins",
            vec!["origin_registry_id", "recorded_at"],
            vec![text(&"2".repeat(32)), a()],
        ),
    ]
}
fn insert(
    c: &Connection,
    table: &str,
    columns: &[&str],
    values: &[rusqlite::types::Value],
    prefix: &str,
) -> rusqlite::Result<usize> {
    let marks = vec!["?"; columns.len()].join(",");
    c.execute(
        &format!(
            "{prefix} INTO {table} ({}) VALUES ({marks})",
            columns.join(",")
        ),
        rusqlite::params_from_iter(values),
    )
}
#[test]
fn fresh_birth_is_immutable_and_old_registry_never_receives_it() {
    let c = fresh();
    let birth: String = c
        .query_row(
            "SELECT value FROM registry_meta WHERE key='confirmation_history_birth'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(birth, "0006_confirmation_history");
    assert!(unknown_origins(&c).unwrap().is_empty());
    assert!(c
        .execute(
            "UPDATE registry_meta SET value='x' WHERE key='confirmation_history_birth'",
            []
        )
        .is_err());
    assert!(c
        .execute(
            "DELETE FROM registry_meta WHERE key='confirmation_history_birth'",
            []
        )
        .is_err());
    assert!(c
        .execute(
            "INSERT OR REPLACE INTO registry_meta VALUES ('confirmation_history_birth','changed')",
            []
        )
        .is_err());
    let mut c = old();
    migrate(&mut c).unwrap();
    assert_eq!(
        unknown_origins(&c).unwrap(),
        BTreeSet::from(["a".repeat(32)])
    );
    assert!(c
        .query_row(
            "SELECT value FROM registry_meta WHERE key='confirmation_history_birth'",
            [],
            |r| r.get::<_, String>(0)
        )
        .optional()
        .unwrap()
        .is_none());
}
#[test]
fn every_table_refuses_update_delete_and_replacement_without_recursive_triggers() {
    let c = fresh();
    c.execute_batch("PRAGMA foreign_keys=OFF;PRAGMA recursive_triggers=OFF")
        .unwrap();
    for (table, columns, values) in rows() {
        insert(&c, table, &columns, &values, "INSERT").unwrap();
        let column = if table == "confirmation_intervals" {
            "from_ms"
        } else {
            "recorded_at"
        };
        assert!(
            c.execute(&format!("UPDATE {table} SET {column}={column}"), [])
                .is_err(),
            "{table}"
        );
        assert!(
            c.execute(&format!("DELETE FROM {table}"), []).is_err(),
            "{table}"
        );
        let mut changed = values.clone();
        let n = columns.iter().position(|v| *v == column).unwrap();
        changed[n] = if column == "from_ms" {
            rusqlite::types::Value::Integer(946684800001)
        } else {
            rusqlite::types::Value::Text("replacement".into())
        };
        assert!(
            insert(&c, table, &columns, &changed, "INSERT OR REPLACE").is_err(),
            "{table}"
        );
        // Content replacement must also fail while the original audit time
        // stays identical; all substituted payloads still satisfy SQL shapes.
        let mut payload = values.clone();
        if let Some(n) = columns.iter().position(|v| *v == "document_json") {
            payload[n] = rusqlite::types::Value::Text("{\"changed\":true}".into());
            assert!(
                insert(&c, table, &columns, &payload, "INSERT OR REPLACE").is_err(),
                "{table}"
            );
        } else if let Some(n) = columns.iter().position(|v| *v == "content_bytes") {
            payload[n] = rusqlite::types::Value::Blob(b"[]".to_vec());
            assert!(
                insert(&c, table, &columns, &payload, "INSERT OR REPLACE").is_err(),
                "{table}"
            );
        }
        assert_eq!(
            insert(&c, table, &columns, &values, "INSERT OR IGNORE").unwrap(),
            0
        );
    }
}
#[test]
fn all_new_foreign_keys_defer_until_commit_and_late_failure_rolls_back() {
    let mut c = fresh();
    {
        let tx = c.transaction().unwrap();
        for (table, columns, values) in rows() {
            insert(&tx, table, &columns, &values, "INSERT").unwrap();
        }
        assert!(tx.commit().is_err());
    }
    for table in TABLES {
        assert_eq!(
            c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    let tx = c.transaction().unwrap();
    for (table, columns, values) in rows() {
        insert(&tx, table, &columns, &values, "INSERT").unwrap();
    }
    tx.execute(
        "INSERT INTO trial_families (family_id,family_key,created_at) VALUES (?1,'{}','now')",
        [family()],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO alpha_family_budgets VALUES (?1,?2,'{}','now')",
        params![family(), id()],
    )
    .unwrap();
    tx.execute(
        "INSERT INTO alpha_reservations VALUES (?1,?2,'{}','now')",
        params![id(), family()],
    )
    .unwrap();
    tx.commit().unwrap();
}
#[test]
fn independent_connection_and_reopen_preserve_birth_and_unknown_receipts() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let origin = "b".repeat(32);
    {
        let c = ledger.lock().unwrap();
        record_unknown_origins(&c, &BTreeSet::from([origin.clone()])).unwrap();
    }
    let other = Connection::open(dirs.registry.join(REGISTRY_FILE_NAME)).unwrap();
    assert_eq!(
        unknown_origins(&other).unwrap(),
        BTreeSet::from([origin.clone()])
    );
    assert!(other
        .execute("DELETE FROM confirmation_unknown_origins", [])
        .is_err());
    drop(other);
    drop(ledger);
    let ledger = dirs.open();
    assert_eq!(
        unknown_origins(&ledger.lock().unwrap()).unwrap(),
        BTreeSet::from([origin])
    );
}
#[test]
fn old_backfill_keeps_all_known_origins_and_capacity_failure_leaves_migration_unapplied() {
    let mut c = old();
    // Isolate migration backfill SQL: checkpoint event payloads are not part
    // of this fixture. Linked/deferred FK behavior is tested separately.
    c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    for ch in ['b', 'c'] {
        c.execute(
            "INSERT INTO origin_genesis VALUES (?1,'old')",
            [ch.to_string().repeat(32)],
        )
        .unwrap();
    }
    c.execute("INSERT INTO registry_imports (source_registry_id,file_sha256,head_seq,head_chain,added_events,skipped_events,conflicts,imported_at) VALUES (?1,'file',0,'head',0,0,0,'old')",["d".repeat(32)]).unwrap();
    c.execute("INSERT INTO origin_checkpoints (origin_registry_id,origin_seq,event_id,origin_chain) VALUES (?1,1,'event','head')",["e".repeat(32)]).unwrap();
    migrate(&mut c).unwrap();
    assert_eq!(
        unknown_origins(&c).unwrap(),
        ['a', 'b', 'c', 'd', 'e']
            .into_iter()
            .map(|v| v.to_string().repeat(32))
            .collect()
    );
    let mut c = old();
    {
        let tx = c.transaction().unwrap();
        for n in 0..10000 {
            tx.execute(
                "INSERT INTO origin_genesis VALUES (?1,'old')",
                [format!("{n:032x}")],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    assert!(migrate(&mut c).is_err());
    let exists: bool = c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='confirmation_unknown_origins')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!exists);
    let applied:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM registry_migrations WHERE version='0006_confirmation_history')",[],|r|r.get(0)).unwrap();
    assert!(!applied);
}
#[test]
fn empty_legacy_imports_keep_unknowns_and_public_exports_refuse_omission() {
    let source_dirs = Dirs::new();
    let source = source_dirs.open();
    let trial = source.export_json_lines().unwrap();
    let alpha = source.export_complete_json_lines().unwrap();
    for bytes in [&trial, &alpha] {
        let target_dirs = Dirs::new();
        let target = target_dirs.open();
        if bytes == &trial {
            target.import_json_lines(bytes).unwrap();
        } else {
            assert_eq!(
                target
                    .import_qualification_json_lines(bytes)
                    .unwrap_err()
                    .code(),
                "confirmation_history_unknown"
            );
            assert!(unknown_origins(&target.lock().unwrap()).unwrap().is_empty());
            target.import_complete_json_lines(bytes).unwrap();
        }
        let saved = unknown_origins(&target.lock().unwrap()).unwrap();
        assert!(saved.contains(source.registry_id()));
        assert!(target.export_json_lines().is_err());
        assert!(target.export_complete_json_lines().is_err());
        if bytes == &trial {
            target.import_json_lines(bytes).unwrap();
        } else {
            target.import_complete_json_lines(bytes).unwrap();
        }
        assert_eq!(unknown_origins(&target.lock().unwrap()).unwrap(), saved);
    }
}

#[test]
fn public_legacy_export_checks_every_confirmation_table() {
    for (table, columns, values) in rows() {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        {
            let c = ledger.lock().unwrap();
            c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
            insert(&c, table, &columns, &values, "INSERT").unwrap();
        }
        for result in [
            ledger.export_json_lines(),
            ledger.export_complete_json_lines(),
        ] {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("complete v5 export"),
                "{table}"
            );
        }
    }
}

#[test]
fn sql_domains_refuse_wrong_ids_variants_nullability_bytes_and_time_bounds() {
    use rusqlite::types::Value as V;
    for (table, columns, values) in rows() {
        let c = fresh();
        c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        for (index, column) in columns.iter().enumerate() {
            let candidates = if column.ends_with("_id")
                || *column == "reservation_key"
                || *column == "report_sha256"
                || *column == "binding_sha256"
                || *column == "blob_sha256"
            {
                if *column == "instrument_id" {
                    vec![V::Text("x".repeat(257))]
                } else {
                    let original = match &values[index] {
                        V::Text(value) => value.clone(),
                        _ => "0".repeat(64),
                    };
                    vec![
                        V::Text("g".repeat(original.len())),
                        V::Text("A".repeat(original.len())),
                        V::Text(String::new()),
                        V::Text(original.clone() + "\0suffix"),
                        V::Blob(original.into_bytes()),
                    ]
                }
            } else if *column == "stage" || *column == "kind" {
                vec![V::Text("future_variant".into())]
            } else if *column == "interval_index" {
                vec![V::Integer(-1), V::Integer(128), V::Real(0.5)]
            } else if *column == "from_ms" {
                vec![
                    V::Integer(MIN_MARKET_TIMESTAMP_MS - 1),
                    V::Integer(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE),
                    V::Real(MIN_MARKET_TIMESTAMP_MS as f64 + 0.5),
                ]
            } else if *column == "to_exclusive_ms" {
                vec![
                    V::Integer(MIN_MARKET_TIMESTAMP_MS),
                    V::Integer(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE + 1),
                ]
            } else if *column == "byte_length" {
                vec![
                    V::Integer(0),
                    V::Integer(8388609),
                    V::Integer(3),
                    V::Real(2.5),
                ]
            } else if *column == "content_bytes" {
                vec![V::Text("{}".into())]
            } else {
                vec![]
            };
            for bad in candidates {
                let mut v = values.clone();
                v[index] = bad;
                assert!(
                    insert(&c, table, &columns, &v, "INSERT").is_err(),
                    "{table}.{column}"
                );
            }
            if values[index] != V::Null {
                let mut v = values.clone();
                v[index] = V::Null;
                assert!(
                    insert(&c, table, &columns, &v, "INSERT").is_err(),
                    "{table}.{column} null"
                );
            }
        }
    }
    let c = fresh();
    c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    let (table, columns, mut v) = rows().remove(4);
    v[4] = V::Integer(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE);
    insert(&c, table, &columns, &v, "INSERT").unwrap();
}

#[test]
fn document_and_blob_caps_count_original_utf8_bytes() {
    use rusqlite::types::Value as V;
    for (table, columns, mut v) in rows()
        .into_iter()
        .filter(|(_, columns, _)| columns.contains(&"document_json"))
    {
        let c = fresh();
        c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        let index = columns.iter().position(|s| *s == "document_json").unwrap();
        v[index] = V::Text("é".repeat(MAX_DOCUMENT_BYTES / 2) + " ");
        assert!(
            insert(&c, table, &columns, &v, "INSERT").is_err(),
            "{table}"
        );
        v[index] = V::Text("é".repeat(MAX_DOCUMENT_BYTES / 2));
        insert(&c, table, &columns, &v, "INSERT").unwrap();
    }
    for (kind, cap) in [("statistics", 4194304usize), ("binding", 8388608usize)] {
        let c = fresh();
        let (table, columns, mut v) = rows().remove(7);
        v[1] = V::Text(kind.into());
        v[2] = V::Integer((cap + 1) as i64);
        v[3] = V::Blob(vec![b' '; cap + 1]);
        assert!(insert(&c, table, &columns, &v, "INSERT").is_err());
        v[2] = V::Integer(cap as i64);
        v[3] = V::Blob(vec![b' '; cap]);
        insert(&c, table, &columns, &v, "INSERT").unwrap();
    }
}

#[test]
fn joint_document_blob_row_capacity_and_idempotent_duplicates_are_exact() {
    let mut c = fresh();
    let trigger: String = c
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='confirmation_conflicts_capacity'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    c.execute_batch("DROP TRIGGER confirmation_conflicts_capacity")
        .unwrap();
    let tx = c.transaction().unwrap();
    for n in 0..9999 {
        tx.execute(
            "INSERT INTO confirmation_conflicts VALUES (?1,'{}','first')",
            [format!("{n:064x}")],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    c.execute_batch(&trigger).unwrap();
    c.execute(
        "INSERT INTO confirmation_conflicts VALUES (?1,'{}','first')",
        [format!("{:064x}", 9999)],
    )
    .unwrap();
    let (table, columns, values) = rows().remove(7);
    assert!(insert(&c, table, &columns, &values, "INSERT").is_err());
    assert_eq!(
        c.execute(
            "INSERT OR IGNORE INTO confirmation_conflicts VALUES (?1,'{}','first')",
            [format!("{:064x}", 0)]
        )
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM confirmation_conflicts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        10000
    );
}

#[test]
fn combined_byte_cap_and_late_failure_rollback_include_blobs() {
    let mut c = fresh();
    let document = " ".repeat(MAX_DOCUMENT_BYTES);
    {
        let tx = c.transaction().unwrap();
        for n in 0..16 {
            tx.execute(
                "INSERT INTO confirmation_conflicts VALUES (?1,?2,'first')",
                params![format!("{n:064x}"), document],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    let (table, columns, values) = rows().remove(7);
    assert!(insert(&c, table, &columns, &values, "INSERT").is_err());
    assert_eq!(
        c.query_row(
            "SELECT SUM(length(CAST(document_json AS BLOB))) FROM confirmation_conflicts",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        67108864
    );
    let mut c = fresh();
    {
        let tx = c.transaction().unwrap();
        tx.execute(
            "INSERT INTO confirmation_unknown_origins VALUES (?1,'first')",
            ["b".repeat(32)],
        )
        .unwrap();
        let mut failed = false;
        for n in 0..17 {
            if tx
                .execute(
                    "INSERT INTO confirmation_conflicts VALUES (?1,?2,'first')",
                    params![format!("{n:064x}"), document],
                )
                .is_err()
            {
                failed = true;
                break;
            }
        }
        assert!(failed);
    }
    assert!(unknown_origins(&c).unwrap().is_empty());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM confirmation_conflicts", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn interval_capacity_is_exact_after_bounded_bulk_fixture_setup() {
    let mut c = fresh();
    c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    let trigger: String = c
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='confirmation_intervals_capacity'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    // Bulk fixture setup omits only the aggregate-count trigger, then restores
    // its exact persisted definition before all boundary assertions.
    c.execute_batch("DROP TRIGGER confirmation_intervals_capacity")
        .unwrap();
    {
        let tx = c.transaction().unwrap();
        let mut stmt = tx
            .prepare(
                "INSERT INTO confirmation_intervals VALUES (?1,?2,'crypto:binance:BTCUSDT',?3,?4)",
            )
            .unwrap();
        for n in 0..127999 {
            stmt.execute(params![
                format!("{:064x}", n / 128),
                n % 128,
                MIN_MARKET_TIMESTAMP_MS + 2 * (n % 128),
                MIN_MARKET_TIMESTAMP_MS + 2 * (n % 128) + 1
            ])
            .unwrap();
        }
        drop(stmt);
        tx.commit().unwrap();
    }
    c.execute_batch(&trigger).unwrap();
    c.execute(
        "INSERT INTO confirmation_intervals VALUES (?1,127,'crypto:binance:BTCUSDT',?2,?3)",
        params![
            format!("{:064x}", 999),
            MIN_MARKET_TIMESTAMP_MS + 254,
            MIN_MARKET_TIMESTAMP_MS + 255
        ],
    )
    .unwrap();
    assert!(c
        .execute(
            "INSERT INTO confirmation_intervals VALUES (?1,0,'crypto:binance:BTCUSDT',?2,?3)",
            params![
                format!("{:064x}", 1000),
                MIN_MARKET_TIMESTAMP_MS,
                MIN_MARKET_TIMESTAMP_MS + 1
            ]
        )
        .is_err());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM confirmation_intervals", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        128000
    );
}

#[test]
fn permanent_unknown_origin_preflight_is_exact_and_diagnostics_are_static() {
    let c = fresh();
    let origins: BTreeSet<_> = (0..10000).map(|v| format!("{v:032x}")).collect();
    record_unknown_origins(&c, &origins).unwrap();
    record_unknown_origins(&c, &origins).unwrap();
    assert!(record_unknown_origins(&c, &BTreeSet::from(["f".repeat(32)])).is_err());
    let err = record_unknown_origins(&c, &BTreeSet::from(["PROTECTED_ORIGIN_SENTINEL".into()]))
        .unwrap_err();
    assert!(!err.to_string().contains("PROTECTED"));
    assert_eq!(unknown_origins(&c).unwrap(), origins);
}

fn populated_source() -> (Dirs, TrialLedger) {
    use crate::research::trial_ledger::{
        family_id_for, TrialBatchInput, TrialEventInput, TrialKind, TrialOrigin,
    };
    use alpha_factor_forge::discovery_core::alpha_allocation::AlphaAllocationDeclaration;
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let instrument = "crypto:binance:BTCUSDT";
    ledger
        .register_batch(&TrialBatchInput {
            workspace_id: "confirmation-schema-test".into(),
            instrument_id: Some(instrument.into()),
            tests_per_trial: 2,
            events: vec![TrialEventInput {
                kind: TrialKind::Variant,
                origin: TrialOrigin::Request {
                    request_id: "schema-source".into(),
                    candidate_index: 0,
                },
                hypothesis_hash: Some("hypothesis".into()),
                strategy_hash: Some("strategy".into()),
                dataset_hash: Some("dataset".into()),
                snapshot_id: Some("snapshot".into()),
                split_hash: Some("split".into()),
                seeds_hash: Some("seeds".into()),
                engine_fingerprint_hash: Some("engine".into()),
                benchmark_id: None,
                benchmark_params_hash: None,
                reproduction_of: None,
                benchmark_evidence: None,
            }],
        })
        .unwrap();
    let family = family_id_for(instrument).unwrap();
    ledger
        .declare_alpha_budget(
            &family,
            &AlphaAllocationDeclaration {
                total_alpha_ppm: 50000,
                schedule: vec![25000, 25000],
            },
        )
        .unwrap();
    ledger
        .reserve_alpha_evidence(&family, &"a".repeat(32), &"b".repeat(64))
        .unwrap();
    (dirs, ledger)
}

#[test]
fn late_confirmation_marker_failure_rolls_back_trial_alpha_and_import_log() {
    let (_dirs, source) = populated_source();
    let trial = source.export_trial_projection_for_test().unwrap();
    let alpha = source.export_complete_json_lines().unwrap();
    for (complete, archive) in [(false, trial), (true, alpha)] {
        let target_dirs = Dirs::new();
        let target = target_dirs.open();
        {
            target.lock().unwrap().execute_batch("CREATE TRIGGER fail_confirmation_marker BEFORE INSERT ON confirmation_unknown_origins BEGIN SELECT RAISE(ABORT,'injected confirmation marker failure'); END;").unwrap();
        }
        if complete {
            assert!(target.import_complete_json_lines(&archive).is_err());
        } else {
            assert!(target.import_json_lines(&archive).is_err());
        }
        let c = target.lock().unwrap();
        for table in [
            "trial_families",
            "trial_events",
            "trial_batches",
            "batch_receipts",
            "registry_imports",
            "origin_genesis",
            "origin_checkpoints",
            "alpha_family_budgets",
            "alpha_reservations",
            "alpha_unknown_origins",
            "confirmation_unknown_origins",
        ] {
            assert_eq!(
                c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                0,
                "{table}"
            );
        }
    }
}

#[test]
fn required_indexes_cover_the_merged_lookup_contract() {
    let c = fresh();
    for (index, expected) in [
        (
            "operations_origin_key",
            vec!["origin_registry_id", "reservation_key"],
        ),
        ("operations_family", vec!["family_id"]),
        ("exposures_operation", vec!["operation_id"]),
        (
            "intervals_support",
            vec!["instrument_id", "from_ms", "to_exclusive_ms"],
        ),
        ("admissions_operation", vec!["operation_id"]),
        ("events_operation_kind", vec!["operation_id", "kind"]),
        ("batches_family", vec!["family_id"]),
    ] {
        let actual: Vec<String> = c
            .prepare(&format!("PRAGMA index_info(idx_confirmation_{index})"))
            .unwrap()
            .query_map([], |r| r.get(2))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(actual, expected);
    }
}
