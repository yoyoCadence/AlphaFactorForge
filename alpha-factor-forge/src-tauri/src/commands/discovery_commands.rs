//! Thin Tauri command boundary for RUNNER-EXEC-001.
//!
//! The command names and camel-case invoke arguments are the existing public
//! contract. Blocking admission/control work runs off the WebView thread; the
//! coordinator and its fixed compute pool continue independently afterward.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::discovery_runner::{DiscoveryProgressSnapshot, TauriDiscoveryEventSink};
use crate::error::{AppError, AppResult};
use crate::AppState;

fn join_error(error: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("discovery command task failed: {error}"))
}

#[tauri::command]
pub async fn start_discovery(
    app: AppHandle,
    state: State<'_, AppState>,
    config: Value,
) -> AppResult<i64> {
    let db = state.db.clone();
    let runner = state.discovery.clone();
    let sink = Arc::new(TauriDiscoveryEventSink::new(app));
    tauri::async_runtime::spawn_blocking(move || runner.start(db, sink, config))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn pause_discovery(state: State<'_, AppState>, run_id: i64) -> AppResult<()> {
    let db = state.db.clone();
    let runner = state.discovery.clone();
    tauri::async_runtime::spawn_blocking(move || runner.pause(&db, run_id))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn resume_discovery(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: i64,
) -> AppResult<()> {
    let db = state.db.clone();
    let runner = state.discovery.clone();
    let sink = Arc::new(TauriDiscoveryEventSink::new(app));
    tauri::async_runtime::spawn_blocking(move || runner.resume(db, sink, run_id))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub async fn cancel_discovery(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: i64,
) -> AppResult<()> {
    let db = state.db.clone();
    let runner = state.discovery.clone();
    let sink = Arc::new(TauriDiscoveryEventSink::new(app));
    tauri::async_runtime::spawn_blocking(move || runner.cancel(&db, sink, run_id))
        .await
        .map_err(join_error)?
}

#[tauri::command]
pub fn get_discovery_progress(
    state: State<'_, AppState>,
    run_id: i64,
) -> AppResult<DiscoveryProgressSnapshot> {
    state.discovery.progress(&state.db, run_id)
}

/// Recovery discoverability: after startup turns an orphan into `paused`, the
/// frontend can find its run id without relying on stale browser memory.
#[tauri::command]
pub fn get_active_discovery_run(
    state: State<'_, AppState>,
) -> AppResult<Option<DiscoveryProgressSnapshot>> {
    state.discovery.active_progress(&state.db)
}
