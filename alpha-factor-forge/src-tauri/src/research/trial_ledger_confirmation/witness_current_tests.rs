use super::*;

fn coverage(ledger: &TrialLedger, w: &SourceWitnessSnapshot) -> Vec<String> {
    let mut conn = ledger.lock().unwrap();
    let tx = conn.transaction().unwrap();
    crate::research::trial_ledger::transfer::WitnessCurrentContext::new(&tx, ledger.registry_id())
        .unwrap()
        .observe(w)
        .unwrap()
        .1
        .unincorporated_origins
}
fn upgrade(ledger: &TrialLedger, ctx: &GraphContext) {
    ledger
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:BTCUSDT".into()),
            tests_per_trial: 3,
            events: ctx.registered_sources[0]
                .candidates
                .iter()
                .map(|c| event("original", c.candidate_index, &c.strategy_hash))
                .collect(),
        })
        .unwrap();
}

#[test]
fn same_head_protocol_root_retains_the_exact_older_eligible_admission_anchor() {
    let (_dirs, ledger, input, ctx) = fixture();
    let old = original(&ledger, &ctx);
    let mut strict = input;
    strict
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, old.text()).unwrap());
    ledger
        .append_witnessed_confirmation_history(&strict)
        .unwrap();
    upgrade(&ledger, &ctx);
    let upgraded = original(&ledger, &ctx);
    let a = crate::research::trial_ledger::transfer::reconstruct_source_witness(&old).unwrap();
    let b = crate::research::trial_ledger::transfer::reconstruct_source_witness(&upgraded).unwrap();
    assert_eq!(a.prefix(), b.prefix());
    assert_eq!(a.trial().tests_per_trial, 2);
    assert_eq!(b.trial().tests_per_trial, 3);
    assert!(coverage(&ledger, &old).is_empty());
    assert!(coverage(&ledger, &upgraded).is_empty());
    ledger
        .append_witnessed_confirmation_history(&ConfirmationInventory {
            blobs: vec![HistoryBlob::new(BlobKind::SourceWitness, upgraded.text()).unwrap()],
            ..Default::default()
        })
        .unwrap();
    let graph = ledger.read_witnessed_confirmation_history().unwrap();
    assert_eq!(graph.inventory().admissions.len(), 2);
    assert_eq!(
        graph
            .inventory()
            .blobs
            .iter()
            .filter(|b| b.kind() == BlobKind::SourceWitness)
            .count(),
        2
    );
    assert!(!graph.is_restricted());
}

#[test]
fn same_event_prefix_does_not_hide_a_missing_protocol_upgrade() {
    let (_dirs, source, _input, ctx) = fixture();
    let old = original(&source, &ctx);
    let receiver_dirs = Dirs::new();
    let receiver = receiver_dirs.open();
    receiver
        .import_json_lines(old.trial_export_text().as_bytes())
        .unwrap();
    upgrade(&source, &ctx);
    let newer = original(&source, &ctx);
    assert!(coverage(&receiver, &old).is_empty());
    assert_eq!(
        coverage(&receiver, &newer),
        vec![source.registry_id().to_string()]
    );
    receiver
        .restore_confirmation_source_witnesses(std::slice::from_ref(&newer))
        .unwrap();
    assert!(receiver
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    upgrade(&receiver, &ctx);
    assert!(coverage(&receiver, &newer).is_empty());
    assert_eq!(counts(&receiver).2, 4);
}

#[test]
fn a_retained_primary_checkpoint_does_not_account_for_missing_foreign_origin_events() {
    let foreign_dirs = Dirs::new();
    let foreign = foreign_dirs.open();
    foreign
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:ETHUSDT".into()),
            tests_per_trial: 2,
            events: vec![event("prior", 0, "prior-0"), event("prior", 1, "prior-1")],
        })
        .unwrap();
    let source_dirs = Dirs::new();
    let source = source_dirs.open();
    source
        .import_json_lines(&foreign.export_json_lines().unwrap())
        .unwrap();
    let f = base();
    let mut claims = RegisteredSource {
        source: serde_json::from_value(f["registeredSource"]["source"].clone()).unwrap(),
        candidates: serde_json::from_value(f["registeredSource"]["candidates"].clone()).unwrap(),
    };
    let batch = register(
        &source,
        claims
            .candidates
            .iter()
            .map(|c| event("original", c.candidate_index, &c.strategy_hash))
            .collect(),
    );
    claims.source.trial_batch_id = batch.strip_prefix("trial-batch-v1:").unwrap().into();
    let w = original(
        &source,
        &GraphContext {
            registered_sources: vec![claims],
            ..Default::default()
        },
    );
    let receiver_dirs = Dirs::new();
    let receiver = receiver_dirs.open();
    receiver
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:ETHUSDT".into()),
            tests_per_trial: 2,
            events: vec![
                event("prior", 0, "different-0"),
                event("prior", 1, "different-1"),
            ],
        })
        .unwrap();
    // A real conflicting archival import cannot incorporate foreign-origin
    // prior trials under reused keys; the unrelated target family is retained.
    receiver
        .import_json_lines(w.trial_export_text().as_bytes())
        .unwrap();
    let anchor = crate::research::trial_ledger::transfer::reconstruct_source_witness(&w).unwrap();
    // Model a retained checkpoint in a partially restored registry. Normal
    // imports deliberately withhold it when any source event is missing. Its
    // exact original chain/last event still cannot account for the foreign gap.
    {
        let conn = receiver.lock().unwrap();
        let last: String = conn
            .query_row(
                "SELECT event_id FROM trial_events ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO origin_checkpoints VALUES (?1,?2,?3,?4)",
            params![
                source.registry_id(),
                anchor.prefix().seq as i64,
                last,
                anchor.prefix().chain_head
            ],
        )
        .unwrap();
    }
    let chain:String=receiver.lock().unwrap().query_row("SELECT origin_chain FROM origin_checkpoints WHERE origin_registry_id=?1 AND origin_seq=?2",params![source.registry_id(),anchor.prefix().seq as i64],|r|r.get(0)).unwrap();
    assert_eq!(chain, anchor.prefix().chain_head);
    assert_eq!(
        coverage(&receiver, &w),
        vec![foreign.registry_id().to_string()]
    );
    receiver
        .restore_confirmation_source_witnesses(&[w])
        .unwrap();
    assert!(receiver
        .read_witnessed_confirmation_history()
        .unwrap()
        .is_restricted());
    assert_eq!(counts(&receiver).2, 4); // no nested import or fabricated extra events
}

#[test]
fn lost_original_restrictive_witnesses_fail_even_with_actual_unknown_stops() {
    for origin_conflict in [false, true] {
        let (_dirs, source, _input, ctx) = fixture();
        if origin_conflict {
            source.lock().unwrap().execute("INSERT INTO registry_conflicts(origin_registry_id,detail_json,recorded_at) VALUES (?1,'{}','original')",["f".repeat(32)]).unwrap();
        } else {
            source.lock().unwrap().execute("INSERT INTO family_conflicts(family_id,kind,detail_json,recorded_at) VALUES (?1,'original-test','{}','original')",[&ctx.registered_sources[0].source.family_id]).unwrap();
        }
        let w = original(&source, &ctx);
        let receiver_dirs = Dirs::new();
        let receiver = receiver_dirs.open();
        record_unknown_origins(
            &receiver.lock().unwrap(),
            &BTreeSet::from([source.registry_id().into(), "f".repeat(32)]),
        )
        .unwrap();
        let before = counts(&receiver);
        assert!(receiver
            .restore_confirmation_source_witnesses(&[w])
            .is_err());
        assert_eq!(counts(&receiver), before);
    }
}

#[test]
fn current_protocol_document_corruption_and_broken_chain_fail_before_restoration() {
    for protocol in [false, true] {
        let (_dirs, ledger, _input, ctx) = fixture();
        let w = original(&ledger, &ctx);
        if protocol {
            upgrade(&ledger, &ctx);
            ledger.lock().unwrap().execute_batch("DROP TRIGGER family_protocol_upgrades_no_update; UPDATE family_protocol_upgrades SET protocol_json='{}'").unwrap();
        } else {
            ledger.lock().unwrap().execute_batch("DROP TRIGGER trial_events_no_update; UPDATE trial_events SET chain='bad' WHERE seq=1").unwrap();
        }
        let before = counts(&ledger);
        assert!(ledger.restore_confirmation_source_witnesses(&[w]).is_err());
        assert_eq!(counts(&ledger), before);
    }
}

#[test]
fn replacement_audit_and_noncanonical_incoming_representations_do_not_mutate_strict_history() {
    let (_dirs, ledger, input, ctx) = fixture();
    let w = original(&ledger, &ctx);
    let mut strict = input;
    strict
        .blobs
        .push(HistoryBlob::new(BlobKind::SourceWitness, w.text()).unwrap());
    ledger
        .append_witnessed_confirmation_history(&strict)
        .unwrap();
    let before = ledger
        .read_witnessed_confirmation_history()
        .unwrap()
        .inventory()
        .clone();
    let mut duplicate = ConfirmationInventory::default();
    let raw = &before.operations[0];
    let needle = format!(
        "\"originRegistryId\":{}",
        serde_json::from_str::<Value>(raw).unwrap()["body"]["originRegistryId"]
    );
    duplicate
        .operations
        .push(raw.replacen(&needle, &format!("{needle},{needle}"), 1));
    assert_ne!(duplicate.operations[0], *raw);
    assert!(ledger
        .append_witnessed_confirmation_history(&duplicate)
        .is_err());
    assert_eq!(
        ledger
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &before
    );
    ledger
        .lock()
        .unwrap()
        .execute_batch("PRAGMA recursive_triggers=OFF")
        .unwrap();
    assert!(ledger
        .lock()
        .unwrap()
        .execute(
            "INSERT OR REPLACE INTO confirmation_source_witnesses VALUES (?1,?2,?3,'replacement')",
            params![w.sha256(), w.text().len() as i64, w.text().as_bytes()]
        )
        .is_err());
    assert_eq!(
        ledger
            .read_witnessed_confirmation_history()
            .unwrap()
            .inventory(),
        &before
    );
}
