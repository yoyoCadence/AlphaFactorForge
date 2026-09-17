//! P03b — the request ledger, the event ledger, and the workspace identity
//! (migration 0005, docs/research-runtime-contract.md §2 / §3).
//!
//! Everything here writes through `ownership::write_transaction_quiet`, so
//! only the lease holder can reserve a request, complete one, or append an
//! event (a stale owner gets `StaleOwner` exactly as it does for run
//! writes), while none of these writes moves the state version: receipts and
//! ledger rows describe the state, they are not the state (R2).
//!
//! Idempotency (§2) is reserve-then-complete: `reserve_request` inserts a
//! `pending` row BEFORE the command runs; a second envelope with the same
//! `request_id` sees that row and never starts a second piece of work. If the
//! first attempt died between reserve and complete, the row stays `pending`
//! forever and every retry is answered `Busy` with a message saying the
//! outcome is unknown — the caller inspects state (`discovery.active`) and
//! issues a new request id. That is the honest answer; silently re-running
//! is not.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::ownership::write_transaction_quiet as write_transaction;
use crate::error::{AppError, AppResult};

/// Requests are kept at least this long (§2 says ≥ 24 h; this is deliberately
/// longer so a slow retry still replays instead of re-executing).
pub const REQUEST_RETENTION_DAYS: i64 = 7;

// ---------- workspace identity ----------

/// The stable workspace id migration 0005 minted. Missing only if
/// `app_settings` was tampered with, which is reported rather than defaulted.
pub fn workspace_id(conn: &Connection) -> AppResult<String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = 'workspace_id'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let raw = raw.ok_or_else(|| AppError::Other("app_settings has no workspace_id".into()))?;
    let id: String = serde_json::from_str(&raw)
        .map_err(|error| AppError::Other(format!("workspace_id is not a JSON string: {error}")))?;
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::Other(format!("workspace_id is malformed: {id:?}")));
    }
    Ok(id)
}

// ---------- command requests (§2) ----------

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredRequest {
    pub request_id: String,
    pub workspace_id: String,
    pub command: String,
    pub payload_hash: String,
    pub epoch: i64,
    pub status: String,
    pub result_json: Option<String>,
    pub error_json: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// What `reserve_request` decided.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reservation {
    /// No row existed; one is now `pending` and the caller must execute the
    /// command and then `complete_request`.
    Fresh,
    /// The same request (same command and payload) was seen before; the
    /// caller answers from this row and executes nothing.
    Replay(StoredRequest),
    /// The id was seen before with a DIFFERENT command or payload.
    Conflict(StoredRequest),
}

pub fn read_request(conn: &Connection, request_id: &str) -> AppResult<Option<StoredRequest>> {
    Ok(conn
        .query_row(
            "SELECT request_id, workspace_id, command, payload_hash, epoch, status,
                    result_json, error_json, created_at, completed_at
             FROM command_requests WHERE request_id = ?1",
            params![request_id],
            |r| {
                Ok(StoredRequest {
                    request_id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    command: r.get(2)?,
                    payload_hash: r.get(3)?,
                    epoch: r.get(4)?,
                    status: r.get(5)?,
                    result_json: r.get(6)?,
                    error_json: r.get(7)?,
                    created_at: r.get(8)?,
                    completed_at: r.get(9)?,
                })
            },
        )
        .optional()?)
}

/// Reserve `request_id` for `command`/`payload_hash`, or report what already
/// holds it. One write transaction under the caller's epoch, so two concurrent
/// attempts at the same id serialize and exactly one is `Fresh`.
pub fn reserve_request(
    conn: &Connection,
    epoch: Option<i64>,
    request_id: &str,
    workspace_id: &str,
    command: &str,
    payload_hash: &str,
) -> AppResult<Reservation> {
    let tx = write_transaction(conn, epoch)?;
    if let Some(existing) = read_request(&tx, request_id)? {
        let same = existing.command == command && existing.payload_hash == payload_hash;
        tx.rollback()?;
        return Ok(if same { Reservation::Replay(existing) } else { Reservation::Conflict(existing) });
    }
    tx.execute(
        "INSERT INTO command_requests
            (request_id, workspace_id, command, payload_hash, epoch, status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
        params![request_id, workspace_id, command, payload_hash, epoch.unwrap_or(0)],
    )?;
    tx.commit()?;
    Ok(Reservation::Fresh)
}

/// Record the outcome of a `Fresh` reservation. `outcome` is `Ok(result)` or
/// `Err(error)`, both already JSON so the row stores exactly what the caller
/// was told.
pub fn complete_request(
    conn: &Connection,
    epoch: Option<i64>,
    request_id: &str,
    outcome: Result<&Value, &Value>,
) -> AppResult<()> {
    let tx = write_transaction(conn, epoch)?;
    let (status, result_json, error_json) = match outcome {
        Ok(result) => ("succeeded", Some(serde_json::to_string(result)?), None),
        Err(error) => ("failed", None, Some(serde_json::to_string(error)?)),
    };
    let updated = tx.execute(
        "UPDATE command_requests
         SET status = ?2, result_json = ?3, error_json = ?4, completed_at = datetime('now')
         WHERE request_id = ?1 AND status = 'pending'",
        params![request_id, status, result_json, error_json],
    )?;
    if updated != 1 {
        return Err(AppError::Other(format!(
            "request {request_id} is not pending; refusing to overwrite its outcome"
        )));
    }
    tx.commit()?;
    Ok(())
}

/// Drop completed requests older than `REQUEST_RETENTION_DAYS`. Pending rows
/// are never purged: they are the only record that an outcome is unknown.
pub fn purge_old_requests(conn: &Connection, epoch: Option<i64>) -> AppResult<usize> {
    let tx = write_transaction(conn, epoch)?;
    let purged = tx.execute(
        "DELETE FROM command_requests
         WHERE status != 'pending'
           AND created_at < datetime('now', ?1)",
        params![format!("-{REQUEST_RETENTION_DAYS} days")],
    )?;
    tx.commit()?;
    Ok(purged)
}

// ---------- event ledger (§3) ----------

/// One appended event, as stored. `event_id` is the persistent cursor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEvent {
    pub event_id: i64,
    pub epoch: i64,
    pub entity_kind: String,
    pub entity_id: String,
    pub event_version: String,
    pub channel: String,
    pub committed_at: String,
    pub payload_json: String,
}

/// Append one event under `epoch` and return its `event_id`. The payload is
/// stored as given; the ledger does not interpret it.
pub fn append_event(
    conn: &Connection,
    epoch: i64,
    entity_kind: &str,
    entity_id: &str,
    event_version: &str,
    channel: &str,
    payload_json: &str,
) -> AppResult<i64> {
    let tx = write_transaction(conn, Some(epoch))?;
    tx.execute(
        "INSERT INTO runtime_events
            (epoch, entity_kind, entity_id, event_version, channel, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![epoch, entity_kind, entity_id, event_version, channel, payload_json],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(id)
}

/// Events with `event_id > after`, oldest first, at most `limit`. This is the
/// reconnect cursor read: snapshot first, then everything after the cursor.
pub fn read_events_after(conn: &Connection, after: i64, limit: usize) -> AppResult<Vec<StoredEvent>> {
    let mut stmt = conn.prepare(
        "SELECT event_id, epoch, entity_kind, entity_id, event_version, channel, committed_at, payload_json
         FROM runtime_events WHERE event_id > ?1 ORDER BY event_id ASC LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![after, limit as i64], |r| {
            Ok(StoredEvent {
                event_id: r.get(0)?,
                epoch: r.get(1)?,
                entity_kind: r.get(2)?,
                entity_id: r.get(3)?,
                event_version: r.get(4)?,
                channel: r.get(5)?,
                committed_at: r.get(6)?,
                payload_json: r.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// R2: a durable marker that at least one event could not be appended.
/// Written (best effort) by the ledger sink when an append fails; a reader
/// that sees it must re-snapshot rather than trust the cursor alone. It is a
/// separate table from the ledger on purpose — the very write that failed
/// may keep failing on `runtime_events`.
pub fn record_ledger_gap(conn: &Connection, epoch: i64, state_version: i64) -> AppResult<()> {
    let tx = write_transaction(conn, Some(epoch))?;
    tx.execute(
        "INSERT INTO app_settings (key, value_json, updated_at)
         VALUES ('ledger_gap', json_object('epoch', ?1, 'stateVersion', ?2), datetime('now'))
         ON CONFLICT(key) DO UPDATE SET
             value_json = excluded.value_json, updated_at = excluded.updated_at",
        params![epoch, state_version],
    )?;
    tx.commit()?;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerGap {
    pub epoch: i64,
    pub state_version: i64,
}

pub fn read_ledger_gap(conn: &Connection) -> AppResult<Option<LedgerGap>> {
    let raw: Option<String> = conn
        .query_row("SELECT value_json FROM app_settings WHERE key = 'ledger_gap'", [], |r| r.get(0))
        .optional()?;
    match raw {
        None => Ok(None),
        Some(json) => Ok(Some(serde_json::from_str(&json).map_err(|error| {
            AppError::Other(format!("ledger_gap marker unreadable: {error}"))
        })?)),
    }
}

/// The highest `event_id` ever issued (0 when none). Survives deletes.
pub fn last_event_id(conn: &Connection) -> AppResult<i64> {
    Ok(conn
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name = 'runtime_events'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ownership::{acquire, HolderKind};
    use serde_json::json;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        crate::db::apply_migrations(&conn).expect("apply migrations");
        conn
    }

    fn owned_db() -> (Connection, i64) {
        let mut conn = mem_db();
        let epoch = acquire(&mut conn, HolderKind::DesktopEmbedded, 1).unwrap().epoch;
        (conn, epoch)
    }

    #[test]
    fn migration_0005_mints_one_stable_workspace_id() {
        let conn = mem_db();
        let id = workspace_id(&conn).unwrap();
        assert_eq!(id.len(), 32);
        crate::db::apply_migrations(&conn).unwrap();
        assert_eq!(workspace_id(&conn).unwrap(), id, "re-running migrations keeps the id");
        let other = mem_db();
        assert_ne!(workspace_id(&other).unwrap(), id, "another workspace gets another id");
    }

    #[test]
    fn a_request_is_reserved_once_replayed_on_repeat_and_refused_on_conflict() {
        let (conn, epoch) = owned_db();
        let ws = workspace_id(&conn).unwrap();

        let first = reserve_request(&conn, Some(epoch), "req-1", &ws, "discovery.pause", "h1").unwrap();
        assert_eq!(first, Reservation::Fresh);

        // Still pending: the repeat is told so and nothing is re-executed.
        match reserve_request(&conn, Some(epoch), "req-1", &ws, "discovery.pause", "h1").unwrap() {
            Reservation::Replay(stored) => assert_eq!(stored.status, "pending"),
            other => panic!("expected a replay, got {other:?}"),
        }

        complete_request(&conn, Some(epoch), "req-1", Ok(&json!({ "runId": 7 }))).unwrap();
        match reserve_request(&conn, Some(epoch), "req-1", &ws, "discovery.pause", "h1").unwrap() {
            Reservation::Replay(stored) => {
                assert_eq!(stored.status, "succeeded");
                assert_eq!(stored.result_json.as_deref(), Some(r#"{"runId":7}"#));
                assert!(stored.completed_at.is_some());
            }
            other => panic!("expected a replay, got {other:?}"),
        }

        // Same id, different payload (or command): a conflict, never a rerun.
        assert!(matches!(
            reserve_request(&conn, Some(epoch), "req-1", &ws, "discovery.pause", "h2").unwrap(),
            Reservation::Conflict(_)
        ));
        assert!(matches!(
            reserve_request(&conn, Some(epoch), "req-1", &ws, "discovery.cancel", "h1").unwrap(),
            Reservation::Conflict(_)
        ));

        // An outcome is written once; completing again is refused.
        let again = complete_request(&conn, Some(epoch), "req-1", Err(&json!({ "code": "Busy" })));
        assert!(again.is_err());
        assert_eq!(read_request(&conn, "req-1").unwrap().unwrap().status, "succeeded");
    }

    #[test]
    fn a_stale_owner_can_neither_reserve_nor_complete_nor_append() {
        let (mut conn, old) = owned_db();
        let new = acquire(&mut conn, HolderKind::Service, 2).unwrap().epoch;
        let ws = workspace_id(&conn).unwrap();

        let reserved = reserve_request(&conn, Some(old), "req-2", &ws, "discovery.cancel", "h");
        assert!(matches!(reserved, Err(AppError::StaleOwner(_))), "got {reserved:?}");
        assert!(read_request(&conn, "req-2").unwrap().is_none(), "nothing reserved");

        reserve_request(&conn, Some(new), "req-2", &ws, "discovery.cancel", "h").unwrap();
        let completed = complete_request(&conn, Some(old), "req-2", Ok(&json!(null)));
        assert!(matches!(completed, Err(AppError::StaleOwner(_))));
        assert_eq!(read_request(&conn, "req-2").unwrap().unwrap().status, "pending");

        let appended = append_event(&conn, old, "discovery_run", "1", "discovery-event-v1", "discovery://done", "{}");
        assert!(matches!(appended, Err(AppError::StaleOwner(_))));
        assert_eq!(read_events_after(&conn, 0, 10).unwrap().len(), 0);
    }

    #[test]
    fn the_event_ledger_is_monotonic_cursor_readable_and_never_reuses_an_id() {
        let (conn, epoch) = owned_db();
        let a = append_event(&conn, epoch, "discovery_run", "1", "discovery-event-v1", "discovery://progress", r#"{"sequence":1}"#).unwrap();
        let b = append_event(&conn, epoch, "discovery_run", "1", "discovery-event-v1", "discovery://result", r#"{"sequence":2}"#).unwrap();
        let c = append_event(&conn, epoch, "discovery_run", "2", "discovery-event-v1", "discovery://done", r#"{"sequence":3}"#).unwrap();
        assert!(a < b && b < c);
        assert_eq!(last_event_id(&conn).unwrap(), c);

        let after_a = read_events_after(&conn, a, 10).unwrap();
        assert_eq!(after_a.iter().map(|e| e.event_id).collect::<Vec<_>>(), vec![b, c]);
        assert_eq!(after_a[0].channel, "discovery://result");
        assert_eq!(after_a[0].epoch, epoch);
        assert_eq!(after_a[0].entity_id, "1");
        assert_eq!(read_events_after(&conn, c, 10).unwrap().len(), 0, "cursor at the end reads nothing");
        assert_eq!(read_events_after(&conn, 0, 2).unwrap().len(), 2, "limit honoured");

        // Ids are never reused, even after the rows are gone: the sequence
        // remembers the high-water mark across deletes (and, on disk, restarts).
        conn.execute("DELETE FROM runtime_events", []).unwrap();
        let d = append_event(&conn, epoch, "discovery_run", "3", "discovery-event-v1", "discovery://done", "{}").unwrap();
        assert!(d > c, "{d} must follow {c}");
    }

    #[test]
    fn purge_keeps_recent_and_pending_requests() {
        let (conn, epoch) = owned_db();
        let ws = workspace_id(&conn).unwrap();
        reserve_request(&conn, Some(epoch), "old-done", &ws, "x", "h").unwrap();
        complete_request(&conn, Some(epoch), "old-done", Ok(&json!(1))).unwrap();
        reserve_request(&conn, Some(epoch), "old-pending", &ws, "x", "h").unwrap();
        reserve_request(&conn, Some(epoch), "fresh", &ws, "x", "h").unwrap();
        complete_request(&conn, Some(epoch), "fresh", Ok(&json!(1))).unwrap();
        conn.execute(
            "UPDATE command_requests SET created_at = datetime('now', '-8 days')
             WHERE request_id IN ('old-done', 'old-pending')",
            [],
        )
        .unwrap();

        assert_eq!(purge_old_requests(&conn, Some(epoch)).unwrap(), 1);
        assert!(read_request(&conn, "old-done").unwrap().is_none());
        assert!(read_request(&conn, "old-pending").unwrap().is_some(), "an unknown outcome is never forgotten");
        assert!(read_request(&conn, "fresh").unwrap().is_some());
    }
}
