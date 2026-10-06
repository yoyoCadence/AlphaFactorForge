//! Thin Tauri command boundary for RUNNER-EXEC-001.
//!
//! The command names and camel-case invoke arguments are the existing public
//! contract. Blocking admission/control work runs off the WebView thread; the
//! coordinator and its fixed compute pool continue independently afterward.
//!
//! P04b: in connect mode every command here is proxied to the background
//! service as a `research-command-v1` envelope (`runtime::connect`); the
//! window sees the same results and the same events, forwarded from the
//! service's ledger. Snapshots are returned as JSON in both modes so the
//! proxied answer and the embedded one are the same bytes.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::desktop::discovery_events::TauriDiscoveryEventSink;
use crate::discovery_runner::DiscoveryEventSink;
use crate::error::{AppError, AppResult};
use crate::runtime::commands::LedgerSink;
use crate::{AppState, HostSnapshot};

/// P03b: the desktop sink, wrapped so every event is appended to the
/// persistent ledger (`runtime_events`) before the window sees it. The
/// envelope path (`runtime_commands`) does the same, so a reconnecting
/// reader gets one cursor regardless of which entry point started the run.
fn ledgered_sink(app: AppHandle, db: crate::runtime::SharedDb, epoch: i64) -> Arc<dyn DiscoveryEventSink> {
    Arc::new(LedgerSink::new(db, epoch, Arc::new(TauriDiscoveryEventSink::new(app))))
}

fn join_error(error: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("discovery command task failed: {error}"))
}

fn to_json<T: serde::Serialize>(value: T) -> AppResult<Value> {
    serde_json::to_value(value).map_err(AppError::from)
}

/// DB-ASYNC-001f: a progress read — the embedded DB mutex, which a running
/// discovery also takes for its commits, or the connect-mode round trip to
/// the service — on a blocking worker, never on the thread polling the
/// command. The window polls these readers while a run is active.
async fn read_task(
    operation: impl FnOnce() -> AppResult<Value> + Send + 'static,
) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn start_discovery(
    app: AppHandle,
    state: State<'_, AppState>,
    config: Value,
) -> AppResult<i64> {
    let _admitted = state.admission.admit()?;
    match state.snapshot()? {
        HostSnapshot::Embedded { db, discovery, epoch, .. } => {
            let sink = ledgered_sink(app, db.clone(), epoch);
            tauri::async_runtime::spawn_blocking(move || discovery.start(db, sink, config))
                .await
                .map_err(join_error)?
        }
        HostSnapshot::Connected(proxy) => {
            tauri::async_runtime::spawn_blocking(move || proxy.start(config)).await.map_err(join_error)?
        }
    }
}

#[tauri::command]
pub async fn pause_discovery(state: State<'_, AppState>, run_id: i64) -> AppResult<()> {
    let _admitted = state.admission.admit()?;
    match state.snapshot()? {
        HostSnapshot::Embedded { db, discovery, .. } => {
            tauri::async_runtime::spawn_blocking(move || discovery.pause(&db, run_id))
                .await
                .map_err(join_error)?
        }
        HostSnapshot::Connected(proxy) => {
            tauri::async_runtime::spawn_blocking(move || proxy.pause(run_id)).await.map_err(join_error)?
        }
    }
}

#[tauri::command]
pub async fn resume_discovery(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: i64,
) -> AppResult<()> {
    let _admitted = state.admission.admit()?;
    match state.snapshot()? {
        HostSnapshot::Embedded { db, discovery, epoch, .. } => {
            let sink = ledgered_sink(app, db.clone(), epoch);
            tauri::async_runtime::spawn_blocking(move || discovery.resume(db, sink, run_id))
                .await
                .map_err(join_error)?
        }
        HostSnapshot::Connected(proxy) => {
            tauri::async_runtime::spawn_blocking(move || proxy.resume(run_id)).await.map_err(join_error)?
        }
    }
}

#[tauri::command]
pub async fn cancel_discovery(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: i64,
) -> AppResult<()> {
    let _admitted = state.admission.admit()?;
    match state.snapshot()? {
        HostSnapshot::Embedded { db, discovery, epoch, .. } => {
            let sink = ledgered_sink(app, db.clone(), epoch);
            tauri::async_runtime::spawn_blocking(move || discovery.cancel(&db, sink, run_id))
                .await
                .map_err(join_error)?
        }
        HostSnapshot::Connected(proxy) => {
            tauri::async_runtime::spawn_blocking(move || proxy.cancel(run_id)).await.map_err(join_error)?
        }
    }
}

/// `discovery-progress-v1`, as JSON (`DiscoveryProgressSnapshot` in embedded
/// mode; the service's identical snapshot in connect mode).
#[tauri::command]
pub async fn get_discovery_progress(state: State<'_, AppState>, run_id: i64) -> AppResult<Value> {
    match state.snapshot()? {
        HostSnapshot::Embedded { db, discovery, .. } => {
            read_task(move || to_json(discovery.progress(&db, run_id)?)).await
        }
        HostSnapshot::Connected(proxy) => read_task(move || proxy.progress(run_id)).await,
    }
}

/// Recovery discoverability: after startup turns an orphan into `paused`, the
/// frontend can find its run id without relying on stale browser memory. In
/// connect mode this is also how a reopened desktop adopts the service's
/// run (contract §3: snapshot first, then the forwarded ledger).
#[tauri::command]
pub async fn get_active_discovery_run(state: State<'_, AppState>) -> AppResult<Value> {
    match state.snapshot()? {
        HostSnapshot::Embedded { db, discovery, .. } => {
            read_task(move || to_json(discovery.active_progress(&db)?)).await
        }
        HostSnapshot::Connected(proxy) => read_task(move || proxy.active()).await,
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::{mpsc, Mutex};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    use super::*;
    use crate::discovery_runner::DiscoveryRunner;

    fn database() -> crate::runtime::SharedDb {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::apply_migrations(&conn).unwrap();
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn a_progress_read_does_not_block_the_command_polling_thread() {
        let db = database();
        let runner = DiscoveryRunner::default();
        // Deliberately keep SQLite busy, as a committing run does, until
        // after the first command poll.
        let held = db.lock().unwrap();
        let (sent, received) = mpsc::channel();
        let worker_db = db.clone();
        let polling_thread = std::thread::spawn(move || {
            let mut command = Box::pin(read_task(move || {
                to_json(runner.active_progress(&worker_db)?)
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
        // Finite test-only deadline; release the lock before asserting.
        let yielded = received.recv_timeout(Duration::from_secs(5));
        drop(held);
        assert_eq!(polling_thread.join().unwrap().unwrap(), Value::Null);
        assert!(
            yielded.unwrap(),
            "the first poll must yield while DB is busy"
        );
    }

    #[test]
    fn reader_results_and_errors_cross_the_worker_unchanged() {
        let db = database();
        let runner = DiscoveryRunner::default();
        let active = tauri::async_runtime::block_on(read_task({
            let (db, runner) = (db.clone(), runner.clone());
            move || to_json(runner.active_progress(&db)?)
        }))
        .unwrap();
        assert_eq!(active, Value::Null, "no active run is still `null`");
        let error =
            tauri::async_runtime::block_on(read_task(move || to_json(runner.progress(&db, 404)?)))
                .unwrap_err();
        assert_eq!(error.to_string(), "discovery run 404 not found");
    }

    #[test]
    fn a_panicking_reader_returns_an_explicit_command_error() {
        let error = tauri::async_runtime::block_on(read_task(|| {
            panic!("controlled test-only worker failure")
        }))
        .unwrap_err();
        assert!(error.to_string().contains("discovery command task failed"));
    }
}
