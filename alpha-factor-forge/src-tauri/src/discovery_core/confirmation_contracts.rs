//! P13c-1: pure frozen documents and instrument/time overlap.
//! Consistency with supplied context is not proof of registry spend, complete
//! lineage, authentic artifacts, unseen data, calibration or execution authority.

use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    confirmation_v2::{r3_block_length, CONFIRMATION_STATISTICS_V2_VERSION},
    identity::canonical_bytes,
    market_data::{MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE, MIN_MARKET_TIMESTAMP_MS},
    market_foundation::{format_instrument_id, interval_ms, parse_instrument_id, Market},
    precision::PRECISION_MAX_COUNT,
    split::{plan_validation_split, SPLIT_CONTRACT_VERSION},
};

mod selection;
pub(crate) use selection::verify_report_shape;
pub use selection::{
    freeze_selection, parse_selection, verify_confirmation_report, SealedValidationInput,
};

pub const BATCH_VERSION: &str = "research-confirmation-batch-v1";
pub const SELECTION_VERSION: &str = "research-confirmation-selection-v1";
pub const OPERATION_VERSION: &str = "research-confirmation-operation-v1";
pub const SELECTION_RULE: &str = "both-holm-reject-index-v1";
pub const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CANDIDATES: usize = 4096;
pub const MAX_INTERVALS: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("confirmation document: {0}")]
pub struct ContractError(pub String);

fn require(value: bool, message: impl Into<String>) -> Result<(), ContractError> {
    if value {
        Ok(())
    } else {
        Err(ContractError(message.into()))
    }
}

fn read<T: DeserializeOwned>(raw: &str) -> Result<T, ContractError> {
    require(raw.len() <= MAX_DOCUMENT_BYTES, "document exceeds 4 MiB")?;
    serde_json::from_str(raw).map_err(|error| ContractError(error.to_string()))
}

fn hex_id(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn hash_field(value: &str, name: &str) -> Result<(), ContractError> {
    require(
        hex_id(value, 64),
        format!("{name} must be 64 lowercase hex digits"),
    )
}

fn short_id(value: &str, name: &str) -> Result<(), ContractError> {
    require(
        hex_id(value, 32),
        format!("{name} must be 32 lowercase hex digits"),
    )
}

fn count(value: u64, minimum: u64, name: &str) -> Result<(), ContractError> {
    require(
        (minimum..=PRECISION_MAX_COUNT).contains(&value),
        format!("{name} outside safe integer domain"),
    )
}

fn prefixed_hash(value: &str, prefix: &str, name: &str) -> Result<(), ContractError> {
    require(
        value.strip_prefix(prefix).is_some_and(|v| hex_id(v, 64)),
        format!("{name} has invalid version/hash"),
    )
}

fn named_id(value: &str, name: &str) -> Result<(), ContractError> {
    require(
        (1..=128).contains(&value.len())
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b)),
        format!("{name} must be a bounded ASCII identifier"),
    )
}

fn instrument(value: &str) -> Result<(), ContractError> {
    require(value.len() <= 256, "instrumentId exceeds 256 bytes")?;
    let parsed =
        parse_instrument_id(value).map_err(|_| ContractError("invalid instrumentId".into()))?;
    require(
        format_instrument_id(&parsed) == value,
        "instrumentId is not canonical",
    )
}

/// Same ASCII-only canonical JSON preimage as the existing trial registry.
/// This creates no family row and proves no registry membership.
pub fn confirmation_family_id(value: &str) -> Result<String, ContractError> {
    instrument(value)?;
    let key = serde_json::json!({"instrumentId":value,"version":"trial-family-v1"});
    let bytes = serde_json::to_vec(&key).map_err(|e| ContractError(e.to_string()))?;
    Ok(format!(
        "trial-family-v1:{}",
        hex::encode(Sha256::digest(bytes))
    ))
}

fn identity<T: Serialize>(version: &str, body: &T) -> Result<String, ContractError> {
    let value = serde_json::to_value(body).map_err(|e| ContractError(e.to_string()))?;
    let bytes = canonical_bytes(&value).map_err(|e| ContractError(e.to_string()))?;
    let mut hash = Sha256::new();
    hash.update(version.as_bytes());
    hash.update([0]);
    hash.update(bytes);
    let id = hex::encode(hash.finalize());
    let stored = serde_json::json!({"id":id,"body":value});
    require(
        serde_json::to_vec(&stored)
            .map_err(|e| ContractError(e.to_string()))?
            .len()
            <= MAX_DOCUMENT_BYTES,
        "stored record exceeds 4 MiB",
    )?;
    Ok(id)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored<T> {
    id: String,
    body: T,
}

fn required_nullable<'de, D: Deserializer<'de>>(de: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(de)
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceBinding {
    pub campaign_id: String,
    pub campaign_document_hash: String,
    pub workspace_id: String,
    pub run_id: u64,
    pub trial_batch_id: String,
    pub instrument_id: String,
    pub family_id: String,
    pub engine_manifest_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateBinding {
    pub candidate_index: u64,
    pub strategy_hash: String,
    pub definition_hash: String,
    pub execution_hash: String,
    pub result_artifact_hash: String,
    pub lineage_hash: String,
    pub minimum_warmup_bars: u64,
    pub minimum_embargo_bars: u64,
}

/// Typed pure context; the future host must prove it is the entire actual
/// registered source. Its domains are checked again, not trusted by type alone.
#[derive(Clone, Debug)]
pub struct RegisteredSource {
    pub source: SourceBinding,
    pub candidates: Vec<CandidateBinding>,
}

pub(crate) fn validate_registered_source(source: &RegisteredSource) -> Result<(), ContractError> {
    validate_source(&source.source)?;
    validate_candidates(&source.candidates)
}

fn validate_source(source: &SourceBinding) -> Result<(), ContractError> {
    for (name, value) in [
        ("campaignId", &source.campaign_id),
        ("campaignDocumentHash", &source.campaign_document_hash),
        ("trialBatchId", &source.trial_batch_id),
        ("engineManifestHash", &source.engine_manifest_hash),
    ] {
        hash_field(value, name)?;
    }
    short_id(&source.workspace_id, "workspaceId")?;
    count(source.run_id, 1, "runId")?;
    require(
        source.family_id == confirmation_family_id(&source.instrument_id)?,
        "familyId differs from canonical instrument",
    )
}

fn validate_candidates(candidates: &[CandidateBinding]) -> Result<(), ContractError> {
    require(
        (1..=MAX_CANDIDATES).contains(&candidates.len()),
        "candidate count must be 1..4096",
    )?;
    require(
        candidates
            .windows(2)
            .all(|p| p[0].candidate_index < p[1].candidate_index),
        "candidate indexes must be sorted and unique",
    )?;
    for candidate in candidates {
        count(candidate.candidate_index, 0, "candidateIndex")?;
        count(candidate.minimum_warmup_bars, 0, "minimumWarmupBars")?;
        count(candidate.minimum_embargo_bars, 0, "minimumEmbargoBars")?;
        prefixed_hash(&candidate.strategy_hash, "strategy-v2:", "strategyHash")?;
        for (name, value) in [
            ("definitionHash", &candidate.definition_hash),
            ("executionHash", &candidate.execution_hash),
            ("resultArtifactHash", &candidate.result_artifact_hash),
            ("lineageHash", &candidate.lineage_hash),
        ] {
            hash_field(value, name)?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeInterval {
    from_ms: u64,
    to_exclusive_ms: u64,
}

impl TimeInterval {
    pub fn new(from_ms: u64, to_exclusive_ms: u64) -> Result<Self, ContractError> {
        let value = Self {
            from_ms,
            to_exclusive_ms,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn from_ms(&self) -> u64 {
        self.from_ms
    }
    pub fn to_exclusive_ms(&self) -> u64 {
        self.to_exclusive_ms
    }
    fn validate(&self) -> Result<(), ContractError> {
        require(
            self.from_ms >= MIN_MARKET_TIMESTAMP_MS as u64
                && self.from_ms < self.to_exclusive_ms
                && self.to_exclusive_ms <= MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE as u64,
            "invalid half-open market-time interval",
        )
    }
}

fn validate_intervals(values: &[TimeInterval]) -> Result<(), ContractError> {
    require(
        (1..=MAX_INTERVALS).contains(&values.len()),
        "interval count must be 1..128",
    )?;
    for value in values {
        value.validate()?;
    }
    require(
        values
            .windows(2)
            .all(|p| p[0].to_exclusive_ms < p[1].from_ms),
        "intervals must be sorted, disjoint and touching intervals merged",
    )
}

/// Pure occupancy geometry, not a persisted claim or proof of unseen history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExposureFootprint {
    instrument_id: String,
    intervals: Vec<TimeInterval>,
}

impl ExposureFootprint {
    pub fn new(instrument_id: &str, intervals: Vec<TimeInterval>) -> Result<Self, ContractError> {
        instrument(instrument_id)?;
        validate_intervals(&intervals)?;
        Ok(Self {
            instrument_id: instrument_id.into(),
            intervals,
        })
    }
    pub fn instrument_id(&self) -> &str {
        &self.instrument_id
    }
    pub fn intervals(&self) -> &[TimeInterval] {
        &self.intervals
    }
}

pub fn footprints_overlap(left: &ExposureFootprint, right: &ExposureFootprint) -> bool {
    if left.instrument_id != right.instrument_id {
        return false;
    }
    let (mut i, mut j) = (0, 0);
    while i < left.intervals.len() && j < right.intervals.len() {
        let (a, b) = (left.intervals[i], right.intervals[j]);
        if a.from_ms < b.to_exclusive_ms && b.from_ms < a.to_exclusive_ms {
            return true;
        }
        if a.to_exclusive_ms <= b.from_ms {
            i += 1;
        } else {
            j += 1;
        }
    }
    false
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Stage {
    Validation {},
    Test {
        validation_batch_id: String,
        selection_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DataBinding {
    snapshot_id: String,
    instrument_revision_hash: String,
    dataset_hash: String,
    interval: String,
    calendar_id: String,
    calendar_hash: String,
    #[serde(deserialize_with = "required_nullable")]
    corporate_actions_hash: Option<String>,
    cost_profile_id: String,
    cost_profile_hash: String,
    numeric_costs_hash: String,
    source_evidence_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrozenSplit {
    contract_version: String,
    total_bars: u64,
    embargo_bars: u64,
    train_window: TimeInterval,
    validation_window: TimeInterval,
    test_window: TimeInterval,
    validation_support: Vec<TimeInterval>,
    test_support: Vec<TimeInterval>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StageStatistics {
    block_length: u64,
    seed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StatisticsTemplate {
    contract_version: String,
    allocation_id: String,
    bootstrap_samples: u64,
    max_bootstrap_samples: u64,
    max_relative_standard_error_ppm: u64,
    block_length_rationale: String,
    validation: StageStatistics,
    test: StageStatistics,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionRule {
    version: String,
    max_candidates: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchBody {
    contract_version: String,
    stage: Stage,
    source: SourceBinding,
    data: DataBinding,
    split: FrozenSplit,
    statistics: StatisticsTemplate,
    selection_rule: SelectionRule,
    candidates: Vec<CandidateBinding>,
}

fn checked_support(
    values: &[TimeInterval],
    window: TimeInterval,
    preceding_end: u64,
) -> Result<(), ContractError> {
    validate_intervals(values)?;
    require(
        values[0].from_ms >= preceding_end
            && values.last().unwrap().to_exclusive_ms <= window.to_exclusive_ms,
        "protected support enters another role",
    )?;
    require(
        values
            .iter()
            .any(|v| v.from_ms <= window.from_ms && window.to_exclusive_ms <= v.to_exclusive_ms),
        "support does not cover the evaluation window",
    )
}

fn stage_bars(body: &BatchBody) -> Result<(u64, u64), ContractError> {
    let s = &body.split;
    count(s.total_bars, 1, "totalBars")?;
    count(s.embargo_bars, 0, "embargoBars")?;
    let plan = plan_validation_split(s.total_bars as i64, s.embargo_bars as i64)
        .map_err(|e| ContractError(e.to_string()))?;
    for (window, bars) in [
        (s.train_window, plan.train.count),
        (s.validation_window, plan.validation.count),
        (s.test_window, plan.test.count),
    ] {
        window.validate()?;
        require(
            window.to_exclusive_ms - window.from_ms >= bars as u64,
            "window too short for declared bar count",
        )?;
    }
    require(
        s.train_window.to_exclusive_ms <= s.validation_window.from_ms
            && s.validation_window.to_exclusive_ms <= s.test_window.from_ms,
        "split windows overlap or are out of order",
    )?;
    checked_support(
        &s.validation_support,
        s.validation_window,
        s.train_window.to_exclusive_ms,
    )?;
    checked_support(
        &s.test_support,
        s.test_window,
        s.validation_window.to_exclusive_ms,
    )?;
    Ok((plan.validation.count as u64, plan.test.count as u64))
}

fn validate_batch(body: &BatchBody, complete_count: usize) -> Result<(), ContractError> {
    require(
        body.contract_version == BATCH_VERSION,
        "unsupported batch version",
    )?;
    validate_source(&body.source)?;
    validate_candidates(&body.candidates)?;
    let data = &body.data;
    for (name, value) in [
        ("snapshotId", &data.snapshot_id),
        ("instrumentRevisionHash", &data.instrument_revision_hash),
        ("calendarHash", &data.calendar_hash),
        ("costProfileHash", &data.cost_profile_hash),
        ("numericCostsHash", &data.numeric_costs_hash),
        ("sourceEvidenceHash", &data.source_evidence_hash),
    ] {
        hash_field(value, name)?;
    }
    prefixed_hash(&data.dataset_hash, "dataset-content-v2:", "datasetHash")?;
    require(
        interval_ms(&data.interval).is_some(),
        "unsupported data interval",
    )?;
    named_id(&data.calendar_id, "calendarId")?;
    named_id(&data.cost_profile_id, "costProfileId")?;
    if let Some(hash) = &data.corporate_actions_hash {
        hash_field(hash, "corporateActionsHash")?;
    }
    require(
        parse_instrument_id(&body.source.instrument_id)
            .unwrap()
            .market
            == Market::Crypto
            || data.corporate_actions_hash.is_some(),
        "ETF corporate actions evidence missing",
    )?;
    require(
        body.split.contract_version == SPLIT_CONTRACT_VERSION,
        "unsupported split version",
    )?;
    let (validation_bars, test_bars) = stage_bars(body)?;
    require(
        body.candidates.iter().all(|c| {
            c.minimum_embargo_bars <= body.split.embargo_bars
                && c.minimum_warmup_bars <= body.split.embargo_bars
        }),
        "candidate warm-up/embargo exceeds frozen split",
    )?;
    let s = &body.statistics;
    require(
        s.contract_version == CONFIRMATION_STATISTICS_V2_VERSION,
        "unsupported statistics version",
    )?;
    hash_field(&s.allocation_id, "allocationId")?;
    count(s.bootstrap_samples, 1, "bootstrapSamples")?;
    count(s.max_bootstrap_samples, 1, "maxBootstrapSamples")?;
    require(
        s.bootstrap_samples <= s.max_bootstrap_samples,
        "bootstrap sample count exceeds frozen cap",
    )?;
    require(
        (1..=999_999).contains(&s.max_relative_standard_error_ppm),
        "precision error bound outside ppm domain",
    )?;
    require(
        (1..=1024).contains(&s.block_length_rationale.len())
            && !s.block_length_rationale.trim().is_empty(),
        "block rationale must be nonblank and at most 1024 bytes",
    )?;
    for (stage, n) in [(&s.validation, validation_bars), (&s.test, test_bars)] {
        count(stage.seed, 0, "seed")?;
        require(
            stage.block_length == r3_block_length(n)
                && 4 * u128::from(stage.block_length).pow(2) <= u128::from(n),
            "stage block length contradicts R3/domain",
        )?;
    }
    require(
        body.selection_rule.version == SELECTION_RULE
            && (1..=complete_count as u64).contains(&body.selection_rule.max_candidates),
        "unsupported or invalid selection rule/limit",
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FrozenBatch {
    id: String,
    body: BatchBody,
}

impl FrozenBatch {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn source(&self) -> &SourceBinding {
        &self.body.source
    }
    pub fn candidates(&self) -> &[CandidateBinding] {
        &self.body.candidates
    }
    pub fn allocation_id(&self) -> &str {
        &self.body.statistics.allocation_id
    }
    pub(crate) fn precision_settings(&self) -> (u64, u64, u64) {
        let s = &self.body.statistics;
        (
            s.bootstrap_samples,
            s.max_bootstrap_samples,
            s.max_relative_standard_error_ppm,
        )
    }
    pub fn is_validation(&self) -> bool {
        self.body.stage == Stage::Validation {}
    }
    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("checked integer/string batch")
    }
    pub fn footprint(&self) -> ExposureFootprint {
        let intervals = if self.is_validation() {
            &self.body.split.validation_support
        } else {
            &self.body.split.test_support
        };
        ExposureFootprint {
            instrument_id: self.body.source.instrument_id.clone(),
            intervals: intervals.clone(),
        }
    }
}

fn checked_validation(
    body: BatchBody,
    source: &RegisteredSource,
) -> Result<FrozenBatch, ContractError> {
    validate_source(&source.source)?;
    validate_candidates(&source.candidates)?;
    validate_batch(&body, source.candidates.len())?;
    require(
        body.stage == Stage::Validation {},
        "Validation stage required",
    )?;
    require(
        body.source == source.source && body.candidates == source.candidates,
        "batch differs from complete registered source",
    )?;
    Ok(FrozenBatch {
        id: identity(BATCH_VERSION, &body)?,
        body,
    })
}

pub fn freeze_validation_batch(
    raw: &str,
    source: &RegisteredSource,
) -> Result<FrozenBatch, ContractError> {
    checked_validation(read(raw)?, source)
}

pub fn parse_validation_batch(
    raw: &str,
    source: &RegisteredSource,
) -> Result<FrozenBatch, ContractError> {
    let stored: Stored<BatchBody> = read(raw)?;
    let result = checked_validation(stored.body, source)?;
    require(stored.id == result.id, "batch ID does not reproduce")?;
    Ok(result)
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionBody {
    contract_version: String,
    validation_batch_id: String,
    alpha_reservation_id: String,
    sealed_receipt_id: String,
    report_sha256: String,
    selected_candidates: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FrozenSelection {
    id: String,
    body: SelectionBody,
}

impl FrozenSelection {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn selected_candidates(&self) -> &[u64] {
        &self.body.selected_candidates
    }
    pub fn validation_batch_id(&self) -> &str {
        &self.body.validation_batch_id
    }
    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("checked integer/string selection")
    }
}

fn checked_test(
    body: BatchBody,
    parent: &FrozenBatch,
    selection: &FrozenSelection,
) -> Result<FrozenBatch, ContractError> {
    require(
        parent.is_validation() && selection.body.validation_batch_id == parent.id,
        "selection has another Validation parent",
    )?;
    require(
        !selection.body.selected_candidates.is_empty(),
        "empty selection cannot schedule Test",
    )?;
    validate_batch(&body, parent.body.candidates.len())?;
    let mut expected = parent.body.clone();
    expected.stage = Stage::Test {
        validation_batch_id: parent.id.clone(),
        selection_id: selection.id.clone(),
    };
    expected.candidates = parent
        .body
        .candidates
        .iter()
        .filter(|c| {
            selection
                .body
                .selected_candidates
                .binary_search(&c.candidate_index)
                .is_ok()
        })
        .cloned()
        .collect();
    require(
        body == expected,
        "Test differs from frozen parent/selection",
    )?;
    Ok(FrozenBatch {
        id: identity(BATCH_VERSION, &body)?,
        body,
    })
}

pub fn freeze_test_batch(
    raw: &str,
    parent: &FrozenBatch,
    selection: &FrozenSelection,
) -> Result<FrozenBatch, ContractError> {
    checked_test(read(raw)?, parent, selection)
}

pub fn parse_test_batch(
    raw: &str,
    parent: &FrozenBatch,
    selection: &FrozenSelection,
) -> Result<FrozenBatch, ContractError> {
    let stored: Stored<BatchBody> = read(raw)?;
    let result = checked_test(stored.body, parent, selection)?;
    require(stored.id == result.id, "batch ID does not reproduce")?;
    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum OperationStage {
    Validation,
    Test,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OperationBody {
    contract_version: String,
    origin_registry_id: String,
    reservation_key: String,
    request_id: String,
    workspace_id: String,
    confirmation_batch_id: String,
    family_id: String,
    allocation_id: String,
    stage: OperationStage,
    #[serde(deserialize_with = "required_nullable")]
    selection_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FrozenOperation {
    id: String,
    body: OperationBody,
}

impl FrozenOperation {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn origin_registry_id(&self) -> &str {
        &self.body.origin_registry_id
    }
    pub fn reservation_key(&self) -> &str {
        &self.body.reservation_key
    }
    pub fn confirmation_batch_id(&self) -> &str {
        &self.body.confirmation_batch_id
    }
    pub fn workspace_id(&self) -> &str {
        &self.body.workspace_id
    }
    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("checked integer/string operation")
    }
}

fn checked_operation(
    body: OperationBody,
    batch: &FrozenBatch,
) -> Result<FrozenOperation, ContractError> {
    require(
        body.contract_version == OPERATION_VERSION,
        "unsupported operation version",
    )?;
    for (name, value) in [
        ("originRegistryId", &body.origin_registry_id),
        ("reservationKey", &body.reservation_key),
        ("requestId", &body.request_id),
        ("workspaceId", &body.workspace_id),
    ] {
        short_id(value, name)?;
    }
    let (stage, selection_id) = match &batch.body.stage {
        Stage::Validation {} => (OperationStage::Validation, None),
        Stage::Test { selection_id, .. } => (OperationStage::Test, Some(selection_id.clone())),
    };
    require(
        body.workspace_id == batch.body.source.workspace_id
            && body.confirmation_batch_id == batch.id
            && body.family_id == batch.body.source.family_id
            && body.allocation_id == batch.body.statistics.allocation_id
            && body.stage == stage
            && body.selection_id == selection_id,
        "operation differs from frozen batch",
    )?;
    Ok(FrozenOperation {
        id: identity(OPERATION_VERSION, &body)?,
        body,
    })
}

pub fn freeze_operation(raw: &str, batch: &FrozenBatch) -> Result<FrozenOperation, ContractError> {
    checked_operation(read(raw)?, batch)
}

pub fn parse_operation(raw: &str, batch: &FrozenBatch) -> Result<FrozenOperation, ContractError> {
    let stored: Stored<OperationBody> = read(raw)?;
    let result = checked_operation(stored.body, batch)?;
    require(stored.id == result.id, "operation ID does not reproduce")?;
    Ok(result)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationRelation {
    ExactReplay,
    OriginKeyConflict,
    Distinct,
}

pub fn operation_relation(left: &FrozenOperation, right: &FrozenOperation) -> OperationRelation {
    if left.id == right.id {
        OperationRelation::ExactReplay
    } else if left.body.origin_registry_id == right.body.origin_registry_id
        && left.body.reservation_key == right.body.reservation_key
    {
        OperationRelation::OriginKeyConflict
    } else {
        OperationRelation::Distinct
    }
}

#[cfg(test)]
mod tests;
