// SKELETON — Tauri app entry. Wires AppState (SQLite) + invoke handlers.
// Verify: cd src-tauri && cargo check  (needs local Rust toolchain)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod desktop;
mod discovery_runner;
mod error;
mod identity;
mod market;
mod research;
mod runtime;
mod single_instance;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;

use runtime::connect::ServiceProxy;
use runtime::host::{Admission, HostMode, ServiceLauncher};
use runtime::SharedDb;

/// Shared application state. Discovery compute workers never receive the
/// database connection; one coordinator serializes runner writes through
/// the mutex.
///
/// P04b: the desktop is one of two hosts. In embedded mode it OWNS the
/// workspace (`HostMode::Embedded`: the P03a lease, epoch, heartbeat, and
/// runner); in connect mode a background service owns it and the desktop
/// proxies (`HostMode::Connected`). Commands take what they need out of the
/// mode under the lock and never hold the lock across their work, so a
/// hand-over (`runtime::host`) can swap modes underneath them.
pub struct AppState {
    pub host: Mutex<HostMode>,
    /// Embedded mutating commands hold an admission guard for their whole
    /// run; a hand-over closes admission and drains them first.
    pub admission: Admission,
    /// The workspace directory (database, lock, endpoint files).
    pub data_dir: PathBuf,
    /// P03b: request ids currently executing, shared by every dispatcher this
    /// process builds, so a retry never runs beside its first attempt.
    pub in_flight: Arc<runtime::commands::InFlightRequests>,
    /// How the desktop starts the background service (the binary beside it).
    pub launcher: Box<dyn ServiceLauncher>,
}

/// What a command copied out of the mode lock.
pub enum HostSnapshot {
    Embedded {
        db: SharedDb,
        discovery: discovery_runner::DiscoveryRunner,
        epoch: i64,
        instance_id: String,
        workspace_id: String,
    },
    Connected(ServiceProxy),
}

impl AppState {
    fn lock_host(&self) -> error::AppResult<std::sync::MutexGuard<'_, HostMode>> {
        self.host.lock().map_err(|_| error::AppError::Other("host mode lock poisoned".into()))
    }

    /// The database for repository commands, in either steady mode.
    pub fn db(&self) -> error::AppResult<SharedDb> {
        self.lock_host()?.db()
    }

    pub fn mode_kind(&self) -> error::AppResult<&'static str> {
        Ok(self.lock_host()?.kind())
    }

    /// A copy of what the current mode offers a command; an error while a
    /// hand-over is in progress.
    pub fn snapshot(&self) -> error::AppResult<HostSnapshot> {
        let mode = self.lock_host()?;
        match &*mode {
            HostMode::Embedded(workspace) => Ok(HostSnapshot::Embedded {
                db: workspace.db.clone(),
                discovery: workspace.discovery.clone(),
                epoch: workspace.ownership.epoch,
                instance_id: workspace.ownership.instance_id.clone(),
                workspace_id: workspace.workspace_id.clone(),
            }),
            HostMode::Connected(connected) => Ok(HostSnapshot::Connected(connected.proxy())),
            HostMode::Switching(reason) => Err(error::AppError::Other(format!("the workspace is not available: {reason}"))),
        }
    }
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
            // orchestration in `runtime`, so the headless service reaches the
            // same database state through the same code.
            // `AFF_DATA_DIR` (P04b) points both binaries at an isolated workspace
            // for native smokes; otherwise this is tauri's app data directory,
            // which the service resolves identically (`service::default_data_dir`).
            let data_dir = std::env::var_os(runtime::service::DATA_DIR_ENV)
                .filter(|dir| !dir.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| app.path().app_data_dir().expect("no app data dir"));
            // P03a/P04b: the desktop OWNS the workspace when nobody does (OS
            // lock -> open -> migrate -> epoch -> recovery -> heartbeat), and
            // CONNECTS when a background service owns it and answers on its
            // published endpoint. An owner that publishes nothing, or a
            // database written by a newer build, stops startup here with the
            // reason. Startup repair is persistence-only: orphaned running
            // work is paused/requeued, but no CPU work resumes without a user
            // command.
            let mode = match runtime::host::open_or_connect(&data_dir) {
                Ok(mode) => mode,
                Err(error) => panic!("cannot use the workspace at {}: {error}", data_dir.display()),
            };
            match &mode {
                HostMode::Embedded(workspace) => {
                    if workspace.recovery != db::discovery::RecoveryReport::default() {
                        eprintln!(
                            "startup recovery: paused {} orphaned run(s), requeued {} job(s)",
                            workspace.recovery.runs_paused, workspace.recovery.jobs_requeued
                        );
                    }
                }
                HostMode::Connected(connected) => {
                    // Follow the service's ledger from its current end; the
                    // window takes its own snapshot first (contract §3).
                    let sink = Arc::new(desktop::discovery_events::TauriDiscoveryEventSink::new(app.handle().clone()));
                    let after = connected.ledger_end().unwrap_or(0);
                    if let Err(error) = connected.follow_events(sink, after) {
                        eprintln!("cannot follow the background service's events: {error}");
                    }
                    let manifest = connected.proxy().manifest;
                    eprintln!("connected to the background service (epoch {}, port {})", manifest.epoch, manifest.port);
                }
                HostMode::Switching(_) => unreachable!("open_or_connect never yields a switching mode"),
            }
            let service_exe = std::env::current_exe()
                .map(|exe| runtime::host::service_executable_beside(&exe))
                .unwrap_or_default();
            app.manage(AppState {
                host: Mutex::new(mode),
                admission: Admission::default(),
                data_dir,
                in_flight: Arc::new(runtime::commands::InFlightRequests::default()),
                launcher: Box::new(runtime::host::ExecutableLauncher { exe: service_exe }),
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
            // --- Versioned command envelope (P03b, research-command-v1) ---
            commands::runtime_commands::get_workspace_info,
            commands::runtime_commands::dispatch_research_command,
            // --- Research history (P05) ---
            commands::research_commands::list_research_attempts,
            commands::research_commands::list_hypotheses,
            commands::research_commands::get_research_attempt,
            commands::research_commands::list_unreferenced_artifacts,
            // --- Host mode (P04b: background service hand-over) ---
            commands::runtime_commands::get_host_status,
            commands::runtime_commands::enter_background_mode,
            commands::runtime_commands::exit_background_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AlphaFactorForge");
}
