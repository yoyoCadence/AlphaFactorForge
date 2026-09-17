//! P04a — the headless research service (plan §3.1, contract §1.1 host
//! kind `service`, §4 control interface).
//!
//! `alpha-factor-forge-service run` owns the workspace exactly as the
//! desktop does (`runtime::open_workspace`, host kind `service`), then
//! answers `research-command-v1` envelopes on a loopback port until it is
//! told to stop. A run started through it keeps going after every client
//! has gone away, because the coordinator lives in this process, not in
//! whoever asked for it; a client that comes back takes a snapshot
//! (`discovery.active`) and follows the ledger from its cursor.
//!
//! Stopping is a drain, not a kill: new work is refused, every live
//! coordinator is asked to pause and given time to commit its checkpoint,
//! the endpoint is withdrawn, and only then is the lock released — so the
//! next owner finds a paused run, not an orphan. There is no signal
//! handler: Ctrl+C or a kill is the crash path, which the next owner's
//! startup recovery already handles (P02/P03a), and the manifest such a
//! service leaves behind is detected as stale by `stop`/`status` because
//! nothing answers on its port for its instance id.
//!
//! This file is compiled into both binaries; only `service_main.rs` calls
//! `main`. Everything here is testable in-process (`run` on a thread).

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::commands::{Dispatcher, InFlightRequests};
use super::control_api::{
    self, ControlServer, ControlToken, EndpointManifest, EventNotifier, NotifyingSink, ServiceIdentity,
    MANIFEST_VERSION, SERVICE_VERSION,
};
use super::control_client::{ClientError, ControlClient};
use super::{open_workspace_with, SharedDb};
use crate::discovery_runner::DiscoveryRunner;
use crate::db::ownership::{HolderKind, HEARTBEAT_PERIOD};
use crate::db::{self, discovery::RecoveryReport};
use crate::error::AppError;

/// The desktop's bundle identifier (`tauri.conf.json`). The workspace lives
/// under `<data dir>/<identifier>`, and the service must find the SAME
/// directory the desktop uses, or the two would own different databases.
pub const APP_IDENTIFIER: &str = "com.alphafactorforge.desktop";
pub const BINARY_NAME: &str = "alpha-factor-forge-service";

pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILURE: i32 = 1;
/// Another host holds the workspace lock.
pub const EXIT_NOT_OWNER: i32 = 2;
/// The database was written by a newer build.
pub const EXIT_SCHEMA_TOO_NEW: i32 = 3;
/// `stop`/`status`: no live service is published for this workspace.
pub const EXIT_NO_SERVICE: i32 = 4;
pub const EXIT_USAGE: i32 = 64;

/// How long `run` waits for paused coordinators to exit before it releases
/// the workspace anyway (the next owner's recovery then pauses them).
pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(60);
/// How long `stop` waits for the service to go away after it accepted.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(90);

pub const USAGE: &str = "\
alpha-factor-forge-service — headless AlphaFactorForge research service

USAGE:
  alpha-factor-forge-service [run]    [--data-dir <dir>]
  alpha-factor-forge-service stop     [--data-dir <dir>]
  alpha-factor-forge-service status   [--data-dir <dir>]
  alpha-factor-forge-service --help

  run     Own the workspace and serve the loopback control API until stopped.
  stop    Ask the running service to drain and exit; waits until it has.
  status  Print the published endpoint and whether it answers.

  --data-dir  The workspace directory (database, lock, endpoint files).
              Default: the desktop's app data directory.

EXIT CODES:
  0 ok   1 failure   2 another host owns the workspace
  3 database is newer than this build   4 no live service   64 usage";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cli {
    Run { data_dir: Option<PathBuf> },
    Stop { data_dir: Option<PathBuf> },
    Status { data_dir: Option<PathBuf> },
    Help,
}

/// `[subcommand] [--data-dir <dir> | --data-dir=<dir>] | --help`.
pub fn parse_args<S: AsRef<str>>(args: &[S]) -> Result<Cli, String> {
    let mut args = args.iter().map(AsRef::as_ref);
    let mut subcommand: Option<&str> = None;
    let mut data_dir: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg {
            "--help" | "-h" | "help" => return Ok(Cli::Help),
            "--data-dir" => {
                let value = args.next().ok_or("--data-dir needs a directory")?;
                data_dir = Some(PathBuf::from(value));
            }
            _ if arg.starts_with("--data-dir=") => {
                data_dir = Some(PathBuf::from(&arg["--data-dir=".len()..]));
            }
            "run" | "stop" | "status" if subcommand.is_none() => subcommand = Some(arg),
            _ => return Err(format!("unexpected argument: {arg}")),
        }
    }
    Ok(match subcommand.unwrap_or("run") {
        "stop" => Cli::Stop { data_dir },
        "status" => Cli::Status { data_dir },
        _ => Cli::Run { data_dir },
    })
}

/// `<platform data dir>/<identifier>`: the same resolution as tauri's
/// `app_data_dir` (which is `dirs::data_dir().join(identifier)`), so on
/// Windows this is `%APPDATA%\com.alphafactorforge.desktop`.
pub fn default_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join(APP_IDENTIFIER))
}

pub fn resolve_data_dir(explicit: Option<PathBuf>) -> Result<PathBuf, ServiceError> {
    match explicit {
        Some(dir) => Ok(dir),
        None => default_data_dir().ok_or_else(|| ServiceError::Other("cannot resolve the platform data directory; pass --data-dir".into())),
    }
}

#[derive(Debug)]
pub enum ServiceError {
    NotOwner(String),
    SchemaTooNew(String),
    Other(String),
}

impl ServiceError {
    pub fn exit_code(&self) -> i32 {
        match self {
            ServiceError::NotOwner(_) => EXIT_NOT_OWNER,
            ServiceError::SchemaTooNew(_) => EXIT_SCHEMA_TOO_NEW,
            ServiceError::Other(_) => EXIT_FAILURE,
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::NotOwner(message) => write!(f, "another host owns this workspace: {message}"),
            ServiceError::SchemaTooNew(message) => write!(f, "{message}"),
            ServiceError::Other(message) => f.write_str(message),
        }
    }
}

impl From<AppError> for ServiceError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::NotOwner(message) => ServiceError::NotOwner(message),
            AppError::SchemaTooNew(message) => ServiceError::SchemaTooNew(message),
            other => ServiceError::Other(other.to_string()),
        }
    }
}

impl From<io::Error> for ServiceError {
    fn from(error: io::Error) -> Self {
        ServiceError::Other(error.to_string())
    }
}

impl From<ClientError> for ServiceError {
    fn from(error: ClientError) -> Self {
        ServiceError::Other(error.to_string())
    }
}

pub struct RunOptions {
    pub heartbeat_period: Duration,
    pub drain_timeout: Duration,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self { heartbeat_period: HEARTBEAT_PERIOD, drain_timeout: DRAIN_TIMEOUT }
    }
}

fn log(message: impl fmt::Display) {
    println!("{} service: {message}", chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
}

/// Own the workspace at `data_dir`, publish the endpoint, serve until a
/// shutdown is requested (or ownership is lost), drain, withdraw, release.
pub fn run(data_dir: &Path, options: RunOptions) -> Result<(), ServiceError> {
    let db_path = data_dir.join(db::DB_FILE_NAME);
    let workspace = open_workspace_with(&db_path, HolderKind::Service, options.heartbeat_period)?;
    log(format!(
        "owning {} as {} (epoch {}, instance {})",
        db_path.display(),
        workspace.ownership.kind.as_str(),
        workspace.ownership.epoch,
        workspace.ownership.instance_id
    ));
    if workspace.recovery != RecoveryReport::default() {
        log(format!(
            "startup recovery: paused {} orphaned run(s), requeued {} job(s)",
            workspace.recovery.runs_paused, workspace.recovery.jobs_requeued
        ));
    }

    let token = ControlToken::generate()?;
    let notifier = Arc::new(EventNotifier::default());
    let dispatcher = Arc::new(Dispatcher::new(
        workspace.db.clone(),
        workspace.discovery.clone(),
        workspace.ownership.epoch,
        workspace.workspace_id.clone(),
        Arc::new(NotifyingSink(notifier.clone())),
        Arc::new(InFlightRequests::default()),
    ));
    let identity = ServiceIdentity {
        workspace_id: workspace.workspace_id.clone(),
        epoch: workspace.ownership.epoch,
        holder_kind: workspace.ownership.kind.as_str(),
        instance_id: workspace.ownership.instance_id.clone(),
        pid: std::process::id(),
    };
    let server = ControlServer::bind(dispatcher, token.clone(), identity, notifier)?;
    let manifest = EndpointManifest {
        manifest_version: MANIFEST_VERSION.into(),
        port: server.port(),
        workspace_id: workspace.workspace_id.clone(),
        epoch: workspace.ownership.epoch,
        holder_kind: workspace.ownership.kind.as_str().into(),
        instance_id: workspace.ownership.instance_id.clone(),
        pid: std::process::id(),
        service_version: SERVICE_VERSION.into(),
        started_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    };
    if let Err(error) = control_api::write_endpoint_files(data_dir, &manifest, &token) {
        server.stop();
        drop(workspace);
        return Err(ServiceError::Other(format!("cannot publish the endpoint in {}: {error}", data_dir.display())));
    }
    log(format!(
        "listening on 127.0.0.1:{} (manifest {})",
        server.port(),
        control_api::manifest_path(data_dir).display()
    ));

    loop {
        if server.wait_for_shutdown_request(Duration::from_secs(1)) {
            log("shutdown requested; draining");
            break;
        }
        if workspace.ownership.lost() {
            log("workspace ownership was lost; stopping");
            server.request_shutdown();
            break;
        }
    }

    drain(&workspace.discovery, &workspace.db, options.drain_timeout);
    // Withdraw the endpoint BEFORE the lock is released: the moment it is
    // free the next owner may publish its own manifest, which must not be
    // the one removed here.
    server.stop();
    if let Err(error) = control_api::remove_endpoint_files(data_dir) {
        log(format!("cannot remove the endpoint files: {error}"));
    }
    drop(workspace);
    log("stopped");
    Ok(())
}

/// Ask every live coordinator to pause and wait for them to exit. A
/// coordinator that cannot be paused (already pausing or cancelling, or a
/// stale epoch) exits on its own; one still alive at the deadline is left
/// to the next owner's startup recovery, which pauses it.
pub(crate) fn drain(runner: &DiscoveryRunner, db: &SharedDb, timeout: Duration) {
    match runner.active_coordinator_run_ids() {
        Ok(run_ids) => {
            for run_id in run_ids {
                match runner.pause(db, run_id) {
                    Ok(()) => log(format!("pausing run {run_id}")),
                    Err(error) => log(format!("run {run_id} not paused ({error}); waiting for its coordinator")),
                }
            }
        }
        Err(error) => log(format!("cannot list coordinators: {error}")),
    }
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = runner.active_coordinator_run_ids().unwrap_or_default();
        if remaining.is_empty() {
            return;
        }
        if Instant::now() >= deadline {
            log(format!("coordinators {remaining:?} still active after {timeout:?}; releasing the workspace anyway"));
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum StopOutcome {
    /// The service accepted, drained, and its port stopped answering.
    Stopped,
    /// No endpoint is published for this workspace.
    NotPublished,
    /// An endpoint is published but nothing (or another instance) answers
    /// on it — left behind by a service that did not exit through `stop`.
    Stale(String),
}

/// Stop the published service and wait until it has gone.
pub fn stop(data_dir: &Path, wait: Duration) -> Result<StopOutcome, ServiceError> {
    let Some((manifest, token)) = control_api::read_endpoint_files(data_dir)? else {
        return Ok(StopOutcome::NotPublished);
    };
    let client = ControlClient::new(manifest.port, token);
    let info = match client.info() {
        Ok(info) => info,
        Err(ClientError::Io(error)) => {
            return Ok(StopOutcome::Stale(format!("nothing answers on port {}: {error}", manifest.port)))
        }
        Err(error) => return Err(error.into()),
    };
    if info["instanceId"] != manifest.instance_id || info["workspaceId"] != manifest.workspace_id {
        return Ok(StopOutcome::Stale(format!(
            "port {} answers for another instance ({} / {})",
            manifest.port, info["workspaceId"], info["instanceId"]
        )));
    }
    client.shutdown()?;
    let deadline = Instant::now() + wait;
    loop {
        if matches!(client.info(), Err(ClientError::Io(_))) && !control_api::manifest_path(data_dir).exists() {
            return Ok(StopOutcome::Stopped);
        }
        if Instant::now() >= deadline {
            return Err(ServiceError::Other(format!("the service accepted the shutdown but is still running after {wait:?}")));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// The published endpoint (`manifest`) and what answers on it (`live`, or
/// `null` with `reason` when nothing does). `None` when nothing is published.
pub fn status(data_dir: &Path) -> Result<Option<Value>, ServiceError> {
    let Some((manifest, token)) = control_api::read_endpoint_files(data_dir)? else {
        return Ok(None);
    };
    let live = ControlClient::new(manifest.port, token).info();
    let mut status = json!({ "manifest": manifest, "live": Value::Null });
    match live {
        Ok(info) => status["live"] = info,
        Err(error) => status["reason"] = json!(error.to_string()),
    }
    Ok(Some(status))
}

/// The binary's entry point: parse, run, print, and return the exit code.
pub fn main(args: Vec<String>) -> i32 {
    let cli = match parse_args(&args) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    let outcome: Result<i32, ServiceError> = match cli {
        Cli::Help => {
            println!("{USAGE}");
            Ok(EXIT_OK)
        }
        Cli::Run { data_dir } => resolve_data_dir(data_dir).and_then(|dir| run(&dir, RunOptions::default()).map(|()| EXIT_OK)),
        Cli::Stop { data_dir } => resolve_data_dir(data_dir).and_then(|dir| match stop(&dir, STOP_TIMEOUT)? {
            StopOutcome::Stopped => {
                println!("service stopped");
                Ok(EXIT_OK)
            }
            StopOutcome::NotPublished => {
                println!("no service is published for {}", dir.display());
                Ok(EXIT_NO_SERVICE)
            }
            StopOutcome::Stale(reason) => {
                println!("stale endpoint in {}: {reason}", dir.display());
                Ok(EXIT_NO_SERVICE)
            }
        }),
        Cli::Status { data_dir } => resolve_data_dir(data_dir).and_then(|dir| match status(&dir)? {
            Some(status) => {
                println!("{}", serde_json::to_string_pretty(&status).unwrap_or_default());
                Ok(if status["live"].is_null() { EXIT_NO_SERVICE } else { EXIT_OK })
            }
            None => {
                println!("no service is published for {}", dir.display());
                Ok(EXIT_NO_SERVICE)
            }
        }),
    };
    match outcome {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            error.exit_code()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::runtime::lease;

    // ---------- arguments and paths ----------

    fn parse(args: &[&str]) -> Result<Cli, String> {
        parse_args(args)
    }

    #[test]
    fn arguments_select_the_subcommand_and_the_data_dir() {
        assert_eq!(parse(&[]).unwrap(), Cli::Run { data_dir: None });
        assert_eq!(parse(&["run"]).unwrap(), Cli::Run { data_dir: None });
        assert_eq!(parse(&["run", "--data-dir", "x"]).unwrap(), Cli::Run { data_dir: Some("x".into()) });
        assert_eq!(parse(&["--data-dir=x", "stop"]).unwrap(), Cli::Stop { data_dir: Some("x".into()) });
        assert_eq!(parse(&["status"]).unwrap(), Cli::Status { data_dir: None });
        assert_eq!(parse(&["--help"]).unwrap(), Cli::Help);
        assert_eq!(parse(&["run", "-h"]).unwrap(), Cli::Help);
        assert!(parse(&["--data-dir"]).unwrap_err().contains("needs a directory"));
        assert!(parse(&["serve"]).unwrap_err().contains("unexpected"));
        assert!(parse(&["run", "stop"]).unwrap_err().contains("unexpected"), "one subcommand");
    }

    // The identifier itself is pinned to the desktop's config in
    // `runtime::boundary_tests` (this file must not name that config).
    #[test]
    fn the_default_data_dir_is_the_platform_data_dir_under_the_identifier() {
        let dir = default_data_dir().expect("a platform data dir");
        assert!(dir.ends_with(APP_IDENTIFIER), "{}", dir.display());
        #[cfg(windows)]
        assert_eq!(
            dir,
            PathBuf::from(std::env::var("APPDATA").unwrap()).join(APP_IDENTIFIER),
            "the CI smoke lane looks under %APPDATA%"
        );
    }

    // ---------- lifecycle, in-process ----------

    fn fresh_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("aff-service-test-{}-{n}", std::process::id()))
    }

    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            if self.0.exists() {
                std::fs::remove_dir_all(&self.0)
                    .unwrap_or_else(|error| panic!("temp dir {} not removed: {error}", self.0.display()));
            }
        }
    }

    const TEST_TIMEOUT: Duration = Duration::from_secs(20);

    fn wait_for_manifest(dir: &Path) -> (EndpointManifest, ControlToken) {
        let deadline = Instant::now() + TEST_TIMEOUT;
        loop {
            if let Ok(Some(published)) = control_api::read_endpoint_files(dir) {
                return published;
            }
            assert!(Instant::now() < deadline, "no endpoint published in {}", dir.display());
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn fast_options() -> RunOptions {
        RunOptions { heartbeat_period: Duration::from_millis(50), drain_timeout: Duration::from_secs(5) }
    }

    #[test]
    fn run_publishes_an_endpoint_serves_it_and_stop_withdraws_it_and_releases_the_lock() {
        let dir = fresh_dir();
        let _guard = TempDir(dir.clone());
        let service = {
            let dir = dir.clone();
            thread::spawn(move || run(&dir, fast_options()))
        };
        let (manifest, token) = wait_for_manifest(&dir);
        assert_eq!(manifest.holder_kind, "service");
        assert_eq!(manifest.epoch, 1);
        assert_eq!(manifest.pid, std::process::id());
        assert_eq!(manifest.service_version, SERVICE_VERSION);
        assert!(dir.join(db::DB_FILE_NAME).is_file(), "the workspace was created and migrated");

        let client = ControlClient::new(manifest.port, token);
        let info = client.info().unwrap();
        assert_eq!(info["instanceId"], manifest.instance_id);
        assert_eq!(info["workspaceId"], manifest.workspace_id);

        // 雙啟: a second service on the same workspace is refused before it
        // touches anything, with the exit code the operator sees.
        let refused = run(&dir, fast_options()).unwrap_err();
        assert!(matches!(refused, ServiceError::NotOwner(_)), "{refused}");
        assert_eq!(refused.exit_code(), EXIT_NOT_OWNER);
        assert_eq!(status(&dir).unwrap().unwrap()["live"]["pid"], std::process::id(), "status sees the live one");

        assert_eq!(stop(&dir, TEST_TIMEOUT).unwrap(), StopOutcome::Stopped);
        service.join().unwrap().expect("run returns Ok after a stop");
        assert_eq!(control_api::read_endpoint_files(&dir).unwrap(), None, "endpoint withdrawn");
        let lock = lease::try_lock_workspace(&dir).expect("lock released");
        drop(lock);
        assert_eq!(stop(&dir, TEST_TIMEOUT).unwrap(), StopOutcome::NotPublished);
        assert_eq!(status(&dir).unwrap(), None);
    }

    #[test]
    fn a_manifest_nobody_answers_is_reported_stale_not_stopped() {
        let dir = fresh_dir();
        let _guard = TempDir(dir.clone());
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let dead_port = listener.local_addr().unwrap().port();
        drop(listener);
        let manifest = EndpointManifest {
            manifest_version: MANIFEST_VERSION.into(),
            port: dead_port,
            workspace_id: "w".repeat(32),
            epoch: 1,
            holder_kind: "service".into(),
            instance_id: "i".repeat(32),
            pid: 1,
            service_version: SERVICE_VERSION.into(),
            started_at: "2026-09-17T00:00:00Z".into(),
        };
        control_api::write_endpoint_files(&dir, &manifest, &ControlToken::generate().unwrap()).unwrap();
        let outcome = stop(&dir, Duration::from_secs(1)).unwrap();
        assert!(matches!(outcome, StopOutcome::Stale(ref reason) if reason.contains("nothing answers")), "{outcome:?}");
        let status = status(&dir).unwrap().unwrap();
        assert!(status["live"].is_null());
        assert!(status["reason"].as_str().unwrap().contains("connection"));
        assert!(control_api::read_endpoint_files(&dir).unwrap().is_some(), "stop does not delete what it did not verify");
    }

    #[test]
    fn a_newer_database_stops_the_service_before_it_publishes_anything() {
        let dir = fresh_dir();
        let _guard = TempDir(dir.clone());
        {
            let conn = db::open_at(&dir.join(db::DB_FILE_NAME)).unwrap();
            conn.execute("INSERT INTO schema_migrations (version) VALUES ('0099_from_the_future')", []).unwrap();
        }
        let error = run(&dir, fast_options()).unwrap_err();
        assert!(matches!(error, ServiceError::SchemaTooNew(_)), "{error}");
        assert_eq!(error.exit_code(), EXIT_SCHEMA_TOO_NEW);
        assert_eq!(control_api::read_endpoint_files(&dir).unwrap(), None);
    }
}
