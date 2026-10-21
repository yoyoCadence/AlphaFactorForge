use super::*;
use alpha_factor_forge::discovery_core::{
    confirmation_contracts::parse_validation_batch,
    confirmation_history::{freeze_conflict_declaration, freeze_manual_exposure},
};

#[test]
fn schema_six_never_persists_a_source_witness_into_the_old_blob_table() {
    let (_dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let before = snapshot(&ledger);
    let old = ledger
        .read_confirmation_history(&ctx)
        .unwrap()
        .inventory()
        .clone();
    let f: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-source-witness-v1.json"
    ))
    .unwrap();
    let proposed = ConfirmationInventory {
        blobs: vec![HistoryBlob::new(
            BlobKind::SourceWitness,
            f["cases"][0]["witnessText"].as_str().unwrap(),
        )
        .unwrap()],
        ..ConfirmationInventory::default()
    };
    assert!(ledger.append_confirmation_history(&proposed, &ctx).is_err());
    assert_eq!(snapshot(&ledger), before);
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        &old
    );
}
pub(super) fn raw_connection(dirs: &Dirs) -> Connection {
    Connection::open(dirs.registry.join(REGISTRY_FILE_NAME)).unwrap()
}
pub(super) fn snapshot(ledger: &TrialLedger) -> (Vec<(String, i64)>, (u64, String)) {
    let mut conn = ledger.lock().unwrap();
    let c = conn.transaction().unwrap();
    let counts = TABLES
        .into_iter()
        .map(|t| {
            (
                t.into(),
                c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                    .unwrap(),
            )
        })
        .collect();
    (counts, ledger::head(&c, ledger.registry_id()).unwrap())
}
pub(super) fn fork(
    input: &ConfirmationInventory,
    ctx: &GraphContext,
    request: char,
) -> ConfirmationInventory {
    let batch = parse_validation_batch(&input.batches[0], &ctx.registered_sources[0]).unwrap();
    let mut body = json(&input.operations[0]).unwrap()["body"].clone();
    body["requestId"] = json!(request.to_string().repeat(32));
    let op = freeze_operation(&body.to_string(), &batch).unwrap();
    let mut body = json(&input.exposures[0]).unwrap()["body"].clone();
    body["claim"]["operationId"] = json!(op.id());
    let exposure =
        freeze_confirmation_exposure(&body.to_string(), &batch, &op, &ctx.reservations[0]).unwrap();
    let mut body = json(&input.admissions[0]).unwrap()["body"].clone();
    body["operationId"] = json!(op.id());
    body["exposureId"] = json!(exposure.id());
    let sha = body["registryObservation"]["bindingSha256"]
        .as_str()
        .unwrap();
    let blob = input.blobs.iter().find(|b| b.sha256() == sha).unwrap();
    let a = freeze_admission(
        &body.to_string(),
        &AdmissionInputs {
            batch: &batch,
            operation: &op,
            reservation: &ctx.reservations[0],
            exposure: &exposure,
            binding: blob,
            trial: &ctx.historical_trials[0],
            allocation: &ctx.allocations[0].declaration,
        },
    )
    .unwrap();
    ConfirmationInventory {
        operations: vec![op.document().to_string()],
        exposures: vec![exposure.document().to_string()],
        admissions: vec![a.document().to_string()],
        ..ConfirmationInventory::default()
    }
}
pub(super) fn manual(origin: &str, key: char, kind: &str, seal: Option<&str>, from: u64, to: u64) -> String {
    freeze_manual_exposure(&json!({"contractVersion":"research-confirmation-exposure-v1","originRegistryId":origin,"reservationKey":key.to_string().repeat(32),"instrumentId":"crypto:binance:BTCUSDT","intervals":[{"fromMs":from,"toExclusiveMs":to}],"claim":{"kind":"manual","accessKind":kind,"afterSealId":seal}}).to_string()).unwrap().document().to_string()
}

#[test]
fn original_origin_lookup_survives_receiver_and_current_family_growth() {
    let (_dirs, source, input, ctx) = fixture();
    let archive = source.export_complete_json_lines().unwrap();
    let dirs = Dirs::new();
    let receiver = dirs.open();
    receiver.import_complete_json_lines(&archive).unwrap();
    register(&receiver, vec![event("later", 0, "later-0")]);
    let Admission::Count(current) = receiver
        .read_admission_count(&ctx.historical_trials[0].trial_batch_id)
        .unwrap()
    else {
        panic!("current count")
    };
    assert_eq!(current.family_effective_trials(), 5);
    receiver.append_confirmation_history(&input, &ctx).unwrap();
    let op = json(&input.operations[0]).unwrap();
    let original = op["body"]["originRegistryId"].as_str().unwrap();
    let key = op["body"]["reservationKey"].as_str().unwrap();
    let found = receiver
        .find_original_confirmation_operations(original, key, &ctx)
        .unwrap();
    assert_eq!(found.operation_ids(), [document_id(&op).unwrap()]);
    assert!(found.graph().is_restricted());
    assert!(found
        .graph()
        .inventory()
        .unknown_origins
        .contains(&source.registry_id().to_string()));
    assert!(receiver
        .find_original_confirmation_operations(receiver.registry_id(), key, &ctx)
        .unwrap()
        .operation_ids()
        .is_empty());
    let ids = found.operation_ids().to_vec();
    drop(receiver);
    assert_eq!(
        dirs.open()
            .find_original_confirmation_operations(original, key, &ctx)
            .unwrap()
            .operation_ids(),
        ids
    );
}

#[test]
fn current_trial_alpha_and_provenance_observations_cannot_be_truncated_or_substituted() {
    let (_dirs, ledger, input, ctx) = fixture();
    let saved = snapshot(&ledger);
    for case in 0..8 {
        let mut c = ctx.clone();
        match case {
            0 => c.family_tests.clear(),
            1 => {
                c.reservations.pop();
            }
            2 => c.allocations.clear(),
            3 => c.alpha_unknown_origins.push("e".repeat(32)),
            4 => c
                .quarantined_families
                .push(c.registered_sources[0].source.family_id.clone()),
            5 => c.conflicted_origins.push("e".repeat(32)),
            6 => {
                c.registered_sources[0].candidates.pop();
            }
            _ => c.historical_trials[0].family_effective_trials = 3,
        };
        assert!(ledger.append_confirmation_history(&input, &c).is_err());
        assert_eq!(snapshot(&ledger), saved);
    }
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let mut c = ctx.clone();
    c.reservations.pop();
    assert!(ledger.read_confirmation_history(&c).is_err());
}

#[test]
fn duplicate_replay_preserves_first_text_and_audit_but_validates_every_incoming_text() {
    let (_dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let saved = ledger.read_confirmation_history(&ctx).unwrap();
    let before: Vec<(String, String)> = {
        let c = ledger.lock().unwrap();
        let mut s=c.prepare("SELECT operation_id,recorded_at FROM confirmation_operations ORDER BY operation_id").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let mut delta = ConfirmationInventory::default();
    delta
        .operations
        .push(serde_json::to_string_pretty(&json(&input.operations[0]).unwrap()).unwrap());
    assert_eq!(
        ledger
            .append_confirmation_history(&delta, &ctx)
            .unwrap()
            .added_records,
        0
    );
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        saved.inventory()
    );
    let origin = json(&input.operations[0]).unwrap()["body"]["originRegistryId"]
        .as_str()
        .unwrap()
        .to_string();
    let token = format!("\"originRegistryId\":\"{origin}\"");
    delta.operations[0] = input.operations[0].replace(&token, &format!("{token},{token}"));
    assert_ne!(delta.operations[0], input.operations[0]);
    assert!(ledger.append_confirmation_history(&delta, &ctx).is_err());
    delta.operations = vec![input.operations[0].clone(), input.operations[0].clone()];
    assert!(ledger.append_confirmation_history(&delta, &ctx).is_err());
    let mut unknown = json(&input.operations[0]).unwrap();
    unknown["privateNote"] = json!("PROTECTED_REJECTED_SENTINEL");
    delta.operations = vec![unknown.to_string()];
    assert!(!ledger
        .append_confirmation_history(&delta, &ctx)
        .unwrap_err()
        .to_string()
        .contains("PROTECTED"));
    delta.operations.clear();
    delta.blobs = vec![input.blobs[0].clone(), input.blobs[0].clone()];
    assert!(ledger.append_confirmation_history(&delta, &ctx).is_err());
    let after: Vec<(String, String)> = {
        let c = ledger.lock().unwrap();
        let mut s=c.prepare("SELECT operation_id,recorded_at FROM confirmation_operations ORDER BY operation_id").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    assert_eq!(after, before);
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        saved.inventory()
    );
}

#[test]
fn contradictory_operations_keep_original_claims_and_generate_actual_stops() {
    let (_dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let delta = fork(&input, &ctx, 'f');
    let summary = ledger.append_confirmation_history(&delta, &ctx).unwrap();
    assert_eq!(summary.added_records, 5);
    assert_eq!(summary.added_intervals, 1);
    let op = json(&input.operations[0]).unwrap();
    let history = ledger
        .find_original_confirmation_operations(
            op["body"]["originRegistryId"].as_str().unwrap(),
            op["body"]["reservationKey"].as_str().unwrap(),
            &ctx,
        )
        .unwrap();
    assert_eq!(history.operation_ids().len(), 2);
    assert!(history.graph().is_restricted());
    assert_eq!(history.graph().inventory().admissions.len(), 3);
    assert_eq!(history.graph().inventory().conflicts.len(), 2);
    assert!(history
        .graph()
        .inventory()
        .conflicts
        .iter()
        .any(|r| json(r).unwrap()["body"]["kind"] == "origin_key"));
    assert!(history
        .graph()
        .inventory()
        .conflicts
        .iter()
        .any(|r| json(r).unwrap()["body"]["kind"] == "exposure_overlap"));
}

#[test]
fn old_witnesses_survive_a_new_representative_and_manual_targets_never_gain_families() {
    let (_dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let mut claims = ["raw", "chart", "worker"].map(|kind| {
        manual(
            ledger.registry_id(),
            '7',
            kind,
            None,
            946684800000,
            946684800001,
        )
    });
    claims.sort_by_key(|r| document_id(&json(r).unwrap()).unwrap().to_string());
    let delta = ConfirmationInventory {
        exposures: claims[1..].to_vec(),
        ..ConfirmationInventory::default()
    };
    ledger.append_confirmation_history(&delta, &ctx).unwrap();
    let old = ledger
        .read_confirmation_history(&ctx)
        .unwrap()
        .inventory()
        .conflicts
        .clone();
    assert_eq!(old.len(), 1);
    ledger
        .append_confirmation_history(
            &ConfirmationInventory {
                exposures: vec![claims[0].clone()],
                ..ConfirmationInventory::default()
            },
            &ctx,
        )
        .unwrap();
    let saved = ledger.read_confirmation_history(&ctx).unwrap();
    assert_eq!(saved.inventory().conflicts.len(), 3);
    assert!(old.iter().all(|r| saved.inventory().conflicts.contains(r)));
    assert!(saved
        .inventory()
        .conflicts
        .iter()
        .all(|r| json(r).unwrap()["body"]["affectedFamilyIds"] == json!([])));
    let mut body = json(&old[0]).unwrap()["body"].clone();
    body["affectedFamilyIds"] = json!([ctx.registered_sources[0].source.family_id]);
    let false_claim = freeze_conflict_declaration(&body.to_string()).unwrap();
    let before = snapshot(&ledger);
    assert!(ledger
        .append_confirmation_history(
            &ConfirmationInventory {
                conflicts: vec![false_claim.document().to_string()],
                ..ConfirmationInventory::default()
            },
            &ctx
        )
        .is_err());
    assert_eq!(snapshot(&ledger), before);
}

#[test]
fn manual_after_seal_occupancy_is_specific_and_unknown_markers_never_clear() {
    let (_dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let exp = json(&input.exposures[0]).unwrap();
    let from = exp["body"]["intervals"][0]["fromMs"].as_u64().unwrap();
    let seal = json(&input.events[1]).unwrap();
    let delta = ConfirmationInventory {
        exposures: vec![manual(
            ledger.registry_id(),
            '7',
            "chart",
            Some(document_id(&seal).unwrap()),
            from + 1,
            from + 2,
        )],
        ..ConfirmationInventory::default()
    };
    ledger.append_confirmation_history(&delta, &ctx).unwrap();
    assert!(!ledger
        .read_confirmation_history(&ctx)
        .unwrap()
        .is_restricted());
    let delta = ConfirmationInventory {
        exposures: vec![manual(
            ledger.registry_id(),
            '8',
            "raw",
            None,
            from + 1,
            from + 2,
        )],
        unknown_origins: vec!["e".repeat(32)],
        ..ConfirmationInventory::default()
    };
    ledger.append_confirmation_history(&delta, &ctx).unwrap();
    let saved = ledger.read_confirmation_history(&ctx).unwrap();
    assert!(saved.is_restricted());
    assert!(saved
        .inventory()
        .conflicts
        .iter()
        .any(|r| json(r).unwrap()["body"]["kind"] == "manual_overlap"));
    ledger
        .append_confirmation_history(&ConfirmationInventory::default(), &ctx)
        .unwrap();
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        saved.inventory()
    );
}

#[test]
fn late_index_insert_failure_rolls_back_all_new_documents_and_blobs() {
    let (dirs, ledger, input, ctx) = fixture();
    let c = raw_connection(&dirs);
    c.execute_batch("CREATE TRIGGER injected_index_failure BEFORE INSERT ON confirmation_intervals BEGIN SELECT RAISE(ABORT,'injected late index failure'); END;").unwrap();
    let before = snapshot(&ledger);
    assert!(ledger.append_confirmation_history(&input, &ctx).is_err());
    assert_eq!(snapshot(&ledger), before);
    assert!(ledger
        .read_confirmation_history(&ctx)
        .unwrap()
        .inventory()
        .batches
        .is_empty());
}

#[test]
fn caller_owned_transaction_is_not_committed_by_the_internal_append_helper() {
    let (_dirs, ledger, input, ctx) = fixture();
    {
        let mut conn = ledger.lock().unwrap();
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        tx.execute(
            "INSERT INTO registry_meta VALUES ('injected_caller_marker','staged')",
            [],
        )
        .unwrap();
        super::super::append_in(&tx, ledger.registry_id(), &input, &ctx).unwrap();
    }
    assert!(ledger
        .read_confirmation_history(&ctx)
        .unwrap()
        .inventory()
        .batches
        .is_empty());
    let exists: bool = ledger
        .lock()
        .unwrap()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM registry_meta WHERE key='injected_caller_marker')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!exists);
}

#[test]
fn postwrite_projection_verification_rolls_back_new_conflicts_and_old_row_damage() {
    let (dirs, ledger, input, ctx) = fixture();
    ledger.append_confirmation_history(&input, &ctx).unwrap();
    let before = ledger.read_confirmation_history(&ctx).unwrap();
    let state = snapshot(&ledger);
    let c = raw_connection(&dirs);
    c.execute_batch("DROP TRIGGER confirmation_intervals_no_update;CREATE TRIGGER injected_projection_damage AFTER INSERT ON confirmation_conflicts BEGIN UPDATE confirmation_intervals SET instrument_id='crypto:binance:ETHUSDT';END;").unwrap();
    assert!(ledger
        .append_confirmation_history(&fork(&input, &ctx, 'f'), &ctx)
        .is_err());
    assert_eq!(snapshot(&ledger), state);
    assert_eq!(
        ledger.read_confirmation_history(&ctx).unwrap().inventory(),
        before.inventory()
    );
}
