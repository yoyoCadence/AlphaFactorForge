//! Complete observed-history archives. No runtime confirmation/reveal caller.
//! Validate input and the proposed union before the destination's first write.

use super::*;
use crate::research::trial_ledger::{
    transfer::ValidatedExport, ImportSummary, REGISTRY_MIGRATIONS,
};
use serde::Serialize;

const VERSION: &str = "trial-ledger-export-v4";
const SCHEMA: &str = "0005_alpha_history_provenance";
const MAX_FILE_BYTES: usize = 128 * 1024 * 1024;
const MAX_TRIAL_BYTES: usize = 64 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 4096;
const MAX_ALPHA_LINE_BYTES: usize = MAX_CONFLICT_BYTES + 2048;

fn integrity(message: impl Into<String>) -> LedgerError {
    LedgerError::ImportIntegrityFailed(message.into())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Header {
    version: String,
    registry_id: String,
    schema_version: String,
    exported_at: String,
    budget_count: usize,
    reservation_count: usize,
    conflict_count: usize,
    unknown_origin_count: usize,
    body_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Record {
    TrialLedger {
        export: String,
    },
    AlphaBudget {
        family_id: String,
        allocation_id: String,
        declaration: Value,
    },
    AlphaReservation {
        record: Value,
    },
    AlphaConflict {
        conflict_id: String,
        family_id: String,
        body: Value,
    },
    AlphaUnknownOrigin {
        origin_registry_id: String,
    },
}

impl Record {
    fn key(&self) -> Result<(u8, String), LedgerError> {
        Ok(match self {
            Self::TrialLedger { .. } => (0, String::new()),
            Self::AlphaBudget { family_id, .. } => (1, family_id.clone()),
            Self::AlphaReservation { record } => (
                2,
                record["reservationId"]
                    .as_str()
                    .ok_or_else(|| integrity("reservation ID missing"))?
                    .into(),
            ),
            Self::AlphaConflict {
                conflict_id,
                family_id,
                ..
            } => (3, format!("{conflict_id}:{family_id}")),
            Self::AlphaUnknownOrigin { origin_registry_id } => (4, origin_registry_id.clone()),
        })
    }

    fn line(&self) -> Result<String, LedgerError> {
        canonical_text(&serde_json::to_value(self)?)
    }
}

#[derive(Default, Clone, Debug, Eq, PartialEq)]
struct Inventory {
    budgets: BTreeMap<String, Record>,
    reservations: BTreeMap<String, Record>,
    conflicts: BTreeMap<(String, String), Record>,
    unknown: BTreeSet<String>,
}

impl Inventory {
    fn read(conn: &Connection) -> Result<Self, LedgerError> {
        let evidence = load(conn)?;
        let mut inventory = Self {
            unknown: unknown_origins(conn)?,
            ..Self::default()
        };
        for (family, declaration) in &evidence.budgets {
            inventory.budgets.insert(
                family.clone(),
                Record::AlphaBudget {
                    family_id: family.clone(),
                    allocation_id: evidence.budget_ids[family].clone(),
                    declaration: declaration.document(),
                },
            );
            // A gap in unblocked observed history is not a complete source.
            family_state(&evidence, family)?;
        }
        for record in &evidence.records {
            inventory.reservations.insert(
                record.reservation_id().into(),
                Record::AlphaReservation {
                    record: record.document(),
                },
            );
        }
        for (family, bodies) in evidence.conflicts {
            for body in bodies {
                let (id, _) = conflict_identity(&body)?;
                inventory.conflicts.insert(
                    (id.clone(), family.clone()),
                    Record::AlphaConflict {
                        conflict_id: id,
                        family_id: family.clone(),
                        body,
                    },
                );
            }
        }
        for row in inventory.conflicts.values() {
            let Record::AlphaConflict { body, .. } = row else {
                unreachable!()
            };
            if body["kind"] == "budget" {
                continue;
            }
            for document in body["documents"]
                .as_array()
                .ok_or_else(|| integrity("conflict documents missing"))?
            {
                let id = document["reservationId"]
                    .as_str()
                    .ok_or_else(|| integrity("conflict reservation ID missing"))?;
                if !matches!(inventory.reservations.get(id), Some(Record::AlphaReservation { record }) if record == document)
                {
                    return Err(integrity(
                        "conflict references a missing or different reservation",
                    ));
                }
            }
        }
        Ok(inventory)
    }

    fn records(&self) -> Vec<Record> {
        self.budgets
            .values()
            .chain(self.reservations.values())
            .chain(self.conflicts.values())
            .cloned()
            .chain(self.unknown.iter().map(|id| Record::AlphaUnknownOrigin {
                origin_registry_id: id.clone(),
            }))
            .collect()
    }

    fn capacity(&self) -> Result<(), LedgerError> {
        if self.budgets.len() + self.reservations.len() + self.conflicts.len()
            > MAX_ALPHA_ROWS as usize
            || self.unknown.len() > MAX_UNKNOWN_ORIGINS
        {
            return Err(integrity(
                "alpha row/unknown-origin limit exceeded; never truncated",
            ));
        }
        let mut bytes = 0;
        for record in self
            .budgets
            .values()
            .chain(self.reservations.values())
            .chain(self.conflicts.values())
        {
            let (document, maximum) = match record {
                Record::AlphaBudget { declaration, .. } => (declaration, MAX_BUDGET_BYTES),
                Record::AlphaReservation { record } => (record, MAX_RECORD_BYTES),
                Record::AlphaConflict { body, .. } => (body, MAX_CONFLICT_BYTES),
                _ => unreachable!("inventory contains alpha rows only"),
            };
            let length = canonical_text(document)?.len();
            if length > maximum {
                return Err(integrity("alpha document limit exceeded"));
            }
            bytes += length;
        }
        if bytes > MAX_ALPHA_BYTES as usize {
            return Err(integrity("alpha byte limit exceeded; never truncated"));
        }
        Ok(())
    }

    fn insert_record(&mut self, record: Record) -> Result<(), LedgerError> {
        match record {
            Record::AlphaBudget { ref family_id, .. } => {
                if self.budgets.insert(family_id.clone(), record).is_some() {
                    return Err(integrity("duplicate budget"));
                }
            }
            Record::AlphaReservation { ref record } => {
                let checked = parse_alpha_reservation(&canonical_text(record)?).map_err(invalid)?;
                if self
                    .reservations
                    .insert(
                        checked.reservation_id().into(),
                        Record::AlphaReservation {
                            record: record.clone(),
                        },
                    )
                    .is_some()
                {
                    return Err(integrity("duplicate reservation"));
                }
            }
            Record::AlphaConflict {
                ref conflict_id,
                ref family_id,
                ..
            } => {
                if self
                    .conflicts
                    .insert((conflict_id.clone(), family_id.clone()), record)
                    .is_some()
                {
                    return Err(integrity("duplicate conflict copy"));
                }
            }
            Record::AlphaUnknownOrigin { origin_registry_id } => {
                if !self.unknown.insert(origin_registry_id) {
                    return Err(integrity("duplicate unknown origin"));
                }
            }
            Record::TrialLedger { .. } => return Err(integrity("unexpected trial record")),
        }
        Ok(())
    }

    /// Only called on a private, disposable validation database or inside the
    /// final IMMEDIATE destination transaction after preflight validation.
    fn write_difference(&self, tx: &Transaction<'_>, old: &Self) -> Result<(), LedgerError> {
        let now = chrono::Utc::now().to_rfc3339();
        for (family, row) in &self.budgets {
            if old.budgets.contains_key(family) {
                continue;
            }
            let Record::AlphaBudget {
                allocation_id,
                declaration,
                ..
            } = row
            else {
                unreachable!()
            };
            tx.execute("INSERT INTO alpha_family_budgets (family_id,allocation_id,document_json,recorded_at) VALUES (?1,?2,?3,?4)", params![family,allocation_id,canonical_text(declaration)?,now])?;
        }
        for (id, row) in &self.reservations {
            if old.reservations.contains_key(id) {
                continue;
            }
            let Record::AlphaReservation { record } = row else {
                unreachable!()
            };
            let checked = parse_alpha_reservation(&canonical_text(record)?).map_err(invalid)?;
            tx.execute("INSERT INTO alpha_reservations (reservation_id,family_id,record_json,recorded_at) VALUES (?1,?2,?3,?4)", params![id,checked.family_id(),canonical_text(record)?,now])?;
        }
        for ((id, family), row) in &self.conflicts {
            if old.conflicts.contains_key(&(id.clone(), family.clone())) {
                continue;
            }
            let Record::AlphaConflict { body, .. } = row else {
                unreachable!()
            };
            tx.execute("INSERT INTO alpha_conflicts (conflict_id,family_id,detail_json,recorded_at) VALUES (?1,?2,?3,?4)", params![id,family,canonical_text(body)?,now])?;
        }
        record_unknown_origins(tx, &self.unknown)?;
        Ok(())
    }

    fn validate(&self, families: &BTreeMap<String, String>) -> Result<(), LedgerError> {
        self.capacity()?;
        let mut conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON; CREATE TABLE registry_migrations (version TEXT PRIMARY KEY, applied_at TEXT NOT NULL)")?;
        for (_, migration) in REGISTRY_MIGRATIONS {
            conn.execute_batch(migration)?;
        }
        let tx = conn.transaction()?;
        for (id, key) in families {
            tx.execute("INSERT INTO trial_families (family_id,family_key,created_at) VALUES (?1,?2,'validation')", params![id,key])?;
        }
        self.write_difference(&tx, &Self::default())?;
        if Self::read(&tx)? != *self {
            return Err(integrity("alpha inventory does not reproduce"));
        }
        Ok(())
    }

    fn add_conflict(&mut self, kind: &str, documents: Vec<Value>) -> Result<(), LedgerError> {
        let mut documents = documents
            .into_iter()
            .map(|document| Ok((canonical_text(&document)?, document)))
            .collect::<Result<Vec<_>, LedgerError>>()?;
        documents.sort_by(|a, b| a.0.cmp(&b.0));
        let body = json!({"contractVersion":CONFLICT_VERSION,"kind":kind,"documents":documents.into_iter().map(|(_,document)|document).collect::<Vec<_>>()});
        let families = conflict_families(&serde_json::from_value(body.clone())?)?;
        let (id, _) = conflict_identity(&body)?;
        for family in families {
            self.conflicts
                .entry((id.clone(), family.clone()))
                .or_insert(Record::AlphaConflict {
                    conflict_id: id.clone(),
                    family_id: family,
                    body: body.clone(),
                });
        }
        if self.budgets.len() + self.reservations.len() + self.conflicts.len()
            > MAX_ALPHA_ROWS as usize
        {
            return Err(integrity("union alpha row limit exceeded"));
        }
        Ok(())
    }

    fn union(&mut self, incoming: Self) -> Result<(), LedgerError> {
        for (family, budget) in incoming.budgets {
            if let Some(old) = self.budgets.get(&family) {
                if old != &budget {
                    let document = |record: &Record| match record {
                        Record::AlphaBudget {
                            family_id,
                            allocation_id,
                            declaration,
                        } => {
                            json!({"familyId":family_id,"allocationId":allocation_id,"declaration":declaration})
                        }
                        _ => unreachable!(),
                    };
                    self.add_conflict("budget", vec![document(old), document(&budget)])?;
                }
            } else {
                self.budgets.insert(family, budget);
            }
        }
        for (id, record) in incoming.reservations {
            if self.reservations.get(&id).is_some_and(|old| old != &record) {
                return Err(integrity("reservation ID has different content"));
            }
            self.reservations.entry(id).or_insert(record);
        }
        for (id, record) in incoming.conflicts {
            if self.conflicts.get(&id).is_some_and(|old| old != &record) {
                return Err(integrity("conflict ID has different content"));
            }
            self.conflicts.entry(id).or_insert(record);
        }
        self.unknown.extend(incoming.unknown);
        self.capacity()?;
        let records = self
            .reservations
            .values()
            .map(|row| match row {
                Record::AlphaReservation { record } => {
                    parse_alpha_reservation(&canonical_text(record)?).map_err(invalid)
                }
                _ => unreachable!(),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut numbers = BTreeMap::new();
        let mut operations = BTreeMap::new();
        // Inventory order fixes each group's representative to its lowest ID.
        // Existing non-representative conflicts are retained independently.
        for record in &records {
            let number = (record.family_id(), record.confirmation_number());
            if let Some(first) = numbers.get(&number) {
                let first: &&AlphaReservation = first;
                self.add_conflict(
                    "confirmation_number",
                    vec![first.document(), record.document()],
                )?;
            } else {
                numbers.insert(number, record);
            }
            let operation = (record.origin_registry_id(), record.reservation_key());
            if let Some(first) = operations.get(&operation) {
                let first: &&AlphaReservation = first;
                self.add_conflict("origin_key", vec![first.document(), record.document()])?;
            } else {
                operations.insert(operation, record);
            }
        }
        self.capacity()
    }
}

struct ValidatedComplete {
    trial: ValidatedExport,
    inventory: Inventory,
}

impl ValidatedComplete {
    fn parse(bytes: &[u8]) -> Result<Self, LedgerError> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(integrity("complete transfer file limit exceeded"));
        }
        let text = std::str::from_utf8(bytes).map_err(|_| integrity("not UTF-8"))?;
        let (header_line, body) = text
            .split_once('\n')
            .ok_or_else(|| integrity("missing header"))?;
        if header_line.len() > MAX_HEADER_BYTES {
            return Err(integrity("header limit exceeded"));
        }
        // Recognize old versions before decoding v4-only mandatory fields.
        let version: Value =
            serde_json::from_str(header_line).map_err(|_| integrity("invalid header JSON"))?;
        if matches!(
            version["version"].as_str(),
            Some("trial-ledger-export-v1" | "trial-ledger-export-v2" | "trial-ledger-export-v3")
        ) {
            return Err(LedgerError::AlphaHistoryUnknown);
        }
        let header: Header =
            serde_json::from_str(header_line).map_err(|e| integrity(format!("header: {e}")))?;
        if canonical_text(&serde_json::to_value(&header)?)? != header_line
            || header.version != VERSION
            || header.schema_version != SCHEMA
            || header.registry_id.len() != 32
            || !header
                .registry_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || sha256_hex(body.as_bytes()) != header.body_sha256
        {
            return Err(integrity("invalid complete header/hash"));
        }
        let mut trial = None;
        let mut previous = None;
        let mut inventory = Inventory::default();
        for (index, line) in body.split_inclusive('\n').enumerate() {
            let line = line
                .strip_suffix('\n')
                .ok_or_else(|| integrity("body needs final newline"))?;
            if line.is_empty() || (index > 0 && line.len() > MAX_ALPHA_LINE_BYTES) {
                return Err(integrity("invalid/oversized body line"));
            }
            let record: Record =
                serde_json::from_str(line).map_err(|e| integrity(format!("record: {e}")))?;
            if record.line()? != line {
                return Err(integrity("noncanonical record"));
            }
            let key = record.key()?;
            if previous.as_ref().is_some_and(|old| old >= &key) {
                return Err(integrity(
                    "record sections/identities must be sorted and unique",
                ));
            }
            previous = Some(key);
            if let Record::TrialLedger { export } = record {
                if index != 0 || export.len() > MAX_TRIAL_BYTES {
                    return Err(integrity("invalid/oversized trial projection"));
                }
                trial = Some(ValidatedExport::parse(export.as_bytes())?);
            } else {
                if trial.is_none() {
                    return Err(integrity("trial projection must be first"));
                }
                inventory.insert_record(record)?;
                if inventory.budgets.len()
                    + inventory.reservations.len()
                    + inventory.conflicts.len()
                    > MAX_ALPHA_ROWS as usize
                    || inventory.unknown.len() > MAX_UNKNOWN_ORIGINS
                {
                    return Err(integrity("alpha record limit exceeded"));
                }
            }
        }
        let mut trial = trial.ok_or_else(|| integrity("missing trial projection"))?;
        if trial.registry_id() != header.registry_id
            || header.budget_count != inventory.budgets.len()
            || header.reservation_count != inventory.reservations.len()
            || header.conflict_count != inventory.conflicts.len()
            || header.unknown_origin_count != inventory.unknown.len()
        {
            return Err(integrity("source/count mismatch"));
        }
        let families = trial
            .family_keys()
            .map(|(id, key)| (id.clone(), key.clone()))
            .collect();
        inventory.validate(&families)?;
        trial.set_file_sha256(sha256_hex(bytes));
        Ok(Self { trial, inventory })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaTransferSummary {
    pub trial: ImportSummary,
    pub added_budget_rows: usize,
    pub added_reservations: usize,
    pub added_conflict_rows: usize,
    pub unknown_origin_count: usize,
}

impl TrialLedger {
    pub fn export_complete_json_lines(&self) -> Result<Vec<u8>, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let inventory = Inventory::read(&tx)?;
        let trial = self.export_trial_in(&tx, Some(MAX_TRIAL_BYTES))?;
        if trial.len() > MAX_TRIAL_BYTES {
            return Err(integrity("trial projection limit exceeded"));
        }
        let mut body = String::new();
        for record in std::iter::once(Record::TrialLedger {
            export: String::from_utf8(trial).map_err(|_| integrity("not UTF-8"))?,
        })
        .chain(inventory.records())
        {
            let line = record.line()?;
            if body.len() + line.len() + 1 > MAX_FILE_BYTES {
                return Err(integrity("complete transfer file limit exceeded"));
            }
            body.push_str(&line);
            body.push('\n');
        }
        let header = Header {
            version: VERSION.into(),
            registry_id: self.registry_id().into(),
            schema_version: SCHEMA.into(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            budget_count: inventory.budgets.len(),
            reservation_count: inventory.reservations.len(),
            conflict_count: inventory.conflicts.len(),
            unknown_origin_count: inventory.unknown.len(),
            body_sha256: sha256_hex(body.as_bytes()),
        };
        let header = canonical_text(&serde_json::to_value(header)?)?;
        let output = format!("{header}\n{body}").into_bytes();
        if header.len() > MAX_HEADER_BYTES || output.len() > MAX_FILE_BYTES {
            return Err(integrity("complete transfer file/header limit exceeded"));
        }
        tx.commit()?;
        Ok(output)
    }

    pub fn import_complete_json_lines(
        &self,
        bytes: &[u8],
    ) -> Result<AlphaTransferSummary, LedgerError> {
        self.import_complete(bytes, false)
    }

    /// Only a transport-history gate, never a confirmation admission token.
    pub fn import_qualification_json_lines(
        &self,
        bytes: &[u8],
    ) -> Result<AlphaTransferSummary, LedgerError> {
        self.import_complete(bytes, true)
    }

    fn import_complete(
        &self,
        bytes: &[u8],
        qualification: bool,
    ) -> Result<AlphaTransferSummary, LedgerError> {
        let source = ValidatedComplete::parse(bytes).map_err(|error| match error {
            LedgerError::AlphaHistoryUnknown => LedgerError::AlphaHistoryUnknown,
            other => integrity(other.to_string()),
        })?;
        if qualification && !source.inventory.unknown.is_empty() {
            return Err(LedgerError::AlphaHistoryUnknown);
        }
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old = Inventory::read(&tx)?;
        if qualification && !old.unknown.is_empty() {
            return Err(LedgerError::AlphaHistoryUnknown);
        }
        let mut families: BTreeMap<String, String> = {
            let mut stmt = tx.prepare("SELECT family_id,family_key FROM trial_families")?;
            let values = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            values
        };
        for (id, key) in source.trial.family_keys() {
            if families.get(id).is_some_and(|old| old != key) {
                return Err(integrity("alpha union family key differs"));
            }
            families.insert(id.clone(), key.clone());
        }
        let mut union = old.clone();
        union.union(source.inventory)?;
        union.validate(&families)?;
        let trial = self.import_trial_in(&tx, source.trial)?;
        // A quarantined trial family may have no local row. Retain its known
        // canonical key for alpha evidence without releasing its quarantine.
        for family in union.budgets.keys() {
            tx.execute("INSERT INTO trial_families (family_id,family_key,created_at) VALUES (?1,?2,?3) ON CONFLICT(family_id) DO NOTHING",params![family,families[family],chrono::Utc::now().to_rfc3339()])?;
        }
        union.write_difference(&tx, &old)?;
        if Inventory::read(&tx)? != union {
            return Err(integrity("written union differs from preflight"));
        }
        let result = AlphaTransferSummary {
            trial,
            added_budget_rows: union.budgets.len() - old.budgets.len(),
            added_reservations: union.reservations.len() - old.reservations.len(),
            added_conflict_rows: union.conflicts.len() - old.conflicts.len(),
            unknown_origin_count: union.unknown.len(),
        };
        tx.commit()?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "transfer_tests.rs"]
mod tests;
