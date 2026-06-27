// STUB — file import/export. Phase A wires export_report minimally; the rest
// (export/import strategies, backup/restore db) are Phase B/maintenance.

use crate::error::{AppError, AppResult};

/// TODO(local): render a backtest result to a report file (JSON/CSV/HTML) and
/// return the written path. The browser version already builds JSON/CSV reports;
/// port that formatting here.
#[tauri::command]
pub fn export_report(_result_id: i64) -> AppResult<String> {
    Err(AppError::NotImplemented(
        "export_report: port report formatting from the existing UI (TODO.md)",
    ))
}
