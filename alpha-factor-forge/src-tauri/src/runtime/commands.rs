//! P03b — the versioned command and event envelopes
//! (docs/research-runtime-contract.md §2 `research-command-v1`,
//! §3 `research-event-v1`) and the host-agnostic dispatcher behind them.
//!
//! Every cross-host entry point (the desktop bridge today, the loopback API
//! and MCP adapter in P04/P16) hands a `CommandEnvelope` to `Dispatcher::
//! dispatch` and gets back either a JSON result or a structured
//! `CommandError`. The dispatcher owns the rules the contract fixes:
//!
//! - the protocol version must match exactly; no lenient acceptance;
//! - the envelope must name THIS workspace;
//! - the command must be on the whitelist (`<domain>.<verb>`);
//! - a mutating command is idempotent by `requestId`: reserved before it runs,
//!   completed after, replayed from the ledger on repeat, refused as
//!   `DuplicateRequest` when the id is reused for something else;
//! - read commands are not ledgered (they have no side effect to repeat).
//!
//! Events reach the ledger through `LedgerSink`, which appends each
//! post-commit runner event to `runtime_events` under the owner's epoch and
//! then forwards it to the host's own sink. The persistent `eventId` is the
//! reconnect cursor: snapshot first (`discovery.active` / `discovery.
//! progress`), then `events.read` with `afterEventId`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::discovery::{self, OutcomeStage};
use crate::db::ownership;
use crate::db::runtime_ledger::{self, Reservation, StoredEvent};
use crate::discovery_runner::{
    DiscoveryEvent, DiscoveryEventSink, DiscoveryRunner, DISCOVERY_DONE_EVENT,
    DISCOVERY_EVENT_VERSION, DISCOVERY_PROGRESS_EVENT, DISCOVERY_RESULT_EVENT,
};
use crate::error::AppError;

use super::SharedDb;

/// Test-only scheduling hook (H1 regression): runs once, on the dispatching
/// thread, right after a mutating envelope's reservation and before its
/// outcome is decided. Never compiled into the application.
#[cfg(test)]
pub(crate) mod test_hooks {
    thread_local! {
        pub static AFTER_RESERVATION: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
            std::cell::RefCell::new(None);
    }

    pub fn after_reservation() {
        AFTER_RESERVATION.with(|slot| {
            let callback = slot.borrow_mut().take();
            if let Some(callback) = callback {
                callback();
            }
        });
    }
}

pub const COMMAND_PROTOCOL_VERSION: &str = "research-command-v1";
pub const EVENT_PROTOCOL_VERSION: &str = "research-event-v1";

/// The most events one `events.read` returns; the caller pages by cursor.
pub const MAX_EVENT_PAGE: usize = 500;

// ---------- envelopes ----------

/// `research-command-v1`. Unknown fields are rejected: an envelope is a
/// contract, not a bag.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandEnvelope {
    pub protocol_version: String,
    pub workspace_id: String,
    pub request_id: String,
    pub command: String,
    #[serde(default)]
    pub payload: Value,
}

/// Contract §2 error codes. Serialized as the exact strings the contract lists.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum ErrorCode {
    UnsupportedProtocol,
    WorkspaceMismatch,
    Unauthorized,
    NotOwner,
    StaleOwner,
    DuplicateRequest,
    Validation,
    NotFound,
    Busy,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl CommandError {
    fn new(code: ErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self { code, message: message.into(), retryable }
    }

    /// A transient host-side failure (a task that could not be joined, a
    /// poisoned lock): retryable, and never mistaken for a contract error.
    pub fn busy(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Busy, message, true)
    }
}

/// Map a runtime error onto the contract's vocabulary. Ownership errors keep
/// their code; everything else is classified by what the caller can do about
/// it, and `Validation` (not retryable) is the honest default.
impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::NotOwner(message) => CommandError::new(ErrorCode::NotOwner, message, false),
            AppError::StaleOwner(message) => CommandError::new(ErrorCode::StaleOwner, message, false),
            other => {
                let message = other.to_string();
                let lower = message.to_ascii_lowercase();
                if lower.contains("not found") || lower.contains("no such") {
                    CommandError::new(ErrorCode::NotFound, message, false)
                } else if lower.contains("already") || lower.contains("lock poisoned") || lower.contains("busy") {
                    CommandError::new(ErrorCode::Busy, message, true)
                } else {
                    CommandError::new(ErrorCode::Validation, message, false)
                }
            }
        }
    }
}

/// `research-event-v1`: one ledger row as published to a reconnecting reader.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope {
    pub protocol_version: &'static str,
    pub event_id: i64,
    pub epoch: i64,
    pub entity: EventEntity,
    pub event_version: String,
    pub channel: String,
    pub committed_at: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventEntity {
    pub kind: String,
    pub id: String,
}

impl EventEnvelope {
    pub fn from_stored(stored: StoredEvent) -> Result<Self, CommandError> {
        let payload = serde_json::from_str(&stored.payload_json).map_err(|error| {
            CommandError::new(
                ErrorCode::Validation,
                format!("ledger event {} holds unreadable JSON: {error}", stored.event_id),
                false,
            )
        })?;
        Ok(Self {
            protocol_version: EVENT_PROTOCOL_VERSION,
            event_id: stored.event_id,
            epoch: stored.epoch,
            entity: EventEntity { kind: stored.entity_kind, id: stored.entity_id },
            event_version: stored.event_version,
            channel: stored.channel,
            committed_at: stored.committed_at,
            payload,
        })
    }
}

// ---------- the command whitelist ----------

/// Every command the dispatcher accepts. Adding one means adding it here,
/// deciding whether it mutates, and handling it in `execute`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    DiscoveryStart,
    DiscoveryPause,
    DiscoveryResume,
    DiscoveryCancel,
    DiscoveryProgress,
    DiscoveryActive,
    EventsRead,
    OwnershipRead,
}

impl Command {
    pub const ALL: &'static [Command] = &[
        Command::DiscoveryStart,
        Command::DiscoveryPause,
        Command::DiscoveryResume,
        Command::DiscoveryCancel,
        Command::DiscoveryProgress,
        Command::DiscoveryActive,
        Command::EventsRead,
        Command::OwnershipRead,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Command::DiscoveryStart => "discovery.start",
            Command::DiscoveryPause => "discovery.pause",
            Command::DiscoveryResume => "discovery.resume",
            Command::DiscoveryCancel => "discovery.cancel",
            Command::DiscoveryProgress => "discovery.progress",
            Command::DiscoveryActive => "discovery.active",
            Command::EventsRead => "events.read",
            Command::OwnershipRead => "ownership.read",
        }
    }

    pub fn parse(name: &str) -> Option<Command> {
        Command::ALL.iter().copied().find(|command| command.name() == name)
    }

    /// Mutating commands are idempotent by request id; reads are not ledgered.
    pub fn mutates(self) -> bool {
        matches!(
            self,
            Command::DiscoveryStart | Command::DiscoveryPause | Command::DiscoveryResume | Command::DiscoveryCancel
        )
    }
}

// ---------- the dispatcher ----------

/// Per-request-id exclusion for THIS process (H1). A mutating envelope holds
/// its request id's claim for the WHOLE flow — reserve, read the receipt and
/// outcome, execute or replay, complete — so two envelopes with the same id
/// never interleave: the second waits for the first to finish and then reads
/// the persistent state the first left behind (a recorded outcome to replay),
/// instead of judging from a reservation it took before the first ran.
/// A shared set plus separate begin/end calls was tried and rejected: any
/// gap between "reserved" and "claimed" let a duplicate run in between.
#[derive(Default)]
pub struct InFlightRequests {
    held: Mutex<HashSet<String>>,
    released: Condvar,
}

/// Holds one request id's claim; dropping it releases the claim and wakes
/// waiters.
pub struct RequestClaim<'a> {
    registry: &'a InFlightRequests,
    request_id: String,
}

impl InFlightRequests {
    /// Block until `request_id` is free, then claim it.
    pub fn claim(&self, request_id: &str) -> RequestClaim<'_> {
        let mut held = self.held.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        while held.contains(request_id) {
            held = self.released.wait(held).unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        held.insert(request_id.to_owned());
        RequestClaim { registry: self, request_id: request_id.to_owned() }
    }
}

impl Drop for RequestClaim<'_> {
    fn drop(&mut self) {
        if let Ok(mut held) = self.registry.held.lock() {
            held.remove(&self.request_id);
        }
        self.registry.released.notify_all();
    }
}

/// Everything a host needs to serve envelopes for one owned workspace. The
/// sink is the host's; the dispatcher wraps it in a `LedgerSink` so every
/// runner event is ledgered before the host sees it. `in_flight` must be
/// shared by every dispatcher a host builds for the same workspace.
pub struct Dispatcher {
    db: SharedDb,
    discovery: DiscoveryRunner,
    ledger: Arc<LedgerSink>,
    sink: Arc<dyn DiscoveryEventSink>,
    epoch: i64,
    workspace_id: String,
    in_flight: Arc<InFlightRequests>,
}

impl Dispatcher {
    pub fn new(
        db: SharedDb,
        discovery: DiscoveryRunner,
        epoch: i64,
        workspace_id: String,
        host_sink: Arc<dyn DiscoveryEventSink>,
        in_flight: Arc<InFlightRequests>,
    ) -> Self {
        let ledger = Arc::new(LedgerSink::new(db.clone(), epoch, host_sink));
        let sink: Arc<dyn DiscoveryEventSink> = ledger.clone();
        Self { db, discovery, ledger, sink, epoch, workspace_id, in_flight }
    }

    /// Parse, check, and run one envelope. Never panics on caller input.
    pub fn dispatch(&self, raw: Value) -> Result<Value, CommandError> {
        let envelope: CommandEnvelope = serde_json::from_value(raw).map_err(|error| {
            CommandError::new(ErrorCode::Validation, format!("malformed command envelope: {error}"), false)
        })?;
        if envelope.protocol_version != COMMAND_PROTOCOL_VERSION {
            return Err(CommandError::new(
                ErrorCode::UnsupportedProtocol,
                format!(
                    "protocol {} is not supported; this host speaks {COMMAND_PROTOCOL_VERSION}",
                    envelope.protocol_version
                ),
                false,
            ));
        }
        if envelope.workspace_id != self.workspace_id {
            return Err(CommandError::new(
                ErrorCode::WorkspaceMismatch,
                format!("envelope names workspace {}, this host owns {}", envelope.workspace_id, self.workspace_id),
                false,
            ));
        }
        if envelope.request_id.trim().is_empty() || envelope.request_id.len() > 128 {
            return Err(CommandError::new(ErrorCode::Validation, "requestId must be 1–128 characters", false));
        }
        let command = Command::parse(&envelope.command).ok_or_else(|| {
            CommandError::new(ErrorCode::Validation, format!("unknown command {:?}", envelope.command), false)
        })?;

        if !command.mutates() {
            return self.execute(command, &envelope.payload, None);
        }

        // H1: the whole idempotent flow runs under this request id's claim.
        // A duplicate envelope waits here and, once let in, sees whatever the
        // first attempt persisted — it never acts on a reservation it took
        // before the first attempt finished.
        let _claim = self.in_flight.claim(&envelope.request_id);

        // Idempotency: reserve first, so a retry can never start a second
        // piece of work whatever happens after this point.
        let payload_hash = payload_hash(&envelope.payload);
        let reservation = {
            let conn = self.lock_db()?;
            runtime_ledger::reserve_request(
                &conn,
                Some(self.epoch),
                &envelope.request_id,
                &self.workspace_id,
                command.name(),
                &payload_hash,
            )?
        };
        #[cfg(test)]
        test_hooks::after_reservation();
        match reservation {
            Reservation::Fresh => self.execute_and_record(command, &envelope),
            Reservation::Replay(stored) if stored.status == "pending" => self.settle_pending(command, &envelope, stored),
            Reservation::Replay(stored) => replay(stored),
            Reservation::Conflict(stored) => Err(CommandError::new(
                ErrorCode::DuplicateRequest,
                format!(
                    "requestId {} was already used for {} with a different payload",
                    stored.request_id, stored.command
                ),
                false,
            )),
        }
    }

    /// A `pending` receipt seen again, under the request's claim (so no
    /// attempt is executing in this process). In order (R1):
    /// 1. an immutable outcome row exists — the command's first answer was
    ///    decided and recorded with the change that decided it: replay THAT
    ///    (accepted → the result, rejected → the error, begun → "never
    ///    completed its admission"), repair the receipt, execute nothing;
    /// 2. none — the first attempt died before it changed anything: this
    ///    is the first execution.
    fn settle_pending(
        &self,
        command: Command,
        envelope: &CommandEnvelope,
        _stored: runtime_ledger::StoredRequest,
    ) -> Result<Value, CommandError> {
        let recorded = {
            let conn = self.lock_db()?;
            discovery::read_request_outcome(&conn, &envelope.request_id)?
        };
        match recorded {
            Some(row) => {
                let outcome = outcome_from_row(&row)?;
                self.record_receipt(&envelope.request_id, &outcome);
                outcome
            }
            None => self.execute_and_record(command, envelope),
        }
    }

    fn execute_and_record(&self, command: Command, envelope: &CommandEnvelope) -> Result<Value, CommandError> {
        // R3: an error that is about to be RECORDED is this request's final
        // answer. Retrying the same id can only replay it, so it must not be
        // labelled retryable; the caller starts a new request.
        let outcome = self
            .execute(command, &envelope.payload, Some(&envelope.request_id))
            .map_err(final_error);
        let receipt_recorded = self.record_receipt(&envelope.request_id, &outcome);
        if receipt_recorded {
            return outcome;
        }
        // The receipt could not be written. A success, and every failure the
        // runner recorded with its own transaction, is still replayable from
        // the outcome row. A failure that changed nothing has NO durable
        // trace now, so it is not final: say so, and let the same request id
        // be tried again (that retry is its first execution).
        match outcome {
            Ok(result) => Ok(result),
            Err(error) => {
                let durable = {
                    let conn = self.lock_db()?;
                    // A `begun` row is a fact about the run, not a recorded answer;
                    // only an accepted/rejected stage replays THIS answer.
                    discovery::read_request_outcome(&conn, &envelope.request_id)?
                        .is_some_and(|row| row.stage != OutcomeStage::Begun)
                };
                if durable {
                    Err(error)
                } else {
                    Err(CommandError::new(
                        error.code,
                        format!("{} (this outcome could not be recorded; the same requestId may be retried)", error.message),
                        true,
                    ))
                }
            }
        }
    }

    /// Write the receipt; returns whether it was recorded. A failure here is
    /// reported, never hidden, and never turns a durable answer into a
    /// different one (see `settle_pending` / `execute_and_record`).
    fn record_receipt(&self, request_id: &str, outcome: &Result<Value, CommandError>) -> bool {
        let recorded = self
            .db
            .lock()
            .map_err(|_| AppError::Other("db lock poisoned".into()))
            .and_then(|conn| match outcome {
                Ok(result) => runtime_ledger::complete_request(&conn, Some(self.epoch), request_id, Ok(result)),
                Err(error) => {
                    let error_json = serde_json::to_value(error)?;
                    runtime_ledger::complete_request(&conn, Some(self.epoch), request_id, Err(&error_json))
                }
            });
        match recorded {
            Ok(()) => true,
            Err(error) => {
                eprintln!("command {request_id} completed but its receipt was not recorded: {error}");
                false
            }
        }
    }

    fn execute(&self, command: Command, payload: &Value, request_id: Option<&str>) -> Result<Value, CommandError> {
        match command {
            Command::DiscoveryStart => {
                let run_id = self.discovery.start_for_request(self.db.clone(), self.sink.clone(), payload.clone(), request_id)?;
                Ok(json!({ "runId": run_id }))
            }
            Command::DiscoveryPause => {
                let run_id = run_id_of(payload)?;
                self.discovery.pause_for_request(&self.db, run_id, request_id)?;
                Ok(json!({ "runId": run_id }))
            }
            Command::DiscoveryResume => {
                let run_id = run_id_of(payload)?;
                self.discovery.resume_for_request(self.db.clone(), self.sink.clone(), run_id, request_id)?;
                Ok(json!({ "runId": run_id }))
            }
            Command::DiscoveryCancel => {
                let run_id = run_id_of(payload)?;
                self.discovery.cancel_for_request(&self.db, self.sink.clone(), run_id, request_id)?;
                Ok(json!({ "runId": run_id }))
            }
            Command::DiscoveryProgress => {
                let run_id = run_id_of(payload)?;
                // Version BEFORE the snapshot: the snapshot is then "at least
                // this version", which is the safe side for gap detection.
                let (version, snapshot) = {
                    let conn = self.lock_db()?;
                    (ownership::state_version(&conn)?, self.discovery.progress_on(&conn, run_id)?)
                };
                Ok(json!({ "run": snapshot, "stateVersion": version }))
            }
            Command::DiscoveryActive => {
                let (version, snapshot) = {
                    let conn = self.lock_db()?;
                    (ownership::state_version(&conn)?, self.discovery.active_progress_on(&conn)?)
                };
                Ok(json!({ "run": snapshot, "stateVersion": version }))
            }
            Command::EventsRead => {
                let after = payload.get("afterEventId").and_then(Value::as_i64).unwrap_or(0);
                if after < 0 {
                    return Err(CommandError::new(ErrorCode::Validation, "afterEventId must be >= 0", false));
                }
                let limit = payload
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map(|n| (n as usize).clamp(1, MAX_EVENT_PAGE))
                    .unwrap_or(MAX_EVENT_PAGE);
                // Events first, version AFTER: if the state moved past the last
                // ledgered event, `stateVersion` says so and the reader must
                // re-snapshot (R2).
                let (events, last, version, gap) = {
                    let conn = self.lock_db()?;
                    (
                        runtime_ledger::read_events_after(&conn, after, limit)?,
                        runtime_ledger::last_event_id(&conn)?,
                        ownership::state_version(&conn)?,
                        runtime_ledger::read_ledger_gap(&conn)?,
                    )
                };
                let envelopes = events.into_iter().map(EventEnvelope::from_stored).collect::<Result<Vec<_>, _>>()?;
                Ok(json!({
                    "events": envelopes,
                    "lastEventId": last,
                    "stateVersion": version,
                    "ledgerGap": gap,
                    "ledgerDegraded": self.ledger.degraded(),
                }))
            }
            Command::OwnershipRead => {
                let row = {
                    let conn = self.lock_db()?;
                    ownership::read(&conn)?
                };
                Ok(json!({ "ownership": row, "workspaceId": self.workspace_id }))
            }
        }
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, CommandError> {
        self.db
            .lock()
            .map_err(|_| CommandError::new(ErrorCode::Busy, "db lock poisoned", true))
    }
}

/// The first answer, rebuilt from the immutable outcome row the change left
/// behind. Never inferred from the run's CURRENT status, which keeps moving
/// after the command answered (a worker can fail a run that a `start` was
/// rightly told had been accepted).
fn outcome_from_row(row: &discovery::RequestOutcomeRow) -> Result<Result<Value, CommandError>, CommandError> {
    let parsed: Option<Value> = match &row.outcome_json {
        Some(json) => Some(serde_json::from_str(json).map_err(|error| {
            CommandError::new(ErrorCode::Validation, format!("stored outcome unreadable: {error}"), false)
        })?),
        None => None,
    };
    Ok(match row.stage {
        OutcomeStage::Accepted => Ok(parsed.unwrap_or(Value::Null)),
        OutcomeStage::Rejected => {
            let message = parsed
                .as_ref()
                .and_then(|value| value.get("error"))
                .and_then(Value::as_str)
                .unwrap_or("the command was rejected")
                .to_owned();
            // The same mapping the first answer went through, so the replay
            // is that answer, code and all.
            Err(final_error(CommandError::from(AppError::Other(message))))
        }
        OutcomeStage::Begun => Err(final_error(CommandError::new(
            ErrorCode::Validation,
            format!(
                "{} for requestId {} began on run {} but never completed its admission (the first attempt failed or was interrupted before an outcome was recorded); inspect run {} and send a new requestId",
                row.command, row.request_id, row.run_id, row.run_id
            ),
            false,
        ))),
    })
}

/// Retryable with the same request id: the request is reserved but its
/// receipt is in a state this build cannot interpret.
fn pending_error(request_id: &str, why: &str) -> CommandError {
    CommandError::new(
        ErrorCode::Busy,
        format!("requestId {request_id} is pending: {why}; retry the same requestId later"),
        true,
    )
}

/// R3: a recorded failure is final for its request id, whatever caused it.
fn final_error(mut error: CommandError) -> CommandError {
    if error.retryable {
        error.retryable = false;
        error.message.push_str("; this requestId is now final — send a new requestId to try again");
    }
    error
}

/// Answer a repeated request from its stored outcome, executing nothing.
fn replay(stored: runtime_ledger::StoredRequest) -> Result<Value, CommandError> {
    match stored.status.as_str() {
        "succeeded" => {
            let json = stored.result_json.unwrap_or_else(|| "null".into());
            serde_json::from_str(&json).map_err(|error| {
                CommandError::new(ErrorCode::Validation, format!("stored result unreadable: {error}"), false)
            })
        }
        "failed" => {
            let json = stored.error_json.unwrap_or_else(|| "{}".into());
            let error: CommandError = serde_json::from_str(&json).map_err(|error| {
                CommandError::new(ErrorCode::Validation, format!("stored error unreadable: {error}"), false)
            })?;
            Err(error)
        }
        other => Err(pending_error(&stored.request_id, &format!("unexpected receipt status {other:?}"))),
    }
}

fn run_id_of(payload: &Value) -> Result<i64, CommandError> {
    payload
        .get("runId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| CommandError::new(ErrorCode::Validation, "payload.runId must be a positive integer", false))
}

/// SHA-256 of the payload's canonical JSON, so "same request" means the same
/// bytes the caller sent, not a lossy summary.
pub fn payload_hash(payload: &Value) -> String {
    use sha2::{Digest, Sha256};
    let canonical = canonical_json(payload);
    hex::encode(Sha256::digest(canonical.as_bytes()))
}

/// JSON with object keys sorted recursively; arrays keep their order.
fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .into_iter()
                .map(|key| format!("{}:{}", serde_json::to_string(key).unwrap_or_default(), canonical_json(&map[key])))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(canonical_json).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

// ---------- the ledger sink ----------

/// Appends every runner event to `runtime_events` under the owner's epoch,
/// then forwards it to the host. The append happens AFTER the runner's own
/// commit (commit-then-emit is unchanged). If the append fails the event is
/// still forwarded, the failure is reported, a durable gap marker is written
/// (best effort) and this sink is flagged degraded — and, independently of
/// all three, the state version moved with the domain write, so a cursor
/// reader sees "state changed, no event" and re-snapshots (R2). The
/// database state is the truth; a reconnecting reader never misses a
/// committed result.
pub struct LedgerSink {
    db: SharedDb,
    epoch: i64,
    inner: Arc<dyn DiscoveryEventSink>,
    degraded: AtomicBool,
}

impl LedgerSink {
    pub fn new(db: SharedDb, epoch: i64, inner: Arc<dyn DiscoveryEventSink>) -> Self {
        Self { db, epoch, inner, degraded: AtomicBool::new(false) }
    }

    /// True once an append has failed on this sink.
    pub fn degraded(&self) -> bool {
        self.degraded.load(Ordering::SeqCst)
    }

    fn append(&self, event: &DiscoveryEvent) -> Result<i64, String> {
        let (channel, run_id, payload) = match event {
            DiscoveryEvent::Progress(p) => (DISCOVERY_PROGRESS_EVENT, p.run_id, serde_json::to_string(p)),
            DiscoveryEvent::Result(r) => (DISCOVERY_RESULT_EVENT, r.run_id, serde_json::to_string(r)),
            DiscoveryEvent::Done(d) => (DISCOVERY_DONE_EVENT, d.run_id, serde_json::to_string(d)),
        };
        let payload = payload.map_err(|error| error.to_string())?;
        let conn = self.db.lock().map_err(|_| "db lock poisoned".to_string())?;
        runtime_ledger::append_event(
            &conn,
            self.epoch,
            "discovery_run",
            &run_id.to_string(),
            DISCOVERY_EVENT_VERSION,
            channel,
            &payload,
        )
        .map_err(|error| error.to_string())
    }

    fn mark_gap(&self) {
        self.degraded.store(true, Ordering::SeqCst);
        let marked = self
            .db
            .lock()
            .map_err(|_| AppError::Other("db lock poisoned".into()))
            .and_then(|conn| {
                let version = ownership::state_version(&conn)?;
                runtime_ledger::record_ledger_gap(&conn, self.epoch, version)
            });
        if let Err(error) = marked {
            eprintln!("event ledger gap marker not written either: {error}");
        }
    }
}

impl DiscoveryEventSink for LedgerSink {
    fn emit(&self, event: &DiscoveryEvent) -> Result<(), String> {
        if let Err(error) = self.append(event) {
            eprintln!("event ledger append failed (event still forwarded; readers must re-snapshot): {error}");
            self.mark_gap();
        }
        self.inner.emit(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ownership::{acquire, HolderKind};
    use std::sync::Mutex;

    struct CountingSink(Mutex<Vec<String>>);

    impl DiscoveryEventSink for CountingSink {
        fn emit(&self, event: &DiscoveryEvent) -> Result<(), String> {
            let label = match event {
                DiscoveryEvent::Progress(_) => "progress",
                DiscoveryEvent::Result(_) => "result",
                DiscoveryEvent::Done(_) => "done",
            };
            self.0.lock().unwrap().push(label.into());
            Ok(())
        }
    }

    fn dispatcher() -> (Dispatcher, String, Arc<CountingSink>) {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::apply_migrations(&conn).unwrap();
        let epoch = acquire(&mut conn, HolderKind::DesktopEmbedded, 1).unwrap().epoch;
        let workspace = runtime_ledger::workspace_id(&conn).unwrap();
        let db: SharedDb = Arc::new(Mutex::new(conn));
        let host = Arc::new(CountingSink(Mutex::new(Vec::new())));
        let dispatcher = Dispatcher::new(
            db,
            DiscoveryRunner::with_epoch(epoch),
            epoch,
            workspace.clone(),
            host.clone(),
            Arc::new(InFlightRequests::default()),
        );
        (dispatcher, workspace, host)
    }

    fn envelope(workspace: &str, request: &str, command: &str, payload: Value) -> Value {
        json!({
            "protocolVersion": COMMAND_PROTOCOL_VERSION,
            "workspaceId": workspace,
            "requestId": request,
            "command": command,
            "payload": payload,
        })
    }

    #[test]
    fn the_envelope_is_checked_before_anything_runs() {
        let (dispatcher, ws, _) = dispatcher();

        let wrong_version = dispatcher.dispatch(json!({
            "protocolVersion": "research-command-v2", "workspaceId": ws, "requestId": "r", "command": "discovery.active"
        }));
        assert_eq!(wrong_version.unwrap_err().code, ErrorCode::UnsupportedProtocol);

        let wrong_workspace = dispatcher.dispatch(envelope("someone-else", "r", "discovery.active", json!({})));
        assert_eq!(wrong_workspace.unwrap_err().code, ErrorCode::WorkspaceMismatch);

        let unknown = dispatcher.dispatch(envelope(&ws, "r", "shell.exec", json!({ "cmd": "rm" })));
        let error = unknown.unwrap_err();
        assert_eq!(error.code, ErrorCode::Validation);
        assert!(error.message.contains("unknown command"));

        let extra_field = dispatcher.dispatch(json!({
            "protocolVersion": COMMAND_PROTOCOL_VERSION, "workspaceId": ws, "requestId": "r",
            "command": "discovery.active", "payload": {}, "token": "x"
        }));
        assert_eq!(extra_field.unwrap_err().code, ErrorCode::Validation);

        let empty_request = dispatcher.dispatch(envelope(&ws, "   ", "discovery.active", json!({})));
        assert_eq!(empty_request.unwrap_err().code, ErrorCode::Validation);

        // Nothing above touched the request ledger.
        let conn = dispatcher.db.lock().unwrap();
        let rows: i64 = conn.query_row("SELECT COUNT(*) FROM command_requests", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn reads_are_served_without_a_ledger_row() {
        let (dispatcher, ws, _) = dispatcher();
        let active = dispatcher.dispatch(envelope(&ws, "r1", "discovery.active", json!({}))).unwrap();
        assert_eq!(active["run"], Value::Null, "no run yet");
        assert_eq!(active["stateVersion"], 0, "nothing has been written to a run yet");
        let ownership = dispatcher.dispatch(envelope(&ws, "r2", "ownership.read", json!({}))).unwrap();
        assert_eq!(ownership["workspaceId"], ws);
        assert_eq!(ownership["ownership"]["epoch"], 1);
        let events = dispatcher.dispatch(envelope(&ws, "r3", "events.read", json!({}))).unwrap();
        assert_eq!(events["events"].as_array().unwrap().len(), 0);
        assert_eq!(events["lastEventId"], 0);
        assert_eq!(events["stateVersion"], 0);
        assert_eq!(events["ledgerGap"], Value::Null);
        assert_eq!(events["ledgerDegraded"], false);
        let conn = dispatcher.db.lock().unwrap();
        let rows: i64 = conn.query_row("SELECT COUNT(*) FROM command_requests", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0, "reads are not idempotency-tracked");
    }

    #[test]
    fn a_repeated_mutating_request_replays_its_first_outcome_and_a_reused_id_is_refused() {
        let (dispatcher, ws, _) = dispatcher();
        // A cancel of a run that does not exist: a real, recorded failure.
        let first = dispatcher.dispatch(envelope(&ws, "req-cancel", "discovery.cancel", json!({ "runId": 99 })));
        let first_error = first.unwrap_err();
        assert_eq!(first_error.code, ErrorCode::NotFound, "{first_error:?}");
        assert!(!first_error.retryable, "a recorded failure is final for its request id (R3)");

        let again = dispatcher.dispatch(envelope(&ws, "req-cancel", "discovery.cancel", json!({ "runId": 99 })));
        assert_eq!(again.unwrap_err(), first_error, "the stored outcome, verbatim");

        let reused = dispatcher.dispatch(envelope(&ws, "req-cancel", "discovery.cancel", json!({ "runId": 100 })));
        assert_eq!(reused.unwrap_err().code, ErrorCode::DuplicateRequest);
        let reused_command = dispatcher.dispatch(envelope(&ws, "req-cancel", "discovery.pause", json!({ "runId": 99 })));
        assert_eq!(reused_command.unwrap_err().code, ErrorCode::DuplicateRequest);

        let conn = dispatcher.db.lock().unwrap();
        let stored = runtime_ledger::read_request(&conn, "req-cancel").unwrap().unwrap();
        assert_eq!(stored.status, "failed");
        assert_eq!(stored.command, "discovery.cancel");
        let rows: i64 = conn.query_row("SELECT COUNT(*) FROM command_requests", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 1, "one row for one request id, however often it is sent");
    }

    #[test]
    fn a_request_that_never_completed_is_reported_as_pending_not_rerun() {
        let (dispatcher, ws, _) = dispatcher();
        {
            let conn = dispatcher.db.lock().unwrap();
            runtime_ledger::reserve_request(
                &conn,
                Some(dispatcher.epoch),
                "req-lost",
                &ws,
                "discovery.pause",
                &payload_hash(&json!({ "runId": 1 })),
            )
            .unwrap();
        }
        // No effect row and no attempt in flight: the first attempt died before
        // it changed anything, so this IS the first execution — it runs (and
        // here fails, because run 1 does not exist) and is recorded as final.
        let retry = dispatcher.dispatch(envelope(&ws, "req-lost", "discovery.pause", json!({ "runId": 1 })));
        let error = retry.unwrap_err();
        assert_eq!(error.code, ErrorCode::Validation, "{error:?}");
        assert!(!error.retryable, "recorded, therefore final");
        let conn = dispatcher.db.lock().unwrap();
        assert_eq!(runtime_ledger::read_request(&conn, "req-lost").unwrap().unwrap().status, "failed");
        drop(conn);

    }

    /// H1: a duplicate envelope does not run beside its first attempt — it
    /// waits for the claim, then replays what the first attempt recorded.
    #[test]
    fn a_duplicate_waits_for_the_claim_and_replays_instead_of_executing() {
        let (dispatcher, ws, _) = dispatcher();
        let registry = dispatcher.in_flight.clone();
        let claim = registry.claim("req-held");
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let result = dispatcher.dispatch(envelope(&ws, "req-held", "discovery.cancel", json!({ "runId": 99 })));
                let _ = done_tx.send(result);
            });
            assert!(
                done_rx.recv_timeout(std::time::Duration::from_millis(200)).is_err(),
                "the duplicate must wait while the claim is held"
            );
            // Meanwhile "the first attempt" records its outcome as a receipt.
            {
                let conn = dispatcher.db.lock().unwrap();
                runtime_ledger::reserve_request(&conn, Some(dispatcher.epoch), "req-held", &ws, "discovery.cancel", &payload_hash(&json!({ "runId": 99 }))).unwrap();
                runtime_ledger::complete_request(&conn, Some(dispatcher.epoch), "req-held", Ok(&json!({ "runId": 99 }))).unwrap();
            }
            drop(claim);
            let result = done_rx.recv_timeout(std::time::Duration::from_secs(5)).expect("released");
            assert_eq!(result.unwrap(), json!({ "runId": 99 }), "replayed the first attempt's receipt, executed nothing");
        });
    }

    #[test]
    fn the_ledger_sink_appends_then_forwards_and_events_read_pages_by_cursor() {
        let (dispatcher, ws, host) = dispatcher();
        let event = |sequence: u64| {
            DiscoveryEvent::Progress(crate::discovery_runner::DiscoveryProgressEvent {
                event_version: DISCOVERY_EVENT_VERSION,
                sequence,
                run_id: 5,
                status: crate::db::discovery::RunStatus::Running,
                counts: Default::default(),
                candidate: None,
                best_strategy_id: None,
            })
        };
        for sequence in 1..=3 {
            dispatcher.sink.emit(&event(sequence)).unwrap();
        }
        assert_eq!(host.0.lock().unwrap().len(), 3, "forwarded to the host");

        let page = dispatcher.dispatch(envelope(&ws, "r", "events.read", json!({ "afterEventId": 1, "limit": 1 }))).unwrap();
        let events = page["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["protocolVersion"], EVENT_PROTOCOL_VERSION);
        assert_eq!(events[0]["eventId"], 2);
        assert_eq!(events[0]["epoch"], 1);
        assert_eq!(events[0]["entity"]["kind"], "discovery_run");
        assert_eq!(events[0]["entity"]["id"], "5");
        assert_eq!(events[0]["eventVersion"], DISCOVERY_EVENT_VERSION);
        assert_eq!(events[0]["channel"], DISCOVERY_PROGRESS_EVENT);
        assert_eq!(events[0]["payload"]["sequence"], 2);
        assert_eq!(page["lastEventId"], 3);

        let rest = dispatcher.dispatch(envelope(&ws, "r", "events.read", json!({ "afterEventId": 2 }))).unwrap();
        assert_eq!(rest["events"].as_array().unwrap().len(), 1);
        let bad = dispatcher.dispatch(envelope(&ws, "r", "events.read", json!({ "afterEventId": -1 })));
        assert_eq!(bad.unwrap_err().code, ErrorCode::Validation);
    }

    #[test]
    fn a_stale_dispatcher_cannot_reserve_a_request_or_ledger_an_event() {
        let (dispatcher, ws, host) = dispatcher();
        {
            let mut conn = dispatcher.db.lock().unwrap();
            acquire(&mut conn, HolderKind::Service, 2).unwrap();
        }
        let refused = dispatcher.dispatch(envelope(&ws, "req", "discovery.cancel", json!({ "runId": 1 })));
        assert_eq!(refused.unwrap_err().code, ErrorCode::StaleOwner);
        let conn = dispatcher.db.lock().unwrap();
        assert!(runtime_ledger::read_request(&conn, "req").unwrap().is_none(), "nothing reserved");
        drop(conn);

        // The sink still forwards (the host decides what to show) but the
        // ledger refuses the stale epoch, so nothing is appended.
        dispatcher
            .sink
            .emit(&DiscoveryEvent::Done(crate::discovery_runner::DiscoveryDoneEvent {
                event_version: DISCOVERY_EVENT_VERSION,
                sequence: 9,
                run_id: 1,
                status: crate::db::discovery::RunStatus::Cancelled,
                best_strategy_id: None,
                error_message: None,
            }))
            .unwrap();
        assert_eq!(host.0.lock().unwrap().as_slice(), ["done"]);
        let conn = dispatcher.db.lock().unwrap();
        assert_eq!(runtime_ledger::read_events_after(&conn, 0, 10).unwrap().len(), 0);
    }

    #[test]
    fn payload_hash_is_order_independent_for_objects_and_order_sensitive_for_arrays() {
        assert_eq!(payload_hash(&json!({ "a": 1, "b": [1, 2] })), payload_hash(&json!({ "b": [1, 2], "a": 1 })));
        assert_ne!(payload_hash(&json!({ "b": [2, 1] })), payload_hash(&json!({ "b": [1, 2] })));
        assert_ne!(payload_hash(&json!({ "a": 1 })), payload_hash(&json!({ "a": 1.0 })));
    }

    #[test]
    fn every_whitelisted_name_parses_back_to_itself() {
        for command in Command::ALL {
            assert_eq!(Command::parse(command.name()), Some(*command));
            assert!(command.name().contains('.'), "{}: <domain>.<verb>", command.name());
        }
        assert_eq!(Command::parse("discovery.Start"), None);
        assert_eq!(Command::parse(""), None);
    }
}
