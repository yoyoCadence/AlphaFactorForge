//! Pure Train-only context assembly. Supplied provenance must be authenticated
//! by the future host; checked consistency is not a declassification authority.
use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    confirmation_contracts::{footprints_overlap, ExposureFootprint, TimeInterval},
    identity::canonical_bytes,
    precision::PRECISION_MAX_COUNT,
};

pub const SCOPE_VERSION: &str = "research-train-scope-v1";
pub const ORIGIN_VERSION: &str = "research-train-origin-v1";
pub const REQUEST_VERSION: &str = "research-train-context-request-v1";
pub const CONTEXT_VERSION: &str = "research-train-context-v1";
pub const MAX_DOCUMENT_BYTES: usize = 64 * 1024;
pub const MAX_ORIGINS: usize = 64;
pub const MAX_INPUTS: usize = 8;
pub const MAX_PROTECTION_ITEMS: usize = 4096;

/// Static diagnostics only: parser errors can contain protected keys/values.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ContextError {
    #[error("train_context.input_too_large")]
    InputTooLarge,
    #[error("train_context.malformed_input")]
    MalformedInput,
    #[error("train_context.invalid_scope")]
    InvalidScope,
    #[error("train_context.invalid_origin")]
    InvalidOrigin,
    #[error("train_context.protected_provenance")]
    ProtectedProvenance,
    #[error("train_context.stale_scope")]
    StaleScope,
    #[error("train_context.origin_set_mismatch")]
    OriginSetMismatch,
    #[error("train_context.protection_unknown")]
    ProtectionUnknown,
    #[error("train_context.protected_exposure")]
    ProtectedExposure,
    #[error("train_context.protected_reference")]
    ProtectedReference,
    #[error("train_context.capacity_exceeded")]
    CapacityExceeded,
    #[error("train_context.context_mismatch")]
    ContextMismatch,
}

fn require(ok: bool, error: ContextError) -> Result<(), ContextError> {
    if ok {
        Ok(())
    } else {
        Err(error)
    }
}

fn read<T: DeserializeOwned>(raw: &str) -> Result<T, ContextError> {
    require(raw.len() <= MAX_DOCUMENT_BYTES, ContextError::InputTooLarge)?;
    serde_json::from_str(raw).map_err(|_| ContextError::MalformedInput)
}

fn hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

fn digest(value: &str) -> Option<&str> {
    let value = value
        .strip_prefix("dataset-content-v2:")
        .or_else(|| value.strip_prefix("strategy-v2:"))
        .unwrap_or(value);
    hex(value, 64).then_some(value)
}

fn train_hash(value: &str) -> bool {
    value
        .strip_prefix("dataset-content-v2:")
        .is_some_and(|v| hex(v, 64))
}

fn identity<T: Serialize>(version: &str, body: &T) -> Result<String, ContextError> {
    let value = serde_json::to_value(body).map_err(|_| ContextError::MalformedInput)?;
    let bytes = canonical_bytes(&value).map_err(|_| ContextError::MalformedInput)?;
    let mut hash = Sha256::new();
    hash.update(version.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    Ok(hex::encode(hash.finalize()))
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScopeBody {
    contract_version: String,
    workspace_id: String,
    run_id: u64,
    campaign_id: String,
    instrument_id: String,
    train_data_hash: String,
    train_window: TimeInterval,
    engine_manifest_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainScope {
    id: String,
    body: ScopeBody,
}

impl TrainScope {
    pub fn id(&self) -> &str {
        &self.id
    }
    fn footprint(&self) -> Result<ExposureFootprint, ContextError> {
        ExposureFootprint::new(&self.body.instrument_id, vec![self.body.train_window])
            .map_err(|_| ContextError::InvalidScope)
    }
}

pub fn freeze_train_scope(raw: &str) -> Result<TrainScope, ContextError> {
    let body: ScopeBody = read(raw)?;
    require(
        body.contract_version == SCOPE_VERSION
            && hex(&body.workspace_id, 32)
            && (1..=PRECISION_MAX_COUNT).contains(&body.run_id)
            && hex(&body.campaign_id, 64)
            && hex(&body.engine_manifest_hash, 64)
            && train_hash(&body.train_data_hash),
        ContextError::InvalidScope,
    )?;
    let scope = TrainScope {
        id: identity(SCOPE_VERSION, &body)?,
        body,
    };
    scope.footprint()?;
    Ok(scope)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Provenance {
    TrainOnly,
    Validation,
    Test,
    Mixed,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    provenance: Provenance,
    instrument_id: String,
    train_data_hash: String,
    support: Vec<TimeInterval>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mechanism {
    TrendFollowing,
    MeanReversion,
    Breakout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Invalidation {
    InsufficientTrades,
    CostsDominate,
    UnstableSubwindows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum FailureClass {
    InsufficientHistory,
    NoTrades,
    NegativeNetReturn,
    UnstableSubwindows,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Fact {
    Summary {
        observations: u64,
        closed_trades: u64,
        net_return_ppm: i64,
    },
    Hypothesis {
        mechanism: Mechanism,
        invalidation: Invalidation,
    },
    Failure {
        class: FailureClass,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OriginBody {
    contract_version: String,
    scope_id: String,
    provenance: Provenance,
    inputs: Vec<Input>,
    fact: Fact,
}

/// Not serializable as a provider payload. Use the assembly gate below.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrainOrigin {
    id: String,
    body: OriginBody,
}

impl TrainOrigin {
    pub fn id(&self) -> &str {
        &self.id
    }
}

pub fn freeze_train_origin(raw: &str, scope: &TrainScope) -> Result<TrainOrigin, ContextError> {
    let body: OriginBody = read(raw)?;
    require(
        body.contract_version == ORIGIN_VERSION,
        ContextError::InvalidOrigin,
    )?;
    require(body.scope_id == scope.id, ContextError::StaleScope)?;
    require(
        body.provenance == Provenance::TrainOnly,
        ContextError::ProtectedProvenance,
    )?;
    require(
        (1..=MAX_INPUTS).contains(&body.inputs.len()),
        ContextError::CapacityExceeded,
    )?;
    for input in &body.inputs {
        require(
            input.provenance == Provenance::TrainOnly,
            ContextError::ProtectedProvenance,
        )?;
        require(
            input.instrument_id == scope.body.instrument_id
                && input.train_data_hash == scope.body.train_data_hash,
            ContextError::InvalidOrigin,
        )?;
        let footprint = ExposureFootprint::new(&input.instrument_id, input.support.clone())
            .map_err(|_| ContextError::InvalidOrigin)?;
        require(
            footprint.intervals().iter().all(|v| {
                v.from_ms() >= scope.body.train_window.from_ms()
                    && v.to_exclusive_ms() <= scope.body.train_window.to_exclusive_ms()
            }),
            ContextError::InvalidOrigin,
        )?;
    }
    if let Fact::Summary {
        observations,
        closed_trades,
        net_return_ppm,
    } = &body.fact
    {
        require(
            (1..=PRECISION_MAX_COUNT).contains(observations)
                && *closed_trades <= PRECISION_MAX_COUNT
                && net_return_ppm.unsigned_abs() <= PRECISION_MAX_COUNT,
            ContextError::InvalidOrigin,
        )?;
    }
    Ok(TrainOrigin {
        id: identity(ORIGIN_VERSION, &body)?,
        body,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryKnowledge {
    Complete,
    Unknown,
}

/// A current host observation, never a self-attested freshness certificate.
/// The host must authenticate and synchronize it at every assembly/use.
pub struct ObservedProtection {
    pub knowledge: HistoryKnowledge,
    pub exposures: Vec<ExposureFootprint>,
    pub protected_references: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    contract_version: String,
    scope_id: String,
    origin_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextFact {
    origin_id: String,
    fact: Fact,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextBody {
    contract_version: String,
    scope_id: String,
    scope: ScopeBody,
    facts: Vec<ContextFact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TrainContext {
    id: String,
    body: ContextBody,
}

impl TrainContext {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("checked Train integer/string context")
    }
}

fn assemble(
    scope: &TrainScope,
    origins: &[TrainOrigin],
    observed: &ObservedProtection,
) -> Result<TrainContext, ContextError> {
    require(
        observed.knowledge == HistoryKnowledge::Complete,
        ContextError::ProtectionUnknown,
    )?;
    require(
        observed.exposures.len() <= MAX_PROTECTION_ITEMS
            && observed.protected_references.len() <= MAX_PROTECTION_ITEMS
            && (1..=MAX_ORIGINS).contains(&origins.len()),
        ContextError::CapacityExceeded,
    )?;
    require(
        origins.windows(2).all(|p| p[0].id < p[1].id),
        ContextError::OriginSetMismatch,
    )?;
    require(
        origins.iter().all(|v| v.body.scope_id == scope.id),
        ContextError::StaleScope,
    )?;
    let train = scope.footprint()?;
    require(
        !observed
            .exposures
            .iter()
            .any(|v| footprints_overlap(&train, v)),
        ContextError::ProtectedExposure,
    )?;
    let protected: BTreeSet<&str> = observed
        .protected_references
        .iter()
        .map(|v| digest(v).ok_or(ContextError::ProtectedReference))
        .collect::<Result<_, _>>()?;
    let mut references = vec![
        scope.id.as_str(),
        scope.body.campaign_id.as_str(),
        scope.body.engine_manifest_hash.as_str(),
        scope.body.train_data_hash.as_str(),
    ];
    for origin in origins {
        references.extend([origin.id.as_str(), origin.body.scope_id.as_str()]);
        references.extend(
            origin
                .body
                .inputs
                .iter()
                .map(|v| v.train_data_hash.as_str()),
        );
    }
    require(
        !references
            .iter()
            .any(|v| digest(v).is_some_and(|v| protected.contains(v))),
        ContextError::ProtectedReference,
    )?;
    // Explicit provider DTO: do not serialize origin bodies, histories, generic
    // report values, request fields or parser diagnostics into this payload.
    let body = ContextBody {
        contract_version: CONTEXT_VERSION.into(),
        scope_id: scope.id.clone(),
        scope: scope.body.clone(),
        facts: origins
            .iter()
            .map(|v| ContextFact {
                origin_id: v.id.clone(),
                fact: v.body.fact.clone(),
            })
            .collect(),
    };
    let context = TrainContext {
        id: identity(CONTEXT_VERSION, &body)?,
        body,
    };
    require(
        serde_json::to_vec(&context)
            .map_err(|_| ContextError::MalformedInput)?
            .len()
            <= MAX_DOCUMENT_BYTES,
        ContextError::InputTooLarge,
    )?;
    Ok(context)
}

pub fn build_train_context(
    raw: &str,
    scope: &TrainScope,
    origins: &[TrainOrigin],
    observed: &ObservedProtection,
) -> Result<TrainContext, ContextError> {
    let request: Request = read(raw)?;
    require(
        request.contract_version == REQUEST_VERSION,
        ContextError::MalformedInput,
    )?;
    require(request.scope_id == scope.id, ContextError::StaleScope)?;
    require(
        request.origin_ids.len() <= MAX_ORIGINS,
        ContextError::CapacityExceeded,
    )?;
    require(
        request
            .origin_ids
            .iter()
            .map(String::as_str)
            .eq(origins.iter().map(TrainOrigin::id)),
        ContextError::OriginSetMismatch,
    )?;
    assemble(scope, origins, observed)
}

pub fn parse_train_context(
    raw: &str,
    scope: &TrainScope,
    origins: &[TrainOrigin],
    observed: &ObservedProtection,
) -> Result<TrainContext, ContextError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Stored {
        id: String,
        body: ContextBody,
    }
    let stored: Stored = read(raw)?;
    let expected = assemble(scope, origins, observed)?;
    require(
        stored.id == expected.id && stored.body == expected.body,
        ContextError::ContextMismatch,
    )?;
    Ok(expected)
}

#[cfg(test)]
mod tests;
