use super::*;
use crate::research::trial_ledger::{
    family_id_for, TrialBatchInput, TrialEventInput, TrialKind, TrialOrigin,
};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Barrier,
};

struct Dirs {
    root: PathBuf,
    registry: PathBuf,
    workspace: PathBuf,
}
impl Dirs {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "aff-alpha-storage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let registry = root.join("registry");
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

fn budget() -> AlphaAllocationDeclaration {
    AlphaAllocationDeclaration {
        total_alpha_ppm: 50_000,
        schedule: vec![25_000, 25_000],
    }
}
fn register(ledger: &TrialLedger, instrument: &str, request: &str) -> String {
    ledger
        .register_batch(&TrialBatchInput {
            workspace_id: "alpha-test".into(),
            instrument_id: Some(instrument.into()),
            tests_per_trial: 2,
            events: vec![TrialEventInput {
                kind: TrialKind::Variant,
                origin: TrialOrigin::Request {
                    request_id: request.into(),
                    candidate_index: 0,
                },
                hypothesis_hash: Some("hypothesis".into()),
                strategy_hash: Some(format!("strategy-{request}")),
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
    family_id_for(instrument).unwrap()
}
fn setup() -> (Dirs, TrialLedger, String) {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let family = register(&ledger, "crypto:binance:BTCUSDT", "first");
    assert_eq!(
        ledger.declare_alpha_budget(&family, &budget()).unwrap(),
        AlphaStorageStatus::Stored
    );
    (dirs, ledger, family)
}
fn reserve(ledger: &TrialLedger, family: &str, key: char, batch: char) -> AlphaStorageOutcome {
    ledger
        .reserve_alpha_evidence(
            family,
            &key.to_string().repeat(32),
            &batch.to_string().repeat(64),
        )
        .unwrap()
}

#[test]
fn replay_keeps_original_number_after_intervening_reservation_and_reopen() {
    let (dirs, ledger, family) = setup();
    let first = reserve(&ledger, &family, 'a', 'c').reservation.unwrap();
    assert_eq!(first.confirmation_number(), 1);
    let second = reserve(&ledger, &family, 'b', 'd').reservation.unwrap();
    assert_eq!(second.confirmation_number(), 2);
    assert_eq!(
        ledger.declare_alpha_budget(&family, &budget()).unwrap(),
        AlphaStorageStatus::Replayed
    );
    let replay = reserve(&ledger, &family, 'a', 'c');
    assert_eq!(replay.status, AlphaStorageStatus::Replayed);
    assert_eq!(replay.reservation, Some(first.clone()));
    assert_eq!(
        reserve(&ledger, &family, 'e', 'f').status,
        AlphaStorageStatus::Exhausted
    );
    let before = ledger.read_alpha_family(&family).unwrap().unwrap();
    assert_eq!(before.reservations.len(), 2);
    assert_eq!(before.allocation.as_ref().unwrap().spent_alpha_ppm, 50_000);
    drop(ledger);
    let reopened = dirs.open();
    assert_eq!(
        reopened.read_alpha_family(&family).unwrap().unwrap(),
        before
    );
    assert_eq!(
        reserve(&reopened, &family, 'a', 'c').reservation,
        Some(first)
    );
}

#[test]
fn changed_budget_is_permanent_conflict_without_reset_or_trial_quarantine() {
    let (dirs, ledger, family) = setup();
    reserve(&ledger, &family, 'a', 'c');
    let changed = AlphaAllocationDeclaration {
        total_alpha_ppm: 50_000,
        schedule: vec![20_000, 30_000],
    };
    assert_eq!(
        ledger.declare_alpha_budget(&family, &changed).unwrap(),
        AlphaStorageStatus::Blocked
    );
    let state = ledger.read_alpha_family(&family).unwrap().unwrap();
    assert_eq!(state.declaration, budget());
    assert!(state.blocked);
    assert!(state.allocation.is_none());
    assert_eq!(
        state.persistent_conflicts[0]["documents"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        reserve(&ledger, &family, 'b', 'd').status,
        AlphaStorageStatus::Blocked
    );
    // An alpha-family stop must not prevent ordinary discovery registration.
    register(&ledger, "crypto:binance:BTCUSDT", "after-alpha-conflict");
    let other = register(&ledger, "crypto:binance:ETHUSDT", "other-family");
    ledger.declare_alpha_budget(&other, &budget()).unwrap();
    assert_eq!(
        reserve(&ledger, &other, 'f', 'c').status,
        AlphaStorageStatus::Stored
    );
    drop(ledger);
    assert!(
        dirs.open()
            .read_alpha_family(&family)
            .unwrap()
            .unwrap()
            .blocked
    );
}

#[test]
fn changed_operation_keeps_both_same_number_claims_even_after_exhaustion() {
    let (dirs, ledger, family) = setup();
    reserve(&ledger, &family, 'a', 'c');
    reserve(&ledger, &family, 'b', 'd');
    assert_eq!(
        reserve(&ledger, &family, 'a', 'e').status,
        AlphaStorageStatus::Blocked
    );
    let state = ledger.read_alpha_family(&family).unwrap().unwrap();
    assert!(state.blocked);
    assert!(state.allocation.is_none());
    assert_eq!(state.reservations.len(), 3);
    assert_eq!(
        state
            .reservations
            .iter()
            .filter(|r| r.confirmation_number() == 1)
            .count(),
        2
    );
    assert_eq!(state.persistent_conflicts.len(), 1);
    drop(ledger);
    assert!(
        dirs.open()
            .read_alpha_family(&family)
            .unwrap()
            .unwrap()
            .blocked
    );
}

#[test]
fn operation_reassignment_across_families_stops_both() {
    let (_dirs, ledger, first) = setup();
    let second = register(&ledger, "crypto:binance:ETHUSDT", "eth");
    ledger.declare_alpha_budget(&second, &budget()).unwrap();
    reserve(&ledger, &first, 'a', 'c');
    assert_eq!(
        reserve(&ledger, &second, 'a', 'c').status,
        AlphaStorageStatus::Blocked
    );
    for family in [&first, &second] {
        let state = ledger.read_alpha_family(family).unwrap().unwrap();
        assert!(state.blocked);
        assert!(state.allocation.is_none());
        assert_eq!(state.persistent_conflicts.len(), 1);
    }
}

#[test]
fn independent_same_number_records_are_preserved_as_a_blocking_union() {
    let (dirs, ledger, family) = setup();
    let first = reserve(&ledger, &family, 'a', 'c').reservation.unwrap();
    let mut other_body = first.document()["body"].clone();
    other_body["originRegistryId"] = json!("f".repeat(32));
    other_body["reservationKey"] = json!("b".repeat(32));
    let other = freeze_alpha_reservation(&other_body.to_string()).unwrap();
    // Exercise storage of a future independently produced union, without
    // pretending this is the still-unimplemented complete transfer API.
    {
        let mut conn = ledger.lock().unwrap();
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        store_record(&tx, &other).unwrap();
        store_conflict(
            &tx,
            "confirmation_number",
            vec![first.document(), other.document()],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let state = ledger.read_alpha_family(&family).unwrap().unwrap();
    assert!(state.blocked);
    assert!(state.allocation.is_none());
    assert_eq!(state.reservations.len(), 2);
    assert!(state.reservations.contains(&first));
    assert!(state.reservations.contains(&other));
    drop(ledger);
    assert_eq!(
        dirs.open().read_alpha_family(&family).unwrap().unwrap(),
        state
    );
}

#[test]
fn independent_connections_serialize_next_number_without_losing_a_spend() {
    let (dirs, ledger, family) = setup();
    let other = dirs.open();
    let barrier = Arc::new(Barrier::new(2));
    let barrier2 = barrier.clone();
    let family2 = family.clone();
    let worker = std::thread::spawn(move || {
        barrier2.wait();
        reserve(&other, &family2, 'b', 'd')
    });
    barrier.wait();
    let first = reserve(&ledger, &family, 'a', 'c');
    let second = worker.join().unwrap();
    assert_eq!(first.status, AlphaStorageStatus::Stored);
    assert_eq!(second.status, AlphaStorageStatus::Stored);
    let mut numbers = [
        first.reservation.unwrap().confirmation_number(),
        second.reservation.unwrap().confirmation_number(),
    ];
    numbers.sort();
    assert_eq!(numbers, [1, 2]);
    assert_eq!(
        ledger
            .read_alpha_family(&family)
            .unwrap()
            .unwrap()
            .reservations
            .len(),
        2
    );
}

#[test]
fn malformed_unknown_and_oversized_requests_write_nothing() {
    let (_dirs, ledger, family) = setup();
    let before = rows(&ledger.lock().unwrap()).unwrap();
    assert!(ledger
        .reserve_alpha_evidence(&family, "bad", &"a".repeat(64))
        .is_err());
    assert!(ledger
        .declare_alpha_budget(&format!("trial-family-v1:{}", "f".repeat(64)), &budget())
        .is_err());
    assert!(ledger
        .declare_alpha_budget(
            &family,
            &AlphaAllocationDeclaration {
                total_alpha_ppm: 1,
                schedule: vec![2]
            }
        )
        .is_err());
    let oversized = AlphaAllocationDeclaration {
        total_alpha_ppm: 200_000,
        schedule: vec![1; 200_000],
    };
    assert!(ledger.declare_alpha_budget(&family, &oversized).is_err());
    assert_eq!(rows(&ledger.lock().unwrap()).unwrap(), before);
    assert!(!ledger.read_alpha_family(&family).unwrap().unwrap().blocked);
}

#[test]
fn append_only_triggers_and_corrupted_evidence_refusal_survive_reopen() {
    let (dirs, ledger, family) = setup();
    reserve(&ledger, &family, 'a', 'c');
    reserve(&ledger, &family, 'a', 'd');
    {
        let conn = ledger.lock().unwrap();
        for table in [
            "alpha_family_budgets",
            "alpha_reservations",
            "alpha_conflicts",
        ] {
            assert!(conn.execute(&format!("DELETE FROM {table}"), []).is_err());
            let column = if table == "alpha_conflicts" {
                "detail_json"
            } else if table == "alpha_reservations" {
                "record_json"
            } else {
                "document_json"
            };
            assert!(conn
                .execute(&format!("UPDATE {table} SET {column}={column}"), [])
                .is_err());
        }
        // A tampering probe in this disposable test registry only.
        conn.execute_batch("DROP TRIGGER alpha_reservations_no_update; UPDATE alpha_reservations SET record_json='{}';").unwrap();
    }
    assert!(ledger.read_alpha_family(&family).is_err());
    assert!(ledger
        .reserve_alpha_evidence(&family, &"f".repeat(32), &"a".repeat(64))
        .is_err());
    drop(ledger);
    let reopened = dirs.open();
    assert!(reopened.read_alpha_family(&family).is_err());
    register(
        &reopened,
        "crypto:binance:BTCUSDT",
        "exploration-still-works",
    );
}

#[test]
fn failed_conflict_write_rolls_back_the_proposed_record_and_keeps_original_spend() {
    let (_dirs, ledger, family) = setup();
    let first = reserve(&ledger, &family, 'a', 'c').reservation.unwrap();
    ledger.lock().unwrap().execute_batch("CREATE TRIGGER injected_alpha_failure BEFORE INSERT ON alpha_conflicts BEGIN SELECT RAISE(ABORT,'injected conflict failure'); END;").unwrap();
    assert!(ledger
        .reserve_alpha_evidence(&family, &"a".repeat(32), &"d".repeat(64))
        .is_err());
    let state = ledger.read_alpha_family(&family).unwrap().unwrap();
    assert_eq!(state.reservations, vec![first]);
    assert!(!state.blocked);
    assert_eq!(state.allocation.unwrap().spent_alpha_ppm, 25_000);
}

#[test]
fn row_bound_refuses_history_instead_of_reading_a_truncated_prefix() {
    let (_dirs, ledger, family) = setup();
    // Deliberately oversized disposable DB. The row bound is checked before
    // malformed rows can be decoded or any prefix can be treated as history.
    ledger.lock().unwrap().execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x < 10000) INSERT INTO alpha_reservations (reservation_id,family_id,record_json,recorded_at) SELECT printf('%064x',x),?1,'{}','test' FROM n", [&family]).unwrap();
    let error = ledger.read_alpha_family(&family).unwrap_err();
    assert!(error.to_string().contains("row limit"));
    assert!(ledger
        .reserve_alpha_evidence(&family, &"a".repeat(32), &"c".repeat(64))
        .is_err());
    let (_byte_dirs, byte_ledger, byte_family) = setup();
    byte_ledger.lock().unwrap().execute_batch("DROP TRIGGER alpha_family_budgets_no_update; UPDATE alpha_family_budgets SET document_json=printf('%.*c',16777217,'x');").unwrap();
    assert!(byte_ledger
        .read_alpha_family(&byte_family)
        .unwrap_err()
        .to_string()
        .contains("byte limit"));
}

#[test]
fn legacy_transfer_is_unchanged_without_alpha_and_refused_once_budget_is_pinned() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let family = register(&ledger, "crypto:binance:BTCUSDT", "before-alpha");
    let original = ledger.export_json_lines().unwrap();
    let header: Value =
        serde_json::from_slice(original.split(|b| *b == b'\n').next().unwrap()).unwrap();
    assert_eq!(header["schemaVersion"], "0003_family_protocol_upgrades");
    ledger.import_json_lines(&original).unwrap();
    ledger.declare_alpha_budget(&family, &budget()).unwrap();
    reserve(&ledger, &family, 'a', 'c');
    let before = ledger.read_alpha_family(&family).unwrap();
    ledger.import_json_lines(&original).unwrap();
    assert_eq!(ledger.read_alpha_family(&family).unwrap(), before);
    assert!(ledger
        .export_json_lines()
        .unwrap_err()
        .to_string()
        .contains("alpha-aware export"));
}
