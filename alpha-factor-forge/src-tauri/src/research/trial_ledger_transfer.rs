//! Complete, versioned registry export and validated event-union import
//! (trial-ledger-v1 §8). Kept separate from registration so the wire format
//! and its validation can be reviewed together.
//!
//! v2 (§20) also carries the permanent evidence a v1 file dropped: family
//! quarantines, registry-origin conflicts, and verified origin genesis. A
//! transfer can therefore neither release a quarantine nor launder a known
//! divergent origin, and an empty source can still be restored.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    batch_id_for, benchmark_params_hash, canonical_text, event_id_for, family_parts, genesis, head,
    next_chain, prepare_batch, sha256_hex, verify_chain, LedgerError, LedgerIntegrity,
    TrialBatchInput, TrialEventInput, TrialKind, TrialLedger, TrialOrigin, REGISTRY_MIGRATIONS,
    REPRODUCTION_IDENTITY, TRIAL_FAMILY_VERSION,
};

/// v1 files cannot say whether their source had quarantined a family, so
/// importing one could silently release that quarantine; they are refused.
pub const TRIAL_LEDGER_EXPORT_VERSION: &str = "trial-ledger-export-v2";

/// Detail recorded when an import sees an origin checkpoint contradict one
/// already held (or this registry's own chain).
const ORIGIN_DIVERGED_DETAIL: &str = "checkpoint_diverged_or_incomplete";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportHeader {
    version: String,
    registry_id: String,
    schema_version: String,
    exported_at: String,
    family_count: usize,
    batch_count: usize,
    event_count: usize,
    head_seq: u64,
    head_chain: String,
    body_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum ExportRecord {
    Family {
        family_id: String,
        family_key: String,
        protocol_json: Option<String>,
    },
    Batch {
        batch_id: String,
        family_id: Option<String>,
        origin_registry_id: String,
        members: Vec<(String, String)>,
    },
    Event {
        event_id: String,
        payload: Value,
        origin_registry_id: String,
        batch_id: String,
        seq: u64,
        chain: String,
    },
    Receipt {
        batch_id: String,
        receipt_registry_id: String,
        family_effective_before: u64,
        batch_effective_trials: u64,
    },
    OriginCheckpoint {
        origin_registry_id: String,
        origin_seq: u64,
        event_id: String,
        origin_chain: String,
    },
    /// Another registry whose complete export the source validated, directly
    /// or transitively: proof of that origin's empty (seq 0) prefix.
    OriginGenesis { origin_registry_id: String },
    /// One distinct `family_conflicts` row; any one quarantines the family.
    FamilyConflict {
        family_id: String,
        kind: String,
        detail_json: String,
    },
    /// One distinct `registry_conflicts` row; the origin can never again
    /// authorize a workspace's registry replacement.
    RegistryConflict {
        origin_registry_id: String,
        detail_json: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportSummary {
    pub source_registry_id: String,
    pub added_events: u64,
    pub skipped_events: u64,
    pub conflicts: u64,
}

fn integrity(message: impl Into<String>) -> LedgerError {
    LedgerError::ImportIntegrityFailed(message.into())
}

fn mark_family_conflict(
    conflicts: &mut BTreeMap<String, Vec<String>>,
    family: Option<&str>,
    detail: String,
) -> Result<(), LedgerError> {
    let id = family.ok_or_else(|| integrity(format!("unknown-family conflict: {detail}")))?;
    conflicts.entry(id.into()).or_default().push(detail);
    Ok(())
}

fn lower_hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_registry_id(id: &str) -> bool {
    lower_hex(id, 32)
}

/// A conflict may name a family the file has no rows for (its conflicting
/// rows were never written), so only the identity's shape is checkable.
fn valid_family_id(id: &str) -> bool {
    id.strip_prefix(TRIAL_FAMILY_VERSION)
        .and_then(|rest| rest.strip_prefix(':'))
        .is_some_and(|digest| lower_hex(digest, 64))
}

fn canonical_detail(detail: &str) -> Result<(), LedgerError> {
    let value: Value =
        serde_json::from_str(detail).map_err(|_| integrity("conflict detail is not JSON"))?;
    if canonical_text(&value)? != detail {
        return Err(integrity("conflict detail is not canonical JSON"));
    }
    Ok(())
}

/// Append a family conflict unless the identical evidence is already held,
/// so repeated imports of the same file stay idempotent.
fn record_family_conflict(
    tx: &rusqlite::Transaction<'_>,
    family_id: &str,
    kind: &str,
    detail_json: &str,
    now: &str,
) -> Result<(), LedgerError> {
    tx.execute(
        "INSERT INTO family_conflicts (family_id, kind, detail_json, recorded_at)
         SELECT ?1, ?2, ?3, ?4 WHERE NOT EXISTS (
             SELECT 1 FROM family_conflicts
              WHERE family_id = ?1 AND kind = ?2 AND detail_json = ?3)",
        params![family_id, kind, detail_json, now],
    )?;
    Ok(())
}

fn record_registry_conflict(
    tx: &rusqlite::Transaction<'_>,
    origin: &str,
    detail_json: &str,
    now: &str,
) -> Result<(), LedgerError> {
    tx.execute(
        "INSERT INTO registry_conflicts (origin_registry_id, detail_json, recorded_at)
         SELECT ?1, ?2, ?3 WHERE NOT EXISTS (
             SELECT 1 FROM registry_conflicts
              WHERE origin_registry_id = ?1 AND detail_json = ?2)",
        params![origin, detail_json, now],
    )?;
    Ok(())
}

fn record_line(record: &ExportRecord) -> Result<String, LedgerError> {
    canonical_text(&serde_json::to_value(record)?)
}

impl TrialLedger {
    /// A complete JSON Lines snapshot in FK order. The body hash covers the
    /// exact bytes after the header, including each terminating newline.
    pub fn export_json_lines(&self) -> Result<Vec<u8>, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        if !matches!(
            verify_chain(&tx, &self.registry_id)?,
            LedgerIntegrity::Intact { .. }
        ) {
            return Err(LedgerError::StateInvalid(
                "cannot export a registry with a broken event chain".into(),
            ));
        }
        let mut body = String::new();
        let mut family_count = 0;
        let mut batch_count = 0;
        let mut event_count = 0;

        {
            let mut stmt = tx.prepare(
                "SELECT family_id, family_key, protocol_json FROM trial_families ORDER BY family_id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(ExportRecord::Family {
                    family_id: row.get(0)?,
                    family_key: row.get(1)?,
                    protocol_json: row.get(2)?,
                })
            })?;
            for row in rows {
                body.push_str(&record_line(&row?)?);
                body.push('\n');
                family_count += 1;
            }
        }
        {
            let mut stmt = tx.prepare(
                "SELECT batch_id, family_id, origin_registry_id, members_json
                   FROM trial_batches ORDER BY batch_id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            for row in rows {
                let (batch_id, family_id, origin_registry_id, members) = row?;
                let record = ExportRecord::Batch {
                    batch_id,
                    family_id,
                    origin_registry_id,
                    members: serde_json::from_str(&members)?,
                };
                body.push_str(&record_line(&record)?);
                body.push('\n');
                batch_count += 1;
            }
        }
        {
            let mut stmt = tx.prepare(
                "SELECT event_id, payload_json, origin_registry_id, batch_id, seq, chain
                   FROM trial_events ORDER BY seq",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?;
            for row in rows {
                let (event_id, payload, origin_registry_id, batch_id, seq, chain) = row?;
                let record = ExportRecord::Event {
                    event_id,
                    payload: serde_json::from_str(&payload)?,
                    origin_registry_id,
                    batch_id,
                    seq: seq as u64,
                    chain,
                };
                body.push_str(&record_line(&record)?);
                body.push('\n');
                event_count += 1;
            }
        }
        {
            let mut stmt = tx.prepare(
                "SELECT batch_id, receipt_registry_id, family_effective_before,
                        batch_effective_trials FROM batch_receipts
                   ORDER BY batch_id, receipt_registry_id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(ExportRecord::Receipt {
                    batch_id: row.get(0)?,
                    receipt_registry_id: row.get(1)?,
                    family_effective_before: row.get::<_, i64>(2)? as u64,
                    batch_effective_trials: row.get::<_, i64>(3)? as u64,
                })
            })?;
            for row in rows {
                body.push_str(&record_line(&row?)?);
                body.push('\n');
            }
        }
        {
            let mut stmt = tx.prepare(
                "SELECT origin_registry_id, origin_seq, event_id, origin_chain
                   FROM origin_checkpoints ORDER BY origin_registry_id, origin_seq",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(ExportRecord::OriginCheckpoint {
                    origin_registry_id: row.get(0)?,
                    origin_seq: row.get::<_, i64>(1)? as u64,
                    event_id: row.get(2)?,
                    origin_chain: row.get(3)?,
                })
            })?;
            for row in rows {
                body.push_str(&record_line(&row?)?);
                body.push('\n');
            }
        }
        // Permanent evidence (§20). Conflicts are exported once per distinct
        // content; their timestamps are local audit data, not identity.
        {
            let mut stmt = tx.prepare(
                "SELECT origin_registry_id FROM origin_genesis ORDER BY origin_registry_id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(ExportRecord::OriginGenesis {
                    origin_registry_id: row.get(0)?,
                })
            })?;
            for row in rows {
                body.push_str(&record_line(&row?)?);
                body.push('\n');
            }
        }
        {
            let mut stmt = tx.prepare(
                "SELECT DISTINCT family_id, kind, detail_json FROM family_conflicts
                   ORDER BY family_id, kind, detail_json",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(ExportRecord::FamilyConflict {
                    family_id: row.get(0)?,
                    kind: row.get(1)?,
                    detail_json: row.get(2)?,
                })
            })?;
            for row in rows {
                body.push_str(&record_line(&row?)?);
                body.push('\n');
            }
        }
        {
            let mut stmt = tx.prepare(
                "SELECT DISTINCT origin_registry_id, detail_json FROM registry_conflicts
                   ORDER BY origin_registry_id, detail_json",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(ExportRecord::RegistryConflict {
                    origin_registry_id: row.get(0)?,
                    detail_json: row.get(1)?,
                })
            })?;
            for row in rows {
                body.push_str(&record_line(&row?)?);
                body.push('\n');
            }
        }
        let (head_seq, head_chain) = head(&tx, &self.registry_id)?;
        let header = ExportHeader {
            version: TRIAL_LEDGER_EXPORT_VERSION.into(),
            registry_id: self.registry_id.clone(),
            schema_version: REGISTRY_MIGRATIONS.last().expect("schema exists").0.into(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            family_count,
            batch_count,
            event_count,
            head_seq,
            head_chain,
            body_sha256: sha256_hex(body.as_bytes()),
        };
        let mut output = canonical_text(&serde_json::to_value(header)?)?.into_bytes();
        output.push(b'\n');
        output.extend_from_slice(body.as_bytes());
        tx.commit()?;
        Ok(output)
    }
}

type BatchRecord = (Option<String>, String, Vec<(String, String)>);

struct ValidatedExport {
    header: ExportHeader,
    file_sha256: String,
    families: BTreeMap<String, (String, Option<String>)>,
    batches: BTreeMap<String, BatchRecord>,
    events: Vec<(String, Value, String, String, u64, String)>,
    receipts: BTreeMap<(String, String), (u64, u64)>,
    checkpoints: BTreeMap<(String, u64), (String, String)>,
    genesis: BTreeSet<String>,
    family_conflicts: BTreeSet<(String, String, String)>,
    registry_conflicts: BTreeSet<(String, String)>,
}

impl ValidatedExport {
    fn parse(bytes: &[u8]) -> Result<Self, LedgerError> {
        let text = std::str::from_utf8(bytes).map_err(|_| integrity("not UTF-8"))?;
        let (header_line, body) = text
            .split_once('\n')
            .ok_or_else(|| integrity("missing header line"))?;
        let header: ExportHeader =
            serde_json::from_str(header_line).map_err(|e| integrity(format!("header: {e}")))?;
        if canonical_text(&serde_json::to_value(&header)?)? != header_line {
            return Err(integrity("header is not canonical JSON"));
        }
        if header.version != TRIAL_LEDGER_EXPORT_VERSION
            || header.schema_version != REGISTRY_MIGRATIONS.last().expect("schema exists").0
            || !valid_registry_id(&header.registry_id)
        {
            return Err(integrity("unsupported export header"));
        }
        if sha256_hex(body.as_bytes()) != header.body_sha256 {
            return Err(integrity("bodySha256 does not match"));
        }

        let mut families = BTreeMap::new();
        let mut batches = BTreeMap::new();
        let mut events = Vec::new();
        let mut receipts = BTreeMap::new();
        let mut checkpoints = BTreeMap::new();
        let mut genesis_origins = BTreeSet::new();
        let mut family_conflicts = BTreeSet::new();
        let mut registry_conflicts = BTreeSet::new();
        let mut section = 0;
        for (line_no, line) in body.split_inclusive('\n').enumerate() {
            let line = line
                .strip_suffix('\n')
                .ok_or_else(|| integrity("body needs a final newline"))?;
            if line.is_empty() {
                return Err(integrity(format!("empty body line {}", line_no + 2)));
            }
            let record: ExportRecord = serde_json::from_str(line)
                .map_err(|e| integrity(format!("line {}: {e}", line_no + 2)))?;
            if record_line(&record)? != line {
                return Err(integrity(format!("line {} is not canonical", line_no + 2)));
            }
            let order = match &record {
                ExportRecord::Family { .. } => 0,
                ExportRecord::Batch { .. } => 1,
                ExportRecord::Event { .. } => 2,
                ExportRecord::Receipt { .. } => 3,
                ExportRecord::OriginCheckpoint { .. } => 4,
                ExportRecord::OriginGenesis { .. } => 5,
                ExportRecord::FamilyConflict { .. } => 6,
                ExportRecord::RegistryConflict { .. } => 7,
            };
            if order < section {
                return Err(integrity("record types are not in FK order"));
            }
            section = order;
            match record {
                ExportRecord::Family {
                    family_id,
                    family_key,
                    protocol_json,
                } => {
                    let key: Value = serde_json::from_str(&family_key)
                        .map_err(|_| integrity("invalid familyKey"))?;
                    if canonical_text(&key)? != family_key {
                        return Err(integrity("familyKey is not canonical"));
                    }
                    let instrument = key["instrumentId"]
                        .as_str()
                        .ok_or_else(|| integrity("familyKey lacks instrumentId"))?;
                    if family_parts(instrument)
                        .map_err(|_| integrity("invalid family instrumentId"))?
                        != (family_id.clone(), family_key.clone())
                    {
                        return Err(integrity("familyId does not match familyKey"));
                    }
                    if let Some(protocol) = &protocol_json {
                        let value: Value = serde_json::from_str(protocol)
                            .map_err(|_| integrity("invalid family protocol"))?;
                        let tests = value["testsPerTrial"]
                            .as_u64()
                            .ok_or_else(|| integrity("family protocol has no testsPerTrial"))?;
                        if tests == 0
                            || tests
                                > alpha_factor_forge::discovery_core::precision::PRECISION_MAX_COUNT
                            || canonical_text(&json!({"correction":"holm","testsPerTrial":tests}))?
                                != *protocol
                        {
                            return Err(integrity("invalid family protocol"));
                        }
                    }
                    if families
                        .insert(family_id, (family_key, protocol_json))
                        .is_some()
                    {
                        return Err(integrity("duplicate family"));
                    }
                }
                ExportRecord::Batch {
                    batch_id,
                    family_id,
                    origin_registry_id,
                    members,
                } => {
                    if !valid_registry_id(&origin_registry_id)
                        || family_id
                            .as_ref()
                            .is_some_and(|id| !families.contains_key(id))
                        || members.is_empty()
                        || !members.windows(2).all(|pair| pair[0].0 < pair[1].0)
                        || batch_id_for(family_id.as_deref(), &json!(members))? != batch_id
                    {
                        return Err(integrity("invalid batch identity or members"));
                    }
                    if batches
                        .insert(batch_id, (family_id, origin_registry_id, members))
                        .is_some()
                    {
                        return Err(integrity("duplicate batch"));
                    }
                }
                ExportRecord::Event {
                    event_id,
                    payload,
                    origin_registry_id,
                    batch_id,
                    seq,
                    chain,
                } => {
                    let payload_json = canonical_text(&payload)?;
                    if !valid_registry_id(&origin_registry_id)
                        || event_id_for(&payload_json) != event_id
                        || !batches.contains_key(&batch_id)
                    {
                        return Err(integrity("invalid event identity or batch"));
                    }
                    events.push((event_id, payload, origin_registry_id, batch_id, seq, chain));
                }
                ExportRecord::Receipt {
                    batch_id,
                    receipt_registry_id,
                    family_effective_before,
                    batch_effective_trials,
                } => {
                    if !batches.contains_key(&batch_id)
                        || !valid_registry_id(&receipt_registry_id)
                        || family_effective_before > i64::MAX as u64
                        || batch_effective_trials > i64::MAX as u64
                    {
                        return Err(integrity("receipt references an unknown batch"));
                    }
                    if receipts
                        .insert(
                            (batch_id, receipt_registry_id),
                            (family_effective_before, batch_effective_trials),
                        )
                        .is_some()
                    {
                        return Err(integrity("duplicate receipt"));
                    }
                }
                ExportRecord::OriginCheckpoint {
                    origin_registry_id,
                    origin_seq,
                    event_id,
                    origin_chain,
                } => {
                    if !valid_registry_id(&origin_registry_id)
                        || origin_seq == 0
                        || origin_seq > i64::MAX as u64
                    {
                        return Err(integrity("invalid origin checkpoint"));
                    }
                    if checkpoints
                        .insert((origin_registry_id, origin_seq), (event_id, origin_chain))
                        .is_some()
                    {
                        return Err(integrity("duplicate origin checkpoint"));
                    }
                }
                ExportRecord::OriginGenesis { origin_registry_id } => {
                    if !valid_registry_id(&origin_registry_id)
                        || origin_registry_id == header.registry_id
                        || !genesis_origins.insert(origin_registry_id)
                    {
                        return Err(integrity("invalid or duplicate origin genesis"));
                    }
                }
                ExportRecord::FamilyConflict {
                    family_id,
                    kind,
                    detail_json,
                } => {
                    canonical_detail(&detail_json)?;
                    if !valid_family_id(&family_id)
                        || kind.trim().is_empty()
                        || !family_conflicts.insert((family_id, kind, detail_json))
                    {
                        return Err(integrity("invalid or duplicate family conflict"));
                    }
                }
                ExportRecord::RegistryConflict {
                    origin_registry_id,
                    detail_json,
                } => {
                    canonical_detail(&detail_json)?;
                    if !valid_registry_id(&origin_registry_id)
                        || !registry_conflicts.insert((origin_registry_id, detail_json))
                    {
                        return Err(integrity("invalid or duplicate registry conflict"));
                    }
                }
            }
        }
        if families.len() != header.family_count
            || batches.len() != header.batch_count
            || events.len() != header.event_count
            || header.head_seq != events.len() as u64
        {
            return Err(integrity("header counts do not match body"));
        }
        let represented: BTreeSet<&String> = batches
            .values()
            .filter_map(|(id, _, _)| id.as_ref())
            .collect();
        if families.keys().any(|family| !represented.contains(family)) {
            return Err(integrity("family has no batch"));
        }
        let mut chain = genesis(&header.registry_id);
        let mut members_by_batch: BTreeMap<&str, Vec<(String, String)>> = BTreeMap::new();
        let mut event_ids = BTreeSet::new();
        for (index, (id, payload, _, batch_id, seq, stored_chain)) in events.iter().enumerate() {
            if *seq != index as u64 + 1 || !event_ids.insert(id.clone()) {
                return Err(integrity("event sequence or ID is duplicated"));
            }
            chain = next_chain(&chain, id);
            if chain != *stored_chain {
                return Err(integrity("source event chain does not follow"));
            }
            let family = &batches[batch_id].0;
            if payload["familyId"].as_str() != family.as_deref()
                || payload["idempotencyKey"].as_str().is_none()
                || payload["kind"].as_str().is_none()
                || payload["effective"].as_bool().is_none()
            {
                return Err(integrity("event payload has invalid identity fields"));
            }
            members_by_batch.entry(batch_id).or_default().push((
                payload["idempotencyKey"].as_str().unwrap().into(),
                id.clone(),
            ));
        }
        if chain != header.head_chain {
            return Err(integrity("headChain does not match events"));
        }
        for (batch_id, (_, _, members)) in &batches {
            let mut observed = members_by_batch
                .remove(batch_id.as_str())
                .unwrap_or_default();
            observed.sort();
            if observed != *members || !receipts.keys().any(|(id, _)| id == batch_id) {
                return Err(integrity("batch members or receipt are incomplete"));
            }
        }
        for ((origin, seq), (event_id, stored_chain)) in &checkpoints {
            let previous = if *seq == 1 {
                genesis(origin)
            } else {
                checkpoints
                    .get(&(origin.clone(), seq - 1))
                    .ok_or_else(|| integrity("origin checkpoints have a gap"))?
                    .1
                    .clone()
            };
            if !event_ids.contains(event_id) || next_chain(&previous, event_id) != *stored_chain {
                return Err(integrity("origin checkpoint chain does not follow"));
            }
        }
        Ok(Self {
            header,
            file_sha256: sha256_hex(bytes),
            families,
            batches,
            events,
            receipts,
            checkpoints,
            genesis: genesis_origins,
            family_conflicts,
            registry_conflicts,
        })
    }

    fn validate_kinds(&self, tx: &rusqlite::Transaction<'_>) -> Result<(), LedgerError> {
        self.validate_batch_payloads()?;
        let by_id: BTreeMap<&str, &Value> = self
            .events
            .iter()
            .map(|(id, payload, _, _, _, _)| (id.as_str(), payload))
            .collect();
        let mut keys = BTreeSet::new();
        let mut effective_by_batch: BTreeMap<&str, u64> = BTreeMap::new();
        for (_, payload, _, batch_id, _, _) in &self.events {
            let kind = payload["kind"]
                .as_str()
                .ok_or_else(|| integrity("event kind missing"))?;
            let effective = payload["effective"]
                .as_bool()
                .ok_or_else(|| integrity("event effective missing"))?;
            let key = payload["idempotencyKey"]
                .as_str()
                .ok_or_else(|| integrity("event idempotencyKey missing"))?;
            if !keys.insert(key) {
                return Err(integrity("duplicate idempotencyKey in export"));
            }
            if effective {
                *effective_by_batch.entry(batch_id).or_default() += 1;
            }
            match kind {
                "benchmark" => {
                    let id = payload["benchmarkId"]
                        .as_str()
                        .ok_or_else(|| integrity("benchmarkId missing"))?;
                    if benchmark_params_hash(id).as_deref()
                        != payload["benchmarkParamsHash"].as_str()
                    {
                        return Err(integrity("benchmark whitelist or params hash mismatch"));
                    }
                    for field in [
                        "strategyHash",
                        "datasetHash",
                        "splitHash",
                        "seedsHash",
                        "engineFingerprintHash",
                    ] {
                        if payload[field].as_str().is_none() {
                            return Err(integrity(format!("benchmark {field} missing")));
                        }
                    }
                    if !effective {
                        return Err(integrity(
                            "non-effective benchmark lacks portable execution provenance",
                        ));
                    }
                }
                "reproduction" => {
                    if effective {
                        return Err(integrity("reproduction cannot be effective"));
                    }
                    let reference = payload["reproductionOf"]
                        .as_str()
                        .ok_or_else(|| integrity("reproductionOf missing"))?;
                    let local: Option<String> = tx
                        .query_row(
                            "SELECT payload_json FROM trial_events WHERE event_id = ?1",
                            [reference],
                            |row| row.get(0),
                        )
                        .optional()?;
                    let original = match by_id.get(reference) {
                        Some(value) => (*value).clone(),
                        None => serde_json::from_str(
                            &local.ok_or_else(|| integrity("reproduction reference missing"))?,
                        )?,
                    };
                    if original["familyId"] != payload["familyId"]
                        || REPRODUCTION_IDENTITY.iter().any(|field| {
                            payload[field].as_str().is_none() || payload[field] != original[field]
                        })
                    {
                        return Err(integrity("reproduction identity differs"));
                    }
                }
                "hypothesis" | "variant" | "diagnostic" | "legacy" => {
                    if !effective {
                        return Err(integrity("counted trial kind is non-effective"));
                    }
                }
                _ => return Err(integrity("unknown trial kind")),
            }
        }
        for (batch_id, (family_id, _, _)) in &self.batches {
            if let Some(family_id) = family_id {
                if effective_by_batch
                    .get(batch_id.as_str())
                    .copied()
                    .unwrap_or(0)
                    > 0
                    && self.families[family_id].1.is_none()
                {
                    return Err(integrity("effective family has no pinned protocol"));
                }
            }
            let expected = effective_by_batch
                .get(batch_id.as_str())
                .copied()
                .unwrap_or(0);
            for ((receipt_batch, _), (_, recorded)) in &self.receipts {
                if receipt_batch == batch_id && *recorded != expected {
                    return Err(integrity("receipt effective count differs from batch"));
                }
            }
        }
        Ok(())
    }

    fn validate_batch_payloads(&self) -> Result<(), LedgerError> {
        const FIELDS: [&str; 19] = [
            "version",
            "familyId",
            "kind",
            "effective",
            "idempotencyKey",
            "workspaceId",
            "requestId",
            "candidateIndex",
            "legacyAttemptKey",
            "hypothesisHash",
            "strategyHash",
            "datasetHash",
            "snapshotId",
            "splitHash",
            "seedsHash",
            "engineFingerprintHash",
            "benchmarkId",
            "benchmarkParamsHash",
            "reproductionOf",
        ];
        let optional = |payload: &Value, name: &str| -> Result<Option<String>, LedgerError> {
            if payload[name].is_null() {
                Ok(None)
            } else {
                payload[name]
                    .as_str()
                    .map(|value| Some(value.to_string()))
                    .ok_or_else(|| integrity(format!("{name} must be text or null")))
            }
        };
        let mut rows_by_batch: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
        let mut payload_by_id: BTreeMap<&str, &Value> = BTreeMap::new();
        for (id, payload, _, batch_id, _, _) in &self.events {
            rows_by_batch.entry(batch_id).or_default().push(payload);
            payload_by_id.insert(id, payload);
        }
        for (batch_id, (family_id, _, members)) in &self.batches {
            let rows = rows_by_batch
                .get(batch_id.as_str())
                .ok_or_else(|| integrity("batch has no events"))?;
            let first = rows
                .first()
                .ok_or_else(|| integrity("batch has no events"))?;
            let workspace_id = first["workspaceId"]
                .as_str()
                .ok_or_else(|| integrity("workspaceId missing"))?
                .to_string();
            let instrument_id = family_id.as_ref().map(|id| {
                let key: Value = serde_json::from_str(&self.families[id].0).expect("validated key");
                key["instrumentId"]
                    .as_str()
                    .expect("validated instrument")
                    .to_string()
            });
            let tests_per_trial = family_id
                .as_ref()
                .and_then(|id| self.families[id].1.as_ref())
                .map(|protocol| {
                    let value: Value = serde_json::from_str(protocol).expect("validated protocol");
                    value["testsPerTrial"]
                        .as_u64()
                        .expect("validated testsPerTrial")
                })
                .unwrap_or(1);
            let mut events = Vec::with_capacity(rows.len());
            for payload in rows {
                let object = payload
                    .as_object()
                    .ok_or_else(|| integrity("event payload is not an object"))?;
                if object.len() != FIELDS.len()
                    || FIELDS.iter().any(|field| !object.contains_key(*field))
                {
                    return Err(integrity("event payload fields differ from trial-event-v1"));
                }
                let kind = match payload["kind"].as_str() {
                    Some("hypothesis") => TrialKind::Hypothesis,
                    Some("variant") => TrialKind::Variant,
                    Some("diagnostic") => TrialKind::Diagnostic,
                    Some("benchmark") => TrialKind::Benchmark,
                    Some("reproduction") => TrialKind::Reproduction,
                    Some("legacy") => TrialKind::Legacy,
                    _ => return Err(integrity("unknown trial kind")),
                };
                let origin = if let Some(attempt_key) = payload["legacyAttemptKey"].as_str() {
                    TrialOrigin::Legacy {
                        attempt_key: attempt_key.into(),
                    }
                } else {
                    TrialOrigin::Request {
                        request_id: payload["requestId"]
                            .as_str()
                            .ok_or_else(|| integrity("requestId missing"))?
                            .into(),
                        candidate_index: payload["candidateIndex"]
                            .as_u64()
                            .ok_or_else(|| integrity("candidateIndex missing"))?,
                    }
                };
                events.push(TrialEventInput {
                    kind,
                    origin,
                    hypothesis_hash: optional(payload, "hypothesisHash")?,
                    strategy_hash: optional(payload, "strategyHash")?,
                    dataset_hash: optional(payload, "datasetHash")?,
                    snapshot_id: optional(payload, "snapshotId")?,
                    split_hash: optional(payload, "splitHash")?,
                    seeds_hash: optional(payload, "seedsHash")?,
                    engine_fingerprint_hash: optional(payload, "engineFingerprintHash")?,
                    benchmark_id: optional(payload, "benchmarkId")?,
                    benchmark_params_hash: optional(payload, "benchmarkParamsHash")?,
                    reproduction_of: optional(payload, "reproductionOf")?,
                    benchmark_evidence: None,
                });
            }
            let prepared = prepare_batch(&TrialBatchInput {
                workspace_id,
                instrument_id,
                tests_per_trial,
                events,
            })
            .map_err(|error| integrity(format!("batch {batch_id}: {error}")))?;
            if prepared.batch_id != *batch_id
                || prepared.members_json != canonical_text(&json!(members))?
                || prepared.events.iter().any(|event| {
                    payload_by_id
                        .get(event.event_id.as_str())
                        .is_none_or(|payload| event.payload != **payload)
                })
            {
                return Err(integrity(format!(
                    "batch {batch_id} payload does not reproduce"
                )));
            }
        }
        Ok(())
    }
}

impl TrialLedger {
    /// Validate the entire file before any write, then union non-conflicting
    /// families inside one immediate transaction. Existing events keep their
    /// local chain positions; new ones append in source sequence order.
    pub fn import_json_lines(&self, bytes: &[u8]) -> Result<ImportSummary, LedgerError> {
        let source = ValidatedExport::parse(bytes)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !matches!(
            verify_chain(&tx, &self.registry_id)?,
            LedgerIntegrity::Intact { .. }
        ) {
            return Err(LedgerError::StateInvalid(
                "cannot import into a registry with a broken event chain".into(),
            ));
        }
        source.validate_kinds(&tx)?;

        let mut quarantined = BTreeSet::new();
        {
            let mut stmt = tx.prepare("SELECT DISTINCT family_id FROM family_conflicts")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                quarantined.insert(row?);
            }
        }
        let mut family_conflicts: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (family_id, (key, protocol)) in &source.families {
            let existing: Option<(String, Option<String>)> = tx
                .query_row(
                    "SELECT family_key, protocol_json FROM trial_families WHERE family_id = ?1",
                    [family_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((old_key, old_protocol)) = existing {
                if old_key != *key
                    || old_protocol
                        .as_ref()
                        .zip(protocol.as_ref())
                        .is_some_and(|(a, b)| a != b)
                {
                    mark_family_conflict(
                        &mut family_conflicts,
                        Some(family_id),
                        "family key or pinned protocol differs".into(),
                    )?;
                }
            }
        }
        for (batch_id, (family_id, _, members)) in &source.batches {
            let old: Option<(Option<String>, String)> = tx
                .query_row(
                    "SELECT family_id, members_json FROM trial_batches WHERE batch_id = ?1",
                    [batch_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((old_family, old_members)) = old {
                if old_family != *family_id || old_members != canonical_text(&json!(members))? {
                    mark_family_conflict(
                        &mut family_conflicts,
                        family_id.as_deref(),
                        format!("batch {batch_id} differs"),
                    )?;
                }
            }
        }
        for (event_id, payload, _, batch_id, _, _) in &source.events {
            let key = payload["idempotencyKey"].as_str().unwrap();
            let family = source.batches[batch_id].0.as_deref();
            let old: Option<(String, String, Option<String>)> = tx
                .query_row(
                    "SELECT event_id, batch_id, family_id FROM trial_events WHERE idempotency_key = ?1",
                    [key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            if let Some((old_id, old_batch, old_family)) = old {
                if old_id != *event_id || old_batch != *batch_id {
                    mark_family_conflict(
                        &mut family_conflicts,
                        family,
                        format!("idempotencyKey {key} differs"),
                    )?;
                    if old_family.as_deref() != family {
                        mark_family_conflict(
                            &mut family_conflicts,
                            old_family.as_deref(),
                            format!("idempotencyKey {key} reused"),
                        )?;
                    }
                }
            }
            let old_by_id: Option<(String, String)> = tx
                .query_row(
                    "SELECT idempotency_key, batch_id FROM trial_events WHERE event_id = ?1",
                    [event_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((old_key, old_batch)) = old_by_id {
                if old_key != key || old_batch != *batch_id {
                    mark_family_conflict(
                        &mut family_conflicts,
                        family,
                        format!("eventId {event_id} belongs to another batch"),
                    )?;
                }
            }
        }
        for ((batch_id, receipt_registry_id), values) in &source.receipts {
            let old: Option<(i64, i64)> = tx
                .query_row(
                    "SELECT family_effective_before, batch_effective_trials
                       FROM batch_receipts WHERE batch_id = ?1 AND receipt_registry_id = ?2",
                    params![batch_id, receipt_registry_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some(old) = old {
                if old != (values.0 as i64, values.1 as i64) {
                    mark_family_conflict(
                        &mut family_conflicts,
                        source.batches[batch_id].0.as_deref(),
                        format!("receipt {batch_id}/{receipt_registry_id} differs"),
                    )?;
                }
            }
        }
        // Rows that conflict now, or belong to a family this registry had
        // already quarantined, are not written (§8.2 step 2).
        quarantined.extend(family_conflicts.keys().cloned());

        let now = chrono::Utc::now().to_rfc3339();
        for (family_id, details) in &family_conflicts {
            record_family_conflict(
                &tx,
                family_id,
                "import_conflict",
                &canonical_text(&json!(details))?,
                &now,
            )?;
        }
        // A quarantine the source carried stays permanent here (§20). Its
        // family's rows do not conflict with this registry, so they are still
        // unioned: the trials stay counted and the batch stays addressable,
        // and admission reports the quarantine instead of a count.
        for (family_id, kind, detail_json) in &source.family_conflicts {
            record_family_conflict(&tx, family_id, kind, detail_json, &now)?;
        }
        for (family_id, (key, protocol)) in &source.families {
            if quarantined.contains(family_id) {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO trial_families (family_id, family_key, protocol_json, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![family_id, key, protocol, now],
            )?;
            if let Some(protocol) = protocol {
                tx.execute(
                    "UPDATE trial_families SET protocol_json = ?2
                      WHERE family_id = ?1 AND protocol_json IS NULL",
                    params![family_id, protocol],
                )?;
            }
        }
        for (batch_id, (family_id, origin, members)) in &source.batches {
            if family_id
                .as_ref()
                .is_some_and(|id| quarantined.contains(id))
            {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO trial_batches
                    (batch_id, family_id, members_json, origin_registry_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    batch_id,
                    family_id,
                    canonical_text(&json!(members))?,
                    origin,
                    now
                ],
            )?;
        }
        let (mut seq, mut chain) = head(&tx, &self.registry_id)?;
        let mut added = 0;
        let mut skipped = 0;
        // A skipped event that is absent here leaves a gap in the source
        // chain; the presence check below then withholds that origin's
        // checkpoints. An already-present event of a quarantined family is
        // not a gap, so re-importing a source still extends its checkpoints.
        for (event_id, payload, origin, batch_id, _, _) in &source.events {
            let family = source.batches[batch_id].0.as_deref();
            let exists = tx
                .query_row(
                    "SELECT 1 FROM trial_events WHERE event_id = ?1",
                    [event_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if exists || family.is_some_and(|id| quarantined.contains(id)) {
                skipped += 1;
                continue;
            }
            seq += 1;
            chain = next_chain(&chain, event_id);
            tx.execute(
                "INSERT INTO trial_events
                    (event_id, seq, chain, family_id, idempotency_key, kind, effective,
                     batch_id, payload_json, origin_registry_id, registered_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    event_id,
                    seq as i64,
                    chain,
                    family,
                    payload["idempotencyKey"].as_str().unwrap(),
                    payload["kind"].as_str().unwrap(),
                    payload["effective"].as_bool().unwrap() as i64,
                    batch_id,
                    canonical_text(payload)?,
                    origin,
                    now,
                ],
            )?;
            added += 1;
        }
        for ((batch_id, receipt_registry_id), (before, batch_effective)) in &source.receipts {
            let family = source.batches[batch_id].0.as_deref();
            if family.is_some_and(|id| quarantined.contains(id)) {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO batch_receipts
                    (batch_id, receipt_registry_id, family_effective_before, batch_effective_trials)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    batch_id,
                    receipt_registry_id,
                    *before as i64,
                    *batch_effective as i64
                ],
            )?;
        }

        // The source registry's own chain is a verified checkpoint for every
        // exported event. Imported checkpoints may establish other origins.
        let mut checkpoints = source.checkpoints.clone();
        for (event_id, _, _, _, seq, chain) in &source.events {
            let key = (source.header.registry_id.clone(), *seq);
            let value = (event_id.clone(), chain.clone());
            if checkpoints
                .insert(key, value.clone())
                .is_some_and(|old| old != value)
            {
                return Err(integrity("source checkpoint contradicts its own chain"));
            }
        }
        // Divergence seen by THIS import (counted in the summary).
        let mut blocked_origins = BTreeSet::new();
        for ((origin, origin_seq), (event_id, origin_chain)) in &checkpoints {
            let old: Option<(String, String)> = tx
                .query_row(
                    "SELECT event_id, origin_chain FROM origin_checkpoints
                      WHERE origin_registry_id = ?1 AND origin_seq = ?2",
                    params![origin, *origin_seq as i64],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if old.is_some_and(|old| old != (event_id.clone(), origin_chain.clone())) {
                blocked_origins.insert(origin.clone());
            }
            if origin == &self.registry_id {
                let local: Option<String> = tx
                    .query_row(
                        "SELECT chain FROM trial_events WHERE seq = ?1",
                        [*origin_seq as i64],
                        |row| row.get(0),
                    )
                    .optional()?;
                if local.is_some_and(|local| local != *origin_chain) {
                    blocked_origins.insert(origin.clone());
                }
            }
        }
        // A conflict is permanent (§8.2, §20): one recorded by an earlier
        // import or carried in by this file blocks the origin as surely as
        // one seen now, so no later import can add evidence for it.
        let mut conflicted_origins: BTreeSet<String> = {
            let mut stmt =
                tx.prepare("SELECT DISTINCT origin_registry_id FROM registry_conflicts")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        conflicted_origins.extend(source.registry_conflicts.iter().map(|(id, _)| id.clone()));
        conflicted_origins.extend(blocked_origins.iter().cloned());
        let mut incomplete_origins = BTreeSet::new();
        for ((origin, _), (event_id, _)) in &checkpoints {
            let present = tx
                .query_row(
                    "SELECT 1 FROM trial_events WHERE event_id = ?1",
                    [event_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !present {
                incomplete_origins.insert(origin.clone());
            }
        }
        let diverged_detail = canonical_text(&json!({"reason": ORIGIN_DIVERGED_DETAIL}))?;
        for origin in &blocked_origins {
            record_registry_conflict(&tx, origin, &diverged_detail, &now)?;
        }
        for (origin, detail_json) in &source.registry_conflicts {
            record_registry_conflict(&tx, origin, detail_json, &now)?;
        }
        for ((origin, origin_seq), (event_id, origin_chain)) in &checkpoints {
            if conflicted_origins.contains(origin)
                || incomplete_origins.contains(origin)
                || origin == &self.registry_id
            {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO origin_checkpoints
                    (origin_registry_id, origin_seq, event_id, origin_chain)
                 VALUES (?1, ?2, ?3, ?4)",
                params![origin, *origin_seq as i64, event_id, origin_chain],
            )?;
        }
        // Genesis evidence (§20): the validated source itself, every origin it
        // held genesis or checkpoints for. An empty prefix needs no events, so
        // an incomplete origin still qualifies; a conflicted one never does.
        let mut genesis_origins = source.genesis.clone();
        genesis_origins.insert(source.header.registry_id.clone());
        genesis_origins.extend(source.checkpoints.keys().map(|(origin, _)| origin.clone()));
        for origin in &genesis_origins {
            if conflicted_origins.contains(origin) || origin == &self.registry_id {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO origin_genesis (origin_registry_id, recorded_at)
                 VALUES (?1, ?2)",
                params![origin, now],
            )?;
        }
        let conflicts = (family_conflicts.len() + blocked_origins.len()) as u64;
        tx.execute(
            "INSERT INTO registry_imports
                (source_registry_id, file_sha256, head_seq, head_chain,
                 added_events, skipped_events, conflicts, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                source.header.registry_id,
                source.file_sha256,
                source.header.head_seq as i64,
                source.header.head_chain,
                added as i64,
                skipped as i64,
                conflicts as i64,
                now,
            ],
        )?;
        if !matches!(
            verify_chain(&tx, &self.registry_id)?,
            LedgerIntegrity::Intact { .. }
        ) {
            return Err(integrity(
                "union would break local event or batch identities",
            ));
        }
        tx.commit()?;
        Ok(ImportSummary {
            source_registry_id: source.header.registry_id,
            added_events: added,
            skipped_events: skipped,
            conflicts,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::super::{
        run_and_attest_deterministic_benchmark, Admission, AdmissionBlocked, BenchmarkContext,
        TrialBatchInput, TrialEventInput, TrialKind, TrialOrigin,
    };
    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);
    const BTC: &str = "crypto:binance:BTCUSDT";

    struct Dirs {
        root: PathBuf,
        registry: PathBuf,
        workspace: PathBuf,
    }

    impl Dirs {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "aff-transfer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let registry = root.join("evidence");
            let workspace = root.join("workspace");
            std::fs::create_dir_all(&workspace).unwrap();
            Self {
                root,
                registry,
                workspace,
            }
        }

        fn open(&self) -> TrialLedger {
            TrialLedger::open(&self.registry, &self.workspace).unwrap()
        }
    }

    impl Drop for Dirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn batch(request: &str) -> TrialBatchInput {
        TrialBatchInput {
            workspace_id: "ws".into(),
            instrument_id: Some(BTC.into()),
            tests_per_trial: 2,
            events: vec![TrialEventInput {
                kind: TrialKind::Variant,
                origin: TrialOrigin::Request {
                    request_id: request.into(),
                    candidate_index: 0,
                },
                hypothesis_hash: None,
                strategy_hash: Some(format!("strategy-{request}")),
                dataset_hash: Some("dataset".into()),
                snapshot_id: Some("snapshot".into()),
                split_hash: Some(super::super::split_hash_for(&json!("split")).unwrap()),
                seeds_hash: Some("seeds".into()),
                engine_fingerprint_hash: Some("engine".into()),
                benchmark_id: None,
                benchmark_params_hash: None,
                reproduction_of: None,
                benchmark_evidence: None,
            }],
        }
    }

    fn event_rows(ledger: &TrialLedger) -> i64 {
        ledger
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM trial_events", [], |row| row.get(0))
            .unwrap()
    }

    /// Rebuild all content IDs and the source chain after changing a payload.
    /// This makes the kind tests exercise semantic validation, not a stale hash.
    fn rewrite_export(bytes: &[u8], mut change: impl FnMut(&mut Value)) -> Vec<u8> {
        let text = std::str::from_utf8(bytes).unwrap();
        let mut lines: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut header = lines.remove(0);
        let mut event_ids = BTreeMap::new();
        for row in &mut lines {
            if row["type"] == "event" {
                let old = row["eventId"].as_str().unwrap().to_string();
                change(&mut row["payload"]);
                let payload = canonical_text(&row["payload"]).unwrap();
                let new = event_id_for(&payload);
                row["eventId"] = json!(new);
                event_ids.insert(old, new);
            }
        }
        let mut batch_ids = BTreeMap::new();
        for row in &mut lines {
            if row["type"] == "batch" {
                let old = row["batchId"].as_str().unwrap().to_string();
                for member in row["members"].as_array_mut().unwrap() {
                    let original = member[1].as_str().unwrap();
                    member[1] = json!(event_ids.get(original).unwrap());
                }
                let new = batch_id_for(row["familyId"].as_str(), &row["members"]).unwrap();
                row["batchId"] = json!(new);
                batch_ids.insert(old, new);
            }
        }
        let mut chain = genesis(header["registryId"].as_str().unwrap());
        for row in &mut lines {
            match row["type"].as_str().unwrap() {
                "event" => {
                    row["batchId"] = json!(batch_ids[row["batchId"].as_str().unwrap()]);
                    chain = next_chain(&chain, row["eventId"].as_str().unwrap());
                    row["chain"] = json!(chain);
                }
                "receipt" => row["batchId"] = json!(batch_ids[row["batchId"].as_str().unwrap()]),
                _ => {}
            }
        }
        header["headChain"] = json!(chain);
        let mut body = String::new();
        for row in lines {
            body.push_str(&canonical_text(&row).unwrap());
            body.push('\n');
        }
        header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
        let mut output = canonical_text(&header).unwrap();
        output.push('\n');
        output.push_str(&body);
        output.into_bytes()
    }

    fn rewrite_records(bytes: &[u8], mut change: impl FnMut(&mut Value)) -> Vec<u8> {
        let text = std::str::from_utf8(bytes).unwrap();
        let mut lines: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut header = lines.remove(0);
        let mut body = String::new();
        for mut row in lines {
            change(&mut row);
            body.push_str(&canonical_text(&row).unwrap());
            body.push('\n');
        }
        header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
        format!("{}\n{body}", canonical_text(&header).unwrap()).into_bytes()
    }

    #[test]
    fn a13_a21_full_export_import_replays_receipt_and_duplicate_is_a_noop() {
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        let original = source.register_batch(&batch("r1")).unwrap();
        let bytes = source.export_json_lines().unwrap();
        let imported = target.import_json_lines(&bytes).unwrap();
        assert_eq!(imported.added_events, 1);
        assert_eq!(imported.conflicts, 0);
        assert_eq!(event_rows(&target), 1);
        let checkpoint: (String, String) = target
            .lock()
            .unwrap()
            .query_row(
                "SELECT event_id, origin_chain FROM origin_checkpoints
              WHERE origin_registry_id = ?1 AND origin_seq = 1",
                [source.registry_id()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(checkpoint.0, original.event_ids[0]);
        assert_eq!(checkpoint.1, source.chain_at(1).unwrap().unwrap());
        let replay = target.register_batch(&batch("r1")).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.event_ids, original.event_ids);
        assert_eq!(replay.registration_receipt, original.registration_receipt);
        assert!(matches!(replay.admission, Admission::Count(_)));
        let again = target.import_json_lines(&bytes).unwrap();
        assert_eq!(
            (again.added_events, again.skipped_events, again.conflicts),
            (0, 1, 0)
        );
        assert_eq!(event_rows(&target), 1);
        assert!(
            target
                .export_json_lines()
                .unwrap()
                .split(|b| *b == b'\n')
                .count()
                > 3
        );
    }

    #[test]
    fn a14_two_registries_union_both_event_sets_without_max_count() {
        let a = Dirs::new();
        let b = Dirs::new();
        let left = a.open();
        let right = b.open();
        left.register_batch(&batch("left")).unwrap();
        right.register_batch(&batch("right")).unwrap();
        let left_file = left.export_json_lines().unwrap();
        let right_file = right.export_json_lines().unwrap();
        assert_eq!(left.import_json_lines(&right_file).unwrap().added_events, 1);
        assert_eq!(right.import_json_lines(&left_file).unwrap().added_events, 1);
        assert_eq!(event_rows(&left), 2);
        assert_eq!(event_rows(&right), 2);
        let l = left
            .read_admission_count(&left.register_batch(&batch("left")).unwrap().batch_id)
            .unwrap();
        let r = right
            .read_admission_count(&right.register_batch(&batch("right")).unwrap().batch_id)
            .unwrap();
        match (l, r) {
            (Admission::Count(l), Admission::Count(r)) => {
                assert_eq!(l.family_effective_trials(), 2);
                assert_eq!(r.family_effective_trials(), 2);
            }
            other => panic!("expected counts: {other:?}"),
        }
    }

    #[test]
    fn a16_one_bit_change_fails_without_writes() {
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        source.register_batch(&batch("r1")).unwrap();
        let mut bytes = source.export_json_lines().unwrap();
        let last = bytes.len() - 2;
        bytes[last] ^= 1;
        assert_eq!(
            target.import_json_lines(&bytes).err().unwrap().code(),
            "import_integrity_failed"
        );
        assert_eq!(event_rows(&target), 0);
    }

    #[test]
    fn a25_reproduction_identity_is_rechecked_even_when_all_hashes_are_rebuilt() {
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        let first = source.register_batch(&batch("original")).unwrap();
        let mut copy = batch("copy");
        copy.events[0].kind = TrialKind::Reproduction;
        copy.events[0].strategy_hash = Some("strategy-original".into());
        copy.events[0].reproduction_of = Some(first.event_ids[0].clone());
        source.register_batch(&copy).unwrap();
        let bytes = rewrite_export(&source.export_json_lines().unwrap(), |payload| {
            if payload["kind"] == "reproduction" {
                payload["seedsHash"] = json!("different-seed");
            }
        });
        assert_eq!(
            target.import_json_lines(&bytes).err().unwrap().code(),
            "import_integrity_failed"
        );
        assert_eq!(event_rows(&target), 0);
    }

    #[test]
    fn exact_reproduction_imports_without_raising_the_effective_count() {
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        let first = source.register_batch(&batch("original")).unwrap();
        let mut copy = batch("copy");
        copy.events[0].kind = TrialKind::Reproduction;
        copy.events[0].strategy_hash = Some("strategy-original".into());
        copy.events[0].reproduction_of = Some(first.event_ids[0].clone());
        let original_receipt = source.register_batch(&copy).unwrap().registration_receipt;
        assert_eq!(original_receipt.batch_effective_trials, 0);
        assert_eq!(
            target
                .import_json_lines(&source.export_json_lines().unwrap())
                .unwrap()
                .added_events,
            2
        );
        assert_eq!(event_rows(&target), 2);
        let replay = target.register_batch(&copy).unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.registration_receipt, original_receipt);
        assert_eq!(
            replay.admission,
            Admission::Blocked(AdmissionBlocked::NoEffectiveTrials)
        );
    }

    #[test]
    fn a26_benchmark_whitelist_is_rechecked_even_when_all_hashes_are_rebuilt() {
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        let mut claim = batch("benchmark");
        claim.events[0].kind = TrialKind::Benchmark;
        claim.events[0].benchmark_id = Some("smaCross".into());
        claim.events[0].benchmark_params_hash = benchmark_params_hash("smaCross");
        source.register_batch(&claim).unwrap(); // unverified, so effective
        let bytes = rewrite_export(&source.export_json_lines().unwrap(), |payload| {
            payload["benchmarkId"] = json!("momentum");
        });
        assert_eq!(
            target.import_json_lines(&bytes).err().unwrap().code(),
            "import_integrity_failed"
        );
        assert_eq!(event_rows(&target), 0);
    }

    #[test]
    fn a_rehashed_variant_missing_required_seed_is_rejected_before_writes() {
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        source.register_batch(&batch("r1")).unwrap();
        let bytes = rewrite_export(&source.export_json_lines().unwrap(), |payload| {
            payload["seedsHash"] = Value::Null;
        });
        assert_eq!(
            target.import_json_lines(&bytes).err().unwrap().code(),
            "import_integrity_failed"
        );
        assert_eq!(event_rows(&target), 0);
    }

    #[test]
    fn a15_same_idempotency_key_with_different_payload_quarantines_the_family() {
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        source.register_batch(&batch("r1")).unwrap();
        let mut clean = batch("other-family");
        clean.instrument_id = Some("crypto:binance:ETHUSDT".into());
        source.register_batch(&clean).unwrap();
        let mut changed = batch("r1");
        changed.events[0].strategy_hash = Some("different".into());
        let registered = target.register_batch(&changed).unwrap();
        let result = target
            .import_json_lines(&source.export_json_lines().unwrap())
            .unwrap();
        assert_eq!(result.added_events, 1, "unrelated family still imports");
        assert!(result.conflicts >= 1);
        assert_eq!(event_rows(&target), 2);
        assert_eq!(
            target.read_admission_count(&registered.batch_id).unwrap(),
            Admission::Blocked(AdmissionBlocked::FamilyQuarantined)
        );
        source.register_batch(&batch("later")).unwrap();
        let later = target
            .import_json_lines(&source.export_json_lines().unwrap())
            .unwrap();
        assert_eq!(later.added_events, 0, "quarantine is permanent in v1");
        assert_eq!(event_rows(&target), 2);
    }

    #[test]
    fn review_quarantine_survives_export_into_replacement() {
        let a = Dirs::new();
        let b = Dirs::new();
        let c = Dirs::new();
        let source = a.open();
        let conflicting = b.open();
        let replacement = c.open();
        let registered = source.register_batch(&batch("same")).unwrap();
        let mut changed = batch("same");
        changed.events[0].strategy_hash = Some("different-strategy".into());
        conflicting.register_batch(&changed).unwrap();
        source.import_json_lines(&conflicting.export_json_lines().unwrap()).unwrap();
        assert_eq!(source.read_admission_count(&registered.batch_id).unwrap(),
            Admission::Blocked(AdmissionBlocked::FamilyQuarantined));
        replacement.import_json_lines(&source.export_json_lines().unwrap()).unwrap();
        assert_eq!(replacement.read_admission_count(&registered.batch_id).unwrap(),
            Admission::Blocked(AdmissionBlocked::FamilyQuarantined),
            "a supported export/import must not release permanent quarantine");
    }

    #[test]
    fn a29_same_event_from_two_registries_is_one_event_with_two_receipts() {
        let a = Dirs::new();
        let b = Dirs::new();
        let left = a.open();
        let right = b.open();
        let one = left.register_batch(&batch("same")).unwrap();
        let two = right.register_batch(&batch("same")).unwrap();
        assert_eq!(one.event_ids, two.event_ids);
        assert_eq!(
            left.import_json_lines(&right.export_json_lines().unwrap())
                .unwrap()
                .added_events,
            0
        );
        assert_eq!(event_rows(&left), 1);
        let receipts: i64 = left
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM batch_receipts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(receipts, 2);
    }

    #[test]
    fn a15_protocol_and_receipt_conflicts_quarantine_without_overwriting() {
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        let mut other_protocol = batch("source");
        other_protocol.tests_per_trial = 3;
        source.register_batch(&other_protocol).unwrap();
        target.register_batch(&batch("target")).unwrap();
        let result = target
            .import_json_lines(&source.export_json_lines().unwrap())
            .unwrap();
        assert_eq!(result.added_events, 0);
        assert!(result.conflicts >= 1);
        assert_eq!(event_rows(&target), 1);

        let c = Dirs::new();
        let d = Dirs::new();
        let source = c.open();
        let target = d.open();
        source.register_batch(&batch("r1")).unwrap();
        let original = source.export_json_lines().unwrap();
        target.import_json_lines(&original).unwrap();
        let forged = rewrite_records(&original, |row| {
            if row["type"] == "receipt" {
                row["familyEffectiveBefore"] = json!(99);
            }
        });
        let result = target.import_json_lines(&forged).unwrap();
        assert!(result.conflicts >= 1);
        let count: i64 = target
            .lock()
            .unwrap()
            .query_row(
                "SELECT family_effective_before FROM batch_receipts",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "historical receipt was not overwritten");
    }

    #[test]
    fn a_origin_checkpoint_divergence_records_registry_conflict() {
        let a = Dirs::new();
        let b = Dirs::new();
        let c = Dirs::new();
        let first = a.open();
        let target = b.open();
        let other = c.open();
        first.register_batch(&batch("first")).unwrap();
        target
            .import_json_lines(&first.export_json_lines().unwrap())
            .unwrap();
        other.register_batch(&batch("other")).unwrap();
        let original: Vec<Value> = std::str::from_utf8(&other.export_json_lines().unwrap())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut header = original[0].clone();
        header["registryId"] = json!(first.registry_id());
        let mut chain = genesis(first.registry_id());
        let mut body = String::new();
        for mut row in original.into_iter().skip(1) {
            if row["type"] == "event" {
                chain = next_chain(&chain, row["eventId"].as_str().unwrap());
                row["chain"] = json!(chain);
            }
            body.push_str(&canonical_text(&row).unwrap());
            body.push('\n');
        }
        header["headChain"] = json!(chain);
        header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
        let forged = format!("{}\n{body}", canonical_text(&header).unwrap());
        let result = target.import_json_lines(forged.as_bytes()).unwrap();
        assert_eq!(result.added_events, 1);
        assert!(result.conflicts >= 1);
        let conflicts: i64 = target
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM registry_conflicts", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(conflicts, 1);
    }

    #[test]
    fn review_divergent_origin_cannot_authorize_replacement() {
        use super::super::{BindingCheck, LedgerBinding};
        let a = Dirs::new();
        let b = Dirs::new();
        let c = Dirs::new();
        let first = a.open();
        let target = b.open();
        let other = c.open();
        first.register_batch(&batch("first")).unwrap();
        let saved = match first.check_binding(None).unwrap() {
            BindingCheck::Current(saved) => saved,
            other => panic!("{other:?}"),
        };
        target.import_json_lines(&first.export_json_lines().unwrap()).unwrap();
        other.register_batch(&batch("other")).unwrap();
        // Model a valid fork of the same registry, as in the existing A30 fixture.
        let rows: Vec<Value> = std::str::from_utf8(&other.export_json_lines().unwrap())
            .unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        let mut header = rows[0].clone();
        header["registryId"] = json!(first.registry_id());
        let mut chain = genesis(first.registry_id());
        let mut body = String::new();
        for mut row in rows.into_iter().skip(1) {
            if row["type"] == "event" {
                chain = next_chain(&chain, row["eventId"].as_str().unwrap());
                row["chain"] = json!(chain);
            }
            body.push_str(&canonical_text(&row).unwrap());
            body.push('\n');
        }
        header["headChain"] = json!(chain);
        header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
        let fork = format!("{}\n{body}", canonical_text(&header).unwrap());
        assert!(target.import_json_lines(fork.as_bytes()).unwrap().conflicts > 0);
        let saved: LedgerBinding = saved;
        assert!(!matches!(target.check_binding(Some(&saved)).unwrap(), BindingCheck::Current(_)),
            "a recorded source divergence must invalidate its existing replacement authority");
    }

    #[test]
    fn review_empty_export_can_restore_empty_workspace_binding() {
        use super::super::BindingCheck;
        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let replacement = b.open();
        let saved = match source.check_binding(None).unwrap() {
            BindingCheck::Current(saved) => saved,
            other => panic!("{other:?}"),
        };
        assert_eq!(saved.seq, 0);
        replacement.import_json_lines(&source.export_json_lines().unwrap()).unwrap();
        assert!(matches!(replacement.check_binding(Some(&saved)).unwrap(), BindingCheck::Current(_)),
            "a validated complete empty export proves the source genesis prefix");
    }

    #[test]
    fn non_effective_benchmark_import_fails_closed_without_portable_run_evidence() {
        use alpha_factor_forge::discovery_core::benchmarks::{BenchmarkCosts, RunBenchmarksArgs};
        use alpha_factor_forge::discovery_core::types::Candle;

        let a = Dirs::new();
        let b = Dirs::new();
        let source = a.open();
        let target = b.open();
        let candles = vec![Candle {
            timestamp: 0,
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.0,
            volume: 1.0,
        }];
        let attested = run_and_attest_deterministic_benchmark(
            "buyHold",
            &RunBenchmarksArgs {
                candles: &candles,
                interval: "1d",
                costs: BenchmarkCosts {
                    fee_pct: 0.0,
                    slip_pct: 0.0,
                },
                start_equity: None,
                from: None,
                to: None,
            },
            BenchmarkContext {
                workspace_id: "ws".into(),
                instrument_id: Some(BTC.into()),
                origin: TrialOrigin::Request {
                    request_id: "bench".into(),
                    candidate_index: 0,
                },
                dataset_hash: "dataset".into(),
                snapshot_id: Some("snapshot".into()),
                split_hash: "split".into(),
                seeds_hash: "seeds".into(),
                engine_fingerprint_hash: "engine".into(),
            },
        )
        .unwrap();
        let mut input = batch("bench");
        input.events[0] = attested;
        source.register_batch(&input).unwrap();
        assert_eq!(
            target
                .import_json_lines(&source.export_json_lines().unwrap())
                .err()
                .unwrap()
                .code(),
            "import_integrity_failed"
        );
        assert_eq!(event_rows(&target), 0);
    }

    // --------------------------- PR #116–#125 acceptance follow-ups (§20)

    use super::super::{BindingCheck, LedgerBinding};

    fn current(ledger: &TrialLedger) -> LedgerBinding {
        match ledger.check_binding(None).unwrap() {
            BindingCheck::Current(binding) => binding,
            other => panic!("expected a current binding, got {other:?}"),
        }
    }

    fn accepts(ledger: &TrialLedger, saved: &LedgerBinding) -> bool {
        match ledger.check_binding(Some(saved)).unwrap() {
            BindingCheck::Current(head) => {
                assert_eq!(head.registry_id, ledger.registry_id());
                true
            }
            BindingCheck::RegistryReplaced => false,
            other => panic!("unexpected binding state {other:?}"),
        }
    }

    fn rows(ledger: &TrialLedger, table: &str) -> i64 {
        ledger
            .lock()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    /// `export` rebuilt as a self-consistent history of `registry_id`: a valid
    /// fork of that registry, as in the origin-divergence fixture above.
    fn fork_as(registry_id: &str, export: &[u8]) -> Vec<u8> {
        let rows: Vec<Value> = std::str::from_utf8(export)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut header = rows[0].clone();
        header["registryId"] = json!(registry_id);
        let mut chain = genesis(registry_id);
        let mut body = String::new();
        for mut row in rows.into_iter().skip(1) {
            if row["type"] == "event" {
                chain = next_chain(&chain, row["eventId"].as_str().unwrap());
                row["chain"] = json!(chain);
            }
            body.push_str(&canonical_text(&row).unwrap());
            body.push('\n');
        }
        header["headChain"] = json!(chain);
        header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
        format!("{}\n{body}", canonical_text(&header).unwrap()).into_bytes()
    }

    /// Append records to a valid export and re-seal its body hash.
    fn with_records(export: &[u8], records: &[Value]) -> Vec<u8> {
        let text = std::str::from_utf8(export).unwrap();
        let (header, body) = text.split_once('\n').unwrap();
        let mut header: Value = serde_json::from_str(header).unwrap();
        let mut body = body.to_string();
        for record in records {
            body.push_str(&canonical_text(record).unwrap());
            body.push('\n');
        }
        header["bodySha256"] = json!(sha256_hex(body.as_bytes()));
        format!("{}\n{body}", canonical_text(&header).unwrap()).into_bytes()
    }

    #[test]
    fn r1_quarantine_survives_chained_and_repeated_transfers() {
        let (a, b, c, d) = (Dirs::new(), Dirs::new(), Dirs::new(), Dirs::new());
        let source = a.open();
        let conflicting = b.open();
        let first_hop = c.open();
        let second_hop = d.open();
        let quarantined = source.register_batch(&batch("same")).unwrap();
        let eth = |request: &str| TrialBatchInput {
            instrument_id: Some("crypto:binance:ETHUSDT".into()),
            ..batch(request)
        };
        let normal = source.register_batch(&eth("eth")).unwrap();
        let mut changed = batch("same");
        changed.events[0].strategy_hash = Some("different-strategy".into());
        conflicting.register_batch(&changed).unwrap();
        source
            .import_json_lines(&conflicting.export_json_lines().unwrap())
            .unwrap();

        let export = source.export_json_lines().unwrap();
        first_hop.import_json_lines(&export).unwrap();
        let again = first_hop.import_json_lines(&export).unwrap();
        assert_eq!((again.added_events, again.conflicts), (0, 0), "a repeat is a no-op");
        let hop = first_hop.export_json_lines().unwrap();
        second_hop.import_json_lines(&hop).unwrap();
        second_hop.import_json_lines(&hop).unwrap();
        for ledger in [&source, &first_hop, &second_hop] {
            assert_eq!(
                ledger.read_admission_count(&quarantined.batch_id).unwrap(),
                Admission::Blocked(AdmissionBlocked::FamilyQuarantined)
            );
            assert!(
                matches!(
                    ledger.read_admission_count(&normal.batch_id).unwrap(),
                    Admission::Count(_)
                ),
                "other families stay usable"
            );
            assert_eq!(
                ledger.register_batch(&batch("later")).err().unwrap().code(),
                "family_quarantined"
            );
            assert_eq!(event_rows(ledger), 2, "the quarantined trials stay counted");
            assert_eq!(rows(ledger, "family_conflicts"), 1, "evidence is unioned once");
        }
        let evidence = |bytes: Vec<u8>| -> Vec<String> {
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .filter(|line| line.contains("\"type\":\"familyConflict\""))
                .map(str::to_string)
                .collect()
        };
        let original = evidence(source.export_json_lines().unwrap());
        assert_eq!(original.len(), 1);
        assert_eq!(evidence(second_hop.export_json_lines().unwrap()), original);

        // An already-present event of the quarantined family is not a gap: a
        // newer export still extends the source's replacement checkpoints.
        source.register_batch(&eth("eth-2")).unwrap();
        let saved = current(&source);
        assert_eq!(saved.seq, 3);
        first_hop
            .import_json_lines(&source.export_json_lines().unwrap())
            .unwrap();
        assert!(accepts(&first_hop, &saved));
    }

    #[test]
    fn r1_v1_files_and_malformed_evidence_are_refused_before_writes() {
        let (a, b) = (Dirs::new(), Dirs::new());
        let source = a.open();
        let target = b.open();
        let registered = source.register_batch(&batch("r1")).unwrap();
        let export = source.export_json_lines().unwrap();
        let text = std::str::from_utf8(&export).unwrap();
        let (header, body) = text.split_once('\n').unwrap();
        let mut header: Value = serde_json::from_str(header).unwrap();
        assert_eq!(header["version"], TRIAL_LEDGER_EXPORT_VERSION);
        header["version"] = json!("trial-ledger-export-v1");
        let v1 = format!("{}\n{body}", canonical_text(&header).unwrap());
        assert_eq!(
            target.import_json_lines(v1.as_bytes()).err().unwrap().code(),
            "import_integrity_failed",
            "a v1 file cannot say whether its source quarantined a family"
        );

        let family = super::super::family_id_for(BTC).unwrap();
        let conflict = json!({"type": "familyConflict", "familyId": family,
            "kind": "import_conflict", "detailJson": "[\"seen elsewhere\"]"});
        for (label, records) in [
            ("non-canonical detail", vec![json!({"type": "familyConflict",
                "familyId": family, "kind": "import_conflict", "detailJson": "[ ]"})]),
            ("malformed family", vec![json!({"type": "familyConflict",
                "familyId": "trial-family-v1:xyz", "kind": "k", "detailJson": "[]"})]),
            ("duplicate conflict", vec![conflict.clone(), conflict.clone()]),
            ("malformed origin", vec![json!({"type": "registryConflict",
                "originRegistryId": "not-hex", "detailJson": "{}"})]),
            ("self genesis", vec![json!({"type": "originGenesis",
                "originRegistryId": source.registry_id()})]),
            ("section order", vec![conflict.clone(),
                json!({"type": "originGenesis", "originRegistryId": "0".repeat(32)})]),
        ] {
            assert_eq!(
                target
                    .import_json_lines(&with_records(&export, &records))
                    .err()
                    .unwrap()
                    .code(),
                "import_integrity_failed",
                "{label}"
            );
        }
        assert_eq!(event_rows(&target), 0);

        // Valid evidence the target never observed itself still quarantines.
        target
            .import_json_lines(&with_records(&export, &[conflict]))
            .unwrap();
        assert_eq!(
            target.read_admission_count(&registered.batch_id).unwrap(),
            Admission::Blocked(AdmissionBlocked::FamilyQuarantined)
        );
    }

    #[test]
    fn r2_a_divergent_origin_stays_refused_after_replay_reopen_and_transfer() {
        let (a, b, c, d) = (Dirs::new(), Dirs::new(), Dirs::new(), Dirs::new());
        let first = a.open();
        let other = c.open();
        first.register_batch(&batch("first")).unwrap();
        let saved = current(&first);
        let normal = first.export_json_lines().unwrap();
        let target = b.open();
        target.import_json_lines(&normal).unwrap();
        assert!(accepts(&target, &saved), "accepted before the divergence");

        other.register_batch(&batch("other")).unwrap();
        let fork = fork_as(first.registry_id(), &other.export_json_lines().unwrap());
        assert!(target.import_json_lines(&fork).unwrap().conflicts > 0);
        assert!(!accepts(&target, &saved));
        target.import_json_lines(&normal).unwrap();
        assert!(!accepts(&target, &saved), "replaying the normal export");
        drop(target);
        let target = b.open();
        assert!(!accepts(&target, &saved), "reopened");

        // A genuine later history of the origin adds no new authority.
        first.register_batch(&batch("second")).unwrap();
        let later = current(&first);
        target
            .import_json_lines(&first.export_json_lines().unwrap())
            .unwrap();
        assert!(!accepts(&target, &later));
        let newest: i64 = target
            .lock()
            .unwrap()
            .query_row(
                "SELECT MAX(origin_seq) FROM origin_checkpoints WHERE origin_registry_id = ?1",
                [first.registry_id()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(newest, 1, "no checkpoint is added for a conflicted origin");
        assert_eq!(rows(&target, "registry_conflicts"), 1);

        // A complete transfer carries the conflict to the next registry.
        let next = d.open();
        next.import_json_lines(&target.export_json_lines().unwrap())
            .unwrap();
        assert!(!accepts(&next, &saved));
        assert!(!accepts(&next, &later));
        let carried: i64 = next
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM origin_checkpoints WHERE origin_registry_id = ?1",
                [first.registry_id()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(carried, 0, "the conflicted origin's checkpoints are not carried");
    }

    #[test]
    fn r5_an_empty_origin_restores_through_transfers_only_with_evidence() {
        let (a, b, c, d) = (Dirs::new(), Dirs::new(), Dirs::new(), Dirs::new());
        let empty = a.open();
        let saved = current(&empty);
        assert_eq!(saved.seq, 0);
        let unrelated = d.open();
        assert!(!accepts(&unrelated, &saved), "seq 0 alone is not evidence");
        let first_hop = b.open();
        let second_hop = c.open();
        first_hop
            .import_json_lines(&empty.export_json_lines().unwrap())
            .unwrap();
        second_hop
            .import_json_lines(&first_hop.export_json_lines().unwrap())
            .unwrap();
        second_hop
            .import_json_lines(&first_hop.export_json_lines().unwrap())
            .unwrap();
        for ledger in [&first_hop, &second_hop] {
            assert!(accepts(ledger, &saved));
            let origins: Vec<String> = {
                let conn = ledger.lock().unwrap();
                let mut stmt = conn
                    .prepare("SELECT origin_registry_id FROM origin_genesis ORDER BY 1")
                    .unwrap();
                let ids = stmt.query_map([], |row| row.get(0)).unwrap();
                ids.collect::<Result<_, _>>().unwrap()
            };
            // Direct sources plus carried evidence, each exactly once.
            let mut expected = vec![empty.registry_id().to_string()];
            if std::ptr::eq(ledger, &second_hop) {
                expected.push(first_hop.registry_id().to_string());
            }
            expected.sort();
            assert_eq!(origins, expected);
        }
        let conn = first_hop.lock().unwrap();
        for sql in [
            "UPDATE origin_genesis SET recorded_at = 'x'",
            "DELETE FROM origin_genesis",
        ] {
            let error = conn.execute(sql, []).err().unwrap();
            assert!(error.to_string().contains("append-only"), "{sql}: {error}");
        }
    }

    #[test]
    fn r3_import_refuses_a_reproduction_of_an_unversioned_split() {
        let (a, b) = (Dirs::new(), Dirs::new());
        let source = a.open();
        let target = b.open();
        let first = source.register_batch(&batch("original")).unwrap();
        let mut copy = batch("copy");
        copy.events[0].kind = TrialKind::Reproduction;
        copy.events[0].strategy_hash = Some("strategy-original".into());
        copy.events[0].reproduction_of = Some(first.event_ids[0].clone());
        source.register_batch(&copy).unwrap();
        // Both events rewritten to one pre-§20 value: still equal, still not proof.
        let bytes = rewrite_export(&source.export_json_lines().unwrap(), |payload| {
            payload["splitHash"] = json!("0".repeat(64));
        });
        assert_eq!(
            target.import_json_lines(&bytes).err().unwrap().code(),
            "import_integrity_failed"
        );
        assert_eq!(event_rows(&target), 0);
    }

    #[test]
    fn r5_genesis_evidence_never_overrides_a_recorded_divergence() {
        let (a, b, c) = (Dirs::new(), Dirs::new(), Dirs::new());
        let origin = a.open();
        let empty = current(&origin);
        origin.register_batch(&batch("first")).unwrap();
        let target = b.open();
        target
            .import_json_lines(&origin.export_json_lines().unwrap())
            .unwrap();
        assert!(accepts(&target, &empty));
        let other = c.open();
        other.register_batch(&batch("other")).unwrap();
        let fork = fork_as(origin.registry_id(), &other.export_json_lines().unwrap());
        assert!(target.import_json_lines(&fork).unwrap().conflicts > 0);
        assert!(!accepts(&target, &empty));
    }

    // ------------------------------------ P12d-2b §6.4 freshness fence

    use super::super::{AdmissionFence, AdmissionSnapshot, FenceBlocked};

    /// `count` effective BTC variants in one batch.
    fn many(request: &str, count: u64) -> TrialBatchInput {
        let mut input = batch(request);
        let template = input.events.remove(0);
        input.events = (0..count)
            .map(|index| TrialEventInput {
                origin: TrialOrigin::Request {
                    request_id: request.into(),
                    candidate_index: index,
                },
                strategy_hash: Some(format!("strategy-{request}-{index}")),
                ..template.clone()
            })
            .collect();
        input
    }

    fn snapshot_of(admission: &Admission) -> AdmissionSnapshot {
        match admission {
            Admission::Count(count) => count.snapshot(),
            Admission::Blocked(reason) => panic!("admission blocked: {reason:?}"),
        }
    }

    #[test]
    fn admission_fence_rejects_another_equal_sized_batch_in_the_same_family() {
        let dirs = Dirs::new();
        let ledger = dirs.open();
        let first = ledger.register_batch(&many("batch-a", 1)).unwrap();
        let second = ledger.register_batch(&many("batch-b", 1)).unwrap();
        let earlier = snapshot_of(&ledger.read_admission_count(&first.batch_id).unwrap());
        assert!(matches!(
            ledger.fence_admission(&first.batch_id, &earlier).unwrap(),
            AdmissionFence::Unchanged(_)
        ));
        assert_eq!(
            ledger.fence_admission(&second.batch_id, &earlier).unwrap(),
            AdmissionFence::Blocked(FenceBlocked::Inconsistent)
        );
    }

    #[test]
    fn a32_the_fence_proves_the_prefix_before_comparing_the_family() {
        let a = Dirs::new();
        let ledger = a.open();
        ledger.register_batch(&many("reqB", 10)).unwrap();
        let decided = ledger.register_batch(&many("reqA", 1)).unwrap();
        let earlier = snapshot_of(&decided.admission);
        assert_eq!(earlier.family_effective_trials, 11);
        let fence = |snapshot: &AdmissionSnapshot| {
            ledger.fence_admission(&decided.batch_id, snapshot).unwrap()
        };
        assert!(matches!(fence(&earlier), AdmissionFence::Unchanged(count)
            if count.snapshot() == earlier));

        ledger.register_batch(&many("reqC", 5)).unwrap();
        match fence(&earlier) {
            AdmissionFence::Grew(count) => {
                assert_eq!(count.family_effective_trials(), 16);
                assert_eq!(count.batch_effective_trials(), 1);
                assert_eq!(count.seq(), 16);
            }
            other => panic!("expected Grew, got {other:?}"),
        }

        // Step 1: a prefix the registry cannot prove.
        let ahead = AdmissionSnapshot { seq: 99, ..earlier.clone() };
        assert_eq!(
            fence(&ahead),
            AdmissionFence::Blocked(FenceBlocked::Prefix(BindingCheck::RegistryRolledBack))
        );
        let other_chain = AdmissionSnapshot { chain_head: "0".repeat(64), ..earlier.clone() };
        assert_eq!(
            fence(&other_chain),
            AdmissionFence::Blocked(FenceBlocked::Prefix(BindingCheck::RegistryDiverged))
        );
        // Step 2: counts that cannot move once the prefix holds.
        for inconsistent in [
            AdmissionSnapshot { family_effective_trials: 17, ..earlier.clone() },
            AdmissionSnapshot { batch_effective_trials: 2, ..earlier.clone() },
            AdmissionSnapshot { tests_per_trial: 3, ..earlier.clone() },
            AdmissionSnapshot {
                family_id: super::super::family_id_for("crypto:binance:ETHUSDT").unwrap(),
                ..earlier.clone()
            },
        ] {
            assert_eq!(
                fence(&inconsistent),
                AdmissionFence::Blocked(FenceBlocked::Inconsistent),
                "{inconsistent:?}"
            );
        }
        assert_eq!(
            FenceBlocked::Inconsistent.code(),
            "admission_count_inconsistent"
        );
        assert_eq!(
            ledger
                .fence_admission("trial-batch-v1:missing", &earlier)
                .err()
                .unwrap()
                .code(),
            "batch_not_found"
        );
        let malformed = AdmissionSnapshot { registry_id: "short".into(), ..earlier };
        assert_eq!(
            ledger
                .fence_admission(&decided.batch_id, &malformed)
                .err()
                .unwrap()
                .code(),
            "registry_state_invalid"
        );
    }

    #[test]
    fn the_fence_follows_a_proven_replacement_and_reports_a_later_quarantine() {
        let (a, b, c, d) = (Dirs::new(), Dirs::new(), Dirs::new(), Dirs::new());
        let origin = a.open();
        let decided = origin.register_batch(&many("req", 2)).unwrap();
        let earlier = snapshot_of(&decided.admission);

        let restored = b.open();
        let unrelated = c.open();
        restored
            .import_json_lines(&origin.export_json_lines().unwrap())
            .unwrap();
        assert!(matches!(
            restored.fence_admission(&decided.batch_id, &earlier).unwrap(),
            AdmissionFence::Unchanged(count) if count.registry_id() == restored.registry_id()
        ));
        assert_eq!(
            unrelated.fence_admission(&decided.batch_id, &earlier).unwrap(),
            AdmissionFence::Blocked(FenceBlocked::Prefix(BindingCheck::RegistryReplaced)),
            "a registry without evidence of the prefix cannot vouch for it"
        );

        // The family is quarantined after the decision was made.
        let conflicting = d.open();
        let mut changed = many("req", 2);
        changed.events[0].strategy_hash = Some("different".into());
        conflicting.register_batch(&changed).unwrap();
        origin
            .import_json_lines(&conflicting.export_json_lines().unwrap())
            .unwrap();
        assert_eq!(
            origin.fence_admission(&decided.batch_id, &earlier).unwrap(),
            AdmissionFence::Blocked(FenceBlocked::Admission(
                AdmissionBlocked::FamilyQuarantined
            ))
        );
    }
}
