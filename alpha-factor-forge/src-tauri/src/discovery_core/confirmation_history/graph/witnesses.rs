//! Strict proof dependencies. Facts remain explicitly supplied by a host that
//! reconstructs original archives and checks actual current accounting/stops.
use super::*;
use crate::discovery_core::confirmation_contracts::SourceBinding;

type SourceKey = (String, u64, String);
fn source_key(s: &SourceBinding) -> SourceKey {
    (s.workspace_id.clone(), s.run_id, s.trial_batch_id.clone())
}
pub(super) struct WitnessHost<'a> {
    sources: BTreeMap<SourceKey, &'a RegisteredSource>,
    roots: BTreeMap<SourceKey, &'a SourceWitnessObservation>,
    groups: BTreeMap<(SourceKey, SourceTrialAnchor), &'a SourceWitnessObservation>,
}
impl<'a> WitnessHost<'a> {
    pub(super) fn new(
        i: &ConfirmationInventory,
        c: &'a GraphContext,
        observations: &'a [SourceWitnessObservation],
    ) -> Result<Self, HistoryRecordError> {
        require(
            observations.len() <= MAX_HISTORY_ROWS && c.witness_coverage.len() <= MAX_HISTORY_ROWS,
            HistoryRecordError::Capacity,
        )?;
        let mut coverage = BTreeMap::new();
        for fact in &c.witness_coverage {
            require(
                hex(&fact.witness_sha256, 64)
                    && fact.unincorporated_origins.len() <= 10000
                    && ordered(&fact.unincorporated_origins)
                    && fact.unincorporated_origins.iter().all(|v| hex(v, 32)),
                HistoryRecordError::Binding,
            )?;
            if let Some(old) = coverage.insert(fact.witness_sha256.as_str(), fact) {
                require(old == fact, HistoryRecordError::Binding)?;
            }
        }
        let blobs: BTreeMap<_, _> = i
            .blobs
            .iter()
            .filter(|b| b.kind() == BlobKind::SourceWitness)
            .map(|b| (b.sha256(), b))
            .collect();
        let mut witnesses: BTreeMap<String, &'a SourceWitnessObservation> = BTreeMap::new();
        let mut sources: BTreeMap<SourceKey, &'a RegisteredSource> = BTreeMap::new();
        let mut roots: BTreeMap<SourceKey, &'a SourceWitnessObservation> = BTreeMap::new();
        let mut groups: BTreeMap<(SourceKey, SourceTrialAnchor), &'a SourceWitnessObservation> =
            BTreeMap::new();
        for w in observations {
            let snapshot = w.witness();
            let blob = blobs
                .get(snapshot.sha256())
                .ok_or(HistoryRecordError::Dependency)?;
            require(blob.text() == snapshot.text(), HistoryRecordError::Blob)?;
            if let Some(old) = witnesses.insert(snapshot.sha256().into(), w) {
                require(old == w, HistoryRecordError::Binding)?;
            }
            let source = snapshot.source_claims();
            let key = source_key(&source.source);
            if let Some(old) = sources.insert(key.clone(), source) {
                require(old == source, HistoryRecordError::Binding)?;
            }
            if let Some(old) = roots.get(&key) {
                if snapshot.sha256() < old.witness().sha256() {
                    roots.insert(key.clone(), w);
                }
            } else {
                roots.insert(key.clone(), w);
            }
            let group = (key, w.anchor().clone());
            if let Some(old) = groups.get(&group) {
                require(old.trial() == w.trial(), HistoryRecordError::Binding)?;
                if snapshot.sha256() < old.witness().sha256() {
                    groups.insert(group, w);
                }
            } else {
                groups.insert(group, w);
            }
            let a = w.anchor();
            // Missing old restrictive witnesses cannot be repaired by merely
            // flagging an origin unknown. Protocol/event incorporation is a
            // separate current-host obligation, with actual permanent stops.
            require(
                a.quarantined_families()
                    .iter()
                    .all(|v| c.quarantined_families.binary_search(v).is_ok())
                    && a.conflicted_origins()
                        .iter()
                        .all(|v| c.conflicted_origins.binary_search(v).is_ok()),
                HistoryRecordError::Dependency,
            )?;
            let accounted = c.historical_prefixes.contains(a.prefix())
                && a.family_tests().iter().all(|(k, v)| {
                    c.family_tests.get(k).is_some_and(|n| n >= v)
                        || c.quarantined_families.binary_search(k).is_ok()
                });
            let current = coverage
                .get(snapshot.sha256())
                .ok_or(HistoryRecordError::Dependency)?;
            if current.unincorporated_origins.is_empty() {
                require(accounted, HistoryRecordError::Dependency)?;
            } else {
                require(
                    current.unincorporated_origins.iter().all(|origin| {
                        i.unknown_origins.binary_search(origin).is_ok()
                            || c.conflicted_origins.binary_search(origin).is_ok()
                    }),
                    HistoryRecordError::Dependency,
                )?;
            }
        }
        require(
            witnesses.len() == blobs.len(),
            HistoryRecordError::Dependency,
        )?;
        for source in &c.registered_sources {
            if let Some(known) = sources.get(&source_key(&source.source)) {
                require(*known == source, HistoryRecordError::Binding)?;
            }
        }
        Ok(Self {
            sources,
            roots,
            groups,
        })
    }
    pub(super) fn source(&self, value: &Value) -> Result<&'a RegisteredSource, HistoryRecordError> {
        let source: SourceBinding =
            serde_json::from_value(value.clone()).map_err(|_| HistoryRecordError::Binding)?;
        let key = (
            source.workspace_id.clone(),
            source.run_id,
            source.trial_batch_id.clone(),
        );
        let full = self
            .sources
            .get(&key)
            .copied()
            .ok_or(HistoryRecordError::Dependency)?;
        require(full.source == source, HistoryRecordError::Binding)?;
        Ok(full)
    }
    fn matching(
        &self,
        source: &SourceBinding,
        binding: Option<&BindingObservation>,
    ) -> Result<&'a SourceWitnessObservation, HistoryRecordError> {
        let source_key = source_key(source);
        let w = if let Some(b) = binding {
            self.groups
                .get(&(source_key, SourceTrialAnchor::from_binding_observation(b)))
        } else {
            self.roots.get(&source_key)
        };
        let w = w.copied().ok_or(HistoryRecordError::Dependency)?;
        require(
            &w.witness().source_claims().source == source,
            HistoryRecordError::Binding,
        )?;
        Ok(w)
    }
    pub(super) fn source_dependencies(
        &self,
        deps: &mut BTreeSet<Key>,
        source: &Value,
        binding: Option<&BindingObservation>,
    ) -> Result<(), HistoryRecordError> {
        let source = self.source(source)?;
        // All roots/facts were checked above. One deterministic exact matching
        // proof edge avoids quadratic fan-out from audit-only duplicate proofs.
        let w = self.matching(&source.source, binding)?;
        deps.insert(key(Kind::Blob, w.witness().sha256()));
        Ok(())
    }
    pub(super) fn trial(
        &self,
        binding: &BindingObservation,
        source: &SourceBinding,
    ) -> Result<&'a HistoricalTrialObservation, HistoryRecordError> {
        let first = self.matching(source, Some(binding))?.trial();
        require(
            first.tests_per_trial == 2
                && first.registered_batch_effective_trials == first.complete_candidates,
            HistoryRecordError::Binding,
        )?;
        Ok(first)
    }
}
