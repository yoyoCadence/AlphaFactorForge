//! Real legacy-writer records in canonical old schema-6 objects. No migration
//! downgrade API exists: this fixture conversion is compiled only for tests.
use super::*;
use crate::research::trial_ledger::{migrate, REGISTRY_MIGRATIONS};
use alpha_factor_forge::discovery_core::confirmation_history::freeze_manual_exposure;

fn schema_six_fixture(conn: &mut Connection) {
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM confirmation_source_witnesses",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let no_delete: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='registry_migrations_no_delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let tx = conn.transaction().unwrap();
    for table in [
        "confirmation_batches",
        "confirmation_selections",
        "confirmation_operations",
        "confirmation_exposures",
        "confirmation_admissions",
        "confirmation_events",
        "confirmation_blobs",
        "confirmation_conflicts",
    ] {
        tx.execute_batch(&format!("DROP TRIGGER {table}_capacity"))
            .unwrap();
    }
    tx.execute_batch("DROP TRIGGER confirmation_blobs_witness_namespace; DROP TABLE confirmation_source_witnesses; DROP TRIGGER registry_migrations_no_delete").unwrap();
    assert_eq!(
        tx.execute(
            "DELETE FROM registry_migrations WHERE version='0007_confirmation_source_witness'",
            []
        )
        .unwrap(),
        1
    );
    tx.execute_batch(&no_delete).unwrap();
    let sql = REGISTRY_MIGRATIONS[5].1;
    for table in [
        "confirmation_batches",
        "confirmation_selections",
        "confirmation_operations",
        "confirmation_exposures",
        "confirmation_admissions",
        "confirmation_events",
        "confirmation_blobs",
        "confirmation_conflicts",
    ] {
        let start = sql
            .find(&format!("CREATE TRIGGER {table}_capacity "))
            .unwrap();
        let statement = &sql[start..];
        let end = statement.find(" END;").unwrap() + 5;
        tx.execute_batch(&statement[..end]).unwrap();
    }
    tx.commit().unwrap();
}
fn objects(c: &Connection) -> Vec<(String, Option<String>)> {
    c.prepare("SELECT name,sql FROM sqlite_master ORDER BY name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}
fn old_rows(c: &Connection) -> Vec<(String, Vec<Vec<SqlValue>>)> {
    TABLES
        .into_iter()
        .filter(|t| {
            !matches!(
                *t,
                "confirmation_source_witnesses" | "confirmation_unknown_origins"
            )
        })
        .map(|table| {
            let mut stmt = c
                .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                .unwrap();
            let width = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..width).map(|n| r.get::<_, SqlValue>(n)).collect()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            (table.into(), rows)
        })
        .collect()
}
fn add_foreign_manual(input: &mut ConfirmationInventory) {
    let exposure=freeze_manual_exposure(&json!({"contractVersion":"research-confirmation-exposure-v1","originRegistryId":"e".repeat(32),"reservationKey":"d".repeat(32),
        "instrumentId":"crypto:binance:BTCUSDT","intervals":[{"fromMs":4102444799999u64,"toExclusiveMs":4102444800000u64}],"claim":{"kind":"manual","accessKind":"raw","afterSealId":null}}).to_string()).unwrap();
    input.exposures.push(exposure.document().to_string());
}

#[test]
fn actual_legacy_managed_records_keep_all_original_bytes_and_foreign_exposure_origins() {
    let (_dirs, ledger, mut input, ctx) = fixture();
    add_foreign_manual(&mut input);
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let mut conn = ledger.lock().unwrap();
    let before = old_rows(&conn);
    schema_six_fixture(&mut conn);
    assert!(unknown_origins(&conn).unwrap().is_empty());
    migrate(&mut conn).unwrap();
    assert_eq!(old_rows(&conn), before);
    assert_eq!(
        unknown_origins(&conn).unwrap(),
        BTreeSet::from([ledger.registry_id().into(), "e".repeat(32)])
    );
    drop(conn);
    assert!(ledger
        .read_confirmation_history(&ctx)
        .unwrap()
        .is_restricted());
}

#[test]
fn late_migration_receipt_failure_rolls_back_ddl_capacity_guards_and_new_unknowns() {
    let (_dirs, ledger, mut input, ctx) = fixture();
    add_foreign_manual(&mut input);
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let mut conn = ledger.lock().unwrap();
    schema_six_fixture(&mut conn);
    conn.execute_batch("CREATE TRIGGER fail_seventh_receipt BEFORE INSERT ON registry_migrations WHEN NEW.version='0007_confirmation_source_witness' BEGIN SELECT RAISE(ABORT,'injected receipt failure'); END;").unwrap();
    let schema = objects(&conn);
    let data = old_rows(&conn);
    assert!(migrate(&mut conn).is_err());
    assert_eq!(objects(&conn), schema);
    assert_eq!(old_rows(&conn), data);
    assert!(unknown_origins(&conn).unwrap().is_empty());
    conn.execute_batch("DROP TRIGGER fail_seventh_receipt")
        .unwrap();
    migrate(&mut conn).unwrap();
    assert!(unknown_origins(&conn).unwrap().contains(&"e".repeat(32)));
}

#[test]
fn exact_unknown_capacity_and_corrupt_projection_fail_before_schema_seven_is_applied() {
    for corrupt in [false, true] {
        let (_dirs, ledger, input, ctx) = fixture();
        ledger.append_confirmation_history(&input, &ctx).unwrap();
        let mut conn = ledger.lock().unwrap();
        schema_six_fixture(&mut conn);
        if corrupt {
            let guard: String = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name='confirmation_operations_no_update'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            conn.execute_batch("DROP TRIGGER confirmation_operations_no_update")
                .unwrap();
            conn.execute(
                "UPDATE confirmation_operations SET origin_registry_id=?1",
                ["f".repeat(32)],
            )
            .unwrap();
            conn.execute_batch(&guard).unwrap();
        } else {
            let prefix = if ledger.registry_id().starts_with('a') {
                'b'
            } else {
                'a'
            };
            let tx = conn.transaction().unwrap();
            for n in 0..10000 {
                tx.execute(
                    "INSERT INTO confirmation_unknown_origins VALUES (?1,'existing-audit')",
                    [format!("{prefix}{n:031x}")],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        }
        let schema = objects(&conn);
        let data = old_rows(&conn);
        let unknown = unknown_origins(&conn).unwrap();
        assert!(migrate(&mut conn).is_err());
        assert_eq!(objects(&conn), schema);
        assert_eq!(old_rows(&conn), data);
        assert_eq!(unknown_origins(&conn).unwrap(), unknown);
    }
}
