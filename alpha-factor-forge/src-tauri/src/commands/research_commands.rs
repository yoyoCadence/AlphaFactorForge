//! P05 — the desktop's read access to the research history: hypotheses,
//! attempts, and the immutable result artifact behind a completed attempt.
//! Reads only: the history is written by the runner inside its own
//! transactions, and nothing edits or deletes it (migration 0007 triggers).
//! Works in both host modes — the rows come from whichever connection the
//! mode offers, and the artifact files live beside the database.
//!
//! DB-ASYNC-001e: SQLite reads run on the shared `database_task` worker and
//! artifact file reads on `artifact_task`, never on the thread polling the
//! command. The DB mutex is still released before any artifact file is read.

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use super::db_commands::database_task;
use crate::error::{AppError, AppResult};
use crate::research::artifacts::{ArtifactRef, ArtifactStore};
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

/// Artifact file work (directory walk, read and checksum) on a blocking
/// worker, like `database_task` does for SQLite. It holds no DB lock.
async fn artifact_task<T: Send + 'static>(
    operation: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| AppError::Other(format!("research artifact task failed: {error}")))?
}

/// The attempt's result document, or why it cannot be shown: a missing or
/// altered file and invalid JSON are reported, never raised.
fn read_result(
    store: &ArtifactStore,
    reference: Option<&ArtifactRef>,
) -> (Option<Value>, Option<String>) {
    match reference {
        Some(reference) => match store.read(reference) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => (Some(value), None),
                Err(error) => (None, Some(format!("artifact is not valid JSON: {error}"))),
            },
            Err(error) => (None, Some(error.to_string())),
        },
        None => (None, None),
    }
}

#[tauri::command]
pub async fn list_research_attempts(
    state: State<'_, AppState>,
    filter: Option<AttemptFilter>,
) -> AppResult<Vec<AttemptRow>> {
    let filter = filter.unwrap_or_default();
    database_task(state.db()?, move |conn| {
        history::list_attempts(conn, &filter)
    })
    .await
}

#[tauri::command]
pub async fn list_hypotheses(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> AppResult<Vec<HypothesisRow>> {
    let limit = limit.unwrap_or(100).clamp(1, history::MAX_ATTEMPT_PAGE);
    database_task(state.db()?, move |conn| {
        history::list_hypotheses(conn, limit)
    })
    .await
}

/// Files under the artifact store that no row references (a crash between
/// storing and committing, or a stale staging entry). Identification only:
/// nothing here deletes, and nothing else does either.
#[tauri::command]
pub async fn list_unreferenced_artifacts(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let referenced =
        database_task(state.db()?, |conn| history::referenced_artifact_paths(conn)).await?;
    let store = ArtifactStore::in_data_dir(&state.data_dir);
    artifact_task(move || Ok(store.unreferenced(&referenced)?)).await
}

#[tauri::command]
pub async fn get_research_attempt(
    state: State<'_, AppState>,
    id: i64,
) -> AppResult<Option<ResearchAttemptDetail>> {
    let rows = database_task(state.db()?, move |conn| {
        let Some(attempt) = history::get_attempt(conn, id)? else {
            return Ok(None);
        };
        let hypothesis = history::get_hypothesis(conn, attempt.hypothesis_id)?;
        Ok(Some((attempt, hypothesis)))
    })
    .await?;
    let Some((attempt, hypothesis)) = rows else {
        return Ok(None);
    };
    let store = ArtifactStore::in_data_dir(&state.data_dir);
    artifact_task(move || {
        let (result, result_error) = read_result(
            &store,
            attempt
                .result_artifact
                .as_ref()
                .map(|stored| &stored.reference),
        );
        Ok(Some(ResearchAttemptDetail {
            attempt,
            hypothesis,
            result,
            result_error,
        }))
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            Self(std::env::temp_dir().join(format!(
                "aff-research-commands-test-{}-{n}",
                std::process::id()
            )))
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            if self.0.exists() {
                std::fs::remove_dir_all(&self.0).unwrap_or_else(|error| {
                    panic!("temp dir {} not removed: {error}", self.0.display())
                });
            }
        }
    }

    #[test]
    fn artifact_work_does_not_run_on_the_command_polling_thread() {
        // The operation cannot finish until the test releases it, so an
        // inline call would make the first poll block instead of yield.
        let (release, gate) = mpsc::channel::<()>();
        let (sent, received) = mpsc::channel();
        let polling_thread = std::thread::spawn(move || {
            let mut command = Box::pin(artifact_task(move || {
                gate.recv().unwrap();
                Ok(7)
            }));
            let mut context = Context::from_waker(Waker::noop());
            match command.as_mut().poll(&mut context) {
                Poll::Pending => {
                    sent.send(true).unwrap();
                    tauri::async_runtime::block_on(command)
                }
                Poll::Ready(result) => {
                    sent.send(false).unwrap();
                    result
                }
            }
        });
        // Finite test-only deadline; release the operation before asserting.
        let yielded = received.recv_timeout(Duration::from_secs(5));
        release.send(()).unwrap();
        assert_eq!(polling_thread.join().unwrap().unwrap(), 7);
        assert!(
            yielded.unwrap(),
            "the first poll must yield while artifact work is pending"
        );
    }

    #[test]
    fn a_panicking_artifact_worker_returns_an_explicit_command_error() {
        let error = tauri::async_runtime::block_on(artifact_task::<()>(|| {
            panic!("controlled test-only worker failure")
        }))
        .unwrap_err();
        assert!(error.to_string().contains("research artifact task failed"));
    }

    #[test]
    fn artifact_results_and_failures_are_reported_unchanged() {
        let dir = TempDir::new();
        let store = ArtifactStore::in_data_dir(&dir.0);
        assert_eq!(read_result(&store, None), (None, None));

        let stored = store.put("candidate-result-v1", br#"{"a":1}"#).unwrap();
        let read = tauri::async_runtime::block_on(artifact_task({
            let (store, stored) = (store.clone(), stored.clone());
            move || Ok(read_result(&store, Some(&stored)))
        }))
        .unwrap();
        assert_eq!(read, (Some(serde_json::json!({ "a": 1 })), None));

        let not_json = store.put("candidate-result-v1", b"not json").unwrap();
        let (result, error) = read_result(&store, Some(&not_json));
        assert_eq!(result, None);
        assert!(error.unwrap().starts_with("artifact is not valid JSON:"));

        std::fs::write(store.path_of(&stored.relative_path).unwrap(), br#"{"a":2}"#).unwrap();
        let (result, error) = read_result(&store, Some(&stored));
        assert_eq!(result, None);
        assert!(error
            .unwrap()
            .contains("does not match its recorded checksum"));

        // The directory walk crosses the worker with its result unchanged.
        let unreferenced = tauri::async_runtime::block_on(artifact_task({
            let store = store.clone();
            let referenced = vec![stored.relative_path.clone()];
            move || Ok(store.unreferenced(&referenced)?)
        }))
        .unwrap();
        assert_eq!(unreferenced, vec![not_json.relative_path]);
    }
}
