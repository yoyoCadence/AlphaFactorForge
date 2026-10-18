//! SQL guards are intentionally independent of the source-proof graph codecs.
use super::*;
use sha2::{Digest, Sha256};

fn old_six() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE registry_migrations(version TEXT PRIMARY KEY,applied_at TEXT NOT NULL DEFAULT(datetime('now')))").unwrap();
    for (version, sql) in REGISTRY_MIGRATIONS.iter().take(6) {
        c.execute_batch(sql).unwrap();
        if version.starts_with("0001") {
            c.execute(
                "INSERT INTO registry_meta VALUES ('registry_id',?1)",
                ["a".repeat(32)],
            )
            .unwrap();
            c.execute("INSERT INTO registry_meta VALUES ('confirmation_history_birth','0006_confirmation_history')", []).unwrap();
        }
        c.execute(
            "INSERT INTO registry_migrations(version) VALUES (?1)",
            [version],
        )
        .unwrap();
    }
    c
}
fn witness(c: &Connection, id: &str, bytes: &[u8], audit: &str) -> rusqlite::Result<usize> {
    c.execute(
        "INSERT INTO confirmation_source_witnesses VALUES (?1,?2,?3,?4)",
        params![id, bytes.len() as i64, bytes, audit],
    )
}
fn applied(c: &Connection) -> bool {
    c.query_row("SELECT EXISTS(SELECT 1 FROM registry_migrations WHERE version='0007_confirmation_source_witness')", [], |r| r.get(0)).unwrap()
}
fn table_exists(c: &Connection) -> bool {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='confirmation_source_witnesses')",
        [],
        |r| r.get(0),
    )
    .unwrap()
}
fn binding() -> String {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-source-witness-v1.json"
    ))
    .unwrap();
    fixture["cases"][0]["bindingJson"].as_str().unwrap().into()
}
fn old_blob(c: &Connection, text: &str, sha: Option<&str>) {
    let id = sha
        .map(str::to_string)
        .unwrap_or_else(|| hex::encode(Sha256::digest(text.as_bytes())));
    c.execute(
        "INSERT INTO confirmation_blobs VALUES (?1,'binding',?2,?3,'old-audit')",
        params![id, text.len() as i64, text.as_bytes()],
    )
    .unwrap();
}

#[test]
fn witness_schema_has_exact_columns_no_receiver_foreign_keys_and_immutable_namespace() {
    let c = fresh();
    let columns: Vec<String> = c
        .prepare("PRAGMA table_info(confirmation_source_witnesses)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        columns,
        [
            "witness_sha256",
            "byte_length",
            "content_bytes",
            "recorded_at"
        ]
    );
    assert!(c
        .prepare("PRAGMA foreign_key_list(confirmation_source_witnesses)")
        .unwrap()
        .query([])
        .unwrap()
        .next()
        .unwrap()
        .is_none());
    c.execute_batch("PRAGMA recursive_triggers=OFF").unwrap();
    let id = "1".repeat(64);
    witness(&c, &id, b"original", "first").unwrap();
    for sql in [
        "UPDATE confirmation_source_witnesses SET recorded_at=recorded_at",
        "DELETE FROM confirmation_source_witnesses",
    ] {
        assert!(c.execute(sql, []).is_err());
    }
    for (bytes, audit) in [
        (b"changed!".as_slice(), "first"),
        (b"original".as_slice(), "changed"),
    ] {
        assert!(c
            .execute(
                "INSERT OR REPLACE INTO confirmation_source_witnesses VALUES (?1,8,?2,?3)",
                params![id, bytes, audit]
            )
            .is_err());
    }
    assert_eq!(
        c.execute(
            "INSERT OR IGNORE INTO confirmation_source_witnesses VALUES (?1,8,?2,'first')",
            params![id, b"original".as_slice()]
        )
        .unwrap(),
        0
    );
    assert!(c
        .execute(
            "INSERT OR IGNORE INTO confirmation_blobs VALUES (?1,'binding',8,?2,'first')",
            params![id, b"original".as_slice()]
        )
        .is_err());
    old_blob(&c, &binding(), None);
    let blob_id: String = c
        .query_row("SELECT blob_sha256 FROM confirmation_blobs", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(witness(&c, &blob_id, b"different", "first").is_err());
    let value: (Vec<u8>, String) = c
        .query_row(
            "SELECT content_bytes,recorded_at FROM confirmation_source_witnesses",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(value, (b"original".to_vec(), "first".into()));
}

#[test]
fn witness_sql_domains_and_exact_individual_utf8_byte_bound_are_closed() {
    let c = fresh();
    let id = "2".repeat(64);
    for id in [
        "F".repeat(64),
        "g".repeat(64),
        "a".repeat(63),
        "a".repeat(65),
        format!("{}\0", "a".repeat(64)),
    ] {
        assert!(witness(&c, &id, b"x", "first").is_err());
    }
    for (n, value) in [
        (0i64, rusqlite::types::Value::Blob(vec![])),
        (2, rusqlite::types::Value::Blob(vec![1])),
        (1, rusqlite::types::Value::Text("x".into())),
    ] {
        assert!(c
            .execute(
                "INSERT INTO confirmation_source_witnesses VALUES (?1,?2,?3,'first')",
                params![id, n, value]
            )
            .is_err());
    }
    let bytes = "中".repeat(2_796_202).into_bytes(); // 8 MiB minus two UTF-8 bytes
    let mut exact = bytes;
    exact.extend_from_slice(b"xx");
    assert_eq!(exact.len(), 8 * 1024 * 1024);
    witness(&c, &id, &exact, "first").unwrap();
    exact.push(b'x');
    assert!(witness(&c, &"3".repeat(64), &exact, "first").is_err());
}

#[test]
fn witness_rows_share_joint_capacity_on_every_old_and_new_insertion_path() {
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
    {
        let tx = c.transaction().unwrap();
        for n in 0..9999 {
            tx.execute(
                "INSERT INTO confirmation_conflicts VALUES (?1,'{}','old')",
                [format!("{n:064x}")],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    c.execute_batch(&trigger).unwrap();
    witness(&c, &"f".repeat(64), b"x", "first").unwrap();
    assert!(witness(&c, &"e".repeat(64), b"x", "first").is_err());
    // Use the independent old SQL-shape fixtures; capacity must be the first
    // failure, regardless of their deliberately absent deferred FK targets.
    for (table, cols, values) in rows()
        .into_iter()
        .filter(|(t, _, _)| !["confirmation_intervals", "confirmation_unknown_origins"].contains(t))
    {
        let error = insert(&c, table, &cols, &values, "INSERT")
            .unwrap_err()
            .to_string();
        assert!(error.contains("capacity exceeded"), "{table}: {error}");
    }
    assert_eq!(
        c.query_row(
            "SELECT COUNT(*) FROM confirmation_source_witnesses",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn witness_bytes_share_the_exact_joint_budget_in_both_directions() {
    let c = fresh();
    let bytes = vec![b'x'; 8 * 1024 * 1024];
    for n in 0..8 {
        witness(&c, &format!("{n:064x}"), &bytes, "first").unwrap();
    }
    for (table, cols, values) in rows()
        .into_iter()
        .filter(|(t, _, _)| !["confirmation_intervals", "confirmation_unknown_origins"].contains(t))
    {
        let error = insert(&c, table, &cols, &values, "INSERT")
            .unwrap_err()
            .to_string();
        assert!(error.contains("capacity exceeded"), "{table}: {error}");
    }
    assert!(witness(&c, &"f".repeat(64), b"x", "first").is_err());
    let c = fresh();
    let document = " ".repeat(MAX_DOCUMENT_BYTES);
    for n in 0..16 {
        c.execute(
            "INSERT INTO confirmation_conflicts VALUES (?1,?2,'old')",
            params![format!("{n:064x}"), document],
        )
        .unwrap();
    }
    assert!(witness(&c, &"f".repeat(64), b"x", "first").is_err());
}

#[test]
fn migration_keeps_original_blobs_birth_and_bounded_original_binding_origins() {
    let mut c = old_six();
    let raw = binding();
    old_blob(&c, &raw, None);
    // Valid original binding bytes only: this orphan is NOT claimed to be a
    // checked graph or source proof. Even such origins must not be omitted.
    let original =
        alpha_factor_forge::discovery_core::confirmation_history::parse_binding_snapshot(&raw)
            .unwrap()
            .registry_id()
            .to_string();
    migrate(&mut c).unwrap();
    assert!(applied(&c));
    assert_eq!(
        unknown_origins(&c).unwrap(),
        BTreeSet::from(["a".repeat(32), original])
    );
    assert_eq!(
        c.query_row(
            "SELECT content_bytes,recorded_at FROM confirmation_blobs",
            [],
            |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))
        )
        .unwrap(),
        (raw.into_bytes(), "old-audit".into())
    );
    assert_eq!(
        c.query_row(
            "SELECT value FROM registry_meta WHERE key='confirmation_history_birth'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "0006_confirmation_history"
    );
}

#[test]
fn original_binding_corruption_and_oversize_roll_back_the_entire_seventh_migration() {
    for kind in 0..3 {
        let mut c = old_six();
        let mut raw = binding();
        match kind {
            0 => old_blob(&c, &raw, Some(&"f".repeat(64))),
            1 => {
                raw = raw.replacen("\"seq\":4", "\"seq\":4,\"seq\":4", 1);
                old_blob(&c, &raw, None);
            }
            _ => {
                c.execute_batch("PRAGMA ignore_check_constraints=ON")
                    .unwrap();
                let bytes = vec![b'x'; 8 * 1024 * 1024 + 1];
                c.execute(
                    "INSERT INTO confirmation_blobs VALUES (?1,'binding',?2,?3,'old-audit')",
                    params!["f".repeat(64), bytes.len() as i64, bytes],
                )
                .unwrap();
                c.execute_batch("PRAGMA ignore_check_constraints=OFF")
                    .unwrap();
            }
        }
        let error = migrate(&mut c).unwrap_err();
        assert!(!error.to_string().contains(&raw));
        assert!(!applied(&c));
        assert!(!table_exists(&c));
        assert!(unknown_origins(&c).unwrap().is_empty());
    }
}

#[test]
fn witness_rows_force_legacy_export_and_old_graph_reads_to_refuse_even_orphan_bytes() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    witness(
        &ledger.lock().unwrap(),
        &"f".repeat(64),
        b"not a valid witness",
        "first",
    )
    .unwrap();
    for result in [
        ledger.export_json_lines(),
        ledger.export_complete_json_lines(),
    ] {
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("complete v5 export"));
    }
    let error = ledger
        .read_confirmation_history(
            &alpha_factor_forge::discovery_core::confirmation_history::GraphContext::default(),
        )
        .unwrap_err();
    assert!(error.to_string().contains("checked schema-7 context"));
    drop(ledger);
    let reopened = dirs.open();
    assert!(reopened
        .read_confirmation_history(&Default::default())
        .is_err());
}

#[test]
fn empty_fresh_and_original_manual_only_migrations_add_no_managed_assertions() {
    use alpha_factor_forge::discovery_core::confirmation_history::freeze_manual_exposure;
    let mut c = old_six();
    let exposure=freeze_manual_exposure(&serde_json::json!({"contractVersion":"research-confirmation-exposure-v1","originRegistryId":"b".repeat(32),"reservationKey":"c".repeat(32),"instrumentId":"crypto:binance:BTCUSDT","intervals":[{"fromMs":946684800000u64,"toExclusiveMs":946684800001u64}],"claim":{"kind":"manual","accessKind":"raw","afterSealId":null}}).to_string()).unwrap();
    c.execute("INSERT INTO confirmation_exposures VALUES (?1,'crypto:binance:BTCUSDT','manual',NULL,NULL,?2,'manual-audit')", params![exposure.id(),exposure.document().to_string()]).unwrap();
    c.execute("INSERT INTO confirmation_intervals VALUES (?1,0,'crypto:binance:BTCUSDT',946684800000,946684800001)",[exposure.id()]).unwrap();
    migrate(&mut c).unwrap();
    assert!(unknown_origins(&c).unwrap().is_empty());
    assert_eq!(
        c.query_row("SELECT recorded_at FROM confirmation_exposures", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "manual-audit"
    );
    let mut empty = old_six();
    migrate(&mut empty).unwrap();
    assert!(unknown_origins(&empty).unwrap().is_empty());
}
