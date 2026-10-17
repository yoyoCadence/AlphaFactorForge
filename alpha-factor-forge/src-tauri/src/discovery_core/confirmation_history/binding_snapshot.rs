//! Format-checked immutable v5 snapshots. These contain claims, not independent
//! observations, a checked database graph, freshness or an execution capability.
use super::{admission, HistoryRecordError, MAX_BINDING_BYTES};
use std::collections::BTreeMap;

pub const CONFIRMATION_BINDING_VERSION: &str = admission::BINDING_VERSION;

/// Each confirmation section is covered separately, including permanent
/// uncertainty. Matching counts or IDs in another section cannot cover loss.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfirmationBindingSet {
    Batches,
    Selections,
    Operations,
    Exposures,
    Admissions,
    Events,
    Blobs,
    Conflicts,
    UnknownOrigins,
}
const SETS: [ConfirmationBindingSet; 9] = [
    ConfirmationBindingSet::Batches,
    ConfirmationBindingSet::Selections,
    ConfirmationBindingSet::Operations,
    ConfirmationBindingSet::Exposures,
    ConfirmationBindingSet::Admissions,
    ConfirmationBindingSet::Events,
    ConfirmationBindingSet::Blobs,
    ConfirmationBindingSet::Conflicts,
    ConfirmationBindingSet::UnknownOrigins,
];

/// Only the strict original-text parser constructs this value. No mutable
/// fields or unchecked deserialization can bypass the shared blob codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingSnapshot(admission::BindingObservation);

pub fn parse_binding_snapshot(raw: &str) -> Result<BindingSnapshot, HistoryRecordError> {
    admission::parse_binding(raw).map(BindingSnapshot)
}

impl BindingSnapshot {
    pub fn registry_id(&self) -> &str {
        &self.0.registry_id
    }
    pub fn seq(&self) -> u64 {
        self.0.seq
    }
    pub fn chain_head(&self) -> &str {
        &self.0.chain_head
    }
    pub fn quarantined_families(&self) -> &[String] {
        &self.0.evidence.quarantined_families
    }
    pub fn conflicted_origins(&self) -> &[String] {
        &self.0.evidence.conflicted_origins
    }
    pub fn family_tests(&self) -> &BTreeMap<String, u64> {
        &self.0.evidence.family_tests
    }
    pub fn alpha_budgets(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .alpha
            .budgets
            .iter()
            .map(|b| (b.family_id.as_str(), b.allocation_id.as_str()))
    }
    pub fn alpha_reservations(&self) -> &[String] {
        &self.0.alpha.reservations
    }
    pub fn alpha_conflicts(&self) -> &[String] {
        &self.0.alpha.conflicts
    }
    pub fn alpha_unknown_origins(&self) -> &[String] {
        &self.0.alpha_unknown_origins
    }
    pub fn confirmation_ids(&self, section: ConfirmationBindingSet) -> &[String] {
        let c = &self.0.confirmation;
        match section {
            ConfirmationBindingSet::Batches => &c.batches,
            ConfirmationBindingSet::Selections => &c.selections,
            ConfirmationBindingSet::Operations => &c.operations,
            ConfirmationBindingSet::Exposures => &c.exposures,
            ConfirmationBindingSet::Admissions => &c.admissions,
            ConfirmationBindingSet::Events => &c.events,
            ConfirmationBindingSet::Blobs => &c.blobs,
            ConfirmationBindingSet::Conflicts => &c.conflicts,
            ConfirmationBindingSet::UnknownOrigins => &c.unknown_origins,
        }
    }

    /// Confirmation IDs only, across all nine sets. The registry host must
    /// independently verify the current graph, prefix, trial/alpha evidence
    /// and provenance in one transaction before using this subset result.
    /// Replacement registries can retain the same original IDs, so registry
    /// identity/prefix acceptance is deliberately a separate host check.
    pub fn covers_confirmation(&self, saved: &Self) -> bool {
        SETS.iter().all(|section| {
            let mut current = self.confirmation_ids(*section).iter().peekable();
            saved.confirmation_ids(*section).iter().all(|id| {
                while current.peek().is_some_and(|next| *next < id) {
                    current.next();
                }
                current.next().is_some_and(|next| next == id)
            })
        })
    }

    /// Deterministic lexicographic JSON, without a trailing newline. This is
    /// for a new binding; existing history blobs retain their original bytes.
    pub fn to_canonical_json(&self) -> Result<String, HistoryRecordError> {
        let mut value = serde_json::to_value(&self.0).map_err(|_| HistoryRecordError::Blob)?;
        value.sort_all_objects();
        let bytes = serde_json::to_vec(&value).map_err(|_| HistoryRecordError::Blob)?;
        if bytes.len() > MAX_BINDING_BYTES {
            return Err(HistoryRecordError::TooLarge);
        }
        String::from_utf8(bytes).map_err(|_| HistoryRecordError::Blob)
    }
}

#[cfg(test)]
mod tests;
