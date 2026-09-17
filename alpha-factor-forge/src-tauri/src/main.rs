// SKELETON — Tauri app entry. Wires AppState (SQLite) + invoke handlers.
// Verify: cd src-tauri && cargo check  (needs local Rust toolchain)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod desktop;
mod discovery_runner;
mod error;
mod identity;
mod runtime;
mod single_instance;

use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Shared application state. Discovery compute workers never receive this
/// connection; one coordinator serializes runner writes through the mutex.
pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub discovery: discovery_runner::DiscoveryRunner,
    /// P03a: the workspace lease (OS lock + epoch + heartbeat). Held here for
    /// the life of the process; dropping it would release ownership.
    pub ownership: runtime::OwnershipHandle,
}

fn main() {
    tauri::Builder::default()
        // Must be the first plugin and must run before setup: a secondary
        // process must exit before startup recovery can touch the live
        // primary process's discovery run.
        .plugin(single_instance::plugin())
        .setup(|app| {
            // The desktop's only host-specific input: where its data directory
            // is. Opening, migrating, and startup repair are the shared
            // orchestration in `runtime`, so a headless host (P04) reaches the
            // same database state through the same code.
            let db_path = app
                .path()
                .app_data_dir()
                .expect("no app data dir")
                .join(db::DB_FILE_NAME);
            // P03a: the desktop is the embedded host and must OWN the
            // workspace (OS lock -> open -> migrate -> epoch -> recovery ->
            // heartbeat). Another host holding the lock, or a database written
            // by a newer build, stops startup here with the reason; connect
            // mode arrives with P04. Startup repair is persistence-only:
            // orphaned running work is paused/requeued, but no CPU work
            // resumes without a user command.
            let workspace = match runtime::open_workspace(&db_path, db::ownership::HolderKind::DesktopEmbedded) {
                Ok(workspace) => workspace,
                Err(error) => panic!("cannot own the workspace at {}: {error}", db_path.display()),
            };
            if workspace.recovery != db::discovery::RecoveryReport::default() {
                eprintln!(
                    "startup recovery: paused {} orphaned run(s), requeued {} job(s)",
                    workspace.recovery.runs_paused, workspace.recovery.jobs_requeued
                );
            }
            app.manage(AppState {
                db: workspace.db,
                discovery: workspace.discovery,
                ownership: workspace.ownership,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // --- Database (Phase A) ---
            commands::db_commands::init_database,
            commands::db_commands::run_migrations,
            commands::db_commands::get_datasets,
            commands::db_commands::get_candles,
            commands::db_commands::import_candles,
            commands::db_commands::save_strategy,
            commands::db_commands::get_strategies,
            commands::db_commands::save_backtest_result,
            commands::db_commands::get_backtest_results,
            // --- Results Explorer (Phase B, P01) ---
            commands::db_commands::get_backtest_result_detail,
            // --- Validation records (Phase B, PERSIST-001) ---
            commands::db_commands::save_validation_record,
            commands::db_commands::list_validation_records,
            commands::db_commands::get_validation_record,
            // --- Files (Phase A, minimal) ---
            commands::file_commands::save_report,
            commands::file_commands::export_report,
            // --- Native pop-out windows (Phase A UI) ---
            commands::window_commands::open_popout_window,
            // --- AI (Phase C stub) ---
            commands::ai_commands::generate_strategy_dsl,
            commands::ai_commands::validate_strategy_dsl,
            // --- Secrets (Phase C stub) ---
            commands::secret_commands::save_ai_api_key,
            commands::secret_commands::get_ai_api_key_status,
            commands::secret_commands::delete_ai_api_key,
            commands::secret_commands::test_ai_connection,
            // --- Discovery runner (Phase B) ---
            commands::discovery_commands::start_discovery,
            commands::discovery_commands::pause_discovery,
            commands::discovery_commands::resume_discovery,
            commands::discovery_commands::cancel_discovery,
            commands::discovery_commands::get_discovery_progress,
            commands::discovery_commands::get_active_discovery_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AlphaFactorForge");
}
