//! P05 — the desktop's read access to the research history: hypotheses,
//! attempts, and the immutable result artifact behind a completed attempt.
//! Reads only: the history is written by the runner inside its own
//! transactions, and nothing edits or deletes it (migration 0007 triggers).
//! Works in both host modes — the rows come from whichever connection the
//! mode offers, and the artifact files live beside the database.

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::research::artifacts::ArtifactStore;
use crate::research::history::{self, AttemptFilter, AttemptRow, HypothesisRow};
use crate::AppState;

/// One attempt with the hypothesis it tested and, when it completed with an
/// artifact, the complete result document (verified against its checksum
/// on every read).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchAttemptDetail {
    pub attempt: AttemptRow,
    pub hypothesis: Option<HypothesisRow>,
    /// `candidate-result-v1`, or `null` when the attempt has no artifact
    /// (not completed, or completed by a host that keeps none).
    pub result: Option<Value>,
    /// Set instead of `result` when the artifact file is missing or fails
    /// its checksum: the row is the evidence, the file is not.
    pub result_error: Option<String>,
}

#[tauri::command]
pub fn list_research_attempts(state: State<'_, AppState>, filter: Option<AttemptFilter>) -> AppResult<Vec<AttemptRow>> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    history::list_attempts(&conn, &filter.unwrap_or_default())
}

#[tauri::command]
pub fn list_hypotheses(state: State<'_, AppState>, limit: Option<usize>) -> AppResult<Vec<HypothesisRow>> {
    let db = state.db()?;
    let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
    history::list_hypotheses(&conn, limit.unwrap_or(100).clamp(1, history::MAX_ATTEMPT_PAGE))
}

/// Files under the artifact store that no row references (a crash between
/// storing and committing, or a stale staging entry). Identification only:
/// nothing here deletes, and nothing else does either.
#[tauri::command]
pub fn list_unreferenced_artifacts(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let db = state.db()?;
    let referenced = {
        let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
        history::referenced_artifact_paths(&conn)?
    };
    Ok(ArtifactStore::in_data_dir(&state.data_dir).unreferenced(&referenced)?)
}

#[tauri::command]
pub fn get_research_attempt(state: State<'_, AppState>, id: i64) -> AppResult<Option<ResearchAttemptDetail>> {
    let db = state.db()?;
    let (attempt, hypothesis) = {
        let conn = db.lock().map_err(|_| AppError::Other("db lock poisoned".into()))?;
        let Some(attempt) = history::get_attempt(&conn, id)? else {
            return Ok(None);
        };
        let hypothesis = history::get_hypothesis(&conn, attempt.hypothesis_id)?;
        (attempt, hypothesis)
    };
    let store = ArtifactStore::in_data_dir(&state.data_dir);
    let (result, result_error) = match &attempt.result_artifact {
        Some(stored) => match store.read(&stored.reference) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => (Some(value), None),
                Err(error) => (None, Some(format!("artifact is not valid JSON: {error}"))),
            },
            Err(error) => (None, Some(error.to_string())),
        },
        None => (None, None),
    };
    Ok(Some(ResearchAttemptDetail { attempt, hypothesis, result, result_error }))
}
