//! Actual SQLite graphs reach the production observer/adoption path here.
use super::*;
use crate::research::trial_ledger::{
    read_workspace_binding, write_workspace_binding, BindingCheck, LedgerBinding,
};
use alpha_factor_forge::discovery_core::confirmation_history::{
    freeze_manual_exposure, freeze_source_witness, ConfirmationBindingSet as Set,
};

fn current(ledger: &TrialLedger, saved: Option<&LedgerBinding>) -> LedgerBinding {
    match ledger.check_binding(saved).unwrap() {
        BindingCheck::Current(binding) => binding,
        other => panic!("expected current, got {other:?}"),
    }
}
fn witness(ledger: &TrialLedger, context: &GraphContext) -> HistoryBlob {
    let mut conn = ledger.lock().unwrap();
    let tx = conn.transaction().unwrap();
    let bytes = ledger.export_trial_in(&tx, Some(8 * 1024 * 1024)).unwrap();
    let w = freeze_source_witness(
        std::str::from_utf8(&bytes).unwrap(),
        &context.registered_sources[0],
    )
    .unwrap();
    HistoryBlob::new(BlobKind::SourceWitness, w.text()).unwrap()
}
fn manual(origin: &str, key: char, from: u64, to: u64) -> String {
    freeze_manual_exposure(
        &json!({"contractVersion":"research-confirmation-exposure-v1",
        "originRegistryId":origin,"reservationKey":key.to_string().repeat(32),
        "instrumentId":"crypto:binance:BTCUSDT","intervals":[{"fromMs":946684800000u64+from,"toExclusiveMs":946684800000u64+to}],
        "claim":{"kind":"manual","accessKind":"chart","afterSealId":null}})
        .to_string(),
    )
    .unwrap()
    .document()
    .to_string()
}
fn settings() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(
        "CREATE TABLE app_settings (key TEXT PRIMARY KEY,value_json TEXT NOT NULL,updated_at TEXT)",
    )
    .unwrap();
    c
}
fn raw(conn: &Connection) -> String {
    conn.query_row(
        "SELECT value_json FROM app_settings WHERE key='trial_ledger_binding'",
        [],
        |r| r.get(0),
    )
    .unwrap()
}
fn parse_saved(conn: &Connection, value: &Value) -> LedgerBinding {
    conn.execute(
        "UPDATE app_settings SET value_json=?1 WHERE key='trial_ledger_binding'",
        [value.to_string()],
    )
    .unwrap();
    read_workspace_binding(conn).unwrap().unwrap()
}

#[test]
fn binding_observes_every_exact_identity_and_witness_without_writes_or_repairs() {
    let (dirs, ledger, mut input, context) = fixture();
    input.blobs.push(witness(&ledger, &context));
    let from = json(&input.exposures[0]).unwrap()["body"]["intervals"][0]["fromMs"]
        .as_u64()
        .unwrap()
        - 946684800000;
    input
        .exposures
        .push(manual(ledger.registry_id(), 'a', from, from + 1));
    input
        .exposures
        .sort_by_key(|v| json(v).unwrap()["id"].as_str().unwrap().to_owned());
    input.unknown_origins.push("e".repeat(32));
    ledger
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    let before = ledger
        .read_witnessed_confirmation_history()
        .unwrap()
        .inventory()
        .clone();
    let saved = current(&ledger, None);
    let confirmation = saved.confirmation.as_ref().unwrap();
    for (section, documents) in [
        (Set::Batches, &before.batches),
        (Set::Selections, &before.selections),
        (Set::Operations, &before.operations),
        (Set::Exposures, &before.exposures),
        (Set::Admissions, &before.admissions),
        (Set::Events, &before.events),
        (Set::Conflicts, &before.conflicts),
    ] {
        assert!(!documents.is_empty(), "fixture covers {section:?}");
        assert_eq!(confirmation.ids(section), ids(documents));
    }
    let mut blobs: Vec<_> = before
        .blobs
        .iter()
        .map(|b| b.sha256().to_string())
        .collect();
    blobs.sort();
    assert_eq!(confirmation.ids(Set::Blobs), blobs);
    assert_eq!(
        confirmation.ids(Set::UnknownOrigins),
        before.unknown_origins
    );
    assert_eq!(current(&ledger, Some(&saved)), saved);
    assert_eq!(
        ledger
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &before
    );
    let independent = dirs.open();
    assert_eq!(current(&independent, Some(&saved)), saved);
    drop(independent);
    drop(ledger);
    assert_eq!(current(&dirs.open(), Some(&saved)), saved);
}

#[test]
fn every_section_requires_exact_coverage_even_with_equal_counts_and_unchanged_prefix() {
    let (_dirs, ledger, mut input, context) = fixture();
    input.blobs.push(witness(&ledger, &context));
    let from = json(&input.exposures[0]).unwrap()["body"]["intervals"][0]["fromMs"]
        .as_u64()
        .unwrap()
        - 946684800000;
    input
        .exposures
        .push(manual(ledger.registry_id(), 'a', from, from + 1));
    input
        .exposures
        .sort_by_key(|v| json(v).unwrap()["id"].as_str().unwrap().to_owned());
    input.unknown_origins.push("e".repeat(32));
    ledger
        .append_witnessed_confirmation_history(&input)
        .unwrap();
    let saved = current(&ledger, None);
    let conn = settings();
    write_workspace_binding(&conn, &saved).unwrap();
    let original: Value = serde_json::from_str(&raw(&conn)).unwrap();
    for section in [
        "batches",
        "selections",
        "operations",
        "exposures",
        "admissions",
        "events",
        "blobs",
        "conflicts",
        "unknownOrigins",
    ] {
        let mut changed = original.clone();
        let values = changed["confirmation"][section].as_array_mut().unwrap();
        let count = values.len();
        assert!(count > 0, "nonempty {section}");
        let replacement = if section == "unknownOrigins" {
            "f".repeat(32)
        } else {
            "f".repeat(64)
        };
        assert!(!values.contains(&json!(replacement)));
        values[0] = json!(replacement);
        values.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        assert_eq!(values.len(), count);
        let saved = parse_saved(&conn, &changed);
        assert_eq!(
            ledger.check_binding(Some(&saved)).unwrap(),
            BindingCheck::RegistryEvidenceRolledBack,
            "{section}"
        );
        assert_eq!(read_workspace_binding(&conn).unwrap(), Some(saved));
    }
}

#[test]
fn valid_manual_history_loss_and_equal_sized_substitution_are_rollbacks() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let exposure = manual(ledger.registry_id(), 'a', 1, 2);
    ledger
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            exposures: vec![exposure.clone()],
            ..Default::default()
        })
        .unwrap();
    let saved = current(&ledger, None);
    {
        let c = ledger.lock().unwrap();
        c.execute_batch("DROP TRIGGER confirmation_intervals_no_delete; DROP TRIGGER confirmation_exposures_no_delete; DELETE FROM confirmation_intervals; DELETE FROM confirmation_exposures").unwrap();
    }
    assert!(
        ledger.read_witnessed_confirmation_history().is_ok(),
        "valid empty graph"
    );
    assert_eq!(
        ledger.check_binding(Some(&saved)).unwrap(),
        BindingCheck::RegistryEvidenceRolledBack
    );
    ledger
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            exposures: vec![manual(ledger.registry_id(), 'b', 1, 2)],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        current(&ledger, None)
            .confirmation
            .as_ref()
            .unwrap()
            .ids(Set::Exposures)
            .len(),
        1
    );
    assert_eq!(
        ledger.check_binding(Some(&saved)).unwrap(),
        BindingCheck::RegistryEvidenceRolledBack
    );
}

#[test]
fn observer_refuses_incomplete_managed_proofs_even_with_unknown_markers_and_never_restores() {
    let (_dirs, ledger, input, context) = fixture();
    let saved = current(&ledger, None);
    ledger
        .append_confirmation_history(&input, &context)
        .unwrap();
    ledger
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO confirmation_unknown_origins VALUES (?1,'old-audit')",
            [ledger.registry_id()],
        )
        .unwrap();
    let conn = settings();
    write_workspace_binding(&conn, &saved).unwrap();
    let before = raw(&conn);
    let error = ledger.check_binding(Some(&saved)).unwrap_err();
    assert_eq!(error.code(), "registry_state_invalid");
    assert_eq!(raw(&conn), before);
    assert_eq!(
        ledger
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
    assert_eq!(
        ledger
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM confirmation_unknown_origins",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn corrupt_original_bytes_and_sql_projections_are_invalid_state_instead_of_rollback() {
    for corrupt in ["DROP TRIGGER confirmation_source_witnesses_no_update; UPDATE confirmation_source_witnesses SET content_bytes=zeroblob(byte_length)",
        "DROP TRIGGER confirmation_operations_no_update; UPDATE confirmation_operations SET workspace_id='ffffffffffffffffffffffffffffffff'",
        "DROP TRIGGER confirmation_intervals_no_delete; DELETE FROM confirmation_intervals"] {
        let (_dirs,ledger,mut input,context)=fixture();
        input.blobs.push(witness(&ledger,&context));
        ledger.append_witnessed_confirmation_history(&input).unwrap();
        let saved=current(&ledger,None);
        let conn=settings();
        write_workspace_binding(&conn,&saved).unwrap();
        let before=raw(&conn);
        ledger.lock().unwrap().execute_batch(corrupt).unwrap();
        assert_eq!(ledger.check_binding(Some(&saved)).unwrap_err().code(),"registry_state_invalid");
        assert_eq!(raw(&conn),before);
    }
}

#[test]
fn incomplete_legacy_adoption_preserves_last_good_then_explicit_restoration_allows_v5() {
    use crate::research::trial_ledger_workspace::{adopt, registry_dir};
    let (dirs, ledger, input, context) = fixture();
    let proof = witness(&ledger, &context);
    let mut conn = crate::db::open_at(&dirs.workspace.join(crate::db::DB_FILE_NAME)).unwrap();
    let workspace = crate::db::runtime_ledger::workspace_id(&conn).unwrap();
    write_workspace_binding(&conn, &current(&ledger, None)).unwrap();
    let before = raw(&conn);
    ledger
        .append_confirmation_history(&input, &context)
        .unwrap();
    drop(ledger);
    let target = registry_dir(&dirs.workspace).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(target.clone());
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(
        dirs.registry.join(REGISTRY_FILE_NAME),
        target.join(REGISTRY_FILE_NAME),
    )
    .unwrap();
    let error = adopt(&mut conn, &dirs.workspace, &workspace, None)
        .err()
        .expect("missing proof blocks actual adoption");
    assert!(
        error.to_string().contains("registry_state_invalid"),
        "{error}"
    );
    assert_eq!(raw(&conn), before);
    let ledger = TrialLedger::open_existing(&target, &dirs.workspace).unwrap();
    let proof = alpha_factor_forge::discovery_core::confirmation_history::parse_source_witness(
        proof.text(),
    )
    .unwrap();
    ledger
        .restore_confirmation_source_witnesses(&[proof])
        .unwrap();
    drop(ledger);
    let adopted = adopt(&mut conn, &dirs.workspace, &workspace, None).unwrap();
    assert_eq!(
        read_workspace_binding(&conn).unwrap(),
        Some(current(&adopted.ledger, None))
    );
    assert_eq!(
        serde_json::from_str::<Value>(&raw(&conn)).unwrap()["version"],
        "trial-ledger-binding-v5"
    );
}

#[test]
fn standalone_witness_and_permanent_unknown_are_both_observed_and_cannot_disappear() {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-source-witness-v1.json"
    ))
    .unwrap();
    let proof = alpha_factor_forge::discovery_core::confirmation_history::parse_source_witness(
        fixture["cases"][0]["witnessText"].as_str().unwrap(),
    )
    .unwrap();
    ledger
        .restore_confirmation_source_witnesses(std::slice::from_ref(&proof))
        .unwrap();
    let saved = current(&ledger, None);
    assert_eq!(
        saved.confirmation.as_ref().unwrap().ids(Set::Blobs),
        [proof.sha256()]
    );
    assert!(!saved
        .confirmation
        .as_ref()
        .unwrap()
        .ids(Set::UnknownOrigins)
        .is_empty());
    assert!(ledger
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    ledger.lock().unwrap().execute_batch("DROP TRIGGER confirmation_source_witnesses_no_delete; DELETE FROM confirmation_source_witnesses").unwrap();
    assert!(ledger.read_witnessed_confirmation_history().is_ok());
    assert_eq!(
        ledger.check_binding(Some(&saved)).unwrap(),
        BindingCheck::RegistryEvidenceRolledBack
    );
    let unknown_only = current(&ledger, None);
    ledger.lock().unwrap().execute_batch("DROP TRIGGER confirmation_unknown_origins_no_delete; DELETE FROM confirmation_unknown_origins").unwrap();
    assert!(ledger.read_witnessed_confirmation_history().is_ok());
    assert_eq!(
        ledger.check_binding(Some(&unknown_only)).unwrap(),
        BindingCheck::RegistryEvidenceRolledBack
    );
}
