//! P05: the research history the runner writes — a hypothesis frozen with
//! the enqueue, one attempt per candidate with its fingerprints, an
//! immutable artifact per completed candidate — and the acceptance of plan
//! phase P05: a re-run never overwrites an earlier attempt's detail, a
//! failure is traceable per attempt, and the latest projection agrees with
//! the latest attempt's artifact.
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::db::ownership::{acquire, HolderKind};
use crate::research::artifacts::ArtifactStore;
use crate::research::history::{self, AttemptFilter, AttemptStatus, HypothesisDraft};
use crate::research::CANDIDATE_RESULT_VERSION;

fn fresh_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("aff-history-test-{}-{n}", std::process::id()))
}

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        if self.0.exists() {
            std::fs::remove_dir_all(&self.0)
                .unwrap_or_else(|error| panic!("temp dir {} not removed: {error}", self.0.display()));
        }
    }
}

/// An owned in-memory workspace whose runner keeps artifacts under `dir`.
fn owned_runner(db: &SharedDb, dir: &std::path::Path, executor: Arc<dyn CandidateExecutor>) -> DiscoveryRunner {
    let epoch = acquire(&mut db.lock().unwrap(), HolderKind::DesktopEmbedded, 1).unwrap().epoch;
    DiscoveryRunner { executor, ..DiscoveryRunner::with_epoch(epoch) }
        .with_artifact_store(ArtifactStore::in_data_dir(dir))
}

fn run_to_completion(runner: &DiscoveryRunner, db: &SharedDb, config: Value) -> i64 {
    let run_id = runner.start(db.clone(), Arc::new(RecordingSink::new(db.clone())), config).expect("start");
    wait_for_status(runner, db, run_id, RunStatus::Completed);
    wait_for_coordinator_exit(runner, run_id);
    run_id
}

fn attempts_of(db: &SharedDb, run_id: i64) -> Vec<history::AttemptRow> {
    let conn = db.lock().unwrap();
    let mut rows = history::list_attempts(&conn, &AttemptFilter { discovery_run_id: Some(run_id), ..Default::default() }).unwrap();
    rows.sort_by_key(|row| row.candidate_index);
    rows
}

fn projection(db: &SharedDb, strategy_id: i64, dataset_id: i64, segment: &str) -> (i64, Option<f64>, Option<i64>, Option<f64>) {
    db.lock()
        .unwrap()
        .query_row(
            "SELECT id, net_return, trade_count, score FROM backtest_summary
             WHERE strategy_id = ?1 AND dataset_id = ?2 AND segment = ?3",
            rusqlite::params![strategy_id, dataset_id, segment],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
}

#[test]
fn a_run_freezes_its_hypothesis_and_one_attempt_per_candidate_with_the_enqueue() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let runner = owned_runner(&db, &dir, Arc::new(PermittedProductionExecutor { started: started_tx, gate: gate.clone() }));

    let run_id = runner
        .start(db.clone(), Arc::new(RecordingSink::new(db.clone())), runner_config(dataset_id, &dataset_hash, 2))
        .unwrap();
    // Before any candidate ran: the hypothesis and both attempts exist.
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);
    let attempts = attempts_of(&db, run_id);
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].status, AttemptStatus::Running, "candidate 0 is claimed");
    assert_eq!(attempts[1].status, AttemptStatus::Submitted, "candidate 1 waits");
    assert_eq!(attempts[0].attempt_key, "run:1:candidate:0");
    assert_eq!(attempts[0].hypothesis_id, attempts[1].hypothesis_id, "both candidates vary ONE base: one hypothesis");
    assert_eq!(attempts[0].input_fingerprint["datasetHash"], dataset_hash);
    assert_ne!(attempts[0].input_fingerprint["strategyHash"], attempts[1].input_fingerprint["strategyHash"]);
    assert_eq!(attempts[0].input_fingerprint["configHash"], attempts[1].input_fingerprint["configHash"]);
    assert_eq!(attempts[0].engine_fingerprint["package"], env!("CARGO_PKG_VERSION"));
    assert!(attempts[0].engine_fingerprint["configContracts"]["gate"].is_string());
    assert!(attempts[0].result_artifact.is_none());
    let hypothesis = {
        let conn = db.lock().unwrap();
        history::get_hypothesis(&conn, attempts[0].hypothesis_id).unwrap().expect("frozen before execution")
    };
    assert_eq!(hypothesis.source, "discovery");
    assert_eq!(hypothesis.variation_kind.as_deref(), Some("param-sweep:fastMA"));
    assert!(hypothesis.mechanism.contains("entry on priceAboveSlow"), "{}", hypothesis.mechanism);
    assert_eq!(hypothesis.applicability["datasetHash"], dataset_hash);
    assert!(hypothesis.strategy_hash.starts_with("strategy-doc-v1:"));

    gate.release();
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 1);
    gate.release();
    wait_for_status(&runner, &db, run_id, RunStatus::Completed);
    wait_for_coordinator_exit(&runner, run_id);

    // Completed: every attempt names its artifact, and the artifact is the
    // complete result, readable and checksum-verified.
    let attempts = attempts_of(&db, run_id);
    let store = runner.artifact_store().unwrap();
    for attempt in &attempts {
        assert_eq!(attempt.status, AttemptStatus::Completed);
        assert!(attempt.finished_at.is_some());
        let stored = attempt.result_artifact.as_ref().expect("an artifact per completed attempt");
        assert_eq!(stored.reference.kind, CANDIDATE_RESULT_VERSION);
        let document: Value = serde_json::from_slice(&store.read(&stored.reference).unwrap()).unwrap();
        assert_eq!(document["version"], CANDIDATE_RESULT_VERSION);
        assert_eq!(document["attemptKey"], attempt.attempt_key);
        assert_eq!(document["strategy"]["id"], attempt.strategy_id);
        assert!(document["train"]["summary"]["trade_count"].is_number());
        assert!(document["validation"]["trades"].is_array());
        assert_eq!(document["record"]["strategy_id"], attempt.strategy_id);
        let outcome = attempt.outcome.as_ref().unwrap();
        assert_eq!(outcome["gatePassed"], document["digest"]["gatePassed"]);
        assert!(outcome["recordId"].is_number());
    }
    // Nothing unreferenced is left behind by a clean run.
    let referenced = history::referenced_artifact_paths(&db.lock().unwrap()).unwrap();
    assert_eq!(referenced.len(), 2);
    assert_eq!(store.unreferenced(&referenced).unwrap(), Vec::<String>::new());
}

/// P05 acceptance: "a re-run does not overwrite the old detail; old and new
/// projections agree". The projection tables are upserted by the second run
/// (same strategy, dataset, segment → same summary id); the first run's
/// attempt, artifact, and full trades are still there, byte for byte.
#[test]
fn a_rerun_replaces_the_projection_but_keeps_the_earlier_attempts_artifact() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let runner = owned_runner(&db, &dir, Arc::new(ProductionExecutor));
    let store = runner.artifact_store().unwrap().clone();

    let first_run = run_to_completion(&runner, &db, runner_config(dataset_id, &dataset_hash, 1));
    let first = attempts_of(&db, first_run).remove(0);
    let first_artifact = first.result_artifact.clone().unwrap();
    let first_bytes = store.read(&first_artifact.reference).unwrap();
    let first_projection = projection(&db, first.strategy_id, dataset_id, "train");

    let second_run = run_to_completion(&runner, &db, runner_config(dataset_id, &dataset_hash, 1));
    assert_ne!(second_run, first_run);
    let second = attempts_of(&db, second_run).remove(0);
    assert_eq!(second.strategy_id, first.strategy_id, "the same candidate strategy row");
    assert_eq!(second.hypothesis_id, first.hypothesis_id, "the same hypothesis, not a second copy");
    assert_ne!(second.id, first.id, "a new attempt");
    let second_artifact = second.result_artifact.clone().unwrap();
    assert_ne!(second_artifact.reference.sha256, first_artifact.reference.sha256, "each attempt's own record");

    // The projection was overwritten in place (same row id, same key)...
    let second_projection = projection(&db, first.strategy_id, dataset_id, "train");
    assert_eq!(second_projection.0, first_projection.0, "the projection row is reused");
    // ...and the first attempt's record is untouched and still readable.
    assert_eq!(store.read(&first_artifact.reference).unwrap(), first_bytes);
    let refreshed = {
        let conn = db.lock().unwrap();
        history::get_attempt(&conn, first.id).unwrap().unwrap()
    };
    assert_eq!(refreshed, first, "the earlier attempt row did not change");
    let hypotheses = history::list_hypotheses(&db.lock().unwrap(), 10).unwrap();
    assert_eq!(hypotheses.len(), 1);

    // Old and new projections agree with their attempts: the projection now
    // shows what the SECOND attempt's artifact holds, and the first artifact
    // holds what the projection showed after the first run.
    let second_document: Value = serde_json::from_slice(&store.read(&second_artifact.reference).unwrap()).unwrap();
    let first_document: Value = serde_json::from_slice(&first_bytes).unwrap();
    let field = |document: &Value, name: &str| document["train"]["summary"][name].clone();
    assert_eq!(field(&second_document, "net_return").as_f64(), second_projection.1);
    assert_eq!(field(&second_document, "trade_count").as_i64(), second_projection.2);
    assert_eq!(field(&second_document, "score").as_f64(), second_projection.3);
    assert_eq!(field(&first_document, "net_return").as_f64(), first_projection.1);
    assert_eq!(field(&first_document, "trade_count").as_i64(), first_projection.2);
    // Deterministic engine: the two attempts computed the same numbers, and
    // only their identity differs — which is exactly why both must be kept.
    assert_eq!(first_document["train"], second_document["train"]);
    assert_ne!(first_document["attemptKey"], second_document["attemptKey"]);
}

/// P05 acceptance: "a failure is traceable". The candidate that failed and
/// the ones the run's failure took with it each keep the reason; a
/// cancelled run's unfinished attempts are skipped with theirs. Terminal
/// rows never change afterwards.
#[test]
fn failed_and_cancelled_attempts_keep_their_reasons_and_stay_frozen() {
    let dir = fresh_dir();
    let _guard = TempDir(dir.clone());
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);

    // Failure: candidate 0 fails, the run fails, candidate 1 never ran.
    let (started_tx, started_rx) = mpsc::channel();
    let fail_gate = Arc::new(PermitGate::new());
    let late_gate = Arc::new(PermitGate::new());
    let runner = owned_runner(
        &db,
        &dir,
        Arc::new(FailThenLateExecutor { started: started_tx, fail_gate: fail_gate.clone(), late_gate: late_gate.clone() }),
    );
    let mut config = runner_config(dataset_id, &dataset_hash, 2);
    config["maxConcurrency"] = json!(1);
    let run_id = runner.start(db.clone(), Arc::new(RecordingSink::new(db.clone())), config).unwrap();
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);
    fail_gate.release();
    wait_for_status(&runner, &db, run_id, RunStatus::Failed);
    wait_for_coordinator_exit(&runner, run_id);
    let attempts = attempts_of(&db, run_id);
    assert_eq!(attempts.len(), 2);
    for attempt in &attempts {
        assert_eq!(attempt.status, AttemptStatus::Failed, "{attempt:?}");
        let reason = attempt.outcome.as_ref().unwrap()["error"].as_str().unwrap().to_string();
        assert!(reason.contains("candidate 0 execution failed") && reason.contains("injected candidate failure"), "{reason}");
        assert!(attempt.result_artifact.is_none());
    }
    late_gate.release();

    // Cancel: a paused run's remaining candidates are skipped with the reason.
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let runner = DiscoveryRunner {
        executor: Arc::new(PermittedProductionExecutor { started: started_tx, gate: gate.clone() }),
        ..runner
    };
    let run_id = runner
        .start(db.clone(), Arc::new(RecordingSink::new(db.clone())), runner_config(dataset_id, &dataset_hash, 2))
        .unwrap();
    assert_eq!(started_rx.recv_timeout(TEST_TIMEOUT).unwrap(), 0);
    runner.cancel(&db, Arc::new(RecordingSink::new(db.clone())), run_id).unwrap();
    gate.release();
    wait_for_status(&runner, &db, run_id, RunStatus::Cancelled);
    wait_for_coordinator_exit(&runner, run_id);
    let attempts = attempts_of(&db, run_id);
    assert!(attempts.iter().all(|attempt| attempt.status == AttemptStatus::Skipped), "{attempts:?}");
    assert_eq!(attempts[1].outcome.as_ref().unwrap()["reason"], "run cancelled before this candidate ran");

    // Frozen: the triggers refuse every edit of a terminal row, of a frozen
    // column, and every delete — for attempts, hypotheses, and artifacts.
    let conn = db.lock().unwrap();
    let refused = |sql: &str| conn.execute(sql, []).expect_err(sql).to_string();
    assert!(refused("UPDATE research_attempts SET status = 'completed' WHERE status = 'skipped'").contains("frozen"));
    assert!(refused("UPDATE research_attempts SET status = 'submitted' WHERE status = 'failed'").contains("frozen"));
    assert!(refused("UPDATE research_attempts SET input_fingerprint_json = '{}'").contains("frozen"));
    assert!(refused("DELETE FROM research_attempts").contains("never deleted"));
    assert!(refused("UPDATE hypotheses SET mechanism = 'rewritten after the fact'").contains("immutable"));
    assert!(refused("DELETE FROM hypotheses").contains("never deleted"));
}

/// The hypothesis is registered once per content; the same draft is the
/// same row, a different one is a new row, and a draft without a mechanism
/// or failure modes is refused.
#[test]
fn a_hypothesis_registers_once_per_content_and_must_state_its_mechanism_and_failure_modes() {
    let db = migrated_db();
    let conn = db.lock().unwrap();
    let draft = HypothesisDraft {
        source: "manual".into(),
        mechanism: "trend persistence after a breakout".into(),
        applicability: json!({ "interval": "1d" }),
        failure_modes: "range-bound regimes; fee drag on frequent exits".into(),
        strategy_hash: "strategy-v2:abc".into(),
        strategy_id: None,
        parent_strategy_id: None,
        variation_kind: Some("manual".into()),
    };
    let (id, created) = history::register_hypothesis(&conn, &draft).unwrap();
    assert!(created);
    let (again, created_again) = history::register_hypothesis(&conn, &draft).unwrap();
    assert_eq!((again, created_again), (id, false), "a repeated request adds nothing");
    let mut varied = draft.clone();
    varied.failure_modes.push_str("; and low liquidity");
    let (other, _) = history::register_hypothesis(&conn, &varied).unwrap();
    assert_ne!(other, id);
    assert_eq!(history::list_hypotheses(&conn, 10).unwrap().len(), 2);
    let row = history::get_hypothesis(&conn, id).unwrap().unwrap();
    assert_eq!(row.hypothesis_hash, draft.hash().unwrap());
    assert_eq!(row.version, crate::research::HYPOTHESIS_VERSION);

    let mut blank = draft.clone();
    blank.failure_modes = "  ".into();
    assert!(history::register_hypothesis(&conn, &blank).is_err());
    let mut bad_source = draft.clone();
    bad_source.source = "oracle".into();
    assert!(history::register_hypothesis(&conn, &bad_source).is_err());
}

/// Crash recovery requeues an interrupted attempt with its jobs: the same
/// attempt (same key, same fingerprints) is submitted again, not a new one.
#[test]
fn recovery_requeues_an_interrupted_attempt_as_the_same_attempt() {
    let db = migrated_db();
    {
        let conn = db.lock().unwrap();
        conn.execute_batch(
            "INSERT INTO datasets (exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash)
             VALUES ('t', 'T', '1h', 1, 2, 0, 'import', 'dataset-content-v2:aa');
             INSERT INTO strategy_def (name, type, original_definition_json, source, strategy_hash)
             VALUES ('s', 'params', '{}', 'manual', 'strategy-v2:bb');
             INSERT INTO discovery_runs (id, name, status, config_json, started_at)
             VALUES (7, 'orphan', 'running', '{}', datetime('now'));
             INSERT INTO discovery_jobs (discovery_run_id, strategy_id, dataset_id, segment, status, candidate_index)
             VALUES (7, 1, 1, 'train', 'running', 0), (7, 1, 1, 'validation', 'running', 0);
             INSERT INTO hypotheses (hypothesis_hash, version, source, mechanism, applicability_json, failure_modes, strategy_hash, content_json)
             VALUES ('h', 'hypothesis-v1', 'discovery', 'm', '{}', 'f', 'strategy-doc-v1:x', '{}');
             INSERT INTO research_attempts (attempt_key, hypothesis_id, strategy_id, dataset_id, discovery_run_id, candidate_index, status, input_fingerprint_json, engine_fingerprint_json)
             VALUES ('run:7:candidate:0', 1, 1, 1, 7, 0, 'running', '{\"k\":1}', '{\"e\":1}');",
        )
        .unwrap();
    }
    let report = DiscoveryRunner::default().recover_orphans(&db).unwrap();
    assert_eq!(report, RecoveryReport { runs_paused: 1, jobs_requeued: 2 });
    let conn = db.lock().unwrap();
    let attempt = history::get_attempt_by_key(&conn, "run:7:candidate:0").unwrap().unwrap();
    assert_eq!(attempt.status, AttemptStatus::Submitted);
    assert_eq!(attempt.input_fingerprint, json!({ "k": 1 }), "the same attempt, fingerprints intact");
    assert_eq!(history::list_attempts(&conn, &AttemptFilter::default()).unwrap().len(), 1, "no second attempt");
}
