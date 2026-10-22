//! Public v5 import against real SQLite, original proofs and immutable audits.
use super::*;
use crate::research::trial_ledger::{self as ledger, BindingCheck};
use rusqlite::types::ValueRef;
use sha2::{Digest, Sha256};
#[path = "archive_import_capacity.rs"]
mod capacity;

fn managed() -> (Dirs, TrialLedger) {
    let (dirs, source, mut input, context) = fixture();
    let proof = witnessed::original(&source, &context);
    // Canonical wire replay must retain a receiver's earlier accepted original
    // document representation and its first audit, not replace it with wire text.
    input.operations[0] = format!(
        "\n{}\n",
        serde_json::to_string_pretty(&json(&input.operations[0]).unwrap()).unwrap()
    );
    input
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, proof.text()).unwrap());
    source
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    (dirs, source)
}
fn unknown_event() -> TrialEventInput {
    let mut unknown = event("unknown", 0, "unknown");
    unknown.snapshot_id = None;
    unknown
}
// Stream exact typed cells, including first-observed text/audits, row IDs and
// sequence high-water marks. Only the replay audit is optionally excluded.
fn image(conn: &Connection, imports: bool) -> String {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut hash = Sha256::new();
    for table in tables {
        if !imports && matches!(table.as_str(), "registry_imports" | "sqlite_sequence") {
            continue;
        }
        hash.update((table.len() as u64).to_le_bytes());
        hash.update(table.as_bytes());
        let quoted = format!("\"{}\"", table.replace('"', "\"\""));
        let mut stmt = conn
            .prepare(&format!("SELECT rowid,* FROM {quoted} ORDER BY rowid"))
            .unwrap();
        let count = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            hash.update(b"row");
            for i in 0..count {
                match row.get_ref(i).unwrap() {
                    ValueRef::Null => hash.update([0]),
                    ValueRef::Integer(v) => {
                        hash.update([1]);
                        hash.update(v.to_le_bytes());
                    }
                    ValueRef::Real(v) => {
                        hash.update([2]);
                        hash.update(v.to_bits().to_le_bytes());
                    }
                    ValueRef::Text(v) | ValueRef::Blob(v) => {
                        hash.update([if matches!(row.get_ref(i).unwrap(), ValueRef::Text(_)) {
                            3
                        } else {
                            4
                        }]);
                        hash.update((v.len() as u64).to_le_bytes());
                        hash.update(v);
                    }
                }
            }
        }
    }
    format!("{:x}", hash.finalize())
}
fn state(l: &TrialLedger) -> String {
    image(&l.lock().unwrap(), true)
}
fn identities(l: &TrialLedger) -> String {
    image(&l.lock().unwrap(), false)
}
fn changes(l: &TrialLedger) -> u64 {
    l.lock()
        .unwrap()
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap()
}
fn rows(l: &TrialLedger, table: &str) -> usize {
    l.lock()
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
fn equivalent(a: &ConfirmationInventory, b: &ConfirmationInventory) {
    for (left, right) in [
        (&a.batches, &b.batches),
        (&a.selections, &b.selections),
        (&a.operations, &b.operations),
        (&a.exposures, &b.exposures),
        (&a.admissions, &b.admissions),
        (&a.events, &b.events),
        (&a.conflicts, &b.conflicts),
    ] {
        assert_eq!(
            left.iter().map(|s| json(s).unwrap()).collect::<Vec<_>>(),
            right.iter().map(|s| json(s).unwrap()).collect::<Vec<_>>()
        );
    }
    assert_eq!(a.blobs, b.blobs);
    assert_eq!(a.unknown_origins, b.unknown_origins);
}
#[test]
fn complete_union_replay_keeps_original_proofs_first_audits_and_outer_file_digest() {
    let (_source_dirs, source) = managed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let source_before = state(&source);
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let added = target
        .import_witnessed_qualification_json_lines(&bytes)
        .unwrap();
    assert_eq!(added.alpha.trial.added_events, 4);
    assert_eq!(added.alpha.added_budget_rows, 1);
    assert_eq!(added.alpha.added_reservations, 2);
    assert!(added.trial_history_known);
    assert!(!added.restricted);
    assert_eq!(added.confirmation_unknown_origin_count, 0);
    equivalent(
        source
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        target
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
    );
    assert_eq!(rows(&target, "alpha_unknown_origins"), 0);
    assert_eq!(rows(&target, "confirmation_unknown_origins"), 0);
    let digest: String = target
        .lock()
        .unwrap()
        .query_row("SELECT file_sha256 FROM registry_imports", [], |r| r.get(0))
        .unwrap();
    assert_eq!(digest, ledger::sha256_hex(&bytes));
    assert!(target.export_json_lines().is_err());
    assert!(target.export_complete_json_lines().is_err());
    let before = identities(&target);
    let replay = target.import_witnessed_json_lines(&bytes).unwrap();
    assert_eq!(replay.alpha.trial.added_events, 0);
    assert_eq!(replay.alpha.added_reservations, 0);
    assert_eq!(
        replay.confirmation.added_records
            + replay.confirmation.added_blobs
            + replay.confirmation.added_intervals
            + replay.confirmation.added_unknown_origins,
        0
    );
    assert_eq!(identities(&target), before);
    assert_eq!(rows(&target, "registry_imports"), 2);
    assert_eq!(state(&source), source_before);
}
#[test]
fn two_hop_replacement_self_import_and_reopen_preserve_saved_whole_graph_binding() {
    let (_source_dirs, source) = managed();
    let saved = match source.check_binding(None).unwrap() {
        BindingCheck::Current(b) => b,
        other => panic!("{other:?}"),
    };
    let middle_dirs = Dirs::new();
    let middle = middle_dirs.open();
    middle
        .import_witnessed_json_lines(&source.export_witnessed_json_lines().unwrap())
        .unwrap();
    let last_dirs = Dirs::new();
    let last = last_dirs.open();
    last.import_witnessed_qualification_json_lines(&middle.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert!(matches!(
        last.check_binding(Some(&saved)).unwrap(),
        BindingCheck::Current(_)
    ));
    equivalent(
        source
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        last.read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
    );
    let before = identities(&source);
    assert_eq!(
        source
            .import_witnessed_qualification_json_lines(
                &source.export_witnessed_json_lines().unwrap()
            )
            .unwrap()
            .alpha
            .trial
            .added_events,
        0
    );
    assert_eq!(identities(&source), before);
    drop(last);
    assert!(matches!(
        last_dirs.open().check_binding(Some(&saved)).unwrap(),
        BindingCheck::Current(_)
    ));
}
#[test]
fn independent_known_conflicts_keep_both_original_histories_and_travel_without_granting_authority()
{
    let (_a_dirs, a) = managed();
    let (_b_dirs, b) = managed();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    target
        .import_witnessed_json_lines(&a.export_witnessed_json_lines().unwrap())
        .unwrap();
    let summary = target
        .import_witnessed_json_lines(&b.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert_eq!(summary.alpha.trial.added_events, 0);
    assert_eq!(summary.alpha.added_reservations, 2);
    assert!(summary.restricted && summary.trial_history_known);
    assert!(rows(&target, "alpha_conflicts") > 0);
    let graph = target.read_witnessed_confirmation_history().unwrap();
    assert_eq!(graph.inventory().operations.len(), 4);
    assert_eq!(graph.inventory().admissions.len(), 4);
    assert!(!graph.inventory().conflicts.is_empty());
    for original in [&a, &b] {
        for blob in &original
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory()
            .blobs
        {
            assert!(graph.inventory().blobs.contains(blob));
        }
    }
    register(&target, vec![event("after-stop", 0, "after-stop")]);
    let last_dirs = Dirs::new();
    let last = last_dirs.open();
    let result = last
        .import_witnessed_qualification_json_lines(&target.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert!(result.restricted && result.trial_history_known);
    equivalent(
        target
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        last.read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
    );
    assert_eq!(rows(&last, "trial_events"), 5);
    assert_eq!(rows(&last, "alpha_reservations"), 4);
}
#[test]
fn unknown_trial_family_is_archival_only_and_qualification_refuses_before_any_write() {
    let source_dirs = Dirs::new();
    let source = source_dirs.open();
    source
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: None,
            tests_per_trial: 2,
            events: vec![unknown_event()],
        })
        .unwrap();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = (state(&target), changes(&target));
    assert_eq!(
        target
            .import_witnessed_qualification_json_lines(&bytes)
            .unwrap_err()
            .code(),
        "trial_history_unknown"
    );
    assert_eq!((state(&target), changes(&target)), before);
    let archived = target.import_witnessed_json_lines(&bytes).unwrap();
    assert!(!archived.trial_history_known && archived.restricted);
    assert_eq!(rows(&target, "trial_events"), 1);
}

#[test]
fn source_and_current_unknown_provenance_each_refuse_before_real_changes() {
    for source_side in [true, false] {
        for kind in ["alpha", "confirmation", "trial"] {
            let (_source_dirs, source) = managed();
            let target_dirs = Dirs::new();
            let target = target_dirs.open();
            let unknown = if source_side { &source } else { &target };
            let expected = match kind {
                "alpha" => {
                    ledger::alpha::record_unknown_origins(
                        &unknown.lock().unwrap(),
                        &BTreeSet::from(["e".repeat(32)]),
                    )
                    .unwrap();
                    "alpha_history_unknown"
                }
                "confirmation" => {
                    unknown
                        .append_witnessed_confirmation_history(&ConfirmationInventory {
                            unknown_origins: vec!["e".repeat(32)],
                            ..Default::default()
                        })
                        .unwrap();
                    "confirmation_history_unknown"
                }
                _ => {
                    unknown
                        .register_batch(&TrialBatchInput {
                            workspace_id: "1".repeat(32),
                            instrument_id: None,
                            tests_per_trial: 2,
                            events: vec![unknown_event()],
                        })
                        .unwrap();
                    "trial_history_unknown"
                }
            };
            let bytes = source.export_witnessed_json_lines().unwrap();
            let before = (state(&target), changes(&target));
            assert_eq!(
                target
                    .import_witnessed_qualification_json_lines(&bytes)
                    .unwrap_err()
                    .code(),
                expected,
                "{source_side} {kind}"
            );
            assert_eq!((state(&target), changes(&target)), before);
            assert!(
                target
                    .import_witnessed_json_lines(&bytes)
                    .unwrap()
                    .restricted
            );
        }
    }
}

#[test]
fn proposed_only_trial_gap_refuses_without_writes_but_archival_keeps_originals_and_stops() {
    let (_source_dirs, source) = managed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    register(&target, vec![event("prior", 0, "contradictory-prior")]);
    assert!(!target
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    let before = (state(&target), changes(&target));
    assert_eq!(
        target
            .import_witnessed_qualification_json_lines(&bytes)
            .unwrap_err()
            .code(),
        "trial_history_unknown"
    );
    assert_eq!((state(&target), changes(&target)), before);
    let result = target.import_witnessed_json_lines(&bytes).unwrap();
    assert!(result.restricted && !result.trial_history_known);
    assert_eq!(rows(&target, "trial_events"), 1);
    let graph = target.read_witnessed_confirmation_history().unwrap();
    for blob in source
        .read_witnessed_confirmation_history()
        .unwrap()
        .inventory()
        .blobs
        .iter()
    {
        assert!(graph.inventory().blobs.contains(blob));
    }
    let last_dirs = Dirs::new();
    let last = last_dirs.open();
    let transported = last
        .import_witnessed_json_lines(&target.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert!(transported.restricted && !transported.trial_history_known);
    equivalent(
        graph.inventory(),
        last.read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
    );
    let before = (state(&last), changes(&last));
    assert_eq!(
        last.import_witnessed_qualification_json_lines(&bytes)
            .unwrap_err()
            .code(),
        "trial_history_unknown"
    );
    assert_eq!((state(&last), changes(&last)), before);
}

#[test]
fn missing_source_proof_cannot_borrow_receiver_evidence_or_wait_for_receiver_mutex() {
    let (_source_dirs, source) = managed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    target.import_witnessed_json_lines(&bytes).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let (header, body) = text.split_once('\n').unwrap();
    let mut header = json(header).unwrap();
    let rows: Vec<_> = body
        .lines()
        .map(|s| json(s).unwrap())
        .filter(|v| !(v["type"] == "confirmationBlob" && v["kind"] == "source_witness"))
        .collect();
    let removed = body.lines().count() - rows.len();
    assert_eq!(removed, 1);
    header["counts"]["blobs"] = json!(header["counts"]["blobs"].as_u64().unwrap() - removed as u64);
    let body: String = rows
        .iter()
        .map(|v| format!("{}\n", ledger::canonical_text(v).unwrap()))
        .collect();
    header["bodySha256"] = json!(ledger::sha256_hex(body.as_bytes()));
    let missing = format!("{}\n{body}", ledger::canonical_text(&header).unwrap()).into_bytes();
    let before = (state(&target), changes(&target));
    for invalid in [missing, b"invalid\n".to_vec()] {
        let guard = target.lock().unwrap();
        let (send, recv) = std::sync::mpsc::channel();
        let observed = std::thread::scope(|scope| {
            let task = scope.spawn(|| {
                send.send(
                    target
                        .import_witnessed_json_lines(&invalid)
                        .map_err(|e| e.code()),
                )
                .unwrap()
            });
            let observed = recv.recv_timeout(std::time::Duration::from_secs(10));
            drop(guard); // Always release before join, even if a mutant takes the lock.
            task.join().unwrap();
            observed
        });
        assert_eq!(observed.unwrap().unwrap_err(), "import_integrity_failed");
    }
    assert_eq!((state(&target), changes(&target)), before);
}

#[test]
fn independent_connections_serialize_duplicate_complete_unions() {
    let (_source_dirs, source) = managed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let first = target_dirs.open();
    let second = target_dirs.open();
    let barrier = std::sync::Barrier::new(2);
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            barrier.wait();
            first.import_witnessed_json_lines(&bytes).unwrap()
        });
        let b = scope.spawn(|| {
            barrier.wait();
            second.import_witnessed_json_lines(&bytes).unwrap()
        });
        (a.join().unwrap(), b.join().unwrap())
    });
    let mut adds = [a.alpha.trial.added_events, b.alpha.trial.added_events];
    adds.sort();
    assert_eq!(adds, [0, 4]);
    assert_eq!(
        a.confirmation.added_records + b.confirmation.added_records,
        13
    );
    assert_eq!(a.confirmation.added_blobs + b.confirmation.added_blobs, 5);
    assert_eq!(rows(&first, "registry_imports"), 2);
    assert_eq!(state(&first), state(&second));
    let before = identities(&first);
    first.import_witnessed_json_lines(&bytes).unwrap();
    assert_eq!(identities(&first), before);
}

#[test]
fn late_actual_only_sql_failure_rolls_back_all_trial_alpha_confirmation_and_audit_rows() {
    let (_source_dirs, source) = managed();
    source
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            unknown_origins: vec!["e".repeat(32)],
            ..Default::default()
        })
        .unwrap();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    register(&target, vec![event("existing", 0, "existing")]);
    let saved = match target.check_binding(None).unwrap() {
        BindingCheck::Current(b) => b,
        other => panic!("{other:?}"),
    };
    {
        let c = target.lock().unwrap();
        c.execute_batch("CREATE TABLE test_changes (value INTEGER); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1100) INSERT INTO test_changes SELECT x FROM n; DELETE FROM test_changes; CREATE TRIGGER fail_real_late BEFORE INSERT ON confirmation_unknown_origins WHEN total_changes() >= 2000 BEGIN SELECT RAISE(ABORT,'actual-only late failure'); END;").unwrap();
    }
    let before = state(&target);
    assert!(changes(&target) >= 2200);
    let error = target.import_witnessed_json_lines(&bytes).unwrap_err();
    assert!(
        error.to_string().contains("actual-only late failure"),
        "{error}"
    );
    assert_eq!(state(&target), before);
    assert!(matches!(
        target.check_binding(Some(&saved)).unwrap(),
        BindingCheck::Current(_)
    ));
    assert_eq!(rows(&target, "registry_imports"), 0);
    assert_eq!(rows(&target, "alpha_reservations"), 0);
    assert_eq!(rows(&target, "confirmation_source_witnesses"), 0);
    target
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER fail_real_late")
        .unwrap();
    assert!(
        target
            .import_witnessed_json_lines(&bytes)
            .unwrap()
            .restricted
    );
}

#[test]
fn empty_manual_same_origin_key_conflicts_and_smaller_representatives_preserve_permanent_stops() {
    let empty_dirs = Dirs::new();
    let empty = empty_dirs.open();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    std::fs::write(
        target_dirs.workspace.join("binding-sentinel"),
        b"last-good-binding",
    )
    .unwrap();
    let zero = target
        .import_witnessed_qualification_json_lines(&empty.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert!(zero.trial_history_known && !zero.restricted);
    let mut claims: Vec<_> = (0..3)
        .map(|n| {
            cases::manual(
                &"d".repeat(32),
                'a',
                "raw",
                None,
                946684800000 + n,
                946684800010 + n,
            )
        })
        .collect();
    claims.sort_by_key(|v| json(v).unwrap()["id"].as_str().unwrap().to_string());
    let mut retained_conflicts = Vec::new();
    for claim in claims.iter().rev() {
        let origin_dirs = Dirs::new();
        let origin = origin_dirs.open();
        origin
            .append_witnessed_confirmation_history(&ConfirmationInventory {
                exposures: vec![claim.clone()],
                ..Default::default()
            })
            .unwrap();
        target
            .import_witnessed_qualification_json_lines(
                &origin.export_witnessed_json_lines().unwrap(),
            )
            .unwrap();
        let graph = target.read_witnessed_confirmation_history().unwrap();
        for old in &retained_conflicts {
            assert!(graph.inventory().conflicts.contains(old));
        }
        retained_conflicts = graph.inventory().conflicts.clone();
    }
    assert!(!retained_conflicts.is_empty());
    assert!(target
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    let next_dirs = Dirs::new();
    let next = next_dirs.open();
    let bytes = target.export_witnessed_json_lines().unwrap();
    assert!(
        next.import_witnessed_qualification_json_lines(&bytes)
            .unwrap()
            .restricted
    );
    equivalent(
        target
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        next.read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
    );
    let before = identities(&next);
    next.import_witnessed_json_lines(&bytes).unwrap();
    assert_eq!(identities(&next), before);
    assert_eq!(
        std::fs::read(target_dirs.workspace.join("binding-sentinel")).unwrap(),
        b"last-good-binding"
    );
    assert_eq!(
        std::fs::read_dir(&target_dirs.workspace).unwrap().count(),
        1
    );
    assert_eq!(std::fs::read_dir(&next_dirs.workspace).unwrap().count(), 0);
}

#[test]
fn corrupt_actual_chain_scalar_or_interval_projection_refuses_before_receiver_changes() {
    let (_source_dirs, source) = managed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    for corrupt in [
        "DROP TRIGGER trial_events_no_update; UPDATE trial_events SET chain='broken' WHERE seq=1;",
        "DROP TRIGGER confirmation_operations_no_update; UPDATE confirmation_operations SET reservation_key='eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee';",
        "DROP TRIGGER confirmation_intervals_no_update; UPDATE confirmation_intervals SET from_ms=from_ms+1;",
    ] {
        let (_target_dirs,target)=managed();
        target.lock().unwrap().execute_batch(corrupt).unwrap();
        let before=(state(&target),changes(&target));
        assert!(target.import_witnessed_json_lines(&bytes).is_err());
        assert_eq!((state(&target),changes(&target)),before);
    }
}

#[test]
fn source_unknown_provenance_refuses_before_receiver_mutex() {
    let (_source_dirs, source) = managed();
    ledger::alpha::record_unknown_origins(
        &source.lock().unwrap(),
        &BTreeSet::from(["e".repeat(32)]),
    )
    .unwrap();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = (state(&target), changes(&target));
    let guard = target.lock().unwrap();
    let (send, recv) = std::sync::mpsc::channel();
    let observed = std::thread::scope(|scope| {
        let task = scope.spawn(|| {
            send.send(
                target
                    .import_witnessed_qualification_json_lines(&bytes)
                    .map_err(|e| e.code()),
            )
            .unwrap()
        });
        let observed = recv.recv_timeout(std::time::Duration::from_secs(10));
        drop(guard);
        task.join().unwrap();
        observed
    });
    assert_eq!(observed.unwrap().unwrap_err(), "alpha_history_unknown");
    assert_eq!((state(&target), changes(&target)), before);
}

#[test]
fn actual_non_effective_benchmark_does_not_require_portable_execution_attestation() {
    use alpha_factor_forge::discovery_core::{
        benchmarks::{BenchmarkCosts, RunBenchmarksArgs},
        types::Candle,
    };
    let (_source_dirs, source) = managed();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let candles = vec![Candle {
        timestamp: 0,
        open: 100.,
        high: 101.,
        low: 99.,
        close: 100.,
        volume: 1.,
    }];
    let benchmark = ledger::run_and_attest_deterministic_benchmark(
        "buyHold",
        &RunBenchmarksArgs {
            candles: &candles,
            interval: "1d",
            costs: BenchmarkCosts {
                fee_pct: 0.,
                slip_pct: 0.,
            },
            start_equity: None,
            from: None,
            to: None,
        },
        ledger::BenchmarkContext {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:BTCUSDT".into()),
            origin: TrialOrigin::Request {
                request_id: "bench".into(),
                candidate_index: 0,
            },
            dataset_hash: "dataset".into(),
            snapshot_id: Some("snapshot".into()),
            split_hash: "split".into(),
            seeds_hash: "seeds".into(),
            engine_fingerprint_hash: "engine".into(),
        },
    )
    .unwrap();
    register(&target, vec![benchmark]);
    let result = target
        .import_witnessed_qualification_json_lines(&source.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert!(result.trial_history_known && !result.restricted);
    assert_eq!(rows(&target, "trial_events"), 5);
    assert_eq!(
        target
            .lock()
            .unwrap()
            .query_row("SELECT SUM(effective) FROM trial_events", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        4
    );
}

#[test]
fn valid_actual_only_trigger_addition_must_match_private_expected_graph_or_roll_back() {
    let (_source_dirs, source) = managed();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let raw = cases::manual(
        &"e".repeat(32),
        'a',
        "raw",
        None,
        946684800000,
        946684800001,
    );
    let id = json(&raw).unwrap()["id"].as_str().unwrap().to_owned();
    target.lock().unwrap().execute_batch(&format!("CREATE TABLE test_changes (value INTEGER); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1100) INSERT INTO test_changes SELECT x FROM n; DELETE FROM test_changes; CREATE TRIGGER valid_real_addition AFTER INSERT ON registry_imports WHEN total_changes() >= 2000 BEGIN INSERT INTO confirmation_exposures VALUES ('{id}','crypto:binance:BTCUSDT','manual',NULL,NULL,'{}','injected-audit'); INSERT INTO confirmation_intervals VALUES ('{id}',0,'crypto:binance:BTCUSDT',946684800000,946684800001); END;",raw.replace('\'',"''"))).unwrap();
    let before = state(&target);
    let error = target.import_witnessed_json_lines(&bytes).unwrap_err();
    assert!(
        error.to_string().contains("differs from private preflight"),
        "{error}"
    );
    assert_eq!(state(&target), before);
    assert_eq!(rows(&target, "registry_imports"), 0);
    assert_eq!(rows(&target, "confirmation_exposures"), 0);
}

#[test]
fn complete_foreign_origin_coverage_and_older_anchor_proofs_survive_two_hops() {
    let (_source_dirs, source, mut input, context) = fixture();
    let older = witnessed::original(&source, &context);
    input
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, older.text()).unwrap());
    source
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    let foreign_dirs = Dirs::new();
    let foreign = foreign_dirs.open();
    foreign
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:ETHUSDT".into()),
            tests_per_trial: 2,
            events: vec![event("foreign", 0, "foreign")],
        })
        .unwrap();
    source
        .import_witnessed_qualification_json_lines(&foreign.export_witnessed_json_lines().unwrap())
        .unwrap();
    let newer = witnessed::original(&source, &context);
    assert_ne!(older.sha256(), newer.sha256());
    source
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            blobs: vec![HistoryBlob::new(BlobKind::SourceWitness, newer.text()).unwrap()],
            ..Default::default()
        })
        .unwrap();
    let middle_dirs = Dirs::new();
    let middle = middle_dirs.open();
    middle
        .import_witnessed_qualification_json_lines(&source.export_witnessed_json_lines().unwrap())
        .unwrap();
    let last_dirs = Dirs::new();
    let last = last_dirs.open();
    let result = last
        .import_witnessed_qualification_json_lines(&middle.export_witnessed_json_lines().unwrap())
        .unwrap();
    assert!(result.trial_history_known && !result.restricted);
    equivalent(
        source
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        last.read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
    );
    assert_eq!(
        last.lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM trial_events WHERE origin_registry_id=?1",
                [foreign.registry_id()],
                |r| r.get::<_, usize>(0)
            )
            .unwrap(),
        1
    );
    for proof in [older, newer] {
        assert!(last
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory()
            .blobs
            .iter()
            .any(|b| b.sha256() == proof.sha256() && b.text() == proof.text()));
    }
}
