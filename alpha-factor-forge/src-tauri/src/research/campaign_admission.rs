//! P12d-2b: the admission decision for one instrument of a frozen campaign
//! (`research-campaign-admission-v1`, docs/research-campaign-declaration-v1.md).
//!
//! It combines what the earlier slices verified — the exact P06 snapshot
//! (P12d-2a), the current trial-ledger count or its §6.4 fence (P12b), every
//! candidate's Train-only walk-forward plan (P12c) and the campaign's P12a
//! sampling — into `ELIGIBLE` or `NOT_ELIGIBLE` with every failing reason.
//! Pure over those inputs: no database, registry, runner, clock or randomness.
//!
//! `ELIGIBLE` only means the declared confirmation is feasible and may be
//! scheduled after a fresh fence. It is never a confirmation `PASS` (P12e/P13).
//! A binding that contradicts the campaign is an error, not `NOT_ELIGIBLE`:
//! the caller must refuse to link that run to the campaign.
#![allow(dead_code)] // P12d-2c adds the runner caller and persistence.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use alpha_factor_forge::discovery_core::{
    campaign::FrozenCampaignDeclaration,
    precision::{evaluate_precision_plan, PrecisionReport, PrecisionStatus},
    walk_forward::{
        evaluate_walk_forward_plan, WalkForwardReport, WalkForwardStatus, WALK_FORWARD_VERSION,
    },
};

use super::campaign_snapshot::ResolvedInstrument;
use super::trial_ledger::{
    family_id_for, precision_plan_from_count, Admission, AdmissionBlocked, AdmissionFence,
    AdmissionSnapshot, FenceBlocked, SamplingPlan,
};
use crate::error::{AppError, AppResult};

pub const CAMPAIGN_ADMISSION_VERSION: &str = "research-campaign-admission-v1";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Declaration {
    sampling: Sampling,
    instruments: Vec<InstrumentBinding>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sampling {
    alpha_ppm: u64,
    max_relative_standard_error_ppm: u64,
    bootstrap_samples: u64,
    max_bootstrap_samples: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstrumentBinding {
    instrument_id: String,
    snapshot_id: String,
    dataset_hash: String,
    sample_policy: SamplePolicy,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SamplePolicy {
    minimum_total_bars: u64,
    minimum_train_bars: u64,
    fold_validation_bars: u64,
    fold_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CampaignAdmissionStatus {
    Eligible,
    NotEligible,
}

/// Every failing check, reported once each in this declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignAdmissionReason {
    /// The snapshot re-verified now is not the one the decision recorded.
    SnapshotChanged,
    /// §6.4 step 1: the decision's registry prefix is not provably present.
    LedgerPrefixUnproven,
    /// §6.4 step 2: the family shrank or its family/protocol/batch changed.
    LedgerCountInconsistent,
    FamilyUnknown,
    FamilyQuarantined,
    RegistryChainBroken,
    NoEffectiveTrials,
    /// The workspace has pre-P05 history in this family that no ledger counts.
    LegacyTrialsUnknown,
    InsufficientTotalBars,
    WalkForwardNotEligible,
    PrecisionNotEligible,
}

/// The exact snapshot rows a decision was made on (P12d-2a observation).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotObservation {
    pub snapshot_id: String,
    pub snapshot_row_id: i64,
    pub instrument_row_id: i64,
    pub dataset_id: i64,
    pub dataset_hash: String,
    pub bar_count: u64,
    pub cost_profile_version: String,
}

/// One candidate's P12c plan as the runner derived it from verified inputs:
/// exactly one of `report` (possibly `NOT_ELIGIBLE`) or `error` (no plan, e.g.
/// its signal lookback exceeds the declared minimum Train length).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateFeasibility {
    pub candidate_index: i64,
    pub strategy_hash: String,
    pub report: Option<WalkForwardReport>,
    pub error: Option<String>,
}

/// Where the ledger half of the decision comes from.
#[derive(Clone, Copy, Debug)]
pub enum LedgerInput<'a> {
    /// Read with the registration (or `read_admission_count`) in one registry
    /// transaction: a first decision.
    Registered(&'a Admission),
    /// A §6.4 fence against the snapshot a stored decision used: required
    /// before anything acts on that decision.
    Fenced(&'a AdmissionFence),
}

pub struct CampaignAdmissionInput<'a> {
    pub campaign: &'a FrozenCampaignDeclaration,
    pub instrument_id: &'a str,
    /// Verified at this read by `resolve_campaign_snapshots`.
    pub resolved: &'a ResolvedInstrument,
    /// What a stored decision observed; `None` for a first decision.
    pub earlier_snapshot: Option<&'a SnapshotObservation>,
    pub batch_id: &'a str,
    pub ledger: LedgerInput<'a>,
    /// `LedgerWorkspaceReport::legacy_trials_unknown` (trial-ledger-v1 §9).
    pub legacy_trials_unknown: &'a [String],
    pub candidates: &'a [CandidateFeasibility],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignAdmissionReport {
    pub contract_version: &'static str,
    pub campaign_id: String,
    pub instrument_id: String,
    pub family_id: String,
    pub batch_id: String,
    pub status: CampaignAdmissionStatus,
    pub reasons: Vec<CampaignAdmissionReason>,
    pub snapshot: SnapshotObservation,
    /// `null` for a first decision; otherwise `unchanged`, `grew`, or the
    /// fence's blocking code.
    pub ledger_fence: Option<&'static str>,
    /// Stored with the decision so a later fence can compare against it.
    pub ledger: Option<AdmissionSnapshot>,
    pub precision: Option<PrecisionReport>,
    pub walk_forward: Vec<CandidateFeasibility>,
}

fn invalid(instrument_id: &str, reason: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("campaign admission {instrument_id}: {reason}"))
}

fn blocked_reason(reason: &AdmissionBlocked) -> CampaignAdmissionReason {
    match reason {
        AdmissionBlocked::FamilyUnknown => CampaignAdmissionReason::FamilyUnknown,
        AdmissionBlocked::FamilyQuarantined => CampaignAdmissionReason::FamilyQuarantined,
        AdmissionBlocked::ChainBroken => CampaignAdmissionReason::RegistryChainBroken,
        AdmissionBlocked::NoEffectiveTrials => CampaignAdmissionReason::NoEffectiveTrials,
    }
}

/// Decide admission for one declared instrument from verified inputs. A first
/// decision takes the count read with its registration; before an `ELIGIBLE`
/// decision is acted on, call this again with the §6.4 fence and a freshly
/// re-verified snapshot.
pub fn evaluate_campaign_admission(
    input: &CampaignAdmissionInput<'_>,
) -> AppResult<CampaignAdmissionReport> {
    let id = input.instrument_id;
    let resolved = input.resolved;
    let (declared_sampling, binding) =
        bind(input.campaign, id, resolved, input.candidates)?;
    if input.batch_id.trim().is_empty() {
        return Err(invalid(id, "batch id is missing"));
    }
    let family_id = family_id_for(id).map_err(|error| invalid(id, error))?;

    let snapshot = SnapshotObservation {
        snapshot_id: resolved.snapshot_id.clone(),
        snapshot_row_id: resolved.snapshot_row_id,
        instrument_row_id: resolved.instrument_row_id,
        dataset_id: resolved.dataset_id,
        dataset_hash: resolved.dataset_hash.clone(),
        bar_count: resolved.bar_count,
        cost_profile_version: resolved.cost_profile_version.clone(),
    };
    let mut reasons = BTreeSet::new();
    if input
        .earlier_snapshot
        .is_some_and(|earlier| *earlier != snapshot)
    {
        reasons.insert(CampaignAdmissionReason::SnapshotChanged);
    }

    let (count, ledger_fence) = match input.ledger {
        LedgerInput::Registered(Admission::Count(count)) => (Some(count), None),
        LedgerInput::Registered(Admission::Blocked(reason)) => {
            reasons.insert(blocked_reason(reason));
            (None, None)
        }
        LedgerInput::Fenced(AdmissionFence::Unchanged(count)) => (Some(count), Some("unchanged")),
        LedgerInput::Fenced(AdmissionFence::Grew(count)) => (Some(count), Some("grew")),
        LedgerInput::Fenced(AdmissionFence::Blocked(blocked)) => {
            reasons.insert(match blocked {
                FenceBlocked::Prefix(_) => CampaignAdmissionReason::LedgerPrefixUnproven,
                FenceBlocked::Inconsistent => CampaignAdmissionReason::LedgerCountInconsistent,
                FenceBlocked::Admission(reason) => blocked_reason(reason),
            });
            (None, Some(blocked.code()))
        }
    };
    if let Some(count) = count {
        if count.family_id() != family_id {
            return Err(invalid(id, "ledger count belongs to another family"));
        }
        if count.batch_id() != input.batch_id {
            return Err(invalid(id, "ledger count belongs to another batch"));
        }
    }
    if input.legacy_trials_unknown.contains(&family_id) {
        reasons.insert(CampaignAdmissionReason::LegacyTrialsUnknown);
    }
    if resolved.bar_count < binding.sample_policy.minimum_total_bars {
        reasons.insert(CampaignAdmissionReason::InsufficientTotalBars);
    }
    if input.candidates.iter().any(|candidate| {
        candidate
            .report
            .as_ref()
            .is_none_or(|report| report.status != WalkForwardStatus::Eligible)
    }) {
        reasons.insert(CampaignAdmissionReason::WalkForwardNotEligible);
    }
    // Precision is computed whenever a current count exists, even alongside
    // other reasons, so the stored decision shows the full picture.
    let sampling = SamplingPlan {
        alpha_ppm: declared_sampling.alpha_ppm,
        max_relative_standard_error_ppm: declared_sampling.max_relative_standard_error_ppm,
        bootstrap_samples: declared_sampling.bootstrap_samples,
        max_bootstrap_samples: declared_sampling.max_bootstrap_samples,
    };
    let precision = count
        .map(|count| {
            evaluate_precision_plan(&precision_plan_from_count(count, &sampling))
                .map_err(|error| invalid(id, error))
        })
        .transpose()?;
    if precision
        .as_ref()
        .is_some_and(|report| report.status != PrecisionStatus::Eligible)
    {
        reasons.insert(CampaignAdmissionReason::PrecisionNotEligible);
    }

    let reasons: Vec<_> = reasons.into_iter().collect();
    Ok(CampaignAdmissionReport {
        contract_version: CAMPAIGN_ADMISSION_VERSION,
        campaign_id: input.campaign.campaign_id().to_string(),
        instrument_id: id.to_string(),
        family_id,
        batch_id: input.batch_id.to_string(),
        status: if reasons.is_empty() {
            CampaignAdmissionStatus::Eligible
        } else {
            CampaignAdmissionStatus::NotEligible
        },
        reasons,
        snapshot,
        ledger_fence,
        ledger: count.map(|count| count.snapshot()),
        precision,
        walk_forward: input.candidates.to_vec(),
    })
}

/// Every check that makes a run this campaign's run, without the ledger. A
/// runner calls it before registering trials, so a run that contradicts the
/// campaign registers nothing; `evaluate_campaign_admission` repeats it.
pub fn validate_campaign_binding(
    campaign: &FrozenCampaignDeclaration,
    instrument_id: &str,
    resolved: &ResolvedInstrument,
    candidates: &[CandidateFeasibility],
) -> AppResult<()> {
    bind(campaign, instrument_id, resolved, candidates).map(|_| ())
}

fn bind(
    campaign: &FrozenCampaignDeclaration,
    id: &str,
    resolved: &ResolvedInstrument,
    candidates: &[CandidateFeasibility],
) -> AppResult<(Sampling, InstrumentBinding)> {
    let Declaration {
        sampling,
        instruments,
    } = serde_json::from_value(campaign.document().clone())?;
    let binding = instruments
        .into_iter()
        .find(|binding| binding.instrument_id == id)
        .ok_or_else(|| invalid(id, "instrument is not declared by this campaign"))?;
    if resolved.instrument_id != id
        || resolved.snapshot_id != binding.snapshot_id
        || resolved.dataset_hash != binding.dataset_hash
    {
        return Err(invalid(
            id,
            "resolved snapshot does not match the declaration",
        ));
    }
    validate_candidates(id, candidates, &binding, resolved.bar_count)?;
    Ok((sampling, binding))
}

/// Every candidate must be planned against this campaign's policy and the
/// verified bar count; a mismatch means the run is not this campaign's run.
fn validate_candidates(
    id: &str,
    candidates: &[CandidateFeasibility],
    binding: &InstrumentBinding,
    bar_count: u64,
) -> AppResult<()> {
    if candidates.is_empty() {
        return Err(invalid(id, "no candidates to admit"));
    }
    let mut indexes = BTreeSet::new();
    for candidate in candidates {
        let index = candidate.candidate_index;
        if index < 0 {
            return Err(invalid(id, "candidate index must be non-negative"));
        }
        if !indexes.insert(index) {
            return Err(invalid(id, format!("candidate {index} is listed twice")));
        }
        if candidate.strategy_hash.trim().is_empty() {
            return Err(invalid(
                id,
                format!("candidate {index} has no strategy hash"),
            ));
        }
        match (&candidate.report, &candidate.error) {
            (Some(report), None) => {
                let policy = &binding.sample_policy;
                let plan = &report.plan;
                if report.contract_version != WALK_FORWARD_VERSION
                    || plan.total_bars != bar_count
                    || plan.minimum_train_bars != policy.minimum_train_bars
                    || plan.fold_validation_bars != policy.fold_validation_bars
                    || plan.fold_count != policy.fold_count
                {
                    return Err(invalid(
                        id,
                        format!("candidate {index} walk-forward plan differs from the campaign sample policy"),
                    ));
                }
                // Reports are public data, not proof that this plan passed
                // P12c. Recompute all derived evidence before trusting it.
                let expected = evaluate_walk_forward_plan(plan)
                    .map_err(|error| invalid(id, format!("candidate {index}: {error}")))?;
                if *report != expected {
                    return Err(invalid(
                        id,
                        format!("candidate {index} walk-forward report disagrees with its plan"),
                    ));
                }
            }
            (None, Some(error)) if !error.trim().is_empty() => {}
            _ => {
                return Err(invalid(
                    id,
                    format!(
                        "candidate {index} needs exactly one of a walk-forward report or an error"
                    ),
                ))
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
