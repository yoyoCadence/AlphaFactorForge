//! P12b-1a — the trial ledger registry core (`trial-ledger-v1`,
//! `docs/trial-ledger-v1.md`).
//!
//! The registry is ONE SQLite database outside every workspace (spec §2) and
//! the sole authority for how many effective trials a family has had. This
//! module owns opening it (containment and volume checks, its own migration
//! sequence, chain verification), registering batches (§4, §6.1), the
//! historical receipt (§6.3), and the current admission count (§6.4) — the
//! only ledger value a P12a precision plan may be built from.
//!
//! Not here yet: export/import, union and checkpoint writes (P12b-1b), the
//! workspace binding and runner wiring (P12b-2), and admission blocking
//! (P12d). Nothing in the runtime calls this module yet.
//!
//! Chain encoding (§7.1): lowercase-hex SHA-256 digests; `chain_0 =
//! sha256("trial-ledger-v1:genesis:" + registryId)` and `chain_n =
//! sha256(chain_{n-1} + eventId)`, both concatenations of UTF-8 text.
//!
//! Host-agnostic: no desktop framework, no events, no UI.

// The first callers are the runner's register-before-enqueue path (P12b-2)
// and admission (P12d). Until then the compiler sees no caller outside this
// module's own tests; that is expected rather than a warning to chase, and
// no command surface is exposed for the same reason (market/mod.rs does the
// same for its not-yet-wired readers).
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{json, Value};

use super::{canonical_json, sha256_hex};
use alpha_factor_forge::discovery_core::benchmarks::BENCHMARK_CONTRACT_VERSION;
use alpha_factor_forge::discovery_core::market_foundation::{
    format_instrument_id, parse_instrument_id,
};
use alpha_factor_forge::discovery_core::precision::{PrecisionPlan, PRECISION_MAX_COUNT};
use alpha_factor_forge::discovery_core::random_entry::RANDOM_ENTRY_CONTRACT_VERSION;

pub const TRIAL_LEDGER_VERSION: &str = "trial-ledger-v1";
pub const TRIAL_FAMILY_VERSION: &str = "trial-family-v1";
pub const TRIAL_EVENT_VERSION: &str = "trial-event-v1";
pub const TRIAL_BATCH_VERSION: &str = "trial-batch-v1";
pub const HOLM_CORRECTION: &str = "holm";

/// Overrides the registry directory, like `AFF_DATA_DIR` does the workspace;
/// isolated tests and native smokes must set both (spec §2.1).
pub const REGISTRY_DIR_ENV: &str = "AFF_REGISTRY_DIR";
/// A sibling of — never inside — the `com.alphafactorforge.desktop` workspace.
pub const REGISTRY_DIR_NAME: &str = "com.alphafactorforge.evidence";
pub const REGISTRY_FILE_NAME: &str = "trial-ledger.sqlite3";

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const ONEDRIVE_ENV_VARS: [&str; 3] = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"];

/// The registry's own migrations (spec §2.2), separate from the workspace's.
/// ADD new migrations to the END only.
const REGISTRY_MIGRATIONS: &[(&str, &str)] = &[(
    "0001_trial_ledger",
    include_str!("../../registry_migrations/0001_trial_ledger.sql"),
)];

// ------------------------------------------------------------------ errors

#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("registry_inside_workspace: {0}")]
    RegistryInsideWorkspace(String),
    #[error("workspace_inside_registry: {0}")]
    WorkspaceInsideRegistry(String),
    #[error("registry_on_unsupported_volume: {0}")]
    UnsupportedVolume(String),
    #[error("registry_schema_newer: {0}")]
    SchemaTooNew(String),
    #[error("invalid_trial_batch: {0}")]
    InvalidBatch(String),
    #[error("idempotency_conflict: {0}")]
    IdempotencyConflict(String),
    #[error("batch_conflict: {0}")]
    BatchConflict(String),
    #[error("family_protocol_mismatch: {0}")]
    FamilyProtocolMismatch(String),
    #[error("family_quarantined: {0}")]
    FamilyQuarantined(String),
    #[error("reproduction_mismatch: {0}")]
    ReproductionMismatch(String),
    #[error("reproduction_reference_missing: {0}")]
    ReproductionReferenceMissing(String),
    #[error("batch_receipt_missing: {0}")]
    BatchReceiptMissing(String),
    #[error("batch_not_found: {0}")]
    BatchNotFound(String),
    #[error("registry_state_invalid: {0}")]
    StateInvalid(String),
}

impl LedgerError {
    /// The spec's reason code (the text before the colon in `Display`).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Db(_) => "database_error",
            Self::Io(_) => "io_error",
            Self::Serde(_) => "serialization_error",
            Self::RegistryInsideWorkspace(_) => "registry_inside_workspace",
            Self::WorkspaceInsideRegistry(_) => "workspace_inside_registry",
            Self::UnsupportedVolume(_) => "registry_on_unsupported_volume",
            Self::SchemaTooNew(_) => "registry_schema_newer",
            Self::InvalidBatch(_) => "invalid_trial_batch",
            Self::IdempotencyConflict(_) => "idempotency_conflict",
            Self::BatchConflict(_) => "batch_conflict",
            Self::FamilyProtocolMismatch(_) => "family_protocol_mismatch",
            Self::FamilyQuarantined(_) => "family_quarantined",
            Self::ReproductionMismatch(_) => "reproduction_mismatch",
            Self::ReproductionReferenceMissing(_) => "reproduction_reference_missing",
            Self::BatchReceiptMissing(_) => "batch_receipt_missing",
            Self::BatchNotFound(_) => "batch_not_found",
            Self::StateInvalid(_) => "registry_state_invalid",
        }
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T, LedgerError> {
    Err(LedgerError::InvalidBatch(message.into()))
}

fn canonical_text(value: &Value) -> Result<String, LedgerError> {
    let bytes =
        canonical_json(value).map_err(|error| LedgerError::StateInvalid(error.to_string()))?;
    String::from_utf8(bytes).map_err(|error| LedgerError::StateInvalid(error.to_string()))
}

// ------------------------------------------------------------------ inputs

/// The v1 closed set of trial kinds (spec §4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrialKind {
    Hypothesis,
    Variant,
    Diagnostic,
    Benchmark,
    Reproduction,
    Legacy,
}

impl TrialKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hypothesis => "hypothesis",
            Self::Variant => "variant",
            Self::Diagnostic => "diagnostic",
            Self::Benchmark => "benchmark",
            Self::Reproduction => "reproduction",
            Self::Legacy => "legacy",
        }
    }

    /// Spec §4.2: only a validated benchmark or reproduction is recorded
    /// without adding an effective trial. Validation happens before this
    /// value is ever stored.
    fn effective(self) -> bool {
        !matches!(self, Self::Benchmark | Self::Reproduction)
    }
}

/// Where the idempotency key comes from (spec §4.3): a new registration is
/// keyed by the enqueue command's `requestId`; only the §9 backfill uses a
/// P05 `attempt_key`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrialOrigin {
    Request {
        request_id: String,
        candidate_index: u64,
    },
    Legacy {
        attempt_key: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrialEventInput {
    pub kind: TrialKind,
    pub origin: TrialOrigin,
    pub hypothesis_hash: Option<String>,
    pub strategy_hash: Option<String>,
    pub dataset_hash: Option<String>,
    pub snapshot_id: Option<String>,
    pub split_hash: Option<String>,
    pub seeds_hash: Option<String>,
    pub engine_fingerprint_hash: Option<String>,
    pub benchmark_id: Option<String>,
    pub benchmark_params_hash: Option<String>,
    pub reproduction_of: Option<String>,
}

/// One batch = one family = one enqueue request (or one backfill).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrialBatchInput {
    pub workspace_id: String,
    /// The P06 instrument id; `None` is `family_unknown` (spec §3).
    pub instrument_id: Option<String>,
    /// The Holm protocol this batch declares; pinned on the family (§3).
    pub tests_per_trial: u64,
    pub events: Vec<TrialEventInput>,
}

// ------------------------------------------------------------------ outputs

/// Historical audit record (spec §6.3). Deliberately NOT named prior/planned
/// and not accepted by [`precision_plan_from_count`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistrationReceipt {
    pub receipt_registry_id: String,
    pub family_effective_before: u64,
    pub batch_effective_trials: u64,
}

/// The current count (spec §6.4). Fields are private: only this module
/// constructs one, so a caller cannot assemble it from a receipt or input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdmissionCount {
    registry_id: String,
    family_id: String,
    family_effective_trials: u64,
    batch_effective_trials: u64,
    tests_per_trial: u64,
    seq: u64,
    chain_head: String,
}

impl AdmissionCount {
    pub fn registry_id(&self) -> &str {
        &self.registry_id
    }
    pub fn family_id(&self) -> &str {
        &self.family_id
    }
    pub fn family_effective_trials(&self) -> u64 {
        self.family_effective_trials
    }
    pub fn batch_effective_trials(&self) -> u64 {
        self.batch_effective_trials
    }
    pub fn tests_per_trial(&self) -> u64 {
        self.tests_per_trial
    }
    pub fn seq(&self) -> u64 {
        self.seq
    }
    pub fn chain_head(&self) -> &str {
        &self.chain_head
    }
}

/// Why no admission count is available (spec §6.4); qualification stops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionBlocked {
    FamilyUnknown,
    FamilyQuarantined,
    ChainBroken,
    NoEffectiveTrials,
}

impl AdmissionBlocked {
    pub fn code(&self) -> &'static str {
        match self {
            Self::FamilyUnknown => "family_unknown",
            Self::FamilyQuarantined => "family_quarantined",
            Self::ChainBroken => "registry_chain_broken",
            Self::NoEffectiveTrials => "no_effective_trials",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admission {
    Count(AdmissionCount),
    Blocked(AdmissionBlocked),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchRegistration {
    pub batch_id: String,
    /// One per input event, in input order.
    pub event_ids: Vec<String>,
    /// `true` when this call replayed an already registered batch.
    pub replayed: bool,
    /// Current count, read in the same transaction (spec §6.4).
    pub admission: Admission,
    /// Historical audit only (spec §6.3).
    pub registration_receipt: RegistrationReceipt,
}

/// The sampling half of a P12a plan; the ledger supplies the family half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SamplingPlan {
    pub alpha_ppm: u64,
    pub max_relative_standard_error_ppm: u64,
    pub bootstrap_samples: u64,
    pub max_bootstrap_samples: u64,
}

/// The only ledger-to-P12a path (spec §5, §11): `priorTrials` is every
/// OTHER effective trial in the family at the snapshot — including trials
/// registered after this batch — never a registration-time number.
pub fn precision_plan_from_count(count: &AdmissionCount, sampling: &SamplingPlan) -> PrecisionPlan {
    PrecisionPlan {
        alpha_ppm: sampling.alpha_ppm,
        max_relative_standard_error_ppm: sampling.max_relative_standard_error_ppm,
        prior_trials: count.family_effective_trials - count.batch_effective_trials,
        planned_trials: count.batch_effective_trials,
        tests_per_trial: count.tests_per_trial,
        bootstrap_samples: sampling.bootstrap_samples,
        max_bootstrap_samples: sampling.max_bootstrap_samples,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerIntegrity {
    Intact {
        head_seq: u64,
    },
    /// Found at open (spec §7.1). Registration still appends, but every
    /// admission is blocked; this detects inconsistency, not tampering.
    ChainBroken {
        reason: String,
    },
}

// --------------------------------------------------------- pure preparation

/// Frozen benchmark identity (spec §4.2). Costs vary per run and are not
/// part of it; the strategy parameters of `benchmark-suite-v1` §6 and the
/// `random-entry-v1` contract are. A contract version change must revisit
/// this table (pinned by a test).
fn frozen_benchmark_params(id: &str) -> Option<Value> {
    let (contract, params) = match id {
        "buyHold" => (BENCHMARK_CONTRACT_VERSION, json!({})),
        "smaCross" => (
            BENCHMARK_CONTRACT_VERSION,
            json!({"fastMA": 50, "slowMA": 200}),
        ),
        "rsiReversion" => (
            BENCHMARK_CONTRACT_VERSION,
            json!({"rsiPeriod": 14, "rsiBuy": 30, "rsiSell": 70,
                   "entrySig": "rsiOversold", "exitSig": "rsiOverbought"}),
        ),
        "bollingerReversion" => (
            BENCHMARK_CONTRACT_VERSION,
            json!({"bbPeriod": 20, "bbMult": 2,
                   "entrySig": "bbLowerTouch", "exitSig": "bbUpperTouch"}),
        ),
        _ if id == RANDOM_ENTRY_CONTRACT_VERSION => (RANDOM_ENTRY_CONTRACT_VERSION, json!({})),
        _ => return None,
    };
    Some(json!({"benchmark": id, "contract": contract, "params": params}))
}

/// The `benchmarkParamsHash` a whitelisted benchmark must carry, or `None`
/// for any id outside the v1 whitelist.
pub fn benchmark_params_hash(id: &str) -> Option<String> {
    let document = frozen_benchmark_params(id)?;
    canonical_text(&document)
        .ok()
        .map(|text| sha256_hex(text.as_bytes()))
}

/// The family id for a canonical P06 instrument id (spec §3).
pub fn family_id_for(instrument_id: &str) -> Result<String, LedgerError> {
    Ok(family_parts(instrument_id)?.0)
}

fn family_parts(instrument_id: &str) -> Result<(String, String), LedgerError> {
    let parsed = parse_instrument_id(instrument_id).map_err(|rule| {
        LedgerError::InvalidBatch(format!("instrumentId {instrument_id:?}: {rule:?}"))
    })?;
    if format_instrument_id(&parsed) != instrument_id {
        return invalid(format!("instrumentId {instrument_id:?} is not canonical"));
    }
    let key =
        canonical_text(&json!({"version": TRIAL_FAMILY_VERSION, "instrumentId": instrument_id}))?;
    Ok((
        format!("{TRIAL_FAMILY_VERSION}:{}", sha256_hex(key.as_bytes())),
        key,
    ))
}

fn protocol_json(tests_per_trial: u64) -> Result<String, LedgerError> {
    canonical_text(&json!({"correction": HOLM_CORRECTION, "testsPerTrial": tests_per_trial}))
}

fn genesis(registry_id: &str) -> String {
    sha256_hex(format!("{TRIAL_LEDGER_VERSION}:genesis:{registry_id}").as_bytes())
}

fn next_chain(previous: &str, event_id: &str) -> String {
    sha256_hex(format!("{previous}{event_id}").as_bytes())
}

fn event_id_for(payload_json: &str) -> String {
    format!(
        "{TRIAL_EVENT_VERSION}:{}",
        sha256_hex(payload_json.as_bytes())
    )
}

fn batch_id_for(family_id: Option<&str>, members: &Value) -> Result<String, LedgerError> {
    let text = canonical_text(&json!({
        "version": TRIAL_BATCH_VERSION,
        "familyId": family_id,
        "members": members,
    }))?;
    Ok(format!(
        "{TRIAL_BATCH_VERSION}:{}",
        sha256_hex(text.as_bytes())
    ))
}

/// The five fields a reproduction must match exactly (spec §4.2).
const REPRODUCTION_IDENTITY: [&str; 5] = [
    "strategyHash",
    "datasetHash",
    "splitHash",
    "seedsHash",
    "engineFingerprintHash",
];

#[derive(Clone, Debug)]
struct PreparedEvent {
    input_index: usize,
    idempotency_key: String,
    kind: TrialKind,
    payload_json: String,
    payload: Value,
    event_id: String,
}

#[derive(Clone, Debug)]
struct PreparedBatch {
    family: Option<(String, String)>,
    protocol_json: String,
    batch_id: String,
    members_json: String,
    /// Sorted by idempotency key: the members order and the chain order.
    events: Vec<PreparedEvent>,
}

impl PreparedBatch {
    fn family_id(&self) -> Option<&str> {
        self.family.as_ref().map(|(id, _)| id.as_str())
    }
    fn effective_count(&self) -> u64 {
        self.events
            .iter()
            .filter(|event| event.kind.effective())
            .count() as u64
    }
}

fn check_text(field: &str, value: &Option<String>) -> Result<(), LedgerError> {
    match value {
        Some(text) if text.trim().is_empty() => invalid(format!("{field}: must not be empty")),
        _ => Ok(()),
    }
}

fn require(field: &str, value: &Option<String>, kind: TrialKind) -> Result<(), LedgerError> {
    if value.is_none() {
        return invalid(format!("{field}: required for kind {}", kind.as_str()));
    }
    Ok(())
}

fn forbid(field: &str, value: &Option<String>, kind: TrialKind) -> Result<(), LedgerError> {
    if value.is_some() {
        return invalid(format!("{field}: must be null for kind {}", kind.as_str()));
    }
    Ok(())
}

/// Validates a batch against spec §3/§4 and computes every identity. Pure:
/// only the reproduction reference check needs the registry.
fn prepare_batch(input: &TrialBatchInput) -> Result<PreparedBatch, LedgerError> {
    if input.events.is_empty() {
        return invalid("a batch needs at least one event");
    }
    let workspace = input.workspace_id.as_str();
    if workspace.trim().is_empty() || workspace.contains(':') {
        return invalid("workspaceId: must be non-empty and contain no ':'");
    }
    if !(1..=PRECISION_MAX_COUNT).contains(&input.tests_per_trial) {
        return invalid(format!(
            "testsPerTrial: must be an integer in [1, {PRECISION_MAX_COUNT}]"
        ));
    }
    let family = match &input.instrument_id {
        Some(instrument) => Some(family_parts(instrument)?),
        None => None,
    };
    let family_id = family.as_ref().map(|(id, _)| id.clone());

    let legacy_batch = matches!(input.events[0].origin, TrialOrigin::Legacy { .. });
    let mut request_ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut prepared = Vec::with_capacity(input.events.len());
    for (index, event) in input.events.iter().enumerate() {
        let kind = event.kind;
        for (field, value) in [
            ("hypothesisHash", &event.hypothesis_hash),
            ("strategyHash", &event.strategy_hash),
            ("datasetHash", &event.dataset_hash),
            ("snapshotId", &event.snapshot_id),
            ("splitHash", &event.split_hash),
            ("seedsHash", &event.seeds_hash),
            ("engineFingerprintHash", &event.engine_fingerprint_hash),
            ("benchmarkId", &event.benchmark_id),
            ("benchmarkParamsHash", &event.benchmark_params_hash),
            ("reproductionOf", &event.reproduction_of),
        ] {
            check_text(field, value)?;
        }
        if event.snapshot_id.is_some() != input.instrument_id.is_some() {
            return invalid("snapshotId: present exactly when the batch has an instrumentId");
        }
        let (idempotency_key, request_id, candidate_index, legacy_attempt_key) = match &event.origin
        {
            TrialOrigin::Request {
                request_id,
                candidate_index,
            } => {
                if legacy_batch || kind == TrialKind::Legacy {
                    return invalid(
                        "kind legacy and legacy origins must not mix with request origins",
                    );
                }
                if request_id.trim().is_empty() || request_id.contains(':') {
                    return invalid("requestId: must be non-empty and contain no ':'");
                }
                request_ids.insert(request_id.clone());
                (
                    format!("{workspace}:request:{request_id}:candidate:{candidate_index}"),
                    Value::String(request_id.clone()),
                    json!(candidate_index),
                    Value::Null,
                )
            }
            TrialOrigin::Legacy { attempt_key } => {
                if !legacy_batch || kind != TrialKind::Legacy {
                    return invalid("a legacy origin requires kind legacy in an all-legacy batch");
                }
                if attempt_key.trim().is_empty() {
                    return invalid("attemptKey: must not be empty");
                }
                (
                    format!("{workspace}:{attempt_key}"),
                    Value::Null,
                    Value::Null,
                    Value::String(attempt_key.clone()),
                )
            }
        };
        if request_ids.len() > 1 {
            return invalid("all events of a request batch share one requestId");
        }
        if !keys.insert(idempotency_key.clone()) {
            return invalid(format!("duplicate idempotency key {idempotency_key}"));
        }

        match kind {
            TrialKind::Hypothesis | TrialKind::Variant | TrialKind::Diagnostic => {
                for (field, value) in [
                    ("strategyHash", &event.strategy_hash),
                    ("datasetHash", &event.dataset_hash),
                    ("splitHash", &event.split_hash),
                    ("seedsHash", &event.seeds_hash),
                    ("engineFingerprintHash", &event.engine_fingerprint_hash),
                ] {
                    require(field, value, kind)?;
                }
                forbid("benchmarkId", &event.benchmark_id, kind)?;
                forbid("benchmarkParamsHash", &event.benchmark_params_hash, kind)?;
                forbid("reproductionOf", &event.reproduction_of, kind)?;
            }
            TrialKind::Legacy => {
                // Split and seeds may be unrecoverable for a backfilled attempt.
                for (field, value) in [
                    ("strategyHash", &event.strategy_hash),
                    ("datasetHash", &event.dataset_hash),
                    ("engineFingerprintHash", &event.engine_fingerprint_hash),
                ] {
                    require(field, value, kind)?;
                }
                forbid("benchmarkId", &event.benchmark_id, kind)?;
                forbid("benchmarkParamsHash", &event.benchmark_params_hash, kind)?;
                forbid("reproductionOf", &event.reproduction_of, kind)?;
            }
            TrialKind::Benchmark => {
                require("datasetHash", &event.dataset_hash, kind)?;
                forbid("reproductionOf", &event.reproduction_of, kind)?;
                let id = event.benchmark_id.as_deref().unwrap_or("");
                let Some(expected) = benchmark_params_hash(id) else {
                    return invalid(format!(
                        "benchmarkId {id:?} is not in the v1 whitelist; register it as a variant"
                    ));
                };
                if event.benchmark_params_hash.as_deref() != Some(expected.as_str()) {
                    return invalid(format!(
                        "benchmarkParamsHash does not match the frozen {id} parameters; register it as a variant"
                    ));
                }
            }
            TrialKind::Reproduction => {
                require("reproductionOf", &event.reproduction_of, kind)?;
                for (field, value) in [
                    ("strategyHash", &event.strategy_hash),
                    ("datasetHash", &event.dataset_hash),
                    ("splitHash", &event.split_hash),
                    ("seedsHash", &event.seeds_hash),
                    ("engineFingerprintHash", &event.engine_fingerprint_hash),
                ] {
                    if value.is_none() {
                        return Err(LedgerError::ReproductionMismatch(format!(
                            "{field} is required for a reproduction"
                        )));
                    }
                }
                forbid("benchmarkId", &event.benchmark_id, kind)?;
                forbid("benchmarkParamsHash", &event.benchmark_params_hash, kind)?;
            }
        }

        // Every field always present; `null` when not applicable (spec §4.3).
        let payload = json!({
            "version": TRIAL_EVENT_VERSION,
            "familyId": family_id,
            "kind": kind.as_str(),
            "effective": kind.effective(),
            "idempotencyKey": idempotency_key,
            "workspaceId": workspace,
            "requestId": request_id,
            "candidateIndex": candidate_index,
            "legacyAttemptKey": legacy_attempt_key,
            "hypothesisHash": event.hypothesis_hash,
            "strategyHash": event.strategy_hash,
            "datasetHash": event.dataset_hash,
            "snapshotId": event.snapshot_id,
            "splitHash": event.split_hash,
            "seedsHash": event.seeds_hash,
            "engineFingerprintHash": event.engine_fingerprint_hash,
            "benchmarkId": event.benchmark_id,
            "benchmarkParamsHash": event.benchmark_params_hash,
            "reproductionOf": event.reproduction_of,
        });
        let payload_json = canonical_text(&payload)?;
        let event_id = event_id_for(&payload_json);
        prepared.push(PreparedEvent {
            input_index: index,
            idempotency_key,
            kind,
            payload_json,
            payload,
            event_id,
        });
    }

    prepared.sort_by(|a, b| a.idempotency_key.cmp(&b.idempotency_key));
    let members = Value::Array(
        prepared
            .iter()
            .map(|event| json!([event.idempotency_key, event.event_id]))
            .collect(),
    );
    let members_json = canonical_text(&members)?;
    let batch_id = batch_id_for(family_id.as_deref(), &members)?;
    Ok(PreparedBatch {
        family,
        protocol_json: protocol_json(input.tests_per_trial)?,
        batch_id,
        members_json,
        events: prepared,
    })
}

// -------------------------------------------------------------- opening

/// `AFF_REGISTRY_DIR`, else `<platform data_local_dir>/com.alphafactorforge.evidence`
/// (Windows: `%LOCALAPPDATA%`, not the roaming workspace directory).
pub fn default_registry_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(REGISTRY_DIR_ENV).filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    dirs::data_local_dir().map(|dir| dir.join(REGISTRY_DIR_NAME))
}

/// Known sync-client roots (OneDrive) that exist on this machine.
fn sync_roots_from_env() -> Vec<PathBuf> {
    ONEDRIVE_ENV_VARS
        .iter()
        .filter_map(std::env::var_os)
        .filter(|value| !value.is_empty())
        .filter_map(|value| std::fs::canonicalize(PathBuf::from(value)).ok())
        .collect()
}

/// Spec §2.1 detection rule: a network (UNC) path, or a path under a known
/// sync-client root, cannot hold the registry. Both inputs are canonical.
fn is_unsupported_volume(path: &Path, sync_roots: &[PathBuf]) -> bool {
    let text = path.to_string_lossy();
    if text.starts_with(r"\\?\UNC\") || (text.starts_with(r"\\") && !text.starts_with(r"\\?\")) {
        return true;
    }
    sync_roots.iter().any(|root| path.starts_with(root))
}

fn new_registry_id() -> Result<String, LedgerError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| {
        LedgerError::Io(std::io::Error::other(format!("os randomness: {error}")))
    })?;
    Ok(hex::encode(bytes))
}

fn migrate(conn: &mut Connection) -> Result<(), LedgerError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS registry_migrations (
            version    TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    let applied: Vec<String> = {
        let mut stmt = conn.prepare("SELECT version FROM registry_migrations ORDER BY version")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let unknown: Vec<&str> = applied
        .iter()
        .map(String::as_str)
        .filter(|version| {
            !REGISTRY_MIGRATIONS
                .iter()
                .any(|(known, _)| known == version)
        })
        .collect();
    if !unknown.is_empty() {
        return Err(LedgerError::SchemaTooNew(format!(
            "this build knows registry migrations up to {}, but the registry already has {}",
            REGISTRY_MIGRATIONS
                .last()
                .map(|(v, _)| *v)
                .unwrap_or("none"),
            unknown.join(", ")
        )));
    }
    for (version, sql) in REGISTRY_MIGRATIONS {
        // Re-checked inside the write transaction: two processes may open a
        // fresh registry at the same moment.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let done = tx
            .query_row(
                "SELECT 1 FROM registry_migrations WHERE version = ?1",
                [version],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !done {
            tx.execute_batch(sql)?;
            if *version == "0001_trial_ledger" {
                tx.execute(
                    "INSERT INTO registry_meta (key, value) VALUES ('registry_id', ?1)",
                    [new_registry_id()?],
                )?;
            }
            tx.execute(
                "INSERT INTO registry_migrations (version) VALUES (?1)",
                [version],
            )?;
        }
        tx.commit()?;
    }
    Ok(())
}

fn broken(reason: String) -> Result<LedgerIntegrity, LedgerError> {
    Ok(LedgerIntegrity::ChainBroken { reason })
}

/// Recomputes every event id, the whole chain, and every batch's members
/// (spec §7.1). Any mismatch is reported, never repaired.
fn verify_chain(conn: &Connection, registry_id: &str) -> Result<LedgerIntegrity, LedgerError> {
    let mut stmt = conn.prepare(
        "SELECT seq, chain, event_id, payload_json, family_id, kind, effective,
                idempotency_key, batch_id
           FROM trial_events ORDER BY seq",
    )?;
    let mut rows = stmt.query([])?;
    let mut previous = genesis(registry_id);
    let mut expected_seq = 1i64;
    let mut members_by_batch: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let seq: i64 = row.get(0)?;
        let chain: String = row.get(1)?;
        let event_id: String = row.get(2)?;
        let payload_json: String = row.get(3)?;
        let family_id: Option<String> = row.get(4)?;
        let kind: String = row.get(5)?;
        let effective: i64 = row.get(6)?;
        let key: String = row.get(7)?;
        let batch_id: String = row.get(8)?;
        if seq != expected_seq {
            return broken(format!("seq {seq} where {expected_seq} was expected"));
        }
        if event_id_for(&payload_json) != event_id {
            return broken(format!("seq {seq}: event id does not match its payload"));
        }
        let payload: Value = serde_json::from_str(&payload_json)?;
        if payload["familyId"].as_str() != family_id.as_deref()
            || payload["kind"].as_str() != Some(kind.as_str())
            || payload["effective"].as_bool() != Some(effective == 1)
            || payload["idempotencyKey"].as_str() != Some(key.as_str())
        {
            return broken(format!("seq {seq}: columns disagree with the payload"));
        }
        let next = next_chain(&previous, &event_id);
        if next != chain {
            return broken(format!("seq {seq}: chain value does not follow"));
        }
        members_by_batch
            .entry(batch_id)
            .or_default()
            .push((key, event_id));
        previous = next;
        expected_seq += 1;
    }
    drop(rows);
    drop(stmt);

    let mut stmt = conn.prepare("SELECT batch_id, family_id, members_json FROM trial_batches")?;
    let batches = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (batch_id, family_id, members_json) in batches {
        let members: Value = serde_json::from_str(&members_json)?;
        if batch_id_for(family_id.as_deref(), &members)? != batch_id {
            return broken(format!("batch {batch_id}: id does not match its members"));
        }
        let mut stored = members_by_batch.remove(&batch_id).unwrap_or_default();
        stored.sort();
        let listed: Vec<(String, String)> = members
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|pair| {
                        (
                            pair[0].as_str().unwrap_or_default().to_string(),
                            pair[1].as_str().unwrap_or_default().to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if stored != listed {
            return broken(format!(
                "batch {batch_id}: members disagree with its events"
            ));
        }
    }
    if let Some(orphan) = members_by_batch.keys().next() {
        return broken(format!("events reference unknown batch {orphan}"));
    }
    Ok(LedgerIntegrity::Intact {
        head_seq: (expected_seq - 1) as u64,
    })
}

// ------------------------------------------------------------ the ledger

pub struct TrialLedger {
    conn: Mutex<Connection>,
    registry_id: String,
    integrity: LedgerIntegrity,
    path: PathBuf,
}

impl TrialLedger {
    /// Opens (creating if needed) the registry in `registry_dir` for the
    /// workspace at `workspace_dir`. Both directories are canonicalized and
    /// must not contain one another (spec §2.1); the workspace must exist.
    pub fn open(registry_dir: &Path, workspace_dir: &Path) -> Result<Self, LedgerError> {
        Self::open_with_sync_roots(registry_dir, workspace_dir, &sync_roots_from_env())
    }

    fn open_with_sync_roots(
        registry_dir: &Path,
        workspace_dir: &Path,
        sync_roots: &[PathBuf],
    ) -> Result<Self, LedgerError> {
        std::fs::create_dir_all(registry_dir)?;
        let registry = std::fs::canonicalize(registry_dir)?;
        let workspace = std::fs::canonicalize(workspace_dir)?;
        if registry.starts_with(&workspace) {
            return Err(LedgerError::RegistryInsideWorkspace(format!(
                "{} is inside the workspace {}",
                registry.display(),
                workspace.display()
            )));
        }
        if workspace.starts_with(&registry) {
            return Err(LedgerError::WorkspaceInsideRegistry(format!(
                "the workspace {} is inside {}",
                workspace.display(),
                registry.display()
            )));
        }
        if is_unsupported_volume(&registry, sync_roots) {
            return Err(LedgerError::UnsupportedVolume(format!(
                "{} is on a network or synced volume",
                registry.display()
            )));
        }
        let path = registry.join(REGISTRY_FILE_NAME);
        let mut conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        migrate(&mut conn)?;
        let registry_id: String = conn
            .query_row(
                "SELECT value FROM registry_meta WHERE key = 'registry_id'",
                [],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| LedgerError::StateInvalid("registry_id is missing".into()))?;
        let integrity = verify_chain(&conn, &registry_id)?;
        Ok(Self {
            conn: Mutex::new(conn),
            registry_id,
            integrity,
            path,
        })
    }

    pub fn registry_id(&self) -> &str {
        &self.registry_id
    }

    pub fn integrity(&self) -> &LedgerIntegrity {
        &self.integrity
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, LedgerError> {
        self.conn
            .lock()
            .map_err(|_| LedgerError::StateInvalid("registry connection lock poisoned".into()))
    }

    /// Registers (or verifies and replays) one batch in a single
    /// `BEGIN IMMEDIATE` transaction (spec §6.1), returning the historical
    /// receipt AND the current admission count read in that same transaction.
    pub fn register_batch(
        &self,
        input: &TrialBatchInput,
    ) -> Result<BatchRegistration, LedgerError> {
        let batch = prepare_batch(input)?;
        let mut conn = self.lock()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let family_id = batch.family_id().map(str::to_string);

        // 1. quarantine
        if let Some(id) = &family_id {
            if is_quarantined(&tx, id)? {
                return Err(LedgerError::FamilyQuarantined(id.clone()));
            }
        }
        // 3. per-key comparison — always, before any replay. Two passes: a
        //    changed payload under an existing key is reported as
        //    `idempotency_conflict` even when an unchanged sibling key would
        //    also fail batch membership (A27 before A28).
        let mut found = Vec::new();
        for event in &batch.events {
            let row: Option<(String, String)> = tx
                .query_row(
                    "SELECT event_id, batch_id FROM trial_events WHERE idempotency_key = ?1",
                    [&event.idempotency_key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((event_id, batch_id)) = row {
                if event_id != event.event_id {
                    return Err(LedgerError::IdempotencyConflict(format!(
                        "{} is already registered with different content",
                        event.idempotency_key
                    )));
                }
                found.push((event.idempotency_key.as_str(), batch_id));
            }
        }
        for (key, batch_id) in &found {
            if *batch_id != batch.batch_id {
                return Err(LedgerError::BatchConflict(format!(
                    "{key} already belongs to batch {batch_id}"
                )));
            }
        }
        let existing = found.len();
        // The declared protocol must match the family's pin, on replay too.
        if let Some(id) = &family_id {
            if let Some(pinned) = pinned_protocol(&tx, id)? {
                if pinned != batch.protocol_json {
                    return Err(LedgerError::FamilyProtocolMismatch(format!(
                        "{id} is pinned to {pinned}, the batch declares {}",
                        batch.protocol_json
                    )));
                }
            }
        }

        let replayed = existing > 0;
        let receipt = if replayed {
            if existing != batch.events.len() {
                return Err(LedgerError::StateInvalid(format!(
                    "batch {} is only partly present",
                    batch.batch_id
                )));
            }
            load_receipt(&tx, &batch.batch_id, &self.registry_id)?
        } else {
            for event in &batch.events {
                if event.kind == TrialKind::Reproduction {
                    verify_reproduction(&tx, event, family_id.as_deref())?;
                }
            }
            let before = count_family_effective(&tx, family_id.as_deref())?;
            let now = chrono::Utc::now().to_rfc3339();
            if let Some((id, key)) = &batch.family {
                tx.execute(
                    "INSERT OR IGNORE INTO trial_families (family_id, family_key, protocol_json, created_at)
                     VALUES (?1, ?2, NULL, ?3)",
                    params![id, key, now],
                )?;
            }
            tx.execute(
                "INSERT INTO trial_batches (batch_id, family_id, members_json, origin_registry_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![batch.batch_id, family_id, batch.members_json, self.registry_id, now],
            )?;
            let (mut seq, mut chain) = head(&tx, &self.registry_id)?;
            for event in &batch.events {
                seq += 1;
                chain = next_chain(&chain, &event.event_id);
                tx.execute(
                    "INSERT INTO trial_events (event_id, seq, chain, family_id, idempotency_key, kind,
                                               effective, batch_id, payload_json, origin_registry_id,
                                               registered_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        event.event_id,
                        seq as i64,
                        chain,
                        family_id,
                        event.idempotency_key,
                        event.kind.as_str(),
                        event.kind.effective() as i64,
                        batch.batch_id,
                        event.payload_json,
                        self.registry_id,
                        now,
                    ],
                )?;
            }
            let batch_effective = batch.effective_count();
            if batch_effective > 0 {
                if let Some(id) = &family_id {
                    tx.execute(
                        "UPDATE trial_families SET protocol_json = ?2
                          WHERE family_id = ?1 AND protocol_json IS NULL",
                        params![id, batch.protocol_json],
                    )?;
                }
            }
            tx.execute(
                "INSERT INTO batch_receipts (batch_id, receipt_registry_id, family_effective_before,
                                             batch_effective_trials)
                 VALUES (?1, ?2, ?3, ?4)",
                params![batch.batch_id, self.registry_id, before as i64, batch_effective as i64],
            )?;
            RegistrationReceipt {
                receipt_registry_id: self.registry_id.clone(),
                family_effective_before: before,
                batch_effective_trials: batch_effective,
            }
        };
        // 6. the current count, in this same transaction.
        let admission = admission_in(
            &tx,
            &batch.batch_id,
            family_id.as_deref(),
            &self.registry_id,
            &self.integrity,
        )?;
        tx.commit()?;

        let mut event_ids = vec![String::new(); batch.events.len()];
        for event in &batch.events {
            event_ids[event.input_index] = event.event_id.clone();
        }
        Ok(BatchRegistration {
            batch_id: batch.batch_id,
            event_ids,
            replayed,
            admission,
            registration_receipt: receipt,
        })
    }

    /// Re-reads the current count for an already registered batch in one
    /// read transaction (spec §6.4 fence re-read).
    pub fn read_admission_count(&self, batch_id: &str) -> Result<Admission, LedgerError> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let family_id: Option<Option<String>> = tx
            .query_row(
                "SELECT family_id FROM trial_batches WHERE batch_id = ?1",
                [batch_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(family_id) = family_id else {
            return Err(LedgerError::BatchNotFound(batch_id.to_string()));
        };
        let admission = admission_in(
            &tx,
            batch_id,
            family_id.as_deref(),
            &self.registry_id,
            &self.integrity,
        )?;
        tx.commit()?;
        Ok(admission)
    }

    /// The chain value at `seq`, for the §6.4 / §7 prefix checks.
    pub fn chain_at(&self, seq: u64) -> Result<Option<String>, LedgerError> {
        let conn = self.lock()?;
        Ok(conn
            .query_row(
                "SELECT chain FROM trial_events WHERE seq = ?1",
                [seq as i64],
                |row| row.get(0),
            )
            .optional()?)
    }
}

fn is_quarantined(tx: &Transaction<'_>, family_id: &str) -> Result<bool, LedgerError> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM family_conflicts WHERE family_id = ?1 LIMIT 1",
            [family_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn pinned_protocol(tx: &Transaction<'_>, family_id: &str) -> Result<Option<String>, LedgerError> {
    Ok(tx
        .query_row(
            "SELECT protocol_json FROM trial_families WHERE family_id = ?1",
            [family_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

fn count_family_effective(
    tx: &Transaction<'_>,
    family_id: Option<&str>,
) -> Result<u64, LedgerError> {
    let count: i64 = match family_id {
        Some(id) => tx.query_row(
            "SELECT COUNT(*) FROM trial_events WHERE family_id = ?1 AND effective = 1",
            [id],
            |row| row.get(0),
        )?,
        None => tx.query_row(
            "SELECT COUNT(*) FROM trial_events WHERE family_id IS NULL AND effective = 1",
            [],
            |row| row.get(0),
        )?,
    };
    Ok(count as u64)
}

fn head(tx: &Transaction<'_>, registry_id: &str) -> Result<(u64, String), LedgerError> {
    let row: Option<(i64, String)> = tx
        .query_row(
            "SELECT seq, chain FROM trial_events ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(match row {
        Some((seq, chain)) => (seq as u64, chain),
        None => (0, genesis(registry_id)),
    })
}

fn load_receipt(
    tx: &Transaction<'_>,
    batch_id: &str,
    registry_id: &str,
) -> Result<RegistrationReceipt, LedgerError> {
    let origin: String = tx.query_row(
        "SELECT origin_registry_id FROM trial_batches WHERE batch_id = ?1",
        [batch_id],
        |row| row.get(0),
    )?;
    // Spec §6.3: this registry's own receipt, else the batch origin's.
    for candidate in [registry_id, origin.as_str()] {
        let row: Option<(i64, i64)> = tx
            .query_row(
                "SELECT family_effective_before, batch_effective_trials FROM batch_receipts
                  WHERE batch_id = ?1 AND receipt_registry_id = ?2",
                params![batch_id, candidate],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((before, batch_effective)) = row {
            return Ok(RegistrationReceipt {
                receipt_registry_id: candidate.to_string(),
                family_effective_before: before as u64,
                batch_effective_trials: batch_effective as u64,
            });
        }
    }
    Err(LedgerError::BatchReceiptMissing(batch_id.to_string()))
}

fn verify_reproduction(
    tx: &Transaction<'_>,
    event: &PreparedEvent,
    family_id: Option<&str>,
) -> Result<(), LedgerError> {
    let reference = event.payload["reproductionOf"].as_str().unwrap_or_default();
    let row: Option<(Option<String>, String)> = tx
        .query_row(
            "SELECT family_id, payload_json FROM trial_events WHERE event_id = ?1",
            [reference],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((reference_family, reference_payload)) = row else {
        return Err(LedgerError::ReproductionReferenceMissing(
            reference.to_string(),
        ));
    };
    if reference_family.as_deref() != family_id {
        return Err(LedgerError::ReproductionMismatch(format!(
            "{reference} belongs to another family"
        )));
    }
    let original: Value = serde_json::from_str(&reference_payload)?;
    for field in REPRODUCTION_IDENTITY {
        let theirs = original[field].as_str();
        if theirs.is_none() || theirs != event.payload[field].as_str() {
            return Err(LedgerError::ReproductionMismatch(format!(
                "{field} differs from (or is missing in) {reference}"
            )));
        }
    }
    Ok(())
}

fn admission_in(
    tx: &Transaction<'_>,
    batch_id: &str,
    family_id: Option<&str>,
    registry_id: &str,
    integrity: &LedgerIntegrity,
) -> Result<Admission, LedgerError> {
    if matches!(integrity, LedgerIntegrity::ChainBroken { .. }) {
        return Ok(Admission::Blocked(AdmissionBlocked::ChainBroken));
    }
    let Some(family_id) = family_id else {
        return Ok(Admission::Blocked(AdmissionBlocked::FamilyUnknown));
    };
    if is_quarantined(tx, family_id)? {
        return Ok(Admission::Blocked(AdmissionBlocked::FamilyQuarantined));
    }
    let batch_effective: i64 = tx.query_row(
        "SELECT COUNT(*) FROM trial_events WHERE batch_id = ?1 AND effective = 1",
        [batch_id],
        |row| row.get(0),
    )?;
    if batch_effective == 0 {
        return Ok(Admission::Blocked(AdmissionBlocked::NoEffectiveTrials));
    }
    let protocol = pinned_protocol(tx, family_id)?.ok_or_else(|| {
        LedgerError::StateInvalid(format!("{family_id} has effective trials but no protocol"))
    })?;
    let protocol: Value = serde_json::from_str(&protocol)?;
    let tests_per_trial = protocol["testsPerTrial"].as_u64().ok_or_else(|| {
        LedgerError::StateInvalid(format!("{family_id} protocol has no testsPerTrial"))
    })?;
    let family_effective = count_family_effective(tx, Some(family_id))?;
    let (seq, chain_head) = head(tx, registry_id)?;
    Ok(Admission::Count(AdmissionCount {
        registry_id: registry_id.to_string(),
        family_id: family_id.to_string(),
        family_effective_trials: family_effective,
        batch_effective_trials: batch_effective as u64,
        tests_per_trial,
        seq,
        chain_head,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use super::*;
    use alpha_factor_forge::discovery_core::benchmarks::DETERMINISTIC_BENCHMARK_IDS;
    use alpha_factor_forge::discovery_core::precision::{
        evaluate_precision_plan, PrecisionReason, PrecisionStatus,
    };

    const BTC: &str = "crypto:binance:BTCUSDT";

    struct Dirs {
        root: PathBuf,
        registry: PathBuf,
        workspace: PathBuf,
    }

    impl Drop for Dirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn scratch() -> Dirs {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("aff-trial-ledger-test-{}-{n}", std::process::id()));
        let registry = root.join("evidence");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        Dirs {
            root,
            registry,
            workspace,
        }
    }

    fn open(dirs: &Dirs) -> TrialLedger {
        TrialLedger::open_with_sync_roots(&dirs.registry, &dirs.workspace, &[]).unwrap()
    }

    fn event(kind: TrialKind, request: &str, index: u64) -> TrialEventInput {
        TrialEventInput {
            kind,
            origin: TrialOrigin::Request {
                request_id: request.into(),
                candidate_index: index,
            },
            hypothesis_hash: Some("hypothesis-a".into()),
            strategy_hash: Some(format!("strategy-{request}-{index}")),
            dataset_hash: Some("dataset-1".into()),
            snapshot_id: Some("snapshot-1".into()),
            split_hash: Some("split-1".into()),
            seeds_hash: Some(format!("seeds-{request}-{index}")),
            engine_fingerprint_hash: Some("engine-1".into()),
            benchmark_id: None,
            benchmark_params_hash: None,
            reproduction_of: None,
        }
    }

    fn batch(request: &str, count: u64) -> TrialBatchInput {
        TrialBatchInput {
            workspace_id: "ws1".into(),
            instrument_id: Some(BTC.into()),
            tests_per_trial: 2,
            events: (0..count)
                .map(|index| event(TrialKind::Variant, request, index))
                .collect(),
        }
    }

    fn count(admission: &Admission) -> &AdmissionCount {
        match admission {
            Admission::Count(count) => count,
            Admission::Blocked(reason) => panic!("admission blocked: {reason:?}"),
        }
    }

    fn event_rows(ledger: &TrialLedger) -> i64 {
        let conn = ledger.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM trial_events", [], |row| row.get(0))
            .unwrap()
    }

    fn benchmark_event(request: &str, index: u64, id: &str) -> TrialEventInput {
        TrialEventInput {
            kind: TrialKind::Benchmark,
            hypothesis_hash: None,
            strategy_hash: None,
            split_hash: None,
            seeds_hash: None,
            engine_fingerprint_hash: None,
            benchmark_id: Some(id.into()),
            benchmark_params_hash: benchmark_params_hash(id),
            ..event(TrialKind::Variant, request, index)
        }
    }

    // ---------------------------------------------------------- opening

    #[test]
    fn open_creates_one_registry_with_a_stable_id() {
        let dirs = scratch();
        let first = open(&dirs);
        let id = first.registry_id().to_string();
        assert_eq!(id.len(), 32);
        assert_eq!(first.integrity(), &LedgerIntegrity::Intact { head_seq: 0 });
        assert!(first.path().ends_with(REGISTRY_FILE_NAME));
        drop(first);
        let second = open(&dirs);
        assert_eq!(second.registry_id(), id, "registry_id never changes");
        let other = scratch();
        assert_ne!(
            open(&other).registry_id(),
            id,
            "each registry mints its own id"
        );
    }

    #[test]
    fn a19_registry_and_workspace_must_not_contain_one_another() {
        let dirs = scratch();
        let inside = dirs.workspace.join("evidence");
        let error = TrialLedger::open_with_sync_roots(&inside, &dirs.workspace, &[])
            .err()
            .unwrap();
        assert_eq!(error.code(), "registry_inside_workspace");

        let error = TrialLedger::open_with_sync_roots(&dirs.workspace, &dirs.workspace, &[])
            .err()
            .unwrap();
        assert_eq!(
            error.code(),
            "registry_inside_workspace",
            "the same directory counts as inside"
        );

        std::fs::create_dir_all(&dirs.registry).unwrap();
        let nested_workspace = dirs.registry.join("ws");
        std::fs::create_dir_all(&nested_workspace).unwrap();
        let error = TrialLedger::open_with_sync_roots(&dirs.registry, &nested_workspace, &[])
            .err()
            .unwrap();
        assert_eq!(error.code(), "workspace_inside_registry");

        assert!(TrialLedger::open_with_sync_roots(&dirs.registry, &dirs.workspace, &[]).is_ok());
    }

    #[test]
    fn synced_and_network_volumes_are_refused() {
        let dirs = scratch();
        std::fs::create_dir_all(&dirs.registry).unwrap();
        let fake_sync_root = std::fs::canonicalize(&dirs.root).unwrap();
        let error =
            TrialLedger::open_with_sync_roots(&dirs.registry, &dirs.workspace, &[fake_sync_root])
                .err()
                .unwrap();
        assert_eq!(error.code(), "registry_on_unsupported_volume");

        assert!(is_unsupported_volume(
            Path::new(r"\\?\UNC\server\share\evidence"),
            &[]
        ));
        assert!(is_unsupported_volume(
            Path::new(r"\\server\share\evidence"),
            &[]
        ));
        assert!(!is_unsupported_volume(
            Path::new(r"\\?\C:\Users\me\AppData\Local\evidence"),
            &[]
        ));
        assert!(!is_unsupported_volume(
            Path::new("/home/me/.local/share/evidence"),
            &[]
        ));
    }

    #[test]
    fn a_registry_written_by_a_newer_build_is_refused() {
        let dirs = scratch();
        let ledger = open(&dirs);
        ledger
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO registry_migrations (version) VALUES ('0099_future')",
                [],
            )
            .unwrap();
        drop(ledger);
        let error = TrialLedger::open_with_sync_roots(&dirs.registry, &dirs.workspace, &[])
            .err()
            .unwrap();
        assert_eq!(error.code(), "registry_schema_newer");
    }

    #[test]
    fn every_registry_table_is_append_only() {
        let dirs = scratch();
        let ledger = open(&dirs);
        ledger.register_batch(&batch("r1", 1)).unwrap();
        let conn = ledger.lock().unwrap();
        for sql in [
            "UPDATE trial_events SET kind = 'benchmark'",
            "DELETE FROM trial_events",
            "UPDATE trial_batches SET origin_registry_id = 'x'",
            "DELETE FROM trial_batches",
            "UPDATE batch_receipts SET family_effective_before = 0",
            "DELETE FROM batch_receipts",
            "UPDATE registry_meta SET value = 'x'",
            "DELETE FROM registry_meta",
            "DELETE FROM trial_families",
            "UPDATE registry_migrations SET version = 'x'",
            "DELETE FROM registry_migrations",
        ] {
            let error = conn
                .execute(sql, [])
                .err()
                .unwrap_or_else(|| panic!("{sql} was allowed"));
            assert!(
                error.to_string().contains("append-only")
                    || error.to_string().contains("protocol pin"),
                "{sql}: {error}"
            );
        }
        // The protocol pin already happened; a second change is refused.
        let error = conn
            .execute("UPDATE trial_families SET protocol_json = '{}'", [])
            .err()
            .unwrap();
        assert!(error.to_string().contains("one-time protocol pin"));
    }

    // ---------------------------------------------------- registration

    #[test]
    fn a_first_registration_returns_matching_receipt_and_count() {
        let dirs = scratch();
        let ledger = open(&dirs);
        let registered = ledger.register_batch(&batch("r1", 3)).unwrap();
        assert!(!registered.replayed);
        assert_eq!(registered.event_ids.len(), 3);
        assert_eq!(
            registered.registration_receipt,
            RegistrationReceipt {
                receipt_registry_id: ledger.registry_id().into(),
                family_effective_before: 0,
                batch_effective_trials: 3,
            }
        );
        let count = count(&registered.admission);
        assert_eq!(count.family_effective_trials(), 3);
        assert_eq!(count.batch_effective_trials(), 3);
        assert_eq!(count.tests_per_trial(), 2);
        assert_eq!(count.family_id(), family_id_for(BTC).unwrap());
        assert_eq!(count.seq(), 3);
        assert_eq!(
            ledger.chain_at(3).unwrap().as_deref(),
            Some(count.chain_head())
        );
        // Chain from genesis, in idempotency-key order.
        let mut chain = genesis(ledger.registry_id());
        let mut keyed: Vec<(String, String)> = (0..3)
            .map(|index| {
                (
                    format!("ws1:request:r1:candidate:{index}"),
                    registered.event_ids[index].clone(),
                )
            })
            .collect();
        keyed.sort();
        for (_, event_id) in &keyed {
            chain = next_chain(&chain, event_id);
        }
        assert_eq!(count.chain_head(), chain);
    }

    #[test]
    fn a3_a_retry_replays_the_batch_and_reads_the_current_count() {
        let dirs = scratch();
        let ledger = open(&dirs);
        let first = ledger.register_batch(&batch("r1", 2)).unwrap();
        let retry = ledger.register_batch(&batch("r1", 2)).unwrap();
        assert!(retry.replayed);
        assert_eq!(retry.batch_id, first.batch_id);
        assert_eq!(retry.event_ids, first.event_ids);
        assert_eq!(retry.registration_receipt, first.registration_receipt);
        assert_eq!(
            retry.admission, first.admission,
            "nothing else registered in between"
        );
        assert_eq!(event_rows(&ledger), 2, "a retry adds nothing");
    }

    #[test]
    fn a4_relabeling_an_existing_key_is_an_idempotency_conflict() {
        let dirs = scratch();
        let ledger = open(&dirs);
        ledger.register_batch(&batch("r1", 1)).unwrap();
        let mut relabeled = batch("r1", 1);
        relabeled.events[0].kind = TrialKind::Diagnostic;
        let error = ledger.register_batch(&relabeled).err().unwrap();
        assert_eq!(error.code(), "idempotency_conflict");

        let mut as_benchmark = batch("r1", 1);
        as_benchmark.events[0] = benchmark_event("r1", 0, "buyHold");
        let error = ledger.register_batch(&as_benchmark).err().unwrap();
        assert_eq!(error.code(), "idempotency_conflict");
        assert_eq!(event_rows(&ledger), 1, "no write");
    }

    #[test]
    fn a27_a_whole_batch_resent_with_changed_content_never_gets_the_old_receipt() {
        let dirs = scratch();
        let ledger = open(&dirs);
        ledger.register_batch(&batch("r1", 3)).unwrap();
        let mut changed = batch("r1", 3);
        changed.events[2].strategy_hash = Some("a different strategy".into());
        let error = ledger.register_batch(&changed).err().unwrap();
        assert_eq!(error.code(), "idempotency_conflict");
        assert_eq!(event_rows(&ledger), 3);
    }

    #[test]
    fn a28_a_batch_overlapping_an_existing_one_is_refused() {
        let dirs = scratch();
        let ledger = open(&dirs);
        ledger.register_batch(&batch("r1", 2)).unwrap();
        let error = ledger.register_batch(&batch("r1", 3)).err().unwrap();
        assert_eq!(error.code(), "batch_conflict");
        assert_eq!(event_rows(&ledger), 2);
    }

    #[test]
    fn a5_only_whitelisted_frozen_benchmarks_are_not_effective() {
        let dirs = scratch();
        let ledger = open(&dirs);
        let mut unknown = batch("r1", 1);
        unknown.events[0] = benchmark_event("r1", 0, "momentum");
        unknown.events[0].benchmark_params_hash = Some("x".into());
        assert_eq!(
            ledger.register_batch(&unknown).err().unwrap().code(),
            "invalid_trial_batch"
        );

        let mut tuned = batch("r1", 1);
        tuned.events[0] = benchmark_event("r1", 0, "smaCross");
        tuned.events[0].benchmark_params_hash = benchmark_params_hash("rsiReversion");
        assert_eq!(
            ledger.register_batch(&tuned).err().unwrap().code(),
            "invalid_trial_batch"
        );

        let mut valid = batch("r2", 1);
        valid.events[0] = benchmark_event("r2", 0, "smaCross");
        valid
            .events
            .push(benchmark_event("r2", 1, RANDOM_ENTRY_CONTRACT_VERSION));
        let registered = ledger.register_batch(&valid).unwrap();
        assert_eq!(registered.registration_receipt.batch_effective_trials, 0);
        assert_eq!(
            registered.admission,
            Admission::Blocked(AdmissionBlocked::NoEffectiveTrials)
        );
        assert_eq!(
            event_rows(&ledger),
            2,
            "benchmark events are still recorded"
        );
    }

    #[test]
    fn a6_reproductions_must_match_all_five_identity_fields() {
        let dirs = scratch();
        let ledger = open(&dirs);
        let original = ledger.register_batch(&batch("r1", 1)).unwrap();
        let reproduce = |request: &str, edit: &dyn Fn(&mut TrialEventInput)| {
            let mut input = batch(request, 1);
            let mut event = event(TrialKind::Reproduction, "r1", 0);
            event.origin = TrialOrigin::Request {
                request_id: request.into(),
                candidate_index: 0,
            };
            event.reproduction_of = Some(original.event_ids[0].clone());
            edit(&mut event);
            input.events[0] = event;
            ledger.register_batch(&input)
        };

        let exact = reproduce("r2", &|_| {}).unwrap();
        assert_eq!(exact.registration_receipt.batch_effective_trials, 0);
        assert_eq!(exact.registration_receipt.family_effective_before, 1);

        for (request, field) in [
            ("r3", "seeds"),
            ("r4", "split"),
            ("r5", "strategy"),
            ("r6", "engine"),
            ("r7", "dataset"),
        ] {
            let error = reproduce(request, &|event| match field {
                "seeds" => event.seeds_hash = Some("other".into()),
                "split" => event.split_hash = Some("other".into()),
                "strategy" => event.strategy_hash = Some("other".into()),
                "engine" => event.engine_fingerprint_hash = Some("other".into()),
                _ => event.dataset_hash = Some("other".into()),
            })
            .err()
            .unwrap();
            assert_eq!(error.code(), "reproduction_mismatch", "{field}");
        }
        let error = reproduce("r8", &|event| event.seeds_hash = None)
            .err()
            .unwrap();
        assert_eq!(
            error.code(),
            "reproduction_mismatch",
            "a null identity field never matches"
        );
        let error = reproduce("r9", &|event| {
            event.reproduction_of = Some("trial-event-v1:missing".into())
        })
        .err()
        .unwrap();
        assert_eq!(error.code(), "reproduction_reference_missing");
    }

    #[test]
    fn a_legacy_event_without_split_or_seeds_cannot_be_reproduced() {
        let dirs = scratch();
        let ledger = open(&dirs);
        let mut legacy = event(TrialKind::Legacy, "unused", 0);
        legacy.origin = TrialOrigin::Legacy {
            attempt_key: "run:4:candidate:0".into(),
        };
        legacy.split_hash = None;
        legacy.seeds_hash = None;
        let registered = ledger
            .register_batch(&TrialBatchInput {
                events: vec![legacy],
                ..batch("unused", 1)
            })
            .unwrap();
        assert_eq!(
            registered.registration_receipt.batch_effective_trials, 1,
            "legacy counts"
        );

        let mut input = batch("r2", 1);
        input.events[0].kind = TrialKind::Reproduction;
        input.events[0].strategy_hash = Some("strategy-unused-0".into());
        input.events[0].seeds_hash = Some("seeds-unused-0".into());
        input.events[0].reproduction_of = Some(registered.event_ids[0].clone());
        assert_eq!(
            ledger.register_batch(&input).err().unwrap().code(),
            "reproduction_mismatch"
        );
    }

    #[test]
    fn a17_a_family_keeps_the_protocol_of_its_first_effective_trial() {
        let dirs = scratch();
        let ledger = open(&dirs);
        ledger.register_batch(&batch("r1", 1)).unwrap();
        let mut other = batch("r2", 1);
        other.tests_per_trial = 1;
        assert_eq!(
            ledger.register_batch(&other).err().unwrap().code(),
            "family_protocol_mismatch"
        );
        let mut replay = batch("r1", 1);
        replay.tests_per_trial = 3;
        assert_eq!(
            ledger.register_batch(&replay).err().unwrap().code(),
            "family_protocol_mismatch",
            "a replay cannot change the protocol either"
        );
    }

    #[test]
    fn an_unknown_family_is_recorded_but_never_admitted() {
        let dirs = scratch();
        let ledger = open(&dirs);
        let mut input = batch("r1", 2);
        input.instrument_id = None;
        for event in &mut input.events {
            event.snapshot_id = None;
        }
        let registered = ledger.register_batch(&input).unwrap();
        assert_eq!(
            registered.admission,
            Admission::Blocked(AdmissionBlocked::FamilyUnknown)
        );
        assert_eq!(event_rows(&ledger), 2);
        assert_eq!(
            ledger.read_admission_count(&registered.batch_id).unwrap(),
            Admission::Blocked(AdmissionBlocked::FamilyUnknown)
        );
    }

    #[test]
    fn a_quarantined_family_refuses_registration_and_admission() {
        let dirs = scratch();
        let ledger = open(&dirs);
        let registered = ledger.register_batch(&batch("r1", 1)).unwrap();
        ledger
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO family_conflicts (family_id, kind, detail_json, recorded_at)
                 VALUES (?1, 'idempotency_conflict', '{}', 'now')",
                [family_id_for(BTC).unwrap()],
            )
            .unwrap();
        assert_eq!(
            ledger.register_batch(&batch("r2", 1)).err().unwrap().code(),
            "family_quarantined"
        );
        assert_eq!(
            ledger.read_admission_count(&registered.batch_id).unwrap(),
            Admission::Blocked(AdmissionBlocked::FamilyQuarantined)
        );
    }

    #[test]
    fn read_admission_count_sees_later_batches_and_rejects_unknown_ids() {
        let dirs = scratch();
        let ledger = open(&dirs);
        let first = ledger.register_batch(&batch("r1", 2)).unwrap();
        ledger.register_batch(&batch("r2", 5)).unwrap();
        let admission = ledger.read_admission_count(&first.batch_id).unwrap();
        let now = count(&admission);
        assert_eq!(now.family_effective_trials(), 7);
        assert_eq!(now.batch_effective_trials(), 2);
        assert_eq!(now.seq(), 7);
        assert_eq!(
            ledger
                .read_admission_count("trial-batch-v1:nope")
                .err()
                .unwrap()
                .code(),
            "batch_not_found"
        );
    }

    #[test]
    fn a8_concurrent_registrations_serialize_without_losing_trials() {
        let dirs = scratch();
        let ledgers = [Arc::new(open(&dirs)), Arc::new(open(&dirs))];
        let handles: Vec<_> = ledgers
            .iter()
            .enumerate()
            .map(|(worker, ledger)| {
                let ledger = Arc::clone(ledger);
                std::thread::spawn(move || {
                    (0..10)
                        .map(|round| {
                            ledger
                                .register_batch(&batch(
                                    &format!("w{worker}r{round}"),
                                    round % 3 + 1,
                                ))
                                .unwrap()
                                .registration_receipt
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut receipts: Vec<RegistrationReceipt> = handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect();
        let expected_total: u64 = 2 * (0..10).map(|round: u64| round % 3 + 1).sum::<u64>();
        receipts.sort_by_key(|receipt| receipt.family_effective_before);
        let mut next = 0;
        for receipt in &receipts {
            assert_eq!(
                receipt.family_effective_before, next,
                "receipts tile the count without overlap"
            );
            next += receipt.batch_effective_trials;
        }
        assert_eq!(next, expected_total);
        let reopened = open(&dirs);
        assert_eq!(
            reopened.integrity(),
            &LedgerIntegrity::Intact {
                head_seq: expected_total
            }
        );
    }

    #[test]
    fn a30_an_edited_chain_or_payload_is_detected_at_open() {
        for target in ["chain", "payload"] {
            let dirs = scratch();
            let ledger = open(&dirs);
            let registered = ledger.register_batch(&batch("r1", 2)).unwrap();
            {
                let conn = ledger.lock().unwrap();
                conn.execute_batch("DROP TRIGGER trial_events_no_update")
                    .unwrap();
                let sql = if target == "chain" {
                    "UPDATE trial_events SET chain = 'forged' WHERE seq = 1"
                } else {
                    "UPDATE trial_events SET payload_json = replace(payload_json, 'dataset-1', 'dataset-9') WHERE seq = 2"
                };
                conn.execute(sql, []).unwrap();
            }
            drop(ledger);
            let reopened = open(&dirs);
            assert!(
                matches!(reopened.integrity(), LedgerIntegrity::ChainBroken { .. }),
                "{target}: {:?}",
                reopened.integrity()
            );
            assert_eq!(
                reopened.read_admission_count(&registered.batch_id).unwrap(),
                Admission::Blocked(AdmissionBlocked::ChainBroken)
            );
        }
    }

    #[test]
    fn a31_a_retry_after_an_intervening_batch_uses_the_current_count() {
        let dirs = scratch();
        let ledger = open(&dirs);
        // A registers, then "crashes" before its workspace write and admission.
        let a = ledger.register_batch(&batch("reqA", 1)).unwrap();
        ledger.register_batch(&batch("reqB", 10)).unwrap();
        let retry = ledger.register_batch(&batch("reqA", 1)).unwrap();
        assert!(retry.replayed);
        assert_eq!(retry.event_ids, a.event_ids);
        assert_eq!(retry.registration_receipt.family_effective_before, 0);
        assert_eq!(retry.registration_receipt.batch_effective_trials, 1);
        let count = count(&retry.admission);
        assert_eq!(count.family_effective_trials(), 11);
        assert_eq!(count.batch_effective_trials(), 1);

        let sampling = SamplingPlan {
            alpha_ppm: 50_000,
            max_relative_standard_error_ppm: 200_000,
            bootstrap_samples: 975,
            max_bootstrap_samples: 100_000,
        };
        let plan = precision_plan_from_count(count, &sampling);
        assert_eq!(
            (plan.prior_trials, plan.planned_trials, plan.tests_per_trial),
            (10, 1, 2)
        );
        let report = evaluate_precision_plan(&plan).unwrap();
        assert_eq!(report.family_tests, 22);
        assert_eq!(report.status, PrecisionStatus::NotEligible);
        assert_eq!(
            report.reasons,
            vec![PrecisionReason::MonteCarloPrecisionInsufficient]
        );
        assert_eq!(report.required_samples_for_precision, Some(10_975));

        // The hazard R5 names: the stale receipt would have said ELIGIBLE.
        let stale = PrecisionPlan {
            prior_trials: retry.registration_receipt.family_effective_before,
            planned_trials: retry.registration_receipt.batch_effective_trials,
            ..plan
        };
        assert_eq!(
            evaluate_precision_plan(&stale).unwrap().status,
            PrecisionStatus::Eligible
        );
    }

    #[test]
    fn the_alphabtc_counts_map_to_146_over_1001() {
        let dirs = scratch();
        let ledger = open(&dirs);
        ledger.register_batch(&batch("earlier", 49)).unwrap();
        let next = ledger.register_batch(&batch("next", 24)).unwrap();
        let plan = precision_plan_from_count(
            count(&next.admission),
            &SamplingPlan {
                alpha_ppm: 50_000,
                max_relative_standard_error_ppm: 200_000,
                bootstrap_samples: 1000,
                max_bootstrap_samples: 100_000,
            },
        );
        assert_eq!((plan.prior_trials, plan.planned_trials), (49, 24));
        let report = evaluate_precision_plan(&plan).unwrap();
        assert_eq!(
            (
                report.best_adjusted_p.numerator,
                report.best_adjusted_p.denominator
            ),
            (146, 1001)
        );
        assert_eq!(report.status, PrecisionStatus::NotEligible);
    }

    // ------------------------------------------------ pure preparation

    #[test]
    fn identities_are_content_hashes_independent_of_the_registry() {
        let one = scratch();
        let two = scratch();
        let a = open(&one).register_batch(&batch("r1", 3)).unwrap();
        let b = open(&two).register_batch(&batch("r1", 3)).unwrap();
        assert_eq!(
            a.event_ids, b.event_ids,
            "originRegistryId is not part of identity"
        );
        assert_eq!(a.batch_id, b.batch_id);

        let prepared = prepare_batch(&batch("r1", 1)).unwrap();
        let event = &prepared.events[0];
        assert_eq!(event.idempotency_key, "ws1:request:r1:candidate:0");
        assert_eq!(event.event_id, event_id_for(&event.payload_json));
        let payload: Value = serde_json::from_str(&event.payload_json).unwrap();
        let keys: Vec<&str> = payload
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys.len(),
            19,
            "every payload field is always present: {keys:?}"
        );
        assert!(payload["benchmarkId"].is_null() && payload["reproductionOf"].is_null());
        assert!(!payload
            .as_object()
            .unwrap()
            .contains_key("originRegistryId"));
    }

    #[test]
    fn malformed_batches_are_rejected_before_any_write() {
        let cases: Vec<(&str, TrialBatchInput)> = vec![
            (
                "empty",
                TrialBatchInput {
                    events: vec![],
                    ..batch("r1", 1)
                },
            ),
            (
                "workspace colon",
                TrialBatchInput {
                    workspace_id: "a:b".into(),
                    ..batch("r1", 1)
                },
            ),
            (
                "zero tests",
                TrialBatchInput {
                    tests_per_trial: 0,
                    ..batch("r1", 1)
                },
            ),
            (
                "non-canonical instrument",
                TrialBatchInput {
                    instrument_id: Some("crypto:Binance:BTCUSDT".into()),
                    ..batch("r1", 1)
                },
            ),
            (
                "bad instrument",
                TrialBatchInput {
                    instrument_id: Some("BTCUSDT".into()),
                    ..batch("r1", 1)
                },
            ),
            ("duplicate index", {
                let mut input = batch("r1", 2);
                input.events[1] = event(TrialKind::Variant, "r1", 0);
                input
            }),
            ("mixed requests", {
                let mut input = batch("r1", 2);
                input.events[1] = event(TrialKind::Variant, "r2", 1);
                input
            }),
            ("legacy kind with request origin", {
                let mut input = batch("r1", 1);
                input.events[0].kind = TrialKind::Legacy;
                input
            }),
            ("missing strategy", {
                let mut input = batch("r1", 1);
                input.events[0].strategy_hash = None;
                input
            }),
            ("snapshot without instrument", {
                let mut input = batch("r1", 1);
                input.instrument_id = None;
                input
            }),
            ("empty text", {
                let mut input = batch("r1", 1);
                input.events[0].hypothesis_hash = Some(" ".into());
                input
            }),
            ("benchmark field on a variant", {
                let mut input = batch("r1", 1);
                input.events[0].benchmark_id = Some("buyHold".into());
                input
            }),
        ];
        for (name, input) in cases {
            let error = prepare_batch(&input)
                .err()
                .unwrap_or_else(|| panic!("{name} was accepted"));
            assert_eq!(error.code(), "invalid_trial_batch", "{name}: {error}");
        }
    }

    #[test]
    fn the_benchmark_whitelist_is_tied_to_its_contract_versions() {
        // A contract bump must revisit `frozen_benchmark_params`.
        assert_eq!(BENCHMARK_CONTRACT_VERSION, "benchmark-suite-v1");
        assert_eq!(RANDOM_ENTRY_CONTRACT_VERSION, "random-entry-v1");
        let mut hashes = BTreeSet::new();
        for id in DETERMINISTIC_BENCHMARK_IDS
            .iter()
            .copied()
            .chain([RANDOM_ENTRY_CONTRACT_VERSION])
        {
            let hash = benchmark_params_hash(id).unwrap_or_else(|| panic!("{id} missing"));
            assert!(hashes.insert(hash), "{id} hash is unique");
        }
        assert_eq!(benchmark_params_hash("momentum"), None);
    }
}
