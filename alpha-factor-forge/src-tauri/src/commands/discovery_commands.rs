// STUB — Phase B discovery job runner commands. NOT active in Phase A.
// The runner executes on a backend thread pool (NOT the UI thread), persists
// checkpoints to SQLite, and emits progress via Tauri events:
//   "discovery://progress" | "discovery://result" | "discovery://done"
// v1 runs TRAIN/VALIDATION segments only — never TEST.

use crate::error::{AppError, AppResult};

#[tauri::command]
pub fn start_discovery(_config: serde_json::Value) -> AppResult<i64> {
    Err(AppError::NotImplemented(
        "start_discovery: Phase B. Create discovery_run, enqueue jobs (train/val), \
         spawn thread pool, emit events (TODO.md).",
    ))
}

#[tauri::command]
pub fn pause_discovery(_run_id: i64) -> AppResult<()> {
    Err(AppError::NotImplemented("pause_discovery: Phase B (TODO.md)."))
}

#[tauri::command]
pub fn resume_discovery(_run_id: i64) -> AppResult<()> {
    Err(AppError::NotImplemented("resume_discovery: Phase B, resume from checkpoint (TODO.md)."))
}

#[tauri::command]
pub fn cancel_discovery(_run_id: i64) -> AppResult<()> {
    Err(AppError::NotImplemented("cancel_discovery: Phase B (TODO.md)."))
}

#[tauri::command]
pub fn get_discovery_progress(_run_id: i64) -> AppResult<serde_json::Value> {
    Err(AppError::NotImplemented("get_discovery_progress: Phase B (TODO.md)."))
}
