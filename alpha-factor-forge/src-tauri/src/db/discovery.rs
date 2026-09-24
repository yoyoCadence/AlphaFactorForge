//! RUNNER-STORE-001: the discovery run/job store (PR #66 Resolution D5/D6).
//!
//! This module owns run and job state and the atomic candidate commit. The
//! RUNNER-EXEC coordinator consumes these operations, while worker-pool,
//! Tauri-command, event, and backtest execution concerns remain outside this
//! persistence module. The hidden Test segment has no representation at all.
//!
//! The central invariant is D5's: one candidate assessment commits as ONE
//! SQLite transaction covering the Train/Validation summaries and trades, the
//! append-only validation record, BOTH job rows, run progress, and the
//! strategy lifecycle. A crash between any two of those must leave nothing
//! behind, because `status = 'done'` is the runner's checkpoint and it must
//! mean "the whole assessment exists".

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::repositories::{
    insert_validation_record_for_run, validate_validation_bundle, write_backtest_result,
    BacktestSummary, TradeRow, ValidationRecordRow,
};
use crate::error::{AppError, AppResult};

// ---------- state vocabulary ----------

pub const DISCOVERY_PROGRESS_VERSION: &str = "discovery-progress-v1";

/// Run states, matching the 0001 CHECK constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Idle => "idle",
            RunStatus::Running => "running",
            RunStatus::Paused => "paused",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<RunStatus> {
        [
            RunStatus::Idle,
            RunStatus::Running,
            RunStatus::Paused,
            RunStatus::Completed,
            RunStatus::Failed,
            RunStatus::Cancelled,
        ]
        .into_iter()
        .find(|status| status.as_str() == value)
    }

    /// Terminal states never resume (D5).
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
        )
    }

    /// A run holding the global non-terminal slot.
    pub fn is_active(self) -> bool {
        matches!(self, RunStatus::Running | RunStatus::Paused)
    }
}

/// Transitions the GENERIC `transition_run` may perform: pause and resume.
///
/// Every other edge of the D5 table is deliberately absent, because reaching
/// it is never just a status write:
///
/// - `idle -> running` must enqueue the candidates — `start_discovery_run`.
/// - `completed` must derive `best_strategy_id` (D6) — `complete_discovery_run`.
/// - `cancelled` must skip the unfinished jobs — `cancel_discovery_run`.
/// - `failed` must persist failure evidence — `fail_discovery_run`.
///
/// Leaving any of them here lets a caller flip the status and then touch the
/// jobs as a SECOND commit — or not at all. `idle -> running` was the sharpest
/// case: a run could be marked running with NO jobs, skipping every candidate
/// check, and then be "completed", because a run with zero jobs trivially has
/// none outstanding.
fn transition_allowed(from: RunStatus, to: RunStatus) -> bool {
    use RunStatus::*;
    matches!((from, to), (Running, Paused) | (Paused, Running))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
    Skipped,
}

/// Discovery evaluates Train and Validation only. Test is not a variant, so a
/// Test job row cannot be constructed — not merely rejected at runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Segment {
    Train,
    Validation,
}

impl Segment {
    pub fn as_str(self) -> &'static str {
        match self {
            Segment::Train => "train",
            Segment::Validation => "validation",
        }
    }
}

// ---------- rows ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryRunRow {
    pub id: i64,
    pub name: String,
    pub status: RunStatus,
    pub config_json: String,
    pub progress_json: Option<String>,
    pub best_strategy_id: Option<i64>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    /// D5 failure evidence, set only by `fail_discovery_run`.
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryJobRow {
    pub id: i64,
    pub discovery_run_id: i64,
    pub candidate_index: i64,
    pub strategy_id: i64,
    pub dataset_id: i64,
    pub segment: Segment,
    pub status: JobStatus,
    pub result_id: Option<i64>,
    pub error_message: Option<String>,
}

/// One enumerated candidate to be queued. Both of its segment rows are created
/// together; the runner never enqueues half a candidate.
#[derive(Clone, Copy, Debug)]
pub struct CandidateJobSpec {
    pub candidate_index: i64,
    pub strategy_id: i64,
    pub dataset_id: i64,
}

/// The paired Train/Validation rows claimed as one scheduling unit.
///
/// There is deliberately no generic segment list here: discovery cannot
/// represent or accidentally schedule a hidden Test job.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimedCandidateJobs {
    pub run_id: i64,
    pub candidate_index: i64,
    pub strategy_id: i64,
    pub dataset_id: i64,
    pub train_job_id: i64,
    pub validation_job_id: i64,
}

/// Everything one finished candidate contributes to the database.
pub struct CandidateAssessment<'a> {
    pub run_id: i64,
    pub candidate_index: i64,
    pub train_summary: &'a BacktestSummary,
    pub train_trades: &'a [TradeRow],
    pub validation_summary: &'a BacktestSummary,
    pub validation_trades: &'a [TradeRow],
    pub record: &'a ValidationRecordRow,
    /// Run-level progress digest written in the same transaction.
    pub progress_json: Option<&'a str>,
    /// P03a: the ownership epoch this assessment was produced under. Checked
    /// INSIDE the commit transaction (contract §1.3), so a worker that is
    /// still reporting after the lease moved on can never write a result.
    /// `None` only for callers with no lease at all (tests, legacy paths).
    pub epoch: Option<i64>,
}

/// P03a: fail with `StaleOwner` unless the stored ownership epoch equals
/// `epoch`; a `None` epoch (no lease) is not checked. This is an advisory
/// preflight only: each store write also checks inside `write_transaction`.
pub fn assert_owner(conn: &Connection, epoch: Option<i64>) -> AppResult<()> {
    match epoch {
        Some(expected) => crate::db::ownership::assert_epoch(conn, expected),
        None => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReport {
    pub runs_paused: usize,
    pub jobs_requeued: usize,
}

// ---------- reads ----------

const RUN_COLS: &str = "id, name, status, config_json, progress_json, best_strategy_id,
     created_at, started_at, completed_at, error_message";

fn map_run(row: &rusqlite::Row) -> rusqlite::Result<DiscoveryRunRow> {
    let raw: String = row.get(2)?;
    Ok(DiscoveryRunRow {
        id: row.get(0)?,
        name: row.get(1)?,
        // The 0001 CHECK constrains this column, so an unknown value means the
        // database was edited outside the app; surface it rather than guess.
        status: RunStatus::parse(&raw).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(AppError::Other(format!("unknown run status {raw}"))),
            )
        })?,
        config_json: row.get(3)?,
        progress_json: row.get(4)?,
        best_strategy_id: row.get(5)?,
        created_at: row.get(6)?,
        started_at: row.get(7)?,
        completed_at: row.get(8)?,
        error_message: row.get(9)?,
    })
}

pub fn get_discovery_run(conn: &Connection, run_id: i64) -> AppResult<DiscoveryRunRow> {
    let sql = format!("SELECT {RUN_COLS} FROM discovery_runs WHERE id = ?1");
    conn.query_row(&sql, [run_id], map_run)
        .optional()?
        .ok_or_else(|| AppError::Other(format!("discovery run {run_id} not found")))
}

/// Newest first.
/// The single non-terminal run, if one exists (D5 allows at most one).
pub fn active_discovery_run(conn: &Connection) -> AppResult<Option<DiscoveryRunRow>> {
    let sql = format!(
        "SELECT {RUN_COLS} FROM discovery_runs
         WHERE status = 'running' OR status = 'paused'"
    );
    Ok(conn.query_row(&sql, [], map_run).optional()?)
}

const JOB_COLS: &str = "id, discovery_run_id, candidate_index, strategy_id, dataset_id,
     segment, status, result_id, error_message";

fn map_job(row: &rusqlite::Row) -> rusqlite::Result<DiscoveryJobRow> {
    let segment: String = row.get(5)?;
    let status: String = row.get(6)?;
    Ok(DiscoveryJobRow {
        id: row.get(0)?,
        discovery_run_id: row.get(1)?,
        candidate_index: row.get(2)?,
        strategy_id: row.get(3)?,
        dataset_id: row.get(4)?,
        segment: match segment.as_str() {
            "train" => Segment::Train,
            "validation" => Segment::Validation,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(AppError::Other(format!("unknown job segment {other}"))),
                ))
            }
        },
        status: match status.as_str() {
            "queued" => JobStatus::Queued,
            "running" => JobStatus::Running,
            "done" => JobStatus::Done,
            "failed" => JobStatus::Failed,
            "skipped" => JobStatus::Skipped,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(AppError::Other(format!("unknown job status {other}"))),
                ))
            }
        },
        result_id: row.get(7)?,
        error_message: row.get(8)?,
    })
}

/// Jobs in candidate order, Train before Validation within a candidate.
pub fn list_discovery_jobs(conn: &Connection, run_id: i64) -> AppResult<Vec<DiscoveryJobRow>> {
    let sql = format!(
        "SELECT {JOB_COLS} FROM discovery_jobs
         WHERE discovery_run_id = ?1
         ORDER BY candidate_index ASC, segment ASC"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map([run_id], map_job)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------- writes ----------

/// Atomically claim one candidate's queued Train/Validation pair.
///
/// The run-status check, pair validation, and both status updates share one
/// transaction. A broken/mismatched/non-queued pair is rejected without
/// moving either row, and a late claim cannot enter a paused or terminal run.
#[cfg(test)]
pub fn claim_candidate_jobs(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    candidate_index: i64,
) -> AppResult<ClaimedCandidateJobs> {
    claim_candidate_jobs_inner(conn, epoch, run_id, candidate_index, false)
}

/// Production claim: the paired jobs and their P05 attempt must move to
/// running together. A missing attempt is an audit-integrity failure, never a
/// legacy success path.
pub fn claim_candidate_jobs_with_attempt(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    candidate_index: i64,
) -> AppResult<ClaimedCandidateJobs> {
    claim_candidate_jobs_inner(conn, epoch, run_id, candidate_index, true)
}

fn claim_candidate_jobs_inner(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    candidate_index: i64,
    require_attempt: bool,
) -> AppResult<ClaimedCandidateJobs> {
    let tx = super::ownership::write_transaction(conn, epoch)?;
    let run_status = current_status(&tx, run_id)?;
    if run_status != RunStatus::Running {
        return Err(AppError::Other(format!(
            "cannot claim a candidate from a {} run",
            run_status.as_str()
        )));
    }

    let sql = format!(
        "SELECT {JOB_COLS} FROM discovery_jobs
         WHERE discovery_run_id = ?1 AND candidate_index = ?2
         ORDER BY segment ASC"
    );
    let mut statement = tx.prepare(&sql)?;
    let jobs = statement
        .query_map(params![run_id, candidate_index], map_job)?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    if jobs.len() != 2 {
        return Err(AppError::Other(format!(
            "candidate {candidate_index} in run {run_id} must have exactly one Train/Validation pair"
        )));
    }

    let mut train = None;
    let mut validation = None;
    for job in jobs {
        match job.segment {
            Segment::Train => train = Some(job),
            Segment::Validation => validation = Some(job),
        }
    }
    let train = train.ok_or_else(|| {
        AppError::Other(format!(
            "candidate {candidate_index} in run {run_id} has no Train job"
        ))
    })?;
    let validation = validation.ok_or_else(|| {
        AppError::Other(format!(
            "candidate {candidate_index} in run {run_id} has no Validation job"
        ))
    })?;
    if train.strategy_id != validation.strategy_id || train.dataset_id != validation.dataset_id {
        return Err(AppError::Other(format!(
            "candidate {candidate_index} in run {run_id} has a mismatched job pair"
        )));
    }
    if train.status != JobStatus::Queued || validation.status != JobStatus::Queued {
        return Err(AppError::Other(format!(
            "candidate {candidate_index} in run {run_id} is not a queued job pair"
        )));
    }

    if require_attempt
        && crate::research::trial_ledger::read_workspace_binding(&tx)
            .map_err(|error| AppError::Other(error.to_string()))?
            .is_some()
    {
        let event: Option<Option<String>> = tx.query_row(
            "SELECT trial_event_id FROM research_attempts
             WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND status = 'submitted'",
            params![run_id, candidate_index],
            |row| row.get(0),
        ).optional()?;
        if event.flatten().is_none() {
            return Err(AppError::Other("trial_not_registered".into()));
        }
    }

    let updated = tx.execute(
        "UPDATE discovery_jobs
         SET status = 'running', error_message = NULL, updated_at = datetime('now')
         WHERE discovery_run_id = ?1 AND candidate_index = ?2
           AND id IN (?3, ?4) AND status = 'queued'",
        params![run_id, candidate_index, train.id, validation.id],
    )?;
    if updated != 2 {
        return Err(AppError::Other(format!(
            "expected exactly two queued jobs for candidate {candidate_index}, updated {updated}"
        )));
    }
    // P05: the attempt follows its jobs. Only the test-only legacy wrapper
    // permits no history; every production claim must move exactly one row.
    let attempts_updated = crate::research::history::mark_candidate_running(&tx, run_id, candidate_index)?;
    if require_attempt && attempts_updated != 1 {
        return Err(AppError::Other(format!(
            "candidate {candidate_index} in run {run_id} has no submitted research attempt"
        )));
    }

    let claimed = ClaimedCandidateJobs {
        run_id,
        candidate_index,
        strategy_id: train.strategy_id,
        dataset_id: train.dataset_id,
        train_job_id: train.id,
        validation_job_id: validation.id,
    };
    tx.commit()?;
    Ok(claimed)
}

// ---------- request outcomes (P03b R1) ----------

/// How far a mutating command got, as recorded in the SAME transaction as the
/// domain change that decides it (`request_outcomes`, migration 0006).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutcomeStage {
    /// State began to change for this request but admission is not finished
    /// (a run row exists, a paused run switched to running). Superseded by
    /// exactly one of the other two; a crash leaves this stage behind.
    Begun,
    /// Admission finished; the caller was told `{ "runId": n }`.
    Accepted,
    /// The command failed after it had begun; the caller was told the message.
    Rejected,
}

impl OutcomeStage {
    pub fn as_str(self) -> &'static str {
        match self {
            OutcomeStage::Begun => "begun",
            OutcomeStage::Accepted => "accepted",
            OutcomeStage::Rejected => "rejected",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "begun" => Some(OutcomeStage::Begun),
            "accepted" => Some(OutcomeStage::Accepted),
            "rejected" => Some(OutcomeStage::Rejected),
            _ => None,
        }
    }
}

/// One request outcome to record inside a store transaction. `outcome` is
/// `None` for `Begun`, the result JSON for `Accepted`, and the error
/// message for `Rejected` — exactly what the caller is (or was) told, so a
/// replay reconstructs the first answer rather than inferring a new one.
#[derive(Clone, Debug)]
pub struct RequestOutcome<'a> {
    pub request_id: &'a str,
    pub command: &'a str,
    pub stage: OutcomeStage,
    pub outcome: Option<serde_json::Value>,
}

impl<'a> RequestOutcome<'a> {
    pub fn begun(request_id: &'a str, command: &'a str) -> Self {
        Self { request_id, command, stage: OutcomeStage::Begun, outcome: None }
    }

    pub fn accepted(request_id: &'a str, command: &'a str, run_id: i64) -> Self {
        Self {
            request_id,
            command,
            stage: OutcomeStage::Accepted,
            outcome: Some(serde_json::json!({ "runId": run_id })),
        }
    }

    pub fn rejected(request_id: &'a str, command: &'a str, message: &str) -> Self {
        Self {
            request_id,
            command,
            stage: OutcomeStage::Rejected,
            outcome: Some(serde_json::json!({ "error": message })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestOutcomeRow {
    pub request_id: String,
    pub run_id: i64,
    pub command: String,
    pub stage: OutcomeStage,
    pub outcome_json: Option<String>,
    pub epoch: i64,
}

/// Record `outcomes` inside `tx`. A new request id inserts; an existing
/// `begun` row may be superseded by `accepted`/`rejected`; an existing
/// `accepted`/`rejected` row is immutable and a conflicting write fails —
/// rolling back the domain change with it, which is the point.
pub(crate) fn record_request_outcomes(
    tx: &Connection,
    epoch: Option<i64>,
    outcomes: &[RequestOutcome<'_>],
    run_id: i64,
) -> AppResult<()> {
    for outcome in outcomes {
        let outcome_json = match &outcome.outcome {
            Some(value) => Some(serde_json::to_string(value)?),
            None => None,
        };
        let changed = tx.execute(
            "INSERT INTO request_outcomes (request_id, run_id, command, stage, outcome_json, epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(request_id) DO UPDATE SET
                 stage = excluded.stage,
                 outcome_json = excluded.outcome_json,
                 updated_at = datetime('now')
             WHERE request_outcomes.stage = 'begun'
               AND request_outcomes.run_id = excluded.run_id",
            params![
                outcome.request_id,
                run_id,
                outcome.command,
                outcome.stage.as_str(),
                outcome_json,
                epoch.unwrap_or(0)
            ],
        )?;
        if changed != 1 {
            return Err(AppError::Other(format!(
                "request {} already has a final outcome; refusing to overwrite it",
                outcome.request_id
            )));
        }
    }
    Ok(())
}

/// Record a rejection that has no domain transaction of its own (the
/// command failed between two of its store writes). Best effort by design:
/// if this cannot be written, the `begun` row still proves the command never
/// completed, which is what a replay reports.
pub fn record_request_rejection(
    conn: &Connection,
    epoch: Option<i64>,
    request_id: &str,
    command: &str,
    run_id: i64,
    message: &str,
) -> AppResult<()> {
    let tx = super::ownership::write_transaction_quiet(conn, epoch)?;
    record_request_outcomes(&tx, epoch, &[RequestOutcome::rejected(request_id, command, message)], run_id)?;
    tx.commit()?;
    Ok(())
}

pub fn read_request_outcome(conn: &Connection, request_id: &str) -> AppResult<Option<RequestOutcomeRow>> {
    let row: Option<(String, i64, String, String, Option<String>, i64)> = conn
        .query_row(
            "SELECT request_id, run_id, command, stage, outcome_json, epoch
             FROM request_outcomes WHERE request_id = ?1",
            params![request_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?;
    match row {
        None => Ok(None),
        Some((request_id, run_id, command, stage, outcome_json, epoch)) => {
            let stage = OutcomeStage::parse(&stage)
                .ok_or_else(|| AppError::Other(format!("unknown request outcome stage {stage:?}")))?;
            Ok(Some(RequestOutcomeRow { request_id, run_id, command, stage, outcome_json, epoch }))
        }
    }
}

/// Create an `idle` run. Idle holds no global slot, so drafting a run never
/// blocks another one. Test convenience: production callers record their
/// request (`create_discovery_run_with_outcomes`).
#[cfg(test)]
pub fn create_discovery_run(
    conn: &Connection,
    epoch: Option<i64>,
    name: &str,
    config_json: &str,
) -> AppResult<i64> {
    create_discovery_run_with_outcomes(conn, epoch, name, config_json, &[])
}

/// `create_discovery_run` that also records request outcomes (normally the
/// creating request's `begun` stage) in the same transaction (P03b R1).
pub fn create_discovery_run_with_outcomes(
    conn: &Connection,
    epoch: Option<i64>,
    name: &str,
    config_json: &str,
    outcomes: &[RequestOutcome<'_>],
) -> AppResult<i64> {
    if name.trim().is_empty() {
        return Err(AppError::Other(
            "discovery run name must not be empty".into(),
        ));
    }
    let tx = super::ownership::write_transaction(conn, epoch)?;
    tx.execute(
        "INSERT INTO discovery_runs (name, status, config_json) VALUES (?1, 'idle', ?2)",
        params![name, config_json],
    )?;
    let id = tx.last_insert_rowid();
    record_request_outcomes(&tx, epoch, outcomes, id)?;
    tx.commit()?;
    Ok(id)
}

fn current_status(conn: &Connection, run_id: i64) -> AppResult<RunStatus> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT status FROM discovery_runs WHERE id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .optional()?;
    let raw = raw.ok_or_else(|| AppError::Other(format!("discovery run {run_id} not found")))?;
    RunStatus::parse(&raw).ok_or_else(|| AppError::Other(format!("unknown run status {raw}")))
}

/// Move a run to `idle -> running` and queue both job rows for every
/// candidate, in ONE transaction. A run either has its complete job set or
/// none of it.
///
/// The global single-active rule is enforced by migration 0003's partial
/// unique index, so a concurrent second start fails at the database rather
/// than relying on a check-then-act race here.
/// Test convenience (no research history): production enqueues through
/// `start_discovery_run_with_lineage`.
#[cfg(test)]
pub fn start_discovery_run(
    conn: &mut Connection,
    epoch: Option<i64>,
    run_id: i64,
    candidates: &[CandidateJobSpec],
) -> AppResult<()> {
    start_discovery_run_bound(conn, epoch, run_id, candidates, None, None)
}

/// Queue jobs, freeze their P05 lineage and P12 event IDs, and advance the
/// registry binding in one owner-checked workspace transaction.
pub fn start_discovery_run_bound(
    conn: &mut Connection,
    epoch: Option<i64>,
    run_id: i64,
    candidates: &[CandidateJobSpec],
    lineage: Option<&crate::research::history::RunLineage>,
    binding: Option<(&[String], &crate::research::trial_ledger::LedgerBinding)>,
) -> AppResult<()> {
    if candidates.is_empty() {
        return Err(AppError::Other(
            "a discovery run must start with at least one candidate".into(),
        ));
    }
    let mut seen_indexes: Vec<i64> = Vec::with_capacity(candidates.len());
    let mut seen_identities: Vec<(i64, i64)> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if candidate.candidate_index < 0 {
            return Err(AppError::Other(
                "candidate_index must be a non-negative enumeration index".into(),
            ));
        }
        if seen_indexes.contains(&candidate.candidate_index) {
            return Err(AppError::Other(format!(
                "duplicate candidate_index {}",
                candidate.candidate_index
            )));
        }
        seen_indexes.push(candidate.candidate_index);

        // Enumeration already deduplicates by `strategy-v2` hash, so two
        // candidates sharing a (strategy, dataset) means the caller built the
        // queue wrong. Rejecting it here fails at enqueue instead of much
        // later, when the second candidate's commit would hit 0003's per-run
        // assessment uniqueness rule after its backtests had already run.
        let identity = (candidate.strategy_id, candidate.dataset_id);
        if seen_identities.contains(&identity) {
            return Err(AppError::Other(format!(
                "candidates {} and {} share strategy {} on dataset {}",
                seen_indexes[seen_identities
                    .iter()
                    .position(|seen| *seen == identity)
                    .expect("identity was recorded")],
                candidate.candidate_index,
                candidate.strategy_id,
                candidate.dataset_id
            )));
        }
        seen_identities.push(identity);
    }

    let tx = super::ownership::write_transaction(conn, epoch)?;
    let from = current_status(&tx, run_id)?;
    if crate::research::trial_ledger::read_workspace_binding(&tx)
        .map_err(|error| AppError::Other(error.to_string()))?
        .is_some() && binding.is_none()
    {
        return Err(AppError::Other("trial_not_registered".into()));
    }
    if binding.is_some() && lineage.is_none() {
        return Err(AppError::Other("trial_not_registered: lineage missing".into()));
    }
    // `idle -> running` lives here, not in the generic table: starting a run
    // is enqueueing its candidates, and the two must not be separable.
    if from != RunStatus::Idle {
        return Err(AppError::Other(format!(
            "illegal run transition {} -> running",
            from.as_str()
        )));
    }

    for candidate in candidates {
        for segment in [Segment::Train, Segment::Validation] {
            tx.execute(
                "INSERT INTO discovery_jobs
                    (discovery_run_id, candidate_index, strategy_id, dataset_id, segment, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'queued')",
                params![
                    run_id,
                    candidate.candidate_index,
                    candidate.strategy_id,
                    candidate.dataset_id,
                    segment.as_str()
                ],
            )?;
        }
    }

    if let Some(lineage) = lineage {
        if lineage.len() != candidates.len() {
            return Err(AppError::Other(format!(
                "run lineage lists {} attempts for {} candidates",
                lineage.len(),
                candidates.len()
            )));
        }
        crate::research::history::register_run_lineage_with_events(
            &tx, lineage, binding.map(|(ids, _)| ids)
        )?;
    }

    if let Some((_, head)) = binding {
        crate::research::trial_ledger::write_workspace_binding(&tx, head)
            .map_err(|error| AppError::Other(error.to_string()))?;
    }

    tx.execute(
        "UPDATE discovery_runs
         SET status = 'running',
             started_at = COALESCE(started_at, datetime('now')),
             updated_at = datetime('now')
         WHERE id = ?1",
        [run_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// Replace a run's versioned progress checkpoint only while it remains in the
/// caller's expected active state.
///
/// The compare-and-update happens in one transaction so a late writer cannot
/// overwrite progress after pause/resume/cancel/fail changed the run state.
pub fn update_discovery_progress(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    expected_status: RunStatus,
    progress_json: &str,
) -> AppResult<()> {
    update_discovery_progress_with_outcomes(conn, epoch, run_id, expected_status, progress_json, &[])
}

/// `update_discovery_progress` that also records request outcomes in the
/// same transaction — the initial checkpoint of `start`/`resume` is where
/// those commands are ACCEPTED (P03b R1).
pub fn update_discovery_progress_with_outcomes(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    expected_status: RunStatus,
    progress_json: &str,
    outcomes: &[RequestOutcome<'_>],
) -> AppResult<()> {
    if !expected_status.is_active() {
        return Err(AppError::Other(
            "progress updates require an expected running or paused status".into(),
        ));
    }
    let progress: serde_json::Value = serde_json::from_str(progress_json)?;
    let version = progress
        .as_object()
        .and_then(|object| object.get("version"))
        .and_then(serde_json::Value::as_str);
    if version != Some(DISCOVERY_PROGRESS_VERSION) {
        return Err(AppError::Other(format!(
            "progress.version must be \"{DISCOVERY_PROGRESS_VERSION}\""
        )));
    }

    let tx = super::ownership::write_transaction(conn, epoch)?;
    let updated = tx.execute(
        "UPDATE discovery_runs
         SET progress_json = ?3, updated_at = datetime('now')
         WHERE id = ?1 AND status = ?2",
        params![run_id, expected_status.as_str(), progress_json],
    )?;
    if updated != 1 {
        let actual = current_status(&tx, run_id)?;
        return Err(AppError::Other(format!(
            "cannot update progress for a {} run while expecting {}",
            actual.as_str(),
            expected_status.as_str()
        )));
    }
    record_request_outcomes(&tx, epoch, outcomes, run_id)?;
    tx.commit()?;
    Ok(())
}

/// Apply a D5 state transition. `completed` is unreachable here (see
/// `transition_allowed`); terminal states stamp `completed_at`.
///
/// The read and the write share one transaction. Without it this would be a
/// check-then-act: two callers could both observe `running` and both write,
/// and the state machine would only be as strong as the caller's discipline.
/// Its siblings `start_discovery_run` and `complete_discovery_run` are already
/// transactional, so leaving this one bare was the odd case out.
/// Test convenience: production callers record their request
/// (`transition_run_with_outcomes`).
#[cfg(test)]
pub fn transition_run(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    to: RunStatus,
) -> AppResult<()> {
    transition_run_with_outcomes(conn, epoch, run_id, to, &[])
}

/// `transition_run` that also records request outcomes in the same
/// transaction (P03b R1): `resume` begins here, a drained `pause` is
/// accepted here.
pub fn transition_run_with_outcomes(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    to: RunStatus,
    outcomes: &[RequestOutcome<'_>],
) -> AppResult<()> {
    let tx = super::ownership::write_transaction(conn, epoch)?;
    let from = current_status(&tx, run_id)?;
    if !transition_allowed(from, to) {
        return Err(AppError::Other(format!(
            "illegal run transition {} -> {}",
            from.as_str(),
            to.as_str()
        )));
    }
    let completed = if to.is_terminal() {
        "datetime('now')"
    } else {
        "completed_at"
    };
    let sql = format!(
        "UPDATE discovery_runs
         SET status = ?2, updated_at = datetime('now'), completed_at = {completed}
         WHERE id = ?1"
    );
    tx.execute(&sql, params![run_id, to.as_str()])?;
    record_request_outcomes(&tx, epoch, outcomes, run_id)?;
    tx.commit()?;
    Ok(())
}

/// Resume a paused run and make its P05 lineage complete in the SAME
/// transaction. Existing P05 attempts are verified; queued candidates from a
/// pre-0007 run receive their first frozen attempt before the run can become
/// claimable.
pub fn resume_discovery_run_bound(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    outcomes: &[RequestOutcome<'_>],
    lineage: &crate::research::history::RunLineage,
    binding: Option<(&std::collections::BTreeMap<String, String>, &crate::research::trial_ledger::LedgerBinding)>,
) -> AppResult<usize> {
    let tx = super::ownership::write_transaction(conn, epoch)?;
    let from = current_status(&tx, run_id)?;
    if crate::research::trial_ledger::read_workspace_binding(&tx)
        .map_err(|error| AppError::Other(error.to_string()))?
        .is_some() && binding.is_none()
    {
        return Err(AppError::Other("trial_not_registered".into()));
    }
    if from != RunStatus::Paused {
        return Err(AppError::Other(format!(
            "illegal run transition {} -> running",
            from.as_str()
        )));
    }
    let inserted = crate::research::history::ensure_resumable_lineage_with_events(
        &tx, lineage, binding.map(|(events, _)| events),
    )?;
    if let Some((_, head)) = binding {
        crate::research::trial_ledger::write_workspace_binding(&tx, head)
            .map_err(|error| AppError::Other(error.to_string()))?;
    }
    tx.execute(
        "UPDATE discovery_runs
         SET status = 'running', updated_at = datetime('now')
         WHERE id = ?1",
        [run_id],
    )?;
    record_request_outcomes(&tx, epoch, outcomes, run_id)?;
    tx.commit()?;
    Ok(inserted)
}

/// D6: the best candidate is the highest FINITE-score gate passer of this run,
/// ties resolved by candidate index then strategy hash. Null when nothing
/// passed. Reads only this run's own assessments.
pub fn select_best_strategy(conn: &Connection, run_id: i64) -> AppResult<Option<i64>> {
    let best: Option<i64> = conn
        .query_row(
            "SELECT r.strategy_id
             FROM validation_records r
             JOIN discovery_jobs j
               ON j.discovery_run_id = r.discovery_run_id
              AND j.strategy_id = r.strategy_id
              AND j.dataset_id = r.dataset_id
              AND j.segment = 'validation'
             JOIN strategy_def s ON s.id = r.strategy_id
             WHERE r.discovery_run_id = ?1
               AND r.gate_passed = 1
               AND r.score IS NOT NULL
               -- FINITE, not merely non-null. SQLite stores NaN as NULL but
               -- keeps +/-Infinity, and an infinite score would outrank every
               -- real candidate. `x * 0 = 0` holds only for finite x
               -- (inf * 0 is NaN, which compares false).
               AND r.score * 0 = 0
             ORDER BY r.score DESC, j.candidate_index ASC, s.strategy_hash ASC
             LIMIT 1",
            [run_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(best)
}

/// Terminate a run, recording its best gate passer. `best_strategy_id` is
/// derived here rather than accepted, so a caller cannot record a winner the
/// stored assessments do not support.
/// Test convenience: production callers record their request
/// (`complete_discovery_run_with_outcomes`).
#[cfg(test)]
pub fn complete_discovery_run(
    conn: &mut Connection,
    epoch: Option<i64>,
    run_id: i64,
) -> AppResult<Option<i64>> {
    complete_discovery_run_with_outcomes(conn, epoch, run_id, &[])
}

/// `complete_discovery_run` that also records request outcomes in the same
/// transaction — a pause that completion wins over is accepted here (P03b R1).
pub fn complete_discovery_run_with_outcomes(
    conn: &mut Connection,
    epoch: Option<i64>,
    run_id: i64,
    outcomes: &[RequestOutcome<'_>],
) -> AppResult<Option<i64>> {
    let tx = super::ownership::write_transaction(conn, epoch)?;
    let from = current_status(&tx, run_id)?;
    if from != RunStatus::Running {
        return Err(AppError::Other(format!(
            "illegal run transition {} -> completed",
            from.as_str()
        )));
    }
    // "Completed" must mean the queue actually drained. Unfinished work would
    // otherwise be frozen behind a terminal state that never resumes, and the
    // derived winner would be computed from a partial set of assessments.
    // `completed` means every candidate was actually assessed — so EVERY job
    // must be `done`, not merely "not queued".
    //
    // Each non-done state is excluded for its own reason. `queued`/`running`
    // would freeze live work behind a state that never resumes. `failed` would
    // erase the difference between "finished, nothing passed" and "the engine
    // broke" (D5 requires such a run to fail WITH evidence). `skipped` belongs
    // to the cancellation flow, so accepting it would let a run that assessed
    // only some of its candidates masquerade as a full run — and its derived
    // winner would be the best of a partial field.
    let mut statement = tx.prepare(
        "SELECT status, COUNT(*) FROM discovery_jobs
         WHERE discovery_run_id = ?1 AND status <> 'done'
         GROUP BY status ORDER BY status",
    )?;
    let outstanding: Vec<(String, i64)> = statement
        .query_map([run_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(statement);
    if !outstanding.is_empty() {
        let detail = outstanding
            .iter()
            .map(|(status, count)| format!("{count} {status}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(AppError::Other(format!(
            "cannot complete run {run_id}: every job must be done, but found {detail}"
        )));
    }
    let best = select_best_strategy(&tx, run_id)?;
    tx.execute(
        "UPDATE discovery_runs
         SET status = 'completed', best_strategy_id = ?2,
             completed_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ?1",
        params![run_id, best],
    )?;
    record_request_outcomes(&tx, epoch, outcomes, run_id)?;
    tx.commit()?;
    Ok(best)
}

/// D5 crash recovery: an orphaned `running` run becomes `paused` and its
/// in-flight jobs return to `queued`. CPU work is never resumed automatically
/// — the user must explicitly resume — and `done` rows are left untouched
/// because they mean a complete atomic assessment already exists.
pub fn recover_orphaned_runs(conn: &mut Connection, epoch: Option<i64>) -> AppResult<RecoveryReport> {
    let tx = super::ownership::write_transaction(conn, epoch)?;
    // P05: interrupted attempts are requeued with their jobs, before the
    // runs they belong to stop being `running`.
    crate::research::history::requeue_running_in_running_runs(&tx)?;
    let jobs_requeued = tx.execute(
        "UPDATE discovery_jobs
         SET status = 'queued', updated_at = datetime('now')
         WHERE status = 'running'
           AND discovery_run_id IN (SELECT id FROM discovery_runs WHERE status = 'running')",
        [],
    )?;
    let runs_paused = tx.execute(
        "UPDATE discovery_runs
         SET status = 'paused', updated_at = datetime('now')
         WHERE status = 'running'",
        [],
    )?;
    tx.commit()?;
    Ok(RecoveryReport {
        runs_paused,
        jobs_requeued,
    })
}

/// Commit ONE finished candidate assessment atomically (D5).
///
/// Train/Validation summaries and trades, the append-only validation record,
/// BOTH job rows, run progress, and the strategy lifecycle all land in a
/// single transaction. Any failure rolls the whole thing back, so a `done`
/// job always implies a complete assessment.
///
/// Returns the new validation record id.
/// Test convenience (no artifact): production commits through
/// `commit_candidate_assessment_with_artifact`.
#[cfg(test)]
pub fn commit_candidate_assessment(
    conn: &mut Connection,
    assessment: &CandidateAssessment<'_>,
) -> AppResult<i64> {
    commit_candidate_assessment_inner(conn, assessment, None, false)
}

/// `commit_candidate_assessment` plus the P05 history: the artifact
/// reference (the file already exists and was verified) and the attempt's
/// completion land in the same transaction as the projection rows, so the
/// projection and the immutable record can never disagree about which
/// attempt produced them.
pub fn commit_candidate_assessment_with_artifact(
    conn: &mut Connection,
    assessment: &CandidateAssessment<'_>,
    artifact: Option<&crate::research::artifacts::ArtifactRef>,
) -> AppResult<i64> {
    commit_candidate_assessment_inner(conn, assessment, artifact, true)
}

fn commit_candidate_assessment_inner(
    conn: &mut Connection,
    assessment: &CandidateAssessment<'_>,
    artifact: Option<&crate::research::artifacts::ArtifactRef>,
    require_attempt: bool,
) -> AppResult<i64> {
    validate_validation_bundle(
        assessment.train_summary,
        assessment.validation_summary,
        assessment.record,
    )?;

    // Rusqlite rolls a Transaction back on drop, so every `?` and early
    // `return Err` below undoes the whole assessment. That behaviour is what
    // `a_failure_after_the_writes_rolls_everything_back` pins down.
    // The lease first: a result produced under an epoch the workspace has
    // since left belongs to a host that no longer owns it (contract §1.3).
    let tx = super::ownership::write_transaction(conn, assessment.epoch)?;

    // A run must be actively running to absorb a result. Committing into a
    // paused/terminal run would resurrect work the user stopped.
    let status = current_status(&tx, assessment.run_id)?;
    if status != RunStatus::Running {
        return Err(AppError::Other(format!(
            "cannot commit a candidate into a {} run",
            status.as_str()
        )));
    }

    // The queued pair must exist and must agree with the record's identity,
    // otherwise this result belongs to a different candidate.
    for (segment, expected) in [
        (Segment::Train, assessment.train_summary),
        (Segment::Validation, assessment.validation_summary),
    ] {
        let found: Option<(i64, i64, String)> = tx
            .query_row(
                "SELECT strategy_id, dataset_id, status FROM discovery_jobs
                 WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND segment = ?3",
                params![
                    assessment.run_id,
                    assessment.candidate_index,
                    segment.as_str()
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (strategy_id, dataset_id, status) = found.ok_or_else(|| {
            AppError::Other(format!(
                "no {} job for candidate {} in run {}",
                segment.as_str(),
                assessment.candidate_index,
                assessment.run_id
            ))
        })?;
        if strategy_id != expected.strategy_id || dataset_id != expected.dataset_id {
            return Err(AppError::Other(format!(
                "{} job identity does not match the committed summary",
                segment.as_str()
            )));
        }
        // Only unfinished work may be completed. Without this a result could
        // flip an already `failed` or `skipped` row to `done` and wipe its
        // error_message, destroying the evidence D5 requires a failure to
        // carry — and re-completing a `done` row would rewrite a checkpoint.
        if status != "queued" && status != "running" {
            return Err(AppError::Other(format!(
                "{} job for candidate {} is already {status}",
                segment.as_str(),
                assessment.candidate_index
            )));
        }
    }

    let train_id = write_backtest_result(&tx, assessment.train_summary, assessment.train_trades)?;
    let validation_id = write_backtest_result(
        &tx,
        assessment.validation_summary,
        assessment.validation_trades,
    )?;
    let record_id =
        insert_validation_record_for_run(&tx, assessment.record, Some(assessment.run_id))?;

    for (segment, result_id) in [
        (Segment::Train, train_id),
        (Segment::Validation, validation_id),
    ] {
        let updated = tx.execute(
            "UPDATE discovery_jobs
             SET status = 'done', result_id = ?4, error_message = NULL,
                 updated_at = datetime('now')
             WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND segment = ?3
               AND status IN ('queued','running')",
            params![
                assessment.run_id,
                assessment.candidate_index,
                segment.as_str(),
                result_id
            ],
        )?;
        if updated != 1 {
            return Err(AppError::Other(format!(
                "expected exactly one {} job row to complete, updated {updated}",
                segment.as_str()
            )));
        }
    }

    // D6 lifecycle, derived from the record itself so a caller cannot record a
    // verdict that contradicts the stored evidence. A validated strategy is
    // never demoted by a later failure.
    if assessment.record.gate_passed {
        tx.execute(
            "UPDATE strategy_def SET lifecycle = 'validated', updated_at = datetime('now')
             WHERE id = ?1 AND lifecycle IN ('candidate','rejected')",
            [assessment.record.strategy_id],
        )?;
    } else {
        tx.execute(
            "UPDATE strategy_def SET lifecycle = 'rejected', updated_at = datetime('now')
             WHERE id = ?1 AND lifecycle = 'candidate'",
            [assessment.record.strategy_id],
        )?;
    }

    if let Some(progress) = assessment.progress_json {
        tx.execute(
            "UPDATE discovery_runs SET progress_json = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![assessment.run_id, progress],
        )?;
    }

    // P05: the immutable record of THIS attempt. A history row exists for
    // every production candidate; the legacy/test path has none (0 rows).
    let artifact_id = match artifact {
        Some(reference) => Some(crate::research::history::insert_artifact(&tx, reference)?),
        None => None,
    };
    let outcome = serde_json::json!({
        "recordId": record_id,
        "trainSummaryId": train_id,
        "validationSummaryId": validation_id,
        "gatePassed": assessment.record.gate_passed,
        "score": assessment.record.score,
    });
    let attempts_updated = crate::research::history::complete_candidate(
        &tx,
        assessment.run_id,
        assessment.candidate_index,
        artifact_id,
        &outcome,
    )?;
    if require_attempt && attempts_updated != 1 {
        return Err(AppError::Other(format!(
            "candidate {} in run {} has no unfinished research attempt",
            assessment.candidate_index, assessment.run_id
        )));
    }

    tx.commit()?;
    Ok(record_id)
}

/// Mark every unfinished job as skipped. Only untouched rows move: a `done`
/// checkpoint is never rewritten.
///
/// Not public: skipping is meaningful only as part of cancelling a run, and
/// exposing it separately is what allowed a status write and a job write to
/// land as two commits. Use `cancel_discovery_run`.
fn skip_unfinished_jobs(conn: &Connection, run_id: i64) -> AppResult<usize> {
    crate::research::history::skip_unfinished(conn, run_id, "run cancelled before this candidate ran")?;
    Ok(conn.execute(
        "UPDATE discovery_jobs
         SET status = 'skipped', updated_at = datetime('now')
         WHERE discovery_run_id = ?1 AND status IN ('queued','running')",
        [run_id],
    )?)
}

/// Cancel a run: status and the fate of its unfinished jobs commit TOGETHER.
///
/// D5 makes cancel cooperative at candidate boundaries, so an in-flight
/// candidate that already committed keeps its `done` checkpoint; everything
/// still queued or running becomes `skipped`. Because crash recovery
/// deliberately ignores terminal runs, a cancelled run left holding queued
/// jobs would never be repaired — hence one transaction.
/// Test convenience: production callers record their request
/// (`cancel_discovery_run_with_outcomes`).
#[cfg(test)]
pub fn cancel_discovery_run(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
) -> AppResult<usize> {
    cancel_discovery_run_with_outcomes(conn, epoch, run_id, &[])
}

/// `cancel_discovery_run` that also records request outcomes in the same
/// transaction: the cancel's own acceptance, and the rejection of a pause
/// that was still draining (P03b R1).
pub fn cancel_discovery_run_with_outcomes(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    outcomes: &[RequestOutcome<'_>],
) -> AppResult<usize> {
    let tx = super::ownership::write_transaction(conn, epoch)?;
    let from = current_status(&tx, run_id)?;
    if !from.is_active() {
        return Err(AppError::Other(format!(
            "illegal run transition {} -> cancelled",
            from.as_str()
        )));
    }
    let skipped = skip_unfinished_jobs(&tx, run_id)?;
    tx.execute(
        "UPDATE discovery_runs
         SET status = 'cancelled', updated_at = datetime('now'),
             completed_at = datetime('now')
         WHERE id = ?1",
        [run_id],
    )?;
    record_request_outcomes(&tx, epoch, outcomes, run_id)?;
    tx.commit()?;
    Ok(skipped)
}

/// Fail a run: status and the failure evidence on its unfinished jobs commit
/// TOGETHER.
///
/// D5 requires an engine/system failure to fail the run WITH evidence. A
/// separate status write could crash before the evidence landed, leaving a
/// terminal run that records no reason and that recovery will never revisit.
/// Test convenience: production callers record their request
/// (`fail_discovery_run_with_outcomes`).
#[cfg(test)]
pub fn fail_discovery_run(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    error_message: &str,
) -> AppResult<usize> {
    fail_discovery_run_with_outcomes(conn, epoch, run_id, error_message, &[])
}

/// `fail_discovery_run` that also records request outcomes in the same
/// transaction — the rejection of the `start`/`resume`/`pause` request
/// that this failure answers (P03b R1).
pub fn fail_discovery_run_with_outcomes(
    conn: &Connection,
    epoch: Option<i64>,
    run_id: i64,
    error_message: &str,
    outcomes: &[RequestOutcome<'_>],
) -> AppResult<usize> {
    if error_message.trim().is_empty() {
        return Err(AppError::Other(
            "a failed run must record why it failed".into(),
        ));
    }
    let tx = super::ownership::write_transaction(conn, epoch)?;
    let from = current_status(&tx, run_id)?;
    // Only a RUNNING run may fail, matching D5's table. `paused -> failed` is
    // not an edge there: a paused run resumes or cancels.
    if from != RunStatus::Running {
        return Err(AppError::Other(format!(
            "illegal run transition {} -> failed",
            from.as_str()
        )));
    }
    // Unfinished work inherits the reason as per-job detail...
    let failed = tx.execute(
        "UPDATE discovery_jobs
         SET status = 'failed', error_message = ?2, updated_at = datetime('now')
         WHERE discovery_run_id = ?1 AND status IN ('queued','running')",
        params![run_id, error_message],
    )?;
    // ...and so do their attempts (P05: a failure is traceable per attempt).
    crate::research::history::fail_unfinished(&tx, run_id, error_message)?;
    // ...but the RUN keeps its own copy regardless. Relying on the job rows
    // alone silently dropped the reason whenever every job was already `done`,
    // leaving a terminal run that records no evidence at all.
    tx.execute(
        "UPDATE discovery_runs
         SET status = 'failed', error_message = ?2, updated_at = datetime('now'),
             completed_at = datetime('now')
         WHERE id = ?1",
        params![run_id, error_message],
    )?;
    record_request_outcomes(&tx, epoch, outcomes, run_id)?;
    tx.commit()?;
    Ok(failed)
}

/// Record an engine/system failure against ONE candidate's pair.
///
/// Visible only inside `db`: on its own it produces a `running` run holding
/// `failed` jobs — exactly the split state the transactional run APIs exist to
/// prevent. D5 treats an engine failure as a RUN failure, so callers outside
/// this module (the RUNNER-EXEC commands) reach it only through
/// `fail_discovery_run`, which stamps the reason on the run AND its unfinished
/// jobs in one commit.
#[cfg(test)]
pub(super) fn fail_candidate_jobs(
    conn: &Connection,
    run_id: i64,
    candidate_index: i64,
    error_message: &str,
) -> AppResult<usize> {
    if error_message.trim().is_empty() {
        return Err(AppError::Other(
            "a failed job must record why it failed".into(),
        ));
    }
    // Only unfinished work can fail. `done`, `skipped`, and an earlier
    // `failed` are all terminal: overwriting them would rewrite a checkpoint
    // or replace the original failure evidence with a later one.
    crate::research::history::fail_candidate(conn, run_id, candidate_index, error_message)?;
    Ok(conn.execute(
        "UPDATE discovery_jobs
         SET status = 'failed', error_message = ?3, updated_at = datetime('now')
         WHERE discovery_run_id = ?1 AND candidate_index = ?2
           AND status IN ('queued','running')",
        params![run_id, candidate_index, error_message],
    )?)
}
