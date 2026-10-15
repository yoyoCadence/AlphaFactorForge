//! P13b-1 external registry storage. No runtime caller or admission permit.
//! Binding/complete transfer are P13b-2; integrated final-fence reservation
//! and exposure are P13c. Alpha stops never quarantine ordinary trial work.

use std::collections::{BTreeMap, BTreeSet};

use alpha_factor_forge::discovery_core::{
    alpha_allocation::{
        alpha_allocation_id, parse_alpha_allocation, AlphaAllocationDeclaration,
        AlphaAllocationReport,
    },
    alpha_reservations::{
        freeze_alpha_reservation, parse_alpha_reservation, review_alpha_reservations,
        AlphaReservation, AlphaReservationConflict, AlphaReservationHistoryStatus,
        ALPHA_RESERVATION_VERSION,
    },
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{canonical_text, sha256_hex, LedgerError, TrialLedger};

const CONFLICT_VERSION: &str = "alpha-registry-conflict-v1";
const MAX_BUDGET_BYTES: usize = 262_144;
const MAX_RECORD_BYTES: usize = 4_096;
const MAX_CONFLICT_BYTES: usize = 1_048_576;
const MAX_ALPHA_ROWS: i64 = 10_000;
const MAX_ALPHA_BYTES: i64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaStorageStatus {
    Stored,
    Replayed,
    Blocked,
    Exhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaStorageOutcome {
    pub status: AlphaStorageStatus,
    pub reservation: Option<AlphaReservation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaFamilyState {
    pub declaration: AlphaAllocationDeclaration,
    pub reservations: Vec<AlphaReservation>,
    pub persistent_conflicts: Vec<Value>,
    pub derived_conflicts: Vec<AlphaReservationConflict>,
    pub blocked: bool,
    /// None on any permanent/derived conflict. This is amount allocation
    /// only, never a production confirmation admission permit.
    pub allocation: Option<AlphaAllocationReport>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConflictBody {
    contract_version: String,
    kind: String,
    documents: Vec<Value>,
}

#[derive(Default)]
struct Evidence {
    budgets: BTreeMap<String, AlphaAllocationDeclaration>,
    records: Vec<AlphaReservation>,
    conflicts: BTreeMap<String, Vec<Value>>,
}

fn invalid(message: impl std::fmt::Display) -> LedgerError {
    LedgerError::StateInvalid(format!("alpha registry: {message}"))
}

fn bounded(raw: &str, maximum: usize) -> Result<(), LedgerError> {
    if raw.len() > maximum {
        return Err(invalid(format!("document exceeds {maximum} bytes")));
    }
    Ok(())
}

fn rows(conn: &Connection) -> Result<i64, LedgerError> {
    Ok(conn.query_row("SELECT (SELECT COUNT(*) FROM alpha_family_budgets) + (SELECT COUNT(*) FROM alpha_reservations) + (SELECT COUNT(*) FROM alpha_conflicts)", [], |row| row.get(0))?)
}

fn capacity(
    conn: &Connection,
    additional: usize,
    additional_bytes: usize,
) -> Result<(), LedgerError> {
    if rows(conn)? + additional as i64 > MAX_ALPHA_ROWS {
        return Err(invalid(
            "evidence row limit exceeded; history is never truncated",
        ));
    }
    let bytes: i64 = conn.query_row("SELECT COALESCE((SELECT SUM(length(CAST(document_json AS BLOB))) FROM alpha_family_budgets),0) + COALESCE((SELECT SUM(length(CAST(record_json AS BLOB))) FROM alpha_reservations),0) + COALESCE((SELECT SUM(length(CAST(detail_json AS BLOB))) FROM alpha_conflicts),0)", [], |row| row.get(0))?;
    if bytes + additional_bytes as i64 > MAX_ALPHA_BYTES {
        return Err(invalid(
            "evidence byte limit exceeded; history is never truncated",
        ));
    }
    Ok(())
}

fn known_family(conn: &Connection, family: &str) -> Result<(), LedgerError> {
    let key: Option<String> = conn
        .query_row(
            "SELECT family_key FROM trial_families WHERE family_id = ?1",
            [family],
            |row| row.get(0),
        )
        .optional()?;
    let key = key.ok_or_else(|| invalid("unknown trial family"))?;
    let document: Value = serde_json::from_str(&key)?;
    let instrument = document["instrumentId"]
        .as_str()
        .ok_or_else(|| invalid("invalid trial family key"))?;
    let (expected_id, expected_key) = super::family_parts(instrument)?;
    if expected_id != family || expected_key != key {
        return Err(invalid("trial family key does not reproduce its ID"));
    }
    Ok(())
}

fn budget_document(
    declaration: &AlphaAllocationDeclaration,
) -> Result<(String, String), LedgerError> {
    let id = alpha_allocation_id(declaration).map_err(invalid)?;
    let raw = canonical_text(&declaration.document())?;
    bounded(&raw, MAX_BUDGET_BYTES)?;
    Ok((id, raw))
}

fn budget_evidence(
    family: &str,
    declaration: &AlphaAllocationDeclaration,
) -> Result<Value, LedgerError> {
    Ok(
        json!({"familyId":family, "allocationId":alpha_allocation_id(declaration).map_err(invalid)?, "declaration":declaration.document()}),
    )
}

fn conflict_identity(body: &Value) -> Result<(String, String), LedgerError> {
    let raw = canonical_text(body)?;
    bounded(&raw, MAX_CONFLICT_BYTES)?;
    Ok((
        sha256_hex(format!("{CONFLICT_VERSION}\0{raw}").as_bytes()),
        raw,
    ))
}

fn conflict_families(body: &ConflictBody) -> Result<BTreeSet<String>, LedgerError> {
    if body.contract_version != CONFLICT_VERSION || body.documents.len() != 2 {
        return Err(invalid("invalid conflict shape"));
    }
    let mut encoded = Vec::new();
    let mut families = BTreeSet::new();
    let mut records = Vec::new();
    for document in &body.documents {
        encoded.push(canonical_text(document)?);
        if body.kind == "budget" {
            let family = document["familyId"]
                .as_str()
                .ok_or_else(|| invalid("conflict family missing"))?;
            let declaration = parse_alpha_allocation(&document["declaration"]).map_err(invalid)?;
            if budget_evidence(family, &declaration)? != *document {
                return Err(invalid("invalid conflict budget"));
            }
            families.insert(family.to_string());
        } else {
            let record = parse_alpha_reservation(&document.to_string()).map_err(invalid)?;
            families.insert(record.family_id().to_string());
            records.push(record);
        }
    }
    if encoded[0] >= encoded[1] {
        return Err(invalid("conflict documents must be distinct and sorted"));
    }
    match body.kind.as_str() {
        "budget" if families.len() == 1 => (),
        "origin_key"
            if records[0].origin_registry_id() == records[1].origin_registry_id()
                && records[0].reservation_key() == records[1].reservation_key() => {}
        "confirmation_number"
            if families.len() == 1
                && records[0].confirmation_number() == records[1].confirmation_number() => {}
        _ => return Err(invalid("conflict documents do not prove their kind")),
    }
    Ok(families)
}

fn load(conn: &Connection) -> Result<Evidence, LedgerError> {
    capacity(conn, 0, 0)?;
    let mut evidence = Evidence::default();
    let mut stmt = conn.prepare("SELECT family_id, allocation_id, document_json FROM alpha_family_budgets ORDER BY family_id")?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (family, id, raw) = row?;
        bounded(&raw, MAX_BUDGET_BYTES)?;
        known_family(conn, &family)?;
        let declaration =
            parse_alpha_allocation(&serde_json::from_str::<Value>(&raw)?).map_err(invalid)?;
        if budget_document(&declaration)? != (id, raw) {
            return Err(invalid("stored budget identity/content mismatch"));
        }
        evidence.budgets.insert(family, declaration);
    }
    let mut stmt = conn.prepare("SELECT reservation_id, family_id, record_json FROM alpha_reservations ORDER BY reservation_id")?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (id, family, raw) = row?;
        bounded(&raw, MAX_RECORD_BYTES)?;
        let record = parse_alpha_reservation(&raw).map_err(invalid)?;
        let budget = evidence
            .budgets
            .get(&family)
            .ok_or_else(|| invalid("reservation has no budget"))?;
        if record.reservation_id() != id
            || record.family_id() != family
            || canonical_text(&record.document())? != raw
            || record.allocation_id() != alpha_allocation_id(budget).map_err(invalid)?
            || budget
                .schedule
                .get(record.confirmation_number() as usize - 1)
                != Some(&record.alpha_ppm())
        {
            return Err(invalid("stored reservation binding/share mismatch"));
        }
        evidence.records.push(record);
    }
    let mut groups: BTreeMap<String, (Value, BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    let mut stmt = conn.prepare("SELECT conflict_id, family_id, detail_json FROM alpha_conflicts ORDER BY conflict_id, family_id")?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (id, family, raw) = row?;
        bounded(&raw, MAX_CONFLICT_BYTES)?;
        let body: ConflictBody = serde_json::from_str(&raw)?;
        let document: Value = serde_json::from_str(&raw)?;
        let expected = conflict_families(&body)?;
        if conflict_identity(&document)? != (id.clone(), raw)
            || !expected.contains(&family)
            || !evidence.budgets.contains_key(&family)
        {
            return Err(invalid("stored conflict identity/binding mismatch"));
        }
        let group = groups
            .entry(id)
            .or_insert_with(|| (document.clone(), expected.clone(), BTreeSet::new()));
        if group.0 != document || group.1 != expected {
            return Err(invalid("inconsistent conflict copies"));
        }
        group.2.insert(family);
    }
    for (_, (body, expected, actual)) in groups {
        if expected != actual {
            return Err(invalid("conflict missing an affected family"));
        }
        for family in actual {
            evidence
                .conflicts
                .entry(family)
                .or_default()
                .push(body.clone());
        }
    }
    Ok(evidence)
}

fn family_state(
    evidence: &Evidence,
    family: &str,
) -> Result<Option<AlphaFamilyState>, LedgerError> {
    let Some(declaration) = evidence.budgets.get(family) else {
        return Ok(None);
    };
    let records: Vec<_> = evidence
        .records
        .iter()
        .filter(|r| r.family_id() == family)
        .cloned()
        .collect();
    let history = review_alpha_reservations(family, declaration, &records).map_err(invalid)?;
    let persistent_conflicts = evidence.conflicts.get(family).cloned().unwrap_or_default();
    // A source operation cannot be reassigned to a different family either.
    let mut operations = BTreeMap::new();
    for record in &evidence.records {
        *operations
            .entry((record.origin_registry_id(), record.reservation_key()))
            .or_insert(0usize) += 1;
    }
    let cross_family_collision = records
        .iter()
        .any(|record| operations[&(record.origin_registry_id(), record.reservation_key())] > 1);
    let blocked = !persistent_conflicts.is_empty()
        || history.status == AlphaReservationHistoryStatus::Blocked
        || cross_family_collision;
    Ok(Some(AlphaFamilyState {
        declaration: declaration.clone(),
        reservations: history.evidence,
        persistent_conflicts,
        derived_conflicts: history.conflicts,
        blocked,
        allocation: if blocked { None } else { history.allocation },
    }))
}

fn store_record(tx: &Transaction<'_>, record: &AlphaReservation) -> Result<(), LedgerError> {
    let raw = canonical_text(&record.document())?;
    bounded(&raw, MAX_RECORD_BYTES)?;
    capacity(tx, 1, raw.len())?;
    tx.execute("INSERT INTO alpha_reservations (reservation_id,family_id,record_json,recorded_at) VALUES (?1,?2,?3,?4)",
        params![record.reservation_id(), record.family_id(), raw, chrono::Utc::now().to_rfc3339()])?;
    Ok(())
}

fn store_conflict(
    tx: &Transaction<'_>,
    kind: &str,
    documents: Vec<Value>,
) -> Result<(), LedgerError> {
    let mut sorted = documents
        .into_iter()
        .map(|document| Ok((canonical_text(&document)?, document)))
        .collect::<Result<Vec<_>, LedgerError>>()?;
    sorted.sort_by(|left, right| left.0.cmp(&right.0));
    let body = json!({"contractVersion": CONFLICT_VERSION,"kind":kind,"documents":sorted.into_iter().map(|(_,d)| d).collect::<Vec<_>>()});
    let families = conflict_families(&serde_json::from_value(body.clone())?)?;
    let (id, raw) = conflict_identity(&body)?;
    capacity(tx, families.len(), raw.len() * families.len())?;
    for family in families {
        tx.execute("INSERT OR IGNORE INTO alpha_conflicts (conflict_id,family_id,detail_json,recorded_at) VALUES (?1,?2,?3,?4)", params![id,family,raw,chrono::Utc::now().to_rfc3339()])?;
    }
    Ok(())
}

pub(super) fn has_evidence(conn: &Connection) -> Result<bool, LedgerError> {
    Ok(rows(conn)? != 0)
}

impl TrialLedger {
    /// No reservation/admission authority: immutable budget storage only.
    pub fn declare_alpha_budget(
        &self,
        family: &str,
        declaration: &AlphaAllocationDeclaration,
    ) -> Result<AlphaStorageStatus, LedgerError> {
        let (id, raw) = budget_document(declaration)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        known_family(&tx, family)?;
        let evidence = load(&tx)?;
        if let Some(existing) = evidence.budgets.get(family) {
            if existing == declaration {
                return Ok(
                    if family_state(&evidence, family)?
                        .expect("budget exists")
                        .blocked
                    {
                        AlphaStorageStatus::Blocked
                    } else {
                        AlphaStorageStatus::Replayed
                    },
                );
            }
            if family_state(&evidence, family)?
                .expect("budget exists")
                .blocked
            {
                return Ok(AlphaStorageStatus::Blocked);
            }
            store_conflict(
                &tx,
                "budget",
                vec![
                    budget_evidence(family, existing)?,
                    budget_evidence(family, declaration)?,
                ],
            )?;
            tx.commit()?;
            return Ok(AlphaStorageStatus::Blocked);
        }
        capacity(&tx, 1, raw.len())?;
        tx.execute("INSERT INTO alpha_family_budgets (family_id,allocation_id,document_json,recorded_at) VALUES (?1,?2,?3,?4)",params![family,id,raw,chrono::Utc::now().to_rfc3339()])?;
        tx.commit()?;
        Ok(AlphaStorageStatus::Stored)
    }

    /// Complete current alpha evidence, not a freshness/rollback proof.
    pub fn read_alpha_family(&self, family: &str) -> Result<Option<AlphaFamilyState>, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        known_family(&tx, family)?;
        let result = family_state(&load(&tx)?, family)?;
        tx.commit()?;
        Ok(result)
    }

    /// Host-agnostic storage primitive. Commits a spent share atomically;
    /// has NO production caller and does not check campaign/precision/fence
    /// or expose data. P13c must wrap the transaction-scoped operation with
    /// those checks before any confirmation execution can be admitted.
    pub fn reserve_alpha_evidence(
        &self,
        family: &str,
        reservation_key: &str,
        batch_id: &str,
    ) -> Result<AlphaStorageOutcome, LedgerError> {
        for (value, length) in [(reservation_key, 32), (batch_id, 64)] {
            if value.len() != length
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(invalid(
                    "operation key/batch ID must be canonical lowercase hex",
                ));
            }
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        known_family(&tx, family)?;
        let evidence = load(&tx)?;
        let state = family_state(&evidence, family)?
            .ok_or_else(|| invalid("family has no alpha budget"))?;
        if state.blocked {
            return Ok(AlphaStorageOutcome {
                status: AlphaStorageStatus::Blocked,
                reservation: None,
            });
        }
        let existing = evidence.records.iter().find(|record| {
            record.origin_registry_id() == self.registry_id()
                && record.reservation_key() == reservation_key
        });
        if let Some(record) = existing {
            if record.family_id() == family && record.confirmation_batch_id() == batch_id {
                return Ok(AlphaStorageOutcome {
                    status: AlphaStorageStatus::Replayed,
                    reservation: Some(record.clone()),
                });
            }
        }
        let allocation = state.allocation.expect("consistent state has allocation");
        let number = match existing {
            // Reassignment evidence stays at the original slot within its
            // family. Never renumber it as a successful next reservation.
            Some(previous) if previous.family_id() == family => previous.confirmation_number(),
            Some(_) => allocation.confirmation_number.unwrap_or(1),
            None => match allocation.confirmation_number {
                Some(number) => number,
                None => {
                    return Ok(AlphaStorageOutcome {
                        status: AlphaStorageStatus::Exhausted,
                        reservation: None,
                    })
                }
            },
        };
        let share = state.declaration.schedule[number as usize - 1];
        let record = freeze_alpha_reservation(&json!({"contractVersion":ALPHA_RESERVATION_VERSION,
            "familyId":family,"allocationId":allocation.allocation_id,"originRegistryId":self.registry_id(),"reservationKey":reservation_key,
            "confirmationBatchId":batch_id,"confirmationNumber":number,"alphaPpm":share}).to_string()).map_err(invalid)?;
        if let Some(previous) = existing {
            store_record(&tx, &record)?;
            store_conflict(
                &tx,
                "origin_key",
                vec![previous.document(), record.document()],
            )?;
            tx.commit()?;
            return Ok(AlphaStorageOutcome {
                status: AlphaStorageStatus::Blocked,
                reservation: None,
            });
        }
        store_record(&tx, &record)?;
        tx.commit()?;
        Ok(AlphaStorageOutcome {
            status: AlphaStorageStatus::Stored,
            reservation: Some(record),
        })
    }
}

#[cfg(test)]
#[path = "trial_ledger_alpha/tests.rs"]
mod tests;
