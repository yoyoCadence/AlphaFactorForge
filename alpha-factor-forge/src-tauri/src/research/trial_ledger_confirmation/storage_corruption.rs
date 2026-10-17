//! Column expectations come from the merged SQL contract, independently of
//! the production projection descriptors (omitting a descriptor must fail).
use super::cases::{fork, raw_connection, snapshot};
use super::*;

#[test]
fn every_document_scalar_column_is_checked_without_relying_on_row_counts() {
    let (dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    ledger
        .append_confirmation_history(&fork(&input, &ctx, 'f'), &ctx)
        .unwrap();
    let saved = ledger.read_confirmation_history(&ctx).unwrap();
    let counts = snapshot(&ledger);
    let c = raw_connection(&dirs);
    c.execute_batch("PRAGMA foreign_keys=OFF;PRAGMA ignore_check_constraints=ON")
        .unwrap();
    for (table, columns) in [
        (
            "confirmation_batches",
            &[
                "batch_id",
                "family_id",
                "stage",
                "parent_batch_id",
                "selection_id",
            ][..],
        ),
        (
            "confirmation_selections",
            &[
                "selection_id",
                "validation_batch_id",
                "sealed_receipt_id",
                "alpha_reservation_id",
                "report_sha256",
            ],
        ),
        (
            "confirmation_operations",
            &[
                "operation_id",
                "origin_registry_id",
                "reservation_key",
                "workspace_id",
                "batch_id",
                "family_id",
            ],
        ),
        (
            "confirmation_exposures",
            &[
                "exposure_id",
                "instrument_id",
                "kind",
                "operation_id",
                "alpha_reservation_id",
            ],
        ),
        (
            "confirmation_admissions",
            &[
                "admission_id",
                "operation_id",
                "alpha_reservation_id",
                "exposure_id",
                "binding_sha256",
            ],
        ),
        (
            "confirmation_events",
            &["event_id", "operation_id", "admission_id", "kind"],
        ),
        ("confirmation_conflicts", &["conflict_id"]),
    ] {
        c.execute_batch(&format!("DROP TRIGGER {table}_no_update"))
            .unwrap();
        for column in columns {
            let (rowid, old): (i64, SqlValue) = c
                .query_row(
                    &format!("SELECT rowid,{column} FROM {table} ORDER BY rowid LIMIT 1"),
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            let wrong = match *column {
                "stage" | "kind" => SqlValue::Text("future".into()),
                "instrument_id" => SqlValue::Text("crypto:binance:ETHUSDT".into()),
                "family_id" => SqlValue::Text(format!("trial-family-v1:{}", "f".repeat(64))),
                "origin_registry_id" | "reservation_key" | "workspace_id" => {
                    SqlValue::Text("f".repeat(32))
                }
                _ => SqlValue::Text("f".repeat(64)),
            };
            assert_ne!(wrong, old);
            c.execute(
                &format!("UPDATE {table} SET {column}=?1 WHERE rowid=?2"),
                params![wrong, rowid],
            )
            .unwrap();
            assert!(
                ledger.read_confirmation_history(&ctx).is_err(),
                "{table}.{column}"
            );
            assert_eq!(snapshot(&ledger), counts);
            c.execute(
                &format!("UPDATE {table} SET {column}=?1 WHERE rowid=?2"),
                params![old, rowid],
            )
            .unwrap();
        }
    }
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        saved.inventory()
    );
}

#[test]
fn every_interval_column_and_equal_count_key_substitution_are_checked() {
    let (dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let saved = ledger.read_confirmation_history(&ctx).unwrap();
    let before = snapshot(&ledger);
    let c = raw_connection(&dirs);
    c.execute_batch("PRAGMA foreign_keys=OFF;PRAGMA ignore_check_constraints=ON;DROP TRIGGER confirmation_intervals_no_update").unwrap();
    for column in [
        "exposure_id",
        "interval_index",
        "instrument_id",
        "from_ms",
        "to_exclusive_ms",
    ] {
        let (rowid, old): (i64, SqlValue) = c
            .query_row(
                &format!(
                    "SELECT rowid,{column} FROM confirmation_intervals ORDER BY rowid LIMIT 1"
                ),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let wrong = match &old {
            SqlValue::Integer(n) => SqlValue::Integer(n + 1),
            _ if column == "instrument_id" => SqlValue::Text("crypto:binance:ETHUSDT".into()),
            _ => SqlValue::Text("f".repeat(64)),
        };
        c.execute(
            &format!("UPDATE confirmation_intervals SET {column}=?1 WHERE rowid=?2"),
            params![wrong, rowid],
        )
        .unwrap();
        assert!(ledger.read_confirmation_history(&ctx).is_err(), "{column}");
        assert_eq!(snapshot(&ledger), before);
        c.execute(
            &format!("UPDATE confirmation_intervals SET {column}=?1 WHERE rowid=?2"),
            params![old, rowid],
        )
        .unwrap();
    }
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        saved.inventory()
    );
}

#[test]
fn blob_kind_sha_length_and_original_display_bytes_cannot_be_rewritten() {
    let (dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let saved = ledger.read_confirmation_history(&ctx).unwrap();
    let c = raw_connection(&dirs);
    c.execute_batch("PRAGMA foreign_keys=OFF;PRAGMA ignore_check_constraints=ON;DROP TRIGGER confirmation_blobs_no_update").unwrap();
    for kind in ["statistics", "binding"] {
        for column in ["blob_sha256", "kind", "byte_length", "content_bytes"] {
            let (rowid,old):(i64,SqlValue)=c.query_row(&format!("SELECT rowid,{column} FROM confirmation_blobs WHERE kind=?1 ORDER BY rowid LIMIT 1"),[kind],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
            let wrong = match (column, &old) {
                ("blob_sha256", _) => SqlValue::Text("f".repeat(64)),
                ("kind", _) => SqlValue::Text(
                    if kind == "binding" {
                        "statistics"
                    } else {
                        "binding"
                    }
                    .into(),
                ),
                ("byte_length", SqlValue::Integer(n)) => SqlValue::Integer(n + 1),
                (_, SqlValue::Blob(bytes)) => {
                    let mut bytes = bytes.clone();
                    let at = bytes.iter().position(|b| *b == b'\n').unwrap();
                    bytes[at] = b' ';
                    SqlValue::Blob(bytes)
                }
                _ => panic!("fixture blob shape"),
            };
            c.execute(
                &format!("UPDATE confirmation_blobs SET {column}=?1 WHERE rowid=?2"),
                params![wrong, rowid],
            )
            .unwrap();
            assert!(
                ledger.read_confirmation_history(&ctx).is_err(),
                "{kind}.{column}"
            );
            c.execute(
                &format!("UPDATE confirmation_blobs SET {column}=?1 WHERE rowid=?2"),
                params![old, rowid],
            )
            .unwrap();
        }
    }
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        saved.inventory()
    );
}

#[test]
fn malformed_json_audit_type_and_unknown_origin_fail_with_static_diagnostics() {
    let (dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    ledger
        .append_confirmation_history(
            &ConfirmationInventory {
                unknown_origins: vec!["e".repeat(32)],
                ..ConfirmationInventory::default()
            },
            &ctx,
        )
        .unwrap();
    let saved = ledger.read_confirmation_history(&ctx).unwrap();
    let c = raw_connection(&dirs);
    c.execute_batch("PRAGMA ignore_check_constraints=ON;DROP TRIGGER confirmation_operations_no_update;DROP TRIGGER confirmation_unknown_origins_no_update").unwrap();
    for (table, column, wrong) in [
        (
            "confirmation_operations",
            "document_json",
            SqlValue::Text("{PRIVATE_PROTECTED_SENTINEL".into()),
        ),
        (
            "confirmation_operations",
            "recorded_at",
            SqlValue::Blob(b"private".to_vec()),
        ),
        (
            "confirmation_unknown_origins",
            "origin_registry_id",
            SqlValue::Text("PRIVATE_PROTECTED_ORIGIN_SENTINEL".into()),
        ),
    ] {
        let (rowid, old): (i64, SqlValue) = c
            .query_row(
                &format!("SELECT rowid,{column} FROM {table} ORDER BY rowid LIMIT 1"),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        c.execute(
            &format!("UPDATE {table} SET {column}=?1 WHERE rowid=?2"),
            params![wrong, rowid],
        )
        .unwrap();
        let error = ledger.read_confirmation_history(&ctx).unwrap_err();
        assert!(!error.to_string().contains("PRIVATE"));
        c.execute(
            &format!("UPDATE {table} SET {column}=?1 WHERE rowid=?2"),
            params![old, rowid],
        )
        .unwrap();
    }
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        saved.inventory()
    );
}

#[test]
fn missing_materialized_witness_is_not_silently_repaired_by_a_later_append() {
    let (dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    ledger
        .append_confirmation_history(&fork(&input, &ctx, 'f'), &ctx)
        .unwrap();
    let c = raw_connection(&dirs);
    c.execute_batch("DROP TRIGGER confirmation_conflicts_no_delete")
        .unwrap();
    let id: String = c
        .query_row(
            "SELECT conflict_id FROM confirmation_conflicts ORDER BY conflict_id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    c.execute(
        "DELETE FROM confirmation_conflicts WHERE conflict_id=?1",
        [id],
    )
    .unwrap();
    let before = snapshot(&ledger);
    assert!(ledger.read_confirmation_history(&ctx).is_err());
    assert!(ledger
        .append_confirmation_history(&ConfirmationInventory::default(), &ctx)
        .is_err());
    assert_eq!(snapshot(&ledger), before);
}

#[test]
fn original_operation_lookup_refuses_invalid_or_substituted_keys() {
    let (_dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    assert!(ledger
        .find_original_confirmation_operations("PRIVATE_KEY", &"4".repeat(32), &ctx)
        .is_err());
    assert!(ledger
        .find_original_confirmation_operations(ledger.registry_id(), &"0".repeat(32), &ctx)
        .unwrap()
        .operation_ids()
        .is_empty());
    let c = ledger.lock().unwrap();
    let count: i64 = c
        .query_row("SELECT COUNT(*) FROM confirmation_operations", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 2);
}
