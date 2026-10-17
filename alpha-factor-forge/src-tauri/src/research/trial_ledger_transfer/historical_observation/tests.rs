use super::*;
use crate::research::trial_ledger::{self as ledger, Admission, BindingCheck, TrialKind};
use alpha_factor_forge::discovery_core::confirmation_history::parse_binding_snapshot;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Dirs {
    root: PathBuf,
    registry: PathBuf,
    workspace: PathBuf,
}
impl Dirs {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "aff-historical-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let registry = root.join("registry");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        Self {
            root,
            registry,
            workspace,
        }
    }
    fn open(&self) -> TrialLedger {
        TrialLedger::open_with_sync_roots(&self.registry, &self.workspace, &[]).unwrap()
    }
}
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn event(request: &str, index: u64, strategy: &str) -> TrialEventInput {
    TrialEventInput {
        kind: TrialKind::Variant,
        origin: TrialOrigin::Request {
            request_id: request.into(),
            candidate_index: index,
        },
        hypothesis_hash: Some("hypothesis".into()),
        strategy_hash: Some(strategy.into()),
        dataset_hash: Some("dataset".into()),
        snapshot_id: Some("snapshot".into()),
        split_hash: Some(format!("trial-split-v1:{}", "4".repeat(64))),
        seeds_hash: Some("seeds".into()),
        engine_fingerprint_hash: Some("engine".into()),
        benchmark_id: None,
        benchmark_params_hash: None,
        reproduction_of: None,
        benchmark_evidence: None,
    }
}
fn register(ledger: &TrialLedger, events: Vec<TrialEventInput>, tests: u64) -> String {
    ledger
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:BTCUSDT".into()),
            tests_per_trial: tests,
            events,
        })
        .unwrap()
        .batch_id
}
fn binding_value(ledger: &TrialLedger) -> Value {
    let BindingCheck::Current(b) = ledger.check_binding(None).unwrap() else {
        panic!("current")
    };
    let evidence = b.evidence.unwrap();
    let alpha = b.alpha.unwrap();
    json!({"version":"trial-ledger-binding-v5","registryId":b.registry_id,"seq":b.seq,"chainHead":b.chain_head,
        "evidence":{"quarantinedFamilies":evidence.quarantined_families,"conflictedOrigins":evidence.conflicted_origins,"familyTests":evidence.family_tests},
        "alpha":{"budgets":alpha.budgets,"reservations":alpha.reservations,"conflicts":alpha.conflicts},
        "alphaUnknownOrigins":b.alpha_unknown_origins.unwrap(),
        "confirmation":{"batches":[],"selections":[],"operations":[],"exposures":[],"admissions":[],"events":[],"blobs":[],"conflicts":[],"unknownOrigins":[]}})
}
fn binding(ledger: &TrialLedger) -> BindingSnapshot {
    parse_binding_snapshot(&binding_value(ledger).to_string()).unwrap()
}
struct Fixture {
    ledger: TrialLedger,
    // Rust drops fields in declaration order; close SQLite before its temp tree.
    _dirs: Dirs,
    source: RegisteredSource,
    raw: Vec<u8>,
    binding: BindingSnapshot,
}
fn fixture() -> Fixture {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    register(
        &ledger,
        vec![event("prior", 0, "prior-0"), event("prior", 1, "prior-1")],
        2,
    );
    let f: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/rs-core/research-confirmation-graph-v1.json"
    ))
    .unwrap();
    let mut source = RegisteredSource {
        source: serde_json::from_value(f["registeredSource"]["source"].clone()).unwrap(),
        candidates: serde_json::from_value(f["registeredSource"]["candidates"].clone()).unwrap(),
    };
    source.source.workspace_id = "1".repeat(32);
    let batch = register(
        &ledger,
        source
            .candidates
            .iter()
            .map(|c| event("original", c.candidate_index, &c.strategy_hash))
            .collect(),
        2,
    );
    source.source.trial_batch_id = batch.strip_prefix("trial-batch-v1:").unwrap().into();
    let raw = ledger.export_trial_projection_for_test().unwrap();
    let binding = binding(&ledger);
    Fixture {
        _dirs: dirs,
        ledger,
        source,
        raw,
        binding,
    }
}
fn observe(f: &Fixture) -> HistoricalSourceObservation {
    reconstruct_historical_source(&f.raw, &f.binding, &f.source).unwrap()
}
fn parts(raw: &[u8]) -> (Value, Vec<Value>) {
    let mut lines = std::str::from_utf8(raw).unwrap().lines();
    (
        serde_json::from_str(lines.next().unwrap()).unwrap(),
        lines.map(|s| serde_json::from_str(s).unwrap()).collect(),
    )
}
fn encode(mut header: Value, rows: Vec<Value>) -> Vec<u8> {
    let body: String = rows
        .iter()
        .map(|v| canonical_text(v).unwrap() + "\n")
        .collect();
    header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
    (canonical_text(&header).unwrap() + "\n" + &body).into_bytes()
}
fn rechain(header: &mut Value, rows: &mut [Value]) {
    let ids: BTreeMap<_, _> = rows
        .iter_mut()
        .filter(|v| v["type"] == "event")
        .map(|row| {
            let old = row["eventId"].as_str().unwrap().to_string();
            let new = event_id_for(&canonical_text(&row["payload"]).unwrap());
            row["eventId"] = json!(new);
            (old, new)
        })
        .collect();
    let batches: BTreeMap<_, _> = rows
        .iter_mut()
        .filter(|v| v["type"] == "batch")
        .map(|row| {
            for pair in row["members"].as_array_mut().unwrap() {
                if let Some(id) = ids.get(pair[1].as_str().unwrap()) {
                    pair[1] = json!(id);
                }
            }
            let old = row["batchId"].as_str().unwrap().to_string();
            let new = batch_id_for(row["familyId"].as_str(), &row["members"]).unwrap();
            row["batchId"] = json!(new);
            (old, new)
        })
        .collect();
    for row in rows.iter_mut() {
        if let Some(new) = row["batchId"].as_str().and_then(|old| batches.get(old)) {
            row["batchId"] = json!(new);
        }
    }
    let mut chain = genesis(header["registryId"].as_str().unwrap());
    let mut seq = 0;
    for row in rows.iter_mut().filter(|v| v["type"] == "event") {
        seq += 1;
        chain = next_chain(&chain, row["eventId"].as_str().unwrap());
        row["seq"] = json!(seq);
        row["chain"] = json!(chain);
    }
    header["headSeq"] = json!(seq);
    header["headChain"] = json!(chain);
    header["eventCount"] = json!(seq);
    header["batchCount"] = json!(rows.iter().filter(|v| v["type"] == "batch").count());
}
fn bad(raw: &[u8], b: &BindingSnapshot, source: &RegisteredSource) -> String {
    let e = reconstruct_historical_source(raw, b, source).unwrap_err();
    assert_eq!(e.code(), "registry_state_invalid");
    assert!(!e.to_string().contains("PROTECTED_SENTINEL"));
    e.to_string()
}

#[test]
fn reconstructs_actual_whole_family_and_complete_batch_without_receipt_totals() {
    let f = fixture();
    let o = observe(&f);
    assert_eq!(o.archive_sha256(), sha256_hex(&f.raw));
    assert_eq!(o.prefix().registry_id, f.ledger.registry_id());
    assert_eq!(o.prefix().seq, 4);
    assert_eq!(o.prefix().chain_head, f.binding.chain_head());
    assert_eq!(o.source_claims().source, f.source.source);
    assert_eq!(o.source_claims().candidates, f.source.candidates);
    assert_eq!(o.trial().family_effective_trials, 4);
    assert_eq!(o.trial().registered_batch_effective_trials, 2);
    assert_eq!(o.trial().complete_candidates, 2);
    assert_eq!(o.trial().tests_per_trial, 2);
    let (h, mut rows) = parts(&f.raw);
    for row in rows.iter_mut().filter(|v| v["type"] == "receipt") {
        row["familyEffectiveBefore"] = json!(999999);
    }
    let changed = encode(h, rows);
    assert!(ValidatedExport::parse(&changed).is_ok());
    let from_changed_audit =
        reconstruct_historical_source(&changed, &f.binding, &f.source).unwrap();
    assert_eq!(from_changed_audit.trial(), o.trial());
    assert_ne!(from_changed_audit.archive_sha256(), o.archive_sha256());
}

#[test]
fn historical_snapshot_survives_receiver_growth_and_source_reopen() {
    let f = fixture();
    let expected = observe(&f).trial().clone();
    let dirs = Dirs::new();
    let receiver = dirs.open();
    receiver.import_json_lines(&f.raw).unwrap();
    register(&receiver, vec![event("later", 0, "later")], 2);
    let Admission::Count(now) = receiver
        .read_admission_count(&expected.trial_batch_id)
        .unwrap()
    else {
        panic!("count")
    };
    assert_eq!(now.family_effective_trials(), 5);
    assert_eq!(observe(&f).trial(), &expected);
    let Fixture {
        _dirs,
        ledger,
        source,
        raw,
        binding,
    } = f;
    drop(ledger);
    let reopened = _dirs.open();
    register(&reopened, vec![event("source-later", 0, "new")], 3);
    drop(reopened);
    let removed_root = _dirs.root.clone();
    drop(_dirs);
    assert!(!removed_root.exists());
    assert_eq!(
        reconstruct_historical_source(&raw, &binding, &source)
            .unwrap()
            .trial(),
        &expected
    );
}

#[test]
fn protocol_upgrade_with_unchanged_event_head_requires_original_protocol_evidence() {
    let f = fixture();
    let batch = register(
        &f.ledger,
        f.source
            .candidates
            .iter()
            .map(|c| event("original", c.candidate_index, &c.strategy_hash))
            .collect(),
        3,
    );
    let upgraded = binding(&f.ledger);
    let raw = f.ledger.export_trial_projection_for_test().unwrap();
    assert_eq!(upgraded.seq(), f.binding.seq());
    assert_eq!(upgraded.chain_head(), f.binding.chain_head());
    assert_eq!(
        reconstruct_historical_source(&raw, &upgraded, &f.source)
            .unwrap()
            .trial()
            .tests_per_trial,
        3
    );
    assert!(bad(&raw, &f.binding, &f.source).contains("trial evidence differs"));
    assert!(bad(&f.raw, &upgraded, &f.source).contains("trial evidence differs"));
    assert_eq!(observe(&f).trial().tests_per_trial, 2);
    assert_eq!(observe(&f).trial().trial_batch_id, batch);
}

#[test]
fn complete_candidate_membership_refuses_subsets_and_equal_count_substitution() {
    let f = fixture();
    let mut source = f.source.clone();
    source.candidates.pop();
    bad(&f.raw, &f.binding, &source);
    let mut source = f.source.clone();
    source.candidates[1].candidate_index += 1;
    bad(&f.raw, &f.binding, &source);
    let mut source = f.source.clone();
    source.candidates[0].strategy_hash = format!("strategy-v2:{}", "f".repeat(64));
    bad(&f.raw, &f.binding, &source);
    let mut source = f.source.clone();
    source.candidates[1] = source.candidates[0].clone();
    bad(&f.raw, &f.binding, &source);
    for field in ["workspace", "batch", "family", "domain"] {
        let mut source = f.source.clone();
        match field {
            "workspace" => source.source.workspace_id = "2".repeat(32),
            "batch" => source.source.trial_batch_id = "f".repeat(64),
            "family" => {
                source.source.instrument_id = "crypto:binance:ETHUSDT".into();
                source.source.family_id = ledger::family_parts(&source.source.instrument_id)
                    .unwrap()
                    .0;
            }
            _ => source.source.run_id = 0,
        }
        bad(&f.raw, &f.binding, &source);
    }
}

#[test]
fn every_prefix_and_restrictive_evidence_component_is_exact() {
    let f = fixture();
    for field in [
        "registryId",
        "seq",
        "chainHead",
        "quarantinedFamilies",
        "conflictedOrigins",
        "familyTests",
    ] {
        let mut value = binding_value(&f.ledger);
        match field {
            "registryId" => value[field] = json!("f".repeat(32)),
            "seq" => value[field] = json!(value[field].as_u64().unwrap() + 1),
            "chainHead" => value[field] = json!("f".repeat(64)),
            "quarantinedFamilies" => value["evidence"][field] = json!([f.source.source.family_id]),
            "conflictedOrigins" => value["evidence"][field] = json!(["f".repeat(32)]),
            _ => value["evidence"][field][&f.source.source.family_id] = json!(3),
        }
        let wrong = parse_binding_snapshot(&value.to_string()).unwrap();
        bad(&f.raw, &wrong, &f.source);
    }
    let (h, mut rows) = parts(&f.raw);
    rows.push(json!({"type":"familyConflict","familyId":f.source.source.family_id,"kind":"import_conflict","detailJson":"[]"}));
    rows.push(
        json!({"type":"registryConflict","originRegistryId":"f".repeat(32),"detailJson":"[]"}),
    );
    let raw = encode(h, rows);
    let mut value = binding_value(&f.ledger);
    value["evidence"]["quarantinedFamilies"] = json!([f.source.source.family_id]);
    value["evidence"]["conflictedOrigins"] = json!(["f".repeat(32)]);
    let b = parse_binding_snapshot(&value.to_string()).unwrap();
    assert_eq!(
        reconstruct_historical_source(&raw, &b, &f.source)
            .unwrap()
            .trial()
            .family_effective_trials,
        4
    );
    bad(&raw, &f.binding, &f.source);
}

#[test]
fn malformed_archive_and_rehashed_invalid_event_kinds_fail_with_static_errors() {
    let f = fixture();
    for raw in [
        b"PROTECTED_SENTINEL".to_vec(),
        f.raw[..f.raw.len() - 1].to_vec(),
        [f.raw.clone(), b" ".to_vec()].concat(),
    ] {
        bad(&raw, &f.binding, &f.source);
    }
    for field in ["kind", "effective"] {
        let (mut h, mut rows) = parts(&f.raw);
        let event = rows.iter_mut().find(|v| v["type"] == "event").unwrap();
        event["payload"][field] = if field == "kind" {
            json!("PROTECTED_SENTINEL")
        } else {
            json!(false)
        };
        rechain(&mut h, &mut rows);
        let raw = encode(h, rows);
        assert!(ValidatedExport::parse(&raw).is_ok());
        assert!(bad(&raw, &f.binding, &f.source).contains("source provenance"));
    }
}

#[test]
fn supplied_artifact_metadata_remains_claims_not_authenticated_file_presence() {
    let f = fixture();
    let mut source = f.source.clone();
    source.candidates[0].result_artifact_hash = "f".repeat(64);
    let o = reconstruct_historical_source(&f.raw, &f.binding, &source).unwrap();
    assert_eq!(
        o.source_claims().candidates[0].result_artifact_hash,
        "f".repeat(64)
    );
    assert_eq!(o.trial(), observe(&f).trial());
    // The trial schema has no artifact bytes, campaign/run or manifest linkage.
    // A successful archival reconstruction must never claim those were checked.
}

#[test]
fn independent_divergent_source_branches_remain_verifiable_after_actual_union_conflict() {
    let Fixture {
        _dirs: left_dirs,
        ledger,
        source,
        ..
    } = fixture();
    let origin = ledger.registry_id().to_string();
    drop(ledger);
    let right_dirs = Dirs::new();
    std::fs::create_dir_all(&right_dirs.registry).unwrap();
    std::fs::copy(
        left_dirs.registry.join(ledger::REGISTRY_FILE_NAME),
        right_dirs.registry.join(ledger::REGISTRY_FILE_NAME),
    )
    .unwrap();
    let left = left_dirs.open();
    let right = right_dirs.open();
    assert_eq!(left.registry_id(), right.registry_id());
    register(&left, vec![event("left", 0, "left-strategy")], 2);
    register(&right, vec![event("right", 0, "right-strategy")], 2);
    let left_raw = left.export_trial_projection_for_test().unwrap();
    let right_raw = right.export_trial_projection_for_test().unwrap();
    let left_binding = binding(&left);
    let right_binding = binding(&right);
    assert_eq!(left_binding.seq(), right_binding.seq());
    assert_ne!(left_binding.chain_head(), right_binding.chain_head());
    let summary = left.import_json_lines(&right_raw).unwrap();
    assert!(summary.conflicts > 0);
    for (raw, b) in [(&left_raw, &left_binding), (&right_raw, &right_binding)] {
        let o = reconstruct_historical_source(raw, b, &source).unwrap();
        assert_eq!(o.trial().registry_id, origin);
        assert_eq!(o.trial().family_effective_trials, 5);
        assert_eq!(o.trial().registered_batch_effective_trials, 2);
    }
    bad(&right_raw, &left_binding, &source);
    let current = binding(&left);
    assert!(current.conflicted_origins().contains(&origin));
    assert!(current.seq() > left_binding.seq());
}

#[test]
fn closed_reproduction_history_rejects_missing_original_but_import_retains_sql_fallback() {
    let f = fixture();
    let (_, original_rows) = parts(&f.raw);
    let batch_id = format!("trial-batch-v1:{}", f.source.source.trial_batch_id);
    let original = original_rows
        .iter()
        .find(|r| r["type"] == "event" && r["batchId"] == batch_id)
        .unwrap();
    let original_payload = original["payload"].clone();
    let mut repro = event(
        "reproduction",
        0,
        original_payload["strategyHash"].as_str().unwrap(),
    );
    repro.kind = TrialKind::Reproduction;
    repro.reproduction_of = Some(original["eventId"].as_str().unwrap().into());
    register(&f.ledger, vec![repro], 2);
    let full = f.ledger.export_trial_projection_for_test().unwrap();
    let current = binding(&f.ledger);
    assert_eq!(
        reconstruct_historical_source(&full, &current, &f.source)
            .unwrap()
            .trial()
            .family_effective_trials,
        4
    );
    let (mut h, mut rows) = parts(&full);
    rows.retain(|r| r["batchId"].as_str() != Some(batch_id.as_str()));
    rechain(&mut h, &mut rows);
    let incomplete = encode(h, rows);
    let parsed = ValidatedExport::parse(&incomplete).unwrap();
    assert!(parsed.validate_kinds_resolving(|_| Ok(None)).is_err());
    let mut conn = f.ledger.lock().unwrap();
    let tx = conn.transaction().unwrap();
    parsed.validate_kinds(&tx).unwrap(); // Existing import fallback still resolves the real original.
    assert!(bad(&incomplete, &current, &f.source).contains("source provenance"));
}

#[test]
fn original_snapshot_does_not_promote_nonportable_exempt_benchmark_history() {
    use alpha_factor_forge::discovery_core::benchmarks::DETERMINISTIC_BENCHMARK_IDS;
    let f = fixture();
    let (mut h, mut rows) = parts(&f.raw);
    let event = rows.iter_mut().find(|r| r["type"] == "event").unwrap();
    let id = DETERMINISTIC_BENCHMARK_IDS[0];
    event["payload"]["kind"] = json!("benchmark");
    event["payload"]["benchmarkId"] = json!(id);
    event["payload"]["benchmarkParamsHash"] = json!(benchmark_params_hash(id).unwrap());
    rechain(&mut h, &mut rows);
    let counted = encode(h, rows);
    ValidatedExport::parse(&counted)
        .unwrap()
        .validate_kinds_resolving(|_| Ok(None))
        .unwrap();
    let (mut h, mut rows) = parts(&counted);
    rows.iter_mut().find(|r| r["type"] == "event").unwrap()["payload"]["effective"] = json!(false);
    rechain(&mut h, &mut rows);
    let exempt = encode(h, rows);
    assert!(ValidatedExport::parse(&exempt).is_ok());
    assert!(bad(&exempt, &f.binding, &f.source).contains("source provenance"));
}

#[test]
fn original_archive_accepts_exact_eight_mib_and_refuses_one_over_before_decode() {
    let f = fixture();
    let (h, mut rows) = parts(&f.raw);
    rows.push(json!({"type":"familyConflict","familyId":f.source.source.family_id,"kind":"import_conflict","detailJson":"{\"padding\":\"\"}"}));
    let base_len = encode(h.clone(), rows.clone()).len();
    rows.last_mut().unwrap()["detailJson"] = json!(canonical_text(
        &json!({"padding":"x".repeat(MAX_HISTORICAL_ARCHIVE_BYTES - base_len)})
    )
    .unwrap());
    let raw = encode(h, rows);
    assert_eq!(raw.len(), MAX_HISTORICAL_ARCHIVE_BYTES);
    let mut value = binding_value(&f.ledger);
    value["evidence"]["quarantinedFamilies"] = json!([f.source.source.family_id]);
    let b = parse_binding_snapshot(&value.to_string()).unwrap();
    assert_eq!(
        reconstruct_historical_source(&raw, &b, &f.source)
            .unwrap()
            .trial()
            .family_effective_trials,
        4
    );
    assert!(bad(&[raw, b" ".to_vec()].concat(), &b, &f.source).contains("capacity exceeded"));
    assert!(
        bad(&vec![b'P'; MAX_HISTORICAL_ARCHIVE_BYTES + 1], &b, &f.source)
            .contains("capacity exceeded")
    );
}
