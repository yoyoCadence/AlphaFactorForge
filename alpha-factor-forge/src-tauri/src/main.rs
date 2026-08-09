// SKELETON — Tauri app entry. Wires AppState (SQLite) + invoke handlers.
// Verify: cd src-tauri && cargo check  (needs local Rust toolchain)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod discovery_runner;
mod error;
mod identity;
mod single_instance;

use std::sync::{Arc, Mutex};
use tauri::Manager;

/// Shared application state. Discovery compute workers never receive this
/// connection; one coordinator serializes runner writes through the mutex.
pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub discovery: discovery_runner::DiscoveryRunner,
}

fn main() {
    tauri::Builder::default()
        // Must be the first plugin and must run before setup: a secondary
        // process must exit before startup recovery can touch the live
        // primary process's discovery run.
        .plugin(single_instance::plugin())
        .setup(|app| {
            // Resolve app data dir and open/initialize the database there.
            let conn = db::initialize(app.handle()).expect("failed to initialize SQLite database");
            let db = Arc::new(Mutex::new(conn));
            let discovery = discovery_runner::DiscoveryRunner::default();
            // Startup repair is persistence-only: orphaned running work is
            // paused/requeued, but no CPU work resumes without a user command.
            discovery
                .recover_orphans(&db)
                .expect("failed to recover orphaned discovery runs");
            app.manage(AppState { db, discovery });
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
