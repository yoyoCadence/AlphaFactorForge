//! Explicit host facts; the host must first reconstruct the complete original
//! archive. Private fields enforce consistency, not publisher authentication.
use super::*;
use crate::discovery_core::confirmation_history::HistoricalTrialObservation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceWitnessObservation {
    witness: SourceWitnessSnapshot,
    anchor: SourceTrialAnchor,
    trial: HistoricalTrialObservation,
}
impl SourceWitnessObservation {
    pub fn from_host_facts(
        witness: SourceWitnessSnapshot,
        anchor: SourceTrialAnchor,
        trial: HistoricalTrialObservation,
    ) -> Result<Self, HistoryRecordError> {
        let source = witness.source_claims();
        let prefix = anchor.prefix();
        if trial.registry_id != prefix.registry_id
            || trial.seq != prefix.seq
            || trial.chain_head != prefix.chain_head
            || trial.family_id != source.source.family_id
            || trial.trial_batch_id != format!("trial-batch-v1:{}", source.source.trial_batch_id)
            || trial.complete_candidates != source.candidates.len() as u64
            || trial.family_effective_trials > PRECISION_MAX_COUNT
            || trial.family_effective_trials < trial.registered_batch_effective_trials
            || trial.registered_batch_effective_trials > trial.complete_candidates
            || anchor.family_tests().get(&trial.family_id) != Some(&trial.tests_per_trial)
        {
            return Err(HistoryRecordError::Binding);
        }
        Ok(Self {
            witness,
            anchor,
            trial,
        })
    }
    pub fn witness(&self) -> &SourceWitnessSnapshot {
        &self.witness
    }
    pub fn anchor(&self) -> &SourceTrialAnchor {
        &self.anchor
    }
    pub fn trial(&self) -> &HistoricalTrialObservation {
        &self.trial
    }
}
