use super::cases::{raw_connection, snapshot};
use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::freeze_manual_exposure;
use std::{sync::Barrier, time::Instant};
fn occupancy(registry: &str, key: u64) -> String {
    freeze_manual_exposure(&json!({"contractVersion":"research-confirmation-exposure-v1","originRegistryId":registry,"reservationKey":format!("{key:032x}"),"instrumentId":"crypto:binance:BTCUSDT","intervals":[{"fromMs":946684800000u64,"toExclusiveMs":946684800001u64}],"claim":{"kind":"manual","accessKind":"raw","afterSealId":null}}).to_string()).unwrap().document().to_string()
}

#[test]
fn independent_writers_serialize_replay_without_duplicate_records_or_intervals() {
    let (dirs, ledger, input, ctx) = fixture();
    let other = dirs.open();
    let barrier = Barrier::new(2);
    let (a, b) = std::thread::scope(|s| {
        let a = s.spawn(|| {
            barrier.wait();
            ledger.append_confirmation_history(&input, &ctx).unwrap()
        });
        let b = s.spawn(|| {
            barrier.wait();
            other.append_confirmation_history(&input, &ctx).unwrap()
        });
        (a.join().unwrap(), b.join().unwrap())
    });
    let mut writes = [a.added_records, b.added_records];
    writes.sort();
    assert_eq!(writes, [0, 13]);
    assert_eq!(a.added_intervals + b.added_intervals, 2);
    assert_eq!(a.added_blobs + b.added_blobs, 4);
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        other.read_confirmation_history(&ctx).unwrap().inventory()
    );
}

#[test]
fn exact_joint_byte_capacity_replays_and_overflow_is_refused_before_sql_mutation() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let ctx = GraphContext::default();
    let input = ConfirmationInventory {
        exposures: (0..16)
            .map(|n| {
                let mut raw = occupancy(ledger.registry_id(), n);
                raw.extend(std::iter::repeat_n(' ', MAX_DOCUMENT_BYTES - raw.len()));
                raw
            })
            .collect(),
        ..ConfirmationInventory::default()
    };
    let summary = ledger.append_confirmation_history(&input, &ctx).unwrap();
    assert_eq!(summary.added_records, 16);
    assert_eq!(summary.added_intervals, 16);
    let bytes: i64 = ledger
        .lock()
        .unwrap()
        .query_row(
            "SELECT SUM(length(CAST(document_json AS BLOB))) FROM confirmation_exposures",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bytes, MAX_HISTORY_BYTES as i64);
    let before = snapshot(&ledger);
    let c = raw_connection(&dirs);
    c.execute_batch("CREATE TRIGGER injected_first_mutation BEFORE INSERT ON confirmation_exposures BEGIN SELECT RAISE(ABORT,'should not reach SQL insertion'); END").unwrap();
    let delta = ConfirmationInventory {
        exposures: vec![occupancy(ledger.registry_id(), 16)],
        unknown_origins: vec!["e".repeat(32)],
        ..ConfirmationInventory::default()
    };
    assert!(matches!(
        ledger
            .append_confirmation_history(&delta, &ctx)
            .unwrap_err(),
        LedgerError::StateInvalid(_)
    ));
    assert_eq!(snapshot(&ledger), before);
    let replay = ledger
        .append_confirmation_history(&ConfirmationInventory::default(), &ctx)
        .unwrap();
    assert_eq!(
        replay.added_records
            + replay.added_blobs
            + replay.added_intervals
            + replay.added_unknown_origins,
        0
    );
}

#[test]
fn exact_ten_thousand_record_union_keeps_every_sql_backstop_and_replays_at_capacity() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let ctx = GraphContext::default();
    let input = ConfirmationInventory {
        exposures: (0..10000)
            .map(|n| occupancy(ledger.registry_id(), n))
            .collect(),
        ..ConfirmationInventory::default()
    };
    let started = Instant::now();
    let result = ledger.append_confirmation_history(&input, &ctx).unwrap();
    eprintln!("10000-record guarded union: {:?}", started.elapsed());
    assert_eq!(result.added_records, 10000);
    assert_eq!(result.added_intervals, 10000);
    let duplicate = ledger.append_confirmation_history(&input, &ctx).unwrap();
    assert_eq!(duplicate.added_records + duplicate.added_intervals, 0);
    let before = snapshot(&ledger);
    let delta = ConfirmationInventory {
        exposures: vec![occupancy(ledger.registry_id(), 10000)],
        ..ConfirmationInventory::default()
    };
    assert!(ledger.append_confirmation_history(&delta, &ctx).is_err());
    assert_eq!(snapshot(&ledger), before);
}

#[test]
fn oversized_corrupt_original_text_is_refused_by_preflight_with_bounded_diagnostics() {
    for blob in [false, true] {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        let c = raw_connection(&dirs);
        c.execute_batch("PRAGMA ignore_check_constraints=ON")
            .unwrap();
        if blob {
            let bytes = vec![b' '; MAX_BINDING_BYTES + 1];
            c.execute(
                "INSERT INTO confirmation_blobs VALUES (?1,'binding',?2,?3,'audit')",
                params!["f".repeat(64), bytes.len() as i64, bytes],
            )
            .unwrap();
        } else {
            let mut raw = occupancy(ledger.registry_id(), 0);
            let id = document_id(&json(&raw).unwrap()).unwrap().to_string();
            raw.extend(std::iter::repeat_n(' ', MAX_DOCUMENT_BYTES + 1 - raw.len()));
            c.execute("INSERT INTO confirmation_exposures VALUES (?1,'crypto:binance:BTCUSDT','manual',NULL,NULL,?2,'audit')",params![id,raw]).unwrap();
        }
        let err = ledger
            .read_confirmation_history(&GraphContext::default())
            .unwrap_err();
        assert!(err.to_string().contains(if blob {
            "invalid stored blob"
        } else {
            "invalid stored row"
        }));
    }
}
