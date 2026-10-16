use super::*;
fn occupancy(n: usize, intervals: usize) -> String {
    freeze_manual_exposure(&json!({"contractVersion":EXPOSURE_VERSION,"originRegistryId":"2".repeat(32),"reservationKey":format!("{n:032x}"),
        "instrumentId":"crypto:binance:BTCUSDT","intervals":(0..intervals).map(|j|json!({"fromMs":946684800000u64+2*j as u64,"toExclusiveMs":946684800001u64+2*j as u64})).collect::<Vec<_>>(),
        "claim":{"kind":"manual","accessKind":"raw","afterSealId":null}}).to_string()).unwrap().document().to_string()
}
#[test]
fn complete_inventory_accepts_exact_rows_then_refuses_new_conflict_without_truncating() {
    let mut i = ConfirmationInventory {
        exposures: (0..MAX_HISTORY_ROWS).map(|n| occupancy(n, 1)).collect(),
        ..ConfirmationInventory::default()
    };
    let context = GraphContext::default();
    assert_eq!(
        complete_confirmation_graph(&i, &context)
            .unwrap()
            .inventory()
            .exposures
            .len(),
        MAX_HISTORY_ROWS
    );
    let mut same_key = doc(&i.exposures[1]);
    same_key["body"]["reservationKey"] = doc(&i.exposures[0])["body"]["reservationKey"].clone();
    same_key["body"]["claim"]["accessKind"] = json!("chart");
    rehash(&mut same_key);
    i.exposures[1] = same_key.to_string();
    assert_eq!(
        complete_confirmation_graph(&i, &context).unwrap_err(),
        HistoryRecordError::Capacity
    );
    assert_eq!(i.exposures.len(), MAX_HISTORY_ROWS);
    assert!(i.conflicts.is_empty());
}
#[test]
fn exact_derived_interval_limit_and_one_over_are_checked() {
    let mut i = ConfirmationInventory {
        exposures: (0..1000).map(|n| occupancy(n, 128)).collect(),
        ..ConfirmationInventory::default()
    };
    let context = GraphContext::default();
    validate_confirmation_graph(&i, &context).unwrap();
    i.exposures.push(occupancy(1000, 1));
    assert_eq!(
        complete_confirmation_graph(&i, &context).unwrap_err(),
        HistoryRecordError::Capacity
    );
}
#[test]
fn manual_and_legacy_targets_do_not_gain_families_from_later_registration() {
    let (mut i, c) = val_only();
    i.batches.clear();
    i.operations.clear();
    i.admissions.clear();
    i.events.clear();
    i.blobs.clear();
    i.exposures.clear();
    let first = occupancy(1, 1);
    let mut legacy = doc(&first);
    legacy["body"]["claim"] =
        json!({"kind":"legacy_unknown","reason":"legacy_import","sourceEvidenceSha256":null});
    rehash(&mut legacy);
    i.exposures = vec![first, legacy.to_string()];
    let completed = complete_confirmation_graph(&i, &c).unwrap();
    assert_eq!(completed.inventory().conflicts.len(), 1);
    let conflict = doc(&completed.inventory().conflicts[0]);
    assert_eq!(conflict["body"]["kind"], "origin_key");
    assert_eq!(conflict["body"]["affectedFamilyIds"], json!([]));
    validate_confirmation_graph(completed.inventory(), &c).unwrap();
    let mut second = doc(&i.exposures[1]);
    second["body"]["reservationKey"] = json!("7".repeat(32));
    rehash(&mut second);
    i.exposures[1] = second.to_string();
    assert!(complete_confirmation_graph(&i, &c)
        .unwrap()
        .inventory()
        .conflicts
        .is_empty());
}
