//! Actual source snapshots and real SQLite transactions, without caller facts.
use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::{
    freeze_source_witness, parse_source_witness, SourceWitnessSnapshot,
};
#[path = "witness_current_tests.rs"]
mod current;
#[path = "witness_storage_tests.rs"]
mod storage;

pub(super) fn original(ledger: &TrialLedger, ctx: &GraphContext) -> SourceWitnessSnapshot {
    let mut conn = ledger.lock().unwrap();
    let tx = conn.transaction().unwrap();
    let bytes = ledger.export_trial_in(&tx, Some(8 * 1024 * 1024)).unwrap();
    freeze_source_witness(
        std::str::from_utf8(&bytes).unwrap(),
        &ctx.registered_sources[0],
    )
    .unwrap()
}
fn install(ledger: &TrialLedger, w: &SourceWitnessSnapshot) {
    ledger
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO confirmation_source_witnesses VALUES (?1,?2,?3,'original-audit')",
            params![w.sha256(), w.text().len() as i64, w.text().as_bytes()],
        )
        .unwrap();
}
fn counts(ledger: &TrialLedger) -> (i64, i64, i64) {
    ledger.lock().unwrap().query_row("SELECT (SELECT COUNT(*) FROM confirmation_source_witnesses),(SELECT COUNT(*) FROM confirmation_unknown_origins),(SELECT COUNT(*) FROM trial_events)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap()
}
fn independent() -> SourceWitnessSnapshot {
    let f: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-source-witness-v1.json"
    ))
    .unwrap();
    parse_source_witness(f["cases"][0]["witnessText"].as_str().unwrap()).unwrap()
}

#[test]
fn actual_managed_restore_is_explicit_and_preserves_old_bytes_audits_and_replay() {
    let (dirs, ledger, input, ctx) = fixture();
    let w = original(&ledger, &ctx);
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let before = ledger
        .read_confirmation_history(&ctx)
        .unwrap()
        .inventory()
        .clone();
    assert!(ledger.read_witnessed_confirmation_history().is_err());
    let summary = ledger
        .restore_confirmation_source_witnesses(std::slice::from_ref(&w))
        .unwrap();
    assert_eq!(
        (
            summary.added_records,
            summary.added_blobs,
            summary.added_intervals,
            summary.added_unknown_origins
        ),
        (0, 1, 0, 0)
    );
    let graph = ledger.read_witnessed_confirmation_history().unwrap();
    assert!(!graph.is_restricted());
    let mut expected = before;
    expected
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, w.text()).unwrap());
    expected.blobs.sort_by(|a, b| a.sha256().cmp(b.sha256()));
    assert_eq!(graph.inventory(), &expected);
    let audit: String = ledger
        .lock()
        .unwrap()
        .query_row(
            "SELECT recorded_at FROM confirmation_source_witnesses",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let replay = ledger.restore_confirmation_source_witnesses(&[w]).unwrap();
    assert_eq!(
        (
            replay.added_records,
            replay.added_blobs,
            replay.added_intervals,
            replay.added_unknown_origins
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(
        ledger
            .lock()
            .unwrap()
            .query_row(
                "SELECT recorded_at FROM confirmation_source_witnesses",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        audit
    );
    let connection = dirs.open();
    assert_eq!(
        connection
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &expected
    );
    drop(connection);
    drop(ledger);
    assert_eq!(
        dirs.open()
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &expected
    );
}

#[test]
fn current_growth_does_not_replace_original_historical_counts_or_lookup() {
    let (_dirs, ledger, input, ctx) = fixture();
    let w = original(&ledger, &ctx);
    let mut strict = input.clone();
    strict
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, w.text()).unwrap());
    ledger
        .append_witnessed_confirmation_history(&strict)
        .unwrap();
    register(&ledger, vec![event("later", 0, "later-0")]);
    let graph = ledger.read_witnessed_confirmation_history().unwrap();
    assert_eq!(graph.inventory().admissions.len(), 2);
    let op: Value = serde_json::from_str(&graph.inventory().operations[0]).unwrap();
    let found = ledger
        .find_original_witnessed_confirmation_operations(
            op["body"]["originRegistryId"].as_str().unwrap(),
            op["body"]["reservationKey"].as_str().unwrap(),
        )
        .unwrap();
    assert_eq!(
        found.operation_ids(),
        &[op["id"].as_str().unwrap().to_string()]
    );
    assert_eq!(found.graph().inventory(), graph.inventory());
    assert_eq!(counts(&ledger), (1, 0, 5));
    let before = graph.inventory().clone();
    assert!(ledger
        .find_original_witnessed_confirmation_operations("bad", "bad")
        .is_err());
    assert_eq!(
        ledger
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &before
    );
}

#[test]
fn original_proof_is_required_even_with_permanent_uncertainty_and_no_auto_repair() {
    let (_dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    record_unknown_origins(
        &ledger.lock().unwrap(),
        &BTreeSet::from([ledger.registry_id().into()]),
    )
    .unwrap();
    let before = counts(&ledger);
    assert!(ledger.read_witnessed_confirmation_history().is_err());
    assert!(ledger
        .append_witnessed_confirmation_history(&Default::default())
        .is_err());
    assert!(ledger.restore_confirmation_source_witnesses(&[]).is_err());
    assert_eq!(counts(&ledger), before);
    ledger
        .restore_confirmation_source_witnesses(&[original(&ledger, &ctx)])
        .unwrap();
    assert!(ledger
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    assert_eq!(counts(&ledger), (1, 1, 4));
}

#[test]
fn read_refuses_unmarked_gaps_and_restoration_derives_only_actual_stops_without_ingestion() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let w = independent();
    install(&ledger, &w);
    assert!(ledger.read_witnessed_confirmation_history().is_err());
    record_unknown_origins(&ledger.lock().unwrap(), &BTreeSet::from(["f".repeat(32)])).unwrap();
    assert!(ledger.read_witnessed_confirmation_history().is_err());
    let result = ledger.restore_confirmation_source_witnesses(&[]).unwrap();
    assert_eq!((result.added_blobs, result.added_unknown_origins), (0, 1));
    let graph = ledger.read_witnessed_confirmation_history().unwrap();
    assert!(graph.is_restricted());
    assert_eq!(counts(&ledger), (1, 2, 0));
    let original_origin = crate::research::trial_ledger::transfer::reconstruct_source_witness(&w)
        .unwrap()
        .prefix()
        .registry_id
        .clone();
    assert!(graph.inventory().unknown_origins.contains(&original_origin));
}

#[test]
fn archival_append_adds_exact_gap_markers_and_preserves_all_old_history() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let w = independent();
    let input = ConfirmationInventory {
        blobs: vec![HistoryBlob::new(BlobKind::SourceWitness, w.text()).unwrap()],
        ..Default::default()
    };
    let added = ledger
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    assert_eq!((added.added_blobs, added.added_unknown_origins), (1, 1));
    assert_eq!(counts(&ledger), (1, 1, 0));
    let before = ledger
        .read_witnessed_confirmation_history()
        .unwrap()
        .inventory()
        .clone();
    let replay = ledger
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    assert_eq!((replay.added_blobs, replay.added_unknown_origins), (0, 0));
    assert_eq!(
        ledger
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &before
    );
}

#[test]
fn late_witness_or_unknown_write_failure_rolls_back_every_addition() {
    for trigger in [
        "CREATE TRIGGER fail_witness BEFORE INSERT ON confirmation_source_witnesses BEGIN SELECT RAISE(ABORT,'late witness'); END;",
        "CREATE TRIGGER fail_unknown BEFORE INSERT ON confirmation_unknown_origins BEGIN SELECT RAISE(ABORT,'late unknown'); END;",
    ] {
        let dirs=Dirs::new();let ledger=dirs.open();let w=independent();
        ledger.lock().unwrap().execute_batch(trigger).unwrap();
        assert!(ledger.restore_confirmation_source_witnesses(&[w]).is_err());
        assert_eq!(counts(&ledger),(0,0,0));
        assert!(ledger.read_witnessed_confirmation_history().unwrap().inventory().blobs.is_empty());
    }
}

#[test]
fn bad_nested_proof_or_original_digest_cannot_borrow_a_good_witness_or_unknown_marker() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let w = independent();
    ledger
        .restore_confirmation_source_witnesses(std::slice::from_ref(&w))
        .unwrap();
    let before = counts(&ledger);
    let invalid = freeze_source_witness("PROTECTED_SENTINEL\n", w.source_claims()).unwrap();
    let error = ledger
        .restore_confirmation_source_witnesses(&[invalid])
        .unwrap_err()
        .to_string();
    assert!(!error.contains("PROTECTED_SENTINEL"));
    assert_eq!(counts(&ledger), before);
    ledger
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO confirmation_source_witnesses VALUES (?1,?2,?3,'bad-audit')",
            params!["f".repeat(64), w.text().len() as i64, w.text().as_bytes()],
        )
        .unwrap();
    assert!(ledger.read_witnessed_confirmation_history().is_err());
    assert!(ledger.restore_confirmation_source_witnesses(&[w]).is_err());
    assert_eq!(counts(&ledger), (2, 1, 0));
}

#[test]
fn restoration_does_not_complete_missing_materialized_conflicts_or_indexes() {
    for missing_conflict in [false, true] {
        let (_dirs, ledger, mut input, ctx) = fixture();
        let w = original(&ledger, &ctx);
        if missing_conflict {
            let val: Value = serde_json::from_str(&input.exposures[0]).unwrap();
            let body = json!({"contractVersion":"research-confirmation-exposure-v1","originRegistryId":"e".repeat(32),"reservationKey":"f".repeat(32),"instrumentId":val["body"]["instrumentId"],"intervals":val["body"]["intervals"],"claim":{"kind":"manual","accessKind":"raw","afterSealId":null}});
            let exposure =
                alpha_factor_forge::discovery_core::confirmation_history::freeze_manual_exposure(
                    &body.to_string(),
                )
                .unwrap();
            input.exposures.push(exposure.document().to_string());
        }
        ledger.append_confirmation_history(&input, &ctx).unwrap();
        {
            let conn = ledger.lock().unwrap();
            if missing_conflict {
                assert!(
                    conn.query_row("SELECT COUNT(*) FROM confirmation_conflicts", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap()
                        > 0
                );
                conn.execute_batch("DROP TRIGGER confirmation_conflicts_no_delete; DELETE FROM confirmation_conflicts").unwrap();
            } else {
                conn.execute_batch("DROP TRIGGER confirmation_intervals_no_delete; DELETE FROM confirmation_intervals").unwrap();
            }
        }
        let before = counts(&ledger);
        assert!(ledger.restore_confirmation_source_witnesses(&[w]).is_err());
        assert_eq!(counts(&ledger), before);
    }
}

#[test]
fn final_strict_reload_failure_rolls_back_witnesses_markers_and_trigger_side_effects() {
    for append in [false, true] {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        let w = independent();
        ledger.lock().unwrap().execute_batch(&format!("CREATE TRIGGER corrupt_after_witness AFTER INSERT ON confirmation_source_witnesses BEGIN INSERT INTO confirmation_conflicts VALUES ('{}','{{}}','late-side-effect'); END;","f".repeat(64))).unwrap();
        let result = if append {
            ledger.append_witnessed_confirmation_history(&ConfirmationInventory {
                blobs: vec![HistoryBlob::new(BlobKind::SourceWitness, w.text()).unwrap()],
                ..Default::default()
            })
        } else {
            ledger.restore_confirmation_source_witnesses(&[w])
        };
        assert!(result.is_err());
        assert_eq!(counts(&ledger), (0, 0, 0));
        assert_eq!(
            ledger
                .lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM confirmation_conflicts", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn conflicting_original_metadata_and_substituted_materialized_columns_are_refused() {
    let (_dirs, ledger, input, ctx) = fixture();
    let w = original(&ledger, &ctx);
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    ledger
        .restore_confirmation_source_witnesses(std::slice::from_ref(&w))
        .unwrap();
    let before = ledger
        .read_witnessed_confirmation_history()
        .unwrap()
        .inventory()
        .clone();
    let mut claims = w.source_claims().clone();
    claims.source.engine_manifest_hash = "f".repeat(64);
    let other = freeze_source_witness(w.trial_export_text(), &claims).unwrap();
    assert!(crate::research::trial_ledger::transfer::reconstruct_source_witness(&other).is_ok());
    assert!(ledger
        .restore_confirmation_source_witnesses(&[other])
        .is_err());
    assert_eq!(
        ledger
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &before
    );
    ledger.lock().unwrap().execute_batch(&format!("DROP TRIGGER confirmation_operations_no_update; UPDATE confirmation_operations SET workspace_id='{}'","f".repeat(32))).unwrap();
    assert!(ledger.read_witnessed_confirmation_history().is_err());
    assert!(ledger.restore_confirmation_source_witnesses(&[w]).is_err());
    assert_eq!(counts(&ledger), (1, 0, 4));
}

#[test]
fn exact_materialized_row_and_byte_limits_refuse_witness_union_before_sql_changes() {
    use alpha_factor_forge::discovery_core::confirmation_history::freeze_manual_exposure;
    for row_limit in [false, true] {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        let n = if row_limit { 10000 } else { 16 };
        {
            let mut conn = ledger.lock().unwrap();
            let tx = conn.transaction().unwrap();
            let guard: String = tx
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE name='confirmation_exposures_capacity'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            // Only the fixture's bulk setup skips repeated aggregate scans;
            // restore the exact production guard before exercising strict IO.
            tx.execute_batch("DROP TRIGGER confirmation_exposures_capacity")
                .unwrap();
            for index in 0..n {
                let e=freeze_manual_exposure(&json!({"contractVersion":"research-confirmation-exposure-v1","originRegistryId":ledger.registry_id(),"reservationKey":format!("{index:032x}"),"instrumentId":"crypto:binance:BTCUSDT","intervals":[{"fromMs":946684800000u64,"toExclusiveMs":946684800001u64}],"claim":{"kind":"manual","accessKind":"raw","afterSealId":null}}).to_string()).unwrap();
                let mut raw = e.document().to_string();
                if !row_limit {
                    raw.push_str(&" ".repeat(MAX_DOCUMENT_BYTES - raw.len()));
                }
                tx.execute("INSERT INTO confirmation_exposures VALUES (?1,'crypto:binance:BTCUSDT','manual',NULL,NULL,?2,'original-audit')",params![e.id(),raw]).unwrap();
                tx.execute("INSERT INTO confirmation_intervals VALUES (?1,0,'crypto:binance:BTCUSDT',946684800000,946684800001)",[e.id()]).unwrap();
            }
            tx.execute_batch(&guard).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(
            ledger
                .read_witnessed_confirmation_history()
                .unwrap()
                .inventory()
                .exposures
                .len(),
            n
        );
        let before = counts(&ledger);
        let w = independent();
        assert!(ledger
            .restore_confirmation_source_witnesses(std::slice::from_ref(&w))
            .is_err());
        assert!(ledger
            .append_witnessed_confirmation_history(&ConfirmationInventory {
                blobs: vec![HistoryBlob::new(BlobKind::SourceWitness, w.text()).unwrap()],
                ..Default::default()
            })
            .is_err());
        assert_eq!(counts(&ledger), before);
    }
}
