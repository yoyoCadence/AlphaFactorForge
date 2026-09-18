//! P04b — which host the desktop is, and how it changes (plan §3.1:
//! "keep embedded mode when nobody owns the workspace; when a service does,
//! the desktop only proxies; entering background mode stops taking new
//! work, checkpoints, releases ownership, then starts the service — never
//! two writers").
//!
//! `HostMode` is the desktop's one piece of host state: `Embedded` holds the
//! P03a `Workspace` (lock, epoch, heartbeat, runner); `Connected` holds the
//! verified P04b `ConnectedHost`; `Switching` is the window during a
//! hand-over in which neither is available. `open_or_connect` picks the
//! starting mode; `hand_over_to_service` and `take_back_from_service` move
//! between the two, and both leave the desktop in a usable mode on failure
//! wherever that is possible.
//!
//! `Admission` is the desktop's equivalent of the service's shutdown lock:
//! an embedded mutating command holds a guard for its whole execution, and
//! a hand-over closes admission and waits for the guards to drain before it
//! asks coordinators to checkpoint — so a `start` that was already past its
//! mode check cannot create a coordinator after the drain scanned.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::connect::{self, ConnectError, ConnectedHost, LedgerEventSink};
use super::service::{self, StopOutcome};
use super::{open_workspace, SharedDb, Workspace};
use crate::db::ownership::HolderKind;
use crate::db::{self};
use crate::error::{AppError, AppResult};

/// How long a hand-over waits for embedded coordinators to checkpoint.
pub const HAND_OVER_DRAIN_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a hand-over waits for the launched service to publish and answer.
pub const SERVICE_START_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a take-back retries the OS lock after the service reported stopped
/// (it releases the lock a moment after its port closes).
pub const RELOCK_TIMEOUT: Duration = Duration::from_secs(10);
/// The service's log when the desktop launches it, beside the database.
pub const SERVICE_LOG_FILE_NAME: &str = "service.log";

pub const DESKTOP_EMBEDDED: &str = "desktop-embedded";
pub const DESKTOP_CONNECT: &str = "desktop-connect";
pub const SWITCHING: &str = "switching";

pub enum HostMode {
    Embedded(Workspace),
    Connected(ConnectedHost),
    /// A hand-over in progress, or one that failed and left nothing usable;
    /// the string says which.
    Switching(String),
}

impl HostMode {
    pub fn kind(&self) -> &'static str {
        match self {
            HostMode::Embedded(_) => DESKTOP_EMBEDDED,
            HostMode::Connected(_) => DESKTOP_CONNECT,
            HostMode::Switching(_) => SWITCHING,
        }
    }

    /// The database for repository commands, in either steady mode.
    pub fn db(&self) -> AppResult<SharedDb> {
        match self {
            HostMode::Embedded(workspace) => Ok(workspace.db.clone()),
            HostMode::Connected(connected) => Ok(connected.db.clone()),
            HostMode::Switching(reason) => Err(AppError::Other(format!("the workspace is not available: {reason}"))),
        }
    }
}

/// Counts embedded mutating commands in flight and lets a hand-over close
/// the door and wait for them.
#[derive(Default)]
pub struct Admission {
    state: Mutex<AdmissionState>,
    changed: Condvar,
}

#[derive(Default)]
struct AdmissionState {
    closed: bool,
    active: usize,
}

pub struct AdmissionGuard<'a>(&'a Admission);

impl Admission {
    /// A guard for one mutating command, or `Busy` while a hand-over runs.
    pub fn admit(&self) -> AppResult<AdmissionGuard<'_>> {
        let mut state = self.state.lock().map_err(|_| AppError::Other("admission lock poisoned".into()))?;
        if state.closed {
            return Err(AppError::Other("busy: the desktop is handing the workspace over; retry in a moment".into()));
        }
        state.active += 1;
        Ok(AdmissionGuard(self))
    }

    /// Refuse new commands and wait for the active ones; false on timeout
    /// (admission stays closed; the caller decides).
    pub fn close_and_drain(&self, timeout: Duration) -> bool {
        let Ok(mut state) = self.state.lock() else { return false };
        state.closed = true;
        let deadline = Instant::now() + timeout;
        while state.active > 0 {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            state = match self.changed.wait_timeout(state, deadline - now) {
                Ok((state, _)) => state,
                Err(_) => return false,
            };
        }
        true
    }

    pub fn reopen(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = false;
        }
        self.changed.notify_all();
    }
}

impl Drop for AdmissionGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.state.lock() {
            state.active = state.active.saturating_sub(1);
        }
        self.0.changed.notify_all();
    }
}

#[derive(Debug)]
pub enum HostError {
    /// Startup: the workspace is owned and cannot be connected to.
    CannotConnect(ConnectError),
    /// Startup: opening the workspace failed for a reason other than ownership.
    CannotOpen(AppError),
    /// A hand-over or take-back failed; the desktop is in the mode named.
    SwitchFailed { reason: String, now: &'static str },
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostError::CannotConnect(error) => write!(f, "another host owns the workspace and {error}"),
            HostError::CannotOpen(error) => write!(f, "cannot open the workspace: {error}"),
            HostError::SwitchFailed { reason, now } => write!(f, "{reason} (the desktop is now {now})"),
        }
    }
}

/// The desktop's starting mode: own the workspace if nobody does; connect
/// to the owner if it is a service that answers; otherwise fail with why.
pub fn open_or_connect(data_dir: &Path) -> Result<HostMode, HostError> {
    let db_path = data_dir.join(db::DB_FILE_NAME);
    match open_workspace(&db_path, HolderKind::DesktopEmbedded) {
        Ok(workspace) => Ok(HostMode::Embedded(workspace)),
        Err(AppError::NotOwner(_)) => connect::connect(data_dir)
            .map(HostMode::Connected)
            .map_err(HostError::CannotConnect),
        Err(error) => Err(HostError::CannotOpen(error)),
    }
}

/// Starts a service for a workspace. The desktop spawns the binary beside
/// its own; tests run the service in-process.
pub trait ServiceLauncher: Send + Sync {
    fn launch(&self, data_dir: &Path) -> io::Result<()>;
}

/// The service binary's expected location: next to the executable that
/// asks (both are built by the same package into the same directory).
pub fn service_executable_beside(current_exe: &Path) -> PathBuf {
    let name = format!("{}{}", service::BINARY_NAME, std::env::consts::EXE_SUFFIX);
    current_exe.parent().map(|dir| dir.join(&name)).unwrap_or_else(|| PathBuf::from(name))
}

/// Spawns `<exe> run --data-dir <dir>` detached from the desktop, with its
/// output in `<dir>/service.log`, so it survives the desktop's exit.
pub struct ExecutableLauncher {
    pub exe: PathBuf,
}

impl ServiceLauncher for ExecutableLauncher {
    fn launch(&self, data_dir: &Path) -> io::Result<()> {
        if !self.exe.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("service binary not found at {}", self.exe.display()),
            ));
        }
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(data_dir.join(SERVICE_LOG_FILE_NAME))?;
        let log_err = log.try_clone()?;
        let mut command = Command::new(&self.exe);
        command
            .arg("run")
            .arg("--data-dir")
            .arg(data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: no console of ours,
            // and a Ctrl+C in ours never reaches it.
            command.creation_flags(0x0000_0008 | 0x0000_0200);
        }
        let child = command.spawn()?;
        // Not waited for: the service outlives this process by design.
        drop(child);
        Ok(())
    }
}

/// Plan §3.1 background mode. `slot` holds the desktop's mode; on entry it
/// must be `Embedded`. Order: close admission and drain in-flight commands →
/// ask every coordinator to checkpoint and wait → release ownership → launch
/// the service → wait for its endpoint and connect → follow its ledger.
/// On failure after the release the desktop re-owns the workspace when it
/// can, so the user is back in embedded mode rather than nowhere.
pub fn hand_over_to_service(
    slot: &Mutex<HostMode>,
    admission: &Admission,
    launcher: &dyn ServiceLauncher,
    sink: Arc<dyn LedgerEventSink>,
    data_dir: &Path,
) -> Result<(), HostError> {
    // 1. No new embedded mutations; wait for the ones running.
    if !admission.close_and_drain(HAND_OVER_DRAIN_TIMEOUT) {
        admission.reopen();
        return Err(HostError::SwitchFailed {
            reason: "commands are still running after the drain timeout".into(),
            now: DESKTOP_EMBEDDED,
        });
    }
    // 2. Take the workspace out of the slot.
    let workspace = {
        let mut mode = slot.lock().map_err(|_| poisoned())?;
        match std::mem::replace(&mut *mode, HostMode::Switching("handing over to the background service".into())) {
            HostMode::Embedded(workspace) => workspace,
            other => {
                let now = other.kind();
                *mode = other;
                admission.reopen();
                return Err(HostError::SwitchFailed { reason: "not in embedded mode".into(), now });
            }
        }
    };
    // 3. Checkpoint every coordinator under THIS epoch before the release.
    if !drain_coordinators(&workspace, HAND_OVER_DRAIN_TIMEOUT) {
        restore(slot, HostMode::Embedded(workspace));
        admission.reopen();
        return Err(HostError::SwitchFailed {
            reason: "a discovery run did not reach its checkpoint in time; still embedded".into(),
            now: DESKTOP_EMBEDDED,
        });
    }
    // 4. Release: the lock and heartbeat go with the workspace.
    drop(workspace);
    // 5. Launch, wait, connect.
    let connected = launcher
        .launch(data_dir)
        .map_err(|error| format!("cannot launch the service: {error}"))
        .and_then(|()| wait_for_service(data_dir, SERVICE_START_TIMEOUT));
    match connected {
        Ok(connected) => {
            let after = connected.ledger_end().unwrap_or(0);
            if let Err(error) = connected.follow_events(sink, after) {
                eprintln!("host: cannot follow the service's events: {error}");
            }
            restore(slot, HostMode::Connected(connected));
            admission.reopen();
            Ok(())
        }
        Err(reason) => {
            // Back to embedded if the lock is free (the service never took it
            // or already died); otherwise report where we are.
            let now = match relock(data_dir, RELOCK_TIMEOUT) {
                Ok(workspace) => {
                    restore(slot, HostMode::Embedded(workspace));
                    DESKTOP_EMBEDDED
                }
                Err(error) => {
                    restore(slot, HostMode::Switching(format!("{reason}; and re-owning failed: {error}")));
                    SWITCHING
                }
            };
            admission.reopen();
            Err(HostError::SwitchFailed { reason, now })
        }
    }
}

/// The reverse: stop the service (it drains to a checkpoint), take the
/// lock back, and be embedded again. On entry `slot` must be `Connected`.
pub fn take_back_from_service(slot: &Mutex<HostMode>, admission: &Admission, data_dir: &Path) -> Result<(), HostError> {
    let connected = {
        let mut mode = slot.lock().map_err(|_| poisoned())?;
        match std::mem::replace(&mut *mode, HostMode::Switching("taking the workspace back from the service".into())) {
            HostMode::Connected(connected) => connected,
            other => {
                let now = other.kind();
                *mode = other;
                return Err(HostError::SwitchFailed { reason: "not connected to a service".into(), now });
            }
        }
    };
    match service::stop(data_dir, service::STOP_TIMEOUT) {
        Ok(StopOutcome::Stopped) | Ok(StopOutcome::NotPublished) | Ok(StopOutcome::Stale(_)) => {}
        Err(error) => {
            // The service is still there: stay connected, with the forwarder
            // that was following it left exactly as it was (H1: a failed
            // take-back must not cost the window its events).
            restore(slot, HostMode::Connected(connected));
            return Err(HostError::SwitchFailed { reason: format!("the service did not stop: {error}"), now: DESKTOP_CONNECT });
        }
    }
    // The service is gone (its shutdown woke the forwarder's poll, so
    // stopping the forwarder is quick) and nothing will republish here
    // until we own the workspace ourselves.
    connected.stop_following();
    drop(connected);
    match relock(data_dir, RELOCK_TIMEOUT) {
        Ok(workspace) => {
            restore(slot, HostMode::Embedded(workspace));
            admission.reopen();
            Ok(())
        }
        Err(error) => {
            restore(slot, HostMode::Switching(format!("the service stopped but the workspace could not be re-owned: {error}")));
            Err(HostError::SwitchFailed { reason: error.to_string(), now: SWITCHING })
        }
    }
}

fn poisoned() -> HostError {
    HostError::SwitchFailed { reason: "host mode lock poisoned".into(), now: SWITCHING }
}

fn restore(slot: &Mutex<HostMode>, mode: HostMode) {
    if let Ok(mut current) = slot.lock() {
        *current = mode;
    }
}

/// Ask every live coordinator to checkpoint (PauseRequested, without
/// waiting on a worker) and wait for all of them to exit.
fn drain_coordinators(workspace: &Workspace, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = workspace.discovery.active_coordinator_run_ids().unwrap_or_default();
        if remaining.is_empty() {
            return true;
        }
        for run_id in remaining {
            if let Err(error) = workspace.discovery.request_pause_for_shutdown(run_id) {
                eprintln!("host: run {run_id} not paused: {error}");
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Poll until the launched service publishes an endpoint that answers for
/// itself, or give up.
fn wait_for_service(data_dir: &Path, timeout: Duration) -> Result<ConnectedHost, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let last = match connect::connect(data_dir) {
            Ok(connected) => return Ok(connected),
            Err(ConnectError::Schema(error)) => return Err(format!("the service's workspace cannot be used: {error}")),
            Err(error) => error.to_string(),
        };
        if Instant::now() >= deadline {
            return Err(format!("the service did not come up within {timeout:?}: {last}"));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Own the workspace again, retrying `NotOwner` briefly (the previous owner
/// releases the OS lock a moment after it stops answering).
fn relock(data_dir: &Path, timeout: Duration) -> AppResult<Workspace> {
    let db_path = data_dir.join(db::DB_FILE_NAME);
    let deadline = Instant::now() + timeout;
    loop {
        match open_workspace(&db_path, HolderKind::DesktopEmbedded) {
            Ok(workspace) => return Ok(workspace),
            Err(AppError::NotOwner(message)) if Instant::now() < deadline => {
                let _ = message;
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error),
        }
    }
}
