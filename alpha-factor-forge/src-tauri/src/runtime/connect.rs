//! P04b — the desktop's connect mode (contract §1.1 `desktop-connect`).
//!
//! When another host owns the workspace and has published a control
//! endpoint (P04a), the desktop does not own anything: it verifies the
//! endpoint, opens the database WITHOUT migrating (repository reads and the
//! user's own saves only; never runner writes, migration, or recovery),
//! proxies every discovery command as a `research-command-v1` envelope, and
//! follows the service's event ledger from a cursor, re-emitting each row
//! to the window under the channel name it already knows
//! (`discovery://progress|result|done`). That is the reconnect the contract
//! fixes (§3): snapshot first — the window's own `get_active_discovery_run`
//! — then the cursor.
//!
//! Nothing here names the desktop framework; the window-side sink is a
//! trait the desktop implements in `desktop::discovery_events`.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::commands::{CommandError, COMMAND_PROTOCOL_VERSION};
use super::control_api::{self, random_hex, EndpointManifest};
use super::control_client::{ClientError, ControlClient};
use super::SharedDb;
use crate::db;
use crate::error::{AppError, AppResult};

/// How long one forwarder poll waits for the ledger to grow. Short enough
/// that `stop` never waits long for a parked poll to come back.
pub const FORWARD_POLL: Duration = Duration::from_secs(5);
/// Pause between reconnection attempts once the service stopped answering.
pub const RECONNECT_PAUSE: Duration = Duration::from_secs(1);

/// Where forwarded ledger events go. The desktop posts them on its event bus;
/// tests record them. Every method is a fact the window must act on, so
/// none has a default: a sink that ignores one is a bug the compiler sees.
pub trait LedgerEventSink: Send + Sync {
    /// One ledger row: its channel name and the payload it carried, verbatim.
    fn emit(&self, channel: &str, payload: &Value) -> Result<(), String>;
    /// The service stopped answering; the forwarder is rediscovering it.
    fn connection_lost(&self, reason: &str);
    /// The service (the same, or a restarted one for this workspace) answers
    /// again after `connection_lost`; the window re-reads its snapshot.
    fn connection_restored(&self);
    /// The window's view may be behind the database (contract §3): a ledger
    /// gap appeared, the state version moved without a row, or a row could
    /// not be handed over. The window re-reads its run's snapshot.
    fn resnapshot_needed(&self, reason: &str, state_version: i64);
}

#[derive(Debug)]
pub enum ConnectError {
    /// The workspace is owned but no endpoint is published (an owner that is
    /// not a service, or a service that has not finished starting).
    NotPublished,
    /// An endpoint is published but nothing answers on it.
    Unreachable(String),
    /// The endpoint answers for another instance or workspace.
    IdentityMismatch(String),
    /// The database cannot be used by this build without migrating
    /// (`SchemaTooNew` / `SchemaPending`).
    Schema(AppError),
    Other(String),
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectError::NotPublished => f.write_str("the workspace is owned by another host that publishes no control endpoint"),
            ConnectError::Unreachable(reason) => write!(f, "the published control endpoint does not answer: {reason}"),
            ConnectError::IdentityMismatch(reason) => write!(f, "the published control endpoint is stale: {reason}"),
            ConnectError::Schema(error) => write!(f, "cannot use the owner's database: {error}"),
            ConnectError::Other(reason) => f.write_str(reason),
        }
    }
}

impl From<AppError> for ConnectError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::SchemaTooNew(_) | AppError::SchemaPending(_) => ConnectError::Schema(error),
            other => ConnectError::Other(other.to_string()),
        }
    }
}

/// The service as the desktop's commands see it: cheap to clone, so a
/// command copies it out of the host-mode lock and makes its request
/// without holding that lock across the round trip.
#[derive(Clone)]
pub struct ServiceProxy {
    pub manifest: EndpointManifest,
    pub client: ControlClient,
}

/// A verified connection to the workspace's owner. The proxy is behind a
/// lock because the forwarder replaces it when it rediscovers a restarted
/// service (M1); commands take a clone through `proxy()`.
pub struct ConnectedHost {
    proxy: Arc<Mutex<ServiceProxy>>,
    /// The workspace database, opened without migrating.
    pub db: SharedDb,
    data_dir: PathBuf,
    forwarder: Mutex<Option<EventForwarder>>,
}

/// Read the published endpoint and prove it is this workspace's live
/// service: the manifest's instance answers on the port, for the workspace
/// the manifest names (and, on a reconnect, the workspace we were connected
/// to), speaking this build's protocol, over a database this build can use
/// without migrating. Shared by the first connect and every rediscovery.
pub fn verify_endpoint(data_dir: &Path, expected_workspace: Option<&str>) -> Result<ServiceProxy, ConnectError> {
    let (manifest, token) = control_api::read_endpoint_files(data_dir)
        .map_err(|error| ConnectError::Other(format!("cannot read the control endpoint: {error}")))?
        .ok_or(ConnectError::NotPublished)?;
    if let Some(expected) = expected_workspace {
        if manifest.workspace_id != expected {
            return Err(ConnectError::IdentityMismatch(format!(
                "the published endpoint is for workspace {}, not {expected}",
                manifest.workspace_id
            )));
        }
    }
    let client = ControlClient::new(manifest.port, token);
    let info = match client.info() {
        Ok(info) => info,
        Err(ClientError::Io(error)) => return Err(ConnectError::Unreachable(error.to_string())),
        Err(error) => return Err(ConnectError::Other(error.to_string())),
    };
    if info["instanceId"] != manifest.instance_id || info["workspaceId"] != manifest.workspace_id {
        return Err(ConnectError::IdentityMismatch(format!(
            "port {} answers for {} / {}, manifest names {} / {}",
            manifest.port, info["workspaceId"], info["instanceId"], manifest.workspace_id, manifest.instance_id
        )));
    }
    if info["commandProtocolVersion"] != COMMAND_PROTOCOL_VERSION {
        return Err(ConnectError::Other(format!(
            "the service speaks {} and this build speaks {COMMAND_PROTOCOL_VERSION}",
            info["commandProtocolVersion"]
        )));
    }
    // The schema check is repeated on every (re)discovery: a restarted
    // service may be another build.
    drop(db::open_migrated(&data_dir.join(db::DB_FILE_NAME))?);
    Ok(ServiceProxy { manifest, client })
}

/// Verify the published endpoint and open the database. Does not start the
/// forwarder; `follow_events` does, once the host has a sink.
pub fn connect(data_dir: &Path) -> Result<ConnectedHost, ConnectError> {
    let proxy = verify_endpoint(data_dir, None)?;
    let conn = db::open_migrated(&data_dir.join(db::DB_FILE_NAME))?;
    Ok(ConnectedHost {
        proxy: Arc::new(Mutex::new(proxy)),
        db: Arc::new(Mutex::new(conn)),
        data_dir: data_dir.to_path_buf(),
        forwarder: Mutex::new(None),
    })
}

impl ConnectedHost {
    /// The current proxy (the service last verified).
    pub fn proxy(&self) -> ServiceProxy {
        self.proxy.lock().unwrap_or_else(|error| error.into_inner()).clone()
    }

    /// Start forwarding ledger rows after `after` to `sink`. The cursor a
    /// host passes is the ledger's current end at connect time: history
    /// before it belongs to the snapshot the window takes itself.
    pub fn follow_events(&self, sink: Arc<dyn LedgerEventSink>, after: i64) -> std::io::Result<()> {
        let forwarder = EventForwarder::spawn(self.proxy.clone(), self.data_dir.clone(), sink, after)?;
        if let Ok(mut slot) = self.forwarder.lock() {
            if let Some(previous) = slot.replace(forwarder) {
                previous.stop();
            }
        }
        Ok(())
    }

    /// True while a forwarder is attached.
    pub fn is_following(&self) -> bool {
        self.forwarder.lock().map(|slot| slot.is_some()).unwrap_or(false)
    }

    /// The ledger's current end, for `follow_events`.
    pub fn ledger_end(&self) -> AppResult<i64> {
        self.proxy().ledger_end()
    }

    /// Stop forwarding (the service is being stopped or the desktop is
    /// taking the workspace back).
    pub fn stop_following(&self) {
        if let Ok(mut slot) = self.forwarder.lock() {
            if let Some(forwarder) = slot.take() {
                forwarder.stop();
            }
        }
    }
}

impl ServiceProxy {
    pub fn workspace_id(&self) -> &str {
        &self.manifest.workspace_id
    }

    /// The ledger's current end.
    pub fn ledger_end(&self) -> AppResult<i64> {
        let page = self.client.events(i64::MAX, Some(1), Duration::ZERO).map_err(unreachable)?;
        Ok(page["lastEventId"].as_i64().unwrap_or(0))
    }

    // ---- proxied discovery commands (the desktop's existing invoke API) ----

    pub fn start(&self, config: Value) -> AppResult<i64> {
        let result = self.call("discovery.start", config)?;
        result["runId"]
            .as_i64()
            .ok_or_else(|| AppError::Other(format!("discovery.start answered without a runId: {result}")))
    }

    pub fn pause(&self, run_id: i64) -> AppResult<()> {
        self.call("discovery.pause", json!({ "runId": run_id })).map(|_| ())
    }

    pub fn resume(&self, run_id: i64) -> AppResult<()> {
        self.call("discovery.resume", json!({ "runId": run_id })).map(|_| ())
    }

    pub fn cancel(&self, run_id: i64) -> AppResult<()> {
        self.call("discovery.cancel", json!({ "runId": run_id })).map(|_| ())
    }

    /// The `discovery-progress-v1` snapshot, as JSON. The state version the
    /// service attaches stays in the bridge: the window's snapshot shape is
    /// the mode-agnostic one, and the bridge's forwarder is what compares
    /// versions (contract §3) and tells the window to re-read.
    pub fn progress(&self, run_id: i64) -> AppResult<Value> {
        self.call("discovery.progress", json!({ "runId": run_id })).map(|result| result["run"].clone())
    }

    /// The one non-terminal run's snapshot, or `null`.
    pub fn active(&self) -> AppResult<Value> {
        self.call("discovery.active", json!({})).map(|result| result["run"].clone())
    }

    /// A caller-built envelope, passed through untouched (its `requestId`
    /// is the caller's idempotency key). Transport failures are `Busy`.
    pub fn dispatch(&self, envelope: Value) -> Result<Value, CommandError> {
        match self.client.dispatch(&envelope) {
            Ok(outcome) => outcome,
            Err(error) => Err(CommandError::busy(format!("background service unreachable: {error}"))),
        }
    }

    /// `/v1/info`, for the desktop's workspace info.
    pub fn info(&self) -> AppResult<Value> {
        self.client.info().map_err(unreachable)
    }

    fn call(&self, command: &str, payload: Value) -> AppResult<Value> {
        let envelope = json!({
            "protocolVersion": COMMAND_PROTOCOL_VERSION,
            "workspaceId": self.manifest.workspace_id,
            "requestId": format!("desktop-{}", random_hex(8)?),
            "command": command,
            "payload": payload,
        });
        match self.client.dispatch(&envelope) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => Err(AppError::Other(format!("{:?}: {}", error.code, error.message))),
            Err(error) => Err(unreachable(error)),
        }
    }
}

fn unreachable(error: ClientError) -> AppError {
    AppError::Other(format!("background service unreachable: {error}"))
}

/// A thread that long-polls the ledger and hands every row to the sink.
pub struct EventForwarder {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EventForwarder {
    /// `proxy` is shared with the host so a rediscovered service replaces
    /// the one commands use; `data_dir` is where the endpoint is republished.
    pub fn spawn(
        proxy: Arc<Mutex<ServiceProxy>>,
        data_dir: PathBuf,
        sink: Arc<dyn LedgerEventSink>,
        after: i64,
    ) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = stop.clone();
            thread::Builder::new()
                .name("ledger-forwarder".into())
                .spawn(move || forward_loop(proxy, data_dir, sink, after, stop))?
        };
        Ok(Self { stop, thread: Some(thread) })
    }

    /// Ask the loop to end and wait for it (at most one poll's wait).
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for EventForwarder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// A ledger gap marker as the page carries it.
fn gap_marker(page: &Value) -> Option<(i64, i64)> {
    let gap = page.get("ledgerGap")?;
    Some((gap["epoch"].as_i64()?, gap["stateVersion"].as_i64()?))
}

/// Rows are forwarded in ledger order and the cursor only moves forward, so
/// a row is never emitted twice. The ledger is durable across service
/// restarts (event ids are never reused), so the cursor survives a
/// reconnect. What the rows cannot carry, the page's version does
/// (contract §3): a NEW gap marker, a state version that moved with no row
/// to show for it, or a row the window could not be handed all mean the
/// window's view may be behind the database, and it is told to re-read —
/// once per such fact, never repeatedly for a marker it has already been
/// told about. Losing the service (any failure to read the ledger) starts
/// rediscovery: the endpoint files are re-read and re-verified for THIS
/// workspace until a service answers, then the host's proxy is replaced.
fn forward_loop(
    proxy: Arc<Mutex<ServiceProxy>>,
    data_dir: PathBuf,
    sink: Arc<dyn LedgerEventSink>,
    after: i64,
    stop: Arc<AtomicBool>,
) {
    let snapshot_proxy = || proxy.lock().unwrap_or_else(|error| error.into_inner()).clone();
    let mut current = snapshot_proxy();
    let workspace_id = current.manifest.workspace_id.clone();
    let mut cursor = after;
    // Facts the window is assumed to have: the version and the gap marker of
    // the first page (its own snapshot is taken around now and covers them).
    let mut known_version: Option<i64> = None;
    let mut known_gap: Option<(i64, i64)> = None;
    let mut primed = false;
    let mut lost = false;
    while !stop.load(Ordering::SeqCst) {
        match current.client.events(cursor, None, FORWARD_POLL) {
            Ok(page) => {
                let version = page["stateVersion"].as_i64().unwrap_or(0);
                let gap = gap_marker(&page);
                let mut resnapshot: Option<String> = None;
                let mut forwarded = 0usize;
                for event in page["events"].as_array().into_iter().flatten() {
                    let Some(event_id) = event["eventId"].as_i64() else { continue };
                    if event_id <= cursor {
                        continue;
                    }
                    let channel = event["channel"].as_str().unwrap_or("");
                    if let Err(error) = sink.emit(channel, &event["payload"]) {
                        // The row is not re-emitted (the failure is the
                        // window's, not the ledger's) but it is not lost
                        // silently either: the window is told to re-read.
                        eprintln!("ledger forwarder: emit of event {event_id} failed: {error}");
                        resnapshot.get_or_insert_with(|| format!("event {event_id} could not be delivered: {error}"));
                    }
                    cursor = event_id;
                    forwarded += 1;
                }
                if primed {
                    if gap.is_some() && gap != known_gap {
                        let (epoch, at) = gap.unwrap_or_default();
                        resnapshot.get_or_insert_with(|| format!("the ledger has a gap (epoch {epoch}, state version {at})"));
                    }
                    if forwarded == 0 && known_version.is_some_and(|known| version > known) {
                        resnapshot.get_or_insert_with(|| format!("the workspace changed (state version {version}) without a ledger event"));
                    }
                }
                primed = true;
                known_version = Some(known_version.map_or(version, |known| known.max(version)));
                if gap.is_some() {
                    known_gap = gap;
                }
                if let Some(reason) = resnapshot {
                    sink.resnapshot_needed(&reason, version);
                }
            }
            Err(error) => {
                if !lost {
                    lost = true;
                    sink.connection_lost(&error.to_string());
                }
                // Rediscover: the same service back, or a restarted one for
                // this workspace. Anything else (another workspace's endpoint
                // copied here, a build that cannot use the database) is
                // refused and the search continues.
                match verify_endpoint(&data_dir, Some(&workspace_id)) {
                    Ok(found) => {
                        *proxy.lock().unwrap_or_else(|error| error.into_inner()) = found.clone();
                        current = found;
                        lost = false;
                        sink.connection_restored();
                        // A restarted service ran startup recovery (which
                        // writes no ledger row): the window re-reads.
                        sink.resnapshot_needed("reconnected to the workspace's service", known_version.unwrap_or(0));
                        continue;
                    }
                    Err(reason) => eprintln!("ledger forwarder: still disconnected: {reason}"),
                }
                // Sleep in small steps so `stop` is honoured promptly.
                let until = Instant::now() + RECONNECT_PAUSE;
                while Instant::now() < until && !stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
}
