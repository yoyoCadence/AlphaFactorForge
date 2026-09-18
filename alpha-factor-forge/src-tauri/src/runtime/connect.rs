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
use std::path::Path;
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
/// tests record them.
pub trait LedgerEventSink: Send + Sync {
    /// One ledger row: its channel name and the payload it carried, verbatim.
    fn emit(&self, channel: &str, payload: &Value) -> Result<(), String>;
    /// The service stopped answering; the forwarder keeps retrying.
    fn connection_lost(&self, _reason: &str) {}
    /// The service answers again after `connection_lost`.
    fn connection_restored(&self) {}
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

/// A verified connection to the workspace's owner.
pub struct ConnectedHost {
    pub proxy: ServiceProxy,
    /// The workspace database, opened without migrating.
    pub db: SharedDb,
    forwarder: Mutex<Option<EventForwarder>>,
}

/// Verify the published endpoint and open the database. Does not start the
/// forwarder; `follow_events` does, once the host has a sink.
pub fn connect(data_dir: &Path) -> Result<ConnectedHost, ConnectError> {
    let (manifest, token) = control_api::read_endpoint_files(data_dir)
        .map_err(|error| ConnectError::Other(format!("cannot read the control endpoint: {error}")))?
        .ok_or(ConnectError::NotPublished)?;
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
    let conn = db::open_migrated(&data_dir.join(db::DB_FILE_NAME))?;
    Ok(ConnectedHost {
        proxy: ServiceProxy { manifest, client },
        db: Arc::new(Mutex::new(conn)),
        forwarder: Mutex::new(None),
    })
}

impl ConnectedHost {
    /// Start forwarding ledger rows after `after` to `sink`. The cursor a
    /// host passes is the ledger's current end at connect time: history
    /// before it belongs to the snapshot the window takes itself.
    pub fn follow_events(&self, sink: Arc<dyn LedgerEventSink>, after: i64) -> std::io::Result<()> {
        let forwarder = EventForwarder::spawn(self.proxy.client.clone(), sink, after)?;
        if let Ok(mut slot) = self.forwarder.lock() {
            if let Some(previous) = slot.replace(forwarder) {
                previous.stop();
            }
        }
        Ok(())
    }

    /// The ledger's current end, for `follow_events`.
    pub fn ledger_end(&self) -> AppResult<i64> {
        self.proxy.ledger_end()
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

    /// The `discovery-progress-v1` snapshot, as JSON.
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
    pub fn spawn(client: ControlClient, sink: Arc<dyn LedgerEventSink>, after: i64) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = stop.clone();
            thread::Builder::new()
                .name("ledger-forwarder".into())
                .spawn(move || forward_loop(client, sink, after, stop))?
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

/// Rows are forwarded in ledger order and the cursor only moves forward, so
/// a row is never emitted twice. A page whose end moved past the rows it
/// carried is not a gap: the next poll starts after the last row emitted,
/// and rows the ledger could not store (`ledgerGap`) are the window's to
/// discover through its snapshot (`stateVersion`), as in embedded mode.
fn forward_loop(client: ControlClient, sink: Arc<dyn LedgerEventSink>, after: i64, stop: Arc<AtomicBool>) {
    let mut cursor = after;
    let mut lost = false;
    while !stop.load(Ordering::SeqCst) {
        match client.events(cursor, None, FORWARD_POLL) {
            Ok(page) => {
                if lost {
                    lost = false;
                    sink.connection_restored();
                }
                for event in page["events"].as_array().into_iter().flatten() {
                    let Some(event_id) = event["eventId"].as_i64() else { continue };
                    if event_id <= cursor {
                        continue;
                    }
                    let channel = event["channel"].as_str().unwrap_or("");
                    if let Err(error) = sink.emit(channel, &event["payload"]) {
                        eprintln!("ledger forwarder: emit of event {event_id} failed: {error}");
                    }
                    cursor = event_id;
                }
            }
            Err(error) => {
                let io = matches!(error, ClientError::Io(_));
                if io && !lost {
                    lost = true;
                    sink.connection_lost(&error.to_string());
                } else if !io {
                    eprintln!("ledger forwarder: {error}");
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
