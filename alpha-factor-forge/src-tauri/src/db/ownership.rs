//! P03a — the `workspace_ownership` row (migration 0004,
//! docs/research-runtime-contract.md §1 `ownership-lease-v1`).
//!
//! This module is the database half of ownership. The OS-level exclusive
//! file lock (`runtime::lease`) is the other half and is taken first; nothing
//! here may run without it, because the epoch only means something when a
//! single process can bump it. Split of responsibilities:
//!
//! - `acquire`   — the new lock holder records itself and bumps the epoch.
//! - `assert_epoch` — a writer proves, inside its own transaction, that it
//!   still owns the epoch it started under (`StaleOwner` otherwise).
//! - `heartbeat` — the owner bumps a counter; readers judge liveness by
//!   whether the counter moved over THEIR monotonic interval (`lease_is_stale`),
//!   never by comparing wall clocks between processes, and never as a
//!   licence to take over (§1.4).

use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// Contract §1.4: heartbeat period and the interval after which a reader may
/// call the owner "unreachable" (advisory only).
pub const HEARTBEAT_PERIOD: Duration = Duration::from_secs(5);
#[cfg_attr(not(test), allow(dead_code))] // P04 connect-mode liveness reader.
pub const STALE_AFTER: Duration = Duration::from_secs(30);

/// Contract §1.1 host kinds that may hold the lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HolderKind {
    DesktopEmbedded,
    Service,
}

impl HolderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            HolderKind::DesktopEmbedded => "desktop-embedded",
            HolderKind::Service => "service",
        }
    }
}

/// The single `workspace_ownership` row as stored.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(test), allow(dead_code))] // P04 connect-mode holder display.
pub struct OwnershipRow {
    pub epoch: i64,
    pub holder_kind: String,
    pub holder_instance_id: String,
    pub pid: i64,
    pub acquired_at: String,
    pub heartbeat_at: String,
    pub heartbeat_seq: i64,
}

/// What `acquire` handed the new owner: the epoch every later write must
/// carry, and the instance id that names this acquisition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Acquired {
    pub epoch: i64,
    pub instance_id: String,
}

#[cfg_attr(not(test), allow(dead_code))] // P04 connect-mode holder display.
pub fn read(conn: &Connection) -> AppResult<Option<OwnershipRow>> {
    Ok(conn
        .query_row(
            "SELECT epoch, holder_kind, holder_instance_id, pid, acquired_at, heartbeat_at, heartbeat_seq
             FROM workspace_ownership WHERE id = 1",
            [],
            |r| {
                Ok(OwnershipRow {
                    epoch: r.get(0)?,
                    holder_kind: r.get(1)?,
                    holder_instance_id: r.get(2)?,
                    pid: r.get(3)?,
                    acquired_at: r.get(4)?,
                    heartbeat_at: r.get(5)?,
                    heartbeat_seq: r.get(6)?,
                })
            },
        )
        .optional()?)
}

/// Record the caller as the owner and bump the epoch. Only the process that
/// holds the OS lock may call this (the runtime enforces the order); the
/// epoch never decreases, so a writer that started under an older value is
/// rejected by `assert_epoch` for the rest of its life.
pub fn acquire(conn: &mut Connection, kind: HolderKind, pid: u32) -> AppResult<Acquired> {
    let tx = conn.transaction()?;
    let instance_id = fresh_instance_id(&tx)?;
    let updated = tx.execute(
        "UPDATE workspace_ownership
         SET epoch = epoch + 1,
             holder_kind = ?1,
             holder_instance_id = ?2,
             pid = ?3,
             acquired_at = datetime('now'),
             heartbeat_at = datetime('now'),
             heartbeat_seq = 0
         WHERE id = 1",
        params![kind.as_str(), instance_id, i64::from(pid)],
    )?;
    if updated != 1 {
        return Err(AppError::Other(
            "workspace_ownership row missing: migration 0004 did not run".into(),
        ));
    }
    let epoch: i64 = tx.query_row(
        "SELECT epoch FROM workspace_ownership WHERE id = 1",
        [],
        |r| r.get(0),
    )?;
    tx.commit()?;
    Ok(Acquired { epoch, instance_id })
}

/// Acquire SQLite's write reservation BEFORE checking the epoch. The check
/// and every subsequent write belong to this transaction; another connection
/// cannot advance the epoch between them. Errors roll back on drop.
/// `None` is reserved for existing store tests/unleased callers; runtime
/// runners always supply the epoch returned by `open_workspace`.
///
/// This is the DOMAIN write boundary: it also bumps `runtime_state.
/// mutation_seq` (the state version readers compare, P03b R2), so every
/// committed change to a run, job, or result moves the version. Writes that
/// only describe state — heartbeats, request receipts, ledger appends — use
/// `write_transaction_quiet` and leave the version alone.
pub fn write_transaction(conn: &Connection, epoch: Option<i64>) -> AppResult<Transaction<'_>> {
    let tx = write_transaction_quiet(conn, epoch)?;
    tx.execute("UPDATE runtime_state SET mutation_seq = mutation_seq + 1 WHERE id = 1", [])?;
    Ok(tx)
}

/// The same reservation and epoch check, without moving the state version.
pub fn write_transaction_quiet(conn: &Connection, epoch: Option<i64>) -> AppResult<Transaction<'_>> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    if let Some(epoch) = epoch {
        assert_epoch(&tx, epoch)?;
    }
    Ok(tx)
}

/// The current state version (`runtime_state.mutation_seq`).
pub fn state_version(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT mutation_seq FROM runtime_state WHERE id = 1", [], |r| r.get(0))?)
}

/// Fail with `StaleOwner` unless the stored epoch is exactly `expected`.
/// Call this INSIDE the transaction that is about to write (contract §1.3),
/// so the check and the write are one unit.
pub fn assert_epoch(conn: &Connection, expected: i64) -> AppResult<()> {
    let current: i64 = conn.query_row(
        "SELECT epoch FROM workspace_ownership WHERE id = 1",
        [],
        |r| r.get(0),
    )?;
    if current == expected {
        Ok(())
    } else {
        Err(AppError::StaleOwner(format!(
            "workspace ownership epoch is {current}, this writer holds {expected}"
        )))
    }
}

/// Bump the liveness counter. Refuses (with `StaleOwner`) if the epoch moved,
/// which is how a heartbeat thread learns it no longer owns the workspace.
pub fn heartbeat(conn: &Connection, epoch: i64) -> AppResult<i64> {
    let tx = write_transaction_quiet(conn, Some(epoch))?;
    tx.execute(
        "UPDATE workspace_ownership
         SET heartbeat_seq = heartbeat_seq + 1, heartbeat_at = datetime('now')
         WHERE id = 1 AND epoch = ?1",
        params![epoch],
    )?;
    let sequence = tx.query_row(
        "SELECT heartbeat_seq FROM workspace_ownership WHERE id = 1",
        [],
        |r| r.get(0),
    )?;
    tx.commit()?;
    Ok(sequence)
}

/// Advisory liveness (contract §1.4): the owner is "unreachable" when its
/// heartbeat counter has not moved for `STALE_AFTER` of the READER's own
/// monotonic time. A stale verdict labels a UI; it never authorises a
/// take-over, because a sleeping owner still holds the OS lock.
#[cfg_attr(not(test), allow(dead_code))] // P04 connect-mode liveness reader.
pub fn lease_is_stale(seq_before: i64, seq_now: i64, elapsed: Duration) -> bool {
    seq_now == seq_before && elapsed >= STALE_AFTER
}

/// 128 random-ish bits as hex: SHA-256 over the process id, a nanosecond
/// clock reading, and the epoch about to be taken, truncated. Uniqueness per
/// acquisition is all that is required (contract §1.2 step 4); this is not a
/// secret and needs no CSPRNG.
fn fresh_instance_id(conn: &Connection) -> AppResult<String> {
    use sha2::{Digest, Sha256};
    let epoch: i64 = conn.query_row(
        "SELECT epoch FROM workspace_ownership WHERE id = 1",
        [],
        |r| r.get(0),
    )?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(nanos.to_le_bytes());
    hasher.update((epoch + 1).to_le_bytes());
    Ok(hex::encode(&hasher.finalize()[..16]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        crate::db::apply_migrations(&conn).expect("apply migrations");
        conn
    }

    #[test]
    fn a_fresh_workspace_is_unowned_at_epoch_zero() {
        let conn = mem_db();
        let row = read(&conn).unwrap().expect("0004 seeds the row");
        assert_eq!(row.epoch, 0);
        assert_eq!(row.holder_kind, "none");
        assert_eq!(row.holder_instance_id, "");
        assert_eq!(row.heartbeat_seq, 0);
    }

    #[test]
    fn acquire_bumps_the_epoch_and_records_the_holder() {
        let mut conn = mem_db();
        let first = acquire(&mut conn, HolderKind::DesktopEmbedded, 42).unwrap();
        assert_eq!(first.epoch, 1);
        assert_eq!(first.instance_id.len(), 32, "128 bits as hex");

        let row = read(&conn).unwrap().unwrap();
        assert_eq!(row.holder_kind, "desktop-embedded");
        assert_eq!(row.pid, 42);
        assert_eq!(row.holder_instance_id, first.instance_id);
        assert_eq!(row.heartbeat_seq, 0, "a new acquisition restarts the counter");

        let second = acquire(&mut conn, HolderKind::Service, 43).unwrap();
        assert_eq!(second.epoch, 2, "the epoch never decreases");
        assert_ne!(second.instance_id, first.instance_id);
        assert_eq!(read(&conn).unwrap().unwrap().holder_kind, "service");
    }

    #[test]
    fn a_writer_under_a_previous_epoch_is_rejected_as_stale() {
        let mut conn = mem_db();
        let old = acquire(&mut conn, HolderKind::DesktopEmbedded, 1).unwrap();
        assert!(assert_epoch(&conn, old.epoch).is_ok());

        // Another host takes over (only possible after the OS lock changed hands).
        let new = acquire(&mut conn, HolderKind::Service, 2).unwrap();
        let error = assert_epoch(&conn, old.epoch).unwrap_err();
        assert!(matches!(error, AppError::StaleOwner(_)), "got {error:?}");
        assert!(error.to_string().contains("epoch is 2, this writer holds 1"));
        assert!(assert_epoch(&conn, new.epoch).is_ok());

        // The stale owner's heartbeat is refused too, and does not touch the counter.
        assert!(matches!(heartbeat(&conn, old.epoch), Err(AppError::StaleOwner(_))));
        assert_eq!(read(&conn).unwrap().unwrap().heartbeat_seq, 0);
    }

    #[test]
    fn heartbeat_advances_the_counter_for_the_current_owner_only() {
        let mut conn = mem_db();
        let owner = acquire(&mut conn, HolderKind::DesktopEmbedded, 1).unwrap();
        assert_eq!(heartbeat(&conn, owner.epoch).unwrap(), 1);
        assert_eq!(heartbeat(&conn, owner.epoch).unwrap(), 2);
        assert_eq!(read(&conn).unwrap().unwrap().heartbeat_seq, 2);
    }

    #[test]
    fn staleness_is_judged_by_the_readers_monotonic_interval_not_by_clocks() {
        // Counter moved: alive, however long the interval.
        assert!(!lease_is_stale(3, 4, Duration::from_secs(3600)));
        // Counter still: unreachable only once the reader has waited STALE_AFTER.
        assert!(!lease_is_stale(3, 3, STALE_AFTER - Duration::from_millis(1)));
        assert!(lease_is_stale(3, 3, STALE_AFTER));
        // Six heartbeat periods of tolerance, as the contract states.
        assert_eq!(STALE_AFTER, HEARTBEAT_PERIOD * 6);
    }
}
