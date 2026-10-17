//! P13c-3a-1a: strict single-record history codecs. Reference shape and supplied
//! bindings are checked here; full graph validity and authority are separate.
use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    alpha_reservations::AlphaReservation,
    confirmation_contracts::{
        confirmation_family_id, ExposureFootprint, FrozenBatch, FrozenOperation, TimeInterval,
    },
    identity::canonical_bytes,
    precision::PRECISION_MAX_COUNT,
};

pub const EXPOSURE_VERSION: &str = "research-confirmation-exposure-v1";
pub const EVENT_VERSION: &str = "research-confirmation-event-v1";
pub const CONFLICT_VERSION: &str = "research-confirmation-conflict-v1";
pub const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

mod admission;
mod binding_snapshot;
mod graph;
mod source_witness;
pub use admission::{
    freeze_admission, parse_admission, AdmissionInputs, AdmissionRecord, BlobKind,
    HistoricalTrialObservation, HistoryBlob, MAX_BINDING_BYTES,
};
pub use binding_snapshot::{
    parse_binding_snapshot, BindingSnapshot, ConfirmationBindingSet, CONFIRMATION_BINDING_VERSION,
};
pub use graph::{
    complete_confirmation_graph, validate_confirmation_graph, ConfirmationInventory, GraphContext,
    HistoricalPrefix, HostAllocation, HostConflict, ValidatedGraph, MAX_HISTORY_BYTES,
    MAX_HISTORY_INTERVALS, MAX_HISTORY_ROWS,
};
pub use source_witness::{
    freeze_source_witness, parse_source_witness, SourceTrialAnchor, SourceWitnessSnapshot,
    MAX_SOURCE_WITNESS_BYTES, SOURCE_WITNESS_VERSION,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HistoryRecordError {
    #[error("confirmation_history.document_too_large")]
    TooLarge,
    #[error("confirmation_history.malformed_document")]
    Malformed,
    #[error("confirmation_history.unsupported_version")]
    Version,
    #[error("confirmation_history.invalid_reference")]
    Reference,
    #[error("confirmation_history.invalid_binding")]
    Binding,
    #[error("confirmation_history.invalid_variant")]
    Variant,
    #[error("confirmation_history.invalid_intervals")]
    Intervals,
    #[error("confirmation_history.invalid_count")]
    Count,
    #[error("confirmation_history.invalid_targets")]
    Targets,
    #[error("confirmation_history.identity_mismatch")]
    Identity,
    #[error("confirmation_history.invalid_blob")]
    Blob,
    #[error("confirmation_history.invalid_precision_proof")]
    Proof,
    #[error("confirmation_history.missing_or_invalid_dependency")]
    Dependency,
    #[error("confirmation_history.cyclic_dependencies")]
    Cycle,
    #[error("confirmation_history.capacity_exceeded")]
    Capacity,
    #[error("confirmation_history.invalid_conflict")]
    Conflict,
}

fn require(ok: bool, error: HistoryRecordError) -> Result<(), HistoryRecordError> {
    if ok {
        Ok(())
    } else {
        Err(error)
    }
}
fn read<T: DeserializeOwned>(raw: &str) -> Result<T, HistoryRecordError> {
    require(
        raw.len() <= MAX_DOCUMENT_BYTES,
        HistoryRecordError::TooLarge,
    )?;
    // Serde diagnostics may echo protected input. Never expose them.
    serde_json::from_str(raw).map_err(|_| HistoryRecordError::Malformed)
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}
fn reference(value: &str, length: usize) -> Result<(), HistoryRecordError> {
    require(hex(value, length), HistoryRecordError::Reference)
}
fn count(value: u64, minimum: u64) -> Result<(), HistoryRecordError> {
    require(
        (minimum..=PRECISION_MAX_COUNT).contains(&value),
        HistoryRecordError::Count,
    )
}
fn nullable_reference(value: &Option<String>) -> Result<(), HistoryRecordError> {
    if let Some(value) = value {
        reference(value, 64)?;
    }
    Ok(())
}
fn required_nullable<'de, D: Deserializer<'de>>(de: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(de)
}
fn identity<T: Serialize>(version: &str, body: &T) -> Result<String, HistoryRecordError> {
    let value = serde_json::to_value(body).map_err(|_| HistoryRecordError::Malformed)?;
    let bytes = canonical_bytes(&value).map_err(|_| HistoryRecordError::Malformed)?;
    let mut hash = Sha256::new();
    hash.update(version.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    let id = hex::encode(hash.finalize());
    require(
        serde_json::to_vec(&serde_json::json!({"id":id,"body":value}))
            .map_err(|_| HistoryRecordError::Malformed)?
            .len()
            <= MAX_DOCUMENT_BYTES,
        HistoryRecordError::TooLarge,
    )?;
    Ok(id)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored<T> {
    id: String,
    body: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum AccessKind {
    Raw,
    Chart,
    Holdout,
    Worker,
    Export,
    History,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum LegacyReason {
    MigratedWorkspace,
    LegacyImport,
    UnmediatedAccess,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum ExposureClaim {
    Confirmation {
        operation_id: String,
        alpha_reservation_id: String,
    },
    Manual {
        access_kind: AccessKind,
        #[serde(deserialize_with = "required_nullable")]
        after_seal_id: Option<String>,
    },
    LegacyUnknown {
        reason: LegacyReason,
        #[serde(deserialize_with = "required_nullable")]
        source_evidence_sha256: Option<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExposureBody {
    contract_version: String,
    origin_registry_id: String,
    reservation_key: String,
    instrument_id: String,
    intervals: Vec<TimeInterval>,
    claim: ExposureClaim,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExposureRecord {
    id: String,
    body: ExposureBody,
}
impl ExposureRecord {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn origin_registry_id(&self) -> &str {
        &self.body.origin_registry_id
    }
    pub fn reservation_key(&self) -> &str {
        &self.body.reservation_key
    }
    pub fn operation_id(&self) -> Option<&str> {
        match &self.body.claim {
            ExposureClaim::Confirmation { operation_id, .. } => Some(operation_id),
            _ => None,
        }
    }
    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("checked exposure record")
    }
    pub fn footprint(&self) -> ExposureFootprint {
        ExposureFootprint::new(&self.body.instrument_id, self.body.intervals.clone())
            .expect("checked exposure intervals")
    }
}
fn checked_exposure(body: ExposureBody) -> Result<ExposureRecord, HistoryRecordError> {
    require(
        body.contract_version == EXPOSURE_VERSION,
        HistoryRecordError::Version,
    )?;
    reference(&body.origin_registry_id, 32)?;
    reference(&body.reservation_key, 32)?;
    ExposureFootprint::new(&body.instrument_id, body.intervals.clone())
        .map_err(|_| HistoryRecordError::Intervals)?;
    match &body.claim {
        ExposureClaim::Confirmation {
            operation_id,
            alpha_reservation_id,
        } => {
            reference(operation_id, 64)?;
            reference(alpha_reservation_id, 64)?;
        }
        ExposureClaim::Manual { after_seal_id, .. } => nullable_reference(after_seal_id)?,
        ExposureClaim::LegacyUnknown {
            source_evidence_sha256,
            ..
        } => nullable_reference(source_evidence_sha256)?,
    }
    Ok(ExposureRecord {
        id: identity(EXPOSURE_VERSION, &body)?,
        body,
    })
}
fn checked_operation(
    batch: &FrozenBatch,
    operation: &FrozenOperation,
) -> Result<(), HistoryRecordError> {
    require(
        operation.confirmation_batch_id() == batch.id(),
        HistoryRecordError::Binding,
    )
}
fn checked_confirmation_exposure(
    body: ExposureBody,
    batch: &FrozenBatch,
    operation: &FrozenOperation,
    alpha: &AlphaReservation,
) -> Result<ExposureRecord, HistoryRecordError> {
    let record = checked_exposure(body)?;
    checked_operation(batch, operation)?;
    let ExposureClaim::Confirmation {
        operation_id,
        alpha_reservation_id,
    } = &record.body.claim
    else {
        return Err(HistoryRecordError::Variant);
    };
    require(
        operation_id == operation.id()
            && alpha_reservation_id == alpha.reservation_id()
            && record.body.origin_registry_id == operation.origin_registry_id()
            && record.body.reservation_key == operation.reservation_key()
            && alpha.origin_registry_id() == operation.origin_registry_id()
            && alpha.reservation_key() == operation.reservation_key()
            && alpha.confirmation_batch_id() == batch.id()
            && alpha.family_id() == batch.source().family_id
            && alpha.allocation_id() == batch.allocation_id()
            && record.footprint() == batch.footprint(),
        HistoryRecordError::Binding,
    )?;
    Ok(record)
}
pub fn freeze_confirmation_exposure(
    raw: &str,
    batch: &FrozenBatch,
    operation: &FrozenOperation,
    alpha: &AlphaReservation,
) -> Result<ExposureRecord, HistoryRecordError> {
    checked_confirmation_exposure(read(raw)?, batch, operation, alpha)
}
pub fn parse_confirmation_exposure(
    raw: &str,
    batch: &FrozenBatch,
    operation: &FrozenOperation,
    alpha: &AlphaReservation,
) -> Result<ExposureRecord, HistoryRecordError> {
    let stored: Stored<ExposureBody> = read(raw)?;
    let result = checked_confirmation_exposure(stored.body, batch, operation, alpha)?;
    require(stored.id == result.id, HistoryRecordError::Identity)?;
    Ok(result)
}
fn checked_manual_exposure(body: ExposureBody) -> Result<ExposureRecord, HistoryRecordError> {
    let result = checked_exposure(body)?;
    require(
        !matches!(result.body.claim, ExposureClaim::Confirmation { .. }),
        HistoryRecordError::Variant,
    )?;
    Ok(result)
}
pub fn freeze_manual_exposure(raw: &str) -> Result<ExposureRecord, HistoryRecordError> {
    checked_manual_exposure(read(raw)?)
}
pub fn parse_manual_exposure(raw: &str) -> Result<ExposureRecord, HistoryRecordError> {
    let stored: Stored<ExposureBody> = read(raw)?;
    let result = checked_manual_exposure(stored.body)?;
    require(stored.id == result.id, HistoryRecordError::Identity)?;
    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum TerminalReason {
    OwnerLost,
    Cancelled,
    EngineMissing,
    InputMissing,
    ArtifactMissing,
    ExecutionFailed,
    SealMissing,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum ExecutionEvent {
    ExecutionStarted {
        workspace_id: String,
        ownership_epoch: u64,
        execution_key: String,
    },
    Sealed {
        execution_started_id: String,
        report_sha256: String,
        result_artifact_sha256: String,
        result_artifact_bytes: u64,
        engine_manifest_hash: String,
    },
    InterruptedSpent {
        #[serde(deserialize_with = "required_nullable")]
        execution_started_id: Option<String>,
        reason: TerminalReason,
    },
    FailedSpent {
        #[serde(deserialize_with = "required_nullable")]
        execution_started_id: Option<String>,
        reason: TerminalReason,
    },
    CancelledSpent {
        #[serde(deserialize_with = "required_nullable")]
        execution_started_id: Option<String>,
        reason: TerminalReason,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EventBody {
    contract_version: String,
    operation_id: String,
    admission_id: String,
    event: ExecutionEvent,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EventRecord {
    id: String,
    body: EventBody,
}
impl EventRecord {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn operation_id(&self) -> &str {
        &self.body.operation_id
    }
    pub fn admission_id(&self) -> &str {
        &self.body.admission_id
    }
    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("checked event record")
    }
}
fn checked_event(
    body: EventBody,
    batch: &FrozenBatch,
    operation: &FrozenOperation,
) -> Result<EventRecord, HistoryRecordError> {
    require(
        body.contract_version == EVENT_VERSION,
        HistoryRecordError::Version,
    )?;
    checked_operation(batch, operation)?;
    reference(&body.operation_id, 64)?;
    reference(&body.admission_id, 64)?;
    require(
        body.operation_id == operation.id(),
        HistoryRecordError::Binding,
    )?;
    match &body.event {
        ExecutionEvent::ExecutionStarted {
            workspace_id,
            ownership_epoch,
            execution_key,
        } => {
            reference(workspace_id, 32)?;
            reference(execution_key, 32)?;
            count(*ownership_epoch, 1)?;
            require(
                workspace_id == operation.workspace_id(),
                HistoryRecordError::Binding,
            )?;
        }
        ExecutionEvent::Sealed {
            execution_started_id,
            report_sha256,
            result_artifact_sha256,
            result_artifact_bytes,
            engine_manifest_hash,
        } => {
            for value in [
                execution_started_id,
                report_sha256,
                result_artifact_sha256,
                engine_manifest_hash,
            ] {
                reference(value, 64)?;
            }
            count(*result_artifact_bytes, 1)?;
            require(
                engine_manifest_hash == &batch.source().engine_manifest_hash,
                HistoryRecordError::Binding,
            )?;
        }
        ExecutionEvent::InterruptedSpent {
            execution_started_id,
            ..
        }
        | ExecutionEvent::FailedSpent {
            execution_started_id,
            ..
        }
        | ExecutionEvent::CancelledSpent {
            execution_started_id,
            ..
        } => nullable_reference(execution_started_id)?,
    }
    Ok(EventRecord {
        id: identity(EVENT_VERSION, &body)?,
        body,
    })
}
pub fn freeze_confirmation_event(
    raw: &str,
    batch: &FrozenBatch,
    operation: &FrozenOperation,
) -> Result<EventRecord, HistoryRecordError> {
    checked_event(read(raw)?, batch, operation)
}
pub fn parse_confirmation_event(
    raw: &str,
    batch: &FrozenBatch,
    operation: &FrozenOperation,
) -> Result<EventRecord, HistoryRecordError> {
    let stored: Stored<EventBody> = read(raw)?;
    let result = checked_event(stored.body, batch, operation)?;
    require(stored.id == result.id, HistoryRecordError::Identity)?;
    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ConflictKind {
    OriginKey,
    Admission,
    ExposureOverlap,
    ManualOverlap,
    ExecutionStarted,
    Seal,
    Terminal,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConflictBody {
    contract_version: String,
    kind: ConflictKind,
    claim_ids: Vec<String>,
    affected_family_ids: Vec<String>,
    affected_instrument_ids: Vec<String>,
}
/// Only a structurally checked declaration. No stop/coverage/authority API is
/// exposed until the complete graph verifies the actual relation and targets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConflictDeclaration {
    id: String,
    body: ConflictBody,
}
impl ConflictDeclaration {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("checked conflict declaration")
    }
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|p| p[0] < p[1])
}
fn checked_conflict(body: ConflictBody) -> Result<ConflictDeclaration, HistoryRecordError> {
    require(
        body.contract_version == CONFLICT_VERSION,
        HistoryRecordError::Version,
    )?;
    require(
        body.claim_ids.len() == 2 && ordered(&body.claim_ids),
        HistoryRecordError::Reference,
    )?;
    for value in &body.claim_ids {
        reference(value, 64)?;
    }
    require(
        (1..=2).contains(&body.affected_instrument_ids.len())
            && ordered(&body.affected_instrument_ids)
            && body.affected_family_ids.len() <= 2
            && ordered(&body.affected_family_ids),
        HistoryRecordError::Targets,
    )?;
    let families: BTreeSet<String> = body
        .affected_instrument_ids
        .iter()
        .map(|v| confirmation_family_id(v).map_err(|_| HistoryRecordError::Targets))
        .collect::<Result<_, _>>()?;
    require(
        body.affected_family_ids
            .iter()
            .all(|v| families.contains(v)),
        HistoryRecordError::Targets,
    )?;
    Ok(ConflictDeclaration {
        id: identity(CONFLICT_VERSION, &body)?,
        body,
    })
}
pub fn freeze_conflict_declaration(raw: &str) -> Result<ConflictDeclaration, HistoryRecordError> {
    checked_conflict(read(raw)?)
}
pub fn parse_conflict_declaration(raw: &str) -> Result<ConflictDeclaration, HistoryRecordError> {
    let stored: Stored<ConflictBody> = read(raw)?;
    let result = checked_conflict(stored.body)?;
    require(stored.id == result.id, HistoryRecordError::Identity)?;
    Ok(result)
}

#[cfg(test)]
mod tests;
