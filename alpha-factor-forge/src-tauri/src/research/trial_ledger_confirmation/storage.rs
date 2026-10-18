//! Bounded archival graph IO. Supplied original-source observations are not
//! publisher authentication, fresh workspace authority or execution permits.
use super::*;
use crate::research::trial_ledger::{self as ledger, TrialLedger};
use alpha_factor_forge::discovery_core::{
    alpha_allocation::alpha_allocation_id,
    confirmation_history::{
        complete_confirmation_graph, validate_confirmation_graph, BlobKind, ConfirmationInventory,
        GraphContext, HistoryBlob, ValidatedGraph, MAX_BINDING_BYTES, MAX_DOCUMENT_BYTES,
        MAX_HISTORY_BYTES, MAX_HISTORY_INTERVALS, MAX_HISTORY_ROWS,
    },
};
use rusqlite::{types::Value as SqlValue, Transaction, TransactionBehavior};
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "projection.rs"]
mod projection;
#[path = "witnessed.rs"]
mod witnessed;
#[path = "write.rs"]
mod write;
use projection::{DocSpec, DOCS};
#[allow(unused_imports)]
pub(in crate::research::trial_ledger) use witnessed::{append_witnessed_in, load_witnessed_in};
pub(in crate::research::trial_ledger) use write::append_in;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmationWriteSummary {
    pub added_records: usize,
    pub added_blobs: usize,
    pub added_intervals: usize,
    pub added_unknown_origins: usize,
}
#[derive(Clone, Debug)]
pub struct ConfirmationLookup {
    graph: ValidatedGraph,
    operation_ids: Vec<String>,
}
impl ConfirmationLookup {
    pub fn graph(&self) -> &ValidatedGraph {
        &self.graph
    }
    pub fn operation_ids(&self) -> &[String] {
        &self.operation_ids
    }
}
fn bad(detail: &str) -> LedgerError {
    LedgerError::StateInvalid(format!("confirmation history: {detail}"))
}
fn section<'a>(i: &'a ConfirmationInventory, spec: &DocSpec) -> &'a Vec<String> {
    match spec.table {
        "confirmation_batches" => &i.batches,
        "confirmation_selections" => &i.selections,
        "confirmation_operations" => &i.operations,
        "confirmation_exposures" => &i.exposures,
        "confirmation_admissions" => &i.admissions,
        "confirmation_events" => &i.events,
        _ => &i.conflicts,
    }
}
fn section_mut<'a>(i: &'a mut ConfirmationInventory, spec: &DocSpec) -> &'a mut Vec<String> {
    match spec.table {
        "confirmation_batches" => &mut i.batches,
        "confirmation_selections" => &mut i.selections,
        "confirmation_operations" => &mut i.operations,
        "confirmation_exposures" => &mut i.exposures,
        "confirmation_admissions" => &mut i.admissions,
        "confirmation_events" => &mut i.events,
        _ => &mut i.conflicts,
    }
}
fn json(raw: &str) -> Result<Value, LedgerError> {
    serde_json::from_str(raw).map_err(|_| bad("invalid document"))
}
fn string<'a>(v: &'a Value, name: &str) -> Result<&'a str, LedgerError> {
    v.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| bad("invalid projection"))
}
fn document_id(v: &Value) -> Result<&str, LedgerError> {
    string(v, "id")
}
fn origin(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

/// Aggregates and metadata caps run before allocating any document/blob row.
fn preflight(conn: &Connection, witnessed: bool) -> Result<(), LedgerError> {
    // Staged foundation: the old two-kind reader must never omit witness rows.
    // Strict combined hydration/current context/restoration arrives in 2b-2.
    let witnesses: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM confirmation_source_witnesses LIMIT 1)",
        [],
        |r| r.get(0),
    )?;
    if witnesses && !witnessed {
        return Err(bad("source witnesses require checked schema-7 context"));
    }
    let mut rows = 0i64;
    let mut bytes = 0i64;
    for spec in DOCS {
        let (n, b): (i64, i64) = conn.query_row(
            &format!(
                "SELECT COUNT(*),COALESCE(SUM(length(CAST(document_json AS BLOB))),0) FROM {}",
                spec.table
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        rows = rows
            .checked_add(n)
            .ok_or_else(|| bad("capacity exceeded"))?;
        bytes = bytes
            .checked_add(b)
            .ok_or_else(|| bad("capacity exceeded"))?;
        let mut predicates=vec![format!("typeof(document_json)<>'text' OR length(CAST(document_json AS BLOB)) NOT BETWEEN 1 AND {MAX_DOCUMENT_BYTES}"),"typeof(recorded_at)<>'text'".into()];
        for (column, cap) in spec.columns {
            predicates.push(format!("({column} IS NOT NULL AND (typeof({column})<>'text' OR length(CAST({column} AS BLOB))>{cap}))"));
        }
        let invalid: bool = conn.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM {} WHERE {} LIMIT 1)",
                spec.table,
                predicates.join(" OR ")
            ),
            [],
            |r| r.get(0),
        )?;
        if invalid {
            return Err(bad("invalid stored row or byte length"));
        }
    }
    let (n, b): (i64, i64) = conn.query_row(
        "SELECT COUNT(*),COALESCE(SUM(length(content_bytes)),0) FROM confirmation_blobs",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    rows = rows
        .checked_add(n)
        .ok_or_else(|| bad("capacity exceeded"))?;
    bytes = bytes
        .checked_add(b)
        .ok_or_else(|| bad("capacity exceeded"))?;
    if witnessed {
        let (n,b):(i64,i64)=conn.query_row("SELECT COUNT(*),COALESCE(SUM(length(content_bytes)),0) FROM confirmation_source_witnesses",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
        rows = rows
            .checked_add(n)
            .ok_or_else(|| bad("capacity exceeded"))?;
        bytes = bytes
            .checked_add(b)
            .ok_or_else(|| bad("capacity exceeded"))?;
        let invalid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM confirmation_source_witnesses WHERE typeof(witness_sha256)<>'text' OR length(CAST(witness_sha256 AS BLOB))<>64 OR witness_sha256 GLOB '*[^0-9a-f]*' OR typeof(byte_length)<>'integer' OR byte_length NOT BETWEEN 1 AND 8388608 OR typeof(content_bytes)<>'blob' OR length(content_bytes)<>byte_length OR typeof(recorded_at)<>'text') OR EXISTS(SELECT 1 FROM confirmation_source_witnesses w JOIN confirmation_blobs b ON b.blob_sha256=w.witness_sha256)",[],|r|r.get(0))?;
        if invalid {
            return Err(bad("invalid stored witness or namespace"));
        }
    }
    if rows > MAX_HISTORY_ROWS as i64 || bytes > MAX_HISTORY_BYTES as i64 {
        return Err(bad("capacity exceeded"));
    }
    let invalid:bool=conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM confirmation_blobs WHERE typeof(blob_sha256)<>'text' OR length(CAST(blob_sha256 AS BLOB))<>64 OR typeof(kind)<>'text' OR kind NOT IN ('statistics','binding') OR typeof(byte_length)<>'integer' OR byte_length<>length(content_bytes) OR typeof(content_bytes)<>'blob' OR byte_length<1 OR byte_length>CASE WHEN kind='binding' THEN {MAX_BINDING_BYTES} ELSE {MAX_DOCUMENT_BYTES} END OR typeof(recorded_at)<>'text')"),[],|r|r.get(0))?;
    if invalid {
        return Err(bad("invalid stored blob"));
    }
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM confirmation_intervals", [], |r| {
        r.get(0)
    })?;
    if count > MAX_HISTORY_INTERVALS as i64 {
        return Err(bad("interval capacity exceeded"));
    }
    let invalid:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM confirmation_intervals WHERE typeof(exposure_id)<>'text' OR length(CAST(exposure_id AS BLOB))<>64 OR typeof(instrument_id)<>'text' OR length(CAST(instrument_id AS BLOB))>256 OR typeof(interval_index)<>'integer' OR typeof(from_ms)<>'integer' OR typeof(to_exclusive_ms)<>'integer')",[],|r|r.get(0))?;
    if invalid {
        return Err(bad("invalid interval projection"));
    }
    unknown_origins(conn)?;
    Ok(())
}
fn read_inventory(conn: &Connection) -> Result<ConfirmationInventory, LedgerError> {
    read_inventory_mode(conn, false)
}
fn read_witnessed_inventory(conn: &Connection) -> Result<ConfirmationInventory, LedgerError> {
    read_inventory_mode(conn, true)
}
fn read_inventory_mode(
    conn: &Connection,
    witnessed: bool,
) -> Result<ConfirmationInventory, LedgerError> {
    preflight(conn, witnessed)?;
    let mut inventory = ConfirmationInventory::default();
    for spec in DOCS {
        let mut stmt = conn.prepare(&format!(
            "SELECT document_json FROM {} ORDER BY {}",
            spec.table, spec.columns[0].0
        ))?;
        *section_mut(&mut inventory, &spec) = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()
            .map_err(|_| bad("invalid document bytes"))?;
    }
    let mut stmt=conn.prepare("SELECT blob_sha256,kind,byte_length,content_bytes FROM confirmation_blobs ORDER BY blob_sha256")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let sha: String = row.get(0).map_err(|_| bad("invalid blob row"))?;
        let kind: String = row.get(1).map_err(|_| bad("invalid blob row"))?;
        let length: i64 = row.get(2).map_err(|_| bad("invalid blob row"))?;
        let bytes: Vec<u8> = row.get(3).map_err(|_| bad("invalid blob row"))?;
        let text = std::str::from_utf8(&bytes).map_err(|_| bad("invalid blob UTF-8"))?;
        let kind = match kind.as_str() {
            "statistics" => BlobKind::Statistics,
            "binding" => BlobKind::Binding,
            _ => return Err(bad("invalid blob kind")),
        };
        let blob = HistoryBlob::new(kind, text).map_err(|_| bad("invalid original blob"))?;
        if blob.sha256() != sha || length as usize != bytes.len() {
            return Err(bad("blob identity or length differs"));
        }
        inventory.blobs.push(blob);
    }
    if witnessed {
        let mut stmt=conn.prepare("SELECT witness_sha256,byte_length,content_bytes FROM confirmation_source_witnesses ORDER BY witness_sha256")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let sha: String = row.get(0).map_err(|_| bad("invalid witness row"))?;
            let length: i64 = row.get(1).map_err(|_| bad("invalid witness row"))?;
            let bytes: Vec<u8> = row.get(2).map_err(|_| bad("invalid witness row"))?;
            let text = std::str::from_utf8(&bytes).map_err(|_| bad("invalid witness UTF-8"))?;
            let blob = HistoryBlob::new(BlobKind::SourceWitness, text)
                .map_err(|_| bad("invalid original witness"))?;
            if blob.sha256() != sha || length as usize != bytes.len() {
                return Err(bad("witness identity or length differs"));
            }
            inventory.blobs.push(blob);
        }
        inventory.blobs.sort_by(|a, b| a.sha256().cmp(b.sha256()));
    }
    inventory.unknown_origins = unknown_origins(conn)?.into_iter().collect();
    Ok(inventory)
}

/// Bounded migration enumeration, not graph adoption or source reconstruction.
/// Original typed provenance and binding codecs precede any origin extraction.
pub(super) fn legacy_managed_origins(conn: &Connection) -> Result<BTreeSet<String>, LedgerError> {
    use alpha_factor_forge::discovery_core::{
        confirmation_contracts::read_operation_provenance,
        confirmation_history::{parse_binding_snapshot, read_exposure_provenance},
    };
    let inventory = read_inventory(conn)?;
    projection::verify(conn, &inventory)?;
    let mut origins = BTreeSet::new();
    let mut managed = !inventory.batches.is_empty()
        || !inventory.selections.is_empty()
        || !inventory.operations.is_empty()
        || !inventory.admissions.is_empty()
        || !inventory.events.is_empty()
        || !inventory.blobs.is_empty();
    for raw in &inventory.operations {
        let (_, origin) = read_operation_provenance(raw)
            .map_err(|_| bad("invalid original operation provenance"))?;
        origins.insert(origin);
    }
    for raw in &inventory.exposures {
        let (_, origin) = read_exposure_provenance(raw)
            .map_err(|_| bad("invalid original exposure provenance"))?;
        managed |= json(raw)?["body"]["claim"]["kind"] == "confirmation";
        origins.insert(origin);
    }
    for blob in &inventory.blobs {
        if blob.kind() == BlobKind::Binding {
            let binding = parse_binding_snapshot(blob.text())
                .map_err(|_| bad("invalid original binding provenance"))?;
            origins.insert(binding.registry_id().into());
        }
    }
    if managed {
        let local: String = conn.query_row(
            "SELECT value FROM registry_meta WHERE key='registry_id'",
            [],
            |r| r.get(0),
        )?;
        if !origin(&local) {
            return Err(bad("invalid local registry origin"));
        }
        origins.insert(local);
        Ok(origins)
    } else {
        Ok(BTreeSet::new())
    }
}
fn check_context(
    tx: &Transaction<'_>,
    registry: &str,
    c: &GraphContext,
) -> Result<(), LedgerError> {
    if !matches!(
        ledger::verify_chain(tx, registry).map_err(|_| bad("invalid trial chain"))?,
        ledger::LedgerIntegrity::Intact { .. }
    ) {
        return Err(bad("invalid trial chain"));
    }
    let evidence =
        ledger::binding::observed_evidence(tx).map_err(|_| bad("invalid trial observations"))?;
    if evidence.quarantined_families != c.quarantined_families.iter().cloned().collect()
        || evidence.conflicted_origins != c.conflicted_origins.iter().cloned().collect()
        || evidence.family_tests != c.family_tests
    {
        return Err(bad("trial observations differ"));
    }
    let alpha =
        ledger::alpha::observed_identities(tx).map_err(|_| bad("invalid alpha observations"))?;
    let budgets = c
        .allocations
        .iter()
        .map(|a| {
            Ok(ledger::AlphaBudgetIdentity {
                family_id: a.family_id.clone(),
                allocation_id: alpha_allocation_id(&a.declaration)
                    .map_err(|_| bad("invalid allocation"))?,
            })
        })
        .collect::<Result<BTreeSet<_>, LedgerError>>()?;
    let reservations = c
        .reservations
        .iter()
        .map(|r| r.reservation_id().to_string())
        .collect();
    let conflicts = c.alpha_conflicts.iter().map(|v| v.id.clone()).collect();
    if alpha.budgets != budgets
        || alpha.reservations != reservations
        || alpha.conflicts != conflicts
        || ledger::alpha::unknown_origins(tx).map_err(|_| bad("invalid alpha provenance"))?
            != c.alpha_unknown_origins.iter().cloned().collect()
    {
        return Err(bad("alpha observations differ"));
    }
    let mut families: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut stmt = tx.prepare(
        "SELECT conflict_id,family_id FROM alpha_conflicts ORDER BY conflict_id,family_id",
    )?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (id, family) = row?;
        families.entry(id).or_default().insert(family);
    }
    let expected: BTreeMap<_, BTreeSet<_>> = c
        .alpha_conflicts
        .iter()
        .map(|v| (v.id.clone(), v.family_ids.iter().cloned().collect()))
        .collect();
    if families != expected {
        return Err(bad("alpha conflict targets differ"));
    }
    Ok(())
}
pub(in crate::research::trial_ledger) fn load_in(
    tx: &Transaction<'_>,
    registry: &str,
    ctx: &GraphContext,
) -> Result<ValidatedGraph, LedgerError> {
    let inventory = read_inventory(tx)?;
    let graph = validate_confirmation_graph(&inventory, ctx)
        .map_err(|_| bad("invalid materialized graph"))?;
    check_context(tx, registry, ctx)?;
    projection::verify(tx, graph.inventory())?;
    Ok(graph)
}

impl TrialLedger {
    /// One consistent archival snapshot against supplied original-source facts
    /// and independently checked current registry restrictions.
    pub fn read_confirmation_history(
        &self,
        ctx: &GraphContext,
    ) -> Result<ValidatedGraph, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let graph = load_in(&tx, self.registry_id(), ctx)?;
        tx.commit()?;
        Ok(graph)
    }
    /// Sorted original-origin IDs and their checked history; never a rerun or
    /// artifact-disclosure capability.
    pub fn find_original_confirmation_operations(
        &self,
        origin_registry_id: &str,
        reservation_key: &str,
        ctx: &GraphContext,
    ) -> Result<ConfirmationLookup, LedgerError> {
        if !origin(origin_registry_id) || !origin(reservation_key) {
            return Err(bad("invalid original operation key"));
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let graph = load_in(&tx, self.registry_id(), ctx)?;
        let ids = {
            let mut stmt=tx.prepare("SELECT operation_id FROM confirmation_operations WHERE origin_registry_id=?1 AND reservation_key=?2 ORDER BY operation_id")?;
            let values = stmt
                .query_map(params![origin_registry_id, reservation_key], |r| {
                    r.get::<_, String>(0)
                })?
                .collect::<Result<_, _>>()?;
            values
        };
        tx.commit()?;
        Ok(ConfirmationLookup {
            graph,
            operation_ids: ids,
        })
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
