//! P05 — hypotheses, research attempts, and artifact references in SQLite
//! (migration 0007). Every writer here is called INSIDE the runner's own
//! transaction for the job transition it mirrors (queue, claim, commit,
//! fail, skip, requeue), so an attempt's status can never disagree with the
//! job rows it describes. The tables' triggers refuse edits to frozen
//! columns, changes after a terminal status, and every delete.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::artifacts::ArtifactRef;
use super::{canonical_json, sha256_hex, HYPOTHESIS_VERSION};
use crate::error::{AppError, AppResult};

/// `research_attempts.status`. Forward-only: `submitted` → `running` →
/// one of the terminal three; `running` → `submitted` only when the host
/// requeues interrupted work (crash recovery), which is not a new attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttemptStatus {
    Submitted,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl AttemptStatus {
    fn parse(text: &str) -> AppResult<Self> {
        Ok(match text {
            "submitted" => AttemptStatus::Submitted,
            "running" => AttemptStatus::Running,
            "completed" => AttemptStatus::Completed,
            "failed" => AttemptStatus::Failed,
            "skipped" => AttemptStatus::Skipped,
            other => return Err(AppError::Other(format!("unknown attempt status {other:?}"))),
        })
    }
}

// ---------- hypotheses ----------

/// What is frozen before execution (plan §3.3 "Hypothesis": mechanism,
/// applicability, failure modes, parent strategy, variation, DSL/strategy
/// hash). Its content hash is its identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HypothesisDraft {
    /// `manual` | `discovery` | `ai`.
    pub source: String,
    /// Why this might work.
    pub mechanism: String,
    /// When it applies (market, interval, split, costs ...), as data.
    pub applicability: Value,
    /// When it is expected to stop working.
    pub failure_modes: String,
    /// The frozen strategy identity (`strategy-v2:…`).
    pub strategy_hash: String,
    pub strategy_id: Option<i64>,
    pub parent_strategy_id: Option<i64>,
    /// e.g. `param-sweep:fastMA`, `ai-proposal`, `manual`.
    pub variation_kind: Option<String>,
}

impl HypothesisDraft {
    /// The document as stored: the draft plus its version.
    pub fn content(&self) -> AppResult<Value> {
        let mut content = serde_json::to_value(self)?;
        content["version"] = json!(HYPOTHESIS_VERSION);
        Ok(content)
    }

    /// The identity: SHA-256 over the canonical content.
    pub fn hash(&self) -> AppResult<String> {
        Ok(sha256_hex(&canonical_json(&self.content()?)?))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HypothesisRow {
    pub id: i64,
    pub hypothesis_hash: String,
    pub version: String,
    pub source: String,
    pub mechanism: String,
    pub applicability: Value,
    pub failure_modes: String,
    pub strategy_hash: String,
    pub strategy_id: Option<i64>,
    pub parent_strategy_id: Option<i64>,
    pub variation_kind: Option<String>,
    pub created_at: String,
}

const HYPOTHESIS_COLUMNS: &str = "id, hypothesis_hash, version, source, mechanism, applicability_json, failure_modes,
    strategy_hash, strategy_id, parent_strategy_id, variation_kind, created_at";

fn hypothesis_from_row(row: &Row<'_>) -> rusqlite::Result<HypothesisRow> {
    let applicability: String = row.get(5)?;
    Ok(HypothesisRow {
        id: row.get(0)?,
        hypothesis_hash: row.get(1)?,
        version: row.get(2)?,
        source: row.get(3)?,
        mechanism: row.get(4)?,
        applicability: serde_json::from_str(&applicability).unwrap_or(Value::Null),
        failure_modes: row.get(6)?,
        strategy_hash: row.get(7)?,
        strategy_id: row.get(8)?,
        parent_strategy_id: row.get(9)?,
        variation_kind: row.get(10)?,
        created_at: row.get(11)?,
    })
}

/// Register a hypothesis; the same content registers as the same row.
/// Returns the id and whether this call created it.
pub fn register_hypothesis(conn: &Connection, draft: &HypothesisDraft) -> AppResult<(i64, bool)> {
    if !["manual", "discovery", "ai"].contains(&draft.source.as_str()) {
        return Err(AppError::Other(format!("hypothesis source {:?} is not manual/discovery/ai", draft.source)));
    }
    if draft.mechanism.trim().is_empty() || draft.failure_modes.trim().is_empty() {
        return Err(AppError::Other("a hypothesis states its mechanism and its failure modes".into()));
    }
    let hash = draft.hash()?;
    if let Some(id) = conn
        .query_row("SELECT id FROM hypotheses WHERE hypothesis_hash = ?1", [&hash], |r| r.get::<_, i64>(0))
        .optional()?
    {
        return Ok((id, false));
    }
    let content = draft.content()?;
    conn.execute(
        "INSERT INTO hypotheses (hypothesis_hash, version, source, mechanism, applicability_json, failure_modes,
             strategy_hash, strategy_id, parent_strategy_id, variation_kind, content_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            hash,
            HYPOTHESIS_VERSION,
            draft.source,
            draft.mechanism,
            serde_json::to_string(&draft.applicability)?,
            draft.failure_modes,
            draft.strategy_hash,
            draft.strategy_id,
            draft.parent_strategy_id,
            draft.variation_kind,
            serde_json::to_string(&content)?,
        ],
    )?;
    Ok((conn.last_insert_rowid(), true))
}

pub fn get_hypothesis(conn: &Connection, id: i64) -> AppResult<Option<HypothesisRow>> {
    Ok(conn
        .query_row(&format!("SELECT {HYPOTHESIS_COLUMNS} FROM hypotheses WHERE id = ?1"), [id], hypothesis_from_row)
        .optional()?)
}

pub fn list_hypotheses(conn: &Connection, limit: usize) -> AppResult<Vec<HypothesisRow>> {
    let mut stmt = conn.prepare(&format!("SELECT {HYPOTHESIS_COLUMNS} FROM hypotheses ORDER BY id DESC LIMIT ?1"))?;
    let rows = stmt.query_map([limit as i64], hypothesis_from_row)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ---------- attempts ----------

/// The idempotency key of a discovery candidate's attempt.
pub fn candidate_attempt_key(run_id: i64, candidate_index: i64) -> String {
    format!("run:{run_id}:candidate:{candidate_index}")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptDraft {
    pub attempt_key: String,
    pub hypothesis_id: i64,
    pub strategy_id: i64,
    pub dataset_id: i64,
    pub discovery_run_id: Option<i64>,
    pub candidate_index: Option<i64>,
    /// Dataset hash, strategy hash, config hash, split/embargo/seed ...
    pub input_fingerprint: Value,
    /// Package version and every contract version the engine ran under.
    pub engine_fingerprint: Value,
    pub epoch: Option<i64>,
}

/// What a run registers when it enqueues: for every candidate, the
/// hypothesis it tests (deduplicated by content, so a run whose candidates
/// vary one base strategy registers one hypothesis) and its attempt
/// (`hypothesis_id` is filled in here).
pub type RunLineage = Vec<(HypothesisDraft, AttemptDraft)>;

/// Register every hypothesis (deduplicated) and attempt, in the caller's
/// transaction. Returns `(hypothesis id, attempt id)` per candidate, in order.
pub fn register_run_lineage(conn: &Connection, lineage: &RunLineage) -> AppResult<Vec<(i64, i64)>> {
    let mut ids = Vec::with_capacity(lineage.len());
    for (hypothesis, attempt) in lineage {
        let (hypothesis_id, _) = register_hypothesis(conn, hypothesis)?;
        let draft = AttemptDraft { hypothesis_id, ..attempt.clone() };
        ids.push((hypothesis_id, register_attempt(conn, &draft)?));
    }
    Ok(ids)
}

/// Queue one attempt (`submitted`). A repeated key is an error: the caller
/// that wants "the same attempt" looks it up instead.
pub fn register_attempt(conn: &Connection, draft: &AttemptDraft) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO research_attempts (attempt_key, hypothesis_id, strategy_id, dataset_id, discovery_run_id,
             candidate_index, status, input_fingerprint_json, engine_fingerprint_json, epoch)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'submitted', ?7, ?8, ?9)",
        params![
            draft.attempt_key,
            draft.hypothesis_id,
            draft.strategy_id,
            draft.dataset_id,
            draft.discovery_run_id,
            draft.candidate_index,
            serde_json::to_string(&draft.input_fingerprint)?,
            serde_json::to_string(&draft.engine_fingerprint)?,
            draft.epoch,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// `submitted` → `running` for a claimed candidate. Returns rows moved
/// (0 when the run has no history rows, e.g. legacy test paths).
pub fn mark_candidate_running(conn: &Connection, run_id: i64, candidate_index: i64) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE research_attempts SET status = 'running'
         WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND status = 'submitted'",
        params![run_id, candidate_index],
    )?)
}

/// Terminal `completed`, with the immutable artifact (when the host keeps
/// one) and the outcome digest.
pub fn complete_candidate(
    conn: &Connection,
    run_id: i64,
    candidate_index: i64,
    artifact_id: Option<i64>,
    outcome: &Value,
) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE research_attempts
         SET status = 'completed', result_artifact_id = ?3, outcome_json = ?4, finished_at = datetime('now')
         WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND status IN ('submitted','running')",
        params![run_id, candidate_index, artifact_id, serde_json::to_string(outcome)?],
    )?)
}

/// Terminal `failed` for one candidate, keeping why (the first reason: a
/// terminal row never changes again). Production fails the RUN with its
/// unfinished attempts (`fail_unfinished`); this mirrors the test-only
/// per-candidate job failure.
#[cfg(test)]
pub fn fail_candidate(conn: &Connection, run_id: i64, candidate_index: i64, message: &str) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE research_attempts
         SET status = 'failed', outcome_json = ?3, finished_at = datetime('now')
         WHERE discovery_run_id = ?1 AND candidate_index = ?2 AND status IN ('submitted','running')",
        params![run_id, candidate_index, serde_json::to_string(&json!({ "error": message }))?],
    )?)
}

/// Terminal `failed` for everything of a run still unfinished (the run
/// failed as a whole; each attempt keeps the run's reason).
pub fn fail_unfinished(conn: &Connection, run_id: i64, message: &str) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE research_attempts
         SET status = 'failed', outcome_json = ?2, finished_at = datetime('now')
         WHERE discovery_run_id = ?1 AND status IN ('submitted','running')",
        params![run_id, serde_json::to_string(&json!({ "error": message }))?],
    )?)
}

/// Crash recovery: every `running` attempt of a run that was running when
/// the previous owner died is requeued with its job (same attempt).
pub fn requeue_running_in_running_runs(conn: &Connection) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE research_attempts SET status = 'submitted'
         WHERE status = 'running'
           AND discovery_run_id IN (SELECT id FROM discovery_runs WHERE status = 'running')",
        [],
    )?)
}

/// Terminal `skipped` for everything of a run still unfinished (cancel).
pub fn skip_unfinished(conn: &Connection, run_id: i64, reason: &str) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE research_attempts
         SET status = 'skipped', outcome_json = ?2, finished_at = datetime('now')
         WHERE discovery_run_id = ?1 AND status IN ('submitted','running')",
        params![run_id, serde_json::to_string(&json!({ "reason": reason }))?],
    )?)
}

/// Record an artifact reference (the file already exists and was verified).
/// The same content is the same row.
pub fn insert_artifact(conn: &Connection, reference: &ArtifactRef) -> AppResult<i64> {
    if let Some(id) = conn
        .query_row("SELECT id FROM research_artifacts WHERE sha256 = ?1", [&reference.sha256], |r| r.get::<_, i64>(0))
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO research_artifacts (sha256, kind, byte_len, relative_path) VALUES (?1, ?2, ?3, ?4)",
        params![reference.sha256, reference.kind, reference.byte_len, reference.relative_path],
    )?;
    Ok(conn.last_insert_rowid())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredArtifact {
    pub id: i64,
    #[serde(flatten)]
    pub reference: ArtifactRef,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRow {
    pub id: i64,
    pub attempt_key: String,
    pub hypothesis_id: i64,
    pub strategy_id: i64,
    pub dataset_id: i64,
    pub discovery_run_id: Option<i64>,
    pub candidate_index: Option<i64>,
    pub status: AttemptStatus,
    pub input_fingerprint: Value,
    pub engine_fingerprint: Value,
    pub outcome: Option<Value>,
    pub result_artifact: Option<StoredArtifact>,
    pub epoch: Option<i64>,
    pub submitted_at: String,
    pub finished_at: Option<String>,
}

const ATTEMPT_COLUMNS: &str = "a.id, a.attempt_key, a.hypothesis_id, a.strategy_id, a.dataset_id, a.discovery_run_id,
    a.candidate_index, a.status, a.input_fingerprint_json, a.engine_fingerprint_json, a.outcome_json, a.epoch,
    a.submitted_at, a.finished_at, r.id, r.sha256, r.kind, r.byte_len, r.relative_path, r.created_at";

const ATTEMPT_FROM: &str = "research_attempts a LEFT JOIN research_artifacts r ON r.id = a.result_artifact_id";

fn attempt_from_row(row: &Row<'_>) -> rusqlite::Result<AttemptRow> {
    let status: String = row.get(7)?;
    let input: String = row.get(8)?;
    let engine: String = row.get(9)?;
    let outcome: Option<String> = row.get(10)?;
    let artifact_id: Option<i64> = row.get(14)?;
    let result_artifact = match artifact_id {
        Some(id) => Some(StoredArtifact {
            id,
            reference: ArtifactRef {
                sha256: row.get(15)?,
                kind: row.get(16)?,
                byte_len: row.get(17)?,
                relative_path: row.get(18)?,
            },
            created_at: row.get(19)?,
        }),
        None => None,
    };
    Ok(AttemptRow {
        id: row.get(0)?,
        attempt_key: row.get(1)?,
        hypothesis_id: row.get(2)?,
        strategy_id: row.get(3)?,
        dataset_id: row.get(4)?,
        discovery_run_id: row.get(5)?,
        candidate_index: row.get(6)?,
        status: AttemptStatus::parse(&status).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(error))
        })?,
        input_fingerprint: serde_json::from_str(&input).unwrap_or(Value::Null),
        engine_fingerprint: serde_json::from_str(&engine).unwrap_or(Value::Null),
        outcome: outcome.and_then(|text| serde_json::from_str(&text).ok()),
        result_artifact,
        epoch: row.get(11)?,
        submitted_at: row.get(12)?,
        finished_at: row.get(13)?,
    })
}

/// Which attempts to list; every field narrows. Newest first.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptFilter {
    pub discovery_run_id: Option<i64>,
    pub strategy_id: Option<i64>,
    pub dataset_id: Option<i64>,
    pub hypothesis_id: Option<i64>,
    pub limit: Option<usize>,
}

pub const MAX_ATTEMPT_PAGE: usize = 500;

pub fn list_attempts(conn: &Connection, filter: &AttemptFilter) -> AppResult<Vec<AttemptRow>> {
    let limit = filter.limit.unwrap_or(100).clamp(1, MAX_ATTEMPT_PAGE) as i64;
    let mut stmt = conn.prepare(&format!(
        "SELECT {ATTEMPT_COLUMNS} FROM {ATTEMPT_FROM}
         WHERE (?1 IS NULL OR a.discovery_run_id = ?1)
           AND (?2 IS NULL OR a.strategy_id = ?2)
           AND (?3 IS NULL OR a.dataset_id = ?3)
           AND (?4 IS NULL OR a.hypothesis_id = ?4)
         ORDER BY a.id DESC LIMIT ?5"
    ))?;
    let rows = stmt
        .query_map(
            params![filter.discovery_run_id, filter.strategy_id, filter.dataset_id, filter.hypothesis_id, limit],
            attempt_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_attempt(conn: &Connection, id: i64) -> AppResult<Option<AttemptRow>> {
    Ok(conn
        .query_row(&format!("SELECT {ATTEMPT_COLUMNS} FROM {ATTEMPT_FROM} WHERE a.id = ?1"), [id], attempt_from_row)
        .optional()?)
}

#[cfg(test)]
pub fn get_attempt_by_key(conn: &Connection, key: &str) -> AppResult<Option<AttemptRow>> {
    Ok(conn
        .query_row(&format!("SELECT {ATTEMPT_COLUMNS} FROM {ATTEMPT_FROM} WHERE a.attempt_key = ?1"), [key], attempt_from_row)
        .optional()?)
}

/// Every artifact path the database references (for `ArtifactStore::unreferenced`).
pub fn referenced_artifact_paths(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT relative_path FROM research_artifacts ORDER BY id")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
