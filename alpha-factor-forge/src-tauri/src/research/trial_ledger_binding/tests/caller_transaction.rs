use super::*;
use std::sync::mpsc;
use std::time::Duration;

fn known(request: &str) -> TrialBatchInput {
    let mut input = batch(request);
    input.instrument_id = Some("crypto:binance:BTCUSDT".into());
    input.events[0].snapshot_id = Some("snapshot".into());
    input.tests_per_trial = 2;
    input
}

fn registered(ledger: &TrialLedger, request: &str) -> AdmissionSnapshot {
    let result = ledger.register_batch(&known(request)).unwrap();
    match result.admission {
        Admission::Count(count) => count.snapshot(),
        other => panic!("known family: {other:?}"),
    }
}

// Small real fixtures: compare every persisted value, including raw audits,
// checkpoints, row IDs and sqlite_sequence, rather than just aggregate counts.
type RowSnapshot = Vec<(String, Vec<Vec<rusqlite::types::Value>>)>;

fn rows(conn: &Connection) -> RowSnapshot {
    let tables = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    tables
        .into_iter()
        .map(|table| {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT rowid,* FROM \"{}\" ORDER BY rowid",
                    table.replace('"', "\"\"")
                ))
                .unwrap();
            let columns = stmt.column_count();
            let values = stmt
                .query_map([], |r| {
                    (0..columns)
                        .map(|i| r.get(i))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            (table, values)
        })
        .collect()
}

fn changes(conn: &Connection) -> u64 {
    conn.query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap()
}

fn same<T: std::fmt::Debug + PartialEq>(
    actual: Result<T, LedgerError>,
    expected: Result<T, LedgerError>,
) {
    match (actual, expected) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
        (Err(actual), Err(expected)) => assert_eq!(actual.code(), expected.code()),
        other => panic!("public/helper mismatch: {other:?}"),
    }
}

fn parity(ledger: &TrialLedger, saved: Option<&LedgerBinding>, earlier: &AdmissionSnapshot) {
    let binding = ledger.check_binding(saved);
    let fence = ledger.fence_admission(&earlier.batch_id, earlier);
    let mut conn = ledger.lock().unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let before = rows(&tx);
    let writes = changes(&tx);
    same(ledger.check_binding_in(&tx, saved), binding);
    same(
        ledger.fence_admission_in(&tx, &earlier.batch_id, earlier),
        fence,
    );
    assert!(!tx.is_autocommit(), "only caller may end transaction");
    assert_eq!(changes(&tx), writes);
    assert_eq!(rows(&tx), before);
    tx.rollback().unwrap();
}

#[test]
fn public_parity_current_growth_protocol_raise_and_inconsistent_comparison() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "one");
    let saved = current(&ledger, None);
    parity(&ledger, Some(&saved), &earlier);
    assert!(matches!(
        ledger.fence_admission(&earlier.batch_id, &earlier).unwrap(),
        AdmissionFence::Unchanged(_)
    ));
    registered(&ledger, "two");
    let mut raised = known("raised");
    raised.tests_per_trial = 4;
    ledger.register_batch(&raised).unwrap();
    parity(&ledger, Some(&saved), &earlier);
    let AdmissionFence::Grew(count) = ledger.fence_admission(&earlier.batch_id, &earlier).unwrap()
    else {
        panic!("actual SQL family/protocol grew");
    };
    assert_eq!(count.family_effective_trials(), 3);
    assert_eq!(count.tests_per_trial(), 4);
    let mut inflated = earlier.clone();
    inflated.family_effective_trials = 100;
    parity(&ledger, Some(&saved), &inflated);
    assert_eq!(
        ledger
            .fence_admission(&earlier.batch_id, &inflated)
            .unwrap(),
        AdmissionFence::Blocked(FenceBlocked::Inconsistent)
    );
}

#[test]
fn replacement_prefix_proofs_and_same_registry_rollback_or_divergence_keep_parity() {
    let source_dirs = Dirs::new();
    let source = source_dirs.open();
    let earlier = registered(&source, "source");
    let saved = current(&source, None);
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    parity(&target, Some(&saved), &earlier);
    assert_eq!(
        target.check_binding(Some(&saved)).unwrap(),
        BindingCheck::RegistryReplaced
    );
    target
        .import_json_lines(&source.export_json_lines().unwrap())
        .unwrap();
    parity(&target, Some(&saved), &earlier);
    assert!(matches!(
        target.fence_admission(&earlier.batch_id, &earlier).unwrap(),
        AdmissionFence::Unchanged(_)
    ));
    let mut future = current(&target, None);
    future.seq += 1;
    let mut future_count = earlier.clone();
    future_count.registry_id = future.registry_id.clone();
    future_count.seq = future.seq;
    future_count.chain_head = future.chain_head.clone();
    parity(&target, Some(&future), &future_count);
    assert_eq!(
        target.check_binding(Some(&future)).unwrap(),
        BindingCheck::RegistryRolledBack
    );
    future.seq -= 1;
    future.chain_head = "f".repeat(64);
    future_count.seq = future.seq;
    future_count.chain_head = future.chain_head.clone();
    parity(&target, Some(&future), &future_count);
    assert_eq!(
        target.check_binding(Some(&future)).unwrap(),
        BindingCheck::RegistryDiverged
    );
}

#[test]
fn caller_pending_import_is_visible_only_inside_and_complete_rollback_restores_rows() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "local");
    let saved = current(&ledger, None);
    let observer = dirs.open();
    let source_dirs = Dirs::new();
    let source = source_dirs.open();
    registered(&source, "foreign");
    let original = source.export_json_lines().unwrap();
    let validated = super::super::super::transfer::ValidatedExport::parse(&original).unwrap();
    let mut conn = ledger.lock().unwrap();
    let baseline = rows(&conn);
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    ledger.import_trial_in(&tx, validated).unwrap();
    let pending = rows(&tx);
    assert_ne!(pending, baseline);
    let writes = changes(&tx);
    let BindingCheck::Current(binding) = ledger.check_binding_in(&tx, Some(&saved)).unwrap() else {
        panic!("pending union is consistent");
    };
    assert_eq!(binding.seq, saved.seq + 1);
    let growth = ledger
        .fence_admission_in(&tx, &earlier.batch_id, &earlier)
        .unwrap();
    assert!(
        matches!(&growth, AdmissionFence::Grew(_)),
        "pending import must report growth"
    );
    let AdmissionFence::Grew(count) = growth else {
        panic!("pending import must enlarge actual SQL count");
    };
    assert_eq!(count.family_effective_trials(), 2);
    assert_eq!(current(&observer, Some(&saved)), saved);
    assert_eq!(rows(&observer.lock().unwrap()), baseline);
    assert!(matches!(
        observer
            .fence_admission(&earlier.batch_id, &earlier)
            .unwrap(),
        AdmissionFence::Unchanged(_)
    ));
    assert!(!tx.is_autocommit());
    assert_eq!(changes(&tx), writes);
    assert_eq!(rows(&tx), pending);
    tx.rollback().unwrap();
    assert_eq!(rows(&conn), baseline);
    drop(conn);
    assert_eq!(current(&ledger, None), saved);
}

#[test]
fn borrowed_readers_never_reacquire_the_already_held_receiver_mutex() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "held");
    let saved = current(&ledger, None);
    let connection = Connection::open(dirs.registry.join(REGISTRY_FILE_NAME)).unwrap();
    std::thread::scope(|scope| {
        let held = ledger.lock().unwrap();
        let (send, receive) = mpsc::channel();
        let ledger = &ledger;
        let saved = &saved;
        let earlier = &earlier;
        let worker = scope.spawn(move || {
            let mut connection = connection;
            let tx = connection.transaction().unwrap();
            let binding = ledger.check_binding_in(&tx, Some(saved));
            let fence = ledger.fence_admission_in(&tx, &earlier.batch_id, earlier);
            send.send(
                matches!(binding, Ok(BindingCheck::Current(_)))
                    && matches!(fence, Ok(AdmissionFence::Unchanged(_)))
                    && !tx.is_autocommit(),
            )
            .unwrap();
            tx.rollback().unwrap();
        });
        let timely = receive.recv_timeout(Duration::from_secs(10));
        // Always unblock a nested-lock mutant before joining or asserting.
        drop(held);
        worker.join().unwrap();
        assert!(timely.is_ok(), "readers must not lock receiver");
        assert!(timely.unwrap(), "borrowed observations must succeed");
    });
}

fn writer_busy(writer: &Connection) {
    let error = writer.execute_batch("BEGIN IMMEDIATE").unwrap_err();
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy)
    );
}

#[test]
fn success_refusal_and_error_leave_immediate_reservation_with_the_caller() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "reservation");
    let saved = current(&ledger, None);
    let writer = Connection::open(dirs.registry.join(REGISTRY_FILE_NAME)).unwrap();
    writer.busy_timeout(Duration::ZERO).unwrap();
    let mut conn = ledger.lock().unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let before = rows(&tx);
    let writes = changes(&tx);
    writer_busy(&writer);
    assert!(matches!(
        ledger.check_binding_in(&tx, Some(&saved)).unwrap(),
        BindingCheck::Current(_)
    ));
    assert!(!tx.is_autocommit(), "caller owns transaction lifetime");
    assert!(matches!(
        ledger
            .fence_admission_in(&tx, &earlier.batch_id, &earlier)
            .unwrap(),
        AdmissionFence::Unchanged(_)
    ));
    let mut bad = earlier.clone();
    bad.chain_head = "malformed".into();
    let mut bad_binding = saved.clone();
    bad_binding.chain_head = "malformed".into();
    assert_eq!(
        ledger
            .check_binding_in(&tx, Some(&bad_binding))
            .unwrap_err()
            .code(),
        "registry_state_invalid"
    );
    assert_eq!(
        ledger
            .fence_admission_in(&tx, &earlier.batch_id, &bad)
            .unwrap_err()
            .code(),
        "registry_state_invalid"
    );
    let mut future = saved.clone();
    future.seq += 1;
    assert_eq!(
        ledger.check_binding_in(&tx, Some(&future)).unwrap(),
        BindingCheck::RegistryRolledBack
    );
    let mut future_count = earlier.clone();
    future_count.seq += 1;
    assert_eq!(
        ledger
            .fence_admission_in(&tx, &earlier.batch_id, &future_count)
            .unwrap(),
        AdmissionFence::Blocked(FenceBlocked::Prefix(BindingCheck::RegistryRolledBack))
    );
    assert_eq!(
        ledger
            .fence_admission_in(&tx, "missing", &earlier)
            .unwrap_err()
            .code(),
        "batch_not_found"
    );
    let mut inflated = earlier.clone();
    inflated.family_effective_trials += 1;
    assert_eq!(
        ledger
            .fence_admission_in(&tx, &earlier.batch_id, &inflated)
            .unwrap(),
        AdmissionFence::Blocked(FenceBlocked::Inconsistent)
    );
    writer_busy(&writer);
    assert!(!tx.is_autocommit());
    assert_eq!(changes(&tx), writes);
    assert_eq!(rows(&tx), before);
    tx.commit().unwrap();
    writer.execute_batch("BEGIN IMMEDIATE; ROLLBACK").unwrap();
    assert_eq!(rows(&conn), before);
}

#[test]
fn inactive_and_foreign_registry_transactions_refuse_without_writing() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "active");
    let other_dirs = Dirs::new();
    let other = other_dirs.open();
    let mut other_conn = other.lock().unwrap();
    let tx = other_conn.transaction().unwrap();
    let before = rows(&tx);
    let writes = changes(&tx);
    assert!(
        ledger.check_binding_in(&tx, None).is_err(),
        "foreign transaction must refuse"
    );
    assert_eq!(
        ledger.check_binding_in(&tx, None).unwrap_err().code(),
        "registry_state_invalid"
    );
    let foreign_fence = ledger.fence_admission_in(&tx, &earlier.batch_id, &earlier);
    assert!(
        matches!(&foreign_fence, Err(error) if error.code() == "registry_state_invalid"),
        "foreign fence transaction must refuse before prefix comparison"
    );
    assert!(!tx.is_autocommit());
    assert_eq!(changes(&tx), writes);
    assert_eq!(rows(&tx), before);
    tx.rollback().unwrap();
    let mut conn = ledger.lock().unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute_batch("ROLLBACK").unwrap();
    let before = rows(&tx);
    let writes = changes(&tx);
    assert_eq!(
        ledger.check_binding_in(&tx, None).unwrap_err().code(),
        "registry_state_invalid"
    );
    assert_eq!(
        ledger
            .fence_admission_in(&tx, &earlier.batch_id, &earlier)
            .unwrap_err()
            .code(),
        "registry_state_invalid"
    );
    assert!(tx.is_autocommit());
    assert_eq!(changes(&tx), writes);
    assert_eq!(rows(&tx), before);
}

#[test]
fn a_deferred_snapshot_remains_old_while_fresh_public_reads_observe_growth() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "old");
    let saved = current(&ledger, None);
    let writer = dirs.open();
    let mut conn = ledger.lock().unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .unwrap();
    assert_eq!(
        ledger.check_binding_in(&tx, None).unwrap(),
        BindingCheck::Current(saved.clone())
    );
    registered(&writer, "new");
    assert_eq!(
        ledger.check_binding_in(&tx, Some(&saved)).unwrap(),
        BindingCheck::Current(saved.clone())
    );
    assert!(matches!(
        ledger
            .fence_admission_in(&tx, &earlier.batch_id, &earlier)
            .unwrap(),
        AdmissionFence::Unchanged(_)
    ));
    assert!(!tx.is_autocommit());
    tx.rollback().unwrap();
    drop(conn);
    assert_eq!(current(&ledger, Some(&saved)).seq, saved.seq + 1);
    assert!(matches!(
        ledger.fence_admission(&earlier.batch_id, &earlier).unwrap(),
        AdmissionFence::Grew(_)
    ));
}

#[test]
fn malformed_public_evidence_still_refuses_before_receiver_mutex() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let mut earlier = registered(&ledger, "bad");
    let mut saved = current(&ledger, None);
    saved.chain_head = "bad".into();
    earlier.chain_head = "bad".into();
    std::thread::scope(|scope| {
        let held = ledger.lock().unwrap();
        let (send, receive) = mpsc::channel();
        let ledger = &ledger;
        let saved = &saved;
        let earlier = &earlier;
        let worker = scope.spawn(move || {
            send.send((
                ledger.check_binding(Some(saved)).unwrap_err().code(),
                ledger
                    .fence_admission(&earlier.batch_id, earlier)
                    .unwrap_err()
                    .code(),
            ))
            .unwrap();
        });
        let timely = receive.recv_timeout(Duration::from_secs(10));
        drop(held);
        worker.join().unwrap();
        assert_eq!(
            timely.unwrap(),
            ("registry_state_invalid", "registry_state_invalid")
        );
    });
}

fn manual(ledger: &TrialLedger, key: char) -> String {
    alpha_factor_forge::discovery_core::confirmation_history::freeze_manual_exposure(
        &serde_json::json!({
            "contractVersion":"research-confirmation-exposure-v1",
            "originRegistryId":ledger.registry_id(),
            "reservationKey":key.to_string().repeat(32),
            "instrumentId":"crypto:binance:BTCUSDT",
            "intervals":[{"fromMs":946684800001u64,"toExclusiveMs":946684800002u64}],
            "claim":{"kind":"manual","accessKind":"chart","afterSealId":null}
        })
        .to_string(),
    )
    .unwrap()
    .document()
    .to_string()
}

#[test]
fn missing_and_substituted_confirmation_identity_block_binding_but_not_narrow_count_fence() {
    use alpha_factor_forge::discovery_core::confirmation_history::ConfirmationInventory;
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "identities");
    ledger
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            exposures: vec![manual(&ledger, 'a')],
            ..Default::default()
        })
        .unwrap();
    let saved = current(&ledger, None);
    ledger.lock().unwrap().execute_batch("DROP TRIGGER confirmation_intervals_no_delete; DROP TRIGGER confirmation_exposures_no_delete; DELETE FROM confirmation_intervals; DELETE FROM confirmation_exposures").unwrap();
    for replacement in [false, true] {
        if replacement {
            ledger
                .append_witnessed_confirmation_history(&ConfirmationInventory {
                    exposures: vec![manual(&ledger, 'b')],
                    ..Default::default()
                })
                .unwrap();
        }
        parity(&ledger, Some(&saved), &earlier);
        assert_eq!(
            ledger.check_binding(Some(&saved)).unwrap(),
            BindingCheck::RegistryEvidenceRolledBack
        );
        assert!(matches!(
            ledger.fence_admission(&earlier.batch_id, &earlier).unwrap(),
            AdmissionFence::Unchanged(_)
        ));
    }
}

#[test]
fn alpha_provenance_loss_and_saved_protocol_high_water_remain_restrictive() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "provenance");
    ledger
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO alpha_unknown_origins VALUES (?1,'original-audit')",
            ["e".repeat(32)],
        )
        .unwrap();
    let saved = current(&ledger, None);
    ledger
        .lock()
        .unwrap()
        .execute_batch(
            "DROP TRIGGER alpha_unknown_origins_no_delete; DELETE FROM alpha_unknown_origins",
        )
        .unwrap();
    parity(&ledger, Some(&saved), &earlier);
    assert_eq!(
        ledger.check_binding(Some(&saved)).unwrap(),
        BindingCheck::RegistryEvidenceRolledBack
    );
    let mut high_water = current(&ledger, None);
    high_water
        .evidence
        .as_mut()
        .unwrap()
        .family_tests
        .insert(earlier.family_id.clone(), earlier.tests_per_trial + 1);
    parity(&ledger, Some(&high_water), &earlier);
    assert_eq!(
        ledger.check_binding(Some(&high_water)).unwrap(),
        BindingCheck::RegistryEvidenceRolledBack
    );
}

#[test]
fn chain_damage_after_open_refuses_in_both_paths_without_repair() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "chain");
    let saved = current(&ledger, None);
    ledger.lock().unwrap().execute_batch("DROP TRIGGER trial_events_no_update; UPDATE trial_events SET chain='ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'").unwrap();
    parity(&ledger, Some(&saved), &earlier);
    assert_eq!(
        ledger.check_binding(Some(&saved)).unwrap(),
        BindingCheck::RegistryChainBroken
    );
    assert_eq!(
        ledger.fence_admission(&earlier.batch_id, &earlier).unwrap(),
        AdmissionFence::Blocked(FenceBlocked::Prefix(BindingCheck::RegistryChainBroken))
    );
}

#[test]
fn missing_registry_metadata_is_new_helper_precondition_not_public_fence_reordering() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "meta");
    ledger.lock().unwrap().execute_batch("DROP TRIGGER registry_meta_no_delete; DELETE FROM registry_meta WHERE key='registry_id'").unwrap();
    // Preserve the old narrow public fence even for this damaged combination.
    assert!(matches!(
        ledger.fence_admission(&earlier.batch_id, &earlier).unwrap(),
        AdmissionFence::Unchanged(_)
    ));
    let mut conn = ledger.lock().unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let before = rows(&tx);
    let writes = changes(&tx);
    assert_eq!(
        ledger.check_binding_in(&tx, None).unwrap_err().code(),
        "registry_state_invalid"
    );
    assert_eq!(
        ledger
            .fence_admission_in(&tx, &earlier.batch_id, &earlier)
            .unwrap_err()
            .code(),
        "registry_state_invalid"
    );
    assert!(!tx.is_autocommit());
    assert_eq!(changes(&tx), writes);
    assert_eq!(rows(&tx), before);
    tx.rollback().unwrap();
}

#[test]
fn sql_read_error_preserves_the_active_transaction_and_its_pending_schema() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let earlier = registered(&ledger, "read-error");
    let mut conn = ledger.lock().unwrap();
    let baseline = rows(&conn);
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    tx.execute_batch("ALTER TABLE alpha_unknown_origins RENAME TO test_unavailable_alpha_origins")
        .unwrap();
    let pending = rows(&tx);
    let writes = changes(&tx);
    assert_eq!(
        ledger.check_binding_in(&tx, None).unwrap_err().code(),
        "database_error"
    );
    assert!(matches!(
        ledger
            .fence_admission_in(&tx, &earlier.batch_id, &earlier)
            .unwrap(),
        AdmissionFence::Unchanged(_)
    ));
    assert!(!tx.is_autocommit());
    assert_eq!(changes(&tx), writes);
    assert_eq!(rows(&tx), pending);
    tx.rollback().unwrap();
    assert_eq!(rows(&conn), baseline);
}
