//! Schema-6 provenance, checked archival graph IO and original-origin lookup.
//! Original-source observations remain a host boundary, not admission rights.
use super::LedgerError;
#[path = "trial_ledger_confirmation/storage.rs"]
mod storage;
#[path = "trial_ledger_confirmation/archive.rs"]
mod archive;
use rusqlite::{params, Connection};
use std::collections::BTreeSet;
// Transaction-scoped extension points for the later final fence/v5 importer.
#[allow(unused_imports)]
pub(super) use storage::{append_in, load_in};
#[allow(unused_imports)]
pub(super) use storage::{append_witnessed_in, load_witnessed_in};
pub use storage::{ConfirmationLookup, ConfirmationWriteSummary};
pub use archive::WitnessedTransferSummary;

pub(super) const TABLES: [&str; 11] = [
    "confirmation_batches",
    "confirmation_selections",
    "confirmation_operations",
    "confirmation_exposures",
    "confirmation_intervals",
    "confirmation_admissions",
    "confirmation_events",
    "confirmation_blobs",
    "confirmation_conflicts",
    "confirmation_unknown_origins",
    "confirmation_source_witnesses",
];
pub(super) const MAX_UNKNOWN_ORIGINS: usize = 10000;
fn valid_origin(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn invalid() -> LedgerError {
    LedgerError::StateInvalid("invalid or oversized confirmation provenance".into())
}
/// Called only after schema-7 DDL, before its receipt in the same transaction.
/// Missing source proofs remain incomplete history; uncertainty is permanent.
pub(super) fn backfill_source_witness_provenance(conn: &Connection) -> Result<(), LedgerError> {
    let origins = storage::legacy_managed_origins(conn)?;
    record_unknown_origins(conn, &origins)
}
pub(super) fn unknown_origins(conn: &Connection) -> Result<BTreeSet<String>, LedgerError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM confirmation_unknown_origins",
        [],
        |r| r.get(0),
    )?;
    if !(0..=MAX_UNKNOWN_ORIGINS as i64).contains(&count) {
        return Err(invalid());
    }
    let malformed:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM confirmation_unknown_origins WHERE typeof(origin_registry_id)<>'text' OR length(CAST(origin_registry_id AS BLOB))<>32 OR typeof(recorded_at)<>'text')",[],|r|r.get(0))?;
    if malformed {
        return Err(invalid());
    }
    let mut statement = conn.prepare(
        "SELECT origin_registry_id FROM confirmation_unknown_origins ORDER BY origin_registry_id",
    )?;
    let values: BTreeSet<String> = statement
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if values.iter().any(|v| !valid_origin(v)) {
        return Err(invalid());
    }
    Ok(values)
}
pub(super) fn preflight_unknown_origins(
    conn: &Connection,
    origins: &BTreeSet<String>,
) -> Result<BTreeSet<String>, LedgerError> {
    let old = unknown_origins(conn)?;
    if origins.iter().any(|v| !valid_origin(v)) || old.union(origins).count() > MAX_UNKNOWN_ORIGINS
    {
        return Err(invalid());
    }
    Ok(origins.difference(&old).cloned().collect())
}
pub(super) fn record_unknown_origins(
    conn: &Connection,
    origins: &BTreeSet<String>,
) -> Result<(), LedgerError> {
    for origin in preflight_unknown_origins(conn, origins)? {
        conn.execute("INSERT INTO confirmation_unknown_origins (origin_registry_id,recorded_at) VALUES (?1,?2)",params![origin,chrono::Utc::now().to_rfc3339()])?;
    }
    Ok(())
}
pub(super) fn require_legacy_exportable(conn: &Connection) -> Result<(), LedgerError> {
    for table in TABLES {
        let found: bool = conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} LIMIT 1)"),
            [],
            |r| r.get(0),
        )?;
        if found {
            return Err(LedgerError::StateInvalid(
                "confirmation history requires complete v5 export; legacy projection refused"
                    .into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "trial_ledger_confirmation/tests.rs"]
mod tests;
