//! Read-only original-source reconstruction. Its snapshot-derived historical
//! counts are separate from current AdmissionCount and grant no live authority.
use super::*;
use alpha_factor_forge::discovery_core::{
    confirmation_contracts::{validate_registered_source, RegisteredSource},
    confirmation_history::{BindingSnapshot, HistoricalPrefix, HistoricalTrialObservation},
};

pub(in crate::research::trial_ledger) const MAX_HISTORICAL_ARCHIVE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(in crate::research::trial_ledger) struct HistoricalSourceObservation {
    archive_sha256: String,
    source: RegisteredSource,
    prefix: HistoricalPrefix,
    trial: HistoricalTrialObservation,
}
impl HistoricalSourceObservation {
    pub(in crate::research::trial_ledger) fn archive_sha256(&self) -> &str {
        &self.archive_sha256
    }
    /// Domains and exact trial membership were checked. Artifact/campaign/run/
    /// manifest metadata remain supplied claims, without file authentication.
    pub(in crate::research::trial_ledger) fn source_claims(&self) -> &RegisteredSource {
        &self.source
    }
    pub(in crate::research::trial_ledger) fn prefix(&self) -> &HistoricalPrefix {
        &self.prefix
    }
    pub(in crate::research::trial_ledger) fn trial(&self) -> &HistoricalTrialObservation {
        &self.trial
    }
}
fn bad(detail: &str) -> LedgerError {
    LedgerError::StateInvalid(format!("historical trial source: {detail}"))
}

/// This does not import/adopt anything or read receiver rows. The source's
/// complete event chain, actual membership and protocol determine counts;
/// receipt family-before numbers and a larger current receiver are not inputs.
pub(in crate::research::trial_ledger) fn reconstruct_historical_source(
    bytes: &[u8],
    binding: &BindingSnapshot,
    source: &RegisteredSource,
) -> Result<HistoricalSourceObservation, LedgerError> {
    if bytes.len() > MAX_HISTORICAL_ARCHIVE_BYTES {
        return Err(bad("archive capacity exceeded"));
    }
    validate_registered_source(source).map_err(|_| bad("invalid source metadata"))?;
    let archive = ValidatedExport::parse(bytes).map_err(|_| bad("invalid original archive"))?;
    // A receiver cannot fill gaps in this original proof. Imports alone retain
    // their existing SQL fallback through the shared resolver-based validator.
    archive
        .validate_kinds_resolving(|_| Ok(None))
        .map_err(|_| bad("invalid or incomplete source provenance"))?;
    if archive.header.registry_id != binding.registry_id()
        || archive.header.head_seq != binding.seq()
        || archive.header.head_chain != binding.chain_head()
    {
        return Err(bad("original prefix differs"));
    }
    let quarantined: BTreeSet<_> = archive
        .family_conflicts
        .iter()
        .map(|(id, _, _)| id.clone())
        .collect();
    let conflicted: BTreeSet<_> = archive
        .registry_conflicts
        .iter()
        .map(|(id, _)| id.clone())
        .collect();
    let mut protocols = BTreeMap::new();
    for (family, (_, protocol)) in &archive.families {
        if let Some(protocol) = protocol {
            protocols.insert(
                family.clone(),
                protocol_tests(protocol).map_err(|_| bad("invalid source protocol"))?,
            );
        }
    }
    for (family, tests) in &archive.upgrades {
        let value = protocols
            .get_mut(family)
            .ok_or_else(|| bad("invalid source protocol"))?;
        *value = (*value).max(*tests);
    }
    if quarantined.iter().ne(binding.quarantined_families().iter())
        || conflicted.iter().ne(binding.conflicted_origins().iter())
        || &protocols != binding.family_tests()
    {
        return Err(bad("original trial evidence differs"));
    }
    let batch_id = format!("trial-batch-v1:{}", source.source.trial_batch_id);
    let (family, _, members) = archive
        .batches
        .get(&batch_id)
        .ok_or_else(|| bad("original source batch is missing"))?;
    if family.as_deref() != Some(source.source.family_id.as_str()) {
        return Err(bad("original source family differs"));
    }
    let mut candidates = BTreeMap::new();
    let mut batch_effective = 0u64;
    let mut family_effective = 0u64;
    for (_, payload, _, event_batch, _, _) in &archive.events {
        let effective = payload["effective"] == true;
        if payload["familyId"].as_str() == Some(source.source.family_id.as_str()) && effective {
            family_effective += 1;
        }
        if event_batch == &batch_id {
            let index = payload["candidateIndex"]
                .as_u64()
                .ok_or_else(|| bad("invalid original candidate membership"))?;
            let strategy = payload["strategyHash"]
                .as_str()
                .ok_or_else(|| bad("invalid original candidate membership"))?;
            if payload["workspaceId"].as_str() != Some(source.source.workspace_id.as_str())
                || candidates.insert(index, strategy).is_some()
            {
                return Err(bad("invalid original candidate membership"));
            }
            batch_effective += u64::from(effective);
        }
    }
    if candidates.len() != members.len()
        || candidates.len() != source.candidates.len()
        || source.candidates.iter().any(|candidate| {
            candidates.get(&candidate.candidate_index).copied()
                != Some(candidate.strategy_hash.as_str())
        })
    {
        return Err(bad("incomplete or different original candidates"));
    }
    let tests_per_trial = protocols
        .get(&source.source.family_id)
        .copied()
        .ok_or_else(|| bad("original source protocol is missing"))?;
    let prefix = HistoricalPrefix {
        registry_id: archive.header.registry_id,
        seq: archive.header.head_seq,
        chain_head: archive.header.head_chain,
    };
    let trial = HistoricalTrialObservation {
        registry_id: prefix.registry_id.clone(),
        seq: prefix.seq,
        chain_head: prefix.chain_head.clone(),
        trial_batch_id: batch_id,
        family_id: source.source.family_id.clone(),
        family_effective_trials: family_effective,
        registered_batch_effective_trials: batch_effective,
        tests_per_trial,
        complete_candidates: candidates.len() as u64,
    };
    Ok(HistoricalSourceObservation {
        archive_sha256: archive.file_sha256,
        source: source.clone(),
        prefix,
        trial,
    })
}

#[cfg(test)]
#[path = "historical_observation/tests.rs"]
mod tests;
