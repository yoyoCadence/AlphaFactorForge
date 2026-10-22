//! All four exact joint limits must reject proposals before receiver DML.
use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::freeze_manual_exposure;

fn fill(target: &TrialLedger, kind: &str) {
    let mut conn = target.lock().unwrap();
    // Keep the bounded fixture's interval index resident while all unmodified
    // per-row SQL capacity guards run. This is only this test connection.
    conn.execute_batch("PRAGMA cache_size=-65536").unwrap();
    let tx = conn.transaction().unwrap();
    if kind == "unknown" {
        for n in 0..10000 {
            tx.prepare_cached("INSERT INTO confirmation_unknown_origins VALUES (?1,'first-audit')")
                .unwrap()
                .execute([format!("f{n:031x}")])
                .unwrap();
        }
    } else {
        let (count, intervals) = match kind {
            "rows" => (10000, 1),
            "bytes" => (16, 1),
            _ => (1000, 128),
        };
        for n in 0..count {
            let intervals: Vec<_> = (0..intervals)
                .map(|i| json!({"fromMs":946684800000u64+i*2,"toExclusiveMs":946684800001u64+i*2}))
                .collect();
            let raw=freeze_manual_exposure(&json!({"contractVersion":"research-confirmation-exposure-v1","originRegistryId":target.registry_id(),"reservationKey":format!("{n:032x}"),"instrumentId":"crypto:binance:BTCUSDT","intervals":intervals,"claim":{"kind":"manual","accessKind":"raw","afterSealId":null}}).to_string()).unwrap();
            let mut text = raw.document().to_string();
            if kind == "bytes" {
                text.extend(std::iter::repeat_n(' ', MAX_DOCUMENT_BYTES - text.len()));
            }
            tx.prepare_cached("INSERT INTO confirmation_exposures VALUES (?1,'crypto:binance:BTCUSDT','manual',NULL,NULL,?2,'first-audit')").unwrap().execute(params![raw.id(),text]).unwrap();
            for (i, interval) in intervals.iter().enumerate() {
                tx.prepare_cached("INSERT INTO confirmation_intervals VALUES (?1,?2,'crypto:binance:BTCUSDT',?3,?4)").unwrap().execute(params![raw.id(),i as i64,interval["fromMs"].as_u64().unwrap() as i64,interval["toExclusiveMs"].as_u64().unwrap() as i64]).unwrap();
            }
        }
    }
    tx.commit().unwrap();
}
fn bounded(kind: &str) {
    let (_source_dirs, source) = managed();
    source
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            unknown_origins: vec!["1".repeat(32)],
            ..Default::default()
        })
        .unwrap();
    let bytes = source.export_witnessed_json_lines().unwrap();
    let empty_dirs = Dirs::new();
    let empty = empty_dirs.open();
    let zero = empty.export_witnessed_json_lines().unwrap();
    let target_dirs = Dirs::new();
    let target = target_dirs.open();
    // Establish the empty source genesis before measuring replay identities.
    target.import_witnessed_json_lines(&zero).unwrap();
    let started = std::time::Instant::now();
    fill(&target, kind);
    target.read_witnessed_confirmation_history().unwrap();
    match kind {
        "rows" => assert_eq!(rows(&target, "confirmation_exposures"), MAX_HISTORY_ROWS),
        "bytes" => assert_eq!(
            target
                .lock()
                .unwrap()
                .query_row(
                    "SELECT SUM(length(CAST(document_json AS BLOB))) FROM confirmation_exposures",
                    [],
                    |r| r.get::<_, usize>(0)
                )
                .unwrap(),
            MAX_HISTORY_BYTES
        ),
        "intervals" => assert_eq!(
            rows(&target, "confirmation_intervals"),
            MAX_HISTORY_INTERVALS
        ),
        _ => assert_eq!(rows(&target, "confirmation_unknown_origins"), 10000),
    }
    let identity = identities(&target);
    let no_op = target.import_witnessed_json_lines(&zero).unwrap();
    assert_eq!(
        no_op.confirmation.added_records
            + no_op.confirmation.added_blobs
            + no_op.confirmation.added_intervals
            + no_op.confirmation.added_unknown_origins,
        0
    );
    assert_eq!(identities(&target), identity);
    let before = (state(&target), changes(&target));
    assert!(matches!(
        target.import_witnessed_json_lines(&bytes).unwrap_err(),
        LedgerError::StateInvalid(_)
    ));
    assert_eq!(
        (state(&target), changes(&target)),
        before,
        "{kind}: no receiver DML, not merely rollback"
    );
    assert_eq!(rows(&target, "registry_imports"), 2);
    eprintln!("v5 exact {kind} preflight: {:?}", started.elapsed());
}
#[test]
fn exact_joint_record_capacity_and_overflow() {
    bounded("rows");
}
#[test]
fn exact_original_byte_capacity_and_overflow() {
    bounded("bytes");
}
#[test]
fn exact_joint_interval_capacity_and_overflow() {
    bounded("intervals");
}
#[test]
fn exact_unknown_origin_capacity_and_overflow() {
    bounded("unknown");
}
