//! RUNNER-EXEC-001: backend discovery orchestration.
//!
//! The pure computation stays in `execution`; this module owns the fixed CPU
//! worker pool, cooperative controls, the single SQLite coordinator/writer,
//! checkpointed progress, and post-commit Tauri events. Compute workers only
//! receive immutable owned/`Arc` data and never receive a database handle.

pub(crate) mod execution;
/// RUNNER-UI-001a: asserts the emitted `discovery-event-v1` JSON against the
/// authored fixture the TypeScript client parses. Test-only; no runtime effect.
#[cfg(test)]
mod event_contract_tests;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use alpha_factor_forge::discovery_core::config::{parse_discovery_config, ResolvedDiscoveryConfig};
use alpha_factor_forge::discovery_core::enumerate::{
    enumerate_candidates, CandidatePlan, EnumeratedCandidate, EnumerationCounts,
};
use alpha_factor_forge::discovery_core::market_data;
use alpha_factor_forge::discovery_core::types::Candle as CoreCandle;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::db::discovery::{
    self, CandidateAssessment, CandidateJobSpec, ClaimedCandidateJobs, DiscoveryJobRow,
    DiscoveryRunRow, JobStatus, RecoveryReport, RunStatus, Segment, DISCOVERY_PROGRESS_VERSION,
};
use crate::db::repositories::{self, StrategyDef};
use crate::error::{AppError, AppResult};

use self::execution::{
    execute_candidate, CandidateExecutionOutput, ExecuteCandidateArgs, ExecutionDataset,
};

pub const DISCOVERY_EVENT_VERSION: &str = "discovery-event-v1";
pub const DISCOVERY_PROGRESS_EVENT: &str = "discovery://progress";
pub const DISCOVERY_RESULT_EVENT: &str = "discovery://result";
pub const DISCOVERY_DONE_EVENT: &str = "discovery://done";

type SharedDb = Arc<Mutex<rusqlite::Connection>>;

fn other(message: impl Into<String>) -> AppError {
    AppError::Other(message.into())
}

fn lock<'a, T>(mutex: &'a Mutex<T>, label: &str) -> AppResult<MutexGuard<'a, T>> {
    mutex
        .lock()
        .map_err(|_| other(format!("{label} lock poisoned")))
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProgressCounts {
    pub total_candidates: i64,
    pub queued_candidates: i64,
    pub running_candidates: i64,
    pub completed_candidates: i64,
    pub failed_candidates: i64,
    pub skipped_candidates: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryJobIds {
    pub train: i64,
    pub validation: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryCandidateDigest {
    pub candidate_index: i64,
    pub strategy_id: i64,
    pub dataset_id: i64,
    pub job_ids: DiscoveryJobIds,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProgressEvent {
    pub event_version: &'static str,
    pub sequence: u64,
    pub run_id: i64,
    pub status: RunStatus,
    pub counts: DiscoveryProgressCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate: Option<DiscoveryCandidateDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_strategy_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResultEvent {
    pub event_version: &'static str,
    pub sequence: u64,
    pub run_id: i64,
    pub candidate_index: i64,
    pub job_ids: DiscoveryJobIds,
    pub strategy_id: i64,
    pub strategy_hash: String,
    pub dataset_id: i64,
    pub validation_record_id: i64,
    pub gate_passed: bool,
    pub score: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryDoneEvent {
    pub event_version: &'static str,
    pub sequence: u64,
    pub run_id: i64,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_strategy_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug)]
pub enum DiscoveryEvent {
    Progress(DiscoveryProgressEvent),
    Result(DiscoveryResultEvent),
    Done(DiscoveryDoneEvent),
}

/// Event boundary used by production Tauri and by coordinator tests.
///
/// Emission errors never roll back or fail already-committed work. The
/// database is the source of truth and the frontend can re-query progress.
pub trait DiscoveryEventSink: Send + Sync {
    fn emit(&self, event: &DiscoveryEvent) -> Result<(), String>;
}

#[derive(Clone)]
pub struct TauriDiscoveryEventSink {
    app: AppHandle,
}

impl TauriDiscoveryEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DiscoveryEventSink for TauriDiscoveryEventSink {
    fn emit(&self, event: &DiscoveryEvent) -> Result<(), String> {
        match event {
            DiscoveryEvent::Progress(payload) => self
                .app
                .emit_to("main", DISCOVERY_PROGRESS_EVENT, payload)
                .map_err(|error| error.to_string()),
            DiscoveryEvent::Result(payload) => self
                .app
                .emit_to("main", DISCOVERY_RESULT_EVENT, payload)
                .map_err(|error| error.to_string()),
            DiscoveryEvent::Done(payload) => self
                .app
                .emit_to("main", DISCOVERY_DONE_EVENT, payload)
                .map_err(|error| error.to_string()),
        }
    }
}

fn emit_after_commit(sink: &dyn DiscoveryEventSink, event: DiscoveryEvent) {
    if let Err(error) = sink.emit(&event) {
        eprintln!("discovery event emission failed after commit: {error}");
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryProgressSnapshot {
    pub version: &'static str,
    pub run_id: i64,
    pub name: String,
    pub status: RunStatus,
    pub counts: DiscoveryProgressCounts,
    pub current_candidate_indexes: Vec<i64>,
    pub best_strategy_id: Option<i64>,
    pub error_message: Option<String>,
    pub last_event_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ControlPhase {
    Running,
    PauseRequested,
    CancelRequested,
    Failed,
    Completed,
    Paused,
}

#[derive(Debug)]
struct ControlState {
    phase: ControlPhase,
    last_sequence: u64,
}

impl ControlState {
    fn reserve_sequences(&mut self, count: u64) -> AppResult<u64> {
        let first = self
            .last_sequence
            .checked_add(1)
            .ok_or_else(|| other("discovery event sequence overflow"))?;
        self.last_sequence = self
            .last_sequence
            .checked_add(count)
            .ok_or_else(|| other("discovery event sequence overflow"))?;
        Ok(first)
    }
}

#[derive(Debug)]
struct RunControl {
    state: Mutex<ControlState>,
    changed: Condvar,
}

impl RunControl {
    fn new(last_sequence: u64) -> Self {
        Self {
            state: Mutex::new(ControlState {
                phase: ControlPhase::Running,
                last_sequence,
            }),
            changed: Condvar::new(),
        }
    }
}

#[derive(Clone)]
struct RuntimeDataset {
    id: i64,
    content_hash: String,
    interval: String,
}

#[derive(Clone)]
struct ScheduledCandidate {
    candidate: EnumeratedCandidate,
    strategy_id: i64,
}

struct PreparedRun {
    config: Arc<ResolvedDiscoveryConfig>,
    enumeration: EnumerationCounts,
    tested_combinations: i64,
    dataset: Arc<RuntimeDataset>,
    candles: Arc<Vec<CoreCandle>>,
    candidates: Vec<ScheduledCandidate>,
    total_candidates: i64,
    completed_candidates: i64,
}

#[derive(Clone)]
struct CandidateWork {
    claimed: ClaimedCandidateJobs,
    candidate: EnumeratedCandidate,
    strategy_id: i64,
    config: Arc<ResolvedDiscoveryConfig>,
    dataset: Arc<RuntimeDataset>,
    candles: Arc<Vec<CoreCandle>>,
    tested_combinations: i64,
}

struct WorkerOutcome {
    work: CandidateWork,
    result: Result<CandidateExecutionOutput, String>,
}

trait CandidateExecutor: Send + Sync {
    fn execute(&self, work: &CandidateWork) -> Result<CandidateExecutionOutput, String>;
}

struct ProductionExecutor;

impl CandidateExecutor for ProductionExecutor {
    fn execute(&self, work: &CandidateWork) -> Result<CandidateExecutionOutput, String> {
        execute_candidate(&ExecuteCandidateArgs {
            config: &work.config,
            candidate: &work.candidate,
            tested_combinations: work.tested_combinations,
            strategy_id: work.strategy_id,
            dataset: ExecutionDataset {
                id: work.dataset.id,
                content_hash: &work.dataset.content_hash,
                interval: &work.dataset.interval,
            },
            candles: &work.candles,
        })
        .map_err(|error| error.to_string())
    }
}

/// Cloneable manager handle. Exactly one control exists for each in-process
/// running coordinator. A recovered paused run deliberately has no control
/// until the user explicitly calls resume.
#[derive(Clone)]
pub struct DiscoveryRunner {
    controls: Arc<Mutex<HashMap<i64, Arc<RunControl>>>>,
    executor: Arc<dyn CandidateExecutor>,
}

impl Default for DiscoveryRunner {
    fn default() -> Self {
        Self {
            controls: Arc::new(Mutex::new(HashMap::new())),
            executor: Arc::new(ProductionExecutor),
        }
    }
}

impl DiscoveryRunner {
    pub fn recover_orphans(&self, db: &SharedDb) -> AppResult<RecoveryReport> {
        let mut conn = lock(db, "db")?;
        discovery::recover_orphaned_runs(&mut conn)
    }

    pub fn start(
        &self,
        db: SharedDb,
        sink: Arc<dyn DiscoveryEventSink>,
        raw_config: Value,
    ) -> AppResult<i64> {
        let logical_cores = logical_cores();
        let config = Arc::new(
            parse_discovery_config(&raw_config, logical_cores as f64)
                .map_err(|error| other(error.to_string()))?,
        );
        let plan = enumerate_candidates(&config).map_err(|error| other(error.to_string()))?;
        let raw_config_json = serde_json::to_string(&raw_config)?;

        let (run_id, prepared) = {
            let mut conn = lock(&db, "db")?;
            if let Some(active) = discovery::active_discovery_run(&conn)? {
                return Err(other(format!(
                    "discovery run {} is already {}",
                    active.id,
                    active.status.as_str()
                )));
            }
            let (dataset, candles) = load_verified_dataset(&conn, &config)?;
            let mut scheduled = Vec::with_capacity(plan.candidates.len());
            let mut specs = Vec::with_capacity(plan.candidates.len());
            for candidate in &plan.candidates {
                let definition_json = serde_json::to_string(&candidate.strategy)?;
                let strategy = StrategyDef {
                    id: None,
                    name: format!(
                        "Discovery {} #{:04}",
                        candidate.base_id,
                        candidate.index + 1
                    ),
                    kind: "params".into(),
                    dsl_json: None,
                    original_definition_json: definition_json,
                    param_schema_json: None,
                    source: "sweep".into(),
                    ai_prompt_hash: None,
                    strategy_hash: candidate.strategy_hash.clone(),
                    lifecycle: "candidate".into(),
                    parent_strategy_id: None,
                };
                let strategy_id =
                    repositories::get_or_insert_verified_runner_strategy(&conn, &strategy)?;
                scheduled.push(ScheduledCandidate {
                    candidate: candidate.clone(),
                    strategy_id,
                });
                specs.push(CandidateJobSpec {
                    candidate_index: candidate.index,
                    strategy_id,
                    dataset_id: dataset.id,
                });
            }

            let name = discovery_run_name(dataset.id, &dataset.content_hash)?;
            let run_id = discovery::create_discovery_run(&conn, &name, &raw_config_json)?;
            discovery::start_discovery_run(&mut conn, run_id, &specs)?;
            let initial_sequence = 1;
            let progress =
                stored_progress_json(plan.counts, plan.counts.final_unique, 0, initial_sequence)?;
            if let Err(error) =
                discovery::update_discovery_progress(&conn, run_id, RunStatus::Running, &progress)
            {
                let message = format!("failed to initialize discovery progress: {error}");
                let _ = discovery::fail_discovery_run(&conn, run_id, &message);
                return Err(other(message));
            }

            (
                run_id,
                PreparedRun {
                    config,
                    enumeration: plan.counts,
                    tested_combinations: plan.tested_combinations.n,
                    dataset: Arc::new(RuntimeDataset {
                        id: dataset.id,
                        content_hash: dataset.content_hash,
                        interval: dataset.interval,
                    }),
                    candles: Arc::new(candles),
                    candidates: scheduled,
                    total_candidates: plan.counts.final_unique,
                    completed_candidates: 0,
                },
            )
        };

        let control = Arc::new(RunControl::new(1));
        self.insert_control(run_id, control.clone())?;
        emit_after_commit(
            sink.as_ref(),
            DiscoveryEvent::Progress(DiscoveryProgressEvent {
                event_version: DISCOVERY_EVENT_VERSION,
                sequence: 1,
                run_id,
                status: RunStatus::Running,
                counts: DiscoveryProgressCounts {
                    total_candidates: prepared.total_candidates,
                    queued_candidates: prepared.total_candidates,
                    ..DiscoveryProgressCounts::default()
                },
                candidate: None,
                best_strategy_id: None,
            }),
        );
        if let Err(error) =
            self.spawn_coordinator(db.clone(), sink.clone(), run_id, control.clone(), prepared)
        {
            self.remove_control(run_id, &control);
            let mut state = lock(&control.state, "discovery control")?;
            let message = format!("failed to spawn discovery coordinator: {error}");
            fail_run_after_commit(
                &db,
                sink.as_ref(),
                run_id,
                &mut state,
                &control.changed,
                &message,
            );
            return Err(other(message));
        }
        Ok(run_id)
    }

    pub fn resume(
        &self,
        db: SharedDb,
        sink: Arc<dyn DiscoveryEventSink>,
        run_id: i64,
    ) -> AppResult<()> {
        if let Some(control) = self.control(run_id)? {
            let phase = lock(&control.state, "discovery control")?.phase;
            if phase == ControlPhase::Paused {
                self.remove_control(run_id, &control);
            } else {
                return Err(other(format!(
                    "discovery run {run_id} already has an active coordinator"
                )));
            }
        }
        let logical_cores = logical_cores();
        let (prepared, prior_sequence) = {
            let conn = lock(&db, "db")?;
            let run = discovery::get_discovery_run(&conn, run_id)?;
            if run.status != RunStatus::Paused {
                return Err(other(format!(
                    "illegal run transition {} -> running",
                    run.status.as_str()
                )));
            }
            let raw: Value = serde_json::from_str(&run.config_json)?;
            let config = Arc::new(
                parse_discovery_config(&raw, logical_cores as f64)
                    .map_err(|error| other(error.to_string()))?,
            );
            let plan = enumerate_candidates(&config).map_err(|error| other(error.to_string()))?;
            let (dataset, candles) = load_verified_dataset(&conn, &config)?;
            let jobs = discovery::list_discovery_jobs(&conn, run_id)?;
            let strategies = repositories::list_strategies(&conn)?;
            let (scheduled, completed) = resume_candidates(&plan, &jobs, &strategies, dataset.id)?;
            (
                PreparedRun {
                    config,
                    enumeration: plan.counts,
                    tested_combinations: plan.tested_combinations.n,
                    dataset: Arc::new(RuntimeDataset {
                        id: dataset.id,
                        content_hash: dataset.content_hash,
                        interval: dataset.interval,
                    }),
                    candles: Arc::new(candles),
                    candidates: scheduled,
                    total_candidates: plan.counts.final_unique,
                    completed_candidates: completed,
                },
                last_event_sequence(run.progress_json.as_deref()),
            )
        };

        let resume_sequence = prior_sequence
            .checked_add(1)
            .ok_or_else(|| other("discovery event sequence overflow"))?;
        {
            let conn = lock(&db, "db")?;
            discovery::transition_run(&conn, run_id, RunStatus::Running)?;
            let progress = stored_progress_json(
                prepared.enumeration,
                prepared.total_candidates,
                prepared.completed_candidates,
                resume_sequence,
            )?;
            if let Err(error) =
                discovery::update_discovery_progress(&conn, run_id, RunStatus::Running, &progress)
            {
                let message = format!("failed to checkpoint resumed discovery: {error}");
                let _ = discovery::fail_discovery_run(&conn, run_id, &message);
                return Err(other(message));
            }
        }

        let control = Arc::new(RunControl::new(resume_sequence));
        self.insert_control(run_id, control.clone())?;
        emit_after_commit(
            sink.as_ref(),
            DiscoveryEvent::Progress(DiscoveryProgressEvent {
                event_version: DISCOVERY_EVENT_VERSION,
                sequence: resume_sequence,
                run_id,
                status: RunStatus::Running,
                counts: DiscoveryProgressCounts {
                    total_candidates: prepared.total_candidates,
                    queued_candidates: prepared.total_candidates - prepared.completed_candidates,
                    completed_candidates: prepared.completed_candidates,
                    ..DiscoveryProgressCounts::default()
                },
                candidate: None,
                best_strategy_id: None,
            }),
        );
        if let Err(error) =
            self.spawn_coordinator(db.clone(), sink.clone(), run_id, control.clone(), prepared)
        {
            self.remove_control(run_id, &control);
            let mut state = lock(&control.state, "discovery control")?;
            let message = format!("failed to spawn discovery coordinator: {error}");
            fail_run_after_commit(
                &db,
                sink.as_ref(),
                run_id,
                &mut state,
                &control.changed,
                &message,
            );
            return Err(other(message));
        }
        Ok(())
    }

    pub fn pause(&self, db: &SharedDb, run_id: i64) -> AppResult<()> {
        let control = self
            .control(run_id)?
            .ok_or_else(|| other(format!("discovery run {run_id} has no active coordinator")))?;
        // Control -> DB is the coordinator's universal lock order. Holding the
        // control lock makes "request pause" and "claim next pair" atomic with
        // respect to each other.
        let mut state = lock(&control.state, "discovery control")?;
        {
            let conn = lock(db, "db")?;
            let run = discovery::get_discovery_run(&conn, run_id)?;
            if run.status != RunStatus::Running || state.phase != ControlPhase::Running {
                return Err(other(format!(
                    "cannot pause discovery run {run_id} from {}",
                    run.status.as_str()
                )));
            }
        }
        state.phase = ControlPhase::PauseRequested;
        while state.phase == ControlPhase::PauseRequested {
            state = control
                .changed
                .wait(state)
                .map_err(|_| other("discovery control lock poisoned while pausing"))?;
        }
        match state.phase {
            ControlPhase::Paused | ControlPhase::Completed => Ok(()),
            ControlPhase::Failed => Err(other(format!(
                "discovery run {run_id} failed while draining for pause"
            ))),
            ControlPhase::CancelRequested => Err(other(format!(
                "discovery run {run_id} was cancelled while draining for pause"
            ))),
            ControlPhase::Running | ControlPhase::PauseRequested => Err(other(
                "discovery pause acknowledgement entered an invalid state",
            )),
        }
    }

    pub fn cancel(
        &self,
        db: &SharedDb,
        sink: Arc<dyn DiscoveryEventSink>,
        run_id: i64,
    ) -> AppResult<()> {
        if let Some(control) = self.control(run_id)? {
            let mut state = lock(&control.state, "discovery control")?;
            if !matches!(
                state.phase,
                ControlPhase::Running | ControlPhase::PauseRequested | ControlPhase::Paused
            ) {
                return Err(other(format!(
                    "discovery run {run_id} is not cancellable in its current coordinator state"
                )));
            }
            let sequence = state.reserve_sequences(1)?;
            let run = {
                let conn = lock(db, "db")?;
                discovery::cancel_discovery_run(&conn, run_id)?;
                discovery::get_discovery_run(&conn, run_id)?
            };
            state.phase = ControlPhase::CancelRequested;
            control.changed.notify_all();
            emit_after_commit(
                sink.as_ref(),
                DiscoveryEvent::Done(done_event(sequence, &run)),
            );
            return Ok(());
        }

        // A recovered/drained paused run intentionally has no in-process
        // control. It is still cancellable directly from its persisted state.
        let (run, sequence) = {
            let conn = lock(db, "db")?;
            let before = discovery::get_discovery_run(&conn, run_id)?;
            if before.status != RunStatus::Paused {
                return Err(other(format!(
                    "discovery run {run_id} has no active coordinator and is {}",
                    before.status.as_str()
                )));
            }
            let sequence = last_event_sequence(before.progress_json.as_deref())
                .checked_add(1)
                .ok_or_else(|| other("discovery event sequence overflow"))?;
            discovery::cancel_discovery_run(&conn, run_id)?;
            (discovery::get_discovery_run(&conn, run_id)?, sequence)
        };
        emit_after_commit(
            sink.as_ref(),
            DiscoveryEvent::Done(done_event(sequence, &run)),
        );
        Ok(())
    }

    pub fn progress(&self, db: &SharedDb, run_id: i64) -> AppResult<DiscoveryProgressSnapshot> {
        let conn = lock(db, "db")?;
        let run = discovery::get_discovery_run(&conn, run_id)?;
        let jobs = discovery::list_discovery_jobs(&conn, run_id)?;
        progress_snapshot(&run, &jobs)
    }

    pub fn active_progress(&self, db: &SharedDb) -> AppResult<Option<DiscoveryProgressSnapshot>> {
        let conn = lock(db, "db")?;
        let Some(run) = discovery::active_discovery_run(&conn)? else {
            return Ok(None);
        };
        let jobs = discovery::list_discovery_jobs(&conn, run.id)?;
        progress_snapshot(&run, &jobs).map(Some)
    }

    fn insert_control(&self, run_id: i64, control: Arc<RunControl>) -> AppResult<()> {
        use std::collections::hash_map::Entry;

        let mut controls = lock(&self.controls, "discovery controls")?;
        match controls.entry(run_id) {
            Entry::Vacant(entry) => {
                entry.insert(control);
                Ok(())
            }
            Entry::Occupied(_) => Err(other(format!(
                "discovery run {run_id} already has an active coordinator"
            ))),
        }
    }

    fn control(&self, run_id: i64) -> AppResult<Option<Arc<RunControl>>> {
        Ok(lock(&self.controls, "discovery controls")?
            .get(&run_id)
            .cloned())
    }

    fn remove_control(&self, run_id: i64, expected: &Arc<RunControl>) {
        if let Ok(mut controls) = self.controls.lock() {
            if controls
                .get(&run_id)
                .is_some_and(|current| Arc::ptr_eq(current, expected))
            {
                controls.remove(&run_id);
            }
        }
    }

    fn spawn_coordinator(
        &self,
        db: SharedDb,
        sink: Arc<dyn DiscoveryEventSink>,
        run_id: i64,
        control: Arc<RunControl>,
        prepared: PreparedRun,
    ) -> std::io::Result<()> {
        let runner = self.clone();
        thread::Builder::new()
            .name(format!("discovery-coordinator-{run_id}"))
            .spawn(move || {
                runner.coordinator_loop(db, sink, run_id, control.clone(), prepared);
                runner.remove_control(run_id, &control);
            })?;
        Ok(())
    }

    fn coordinator_loop(
        &self,
        db: SharedDb,
        sink: Arc<dyn DiscoveryEventSink>,
        run_id: i64,
        control: Arc<RunControl>,
        prepared: PreparedRun,
    ) {
        let concurrency = prepared.config.concurrency.resolved.max(1) as usize;
        let (work_tx, result_rx, workers) =
            match spawn_worker_pool(run_id, concurrency, self.executor.clone()) {
                Ok(pool) => pool,
                Err(error) => {
                    if let Ok(mut state) = control.state.lock() {
                        fail_run_after_commit(
                            &db,
                            sink.as_ref(),
                            run_id,
                            &mut state,
                            &control.changed,
                            &format!("failed to spawn discovery worker pool: {error}"),
                        );
                    }
                    return;
                }
            };

        let mut next = 0usize;
        let mut in_flight = 0i64;
        let mut completed = prepared.completed_candidates;
        let total = prepared.total_candidates;
        let mut stop = false;

        while !stop {
            // Claim and dispatch only while the control is exactly Running.
            // Pause/cancel commands acquire the same control lock first, so a
            // request and the next claim have a single, deterministic winner.
            {
                let mut state = match control.state.lock() {
                    Ok(state) => state,
                    Err(_) => break,
                };
                if state.phase == ControlPhase::Running {
                    while in_flight < concurrency as i64 && next < prepared.candidates.len() {
                        if state.phase != ControlPhase::Running {
                            break;
                        }
                        let scheduled = prepared.candidates[next].clone();
                        let claimed = match lock(&db, "db").and_then(|conn| {
                            discovery::claim_candidate_jobs(
                                &conn,
                                run_id,
                                scheduled.candidate.index,
                            )
                        }) {
                            Ok(claimed) => claimed,
                            Err(error) => {
                                fail_run_after_commit(
                                    &db,
                                    sink.as_ref(),
                                    run_id,
                                    &mut state,
                                    &control.changed,
                                    &format!(
                                        "failed to claim candidate {}: {error}",
                                        scheduled.candidate.index
                                    ),
                                );
                                stop = true;
                                break;
                            }
                        };
                        let work = CandidateWork {
                            claimed,
                            candidate: scheduled.candidate,
                            strategy_id: scheduled.strategy_id,
                            config: prepared.config.clone(),
                            dataset: prepared.dataset.clone(),
                            candles: prepared.candles.clone(),
                            tested_combinations: prepared.tested_combinations,
                        };
                        if work_tx.send(work).is_err() {
                            fail_run_after_commit(
                                &db,
                                sink.as_ref(),
                                run_id,
                                &mut state,
                                &control.changed,
                                "discovery worker queue closed before dispatch",
                            );
                            stop = true;
                            break;
                        }
                        next += 1;
                        in_flight += 1;
                    }
                }

                if !stop && in_flight == 0 {
                    if matches!(
                        state.phase,
                        ControlPhase::CancelRequested
                            | ControlPhase::Failed
                            | ControlPhase::Completed
                            | ControlPhase::Paused
                    ) {
                        stop = true;
                    } else if next == prepared.candidates.len() {
                        // Completion wins over a simultaneous pause request:
                        // a paused run with no queued work could never make
                        // progress when resumed.
                        match complete_run_after_commit(
                            &db,
                            sink.as_ref(),
                            run_id,
                            &mut state,
                            &control.changed,
                        ) {
                            Ok(()) => {}
                            Err(error) => fail_run_after_commit(
                                &db,
                                sink.as_ref(),
                                run_id,
                                &mut state,
                                &control.changed,
                                &format!("failed to complete discovery run: {error}"),
                            ),
                        }
                        stop = true;
                    } else if state.phase == ControlPhase::PauseRequested {
                        match pause_run_after_drain(
                            &db,
                            sink.as_ref(),
                            run_id,
                            &mut state,
                            &control.changed,
                            &prepared,
                            completed,
                        ) {
                            Ok(()) => {}
                            Err(error) => fail_run_after_commit(
                                &db,
                                sink.as_ref(),
                                run_id,
                                &mut state,
                                &control.changed,
                                &format!("failed to pause discovery run: {error}"),
                            ),
                        }
                        stop = true;
                    }
                }
            }

            if stop {
                break;
            }

            let outcome = match result_rx.recv() {
                Ok(outcome) => outcome,
                Err(_) => {
                    if let Ok(mut state) = control.state.lock() {
                        if matches!(
                            state.phase,
                            ControlPhase::Running | ControlPhase::PauseRequested
                        ) {
                            fail_run_after_commit(
                                &db,
                                sink.as_ref(),
                                run_id,
                                &mut state,
                                &control.changed,
                                "discovery worker pool closed with candidates in flight",
                            );
                        }
                    }
                    break;
                }
            };
            in_flight = in_flight.saturating_sub(1);

            let mut state = match control.state.lock() {
                Ok(state) => state,
                Err(_) => break,
            };
            if matches!(
                state.phase,
                ControlPhase::CancelRequested
                    | ControlPhase::Failed
                    | ControlPhase::Completed
                    | ControlPhase::Paused
            ) {
                // Late result after cancel/fail: intentionally discard it.
                continue;
            }

            let output = match outcome.result {
                Ok(output) => output,
                Err(error) => {
                    fail_run_after_commit(
                        &db,
                        sink.as_ref(),
                        run_id,
                        &mut state,
                        &control.changed,
                        &format!(
                            "candidate {} execution failed: {error}",
                            outcome.work.candidate.index
                        ),
                    );
                    stop = true;
                    continue;
                }
            };

            let result_sequence = match state.reserve_sequences(2) {
                Ok(sequence) => sequence,
                Err(error) => {
                    fail_run_after_commit(
                        &db,
                        sink.as_ref(),
                        run_id,
                        &mut state,
                        &control.changed,
                        &error.to_string(),
                    );
                    stop = true;
                    continue;
                }
            };
            let progress_sequence = result_sequence + 1;
            let completed_after = completed + 1;
            let progress_json = match stored_progress_json(
                prepared.enumeration,
                total,
                completed_after,
                progress_sequence,
            ) {
                Ok(progress) => progress,
                Err(error) => {
                    fail_run_after_commit(
                        &db,
                        sink.as_ref(),
                        run_id,
                        &mut state,
                        &control.changed,
                        &error.to_string(),
                    );
                    stop = true;
                    continue;
                }
            };

            let record_id = {
                let mut conn = match lock(&db, "db") {
                    Ok(conn) => conn,
                    Err(error) => {
                        fail_run_after_commit(
                            &db,
                            sink.as_ref(),
                            run_id,
                            &mut state,
                            &control.changed,
                            &error.to_string(),
                        );
                        stop = true;
                        continue;
                    }
                };
                let assessment = CandidateAssessment {
                    run_id,
                    candidate_index: outcome.work.candidate.index,
                    train_summary: &output.train_summary,
                    train_trades: &output.train_trades,
                    validation_summary: &output.validation_summary,
                    validation_trades: &output.validation_trades,
                    record: &output.record,
                    progress_json: Some(&progress_json),
                };
                match discovery::commit_candidate_assessment(&mut conn, &assessment) {
                    Ok(record_id) => record_id,
                    Err(error) => {
                        drop(conn);
                        fail_run_after_commit(
                            &db,
                            sink.as_ref(),
                            run_id,
                            &mut state,
                            &control.changed,
                            &format!(
                                "candidate {} commit failed: {error}",
                                outcome.work.candidate.index
                            ),
                        );
                        stop = true;
                        continue;
                    }
                }
            };
            completed = completed_after;

            // The control lock stays held across commit + emission. Therefore
            // cancel either commits before this candidate (and this branch is
            // skipped) or after both success events; it can never interleave
            // between the checkpoint and its digest.
            let best_strategy_id = lock(&db, "db")
                .and_then(|conn| discovery::select_best_strategy(&conn, run_id))
                .unwrap_or(None);
            let candidate_digest = digest(&outcome.work);
            emit_after_commit(
                sink.as_ref(),
                DiscoveryEvent::Result(DiscoveryResultEvent {
                    event_version: DISCOVERY_EVENT_VERSION,
                    sequence: result_sequence,
                    run_id,
                    candidate_index: outcome.work.candidate.index,
                    job_ids: candidate_digest.job_ids,
                    strategy_id: outcome.work.strategy_id,
                    strategy_hash: outcome.work.candidate.strategy_hash.clone(),
                    dataset_id: outcome.work.dataset.id,
                    validation_record_id: record_id,
                    gate_passed: output.digest.gate_passed,
                    score: output.digest.score,
                }),
            );
            emit_after_commit(
                sink.as_ref(),
                DiscoveryEvent::Progress(DiscoveryProgressEvent {
                    event_version: DISCOVERY_EVENT_VERSION,
                    sequence: progress_sequence,
                    run_id,
                    status: RunStatus::Running,
                    counts: DiscoveryProgressCounts {
                        total_candidates: total,
                        queued_candidates: (total - completed - in_flight).max(0),
                        running_candidates: in_flight,
                        completed_candidates: completed,
                        failed_candidates: 0,
                        skipped_candidates: 0,
                    },
                    candidate: Some(candidate_digest),
                    best_strategy_id,
                }),
            );
        }

        drop(work_tx);
        for worker in workers {
            let _ = worker.join();
        }
    }
}

fn logical_cores() -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .max(1)
}

struct VerifiedDataset {
    id: i64,
    interval: String,
    content_hash: String,
}

fn load_verified_dataset(
    conn: &rusqlite::Connection,
    config: &ResolvedDiscoveryConfig,
) -> AppResult<(VerifiedDataset, Vec<CoreCandle>)> {
    let dataset = repositories::get_dataset_by_id(conn, config.dataset.id)?;
    if dataset.dataset_hash != config.dataset.content_hash {
        return Err(other(format!(
            "dataset {} content hash does not match discovery config",
            config.dataset.id
        )));
    }
    let candles = repositories::get_candles(
        conn,
        config.dataset.id,
        dataset.start_time,
        dataset.end_time,
    )?;
    let normalized = crate::identity::verify_dataset_identity(&dataset, &candles)?;
    // DATA-QUALITY-001 mount point 4 — fail closed on data that was stored
    // BEFORE this contract existed. Deliberately placed AFTER the identity
    // check so a tampered payload still reports the identity mismatch first and
    // the two failure classes stay distinguishable. Invalid stored candles are
    // never repaired, dropped, or re-hashed here; the user re-imports.
    market_data::ensure_admissible(normalized.iter().map(repositories::db_candle_fields))
        .map_err(|error| other(error.0))?;
    if normalized.len() as i64 != dataset.candle_count {
        return Err(other(format!(
            "dataset {} candle count changed before discovery start",
            config.dataset.id
        )));
    }
    let core = normalized
        .into_iter()
        .map(|candle| CoreCandle {
            timestamp: candle.timestamp,
            open: candle.open,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            volume: candle.volume,
        })
        .collect();
    Ok((
        VerifiedDataset {
            id: config.dataset.id,
            interval: dataset.interval,
            content_hash: dataset.dataset_hash,
        },
        core,
    ))
}

fn discovery_run_name(dataset_id: i64, content_hash: &str) -> AppResult<String> {
    let digest = content_hash
        .strip_prefix("dataset-content-v2:")
        .ok_or_else(|| other("dataset content hash is not dataset-content-v2"))?;
    let prefix = digest
        .get(..12)
        .ok_or_else(|| other("dataset content hash digest is truncated"))?;
    Ok(format!("discovery-{dataset_id}-{prefix}"))
}

fn resume_candidates(
    plan: &CandidatePlan,
    jobs: &[DiscoveryJobRow],
    strategies: &[StrategyDef],
    dataset_id: i64,
) -> AppResult<(Vec<ScheduledCandidate>, i64)> {
    let by_strategy_id: HashMap<i64, &StrategyDef> = strategies
        .iter()
        .filter_map(|strategy| strategy.id.map(|id| (id, strategy)))
        .collect();
    let mut by_candidate: BTreeMap<i64, Vec<&DiscoveryJobRow>> = BTreeMap::new();
    for job in jobs {
        by_candidate
            .entry(job.candidate_index)
            .or_default()
            .push(job);
    }
    if by_candidate.len() != plan.candidates.len() {
        return Err(other(
            "persisted job count does not match the re-enumerated candidate plan",
        ));
    }

    let mut scheduled = Vec::new();
    let mut completed = 0i64;
    for candidate in &plan.candidates {
        let pair = by_candidate.get(&candidate.index).ok_or_else(|| {
            other(format!(
                "candidate {} has no persisted jobs",
                candidate.index
            ))
        })?;
        if pair.len() != 2
            || !pair.iter().any(|job| job.segment == Segment::Train)
            || !pair.iter().any(|job| job.segment == Segment::Validation)
        {
            return Err(other(format!(
                "candidate {} does not have one Train/Validation pair",
                candidate.index
            )));
        }
        let strategy_id = pair[0].strategy_id;
        if pair
            .iter()
            .any(|job| job.strategy_id != strategy_id || job.dataset_id != dataset_id)
        {
            return Err(other(format!(
                "candidate {} persisted job identity is inconsistent",
                candidate.index
            )));
        }
        let strategy = by_strategy_id.get(&strategy_id).ok_or_else(|| {
            other(format!(
                "candidate {} strategy {} no longer exists",
                candidate.index, strategy_id
            ))
        })?;
        if strategy.strategy_hash != candidate.strategy_hash {
            return Err(other(format!(
                "candidate {} strategy hash changed before resume",
                candidate.index
            )));
        }
        if pair.iter().all(|job| job.status == JobStatus::Done) {
            completed += 1;
        } else if pair.iter().all(|job| job.status == JobStatus::Queued) {
            scheduled.push(ScheduledCandidate {
                candidate: candidate.clone(),
                strategy_id,
            });
        } else {
            return Err(other(format!(
                "candidate {} has a non-resumable persisted job state",
                candidate.index
            )));
        }
    }
    Ok((scheduled, completed))
}

fn stored_progress_json(
    enumeration: EnumerationCounts,
    total_candidates: i64,
    completed_candidates: i64,
    last_event_sequence: u64,
) -> AppResult<String> {
    Ok(serde_json::to_string(&json!({
        "version": DISCOVERY_PROGRESS_VERSION,
        "enumeration": enumeration,
        "totalCandidates": total_candidates,
        "completedCandidates": completed_candidates,
        "lastEventSequence": last_event_sequence,
    }))?)
}

fn last_event_sequence(progress_json: Option<&str>) -> u64 {
    progress_json
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|value| {
            (value.get("version").and_then(Value::as_str) == Some(DISCOVERY_PROGRESS_VERSION))
                .then(|| value.get("lastEventSequence").and_then(Value::as_u64))
                .flatten()
        })
        .unwrap_or(0)
}

fn progress_snapshot(
    run: &DiscoveryRunRow,
    jobs: &[DiscoveryJobRow],
) -> AppResult<DiscoveryProgressSnapshot> {
    let mut groups: BTreeMap<i64, Vec<&DiscoveryJobRow>> = BTreeMap::new();
    for job in jobs {
        groups.entry(job.candidate_index).or_default().push(job);
    }
    let mut counts = DiscoveryProgressCounts {
        total_candidates: groups.len() as i64,
        ..DiscoveryProgressCounts::default()
    };
    let mut current = Vec::new();
    for (candidate_index, pair) in groups {
        if pair.len() != 2
            || !pair.iter().any(|job| job.segment == Segment::Train)
            || !pair.iter().any(|job| job.segment == Segment::Validation)
        {
            return Err(other(format!(
                "candidate {candidate_index} does not have one Train/Validation pair"
            )));
        }
        let status = pair[0].status;
        if pair.iter().any(|job| job.status != status) {
            return Err(other(format!(
                "candidate {candidate_index} has split job states"
            )));
        }
        match status {
            JobStatus::Queued => counts.queued_candidates += 1,
            JobStatus::Running => {
                counts.running_candidates += 1;
                current.push(candidate_index);
            }
            JobStatus::Done => counts.completed_candidates += 1,
            JobStatus::Failed => counts.failed_candidates += 1,
            JobStatus::Skipped => counts.skipped_candidates += 1,
        }
    }
    Ok(DiscoveryProgressSnapshot {
        version: DISCOVERY_PROGRESS_VERSION,
        run_id: run.id,
        name: run.name.clone(),
        status: run.status,
        counts,
        current_candidate_indexes: current,
        best_strategy_id: run.best_strategy_id,
        error_message: run.error_message.clone(),
        last_event_sequence: last_event_sequence(run.progress_json.as_deref()),
    })
}

fn digest(work: &CandidateWork) -> DiscoveryCandidateDigest {
    DiscoveryCandidateDigest {
        candidate_index: work.candidate.index,
        strategy_id: work.strategy_id,
        dataset_id: work.dataset.id,
        job_ids: DiscoveryJobIds {
            train: work.claimed.train_job_id,
            validation: work.claimed.validation_job_id,
        },
    }
}

fn done_event(sequence: u64, run: &DiscoveryRunRow) -> DiscoveryDoneEvent {
    DiscoveryDoneEvent {
        event_version: DISCOVERY_EVENT_VERSION,
        sequence,
        run_id: run.id,
        status: run.status,
        best_strategy_id: run.best_strategy_id,
        error_message: run.error_message.clone(),
    }
}

fn pause_run_after_drain(
    db: &SharedDb,
    sink: &dyn DiscoveryEventSink,
    run_id: i64,
    state: &mut ControlState,
    changed: &Condvar,
    prepared: &PreparedRun,
    completed: i64,
) -> AppResult<()> {
    let sequence = state.reserve_sequences(1)?;
    let progress = stored_progress_json(
        prepared.enumeration,
        prepared.total_candidates,
        completed,
        sequence,
    )?;
    {
        let conn = lock(db, "db")?;
        // Persist the next sequence while the run is still running, then move
        // to paused. A crash between these commits can create a harmless
        // sequence gap, but can never repeat an emitted sequence.
        discovery::update_discovery_progress(&conn, run_id, RunStatus::Running, &progress)?;
        discovery::transition_run(&conn, run_id, RunStatus::Paused)?;
    }
    state.phase = ControlPhase::Paused;
    changed.notify_all();
    emit_after_commit(
        sink,
        DiscoveryEvent::Progress(DiscoveryProgressEvent {
            event_version: DISCOVERY_EVENT_VERSION,
            sequence,
            run_id,
            status: RunStatus::Paused,
            counts: DiscoveryProgressCounts {
                total_candidates: prepared.total_candidates,
                queued_candidates: prepared.total_candidates - completed,
                completed_candidates: completed,
                ..DiscoveryProgressCounts::default()
            },
            candidate: None,
            best_strategy_id: None,
        }),
    );
    Ok(())
}

fn complete_run_after_commit(
    db: &SharedDb,
    sink: &dyn DiscoveryEventSink,
    run_id: i64,
    state: &mut ControlState,
    changed: &Condvar,
) -> AppResult<()> {
    let sequence = state.reserve_sequences(1)?;
    let run = {
        let mut conn = lock(db, "db")?;
        discovery::complete_discovery_run(&mut conn, run_id)?;
        discovery::get_discovery_run(&conn, run_id)?
    };
    state.phase = ControlPhase::Completed;
    changed.notify_all();
    emit_after_commit(sink, DiscoveryEvent::Done(done_event(sequence, &run)));
    Ok(())
}

fn fail_run_after_commit(
    db: &SharedDb,
    sink: &dyn DiscoveryEventSink,
    run_id: i64,
    state: &mut ControlState,
    changed: &Condvar,
    message: &str,
) {
    if !matches!(
        state.phase,
        ControlPhase::Running | ControlPhase::PauseRequested
    ) {
        return;
    }
    let committed = (|| -> AppResult<DiscoveryRunRow> {
        let conn = lock(db, "db")?;
        discovery::fail_discovery_run(&conn, run_id, message)?;
        discovery::get_discovery_run(&conn, run_id)
    })();
    match committed {
        Ok(run) => {
            if let Ok(sequence) = state.reserve_sequences(1) {
                state.phase = ControlPhase::Failed;
                changed.notify_all();
                emit_after_commit(sink, DiscoveryEvent::Done(done_event(sequence, &run)));
            } else {
                state.phase = ControlPhase::Failed;
                changed.notify_all();
            }
        }
        Err(error) => {
            state.phase = ControlPhase::Failed;
            changed.notify_all();
            eprintln!("failed to persist discovery run failure: {error}");
        }
    }
}

type WorkerPool = (
    mpsc::Sender<CandidateWork>,
    mpsc::Receiver<WorkerOutcome>,
    Vec<JoinHandle<()>>,
);

fn spawn_worker_pool(
    run_id: i64,
    concurrency: usize,
    executor: Arc<dyn CandidateExecutor>,
) -> std::io::Result<WorkerPool> {
    let (work_tx, work_rx) = mpsc::channel::<CandidateWork>();
    let work_rx = Arc::new(Mutex::new(work_rx));
    let (result_tx, result_rx) = mpsc::channel::<WorkerOutcome>();
    let mut workers = Vec::with_capacity(concurrency);
    for worker_index in 0..concurrency {
        let work_rx = work_rx.clone();
        let result_tx = result_tx.clone();
        let executor = executor.clone();
        let worker = thread::Builder::new()
            .name(format!("discovery-worker-{run_id}-{worker_index}"))
            .spawn(move || loop {
                let received = match work_rx.lock() {
                    Ok(receiver) => receiver.recv(),
                    Err(_) => return,
                };
                let work = match received {
                    Ok(work) => work,
                    Err(_) => return,
                };
                let result = match catch_unwind(AssertUnwindSafe(|| executor.execute(&work))) {
                    Ok(result) => result,
                    Err(payload) => Err(format!(
                        "worker panic: {}",
                        panic_payload_message(payload.as_ref())
                    )),
                };
                if result_tx.send(WorkerOutcome { work, result }).is_err() {
                    return;
                }
            })?;
        workers.push(worker);
    }
    drop(result_tx);
    Ok((work_tx, result_rx, workers))
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        message
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.as_str()
    } else {
        "non-string panic payload"
    }
}
