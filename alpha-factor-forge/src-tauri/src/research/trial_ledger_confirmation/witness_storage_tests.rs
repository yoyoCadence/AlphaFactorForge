use super::*;

#[test]
fn corrupt_witness_metadata_bytes_or_cross_table_namespace_are_never_repaired() {
    for kind in 0..6 {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        let w = independent();
        ledger
            .restore_confirmation_source_witnesses(std::slice::from_ref(&w))
            .unwrap();
        {
            let conn = ledger.lock().unwrap();
            conn.execute_batch("PRAGMA ignore_check_constraints=ON; DROP TRIGGER confirmation_source_witnesses_no_update").unwrap();
            match kind {
                0=>conn.execute_batch("UPDATE confirmation_source_witnesses SET byte_length=byte_length+1").unwrap(),
                1=>conn.execute_batch("UPDATE confirmation_source_witnesses SET recorded_at=X'FF'").unwrap(),
                2=>conn.execute_batch("UPDATE confirmation_source_witnesses SET byte_length=1,content_bytes=X'FF'").unwrap(),
                3=>conn.execute("UPDATE confirmation_source_witnesses SET witness_sha256=?1",["F".repeat(64)]).map(|_|()).unwrap(),
                4=>conn.execute_batch("UPDATE confirmation_source_witnesses SET byte_length=8388609,content_bytes=zeroblob(8388609)").unwrap(),
                _=>{
                    conn.execute_batch("DROP TRIGGER confirmation_blobs_witness_namespace").unwrap();
                    conn.execute("INSERT INTO confirmation_blobs VALUES (?1,'binding',?2,?3,'collision')",params![w.sha256(),w.text().len() as i64,w.text().as_bytes()]).unwrap();
                }
            }
            conn.execute_batch("PRAGMA ignore_check_constraints=OFF")
                .unwrap();
        }
        let before = counts(&ledger);
        assert!(ledger.read_witnessed_confirmation_history().is_err());
        assert!(ledger.restore_confirmation_source_witnesses(&[w]).is_err());
        assert_eq!(counts(&ledger), before);
    }
}

#[test]
fn derived_unknown_overflow_fails_before_inserting_any_original_witness() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let w = independent();
    let source = crate::research::trial_ledger::transfer::reconstruct_source_witness(&w)
        .unwrap()
        .prefix()
        .registry_id
        .clone();
    let prefix = if source.starts_with('f') { 'e' } else { 'f' };
    {
        let mut conn = ledger.lock().unwrap();
        let tx = conn.transaction().unwrap();
        for n in 0..10000 {
            tx.execute(
                "INSERT INTO confirmation_unknown_origins VALUES (?1,'original-audit')",
                [format!("{prefix}{n:031x}")],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    assert!(ledger
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    assert!(ledger.restore_confirmation_source_witnesses(&[w]).is_err());
    assert_eq!(counts(&ledger), (0, 10000, 0));
}

#[test]
fn independent_concurrent_restorations_preserve_first_bytes_audit_and_single_identity() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let second = dirs.open();
    let w = independent();
    let barrier = std::sync::Barrier::new(2);
    let totals = std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            barrier.wait();
            ledger
                .restore_confirmation_source_witnesses(std::slice::from_ref(&w))
                .unwrap()
        });
        let b = scope.spawn(|| {
            barrier.wait();
            second
                .restore_confirmation_source_witnesses(std::slice::from_ref(&w))
                .unwrap()
        });
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(totals.0.added_blobs + totals.1.added_blobs, 1);
    assert_eq!(
        totals.0.added_unknown_origins + totals.1.added_unknown_origins,
        1
    );
    assert_eq!(counts(&ledger), (1, 1, 0));
    let graph = ledger.read_witnessed_confirmation_history().unwrap();
    assert_eq!(
        graph.inventory(),
        second
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory()
    );
    assert_eq!(graph.inventory().blobs[0].text(), w.text());
    let audit: String = ledger
        .lock()
        .unwrap()
        .query_row(
            "SELECT recorded_at FROM confirmation_source_witnesses",
            [],
            |r| r.get(0),
        )
        .unwrap();
    second.restore_confirmation_source_witnesses(&[w]).unwrap();
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
}
