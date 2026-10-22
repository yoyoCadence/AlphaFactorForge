//! Complete canonical v5 transport. A snapshot/union grants no live rights.
use super::super::{canonical_text, sha256_hex, TrialLedger};
use super::{load_witnessed_in, LedgerError};
use alpha_factor_forge::discovery_core::confirmation_history::{BlobKind, ConfirmationInventory};
use rusqlite::{Transaction, TransactionBehavior};
use serde_json::{json, Value};
use std::io::{self, Write};

#[path = "archive_decode.rs"]
mod decode;
#[path = "archive_import.rs"]
mod import;
pub use import::WitnessedTransferSummary;

const VERSION: &str = "trial-ledger-export-v5";
const SCHEMA: &str = "0007_confirmation_source_witness";
const LIMITS: Limits = Limits {
    file: 256 * 1024 * 1024,
    alpha: 128 * 1024 * 1024,
    header: 4096,
    line: 16 * 1024 * 1024 + 4096,
};

#[derive(Clone, Copy)]
struct Limits {
    file: usize,
    alpha: usize,
    header: usize,
    line: usize,
}
fn bad(detail: &str) -> LedgerError {
    LedgerError::StateInvalid(format!("witnessed v5 export: {detail}"))
}

/// Enforce encoded limits during serialization, including JSON escaping. A
/// failed row remains private and is dropped; partial archives never escape.
struct Bounded<'a> {
    bytes: &'a mut Vec<u8>,
    end: usize,
}
impl Write for Bounded<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.end.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("encoded archive limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn line(body: &mut Vec<u8>, mut value: Value, cap: usize, file: usize) -> Result<(), LedgerError> {
    value.sort_all_objects();
    let end = body
        .len()
        .checked_add(cap)
        .ok_or_else(|| bad("encoded capacity overflow"))?
        .min(file.saturating_sub(1));
    serde_json::to_writer(&mut Bounded { bytes: body, end }, &value)
        .map_err(|_| bad("encoded file/line limit exceeded; never truncated"))?;
    body.push(b'\n');
    Ok(())
}

fn encode(
    registry: &str,
    alpha: &str,
    inventory: &ConfirmationInventory,
    exported_at: &str,
    limits: Limits,
) -> Result<Vec<u8>, LedgerError> {
    if alpha.len() > limits.alpha {
        return Err(bad("embedded v4 projection limit exceeded"));
    }
    let mut body = Vec::new();
    // The first embedded projection has its own bound; only subsequent rows
    // have the smaller per-line bound, as in the existing v4 framing.
    line(
        &mut body,
        json!({"type":"alphaLedger","export":alpha}),
        limits.file,
        limits.file,
    )?;
    for (kind, documents) in [
        ("confirmationBatch", &inventory.batches),
        ("confirmationSelection", &inventory.selections),
        ("confirmationOperation", &inventory.operations),
        ("confirmationExposure", &inventory.exposures),
        ("confirmationAdmission", &inventory.admissions),
        ("confirmationEvent", &inventory.events),
        ("confirmationConflict", &inventory.conflicts),
    ] {
        let mut records = documents
            .iter()
            .map(|raw| {
                let value: Value =
                    serde_json::from_str(raw).map_err(|_| bad("invalid checked document"))?;
                let id = value["id"]
                    .as_str()
                    .ok_or_else(|| bad("checked document has no ID"))?
                    .to_string();
                Ok((id, raw))
            })
            .collect::<Result<Vec<_>, LedgerError>>()?;
        records.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, raw) in records {
            let record: Value =
                serde_json::from_str(raw).map_err(|_| bad("invalid checked document"))?;
            line(
                &mut body,
                json!({"type":kind,"record":record}),
                limits.line,
                limits.file,
            )?;
        }
    }
    let mut blobs: Vec<_> = inventory.blobs.iter().collect();
    blobs.sort_by(|a, b| a.sha256().cmp(b.sha256()));
    for blob in blobs {
        let kind = match blob.kind() {
            BlobKind::Statistics => "statistics",
            BlobKind::Binding => "binding",
            BlobKind::SourceWitness => "source_witness",
        };
        line(
            &mut body,
            json!({"type":"confirmationBlob", "sha256":blob.sha256(),
            "kind":kind,"text":blob.text()}),
            limits.line,
            limits.file,
        )?;
    }
    for origin in &inventory.unknown_origins {
        line(
            &mut body,
            json!({"type":"confirmationUnknownOrigin","originRegistryId":origin}),
            limits.line,
            limits.file,
        )?;
    }
    let header = canonical_text(&json!({
        "version":VERSION,"registryId":registry,"schemaVersion":SCHEMA,
        "exportedAt":exported_at,"bodySha256":sha256_hex(&body),
        "counts":{
            "batches":inventory.batches.len(),"selections":inventory.selections.len(),
            "operations":inventory.operations.len(),"exposures":inventory.exposures.len(),
            "admissions":inventory.admissions.len(),"events":inventory.events.len(),
            "blobs":inventory.blobs.len(),"conflicts":inventory.conflicts.len(),
            "unknownOrigins":inventory.unknown_origins.len(),
        }
    }))?;
    if header.len() > limits.header
        || header.len().saturating_add(1).saturating_add(body.len()) > limits.file
    {
        return Err(bad("encoded file/header limit exceeded; never truncated"));
    }
    let mut output = header.into_bytes();
    output.push(b'\n');
    output.append(&mut body);
    Ok(output)
}

pub(in crate::research::trial_ledger) fn export_in(
    ledger: &TrialLedger,
    tx: &Transaction<'_>,
) -> Result<Vec<u8>, LedgerError> {
    let graph = load_witnessed_in(tx, ledger.registry_id())?;
    let projection = ledger.export_alpha_in(tx)?;
    let alpha = std::str::from_utf8(&projection).map_err(|_| bad("projection is not UTF-8"))?;
    encode(
        ledger.registry_id(),
        alpha,
        graph.inventory(),
        &chrono::Utc::now().to_rfc3339(),
        LIMITS,
    )
}
impl TrialLedger {
    /// Backend-only complete snapshot. Legacy v3/v4 public omission guards
    /// remain active; this does not expose an import or execution capability.
    pub fn export_witnessed_json_lines(&self) -> Result<Vec<u8>, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let output = export_in(self, &tx)?;
        tx.commit()?;
        Ok(output)
    }
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
