//! Dormant workspace metadata observations for P13c final preparation.
//! Matching values (including a copy) prove consistency, not physical ownership
//! or unseen input authority. The caller owns the transaction and its lifetime.
#![cfg_attr(not(test), allow(dead_code))] // No runtime caller in P13c-4b-1.

use rusqlite::{types::ValueRef, OptionalExtension, Transaction};

use crate::db::ownership::HolderKind;
use crate::error::{AppError, AppResult};

const MAX_WORKSPACE_ID_JSON_BYTES: usize = 256;

mod original_json;
mod source_rows;

fn invalid() -> AppError {
    AppError::Other("workspace_observation_invalid".into())
}

fn changed() -> AppError {
    AppError::Other("workspace_observation_changed".into())
}

fn stale_owner() -> AppError {
    AppError::StaleOwner("workspace observation owner mismatch".into())
}

fn identity(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Validated comparison values only. Construction authenticates no caller/lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExpectedWorkspace {
    workspace_id: String,
    epoch: i64,
    holder_kind: HolderKind,
    holder_instance_id: String,
}

impl ExpectedWorkspace {
    pub(super) fn new(
        workspace_id: &str,
        epoch: i64,
        holder_kind: HolderKind,
        holder_instance_id: &str,
    ) -> AppResult<Self> {
        if !identity(workspace_id) || epoch <= 0 || !identity(holder_instance_id) {
            return Err(invalid());
        }
        Ok(Self {
            workspace_id: workspace_id.into(),
            epoch,
            holder_kind,
            holder_instance_id: holder_instance_id.into(),
        })
    }
}

/// Private actual SQL values; no wire/default constructor or live capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceObservation {
    workspace_id: String,
    epoch: i64,
    holder_kind: HolderKind,
    holder_instance_id: String,
    mutation_seq: i64,
}

impl WorkspaceObservation {
    fn check_owner(&self, epoch: i64, kind: HolderKind, instance: &str) -> AppResult<()> {
        if self.epoch != epoch || self.holder_kind != kind || self.holder_instance_id != instance {
            return Err(stale_owner());
        }
        Ok(())
    }
}

/// Observe three bounded actual records in the caller's consistent snapshot.
/// Deferred equality is historical evidence only. This never reserves or writes.
pub(super) fn capture_workspace_in(
    tx: &Transaction<'_>,
    expected: &ExpectedWorkspace,
) -> AppResult<WorkspaceObservation> {
    if tx.is_autocommit() {
        return Err(invalid());
    }
    // CASE guards prevent copying arbitrary stored text/BLOBs into Rust.
    let raw: Option<Option<Vec<u8>>> = tx
        .query_row(
            "SELECT CASE WHEN typeof(value_json) = 'text'
                         AND length(CAST(value_json AS BLOB)) <= ?1
                    THEN value_json ELSE NULL END
               FROM app_settings WHERE key = 'workspace_id'",
            [MAX_WORKSPACE_ID_JSON_BYTES as i64],
            |row| {
                Ok(match row.get_ref(0)? {
                    ValueRef::Text(bytes) => Some(bytes.to_vec()),
                    _ => None,
                })
            },
        )
        .optional()?;
    let raw = raw.flatten().ok_or_else(invalid)?;
    let workspace_id: String = serde_json::from_slice(&raw).map_err(|_| invalid())?;
    if !identity(&workspace_id) {
        return Err(invalid());
    }
    let owner = tx
        .query_row(
            "SELECT CASE WHEN typeof(epoch) = 'integer' AND epoch > 0
                         AND typeof(holder_kind) = 'text'
                         AND length(CAST(holder_kind AS BLOB)) <= 16
                         AND typeof(holder_instance_id) = 'text'
                         AND length(CAST(holder_instance_id AS BLOB)) <= 32
                    THEN 1 ELSE 0 END, epoch, holder_kind, holder_instance_id
               FROM workspace_ownership WHERE id = 1",
            [],
            |row| {
                if !row.get::<_, bool>(0)? {
                    return Ok(None);
                }
                let (ValueRef::Text(kind), ValueRef::Text(instance)) =
                    (row.get_ref(2)?, row.get_ref(3)?)
                else {
                    return Ok(None);
                };
                Ok(Some((
                    row.get::<_, i64>(1)?,
                    kind.to_vec(),
                    instance.to_vec(),
                )))
            },
        )
        .optional()?
        .flatten()
        .ok_or_else(invalid)?;
    let (epoch, kind, holder_instance_id) = owner;
    let holder_kind = match kind.as_slice() {
        b"desktop-embedded" => HolderKind::DesktopEmbedded,
        b"service" => HolderKind::Service,
        _ => return Err(invalid()),
    };
    let holder_instance_id = String::from_utf8(holder_instance_id).map_err(|_| invalid())?;
    if !identity(&holder_instance_id) {
        return Err(invalid());
    }
    let mutation_seq: Option<Option<i64>> = tx
        .query_row(
            "SELECT CASE WHEN typeof(mutation_seq) = 'integer' AND mutation_seq >= 0
                    THEN mutation_seq ELSE NULL END FROM runtime_state WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let observed = WorkspaceObservation {
        workspace_id,
        epoch,
        holder_kind,
        holder_instance_id,
        mutation_seq: mutation_seq.flatten().ok_or_else(invalid)?,
    };
    if observed.workspace_id != expected.workspace_id {
        return Err(changed());
    }
    observed.check_owner(
        expected.epoch,
        expected.holder_kind,
        &expected.holder_instance_id,
    )?;
    Ok(observed)
}

/// Compare before the future caller's domain increment, without replacing saved
/// preparation or acquiring its final IMMEDIATE/physical owner guard for it.
pub(super) fn recheck_workspace_in(
    tx: &Transaction<'_>,
    expected: &ExpectedWorkspace,
    saved: &WorkspaceObservation,
) -> AppResult<()> {
    let observed = capture_workspace_in(tx, expected)?;
    if observed.workspace_id != saved.workspace_id {
        return Err(changed());
    }
    observed.check_owner(saved.epoch, saved.holder_kind, &saved.holder_instance_id)?;
    if observed.mutation_seq != saved.mutation_seq {
        return Err(changed());
    }
    Ok(())
}

#[cfg(test)]
#[path = "confirmation_inputs/tests.rs"]
mod tests;
