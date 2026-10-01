//! P12d-2c: campaign-bound runs (docs/research-campaign-declaration-v1.md).
//! A real P06 snapshot and trial ledger back every case: the binding is
//! checked before any write, trials register under the declared snapshot's
//! family, the admission decision is stored with the enqueue, and a
//! `NOT_ELIGIBLE` run still explores.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::db::ownership::{acquire, HolderKind};
use crate::market::{
    provenance::{self, RawObservation},
    registry,
    snapshot::{self, SnapshotKind, SnapshotOutcome, SnapshotRequest},
};
use crate::research::artifacts::ArtifactStore;
use crate::research::trial_ledger::{
    family_id_for, Admission, AdmissionBlocked, TrialBatchInput, TrialEventInput, TrialKind,
    TrialOrigin,
};
use alpha_factor_forge::discovery_core::market_foundation::{PriceBasis, SeriesRole};

const BTC: &str = "crypto:binance:BTCUSDT";
const START: i64 = 1_577_836_800_000;
const BARS: usize = 240;
const CAMPAIGN_FIXTURE: &str =
    include_str!("../../../../fixtures/rs-core/research-campaign-declaration-v1.json");

struct Workspace {
    dir: PathBuf,
    db: SharedDb,
    store: ArtifactStore,
    runner: DiscoveryRunner,
    ledger: Arc<TrialLedger>,
    epoch: i64,
    workspace_id: String,
    dataset_id: i64,
    dataset_hash: String,
    snapshot_id: String,
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let registry = crate::research::trial_ledger_workspace::registry_dir(&self.dir).ok();
        let _ = std::fs::remove_dir_all(&self.dir);
        if let Some(registry) = registry {
            let _ = std::fs::remove_dir_all(registry);
        }
    }
}

fn raw_source(scope: &str) -> RawObservation {
    RawObservation {
        instrument_id: BTC.into(),
        interval: "1d".into(),
        source: "binance-archive".into(),
        role: SeriesRole::Primary,
        request_scope: json!({ "scope": scope }),
        retrieved_at: "2021-01-01T00:00:00Z".into(),
        available_at: None,
        accepted: true,
        rejection_reason: None,
        revision_of: None,
        media_type: "text/csv".into(),
    }
}

/// One accepted primary source per call, so a second call yields a second
/// snapshot of the same dataset.
fn build_snapshot(workspace: &Workspace, scope: &str) -> String {
    let mut conn = workspace.db.lock().unwrap();
    let source = provenance::record_raw(
        &conn,
        &workspace.store,
        &raw_source(scope),
        scope.as_bytes(),
    )
    .unwrap()
    .0;
    let request = SnapshotRequest {
        instrument_id: BTC.into(),
        interval: "1d".into(),
        dataset_id: workspace.dataset_id,
        price_basis: PriceBasis::Raw,
        corporate_action_version: None,
        cost_profile_version: Some("cost-profile-v1".into()),
        kind: SnapshotKind::Historical,
        provenance_ids: vec![source],
        as_of_ms: START + BARS as i64 * 86_400_000,
    };
    match snapshot::build_snapshot(&mut conn, &request).unwrap() {
        SnapshotOutcome::Created(snapshot) => snapshot.snapshot_id,
        other => panic!("snapshot should be created: {other:?}"),
    }
}

fn workspace() -> Workspace {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "aff-campaign-run-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = migrated_db();
    let candles = alternating_candles(BARS, START);
    let (dataset_id, dataset_hash, epoch, workspace_id, ledger) = {
        let mut conn = db.lock().unwrap();
        registry::ensure_builtin_calendars(&conn).unwrap();
        let mut instrument = registry::default_crypto_instruments().remove(0);
        instrument.listed_from = Some(START);
        registry::register_instrument(&conn, &instrument).unwrap();
        let mut dataset = Dataset {
            id: None,
            exchange: "binance".into(),
            symbol: "BTCUSDT".into(),
            interval: "1d".into(),
            start_time: START,
            end_time: candles.last().unwrap().timestamp,
            candle_count: BARS as i64,
            source: "campaign-run-test".into(),
            dataset_hash: String::new(),
        };
        dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, &candles).unwrap();
        let dataset_id =
            repositories::import_dataset_with_candles(&mut conn, &dataset, &candles).unwrap();
        let epoch = acquire(&mut conn, HolderKind::DesktopEmbedded, 1)
            .unwrap()
            .epoch;
        let workspace_id = crate::db::runtime_ledger::workspace_id(&conn).unwrap();
        let bound = crate::research::trial_ledger_workspace::adopt(
            &mut conn,
            &dir,
            &workspace_id,
            Some(epoch),
        )
        .unwrap();
        (
            dataset_id,
            dataset.dataset_hash,
            epoch,
            workspace_id,
            bound.ledger,
        )
    };
    let store = ArtifactStore::in_data_dir(&dir);
    let runner = DiscoveryRunner {
        executor: Arc::new(ProductionExecutor),
        ..DiscoveryRunner::with_epoch(epoch)
    }
    .with_artifact_store(ArtifactStore::in_data_dir(&dir))
    .with_trial_ledger(ledger.clone(), &workspace_id);
    let mut workspace = Workspace {
        dir,
        db,
        store,
        runner,
        ledger,
        epoch,
        workspace_id,
        dataset_id,
        dataset_hash,
        snapshot_id: String::new(),
    };
    workspace.snapshot_id = build_snapshot(&workspace, "first");
    workspace
}

/// The fixture declaration bound to this workspace's snapshot, with the
/// v3 test config's fold plan as its sample policy.
fn declaration(workspace: &Workspace, bootstrap_samples: u64, max_bootstrap_samples: u64) -> Value {
    let mut raw: Value = serde_json::from_str(CAMPAIGN_FIXTURE).unwrap();
    raw["sampling"]["bootstrapSamples"] = json!(bootstrap_samples);
    raw["sampling"]["maxBootstrapSamples"] = json!(max_bootstrap_samples);
    let binding = &mut raw["instruments"][0];
    binding["listedAtMs"] = json!(START);
    binding["snapshotId"] = json!(workspace.snapshot_id);
    binding["datasetHash"] = json!(workspace.dataset_hash);
    binding["interval"] = json!("1d");
    binding["fromMs"] = json!(START);
    binding["toMs"] = json!(START + (BARS as i64 - 1) * 86_400_000);
    binding["samplePolicy"]["minimumTotalBars"] = json!(BARS);
    binding["samplePolicy"]["minimumTrainBars"] = json!(20);
    binding["samplePolicy"]["foldValidationBars"] = json!(8);
    binding["samplePolicy"]["foldCount"] = json!(3);
    raw
}

fn start(
    workspace: &Workspace,
    config: Value,
    declaration: Value,
    instrument: &str,
) -> AppResult<i64> {
    workspace.runner.start_campaign_for_request(
        workspace.db.clone(),
        Arc::new(RecordingSink::new(workspace.db.clone())),
        config,
        &CampaignStart {
            declaration,
            instrument_id: instrument.into(),
        },
        None,
    )
}

fn complete(workspace: &Workspace, run_id: i64) {
    wait_for_status(
        &workspace.runner,
        &workspace.db,
        run_id,
        RunStatus::Completed,
    );
    wait_for_coordinator_exit(&workspace.runner, run_id);
}

/// The registered events' payloads, read from the registry file.
fn registered_payloads(workspace: &Workspace) -> Vec<Value> {
    let registry = rusqlite::Connection::open(workspace.ledger.path()).unwrap();
    let mut stmt = registry
        .prepare("SELECT payload_json FROM trial_events ORDER BY seq")
        .unwrap();
    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
    rows.map(|text| serde_json::from_str(&text.unwrap()).unwrap())
        .collect()
}

fn count(db: &SharedDb, sql: &str) -> i64 {
    db.lock()
        .unwrap()
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

#[test]
fn an_eligible_campaign_run_stores_its_decision_with_the_enqueue() {
    let workspace = workspace();
    let config = walk_forward_runner_config(workspace.dataset_id, &workspace.dataset_hash);
    let declared = declaration(&workspace, 100_000, 200_000);
    let campaign_id = freeze_campaign(&declared)
        .unwrap()
        .campaign_id()
        .to_string();
    let run_id = start(&workspace, config.clone(), declared, BTC).unwrap();
    complete(&workspace, run_id);

    let conn = workspace.db.lock().unwrap();
    let stored = crate::db::campaign::get_campaign_admission(&conn, run_id)
        .unwrap()
        .expect("a campaign run has a decision");
    assert_eq!(
        stored.campaign_id,
        campaign_id,
        "re-frozen from storage"
    );
    assert_eq!(stored.status, "ELIGIBLE", "{}", stored.report["reasons"]);
    assert_eq!(stored.instrument_id, BTC);
    assert_eq!(
        (stored.fee_pct, stored.slip_pct),
        (
            config["benchmarkCosts"]["feePct"].as_f64().unwrap(),
            config["benchmarkCosts"]["slipPct"].as_f64().unwrap()
        )
    );
    let report = &stored.report;
    assert_eq!(report["contractVersion"], "research-campaign-admission-v1");
    assert_eq!(report["snapshot"]["snapshotId"], workspace.snapshot_id);
    assert_eq!(report["ledger"]["batchId"], stored.batch_id);
    assert_eq!(report["ledger"]["familyEffectiveTrials"], 1);
    let jobs: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT DISTINCT candidate_index FROM discovery_jobs WHERE discovery_run_id = ?1 ORDER BY 1")
            .unwrap();
        let rows = stmt.query_map([run_id], |row| row.get(0)).unwrap();
        rows.collect::<Result<_, _>>().unwrap()
    };
    let admitted: Vec<i64> = report["walkForward"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["candidateIndex"].as_i64().unwrap())
        .collect();
    assert_eq!(
        admitted, jobs,
        "the decision covers exactly the queued candidates"
    );
    drop(conn);

    let payloads = registered_payloads(&workspace);
    assert_eq!(payloads.len(), jobs.len());
    assert!(payloads.iter().all(|payload| {
        payload["familyId"] == json!(family_id_for(BTC).unwrap())
            && payload["snapshotId"] == json!(workspace.snapshot_id)
    }));
    assert!(matches!(
        workspace
            .ledger
            .read_admission_count(&stored.batch_id)
            .unwrap(),
        Admission::Count(_)
    ));
    // An ordinary run has no decision.
    assert!(
        crate::db::campaign::get_campaign_admission(&workspace.db.lock().unwrap(), run_id + 1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn an_earlier_single_test_family_rises_to_two_tests_on_the_next_run() {
    // trial-ledger-v1 §22: a pre-P12e build pinned this family at one test
    // per trial. The next run registers two, raising the family's m.
    let workspace = workspace();
    workspace
        .ledger
        .register_batch(&TrialBatchInput {
            workspace_id: "earlier-build".into(),
            instrument_id: Some(BTC.into()),
            tests_per_trial: 1,
            events: vec![TrialEventInput {
                kind: TrialKind::Variant,
                origin: TrialOrigin::Request {
                    request_id: "earlier".into(),
                    candidate_index: 0,
                },
                hypothesis_hash: None,
                strategy_hash: Some("strategy-earlier".into()),
                dataset_hash: Some(workspace.dataset_hash.clone()),
                snapshot_id: Some(workspace.snapshot_id.clone()),
                split_hash: Some("split".into()),
                seeds_hash: Some("seeds".into()),
                engine_fingerprint_hash: Some("engine".into()),
                benchmark_id: None,
                benchmark_params_hash: None,
                reproduction_of: None,
                benchmark_evidence: None,
            }],
        })
        .unwrap();
    let config = walk_forward_runner_config(workspace.dataset_id, &workspace.dataset_hash);
    let run_id = start(&workspace, config, declaration(&workspace, 100_000, 200_000), BTC).unwrap();
    complete(&workspace, run_id);
    let stored = crate::db::campaign::get_campaign_admission(&workspace.db.lock().unwrap(), run_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::research::trial_ledger_workspace::DISCOVERY_TESTS_PER_TRIAL,
        2
    );
    assert_eq!(stored.report["ledger"]["testsPerTrial"], 2);
    assert_eq!(stored.report["ledger"]["familyEffectiveTrials"], 2);
    assert_eq!(stored.report["precision"]["familyTests"], 4, "m = 2 trials x 2 tests");
}

#[test]
fn a_not_eligible_campaign_run_still_explores() {
    let workspace = workspace();
    let config = walk_forward_runner_config(workspace.dataset_id, &workspace.dataset_hash);
    // Ten bootstrap samples cannot resolve even one test at alpha 5%.
    let run_id = start(&workspace, config, declaration(&workspace, 10, 10), BTC).unwrap();
    complete(&workspace, run_id);
    let stored = crate::db::campaign::get_campaign_admission(&workspace.db.lock().unwrap(), run_id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, "NOT_ELIGIBLE");
    assert_eq!(stored.report["reasons"], json!(["precision_not_eligible"]));
    let completed = count(
        &workspace.db,
        "SELECT COUNT(*) FROM research_attempts WHERE status = 'completed'",
    );
    assert_eq!(completed, 1, "exploration ran and its evidence was kept");
}

#[test]
fn a_run_that_contradicts_its_campaign_writes_nothing() {
    let workspace = workspace();
    let config = walk_forward_runner_config(workspace.dataset_id, &workspace.dataset_hash);
    let declared = declaration(&workspace, 100_000, 200_000);
    let mut other_policy = config.clone();
    other_policy["walkForward"]["minimumTrainBars"] = json!(21);
    let mut other_snapshot = declared.clone();
    other_snapshot["instruments"][0]["snapshotId"] = json!("f".repeat(64));
    let v2 = {
        let mut raw = runner_config(workspace.dataset_id, &workspace.dataset_hash, 1);
        raw["envelopeVersion"] = json!("discovery-config-v2");
        raw["contracts"]["enumeration"] = json!("discovery-enumeration-v2");
        raw["contracts"]["strategyDsl"] = json!("strategy-dsl-v1");
        raw
    };
    for (label, config, declared, instrument, expected) in [
        (
            "fold policy",
            other_policy,
            declared.clone(),
            BTC,
            "sample policy",
        ),
        (
            "undeclared instrument",
            config.clone(),
            declared.clone(),
            "crypto:binance:ETHUSDT",
            "not declared",
        ),
        (
            "missing snapshot",
            config.clone(),
            other_snapshot,
            BTC,
            "snapshot is missing",
        ),
        (
            "no fold declaration",
            v2,
            declared.clone(),
            BTC,
            "walk-forward declaration",
        ),
    ] {
        let error = start(&workspace, config, declared, instrument).unwrap_err();
        assert!(error.to_string().contains(expected), "{label}: {error}");
    }
    assert_eq!(
        count(&workspace.db, "SELECT COUNT(*) FROM discovery_runs"),
        0
    );
    assert_eq!(
        count(
            &workspace.db,
            "SELECT COUNT(*) FROM campaign_run_admissions"
        ),
        0
    );
    assert!(
        registered_payloads(&workspace).is_empty(),
        "no trial was registered"
    );
}

#[test]
fn a_dataset_with_two_snapshots_registers_under_the_declared_family() {
    let workspace = workspace();
    let second = build_snapshot(&workspace, "second");
    assert_ne!(second, workspace.snapshot_id);
    assert_eq!(
        unique_snapshot(
            &workspace.db.lock().unwrap(),
            workspace.dataset_id,
            &workspace.dataset_hash
        )
        .unwrap(),
        None,
        "an unbound run could not tell the family"
    );
    let config = walk_forward_runner_config(workspace.dataset_id, &workspace.dataset_hash);
    let run_id = start(
        &workspace,
        config,
        declaration(&workspace, 100_000, 200_000),
        BTC,
    )
    .unwrap();
    complete(&workspace, run_id);
    let stored = crate::db::campaign::get_campaign_admission(&workspace.db.lock().unwrap(), run_id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, "ELIGIBLE", "{}", stored.report["reasons"]);
    assert!(registered_payloads(&workspace).iter().all(|payload| {
        payload["familyId"] == json!(family_id_for(BTC).unwrap())
            && payload["snapshotId"] == json!(workspace.snapshot_id)
    }));
}

#[test]
fn the_enqueue_refuses_a_decision_for_missing_or_substituted_candidates() {
    let workspace = workspace();
    let mut raw = runner_config(workspace.dataset_id, &workspace.dataset_hash, 2);
    let v3 = walk_forward_runner_config(workspace.dataset_id, &workspace.dataset_hash);
    for key in ["envelopeVersion", "contracts", "walkForward"] {
        raw[key] = v3[key].clone();
    }
    let config = parse_discovery_config(&raw, 8.0).unwrap();
    let plan = enumerate_candidates(&config).unwrap();
    assert_eq!(plan.candidates.len(), 2);
    let campaign = CampaignStart {
        declaration: declaration(&workspace, 100_000, 200_000),
        instrument_id: BTC.into(),
    };
    let conn = workspace.db.lock().unwrap();
    let (dataset, candles) = load_verified_dataset(&conn, &config).unwrap();
    let bound = workspace
        .runner
        .bind_campaign(
            &conn,
            &campaign,
            &config,
            &plan.candidates,
            &dataset,
            candles.len(),
        )
        .unwrap();
    // The ledger half is irrelevant to candidate completeness.
    let blocked = Admission::Blocked(AdmissionBlocked::FamilyUnknown);
    let report = evaluate_campaign_admission(&CampaignAdmissionInput {
        campaign: &bound.frozen,
        instrument_id: BTC,
        resolved: &bound.resolved,
        earlier_snapshot: None,
        batch_id: "trial-batch-v1:test",
        ledger: LedgerInput::Registered(&blocked),
        legacy_trials_unknown: &[],
        candidates: &bound.candidates,
    })
    .unwrap();
    let run_id =
        discovery::create_discovery_run_with_outcomes(&conn, None, "campaign", "{}", &[]).unwrap();
    let scheduled: Vec<ScheduledCandidate> = plan
        .candidates
        .iter()
        .map(|candidate| ScheduledCandidate {
            candidate: candidate.clone(),
            strategy_id: 1,
        })
        .collect();
    let lineage = run_lineage(&config, &raw, run_id, &dataset, &scheduled, None).unwrap();
    let record = CampaignAdmissionRecord {
        campaign: &bound.frozen,
        report: &report,
        fee_pct: 0.1,
        slip_pct: 0.05,
    };
    let mut substituted = lineage.clone();
    substituted[1].1.input_fingerprint["strategyHash"] = json!("strategy-v2:substitute");
    for (label, lineage) in [
        ("missing", lineage[..1].to_vec()),
        ("substituted", substituted),
    ] {
        let error =
            crate::db::campaign::record_campaign_admission(&conn, None, run_id, &lineage, &record)
                .unwrap_err();
        assert!(
            error.to_string().contains("differ from the candidates"),
            "{label}: {error}"
        );
    }
    let rows = |sql: &str| conn.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap();
    assert_eq!(
        rows("SELECT COUNT(*) FROM research_campaigns"),
        0,
        "refused before any write"
    );

    crate::db::campaign::record_campaign_admission(&conn, None, run_id, &lineage, &record).unwrap();
    assert_eq!(rows("SELECT COUNT(*) FROM campaign_run_admissions"), 1);
    for sql in [
        "UPDATE research_campaigns SET version = 'x'",
        "DELETE FROM research_campaigns",
        "UPDATE campaign_run_admissions SET status = 'ELIGIBLE'",
        "DELETE FROM campaign_run_admissions",
    ] {
        let error = conn.execute(sql, []).unwrap_err();
        assert!(error.to_string().contains("append-only"), "{sql}: {error}");
    }
}

// ---------------------------- P12d-2d campaign.freeze / campaign.start

fn dispatcher(workspace: &Workspace) -> crate::runtime::commands::Dispatcher {
    crate::runtime::commands::Dispatcher::new(
        workspace.db.clone(),
        workspace.runner.clone(),
        workspace.epoch,
        workspace.workspace_id.clone(),
        Arc::new(RecordingSink::new(workspace.db.clone())),
        Arc::new(crate::runtime::commands::InFlightRequests::default()),
    )
}

fn envelope(workspace: &Workspace, request: &str, command: &str, payload: Value) -> Value {
    json!({
        "protocolVersion": crate::runtime::commands::COMMAND_PROTOCOL_VERSION,
        "workspaceId": workspace.workspace_id,
        "requestId": request,
        "command": command,
        "payload": payload,
    })
}

#[test]
fn freezing_saves_once_and_runs_start_from_the_saved_list() {
    use crate::runtime::commands::ErrorCode;
    let workspace = workspace();
    let dispatcher = dispatcher(&workspace);
    let declared = declaration(&workspace, 100_000, 200_000);
    let expected = freeze_campaign(&declared).unwrap().campaign_id().to_string();
    let freeze = |request: &str, payload: Value| {
        dispatcher.dispatch(envelope(&workspace, request, "campaign.freeze", payload))
    };

    let frozen = freeze("freeze-1", json!({ "declaration": declared })).unwrap();
    assert_eq!(frozen, json!({ "campaignId": expected }), "the backend derives the ID");
    assert_eq!(freeze("freeze-1", json!({ "declaration": declared })).unwrap(), frozen);
    assert_eq!(freeze("freeze-2", json!({ "declaration": declared })).unwrap(), frozen);
    assert_eq!(count(&workspace.db, "SELECT COUNT(*) FROM research_campaigns"), 1);
    let unknown_key = freeze("freeze-3", json!({ "declaration": declared, "campaignId": expected }));
    assert_eq!(unknown_key.unwrap_err().code, ErrorCode::Validation);
    let mut invalid = declared.clone();
    invalid["sampling"]["priorTrials"] = json!(0);
    assert_eq!(
        freeze("freeze-4", json!({ "declaration": invalid })).unwrap_err().code,
        ErrorCode::Validation
    );
    assert_eq!(count(&workspace.db, "SELECT COUNT(*) FROM research_campaigns"), 1);
    {
        let conn = workspace.db.lock().unwrap();
        let listed = crate::db::campaign::list_campaigns(&conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].campaign_id, expected);
        assert_eq!(listed[0].document, *freeze_campaign(&declared).unwrap().document());
        assert!(listed[0].runs.is_empty());
        assert!(crate::db::campaign::get_campaign(&conn, &"0".repeat(64)).unwrap().is_none());
    }

    let config = walk_forward_runner_config(workspace.dataset_id, &workspace.dataset_hash);
    let start = |request: &str, campaign_id: &str| {
        dispatcher.dispatch(envelope(
            &workspace,
            request,
            "campaign.start",
            json!({ "config": config, "campaignId": campaign_id, "instrumentId": BTC }),
        ))
    };
    let started = start("start-1", &expected).unwrap();
    let run_id = started["runId"].as_i64().unwrap();
    complete(&workspace, run_id);
    assert_eq!(start("start-1", &expected).unwrap(), started, "one run per request id");
    assert_eq!(count(&workspace.db, "SELECT COUNT(*) FROM discovery_runs"), 1);
    let (command, stage): (String, String) = workspace
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT command, stage FROM request_outcomes WHERE request_id = 'start-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((command.as_str(), stage.as_str()), ("campaign.start", "accepted"));
    {
        let conn = workspace.db.lock().unwrap();
        let listed = crate::db::campaign::list_campaigns(&conn).unwrap();
        assert_eq!(listed[0].runs.len(), 1);
        assert_eq!(
            (listed[0].runs[0].run_id, listed[0].runs[0].instrument_id.as_str(), listed[0].runs[0].status.as_str()),
            (run_id, BTC, "ELIGIBLE")
        );
    }

    let missing = start("start-2", &"0".repeat(64)).unwrap_err();
    assert_eq!(missing.code, ErrorCode::NotFound, "{}", missing.message);
    let wrong_shape = dispatcher.dispatch(envelope(
        &workspace,
        "start-3",
        "campaign.start",
        json!({ "config": config, "campaignId": expected }),
    ));
    assert_eq!(wrong_shape.unwrap_err().code, ErrorCode::Validation);
    assert_eq!(count(&workspace.db, "SELECT COUNT(*) FROM discovery_runs"), 1);
}

#[test]
fn a_stored_campaign_that_no_longer_refreezes_is_an_error() {
    let workspace = workspace();
    let declared = declaration(&workspace, 100_000, 200_000);
    let document = String::from_utf8(
        crate::research::canonical_json(freeze_campaign(&declared).unwrap().document()).unwrap(),
    )
    .unwrap();
    let conn = workspace.db.lock().unwrap();
    // A row whose ID is not its document's: listing must not skip or trust it.
    conn.execute(
        "INSERT INTO research_campaigns (campaign_id, version, document_json) VALUES (?1, ?2, ?3)",
        rusqlite::params!["a".repeat(64), "research-campaign-declaration-v1", document],
    )
    .unwrap();
    let error = crate::db::campaign::list_campaigns(&conn).unwrap_err();
    assert!(error.to_string().contains("no longer re-freezes"), "{error}");
    let error = crate::db::campaign::get_campaign(&conn, &"a".repeat(64)).unwrap_err();
    assert!(error.to_string().contains("no longer re-freezes"), "{error}");
}
