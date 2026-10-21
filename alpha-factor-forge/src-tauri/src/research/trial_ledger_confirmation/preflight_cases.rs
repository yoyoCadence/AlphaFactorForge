//! The staging substrate must reproduce a real full schema-7 witnessed ledger.
use super::*;
use crate::research::trial_ledger::{self as ledger, preflight::with_snapshot};

fn installed() -> (Dirs, TrialLedger) {
    let (dirs, source, mut input, context) = fixture();
    let old = witnessed::original(&source, &context);
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
        .import_json_lines(&foreign.export_json_lines().unwrap())
        .unwrap();
    let complete = witnessed::original(&source, &context);
    input.blobs.extend([
        HistoryBlob::new(BlobKind::SourceWitness, old.text()).unwrap(),
        HistoryBlob::new(BlobKind::SourceWitness, complete.text()).unwrap(),
    ]);
    let from = json(&input.exposures[0]).unwrap()["body"]["intervals"][0]["fromMs"]
        .as_u64()
        .unwrap();
    input.exposures.push(cases::manual(
        source.registry_id(),
        'd',
        "raw",
        None,
        from,
        from + 1,
    ));
    source
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    (dirs, source)
}
fn snapshot(conn: &Connection) -> String {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut result = String::new();
    for table in tables {
        let mut stmt = conn
            .prepare(&format!("SELECT rowid,* FROM {table} ORDER BY rowid"))
            .unwrap();
        let n = stmt.column_count();
        let rows: Vec<Vec<SqlValue>> = stmt
            .query_map([], |r| (0..n).map(|i| r.get(i)).collect())
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        result.push_str(&format!("{table}:{rows:?}\n"));
    }
    result
}
#[test]
fn actual_complete_witnessed_snapshot_preserves_all_tables_audits_foreign_history_and_stops() {
    let (_dirs, source) = installed();
    let old = source.read_witnessed_confirmation_history().unwrap();
    assert!(old.is_restricted());
    let mut conn = source.lock().unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let before = snapshot(&tx);
    let changed: u64 = tx
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let path = with_snapshot(&tx, |private| {
        assert_eq!(snapshot(private), before);
        let checked = ledger::confirmation::load_witnessed_in(private, source.registry_id())?;
        assert_eq!(checked.inventory(), old.inventory());
        assert!(checked.is_restricted());
        let addition = ConfirmationInventory {
            exposures: vec![cases::manual(
                source.registry_id(),
                'a',
                "chart",
                None,
                946684800000,
                946684800001,
            )],
            ..Default::default()
        };
        let added =
            ledger::confirmation::append_witnessed_in(private, source.registry_id(), &addition)?;
        assert_eq!(added.added_records, 1);
        assert_eq!(
            ledger::confirmation::load_witnessed_in(private, source.registry_id())?
                .inventory()
                .exposures
                .len(),
            old.inventory().exposures.len() + 1
        );
        assert!(private
            .execute(
                "UPDATE confirmation_source_witnesses SET recorded_at='changed'",
                []
            )
            .is_err());
        Ok(PathBuf::from(private.path().unwrap())
            .parent()
            .unwrap()
            .to_owned())
    })
    .unwrap();
    assert!(!path.exists());
    assert_eq!(snapshot(&tx), before);
    assert_eq!(
        tx.query_row("SELECT total_changes()", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        changed
    );
    assert_eq!(
        ledger::confirmation::load_witnessed_in(&tx, source.registry_id())
            .unwrap()
            .inventory(),
        old.inventory()
    );
    tx.rollback().unwrap();
}
#[test]
fn copied_trigger_causes_late_private_union_failure_without_any_original_change() {
    let (_dirs, source) = installed();
    source.lock().unwrap().execute_batch("CREATE TRIGGER private_late_failure BEFORE INSERT ON confirmation_unknown_origins BEGIN SELECT RAISE(ABORT,'injected private provenance failure'); END").unwrap();
    let mut conn = source.lock().unwrap();
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let before = snapshot(&tx);
    let mut root = None;
    let result = with_snapshot(&tx, |private| {
        root = Some(
            PathBuf::from(private.path().unwrap())
                .parent()
                .unwrap()
                .to_owned(),
        );
        ledger::confirmation::append_witnessed_in(
            private,
            source.registry_id(),
            &ConfirmationInventory {
                exposures: vec![cases::manual(
                    source.registry_id(),
                    'a',
                    "chart",
                    None,
                    946684800000,
                    946684800001,
                )],
                unknown_origins: vec!["f".repeat(32)],
                ..Default::default()
            },
        )?;
        Ok(())
    });
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("injected private provenance failure"));
    assert!(!root.unwrap().exists());
    assert_eq!(snapshot(&tx), before);
    assert!(!tx.is_autocommit());
}
