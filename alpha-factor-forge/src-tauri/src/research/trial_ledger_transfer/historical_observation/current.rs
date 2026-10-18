//! Actual current accounting of the COMPLETE original snapshot. The borrowed
//! transaction fixes current facts; no receipt, caller counts or nested import.
use super::*;
use crate::research::trial_ledger::{self as ledger, binding::LedgerEvidence};
use alpha_factor_forge::discovery_core::{
    confirmation_history::{HostWitnessCoverage, SourceWitnessObservation},
    precision::PRECISION_MAX_COUNT,
};
use rusqlite::Transaction;

pub(in crate::research::trial_ledger) struct WitnessCurrentContext<'a, 'conn> {
    tx: &'a Transaction<'conn>,
    evidence: LedgerEvidence,
}
impl<'a, 'conn> WitnessCurrentContext<'a, 'conn> {
    pub(in crate::research::trial_ledger) fn new(
        tx: &'a Transaction<'conn>,
        registry: &str,
    ) -> Result<Self, LedgerError> {
        let actual: String = tx.query_row(
            "SELECT value FROM registry_meta WHERE key='registry_id'",
            [],
            |r| r.get(0),
        )?;
        if actual != registry || !valid_registry_id(registry) {
            return Err(bad("invalid current registry"));
        }
        let invalid: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM trial_events WHERE typeof(payload_json)<>'text' OR length(CAST(payload_json AS BLOB))>8388608)",
            [], |r| r.get(0),
        )?;
        if invalid
            || !matches!(
                ledger::verify_chain(tx, registry)
                    .map_err(|_| bad("invalid current trial chain"))?,
                LedgerIntegrity::Intact { .. }
            )
        {
            return Err(bad("invalid current trial chain"));
        }
        // Validate every actual protocol row, including upgrade document/scalar
        // agreement. MAX(tests_per_trial) alone cannot authenticate that row.
        let mut pins = BTreeMap::new();
        let invalid: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM trial_families WHERE typeof(family_key)<>'text' OR length(CAST(family_key AS BLOB))>4096 OR (protocol_json IS NOT NULL AND (typeof(protocol_json)<>'text' OR length(CAST(protocol_json AS BLOB))>4096))) OR EXISTS(SELECT 1 FROM family_protocol_upgrades WHERE typeof(protocol_json)<>'text' OR length(CAST(protocol_json AS BLOB))>4096 OR typeof(tests_per_trial)<>'integer')", [], |r| r.get(0))?;
        if invalid {
            return Err(bad("invalid current protocol bytes"));
        }
        let mut stmt = tx.prepare(
            "SELECT family_id,family_key,protocol_json FROM trial_families ORDER BY family_id",
        )?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (id, key, protocol) = row?;
            let value: Value =
                serde_json::from_str(&key).map_err(|_| bad("invalid current family"))?;
            let instrument = value["instrumentId"]
                .as_str()
                .ok_or_else(|| bad("invalid current family"))?;
            if family_parts(instrument).map_err(|_| bad("invalid current family"))?
                != (id.clone(), key)
            {
                return Err(bad("invalid current family"));
            }
            if let Some(raw) = protocol {
                let n = checked_protocol(&raw)?;
                pins.insert(id, n);
                if pins.len() > 10000 {
                    return Err(bad("current protocol capacity exceeded"));
                }
            }
        }
        let mut stmt = tx.prepare("SELECT family_id,tests_per_trial,protocol_json FROM family_protocol_upgrades ORDER BY family_id,tests_per_trial")?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (family, n, raw) = row?;
            if checked_protocol(&raw)? != n || pins.get(&family).is_none_or(|pin| n <= *pin) {
                return Err(bad("invalid current protocol upgrade"));
            }
        }
        let evidence = ledger::binding::observed_evidence(tx)
            .map_err(|_| bad("invalid current trial evidence"))?;
        let (seq, chain_head) = head(tx, registry)?;
        SourceTrialAnchor::from_host_facts(
            HistoricalPrefix {
                registry_id: registry.into(),
                seq,
                chain_head,
            },
            evidence.quarantined_families.iter().cloned().collect(),
            evidence.conflicted_origins.iter().cloned().collect(),
            evidence.family_tests.clone(),
        )
        .map_err(|_| bad("invalid current trial evidence"))?;
        Ok(Self { tx, evidence })
    }
    pub(in crate::research::trial_ledger) fn evidence(&self) -> &LedgerEvidence {
        &self.evidence
    }

    pub(in crate::research::trial_ledger) fn observe(
        &self,
        witness: &SourceWitnessSnapshot,
    ) -> Result<(SourceWitnessObservation, HostWitnessCoverage), LedgerError> {
        let (archive, anchor) = parse_original_archive(
            witness.trial_export_text().as_bytes(),
            witness.source_claims(),
        )?;
        if !anchor
            .quarantined_families()
            .iter()
            .all(|v| self.evidence.quarantined_families.contains(v))
            || !anchor
                .conflicted_origins()
                .iter()
                .all(|v| self.evidence.conflicted_origins.contains(v))
        {
            return Err(bad("original restrictive evidence is missing"));
        }
        let mut gaps = BTreeSet::new();
        let mut stmt = self
            .tx
            .prepare("SELECT payload_json,batch_id FROM trial_events WHERE event_id=?1")?;
        // This includes every original event, not only the target source batch,
        // family or events originally registered by the archive's primary ID.
        for (id, payload, origin, batch, _, _) in &archive.events {
            let found: Option<(String, String)> = stmt
                .query_row([id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            if found != Some((canonical_text(payload)?, batch.clone())) {
                gaps.insert(origin.clone());
            }
        }
        for (family, tests) in anchor.family_tests() {
            if self
                .evidence
                .family_tests
                .get(family)
                .is_none_or(|v| v < tests)
                && !self.evidence.quarantined_families.contains(family)
            {
                // Upgrade rows have no original-origin column. The complete
                // archive's registry is the origin of this unincorporated claim.
                gaps.insert(archive.header.registry_id.clone());
            }
        }
        let observed = observe_original(archive, anchor, witness.source_claims())?;
        let facts = SourceWitnessObservation::from_host_facts(
            witness.clone(),
            observed.anchor().clone(),
            observed.trial().clone(),
        )
        .map_err(|_| bad("invalid original witness observations"))?;
        Ok((
            facts,
            HostWitnessCoverage {
                witness_sha256: witness.sha256().into(),
                unincorporated_origins: gaps.into_iter().collect(),
            },
        ))
    }
}
fn checked_protocol(raw: &str) -> Result<u64, LedgerError> {
    let n = protocol_tests(raw).map_err(|_| bad("invalid current protocol"))?;
    if !(1..=PRECISION_MAX_COUNT).contains(&n) || ledger::protocol_json(n)? != raw {
        return Err(bad("invalid current protocol"));
    }
    Ok(n)
}
