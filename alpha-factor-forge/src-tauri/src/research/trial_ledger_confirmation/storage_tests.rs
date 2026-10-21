use super::*;
use crate::research::trial_ledger::{
    Admission, TrialBatchInput, TrialEventInput, TrialKind, TrialOrigin, REGISTRY_FILE_NAME,
};
use alpha_factor_forge::discovery_core::{
    alpha_allocation::{parse_alpha_allocation, AlphaAllocationDeclaration},
    confirmation_contracts::{
        freeze_operation, freeze_selection, freeze_test_batch, freeze_validation_batch,
        RegisteredSource, SealedValidationInput,
    },
    confirmation_history::{
        freeze_admission, freeze_confirmation_event, freeze_confirmation_exposure, AdmissionInputs,
        HistoricalPrefix, HistoricalTrialObservation, HostAllocation,
    },
};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
#[path = "storage_capacity.rs"]
mod capacity;
#[path = "storage_cases.rs"]
mod cases;
#[path = "storage_corruption.rs"]
mod corruption;
#[path = "witness_migration_tests.rs"]
mod witness_migrations;
#[path = "witnessed_tests.rs"]
mod witnessed;
#[path = "binding_observation_tests.rs"]
mod binding_observation;
#[path = "archive_cases.rs"]
mod archive;
#[path = "archive_source_cases.rs"]
mod archive_source;
#[path = "preflight_cases.rs"]
mod preflight;

struct Dirs {
    root: PathBuf,
    registry: PathBuf,
    workspace: PathBuf,
}
impl Dirs {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "aff-confirmation-storage-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let registry = root.join("evidence");
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
fn base() -> Value {
    serde_json::from_str(include_str!(
        "../../../../fixtures/rs-core/research-confirmation-graph-v1.json"
    ))
    .unwrap()
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
        split_hash: Some("split".into()),
        seeds_hash: Some("seeds".into()),
        engine_fingerprint_hash: Some("engine".into()),
        benchmark_id: None,
        benchmark_params_hash: None,
        reproduction_of: None,
        benchmark_evidence: None,
    }
}
fn register(ledger: &TrialLedger, items: Vec<TrialEventInput>) -> String {
    ledger
        .register_batch(&TrialBatchInput {
            workspace_id: "1".repeat(32),
            instrument_id: Some("crypto:binance:BTCUSDT".into()),
            tests_per_trial: 2,
            events: items,
        })
        .unwrap()
        .batch_id
}
fn ids(raws: &[String]) -> Vec<String> {
    let mut ids = raws
        .iter()
        .map(|r| document_id(&json(r).unwrap()).unwrap().to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}
fn fixture() -> (Dirs, TrialLedger, ConfirmationInventory, GraphContext) {
    let dirs = Dirs::new();
    let ledger = dirs.open();
    let f = base();
    register(
        &ledger,
        vec![event("prior", 0, "prior-0"), event("prior", 1, "prior-1")],
    );
    let candidates = f["registeredSource"]["candidates"].as_array().unwrap();
    let batch_id = register(
        &ledger,
        candidates
            .iter()
            .map(|c| {
                event(
                    "original",
                    c["candidateIndex"].as_u64().unwrap(),
                    c["strategyHash"].as_str().unwrap(),
                )
            })
            .collect(),
    );
    let Admission::Count(count) = ledger.read_admission_count(&batch_id).unwrap() else {
        panic!("real registered count")
    };
    assert_eq!(count.family_effective_trials(), 4);
    assert_eq!(count.batch_effective_trials(), 2);
    let mut source = f["registeredSource"]["source"].clone();
    source["trialBatchId"] = json!(batch_id.strip_prefix("trial-batch-v1:").unwrap());
    let source = RegisteredSource {
        source: serde_json::from_value(source).unwrap(),
        candidates: serde_json::from_value(f["registeredSource"]["candidates"].clone()).unwrap(),
    };
    let allocation: AlphaAllocationDeclaration =
        parse_alpha_allocation(&f["allocationDeclaration"]).unwrap();
    ledger
        .declare_alpha_budget(count.family_id(), &allocation)
        .unwrap();
    let observation = HistoricalTrialObservation {
        registry_id: count.registry_id().into(),
        seq: count.seq(),
        chain_head: count.chain_head().into(),
        trial_batch_id: batch_id,
        family_id: count.family_id().into(),
        family_effective_trials: 4,
        registered_batch_effective_trials: 2,
        tests_per_trial: 2,
        complete_candidates: 2,
    };
    let mut ctx = GraphContext {
        registered_sources: vec![source.clone()],
        allocations: vec![HostAllocation {
            family_id: source.source.family_id.clone(),
            declaration: allocation.clone(),
        }],
        family_tests: BTreeMap::from([(source.source.family_id.clone(), 2)]),
        historical_prefixes: vec![HistoricalPrefix {
            registry_id: observation.registry_id.clone(),
            seq: observation.seq,
            chain_head: observation.chain_head.clone(),
        }],
        historical_trials: vec![observation.clone()],
        ..GraphContext::default()
    };
    let mut inventory = ConfirmationInventory::default();
    let mut body = f["records"]["validation"]["body"].clone();
    body["source"] = serde_json::to_value(&source.source).unwrap();
    let val = freeze_validation_batch(&body.to_string(), &source).unwrap();
    let mut last_seal = None;
    let mut last_alpha = None;
    let mut last_report = None;
    for stage in ["validation", "test"] {
        let batch = if stage == "validation" {
            val.clone()
        } else {
            let mut body = f["records"]["selection"]["body"].clone();
            body["validationBatchId"] = json!(val.id());
            body["alphaReservationId"] = json!(ctx.reservations[0].reservation_id());
            body["sealedReceiptId"] = json!(last_seal.as_deref().unwrap());
            body["reportSha256"] = json!(last_report.as_deref().unwrap());
            let report = inventory
                .blobs
                .iter()
                .find(|b| Some(b.sha256()) == last_report.as_deref())
                .unwrap();
            let selection = freeze_selection(
                &body.to_string(),
                &val,
                &SealedValidationInput {
                    receipt_id: last_seal.as_deref().unwrap(),
                    reservation: &ctx.reservations[0],
                    report_json: report.text(),
                },
            )
            .unwrap();
            inventory.selections.push(selection.document().to_string());
            let mut body = val.document()["body"].clone();
            body["stage"] =
                json!({"kind":"test","validationBatchId":val.id(),"selectionId":selection.id()});
            body["candidates"].as_array_mut().unwrap().retain(|c| {
                selection.document()["body"]["selectedCandidates"]
                    .as_array()
                    .unwrap()
                    .contains(&c["candidateIndex"])
            });
            freeze_test_batch(&body.to_string(), &val, &selection).unwrap()
        };
        let mut opbody = f["records"][&format!("{stage}Operation")]["body"].clone();
        opbody["originRegistryId"] = json!(ledger.registry_id());
        opbody["confirmationBatchId"] = json!(batch.id());
        if stage == "test" {
            opbody["selectionId"] =
                json!(document_id(&json(&inventory.selections[0]).unwrap()).unwrap());
        }
        let operation = freeze_operation(&opbody.to_string(), &batch).unwrap();
        let reservation = ledger
            .reserve_alpha_evidence(count.family_id(), operation.reservation_key(), batch.id())
            .unwrap()
            .reservation
            .unwrap();
        let mut binding = json(f["blobs"][0]["text"].as_str().unwrap()).unwrap();
        binding["registryId"] = json!(count.registry_id());
        binding["seq"] = json!(count.seq());
        binding["chainHead"] = json!(count.chain_head());
        if let Some(alpha) = &last_alpha {
            binding["alpha"]["reservations"] = json!([alpha]);
        }
        for (name, values) in [
            ("batches", &inventory.batches),
            ("selections", &inventory.selections),
            ("operations", &inventory.operations),
            ("exposures", &inventory.exposures),
            ("admissions", &inventory.admissions),
            ("events", &inventory.events),
            ("conflicts", &inventory.conflicts),
        ] {
            binding["confirmation"][name] = json!(ids(values));
        }
        let mut blobs = inventory
            .blobs
            .iter()
            .map(|b| b.sha256())
            .collect::<Vec<_>>();
        blobs.sort();
        binding["confirmation"]["blobs"] = json!(blobs);
        let binding = HistoryBlob::new(
            BlobKind::Binding,
            &format!("{}\n", serde_json::to_string_pretty(&binding).unwrap()),
        )
        .unwrap();
        let mut exposure = f["records"][&format!("{stage}Exposure")]["body"].clone();
        exposure["originRegistryId"] = json!(ledger.registry_id());
        exposure["claim"]["operationId"] = json!(operation.id());
        exposure["claim"]["alphaReservationId"] = json!(reservation.reservation_id());
        let exposure =
            freeze_confirmation_exposure(&exposure.to_string(), &batch, &operation, &reservation)
                .unwrap();
        let mut admission = f["records"][&format!("{stage}Admission")]["body"].clone();
        admission["operationId"] = json!(operation.id());
        admission["alphaReservationId"] = json!(reservation.reservation_id());
        admission["exposureId"] = json!(exposure.id());
        admission["registryObservation"]["registryId"] = json!(count.registry_id());
        admission["registryObservation"]["seq"] = json!(count.seq());
        admission["registryObservation"]["chainHead"] = json!(count.chain_head());
        admission["registryObservation"]["bindingSha256"] = json!(binding.sha256());
        let admission = freeze_admission(
            &admission.to_string(),
            &AdmissionInputs {
                batch: &batch,
                operation: &operation,
                reservation: &reservation,
                exposure: &exposure,
                binding: &binding,
                trial: &observation,
                allocation: &allocation,
            },
        )
        .unwrap();
        let report = f["blobs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b["kind"] == "statistics")
            .find(|b| {
                json(b["text"].as_str().unwrap()).unwrap()["seed"]
                    == if stage == "validation" { 7 } else { 8 }
            })
            .unwrap();
        let report =
            HistoryBlob::new(BlobKind::Statistics, report["text"].as_str().unwrap()).unwrap();
        let mut start = f["records"][&format!("{stage}Start")]["body"].clone();
        start["operationId"] = json!(operation.id());
        start["admissionId"] = json!(admission.id());
        let start = freeze_confirmation_event(&start.to_string(), &batch, &operation).unwrap();
        let mut seal = f["records"][&format!("{stage}Seal")]["body"].clone();
        seal["operationId"] = json!(operation.id());
        seal["admissionId"] = json!(admission.id());
        seal["event"]["executionStartedId"] = json!(start.id());
        seal["event"]["reportSha256"] = json!(report.sha256());
        let seal = freeze_confirmation_event(&seal.to_string(), &batch, &operation).unwrap();
        inventory.batches.push(batch.document().to_string());
        inventory.operations.push(operation.document().to_string());
        inventory.exposures.push(exposure.document().to_string());
        inventory.admissions.push(admission.document().to_string());
        inventory
            .events
            .extend([start.document().to_string(), seal.document().to_string()]);
        last_seal = Some(seal.id().to_string());
        last_alpha = Some(reservation.reservation_id().to_string());
        last_report = Some(report.sha256().to_string());
        ctx.reservations.push(reservation);
        inventory.blobs.extend([binding, report]);
    }
    validate_confirmation_graph(&inventory, &ctx).unwrap();
    (dirs, ledger, inventory, ctx)
}

#[test]
fn full_chain_roundtrip_replay_independent_connections_and_reopen() {
    let (dirs, ledger, input, ctx) = fixture();
    let summary = ledger.append_confirmation_history(&input, &ctx).unwrap();
    assert_eq!(
        summary,
        ConfirmationWriteSummary {
            added_records: 13,
            added_blobs: 4,
            added_intervals: 2,
            added_unknown_origins: 0
        }
    );
    let saved = ledger.read_confirmation_history(&ctx).unwrap();
    assert!(!saved.is_restricted());
    let zero = ledger.append_confirmation_history(&input, &ctx).unwrap();
    assert_eq!(
        zero.added_records + zero.added_blobs + zero.added_intervals + zero.added_unknown_origins,
        0
    );
    let other = dirs.open();
    assert_eq!(
        other.read_confirmation_history(&ctx).unwrap().inventory(),
        saved.inventory()
    );
    drop(other);
    drop(ledger);
    let reopened = dirs.open();
    assert_eq!(
        reopened
            .read_confirmation_history(&ctx)
            .unwrap()
            .inventory(),
        saved.inventory()
    );
}
