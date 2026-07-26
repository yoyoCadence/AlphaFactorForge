//! RUNNER-STORE-001: the discovery run/job store (PR #66 Resolution D5/D6).
//!
//! This module owns run and job STATE and the atomic candidate commit. It
//! deliberately contains no worker pool, no Tauri commands, and no events —
//! those arrive in RUNNER-EXEC-001. Nothing here executes a backtest, and the
//! hidden Test segment has no representation at all.
//!
//! The central invariant is D5's: one candidate assessment commits as ONE
//! SQLite transaction covering the Train/Validation summaries and trades, the
//! append-only validation record, BOTH job rows, run progress, and the
//! strategy lifecycle. A crash between any two of those must leave nothing
//! behind, because `status = 'done'` is the runner's checkpoint and it must
//! mean "the whole assessment exists".

// This module is the store API the next slice consumes. RUNNER-EXEC-001 adds
// the Tauri commands and worker pool that call it, so in a non-test build
// nothing references it yet. REMOVE this allow when EXEC wires the commands —
// at that point genuinely dead code should start failing again.
#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::repositories::{
    insert_validation_record_for_run, validate_validation_bundle, write_backtest_result,
    BacktestSummary, TradeRow, ValidationRecordRow,
};
use crate::error::{AppError, AppResult};

// ---------- state vocabulary ----------

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

/// The fixed D5 transition table. Anything absent here is rejected.
fn transition_allowed(from: RunStatus, to: RunStatus) -> bool {
    use RunStatus::*;
    matches!(
        (from, to),
        (Idle, Running)
            | (Running, Paused)
            | (Running, Completed)
            | (Running, Failed)
            | (Running, Cancelled)
            | (Paused, Running)
            | (Paused, Cancelled)
    )
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

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Done => "done",
            JobStatus::Failed => "failed",
            JobStatus::Skipped => "skipped",
        }
    }
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReport {
    pub runs_paused: usize,
    pub jobs_requeued: usize,
}

// ---------- reads ----------

const RUN_COLS: &str = "id, name, status, config_json, progress_json, best_strategy_id,
     created_at, started_at, completed_at";

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
    })
}

pub fn get_discovery_run(conn: &Connection, run_id: i64) -> AppResult<DiscoveryRunRow> {
    let sql = format!("SELECT {RUN_COLS} FROM discovery_runs WHERE id = ?1");
    conn.query_row(&sql, [run_id], map_run)
        .optional()?
        .ok_or_else(|| AppError::Other(format!("discovery run {run_id} not found")))
}

/// Newest first.
pub fn list_discovery_runs(conn: &Connection) -> AppResult<Vec<DiscoveryRunRow>> {
    let sql = format!("SELECT {RUN_COLS} FROM discovery_runs ORDER BY id DESC");
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map([], map_run)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

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

/// Create an `idle` run. Idle holds no global slot, so drafting a run never
/// blocks another one.
pub fn create_discovery_run(conn: &Connection, name: &str, config_json: &str) -> AppResult<i64> {
    if name.trim().is_empty() {
        return Err(AppError::Other(
            "discovery run name must not be empty".into(),
        ));
    }
    conn.execute(
        "INSERT INTO discovery_runs (name, status, config_json) VALUES (?1, 'idle', ?2)",
        params![name, config_json],
    )?;
    Ok(conn.last_insert_rowid())
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
pub fn start_discovery_run(
    conn: &mut Connection,
    run_id: i64,
    candidates: &[CandidateJobSpec],
) -> AppResult<()> {
    if candidates.is_empty() {
        return Err(AppError::Other(
            "a discovery run must start with at least one candidate".into(),
        ));
    }
    let mut seen: Vec<i64> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if candidate.candidate_index < 0 {
            return Err(AppError::Other(
                "candidate_index must be a non-negative enumeration index".into(),
            ));
        }
        if seen.contains(&candidate.candidate_index) {
            return Err(AppError::Other(format!(
                "duplicate candidate_index {}",
                candidate.candidate_index
            )));
        }
        seen.push(candidate.candidate_index);
    }

    let tx = conn.transaction()?;
    let from = current_status(&tx, run_id)?;
    if !transition_allowed(from, RunStatus::Running) {
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

/// Apply a D5 state transition. Terminal states are refused, and completion
/// stamps `completed_at`.
pub fn transition_run(conn: &Connection, run_id: i64, to: RunStatus) -> AppResult<()> {
    let from = current_status(conn, run_id)?;
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
    conn.execute(&sql, params![run_id, to.as_str()])?;
    Ok(())
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
pub fn complete_discovery_run(conn: &mut Connection, run_id: i64) -> AppResult<Option<i64>> {
    let tx = conn.transaction()?;
    let from = current_status(&tx, run_id)?;
    if !transition_allowed(from, RunStatus::Completed) {
        return Err(AppError::Other(format!(
            "illegal run transition {} -> completed",
            from.as_str()
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
    tx.commit()?;
    Ok(best)
}

/// D5 crash recovery: an orphaned `running` run becomes `paused` and its
/// in-flight jobs return to `queued`. CPU work is never resumed automatically
/// — the user must explicitly resume — and `done` rows are left untouched
/// because they mean a complete atomic assessment already exists.
pub fn recover_orphaned_runs(conn: &mut Connection) -> AppResult<RecoveryReport> {
    let tx = conn.transaction()?;
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
pub fn commit_candidate_assessment(
    conn: &mut Connection,
    assessment: &CandidateAssessment<'_>,
) -> AppResult<i64> {
    validate_validation_bundle(
        assessment.train_summary,
        assessment.validation_summary,
        assessment.record,
    )?;

    // Rusqlite rolls a Transaction back on drop, so every `?` and early
    // `return Err` below undoes the whole assessment. That behaviour is what
    // `a_failure_after_the_writes_rolls_everything_back` pins down.
    let tx = conn.transaction()?;

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
        let found: Option<(i64, i64)> = tx
            .query_row(
                "SELECT strategy_id, dataset_id FROM discovery_jobs
                 WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND segment = ?3",
                params![
                    assessment.run_id,
                    assessment.candidate_index,
                    segment.as_str()
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (strategy_id, dataset_id) = found.ok_or_else(|| {
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
             WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND segment = ?3",
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

    tx.commit()?;
    Ok(record_id)
}

/// Mark a candidate's pair as skipped (cancellation at a candidate boundary).
/// Only untouched rows move: a `done` checkpoint is never rewritten.
pub fn skip_remaining_jobs(conn: &Connection, run_id: i64) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE discovery_jobs
         SET status = 'skipped', updated_at = datetime('now')
         WHERE discovery_run_id = ?1 AND status IN ('queued','running')",
        [run_id],
    )?)
}

/// Record an engine/system failure against a candidate's pair. D5 requires
/// failures to carry evidence rather than be silently retried.
pub fn fail_candidate_jobs(
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
    Ok(conn.execute(
        "UPDATE discovery_jobs
         SET status = 'failed', error_message = ?3, updated_at = datetime('now')
         WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND status != 'done'",
        params![run_id, candidate_index, error_message],
    )?)
}
