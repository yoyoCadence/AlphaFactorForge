//! Real schema-7 sources use the public complete exporter, never caller facts.
use super::*;
use crate::research::trial_ledger::{self as ledger, BindingCheck};

fn frame(bytes: &[u8]) -> (Value, Vec<Value>) {
    let text = std::str::from_utf8(bytes).unwrap();
    assert!(text.ends_with('\n'));
    let (header, body) = text.split_once('\n').unwrap();
    let header: Value = serde_json::from_str(header).unwrap();
    assert_eq!(header["version"], "trial-ledger-export-v5");
    assert_eq!(header["schemaVersion"], "0007_confirmation_source_witness");
    assert_eq!(header["bodySha256"], ledger::sha256_hex(body.as_bytes()));
    let rows = body
        .lines()
        .map(|line| {
            let value: Value = serde_json::from_str(line).unwrap();
            assert_eq!(ledger::canonical_text(&value).unwrap(), line);
            value
        })
        .collect();
    (header, rows)
}
fn install(ledger: &TrialLedger, mut inventory: ConfirmationInventory, context: &GraphContext) {
    let proof = witnessed::original(ledger, context);
    inventory
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, proof.text()).unwrap());
    ledger
        .append_witnessed_confirmation_history(&inventory)
        .unwrap();
}
fn total_changes(ledger: &TrialLedger) -> u64 {
    ledger
        .lock()
        .unwrap()
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn actual_complete_export_keeps_all_nine_sections_three_blob_kinds_and_original_bytes() {
    let (_dirs, source, mut input, context) = fixture();
    let from = json(&input.exposures[0]).unwrap()["body"]["intervals"][0]["fromMs"]
        .as_u64()
        .unwrap();
    input.exposures.push(cases::manual(
        source.registry_id(),
        'a',
        "raw",
        None,
        from,
        from + 1,
    ));
    input
        .exposures
        .sort_by_key(|v| json(v).unwrap()["id"].as_str().unwrap().to_owned());
    input.unknown_origins.push("e".repeat(32));
    install(&source, input, &context);
    let before = source
        .read_witnessed_confirmation_history()
        .unwrap()
        .inventory()
        .clone();
    let changes = total_changes(&source);
    let (header, rows) = frame(&source.export_witnessed_json_lines().unwrap());
    for (name, kind, expected) in [
        ("batches", "confirmationBatch", before.batches.len()),
        (
            "selections",
            "confirmationSelection",
            before.selections.len(),
        ),
        (
            "operations",
            "confirmationOperation",
            before.operations.len(),
        ),
        ("exposures", "confirmationExposure", before.exposures.len()),
        (
            "admissions",
            "confirmationAdmission",
            before.admissions.len(),
        ),
        ("events", "confirmationEvent", before.events.len()),
        ("conflicts", "confirmationConflict", before.conflicts.len()),
        ("blobs", "confirmationBlob", before.blobs.len()),
        (
            "unknownOrigins",
            "confirmationUnknownOrigin",
            before.unknown_origins.len(),
        ),
    ] {
        assert!(expected > 0, "nonempty {name}");
        assert_eq!(header["counts"][name], expected);
        assert_eq!(rows.iter().filter(|r| r["type"] == kind).count(), expected);
    }
    let rank = |r: &Value| {
        [
            "alphaLedger",
            "confirmationBatch",
            "confirmationSelection",
            "confirmationOperation",
            "confirmationExposure",
            "confirmationAdmission",
            "confirmationEvent",
            "confirmationConflict",
            "confirmationBlob",
            "confirmationUnknownOrigin",
        ]
        .iter()
        .position(|k| r["type"] == *k)
        .unwrap()
    };
    let keys: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                rank(r),
                r["record"]["id"]
                    .as_str()
                    .or(r["sha256"].as_str())
                    .or(r["originRegistryId"].as_str())
                    .unwrap_or("")
                    .to_owned(),
            )
        })
        .collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]));
    for blob in &before.blobs {
        let row = rows.iter().find(|r| r["sha256"] == blob.sha256()).unwrap();
        assert_eq!(
            row["text"].as_str().unwrap().as_bytes(),
            blob.text().as_bytes()
        );
        assert_eq!(
            ledger::sha256_hex(row["text"].as_str().unwrap().as_bytes()),
            blob.sha256()
        );
        assert_eq!(
            row["kind"],
            match blob.kind() {
                BlobKind::Binding => "binding",
                BlobKind::Statistics => "statistics",
                BlobKind::SourceWitness => "source_witness",
            }
        );
    }
    assert_eq!(header["registryId"], source.registry_id());
    assert_eq!(total_changes(&source), changes);
    assert_eq!(
        source
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &before
    );
    assert!(source.export_complete_json_lines().is_err());
    assert!(source.export_json_lines().is_err());
}

#[test]
fn empty_manual_and_permanent_unknown_exports_remain_readable_with_legacy_guards() {
    let dirs = Dirs::new();
    let source = dirs.open();
    let (_, rows) = frame(&source.export_witnessed_json_lines().unwrap());
    assert_eq!(rows.len(), 1);
    assert!(source.export_complete_json_lines().is_ok());
    source
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            exposures: vec![cases::manual(
                source.registry_id(),
                'a',
                "chart",
                None,
                946684800000,
                946684800001,
            )],
            unknown_origins: vec!["e".repeat(32)],
            ..Default::default()
        })
        .unwrap();
    let (header, rows) = frame(&source.export_witnessed_json_lines().unwrap());
    assert_eq!(header["counts"]["exposures"], 1);
    assert_eq!(header["counts"]["unknownOrigins"], 1);
    assert!(rows.iter().any(|r| r["originRegistryId"] == "e".repeat(32)));
    assert!(matches!(
        source.check_binding(None).unwrap(),
        BindingCheck::Current(_)
    ));
    assert!(source.export_complete_json_lines().is_err());
    assert!(source.export_json_lines().is_err());
}

#[test]
fn corrupt_or_incomplete_history_never_publishes_or_repairs_an_archive() {
    for corruption in [
        "DROP TRIGGER confirmation_source_witnesses_no_update; UPDATE confirmation_source_witnesses SET content_bytes=zeroblob(byte_length)",
        "DROP TRIGGER confirmation_source_witnesses_no_delete; DELETE FROM confirmation_source_witnesses",
        "DROP TRIGGER confirmation_operations_no_update; UPDATE confirmation_operations SET workspace_id='ffffffffffffffffffffffffffffffff'",
        "DROP TRIGGER confirmation_intervals_no_delete; DELETE FROM confirmation_intervals",
        "DROP TRIGGER trial_events_no_update; UPDATE trial_events SET chain='bad' WHERE seq=1",
    ] {
        let (_dirs,source,input,context)=fixture();
        install(&source,input,&context);
        source.lock().unwrap().execute_batch(corruption).unwrap();
        let before=total_changes(&source);
        assert_eq!(source.export_witnessed_json_lines().err().map(|e|e.code()),Some("registry_state_invalid"));
        assert_eq!(total_changes(&source),before);
    }
    let (_dirs, source, input, context) = fixture();
    source
        .append_confirmation_history(&input, &context)
        .unwrap();
    source
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO confirmation_unknown_origins VALUES (?1,'original')",
            [source.registry_id()],
        )
        .unwrap();
    assert!(
        source.export_witnessed_json_lines().is_err(),
        "unknown never replaces missing original proof"
    );
    assert_eq!(
        source
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM confirmation_source_witnesses",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn one_deferred_snapshot_keeps_trial_alpha_and_confirmation_together_during_other_writes() {
    let (dirs, source, input, context) = fixture();
    install(&source, input, &context);
    let mut conn = source.lock().unwrap();
    let tx = conn.transaction().unwrap();
    let before: i64 = tx
        .query_row("SELECT COUNT(*) FROM trial_events", [], |r| r.get(0))
        .unwrap();
    let other = dirs.open();
    register(&other, vec![event("later", 0, "later-strategy")]);
    {
        let mut c = other.lock().unwrap();
        let write = c.transaction().unwrap();
        ledger::alpha::record_unknown_origins(&write, &BTreeSet::from(["e".repeat(32)])).unwrap();
        record_unknown_origins(&write, &BTreeSet::from(["e".repeat(32)])).unwrap();
        write.commit().unwrap();
    }
    let old = ledger::confirmation::archive::export_in(&source, &tx).unwrap();
    let (old_header, old_rows) = frame(&old);
    let alpha = old_rows[0]["export"].as_str().unwrap();
    let alpha_rows: Vec<Value> = alpha
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let trial = alpha_rows[1]["export"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(trial.lines().next().unwrap()).unwrap()["headSeq"],
        before
    );
    assert_eq!(alpha_rows[0]["unknownOriginCount"], 0);
    assert_eq!(old_header["counts"]["unknownOrigins"], 0);
    tx.commit().unwrap();
    drop(conn);
    let (now, rows) = frame(&source.export_witnessed_json_lines().unwrap());
    let alpha_rows: Vec<Value> = rows[0]["export"]
        .as_str()
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(alpha_rows[0]["unknownOriginCount"], 1);
    let trial = alpha_rows[1]["export"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(trial.lines().next().unwrap()).unwrap()["headSeq"],
        before + 1
    );
    assert_eq!(now["counts"]["unknownOrigins"], 1);
}

#[test]
fn reopen_retains_original_records_blobs_and_stops_without_changing_audits() {
    let (dirs, source, input, context) = fixture();
    install(&source, input, &context);
    let original = source
        .read_witnessed_confirmation_history()
        .unwrap()
        .inventory()
        .clone();
    let (_, before) = frame(&source.export_witnessed_json_lines().unwrap());
    let audit: String = source
        .lock()
        .unwrap()
        .query_row(
            "SELECT recorded_at FROM confirmation_source_witnesses",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(source);
    let reopened = dirs.open();
    let (_, after) = frame(&reopened.export_witnessed_json_lines().unwrap());
    assert_eq!(&before[1..], &after[1..]);
    assert_eq!(
        reopened
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &original
    );
    assert_eq!(
        reopened
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
