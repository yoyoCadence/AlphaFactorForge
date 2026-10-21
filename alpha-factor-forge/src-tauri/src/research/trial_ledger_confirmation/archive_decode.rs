//! Strict original-byte v5 input. No receiver, destination lock or live rights.
use super::*;
use crate::research::trial_ledger::{alpha::ValidatedComplete, REGISTRY_MIGRATIONS};
use alpha_factor_forge::discovery_core::confirmation_history::{
    HistoryBlob, ValidatedGraph, MAX_DOCUMENT_BYTES, MAX_HISTORY_BYTES, MAX_HISTORY_ROWS,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

fn integrity(detail: &str) -> LedgerError {
    LedgerError::ImportIntegrityFailed(format!("witnessed v5 source: {detail}"))
}
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Counts {
    batches: usize,
    selections: usize,
    operations: usize,
    exposures: usize,
    admissions: usize,
    events: usize,
    blobs: usize,
    conflicts: usize,
    unknown_origins: usize,
}
impl Counts {
    fn from(i: &ConfirmationInventory) -> Self {
        Self {
            batches: i.batches.len(),
            selections: i.selections.len(),
            operations: i.operations.len(),
            exposures: i.exposures.len(),
            admissions: i.admissions.len(),
            events: i.events.len(),
            blobs: i.blobs.len(),
            conflicts: i.conflicts.len(),
            unknown_origins: i.unknown_origins.len(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Header {
    version: String,
    registry_id: String,
    schema_version: String,
    exported_at: String,
    counts: Counts,
    body_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Binding,
    Statistics,
    SourceWitness,
}
#[derive(Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Record {
    AlphaLedger {
        export: String,
    },
    ConfirmationBatch {
        record: Value,
    },
    ConfirmationSelection {
        record: Value,
    },
    ConfirmationOperation {
        record: Value,
    },
    ConfirmationExposure {
        record: Value,
    },
    ConfirmationAdmission {
        record: Value,
    },
    ConfirmationEvent {
        record: Value,
    },
    ConfirmationConflict {
        record: Value,
    },
    ConfirmationBlob {
        sha256: String,
        kind: Kind,
        text: String,
    },
    ConfirmationUnknownOrigin {
        origin_registry_id: String,
    },
}
impl Record {
    fn key(&self) -> Result<(u8, String), LedgerError> {
        let (rank, value) = match self {
            Self::AlphaLedger { .. } => return Ok((0, String::new())),
            Self::ConfirmationBatch { record } => (1, record),
            Self::ConfirmationSelection { record } => (2, record),
            Self::ConfirmationOperation { record } => (3, record),
            Self::ConfirmationExposure { record } => (4, record),
            Self::ConfirmationAdmission { record } => (5, record),
            Self::ConfirmationEvent { record } => (6, record),
            Self::ConfirmationConflict { record } => (7, record),
            Self::ConfirmationBlob { sha256, .. } => return Ok((8, sha256.clone())),
            Self::ConfirmationUnknownOrigin { origin_registry_id } => {
                return Ok((9, origin_registry_id.clone()))
            }
        };
        Ok((
            rank,
            value["id"]
                .as_str()
                .ok_or_else(|| integrity("missing record ID"))?
                .into(),
        ))
    }
}

/// Opaque validated source retained for the subsequent atomic-union slice.
/// Its construction always includes independent source reconstruction.
#[allow(dead_code)]
pub(in crate::research::trial_ledger) struct ValidatedWitnessed {
    alpha: ValidatedComplete,
    graph: ValidatedGraph,
    file_sha256: String,
}
impl ValidatedWitnessed {
    pub(in crate::research::trial_ledger) fn parse(bytes: &[u8]) -> Result<Self, LedgerError> {
        Self::parse_with_limits(bytes, LIMITS)
    }
    fn parse_with_limits(bytes: &[u8], limits: Limits) -> Result<Self, LedgerError> {
        if bytes.len() > limits.file {
            return Err(integrity("file limit exceeded"));
        }
        let text = std::str::from_utf8(bytes).map_err(|_| integrity("not UTF-8"))?;
        let (header_line, body) = text
            .split_once('\n')
            .ok_or_else(|| integrity("missing header"))?;
        if header_line.len() > limits.header {
            return Err(integrity("header limit exceeded"));
        }
        let header: Header =
            serde_json::from_str(header_line).map_err(|_| integrity("invalid closed header"))?;
        if canonical_text(&serde_json::to_value(&header)?)? != header_line
            || header.version != VERSION
            || header.schema_version != SCHEMA
            || !super::super::valid_origin(&header.registry_id)
            || header.body_sha256 != sha256_hex(body.as_bytes())
        {
            return Err(integrity(
                "noncanonical header, version, source or body hash",
            ));
        }
        let mut alpha = None;
        let mut inventory = ConfirmationInventory::default();
        let mut previous = None;
        let mut rows = 0usize;
        let mut decoded_bytes = 0usize;
        for (index, raw) in body.split_inclusive('\n').enumerate() {
            let raw = raw
                .strip_suffix('\n')
                .ok_or_else(|| integrity("missing final newline"))?;
            if raw.is_empty() || (index > 0 && raw.len() > limits.line) {
                return Err(integrity("empty or oversized encoded line"));
            }
            let record: Record =
                serde_json::from_str(raw).map_err(|_| integrity("invalid closed record"))?;
            // Compare ORIGINAL text before normalization; nested duplicate keys,
            // hidden fields, whitespace and noncanonical numeric forms fail.
            if canonical_text(&serde_json::to_value(&record)?)? != raw {
                return Err(integrity("noncanonical original record"));
            }
            let key = record.key()?;
            if previous.as_ref().is_some_and(|old| old >= &key) {
                return Err(integrity("sections/IDs must be sorted and unique"));
            }
            previous = Some(key);
            if let Record::AlphaLedger { export } = record {
                if index != 0 || export.len() > limits.alpha {
                    return Err(integrity("invalid or oversized embedded v4"));
                }
                let checked = ValidatedComplete::parse(export.as_bytes())
                    .map_err(|_| integrity("invalid nested v4 source"))?;
                if checked.registry_id() != header.registry_id {
                    return Err(integrity("nested source differs"));
                }
                alpha = Some(checked);
                continue;
            }
            if alpha.is_none() {
                return Err(integrity("embedded v4 must be first"));
            }
            let (section, document) = match record {
                Record::ConfirmationBatch { record } => (&mut inventory.batches, record),
                Record::ConfirmationSelection { record } => (&mut inventory.selections, record),
                Record::ConfirmationOperation { record } => (&mut inventory.operations, record),
                Record::ConfirmationExposure { record } => (&mut inventory.exposures, record),
                Record::ConfirmationAdmission { record } => (&mut inventory.admissions, record),
                Record::ConfirmationEvent { record } => (&mut inventory.events, record),
                Record::ConfirmationConflict { record } => (&mut inventory.conflicts, record),
                Record::ConfirmationBlob { sha256, kind, text } => {
                    decoded_bytes = decoded_bytes
                        .checked_add(text.len())
                        .ok_or_else(|| integrity("history byte overflow"))?;
                    rows += 1;
                    if decoded_bytes > MAX_HISTORY_BYTES || rows > MAX_HISTORY_ROWS {
                        return Err(integrity("history capacity exceeded"));
                    }
                    let kind = match kind {
                        Kind::Binding => BlobKind::Binding,
                        Kind::Statistics => BlobKind::Statistics,
                        Kind::SourceWitness => BlobKind::SourceWitness,
                    };
                    let blob = HistoryBlob::new(kind, &text)
                        .map_err(|_| integrity("invalid original blob"))?;
                    if blob.sha256() != sha256 {
                        return Err(integrity("blob digest differs"));
                    }
                    inventory.blobs.push(blob);
                    continue;
                }
                Record::ConfirmationUnknownOrigin { origin_registry_id } => {
                    if inventory.unknown_origins.len() >= super::super::MAX_UNKNOWN_ORIGINS
                        || !super::super::valid_origin(&origin_registry_id)
                    {
                        return Err(integrity("unknown-origin capacity or identity"));
                    }
                    inventory.unknown_origins.push(origin_registry_id);
                    continue;
                }
                Record::AlphaLedger { .. } => unreachable!(),
            };
            let document = canonical_text(&document)?;
            decoded_bytes = decoded_bytes
                .checked_add(document.len())
                .ok_or_else(|| integrity("history byte overflow"))?;
            rows += 1;
            if document.len() > MAX_DOCUMENT_BYTES
                || decoded_bytes > MAX_HISTORY_BYTES
                || rows > MAX_HISTORY_ROWS
            {
                return Err(integrity("document/history capacity exceeded"));
            }
            section.push(document);
        }
        let alpha = alpha.ok_or_else(|| integrity("missing embedded v4"))?;
        if header.counts != Counts::from(&inventory) {
            return Err(integrity("nine counts differ"));
        }
        let graph = validate_source(&alpha, &inventory)
            .map_err(|_| integrity("invalid or incomplete reconstructed source"))?;
        Ok(Self {
            alpha,
            graph,
            file_sha256: sha256_hex(bytes),
        })
    }
}
fn validate_source(
    alpha: &ValidatedComplete,
    inventory: &ConfirmationInventory,
) -> Result<ValidatedGraph, LedgerError> {
    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE registry_migrations (version TEXT PRIMARY KEY, applied_at TEXT NOT NULL)")?;
    for (_, migration) in REGISTRY_MIGRATIONS {
        conn.execute_batch(migration)?;
    }
    let tx = conn.transaction()?;
    alpha.stage_source_in(&tx)?;
    let graph = super::super::storage::stage_source_in(&tx, alpha.registry_id(), inventory)?;
    if tx.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err(integrity("source foreign-key dependency missing"));
    }
    // Disposable transaction is rolled back/dropped. Never a target receipt.
    Ok(graph)
}
impl TrialLedger {
    /// Validate a complete original source without opening/locking a receiver.
    /// This grants no import, qualification, adoption or execution capability.
    pub fn validate_witnessed_archive(bytes: &[u8]) -> Result<(), LedgerError> {
        ValidatedWitnessed::parse(bytes).map(drop)
    }
}

#[cfg(test)]
#[path = "archive_decode_tests.rs"]
mod tests;
