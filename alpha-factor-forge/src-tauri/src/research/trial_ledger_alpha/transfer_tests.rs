use super::super::tests::{budget, observed, register, reserve, setup, Dirs};
use super::*;
use crate::research::trial_ledger::BindingCheck;
use std::sync::{Arc, Barrier};

fn state(ledger: &TrialLedger) -> (Inventory, BTreeMap<String, i64>, (u64, String)) {
    let mut conn = ledger.lock().unwrap();
    let conn = conn.transaction().unwrap();
    let mut counts = BTreeMap::new();
    for table in [
        "trial_families",
        "trial_batches",
        "trial_events",
        "batch_receipts",
        "origin_checkpoints",
        "origin_genesis",
        "family_conflicts",
        "registry_conflicts",
        "family_protocol_upgrades",
        "registry_imports",
    ] {
        counts.insert(
            table.into(),
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap(),
        );
    }
    (
        Inventory::read(&conn).unwrap(),
        counts,
        crate::research::trial_ledger::head(&conn, ledger.registry_id()).unwrap(),
    )
}

fn rewrite(bytes: &[u8], change: impl FnOnce(&mut Value, &mut Vec<Value>)) -> Vec<u8> {
    let text = std::str::from_utf8(bytes).unwrap();
    let mut lines = text.lines();
    let mut header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    let mut rows: Vec<Value> = lines
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    change(&mut header, &mut rows);
    for (key, kind) in [
        ("budgetCount", "alphaBudget"),
        ("reservationCount", "alphaReservation"),
        ("conflictCount", "alphaConflict"),
        ("unknownOriginCount", "alphaUnknownOrigin"),
    ] {
        header[key] = json!(rows.iter().filter(|row| row["type"] == kind).count());
    }
    let body = rows
        .iter()
        .map(|row| format!("{}\n", canonical_text(row).unwrap()))
        .collect::<String>();
    header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
    format!("{}\n{body}", canonical_text(&header).unwrap()).into_bytes()
}

#[test]
fn complete_roundtrip_replay_two_hops_and_reopen_keep_spend_and_binding() {
    let (_dirs, source, family) = setup();
    reserve(&source, &family, 'a', 'c');
    reserve(&source, &family, 'b', 'd');
    let saved = observed(&source, None);
    let archive = source.export_complete_json_lines().unwrap();
    let header: Value =
        serde_json::from_slice(archive.split(|b| *b == b'\n').next().unwrap()).unwrap();
    assert_eq!(header["version"], VERSION);
    assert_eq!(header["reservationCount"], 2);
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let first = target.import_qualification_json_lines(&archive).unwrap();
    let imported_hash: String = target
        .lock()
        .unwrap()
        .query_row(
            "SELECT file_sha256 FROM registry_imports ORDER BY import_id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(imported_hash, sha256_hex(&archive));
    assert_eq!(
        (
            first.added_budget_rows,
            first.added_reservations,
            first.added_conflict_rows,
            first.unknown_origin_count
        ),
        (1, 2, 0, 0)
    );
    let original = source.read_alpha_family(&family).unwrap();
    assert_eq!(target.read_alpha_family(&family).unwrap(), original);
    let rebound = observed(&target, Some(&saved));
    assert_eq!(rebound.alpha, saved.alpha);
    let second = target.import_complete_json_lines(&archive).unwrap();
    assert_eq!(
        (
            second.trial.added_events,
            second.added_budget_rows,
            second.added_reservations,
            second.added_conflict_rows
        ),
        (0, 0, 0, 0)
    );
    let last_dirs = Dirs::new();
    let last = last_dirs.open();
    last.import_qualification_json_lines(&target.export_complete_json_lines().unwrap())
        .unwrap();
    assert_eq!(last.read_alpha_family(&family).unwrap(), original);
    observed(&last, Some(&saved));
    observed(&last, Some(&rebound));
    assert_eq!(
        reserve(&last, &family, 'e', 'f').status,
        AlphaStorageStatus::Exhausted
    );
    drop(target);
    drop(last);
    assert_eq!(
        target_dirs.open().read_alpha_family(&family).unwrap(),
        original
    );
    assert_eq!(
        last_dirs.open().read_alpha_family(&family).unwrap(),
        original
    );
}

#[test]
fn empty_complete_archive_preserves_verified_genesis_without_unknown_provenance() {
    let source_dirs = Dirs::new();
    let source = source_dirs.open();
    let saved = observed(&source, None);
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    target
        .import_qualification_json_lines(&source.export_complete_json_lines().unwrap())
        .unwrap();
    observed(&target, Some(&saved));
    assert!(target.require_known_alpha_history().is_ok());
}

#[test]
fn independent_numbers_and_different_budgets_preserve_both_claims_through_two_hops() {
    for different_budget in [false, true] {
        let (_dirs, first, family) = setup();
        let a = reserve(&first, &family, 'a', 'c').reservation.unwrap();
        let other_dirs = Dirs::new();
        let other = other_dirs.open();
        register(&other, "crypto:binance:BTCUSDT", "independent");
        let declaration = if different_budget {
            AlphaAllocationDeclaration {
                total_alpha_ppm: 40_000,
                schedule: vec![20_000, 20_000],
            }
        } else {
            budget()
        };
        other.declare_alpha_budget(&family, &declaration).unwrap();
        let b = reserve(&other, &family, 'a', 'd').reservation.unwrap();
        let saved_other = observed(&other, None);
        let archive = other.export_complete_json_lines().unwrap();
        first.import_complete_json_lines(&archive).unwrap();
        let merged = first.read_alpha_family(&family).unwrap().unwrap();
        assert!(merged.blocked);
        assert!(merged.allocation.is_none());
        assert_eq!(merged.reservations.len(), 2);
        assert!(merged.reservations.contains(&a));
        assert!(merged.reservations.contains(&b));
        observed(&first, Some(&saved_other));
        let identities = observed(&first, None).alpha.unwrap();
        assert_eq!(
            identities.budgets.len(),
            if different_budget { 2 } else { 1 }
        );
        let before = state(&first).0;
        let replay = first.import_complete_json_lines(&archive).unwrap();
        assert_eq!(
            (replay.added_reservations, replay.added_conflict_rows),
            (0, 0)
        );
        assert_eq!(state(&first).0, before);
        let last_dirs = Dirs::new();
        let last = last_dirs.open();
        last.import_complete_json_lines(&first.export_complete_json_lines().unwrap())
            .unwrap();
        assert_eq!(last.read_alpha_family(&family).unwrap().unwrap(), merged);
        assert_eq!(observed(&last, None).alpha.unwrap(), identities);
        register(&last, "crypto:binance:BTCUSDT", "still-explore");
    }
}

#[test]
fn cross_family_operation_union_stops_both_and_requires_both_conflict_copies() {
    let (_dirs, first, family) = setup();
    let original = reserve(&first, &family, 'a', 'c').reservation.unwrap();
    let other_dirs = Dirs::new();
    let other = other_dirs.open();
    let eth = register(&other, "crypto:binance:ETHUSDT", "eth");
    other.declare_alpha_budget(&eth, &budget()).unwrap();
    let mut body = original.document()["body"].clone();
    body["familyId"] = json!(eth);
    body["confirmationBatchId"] = json!("d".repeat(64));
    let reassigned = freeze_alpha_reservation(&body.to_string()).unwrap();
    {
        let mut conn = other.lock().unwrap();
        let tx = conn.transaction().unwrap();
        store_record(&tx, &reassigned).unwrap();
        tx.commit().unwrap();
    }
    first
        .import_complete_json_lines(&other.export_complete_json_lines().unwrap())
        .unwrap();
    for family in [&family, &eth] {
        let value = first.read_alpha_family(family).unwrap().unwrap();
        assert!(value.blocked);
        assert!(value.allocation.is_none());
    }
    let archive = first.export_complete_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    let missing = rewrite(&archive, |_, rows| {
        let index = rows
            .iter()
            .position(|row| row["type"] == "alphaConflict")
            .unwrap();
        rows.remove(index);
    });
    assert!(target.import_complete_json_lines(&missing).is_err());
    assert_eq!(state(&target), before);
    target.import_complete_json_lines(&archive).unwrap();
    assert_eq!(state(&target).0, state(&first).0);
    register(&target, "crypto:binance:ETHUSDT", "still-explore");
}

#[test]
fn legacy_import_cannot_be_laundered_by_complete_export_or_old_reservation_api() {
    let (_dirs, source, family) = setup();
    reserve(&source, &family, 'a', 'c');
    let legacy_dirs = Dirs::new();
    let legacy = legacy_dirs.open();
    register(&legacy, "crypto:binance:BTCUSDT", "legacy-only");
    let trial_only = legacy.export_json_lines().unwrap();
    source.import_json_lines(&trial_only).unwrap();
    assert_eq!(
        source.require_known_alpha_history().unwrap_err().code(),
        "alpha_history_unknown"
    );
    let saved = source.read_alpha_family(&family).unwrap().unwrap();
    assert!(saved.history_unknown && saved.blocked && saved.allocation.is_none());
    assert_eq!(
        reserve(&source, &family, 'b', 'd').status,
        AlphaStorageStatus::Blocked
    );
    assert_eq!(source.read_alpha_family(&family).unwrap().unwrap(), saved);
    register(&source, "crypto:binance:BTCUSDT", "explore-unknown");
    let complete = source.export_complete_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    for version in [
        "trial-ledger-export-v1",
        "trial-ledger-export-v2",
        "trial-ledger-export-v3",
    ] {
        let old = String::from_utf8(trial_only.clone()).unwrap().replacen(
            "trial-ledger-export-v3",
            version,
            1,
        );
        assert_eq!(
            target
                .import_qualification_json_lines(old.as_bytes())
                .unwrap_err()
                .code(),
            "alpha_history_unknown"
        );
        assert_eq!(state(&target), before);
    }
    assert_eq!(
        target
            .import_qualification_json_lines(&complete)
            .unwrap_err()
            .code(),
        "alpha_history_unknown"
    );
    assert_eq!(state(&target), before);
    target.import_complete_json_lines(&complete).unwrap();
    let hop = target.export_complete_json_lines().unwrap();
    let last_dirs = Dirs::new();
    let last = last_dirs.open();
    last.import_complete_json_lines(&hop).unwrap();
    assert_eq!(state(&last).0.unknown, state(&source).0.unknown);
    assert!(
        last.read_alpha_family(&family)
            .unwrap()
            .unwrap()
            .history_unknown
    );
    // A clean source cannot erase a destination's already-unknown history.
    let clean_dirs = Dirs::new();
    let clean = clean_dirs.open();
    let before = state(&last);
    assert_eq!(
        last.import_qualification_json_lines(&clean.export_complete_json_lines().unwrap())
            .unwrap_err()
            .code(),
        "alpha_history_unknown"
    );
    assert_eq!(state(&last), before);
}

#[test]
fn malformed_rehashed_inputs_are_refused_without_any_destination_writes() {
    let (_dirs, source, family) = setup();
    reserve(&source, &family, 'a', 'c');
    let archive = source.export_complete_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    let mut cases = Vec::new();
    cases.push(rewrite(&archive, |_, rows| {
        rows.retain(|row| row["type"] != "alphaBudget")
    }));
    cases.push(rewrite(&archive, |_, rows| {
        rows.iter_mut()
            .find(|row| row["type"] == "alphaReservation")
            .unwrap()["record"]["body"]["confirmationBatchId"] = json!("d".repeat(64));
    }));
    cases.push(rewrite(&archive, |_, rows| {
        let row = rows
            .iter_mut()
            .find(|row| row["type"] == "alphaReservation")
            .unwrap();
        let mut body = row["record"]["body"].clone();
        body["alphaPpm"] = json!(1);
        row["record"] = freeze_alpha_reservation(&body.to_string())
            .unwrap()
            .document();
    }));
    cases.push(rewrite(&archive, |_, rows| {
        let row = rows
            .iter()
            .find(|row| row["type"] == "alphaBudget")
            .unwrap()
            .clone();
        rows.insert(1, row);
    }));
    cases.push(rewrite(&archive, |_, rows| rows.swap(1, 2)));
    cases.push(rewrite(&archive, |_, rows| {
        rows[1]["unexpected"] = json!(true)
    }));
    cases.push(rewrite(&archive, |header, _| {
        header["registryId"] = json!("f".repeat(32))
    }));
    cases.push(rewrite(&archive, |_, rows| {
        let inner = rows[0]["export"].as_str().unwrap();
        let (header, body) = inner.split_once('\n').unwrap();
        let mut header: Value = serde_json::from_str(header).unwrap();
        header["eventCount"] = json!(99);
        rows[0]["export"] = json!(format!("{}\n{body}", canonical_text(&header).unwrap()));
    }));
    let mut truncated = archive.clone();
    truncated.pop();
    cases.push(truncated);
    let text = std::str::from_utf8(&archive).unwrap();
    let (header, body) = text.split_once('\n').unwrap();
    let body = body.strip_suffix('\n').unwrap();
    let mut header: Value = serde_json::from_str(header).unwrap();
    header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
    cases.push(format!("{}\n{body}", canonical_text(&header).unwrap()).into_bytes());
    cases.push(rewrite(&archive, |header, _| {
        header["version"] = json!("trial-ledger-export-v99")
    }));
    cases.push(rewrite(&archive, |header, _| {
        header["unexpected"] = json!(true)
    }));
    let (header, body) = text.split_once('\n').unwrap();
    let mut header: Value = serde_json::from_str(header).unwrap();
    header["reservationCount"] = json!(99);
    cases.push(format!("{}\n{body}", canonical_text(&header).unwrap()).into_bytes());
    let mut hash_changed = archive.clone();
    let last = hash_changed.len() - 2;
    hash_changed[last] = b' ';
    cases.push(hash_changed);
    let text = String::from_utf8(archive.clone()).unwrap();
    cases.push(
        text.replacen("\"budgetCount\":", "\"budgetCount\":1,\"budgetCount\":", 1)
            .into_bytes(),
    );
    for (index, case) in cases.iter().enumerate() {
        assert!(
            target.import_complete_json_lines(case).is_err(),
            "case {index}"
        );
        assert_eq!(state(&target), before, "case {index}");
    }
}

#[test]
fn failed_alpha_write_rolls_back_trial_events_checkpoints_receipts_and_all_alpha() {
    let (_dirs, source, family) = setup();
    reserve(&source, &family, 'a', 'c');
    let archive = source.export_complete_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    target.lock().unwrap().execute_batch("CREATE TRIGGER injected_alpha_failure BEFORE INSERT ON alpha_reservations BEGIN SELECT RAISE(ABORT, 'injected alpha failure'); END").unwrap();
    assert!(target
        .import_complete_json_lines(&archive)
        .unwrap_err()
        .to_string()
        .contains("injected alpha failure"));
    assert_eq!(state(&target), before);
    target
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER injected_alpha_failure")
        .unwrap();
    assert_eq!(
        target
            .import_complete_json_lines(&archive)
            .unwrap()
            .added_reservations,
        1
    );
}

#[test]
fn independent_connections_serialize_duplicate_complete_imports() {
    let (_dirs, source, family) = setup();
    reserve(&source, &family, 'a', 'c');
    let archive = source.export_complete_json_lines().unwrap();
    let dirs = Dirs::new();
    let first = dirs.open();
    let other = dirs.open();
    let barrier = Arc::new(Barrier::new(2));
    let second_barrier = barrier.clone();
    let second_archive = archive.clone();
    let worker = std::thread::spawn(move || {
        second_barrier.wait();
        other.import_complete_json_lines(&second_archive).unwrap()
    });
    barrier.wait();
    let a = first.import_complete_json_lines(&archive).unwrap();
    let b = worker.join().unwrap();
    assert_eq!(a.added_reservations + b.added_reservations, 1);
    assert_eq!(a.trial.added_events + b.trial.added_events, 1);
    assert_eq!(
        first.read_alpha_family(&family).unwrap(),
        source.read_alpha_family(&family).unwrap()
    );
}

#[test]
fn provenance_migration_backfills_and_is_append_only_with_a_hard_capacity() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE registry_migrations (version TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
    )
    .unwrap();
    for (_, sql) in &REGISTRY_MIGRATIONS[..4] {
        conn.execute_batch(sql).unwrap();
    }
    let origin = "a".repeat(32);
    let transitive = "b".repeat(32);
    conn.execute("INSERT INTO registry_imports (source_registry_id,file_sha256,head_seq,head_chain,added_events,skipped_events,conflicts,imported_at) VALUES (?1,?2,0,?3,0,0,0,'before')",params![origin,"c".repeat(64),crate::research::trial_ledger::genesis(&origin)]).unwrap();
    conn.execute(
        "INSERT INTO origin_genesis (origin_registry_id,recorded_at) VALUES (?1,'before')",
        [&transitive],
    )
    .unwrap();
    conn.execute_batch(REGISTRY_MIGRATIONS[4].1).unwrap();
    assert_eq!(
        unknown_origins(&conn).unwrap(),
        BTreeSet::from([origin, transitive])
    );
    for sql in [
        "DELETE FROM alpha_unknown_origins",
        "UPDATE alpha_unknown_origins SET recorded_at = 'changed'",
    ] {
        assert!(conn.execute_batch(sql).is_err());
    }
    let all: BTreeSet<_> = (0..MAX_UNKNOWN_ORIGINS - 2)
        .map(|n| format!("{n:032x}"))
        .collect();
    {
        let tx = conn.transaction().unwrap();
        record_unknown_origins(&tx, &all).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(unknown_origins(&conn).unwrap().len(), MAX_UNKNOWN_ORIGINS);
    assert!(conn
        .execute(
            "INSERT INTO alpha_unknown_origins VALUES (?1,'excess')",
            ["d".repeat(32)]
        )
        .is_err());
    assert!(record_unknown_origins(&conn, &BTreeSet::from(["e".repeat(32)])).is_err());
    assert_eq!(unknown_origins(&conn).unwrap().len(), MAX_UNKNOWN_ORIGINS);
}

#[test]
fn complete_limits_reject_oversize_before_writing_and_replay_at_capacity() {
    let dirs = Dirs::new();
    let source = dirs.open();
    let empty = source.export_complete_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    let oversized_header = rewrite(&empty, |header, _| {
        header["exportedAt"] = json!("x".repeat(MAX_HEADER_BYTES))
    });
    assert!(target
        .import_complete_json_lines(&oversized_header)
        .unwrap_err()
        .to_string()
        .contains("header limit"));
    let oversized_line = rewrite(&empty, |_, rows| {
        rows.push(json!({"type":"alphaUnknownOrigin","originRegistryId":"a".repeat(32),"padding":"x".repeat(MAX_ALPHA_LINE_BYTES)}))
    });
    assert!(target
        .import_complete_json_lines(&oversized_line)
        .unwrap_err()
        .to_string()
        .contains("oversized body line"));
    {
        let mut conn = source.lock().unwrap();
        let tx = conn.transaction().unwrap();
        assert!(source
            .export_trial_in(&tx, Some(1))
            .unwrap_err()
            .to_string()
            .contains("trial projection limit"));
    }
    assert!(target
        .import_complete_json_lines(&vec![b' '; MAX_FILE_BYTES + 1])
        .unwrap_err()
        .to_string()
        .contains("file limit"));
    let too_many =
        rewrite(&empty, |_, rows| {
            rows.extend((0..=MAX_UNKNOWN_ORIGINS).map(
                |n| json!({"type":"alphaUnknownOrigin","originRegistryId":format!("{n:032x}")}),
            ))
        });
    assert!(target
        .import_complete_json_lines(&too_many)
        .unwrap_err()
        .to_string()
        .contains("record limit"));
    assert_eq!(state(&target), before);
    let full =
        rewrite(&empty, |_, rows| {
            rows.extend((0..MAX_UNKNOWN_ORIGINS).map(
                |n| json!({"type":"alphaUnknownOrigin","originRegistryId":format!("{n:032x}")}),
            ))
        });
    target.import_complete_json_lines(&full).unwrap();
    assert_eq!(
        target
            .import_complete_json_lines(&full)
            .unwrap()
            .unknown_origin_count,
        MAX_UNKNOWN_ORIGINS
    );
    let at_capacity = state(&target);
    let excess = rewrite(&empty, |_, rows| {
        rows.push(json!({"type":"alphaUnknownOrigin","originRegistryId":"f".repeat(32)}))
    });
    assert!(target.import_complete_json_lines(&excess).is_err());
    assert_eq!(state(&target), at_capacity);
}

#[test]
fn lost_provenance_with_unchanged_trial_chain_is_binding_rollback() {
    let (_dirs, ledger, _family) = setup();
    let source_dirs = Dirs::new();
    let source = source_dirs.open();
    ledger
        .import_json_lines(&source.export_json_lines().unwrap())
        .unwrap();
    let saved = observed(&ledger, None);
    {
        let conn = ledger.lock().unwrap();
        conn.execute_batch(
            "DROP TRIGGER alpha_unknown_origins_no_delete; DELETE FROM alpha_unknown_origins",
        )
        .unwrap();
    }
    assert_eq!(
        ledger.check_binding(Some(&saved)).unwrap(),
        BindingCheck::RegistryEvidenceRolledBack
    );
}

#[test]
fn a_new_collision_representative_preserves_older_conflict_ids() {
    let mut archives = Vec::new();
    let mut owners = Vec::new();
    for (key, batch) in [('a', 'c'), ('b', 'd'), ('e', 'f')] {
        let (dirs, ledger, family) = setup();
        let record = reserve(&ledger, &family, key, batch).reservation.unwrap();
        archives.push((
            record.reservation_id().to_string(),
            ledger.export_complete_json_lines().unwrap(),
        ));
        owners.push((dirs, ledger));
    }
    archives.sort_by(|a, b| a.0.cmp(&b.0));
    let dirs = Dirs::new();
    let target = dirs.open();
    target.import_complete_json_lines(&archives[2].1).unwrap();
    target.import_complete_json_lines(&archives[1].1).unwrap();
    let saved = observed(&target, None);
    assert_eq!(saved.alpha.as_ref().unwrap().conflicts.len(), 1);
    target.import_complete_json_lines(&archives[0].1).unwrap();
    let grown = observed(&target, Some(&saved));
    assert_eq!(grown.alpha.as_ref().unwrap().reservations.len(), 3);
    assert_eq!(grown.alpha.as_ref().unwrap().conflicts.len(), 3);
    let last_dirs = Dirs::new();
    let last = last_dirs.open();
    last.import_complete_json_lines(&target.export_complete_json_lines().unwrap())
        .unwrap();
    assert_eq!(observed(&last, None).alpha, grown.alpha);
}

#[test]
fn late_provenance_failure_rolls_back_already_staged_trial_and_alpha_rows() {
    let (_dirs, source, family) = setup();
    reserve(&source, &family, 'a', 'c');
    let legacy_dirs = Dirs::new();
    let legacy = legacy_dirs.open();
    source
        .import_json_lines(&legacy.export_json_lines().unwrap())
        .unwrap();
    let archive = source.export_complete_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    target.lock().unwrap().execute_batch("CREATE TRIGGER injected_provenance_failure BEFORE INSERT ON alpha_unknown_origins BEGIN SELECT RAISE(ABORT, 'injected provenance failure'); END").unwrap();
    assert!(target
        .import_complete_json_lines(&archive)
        .unwrap_err()
        .to_string()
        .contains("injected provenance failure"));
    assert_eq!(state(&target), before);
    assert_eq!(
        source
            .read_alpha_family(&family)
            .unwrap()
            .unwrap()
            .reservations
            .len(),
        1
    );
}

#[test]
fn a_valid_hash_gap_is_not_a_complete_source_and_corrupt_sources_cannot_export() {
    let (_dirs, source, family) = setup();
    let first = reserve(&source, &family, 'a', 'c').reservation.unwrap();
    reserve(&source, &family, 'b', 'd');
    let archive = source.export_complete_json_lines().unwrap();
    let missing = rewrite(&archive, |_, rows| {
        rows.retain(|row| {
            row["type"] != "alphaReservation"
                || row["record"]["reservationId"] != first.reservation_id()
        })
    });
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    assert!(target.import_complete_json_lines(&missing).is_err());
    assert_eq!(state(&target), before);
    {
        let conn = source.lock().unwrap();
        conn.execute_batch("DROP TRIGGER alpha_reservations_no_delete")
            .unwrap();
        conn.execute(
            "DELETE FROM alpha_reservations WHERE reservation_id = ?1",
            [first.reservation_id()],
        )
        .unwrap();
    }
    assert!(source.export_complete_json_lines().is_err());
}

#[test]
fn a_quarantined_family_without_local_rows_still_retains_imported_alpha() {
    let (_dirs, source, family) = setup();
    reserve(&source, &family, 'a', 'c');
    let saved = observed(&source, None);
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    target.lock().unwrap().execute("INSERT INTO family_conflicts (family_id,kind,detail_json,recorded_at) VALUES (?1,'test','{}','now')",[&family]).unwrap();
    let result = target
        .import_complete_json_lines(&source.export_complete_json_lines().unwrap())
        .unwrap();
    assert_eq!(result.trial.added_events, 0);
    assert_eq!(
        target
            .read_alpha_family(&family)
            .unwrap()
            .unwrap()
            .reservations
            .len(),
        1
    );
    assert_eq!(state(&target).1["family_conflicts"], 1);
    assert!(!matches!(
        target.check_binding(Some(&saved)).unwrap(),
        BindingCheck::Current(_)
    ));
}

#[test]
fn conflict_documents_cannot_substitute_for_missing_reservation_rows() {
    let (_dirs, source, family) = setup();
    let first = reserve(&source, &family, 'a', 'c').reservation.unwrap();
    reserve(&source, &family, 'a', 'd');
    let archive = source.export_complete_json_lines().unwrap();
    let missing = rewrite(&archive, |_, rows| {
        rows.retain(|row| {
            row["type"] != "alphaReservation"
                || row["record"]["reservationId"] != first.reservation_id()
        })
    });
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    assert!(target
        .import_complete_json_lines(&missing)
        .unwrap_err()
        .to_string()
        .contains("conflict references a missing"));
    assert_eq!(state(&target), before);
}

#[test]
fn oversized_alpha_and_conflict_expansion_are_rejected_before_trial_writes() {
    let (_dirs, source, family) = setup();
    let original = reserve(&source, &family, 'a', 'c').reservation.unwrap();
    let archive = source.export_complete_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    let before = state(&target);
    target.lock().unwrap().execute_batch("CREATE TRIGGER unexpected_trial_write BEFORE INSERT ON trial_events BEGIN SELECT RAISE(ABORT, 'unexpected trial write'); END").unwrap();
    let candidates:Vec<Value>=(0..MAX_ALPHA_ROWS).map(|n| {
        let mut body=original.document()["body"].clone();
        body["reservationKey"]=json!(format!("{n:032x}"));
        json!({"type":"alphaReservation","record":freeze_alpha_reservation(&body.to_string()).unwrap().document()})
    }).collect();
    let mut candidates = candidates;
    candidates.sort_by(|a, b| {
        a["record"]["reservationId"]
            .as_str()
            .cmp(&b["record"]["reservationId"].as_str())
    });
    for (length, expected) in [
        (MAX_ALPHA_ROWS as usize, "record limit"),
        (MAX_ALPHA_ROWS as usize - 1, "union alpha row limit"),
    ] {
        let oversized = rewrite(&archive, |_, rows| {
            rows.retain(|row| row["type"] != "alphaReservation");
            rows.extend(candidates[..length].iter().cloned());
        });
        let error = target.import_complete_json_lines(&oversized).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
        assert_eq!(state(&target), before);
    }
    let declaration = AlphaAllocationDeclaration {
        total_alpha_ppm: 500_000,
        schedule: vec![1; 140_000],
    };
    let oversized = rewrite(&archive, |_, rows| {
        rows.retain(|row| row["type"] != "alphaReservation");
        let row = rows
            .iter_mut()
            .find(|row| row["type"] == "alphaBudget")
            .unwrap();
        row["declaration"] = declaration.document();
        row["allocationId"] = json!(alpha_allocation_id(&declaration).unwrap());
    });
    assert!(target
        .import_complete_json_lines(&oversized)
        .unwrap_err()
        .to_string()
        .contains("document limit"));
    assert_eq!(state(&target), before);
}
