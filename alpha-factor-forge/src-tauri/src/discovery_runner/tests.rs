use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::*;
use crate::db::repositories::{self, Candle, Dataset};

const RUNNER_CONFIG_FIXTURE: &str = include_str!("../../../fixtures/rs-core/runner-config-v1.json");
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

fn migrated_db() -> SharedDb {
    let conn = Connection::open_in_memory().expect("open in-memory runner db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("enable foreign keys");
    crate::db::apply_migrations(&conn).expect("apply runner migrations");
    Arc::new(Mutex::new(conn))
}

fn alternating_candles(count: usize, start: i64) -> Vec<Candle> {
    (0..count)
        .map(|index| {
            let close = if index % 2 == 0 { 110.0 } else { 90.0 };
            Candle {
                timestamp: start + index as i64 * 86_400_000,
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: 1_000.0,
            }
        })
        .collect()
}

fn import_dataset(db: &SharedDb, candles: &[Candle]) -> (i64, String) {
    let mut dataset = Dataset {
        id: None,
        exchange: "runner-test".into(),
        symbol: "BTCUSDT".into(),
        interval: "1d".into(),
        start_time: candles.first().expect("first candle").timestamp,
        end_time: candles.last().expect("last candle").timestamp,
        candle_count: candles.len() as i64,
        source: "runner-acceptance-test".into(),
        dataset_hash: String::new(),
    };
    dataset.dataset_hash =
        crate::identity::dataset_content_hash(&dataset, candles).expect("hash test dataset");
    let hash = dataset.dataset_hash.clone();
    let id = repositories::import_dataset_with_candles(
        &mut db.lock().expect("lock db for dataset import"),
        &dataset,
        candles,
    )
    .expect("import test dataset");
    (id, hash)
}

fn runner_config(dataset_id: i64, dataset_hash: &str, candidate_count: usize) -> Value {
    assert!((1..=2).contains(&candidate_count));
    let fixture: Value =
        serde_json::from_str(RUNNER_CONFIG_FIXTURE).expect("parse runner config fixture");
    let mut input = fixture["enumerationCases"][0]["input"].clone();
    input["dataset"]["id"] = json!(dataset_id);
    input["dataset"]["contentHash"] = json!(dataset_hash);
    input["embargo"]["holdingAllowanceBars"] = json!(0);
    input["randomEntry"]["runs"] = json!(5);
    input["maxConcurrency"] = json!(1);

    let strategy = &mut input["bases"][0]["strategy"];
    strategy["fastMA"] = json!(1);
    strategy["slowMA"] = json!(3);
    strategy["emaPeriod"] = json!(2);
    strategy["rsiPeriod"] = json!(2);
    strategy["macdFast"] = json!(1);
    strategy["macdSlow"] = json!(2);
    strategy["macdSignal"] = json!(1);
    strategy["bbPeriod"] = json!(2);
    strategy["entrySig"] = json!("priceAboveSlow");
    strategy["exitSig"] = json!("priceBelowSlow");
    input["bases"][0]["axes"] = if candidate_count == 1 {
        json!([])
    } else {
        json!([{
            "key": "fastMA",
            "min": 1,
            "max": 2,
            "step": 1
        }])
    };
    input
}

#[derive(Clone)]
struct RecordedEvent {
    event: DiscoveryEvent,
    persisted_error: Option<String>,
}

struct RecordingSink {
    db: SharedDb,
    events: Mutex<Vec<RecordedEvent>>,
    changed: Condvar,
}

impl RecordingSink {
    fn new(db: SharedDb) -> Self {
        Self {
            db,
            events: Mutex::new(Vec::new()),
            changed: Condvar::new(),
        }
    }

    fn snapshot(&self) -> Vec<RecordedEvent> {
        self.events.lock().expect("lock recorded events").clone()
    }

    fn wait_for<F>(&self, predicate: F) -> DiscoveryEvent
    where
        F: Fn(&DiscoveryEvent) -> bool,
    {
        let deadline = Instant::now() + TEST_TIMEOUT;
        let mut events = self.events.lock().expect("lock recorded events");
        loop {
            if let Some(event) = events
                .iter()
                .map(|record| &record.event)
                .find(|event| predicate(event))
            {
                return event.clone();
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .expect("timed out waiting for discovery event");
            let (next, timeout) = self
                .changed
                .wait_timeout(events, remaining)
                .expect("wait for discovery event");
            events = next;
            assert!(
                !timeout.timed_out(),
                "timed out waiting for discovery event"
            );
        }
    }

    fn result_count(&self) -> usize {
        self.snapshot()
            .iter()
            .filter(|record| matches!(record.event, DiscoveryEvent::Result(_)))
            .count()
    }

    fn assert_all_observed_after_commit(&self) {
        let errors: Vec<_> = self
            .snapshot()
            .into_iter()
            .filter_map(|record| record.persisted_error)
            .collect();
        assert!(
            errors.is_empty(),
            "events observed uncommitted database state: {errors:?}"
        );
    }
}

impl DiscoveryEventSink for RecordingSink {
    fn emit(&self, event: &DiscoveryEvent) -> Result<(), String> {
        let persisted_error = match event {
            DiscoveryEvent::Result(payload) => {
                let conn = self
                    .db
                    .lock()
                    .map_err(|_| "db lock poisoned while observing result".to_string())?;
                let record_count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*)
                         FROM validation_records
                         WHERE id = ?1
                           AND discovery_run_id = ?2
                           AND strategy_id = ?3",
                        params![
                            payload.validation_record_id,
                            payload.run_id,
                            payload.strategy_id
                        ],
                        |row| row.get(0),
                    )
                    .map_err(|error| error.to_string())?;
                let committed_jobs: i64 = conn
                    .query_row(
                        "SELECT COUNT(*)
                         FROM discovery_jobs
                         WHERE discovery_run_id = ?1
                           AND candidate_index = ?2
                           AND status = 'done'
                           AND result_id IS NOT NULL",
                        params![payload.run_id, payload.candidate_index],
                        |row| row.get(0),
                    )
                    .map_err(|error| error.to_string())?;
                (record_count != 1 || committed_jobs != 2).then(|| {
                    format!(
                        "result sequence {} saw record_count={record_count}, committed_jobs={committed_jobs}",
                        payload.sequence
                    )
                })
            }
            DiscoveryEvent::Done(payload) => {
                let conn = self
                    .db
                    .lock()
                    .map_err(|_| "db lock poisoned while observing done".to_string())?;
                let status: String = conn
                    .query_row(
                        "SELECT status FROM discovery_runs WHERE id = ?1",
                        [payload.run_id],
                        |row| row.get(0),
                    )
                    .map_err(|error| error.to_string())?;
                (status != payload.status.as_str()).then(|| {
                    format!(
                        "done sequence {} reported {} while DB was {status}",
                        payload.sequence,
                        payload.status.as_str()
                    )
                })
            }
            DiscoveryEvent::Progress(_) => None,
        };
        self.events
            .lock()
            .map_err(|_| "event lock poisoned".to_string())?
            .push(RecordedEvent {
                event: event.clone(),
                persisted_error,
            });
        self.changed.notify_all();
        Ok(())
    }
}

fn wait_for_status(
    runner: &DiscoveryRunner,
    db: &SharedDb,
    run_id: i64,
    expected: RunStatus,
) -> DiscoveryProgressSnapshot {
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let progress = runner.progress(db, run_id).expect("read runner progress");
        if progress.status == expected {
            return progress;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected:?}; latest progress was {progress:?}"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_coordinator_exit(runner: &DiscoveryRunner, run_id: i64) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        if runner
            .control(run_id)
            .expect("read runner control")
            .is_none()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for coordinator {run_id} to exit"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

struct PermitGate {
    permits: Mutex<usize>,
    changed: Condvar,
}

impl PermitGate {
    fn new() -> Self {
        Self {
            permits: Mutex::new(0),
            changed: Condvar::new(),
        }
    }

    fn release(&self) {
        *self.permits.lock().expect("lock executor permits") += 1;
        self.changed.notify_one();
    }

    fn acquire(&self) -> Result<(), String> {
        let mut permits = self
            .permits
            .lock()
            .map_err(|_| "executor permit lock poisoned".to_string())?;
        while *permits == 0 {
            permits = self
                .changed
                .wait(permits)
                .map_err(|_| "executor permit lock poisoned while waiting".to_string())?;
        }
        *permits -= 1;
        Ok(())
    }
}

struct PermittedProductionExecutor {
    started: mpsc::Sender<i64>,
    gate: Arc<PermitGate>,
}

impl CandidateExecutor for PermittedProductionExecutor {
    fn execute(&self, work: &CandidateWork) -> Result<CandidateExecutionOutput, String> {
        self.started
            .send(work.candidate.index)
            .map_err(|_| "test executor start receiver dropped".to_string())?;
        self.gate.acquire()?;
        ProductionExecutor.execute(work)
    }
}

struct FailThenLateExecutor {
    started: mpsc::Sender<i64>,
    fail_gate: Arc<PermitGate>,
    late_gate: Arc<PermitGate>,
}

impl CandidateExecutor for FailThenLateExecutor {
    fn execute(&self, work: &CandidateWork) -> Result<CandidateExecutionOutput, String> {
        self.started
            .send(work.candidate.index)
            .map_err(|_| "test executor start receiver dropped".to_string())?;
        if work.candidate.index == 0 {
            self.fail_gate.acquire()?;
            Err("injected candidate failure".into())
        } else {
            self.late_gate.acquire()?;
            ProductionExecutor.execute(work)
        }
    }
}

fn runner_with_executor(executor: Arc<dyn CandidateExecutor>) -> DiscoveryRunner {
    DiscoveryRunner {
        executor,
        ..DiscoveryRunner::default()
    }
}

fn wait_for_phase(runner: &DiscoveryRunner, run_id: i64, expected: ControlPhase) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        let phase = runner
            .control(run_id)
            .expect("read runner control")
            .map(|control| {
                let phase = control
                    .state
                    .lock()
                    .expect("lock runner control for phase")
                    .phase;
                phase
            });
        if phase == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for control phase {expected:?}; latest phase was {phase:?}"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

type RunStateSnapshot = (
    RunStatus,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
);
type JobStateSnapshot = Vec<(
    i64,
    i64,
    Segment,
    JobStatus,
    Option<i64>,
    Option<String>,
)>;

/// Everything a rejected lifecycle call must leave untouched: run row, job
/// rows, and the connection's total write count.
fn run_snapshot(db: &SharedDb, run_id: i64) -> (RunStateSnapshot, JobStateSnapshot, i64) {
    let conn = db.lock().expect("lock db for run snapshot");
    let run = discovery::get_discovery_run(&conn, run_id).expect("read run for snapshot");
    let jobs = discovery::list_discovery_jobs(&conn, run_id)
        .expect("read jobs for snapshot")
        .into_iter()
        .map(|job| {
            (
                job.id,
                job.candidate_index,
                job.segment,
                job.status,
                job.result_id,
                job.error_message,
            )
        })
        .collect::<Vec<_>>();
    let total_changes = conn
        .query_row("SELECT total_changes()", [], |row| row.get::<_, i64>(0))
        .expect("read total changes for snapshot");
    (
        (
            run.status,
            run.config_json,
            run.progress_json,
            run.best_strategy_id,
            run.error_message,
        ),
        jobs,
        total_changes,
    )
}

/// Insert a dataset and its candles with RAW SQL, deliberately bypassing
/// `import_dataset_with_candles` and therefore the DATA-QUALITY-001 admission
/// gate. This is how data stored BEFORE the contract existed is simulated.
///
/// The dataset hash is computed over these exact (invalid) candles, because
/// hashing performs no semantic validation. That is what keeps the row
/// internally consistent so `verify_dataset_identity` still passes — without it
/// the fail-closed test would pass for the wrong reason, reporting an identity
/// mismatch instead of a market-data rejection.
fn insert_stored_dataset_bypassing_admission(db: &SharedDb, candles: &[Candle]) -> (i64, String) {
    let mut dataset = Dataset {
        id: None,
        exchange: "runner-test".into(),
        symbol: "BTCUSDT".into(),
        interval: "1d".into(),
        start_time: candles.iter().map(|c| c.timestamp).min().expect("min"),
        end_time: candles.iter().map(|c| c.timestamp).max().expect("max"),
        candle_count: candles.len() as i64,
        source: "pre-contract-storage".into(),
        dataset_hash: String::new(),
    };
    dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, candles)
        .expect("hash the invalid candles: hashing does not validate semantics");
    let hash = dataset.dataset_hash.clone();

    let conn = db.lock().expect("lock db for raw dataset insert");
    conn.execute(
        "INSERT INTO datasets
            (exchange, symbol, interval, start_time, end_time, candle_count, source, dataset_hash)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            dataset.exchange,
            dataset.symbol,
            dataset.interval,
            dataset.start_time,
            dataset.end_time,
            dataset.candle_count,
            dataset.source,
            dataset.dataset_hash
        ],
    )
    .expect("raw dataset insert");
    let dataset_id = conn.last_insert_rowid();
    {
        let mut statement = conn
            .prepare(
                "INSERT INTO candles
                    (dataset_id, timestamp, open, high, low, close, volume)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
            )
            .expect("prepare raw candle insert");
        // Sorted, so the stored order matches the identity normalization.
        let mut sorted = candles.to_vec();
        sorted.sort_by_key(|candle| candle.timestamp);
        for candle in &sorted {
            statement
                .execute(params![
                    dataset_id,
                    candle.timestamp,
                    candle.open,
                    candle.high,
                    candle.low,
                    candle.close,
                    candle.volume
                ])
                .expect("raw candle insert");
        }
    }
    drop(conn);
    (dataset_id, hash)
}

fn table_count(db: &SharedDb, table: &str) -> i64 {
    let conn = db.lock().expect("lock db for row count");
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap_or_else(|error| panic!("count {table}: {error}"))
}

/// DATA-QUALITY-001 (planning decision 2) — a dataset stored BEFORE the
/// market-data contract existed must fail closed at `load_verified_dataset`,
/// which runs before any run row is inserted. `start` therefore returns `Err`
/// and the correct assertion is that NOTHING was written at all.
///
/// The timestamp is below JavaScript's max safe integer, so identity accepts it
/// and the rejection can only come from the new validator. Rule 2 precedes rule
/// 3, so the reported rule is `timestamp_out_of_range` rather than the chrono
/// representability message — the metrics-layer chrono guard keeps its own
/// coverage in `execution.rs`.
#[test]
fn stored_invalid_dataset_fails_closed_on_start_without_writing_anything() {
    let candles = alternating_candles(240, 8_500_000_000_000_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = insert_stored_dataset_bypassing_admission(&db, &candles);
    let config = runner_config(dataset_id, &dataset_hash, 1);
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let runner = DiscoveryRunner::default();

    let total_changes_before = {
        let conn = db.lock().expect("lock before rejected start");
        conn.query_row("SELECT total_changes()", [], |row| row.get::<_, i64>(0))
            .expect("read total changes before start")
    };

    let error = runner
        .start(db.clone(), sink.clone(), config)
        .expect_err("stored invalid market data must reject start");
    let message = error.to_string();

    assert!(
        message.contains("timestamp_out_of_range"),
        "rejection did not name the market-data rule: {message}"
    );
    // The whole point of hashing over the invalid candles: this must NOT be an
    // identity failure, or the test would prove nothing about the validator.
    assert!(
        !message.contains("identity mismatch"),
        "rejection came from identity, not the market-data validator: {message}"
    );

    assert_eq!(table_count(&db, "discovery_runs"), 0, "a run row was written");
    assert_eq!(table_count(&db, "discovery_jobs"), 0, "a job row was written");
    assert_eq!(
        table_count(&db, "validation_records"),
        0,
        "a validation record was written"
    );
    assert_eq!(
        table_count(&db, "backtest_summary"),
        0,
        "a summary row was written"
    );
    let total_changes_after = {
        let conn = db.lock().expect("lock after rejected start");
        conn.query_row("SELECT total_changes()", [], |row| row.get::<_, i64>(0))
            .expect("read total changes after start")
    };
    assert_eq!(
        total_changes_after, total_changes_before,
        "rejected start performed a database write"
    );
    assert!(
        sink.snapshot().is_empty(),
        "rejected start emitted an event"
    );
    assert!(
        runner
            .control(1)
            .expect("read control after rejected start")
            .is_none(),
        "rejected start registered a coordinator control"
    );
}

/// The `resume` half of the same contract: a run that was legitimately started
/// and paused must not resume once its stored candles no longer satisfy the
/// market-data rules. The dataset hash and the run's config content hash are
/// both re-pointed at the corrupted payload, so identity still verifies and the
/// rejection can only come from the validator.
#[test]
fn stored_invalid_dataset_fails_closed_on_resume_without_changing_run_state() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let config = runner_config(dataset_id, &dataset_hash, 2);
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let runner = runner_with_executor(Arc::new(PermittedProductionExecutor {
        started: started_tx,
        gate: gate.clone(),
    }));

    let run_id = runner
        .start(db.clone(), sink.clone(), config)
        .expect("start valid runner");
    started_rx
        .recv_timeout(TEST_TIMEOUT)
        .expect("first candidate reached executor");

    let (pause_tx, pause_rx) = mpsc::channel();
    let pause_runner = runner.clone();
    let pause_db = db.clone();
    thread::spawn(move || {
        let _ = pause_tx.send(pause_runner.pause(&pause_db, run_id));
    });
    wait_for_phase(&runner, run_id, ControlPhase::PauseRequested);
    gate.release();
    pause_rx
        .recv_timeout(TEST_TIMEOUT)
        .expect("pause acknowledgement")
        .expect("pause succeeds after drain");
    wait_for_status(&runner, &db, run_id, RunStatus::Paused);
    wait_for_coordinator_exit(&runner, run_id);

    // Corrupt the STORED candles the way pre-contract data would already be
    // corrupt, then re-point identity at the corrupted payload so the run can
    // only be stopped by the market-data gate.
    let mut corrupted = candles.clone();
    corrupted.sort_by_key(|candle| candle.timestamp);
    corrupted[7].volume = -1.0;
    {
        let conn = db.lock().expect("lock paused run for corruption");
        conn.execute(
            "UPDATE candles SET volume = ?1 WHERE dataset_id = ?2 AND timestamp = ?3",
            params![corrupted[7].volume, dataset_id, corrupted[7].timestamp],
        )
        .expect("corrupt stored candle");

        let mut dataset = Dataset {
            id: Some(dataset_id),
            exchange: "runner-test".into(),
            symbol: "BTCUSDT".into(),
            interval: "1d".into(),
            start_time: corrupted.first().expect("first").timestamp,
            end_time: corrupted.last().expect("last").timestamp,
            candle_count: corrupted.len() as i64,
            source: "runner-acceptance-test".into(),
            dataset_hash: String::new(),
        };
        dataset.dataset_hash = crate::identity::dataset_content_hash(&dataset, &corrupted)
            .expect("hash the corrupted candles");
        conn.execute(
            "UPDATE datasets SET dataset_hash = ?1 WHERE id = ?2",
            params![dataset.dataset_hash, dataset_id],
        )
        .expect("re-point stored dataset hash");

        let run = discovery::get_discovery_run(&conn, run_id).expect("read paused run");
        let mut stored_config: Value =
            serde_json::from_str(&run.config_json).expect("parse stored config");
        stored_config["dataset"]["contentHash"] = json!(dataset.dataset_hash);
        conn.execute(
            "UPDATE discovery_runs SET config_json = ?1 WHERE id = ?2",
            params![
                serde_json::to_string(&stored_config).expect("serialize config"),
                run_id
            ],
        )
        .expect("re-point run config content hash");
    }

    let (run_before, jobs_before, total_changes_before) = run_snapshot(&db, run_id);
    let event_count_before = sink.snapshot().len();
    assert!(runner
        .control(run_id)
        .expect("read control before resume")
        .is_none());

    let error = runner
        .resume(db.clone(), sink.clone(), run_id)
        .expect_err("stored invalid market data must reject resume");
    let message = error.to_string();
    assert!(
        message.contains("volume_negative"),
        "rejection did not name the market-data rule: {message}"
    );
    assert!(
        !message.contains("identity mismatch"),
        "rejection came from identity, not the market-data validator: {message}"
    );

    let (run_after, jobs_after, total_changes_after) = run_snapshot(&db, run_id);
    assert_eq!(
        run_after.0,
        RunStatus::Paused,
        "rejected resume did not leave the run paused"
    );
    assert_eq!(
        run_after, run_before,
        "rejected resume changed run/progress state"
    );
    assert_eq!(jobs_after, jobs_before, "rejected resume changed job state");
    assert_eq!(
        total_changes_after, total_changes_before,
        "rejected resume performed a database write"
    );
    assert_eq!(
        sink.snapshot().len(),
        event_count_before,
        "rejected resume emitted an event"
    );
    assert!(
        runner
            .control(run_id)
            .expect("read control after rejected resume")
            .is_none(),
        "rejected resume registered a coordinator control"
    );
}

#[test]
fn late_worker_result_after_cancel_is_discarded() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let config = runner_config(dataset_id, &dataset_hash, 1);
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let runner = runner_with_executor(Arc::new(PermittedProductionExecutor {
        started: started_tx,
        gate: gate.clone(),
    }));

    let run_id = runner
        .start(db.clone(), sink.clone(), config)
        .expect("start cancellable runner");
    assert_eq!(
        started_rx
            .recv_timeout(TEST_TIMEOUT)
            .expect("worker reached blocked executor"),
        0
    );

    runner
        .cancel(&db, sink.clone(), run_id)
        .expect("cancel blocked runner");
    gate.release();
    wait_for_coordinator_exit(&runner, run_id);
    let progress = wait_for_status(&runner, &db, run_id, RunStatus::Cancelled);

    assert_eq!(progress.counts.skipped_candidates, 1);
    assert_eq!(sink.result_count(), 0, "late result must not emit success");
    let conn = db.lock().expect("lock cancelled runner db");
    let records: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM validation_records WHERE discovery_run_id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .expect("count cancelled validation records");
    assert_eq!(records, 0, "late result must not persist evidence");
    drop(conn);
    sink.wait_for(|event| {
        matches!(
            event,
            DiscoveryEvent::Done(payload) if payload.status == RunStatus::Cancelled
        )
    });
    sink.assert_all_observed_after_commit();
}

#[test]
fn late_worker_result_after_failure_is_discarded() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let mut config = runner_config(dataset_id, &dataset_hash, 2);
    config["maxConcurrency"] = json!(2);
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let (started_tx, started_rx) = mpsc::channel();
    let fail_gate = Arc::new(PermitGate::new());
    let late_gate = Arc::new(PermitGate::new());
    let runner = runner_with_executor(Arc::new(FailThenLateExecutor {
        started: started_tx,
        fail_gate: fail_gate.clone(),
        late_gate: late_gate.clone(),
    }));

    let run_id = runner
        .start(db.clone(), sink.clone(), config)
        .expect("start failure-race runner");
    let mut started = vec![
        started_rx
            .recv_timeout(TEST_TIMEOUT)
            .expect("first worker started"),
        started_rx
            .recv_timeout(TEST_TIMEOUT)
            .expect("second worker started"),
    ];
    started.sort_unstable();
    assert_eq!(started, vec![0, 1]);

    fail_gate.release();
    let failed = wait_for_status(&runner, &db, run_id, RunStatus::Failed);
    assert_eq!(failed.counts.failed_candidates, 2);
    assert!(failed
        .error_message
        .as_deref()
        .is_some_and(|message| message.contains("injected candidate failure")));
    sink.wait_for(|event| {
        matches!(
            event,
            DiscoveryEvent::Done(payload) if payload.status == RunStatus::Failed
        )
    });

    late_gate.release();
    wait_for_coordinator_exit(&runner, run_id);
    assert_eq!(sink.result_count(), 0, "late result must not emit success");
    let records: i64 = db
        .lock()
        .expect("lock failed runner db")
        .query_row(
            "SELECT COUNT(*) FROM validation_records WHERE discovery_run_id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .expect("count failed-run validation records");
    assert_eq!(records, 0, "late result must not persist evidence");
    sink.assert_all_observed_after_commit();
}

#[test]
fn pause_drains_in_flight_candidate_and_resume_is_explicit() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let config = runner_config(dataset_id, &dataset_hash, 2);
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let runner = runner_with_executor(Arc::new(PermittedProductionExecutor {
        started: started_tx,
        gate: gate.clone(),
    }));

    let run_id = runner
        .start(db.clone(), sink.clone(), config)
        .expect("start pausable runner");
    assert_eq!(
        started_rx
            .recv_timeout(TEST_TIMEOUT)
            .expect("first candidate reached executor"),
        0
    );

    let (pause_tx, pause_rx) = mpsc::channel();
    let pause_runner = runner.clone();
    let pause_db = db.clone();
    thread::spawn(move || {
        let _ = pause_tx.send(pause_runner.pause(&pause_db, run_id));
    });
    wait_for_phase(&runner, run_id, ControlPhase::PauseRequested);
    assert!(
        pause_rx.try_recv().is_err(),
        "pause returned before in-flight work drained"
    );

    gate.release();
    pause_rx
        .recv_timeout(TEST_TIMEOUT)
        .expect("pause acknowledgement")
        .expect("pause succeeds after drain");
    let paused = wait_for_status(&runner, &db, run_id, RunStatus::Paused);
    assert_eq!(paused.counts.completed_candidates, 1);
    assert_eq!(paused.counts.queued_candidates, 1);
    assert_eq!(paused.counts.running_candidates, 0);

    runner
        .resume(db.clone(), sink.clone(), run_id)
        .expect("explicitly resume paused runner");
    assert_eq!(
        started_rx
            .recv_timeout(TEST_TIMEOUT)
            .expect("queued candidate starts only after resume"),
        1
    );
    gate.release();
    let completed = wait_for_status(&runner, &db, run_id, RunStatus::Completed);
    wait_for_coordinator_exit(&runner, run_id);

    assert_eq!(completed.counts.completed_candidates, 2);
    assert_eq!(completed.counts.queued_candidates, 0);
    assert_eq!(sink.result_count(), 2);
    sink.wait_for(|event| {
        matches!(
            event,
            DiscoveryEvent::Done(payload) if payload.status == RunStatus::Completed
        )
    });
    sink.assert_all_observed_after_commit();
}

#[test]
fn result_and_done_events_observe_committed_database_state() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let config = runner_config(dataset_id, &dataset_hash, 1);
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let runner = DiscoveryRunner::default();

    let run_id = runner
        .start(db.clone(), sink.clone(), config)
        .expect("start successful runner");
    wait_for_status(&runner, &db, run_id, RunStatus::Completed);
    wait_for_coordinator_exit(&runner, run_id);
    sink.wait_for(|event| matches!(event, DiscoveryEvent::Result(_)));
    sink.wait_for(|event| {
        matches!(
            event,
            DiscoveryEvent::Done(payload) if payload.status == RunStatus::Completed
        )
    });

    assert_eq!(sink.result_count(), 1);
    sink.assert_all_observed_after_commit();
}

#[test]
fn resume_rejects_a_paused_run_with_the_stale_metrics_contract_without_writes() {
    let candles = alternating_candles(240, 1_577_836_800_000);
    let db = migrated_db();
    let (dataset_id, dataset_hash) = import_dataset(&db, &candles);
    let config = runner_config(dataset_id, &dataset_hash, 2);
    let sink = Arc::new(RecordingSink::new(db.clone()));
    let (started_tx, started_rx) = mpsc::channel();
    let gate = Arc::new(PermitGate::new());
    let runner = runner_with_executor(Arc::new(PermittedProductionExecutor {
        started: started_tx,
        gate: gate.clone(),
    }));

    let run_id = runner
        .start(db.clone(), sink.clone(), config)
        .expect("start stale-contract runner");
    started_rx
        .recv_timeout(TEST_TIMEOUT)
        .expect("first candidate reached executor");

    let (pause_tx, pause_rx) = mpsc::channel();
    let pause_runner = runner.clone();
    let pause_db = db.clone();
    thread::spawn(move || {
        let _ = pause_tx.send(pause_runner.pause(&pause_db, run_id));
    });
    wait_for_phase(&runner, run_id, ControlPhase::PauseRequested);
    gate.release();
    pause_rx
        .recv_timeout(TEST_TIMEOUT)
        .expect("pause acknowledgement")
        .expect("pause succeeds after drain");
    wait_for_status(&runner, &db, run_id, RunStatus::Paused);
    wait_for_coordinator_exit(&runner, run_id);

    {
        let conn = db.lock().expect("lock paused stale-contract run");
        let run = discovery::get_discovery_run(&conn, run_id).expect("read paused run");
        let mut stale_config: Value =
            serde_json::from_str(&run.config_json).expect("parse stored config");
        stale_config["contracts"]["metrics"] = json!("metrics-v1");
        conn.execute(
            "UPDATE discovery_runs SET config_json = ?1 WHERE id = ?2",
            params![
                serde_json::to_string(&stale_config).expect("serialize stale config"),
                run_id
            ],
        )
        .expect("persist stale metrics contract");
    }

    let (run_before, jobs_before, total_changes_before) = {
        let conn = db.lock().expect("lock before rejected resume");
        let run = discovery::get_discovery_run(&conn, run_id).expect("read run before resume");
        let jobs = discovery::list_discovery_jobs(&conn, run_id)
            .expect("read jobs before resume")
            .into_iter()
            .map(|job| {
                (
                    job.id,
                    job.candidate_index,
                    job.segment,
                    job.status,
                    job.result_id,
                    job.error_message,
                )
            })
            .collect::<Vec<_>>();
        (
            (
                run.status,
                run.config_json,
                run.progress_json,
                run.best_strategy_id,
                run.error_message,
            ),
            jobs,
            conn.query_row("SELECT total_changes()", [], |row| row.get::<_, i64>(0))
                .expect("read total changes before resume"),
        )
    };
    let event_count_before = sink.snapshot().len();
    assert!(runner
        .control(run_id)
        .expect("read control before resume")
        .is_none());

    let error = runner
        .resume(db.clone(), sink.clone(), run_id)
        .expect_err("stale metrics contract must reject resume");
    let message = error.to_string();
    assert!(
        message.contains("contracts.metrics"),
        "unexpected stale-contract error: {message}"
    );
    assert!(message.contains("metrics-v2"));
    assert!(message.contains("metrics-v1"));

    let (run_after, jobs_after, total_changes_after) = {
        let conn = db.lock().expect("lock after rejected resume");
        let run = discovery::get_discovery_run(&conn, run_id).expect("read run after resume");
        let jobs = discovery::list_discovery_jobs(&conn, run_id)
            .expect("read jobs after resume")
            .into_iter()
            .map(|job| {
                (
                    job.id,
                    job.candidate_index,
                    job.segment,
                    job.status,
                    job.result_id,
                    job.error_message,
                )
            })
            .collect::<Vec<_>>();
        (
            (
                run.status,
                run.config_json,
                run.progress_json,
                run.best_strategy_id,
                run.error_message,
            ),
            jobs,
            conn.query_row("SELECT total_changes()", [], |row| row.get::<_, i64>(0))
                .expect("read total changes after resume"),
        )
    };

    assert_eq!(
        run_after.0,
        RunStatus::Paused,
        "rejected resume did not leave the run paused"
    );
    assert_eq!(
        run_after, run_before,
        "rejected resume changed run/progress state"
    );
    assert_eq!(jobs_after, jobs_before, "rejected resume changed job state");
    assert_eq!(
        total_changes_after, total_changes_before,
        "rejected resume performed a database write"
    );
    assert_eq!(
        sink.snapshot().len(),
        event_count_before,
        "rejected resume emitted an event"
    );
    assert!(
        runner
            .control(run_id)
            .expect("read control after rejected resume")
            .is_none(),
        "rejected resume registered a coordinator control"
    );
}
