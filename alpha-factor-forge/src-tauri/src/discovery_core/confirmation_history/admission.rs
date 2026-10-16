//! Integer-only admission evidence and original-byte blobs. All observations
//! are supplied by the host; this module cannot authenticate them or grant IO.
use super::super::{
    alpha_allocation::{alpha_allocation_id, AlphaAllocationDeclaration},
    confirmation_contracts::verify_report_shape,
    precision::{
        evaluate_precision_plan, PrecisionPlan, PrecisionReason, PrecisionStatus,
        RESEARCH_PRECISION_VERSION,
    },
};
use super::*;
use std::collections::BTreeMap;

pub const ADMISSION_VERSION: &str = "research-confirmation-admission-v1";
pub const BINDING_VERSION: &str = "trial-ledger-binding-v5";
pub const MAX_BINDING_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobKind {
    Statistics,
    Binding,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryBlob {
    sha256: String,
    kind: BlobKind,
    text: String,
    pub(super) binding: Option<BindingObservation>,
}
impl HistoryBlob {
    pub fn new(kind: BlobKind, text: &str) -> Result<Self, HistoryRecordError> {
        let cap = if kind == BlobKind::Binding {
            MAX_BINDING_BYTES
        } else {
            MAX_DOCUMENT_BYTES
        };
        require(text.len() <= cap, HistoryRecordError::TooLarge)?;
        let binding = if kind == BlobKind::Binding {
            Some(parse_binding(text)?)
        } else {
            verify_report_shape(text).map_err(|_| HistoryRecordError::Blob)?;
            None
        };
        Ok(Self {
            sha256: hex::encode(Sha256::digest(text.as_bytes())),
            kind,
            text: text.into(),
            binding,
        })
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn kind(&self) -> BlobKind {
        self.kind
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BindingObservation {
    pub version: String,
    pub registry_id: String,
    pub seq: u64,
    pub chain_head: String,
    pub evidence: TrialEvidence,
    pub alpha: AlphaEvidence,
    pub alpha_unknown_origins: Vec<String>,
    pub confirmation: ConfirmationEvidence,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct TrialEvidence {
    pub quarantined_families: Vec<String>,
    pub conflicted_origins: Vec<String>,
    #[serde(deserialize_with = "unique_family_counts")]
    pub family_tests: BTreeMap<String, u64>,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BudgetIdentity {
    pub family_id: String,
    pub allocation_id: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AlphaEvidence {
    pub budgets: Vec<BudgetIdentity>,
    pub reservations: Vec<String>,
    pub conflicts: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ConfirmationEvidence {
    pub batches: Vec<String>,
    pub selections: Vec<String>,
    pub operations: Vec<String>,
    pub exposures: Vec<String>,
    pub admissions: Vec<String>,
    pub events: Vec<String>,
    pub blobs: Vec<String>,
    pub conflicts: Vec<String>,
    pub unknown_origins: Vec<String>,
}
impl ConfirmationEvidence {
    pub(super) fn sections(&self) -> [&Vec<String>; 8] {
        [
            &self.batches,
            &self.selections,
            &self.operations,
            &self.exposures,
            &self.admissions,
            &self.events,
            &self.blobs,
            &self.conflicts,
        ]
    }
}
fn unique_family_counts<'de, D: Deserializer<'de>>(
    de: D,
) -> Result<BTreeMap<String, u64>, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = BTreeMap<String, u64>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("sorted unique family map")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut values = BTreeMap::new();
            let mut previous: Option<String> = None;
            while let Some((key, value)) = map.next_entry::<String, u64>()? {
                if previous.as_ref().is_some_and(|p| p >= &key) {
                    return Err(serde::de::Error::custom("noncanonical family map"));
                }
                previous = Some(key.clone());
                values.insert(key, value);
            }
            Ok(values)
        }
    }
    de.deserialize_map(Visitor)
}
pub(super) fn family(value: &str) -> bool {
    value
        .strip_prefix("trial-family-v1:")
        .is_some_and(|v| hex(v, 64))
}
fn checked_ids(values: &[String], length: usize) -> Result<(), HistoryRecordError> {
    require(
        ordered(values) && values.iter().all(|v| hex(v, length)),
        HistoryRecordError::Blob,
    )
}
pub(super) fn genesis(registry: &str) -> String {
    hex::encode(Sha256::digest(
        format!("trial-ledger-v1:genesis:{registry}").as_bytes(),
    ))
}
fn parse_binding(raw: &str) -> Result<BindingObservation, HistoryRecordError> {
    let b: BindingObservation = serde_json::from_str(raw).map_err(|_| HistoryRecordError::Blob)?;
    require(
        b.version == BINDING_VERSION
            && hex(&b.registry_id, 32)
            && b.seq <= PRECISION_MAX_COUNT
            && hex(&b.chain_head, 64)
            && (b.seq != 0 || b.chain_head == genesis(&b.registry_id)),
        HistoryRecordError::Blob,
    )?;
    require(
        ordered(&b.evidence.quarantined_families)
            && b.evidence.quarantined_families.iter().all(|v| family(v))
            && b.evidence
                .family_tests
                .iter()
                .all(|(k, v)| family(k) && (1..=PRECISION_MAX_COUNT).contains(v)),
        HistoryRecordError::Blob,
    )?;
    checked_ids(&b.evidence.conflicted_origins, 32)?;
    require(
        b.alpha.budgets.len() + b.alpha.reservations.len() + b.alpha.conflicts.len() <= 30000
            && b.alpha.budgets.windows(2).all(|p| p[0] < p[1])
            && b.alpha
                .budgets
                .iter()
                .all(|v| family(&v.family_id) && hex(&v.allocation_id, 64)),
        HistoryRecordError::Blob,
    )?;
    checked_ids(&b.alpha.reservations, 64)?;
    checked_ids(&b.alpha.conflicts, 64)?;
    require(
        b.alpha_unknown_origins.len() <= 10000
            && b.confirmation.unknown_origins.len() <= 10000
            && b.confirmation
                .sections()
                .iter()
                .map(|v| v.len())
                .sum::<usize>()
                <= 10000,
        HistoryRecordError::Capacity,
    )?;
    checked_ids(&b.alpha_unknown_origins, 32)?;
    checked_ids(&b.confirmation.unknown_origins, 32)?;
    for ids in b.confirmation.sections() {
        checked_ids(ids, 64)?;
    }
    Ok(b)
}

/// Historical prefix/count evidence supplied independently of the receipt.
/// It is not the current admission permit and cannot authenticate its source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalTrialObservation {
    pub registry_id: String,
    pub seq: u64,
    pub chain_head: String,
    pub trial_batch_id: String,
    pub family_id: String,
    pub family_effective_trials: u64,
    pub registered_batch_effective_trials: u64,
    pub tests_per_trial: u64,
    pub complete_candidates: u64,
}
pub struct AdmissionInputs<'a> {
    pub batch: &'a FrozenBatch,
    pub operation: &'a FrozenOperation,
    pub reservation: &'a AlphaReservation,
    pub exposure: &'a ExposureRecord,
    pub binding: &'a HistoryBlob,
    pub trial: &'a HistoricalTrialObservation,
    pub allocation: &'a AlphaAllocationDeclaration,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct WorkspaceObservation {
    pub workspace_id: String,
    pub ownership_epoch: u64,
    pub mutation_seq: u64,
    pub references_sha256: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RegistryObservation {
    pub registry_id: String,
    pub seq: u64,
    pub chain_head: String,
    pub binding_sha256: String,
    pub family_effective_trials: u64,
    pub registered_batch_effective_trials: u64,
    pub tests_per_trial: u64,
    pub protocol_sha256: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Plan {
    contract_version: String,
    correction: String,
    alpha_ppm: u64,
    max_relative_standard_error_ppm: u64,
    prior_trials: u64,
    planned_trials: u64,
    tests_per_trial: u64,
    bootstrap_samples: u64,
    max_bootstrap_samples: u64,
}
impl Plan {
    fn core(&self) -> PrecisionPlan {
        PrecisionPlan {
            alpha_ppm: self.alpha_ppm,
            max_relative_standard_error_ppm: self.max_relative_standard_error_ppm,
            prior_trials: self.prior_trials,
            planned_trials: self.planned_trials,
            tests_per_trial: self.tests_per_trial,
            bootstrap_samples: self.bootstrap_samples,
            max_bootstrap_samples: self.max_bootstrap_samples,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ratio {
    numerator: u64,
    denominator: u64,
}
fn nullable_count<'de, D: Deserializer<'de>>(de: D) -> Result<Option<u64>, D::Error> {
    Option::<u64>::deserialize(de)
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Proof {
    contract_version: String,
    status: String,
    reasons: Vec<String>,
    family_trials: u64,
    pub family_tests: u64,
    min_attainable_raw_p: Ratio,
    best_adjusted_p: Ratio,
    #[serde(deserialize_with = "nullable_count")]
    required_samples_for_resolution: Option<u64>,
    #[serde(deserialize_with = "nullable_count")]
    required_samples_for_precision: Option<u64>,
}
fn proof(plan: &PrecisionPlan) -> Result<Proof, HistoryRecordError> {
    let r = evaluate_precision_plan(plan).map_err(|_| HistoryRecordError::Proof)?;
    require(
        r.status == PrecisionStatus::Eligible,
        HistoryRecordError::Proof,
    )?;
    Ok(Proof {
        contract_version: r.contract_version.into(),
        status: "ELIGIBLE".into(),
        reasons: r
            .reasons
            .iter()
            .map(|v| {
                match v {
                    PrecisionReason::BootstrapResolutionInsufficient => {
                        "bootstrap_resolution_insufficient"
                    }
                    PrecisionReason::MonteCarloPrecisionInsufficient => {
                        "monte_carlo_precision_insufficient"
                    }
                    PrecisionReason::BootstrapBudgetInsufficient => "bootstrap_budget_insufficient",
                }
                .into()
            })
            .collect(),
        family_trials: r.family_trials,
        family_tests: r.family_tests,
        min_attainable_raw_p: Ratio {
            numerator: r.min_attainable_raw_p.numerator,
            denominator: r.min_attainable_raw_p.denominator,
        },
        best_adjusted_p: Ratio {
            numerator: r.best_adjusted_p.numerator,
            denominator: r.best_adjusted_p.denominator,
        },
        required_samples_for_resolution: r.required_samples_for_resolution,
        required_samples_for_precision: r.required_samples_for_precision,
    })
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AdmissionBody {
    pub contract_version: String,
    pub operation_id: String,
    pub alpha_reservation_id: String,
    pub exposure_id: String,
    pub workspace_observation: WorkspaceObservation,
    pub registry_observation: RegistryObservation,
    pub precision_plan: Plan,
    pub precision_proof: Proof,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdmissionRecord {
    pub(super) id: String,
    pub(super) body: AdmissionBody,
}
impl AdmissionRecord {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn document(&self) -> Value {
        serde_json::to_value(self).expect("checked integer admission")
    }
    pub fn family_tests(&self) -> u64 {
        self.body.precision_proof.family_tests
    }
}
fn checked(
    body: AdmissionBody,
    i: &AdmissionInputs<'_>,
) -> Result<AdmissionRecord, HistoryRecordError> {
    require(
        body.contract_version == ADMISSION_VERSION,
        HistoryRecordError::Version,
    )?;
    checked_confirmation_exposure(i.exposure.body.clone(), i.batch, i.operation, i.reservation)?;
    require(
        body.operation_id == i.operation.id()
            && body.alpha_reservation_id == i.reservation.reservation_id()
            && body.exposure_id == i.exposure.id(),
        HistoryRecordError::Binding,
    )?;
    let w = &body.workspace_observation;
    reference(&w.workspace_id, 32)?;
    reference(&w.references_sha256, 64)?;
    count(w.ownership_epoch, 1)?;
    count(w.mutation_seq, 0)?;
    require(
        w.workspace_id == i.operation.workspace_id(),
        HistoryRecordError::Binding,
    )?;
    let b = i.binding.binding.as_ref().ok_or(HistoryRecordError::Blob)?;
    let r = &body.registry_observation;
    let t = i.trial;
    for id in [&r.chain_head, &r.binding_sha256, &r.protocol_sha256] {
        reference(id, 64)?;
    }
    reference(&r.registry_id, 32)?;
    for value in [r.seq, t.seq] {
        count(value, 0)?;
    }
    for value in [
        r.family_effective_trials,
        r.registered_batch_effective_trials,
        r.tests_per_trial,
        t.complete_candidates,
    ] {
        count(value, 1)?;
    }
    require(
        r.registry_id == b.registry_id
            && r.seq == b.seq
            && r.chain_head == b.chain_head
            && r.binding_sha256 == i.binding.sha256()
            && r.registry_id == t.registry_id
            && r.seq == t.seq
            && r.chain_head == t.chain_head
            && t.family_id == i.batch.source().family_id
            && t.trial_batch_id == format!("trial-batch-v1:{}", i.batch.source().trial_batch_id)
            && r.family_effective_trials == t.family_effective_trials
            && r.registered_batch_effective_trials == t.registered_batch_effective_trials
            && r.tests_per_trial == t.tests_per_trial
            && r.tests_per_trial == 2
            && t.complete_candidates <= 4096
            && t.complete_candidates >= i.batch.candidates().len() as u64
            && r.family_effective_trials >= r.registered_batch_effective_trials
            && r.registered_batch_effective_trials >= t.complete_candidates,
        HistoryRecordError::Binding,
    )?;
    let protocol = serde_json::to_vec(&serde_json::json!({"correction":"holm","testsPerTrial":2}))
        .map_err(|_| HistoryRecordError::Malformed)?;
    require(
        r.protocol_sha256 == hex::encode(Sha256::digest(protocol)),
        HistoryRecordError::Binding,
    )?;
    let allocation_id =
        alpha_allocation_id(i.allocation).map_err(|_| HistoryRecordError::Binding)?;
    require(
        allocation_id == i.batch.allocation_id()
            && i.allocation
                .schedule
                .get((i.reservation.confirmation_number() - 1) as usize)
                == Some(&i.reservation.alpha_ppm())
            && b.alpha
                .budgets
                .iter()
                .any(|v| v.family_id == t.family_id && v.allocation_id == allocation_id)
            && b.evidence.family_tests.get(&t.family_id) == Some(&2)
            && !b.evidence.quarantined_families.contains(&t.family_id)
            && !b
                .evidence
                .conflicted_origins
                .iter()
                .any(|v| v == &r.registry_id || v == i.operation.origin_registry_id())
            && b.alpha_unknown_origins.is_empty()
            && b.confirmation.unknown_origins.is_empty()
            && !b
                .alpha
                .reservations
                .iter()
                .any(|v| v == i.reservation.reservation_id()),
        HistoryRecordError::Binding,
    )?;
    let (sampling, cap, error) = i.batch.precision_settings();
    let expected = Plan {
        contract_version: RESEARCH_PRECISION_VERSION.into(),
        correction: "holm".into(),
        alpha_ppm: i.reservation.alpha_ppm(),
        max_relative_standard_error_ppm: error,
        prior_trials: r.family_effective_trials - r.registered_batch_effective_trials,
        planned_trials: r.registered_batch_effective_trials,
        tests_per_trial: r.tests_per_trial,
        bootstrap_samples: sampling,
        max_bootstrap_samples: cap,
    };
    require(
        body.precision_plan == expected && body.precision_proof == proof(&expected.core())?,
        HistoryRecordError::Proof,
    )?;
    let id = identity(ADMISSION_VERSION, &body)?;
    require(
        !b.confirmation.admissions.contains(&id)
            && !b
                .confirmation
                .exposures
                .iter()
                .any(|v| v == i.exposure.id()),
        HistoryRecordError::Dependency,
    )?;
    Ok(AdmissionRecord { id, body })
}
pub fn freeze_admission(
    raw: &str,
    inputs: &AdmissionInputs<'_>,
) -> Result<AdmissionRecord, HistoryRecordError> {
    checked(read(raw)?, inputs)
}
pub fn parse_admission(
    raw: &str,
    inputs: &AdmissionInputs<'_>,
) -> Result<AdmissionRecord, HistoryRecordError> {
    let stored: Stored<AdmissionBody> = read(raw)?;
    let result = checked(stored.body, inputs)?;
    require(stored.id == result.id, HistoryRecordError::Identity)?;
    Ok(result)
}
