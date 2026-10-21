//! The v5 codec is format authority; strict witnessed IO is database authority.
//! Neither a parsed binding nor an archival graph grants execution rights.
use super::*;
use alpha_factor_forge::discovery_core::confirmation_history::{
    parse_binding_snapshot, BindingSnapshot, ConfirmationBindingSet,
};

pub(super) const MAX_BYTES: usize =
    alpha_factor_forge::discovery_core::confirmation_history::MAX_BINDING_BYTES;
const SECTIONS: [(ConfirmationBindingSet, &str); 9] = [
    (ConfirmationBindingSet::Batches, "batches"),
    (ConfirmationBindingSet::Selections, "selections"),
    (ConfirmationBindingSet::Operations, "operations"),
    (ConfirmationBindingSet::Exposures, "exposures"),
    (ConfirmationBindingSet::Admissions, "admissions"),
    (ConfirmationBindingSet::Events, "events"),
    (ConfirmationBindingSet::Blobs, "blobs"),
    (ConfirmationBindingSet::Conflicts, "conflicts"),
    (ConfirmationBindingSet::UnknownOrigins, "unknownOrigins"),
];

/// Immutable exact identities. Constructed only from the strict original-text
/// codec or the complete, content/projection checked graph in a registry TX.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmationEvidence {
    sets: [Vec<String>; 9],
}
impl ConfirmationEvidence {
    pub fn ids(&self, section: ConfirmationBindingSet) -> &[String] {
        let index = SECTIONS
            .iter()
            .position(|(set, _)| *set == section)
            .unwrap();
        &self.sets[index]
    }
}

pub(super) fn parse(raw: &str) -> Result<LedgerBinding, LedgerError> {
    let checked =
        parse_binding_snapshot(raw).map_err(|_| invalid_binding("invalid original v5 binding"))?;
    Ok(LedgerBinding {
        registry_id: checked.registry_id().into(),
        seq: checked.seq(),
        chain_head: checked.chain_head().into(),
        evidence: Some(LedgerEvidence {
            quarantined_families: checked.quarantined_families().iter().cloned().collect(),
            conflicted_origins: checked.conflicted_origins().iter().cloned().collect(),
            family_tests: checked.family_tests().clone(),
        }),
        alpha: Some(Box::new(AlphaEvidence {
            budgets: checked
                .alpha_budgets()
                .map(|(family, allocation)| AlphaBudgetIdentity {
                    family_id: family.into(),
                    allocation_id: allocation.into(),
                })
                .collect(),
            reservations: checked.alpha_reservations().iter().cloned().collect(),
            conflicts: checked.alpha_conflicts().iter().cloned().collect(),
        })),
        alpha_unknown_origins: Some(checked.alpha_unknown_origins().iter().cloned().collect()),
        confirmation: Some(Box::new(ConfirmationEvidence {
            sets: std::array::from_fn(|n| checked.confirmation_ids(SECTIONS[n].0).to_vec()),
        })),
    })
}

pub(super) fn snapshot(binding: &LedgerBinding) -> Result<BindingSnapshot, LedgerError> {
    let evidence = binding
        .evidence
        .as_ref()
        .ok_or_else(|| invalid_binding("unknown trial evidence cannot be written"))?;
    let alpha = binding
        .alpha
        .as_ref()
        .ok_or_else(|| invalid_binding("unknown alpha evidence cannot be written"))?;
    let unknown = binding
        .alpha_unknown_origins
        .as_ref()
        .ok_or_else(|| invalid_binding("unknown alpha provenance cannot be written"))?;
    let confirmation = binding
        .confirmation
        .as_ref()
        .ok_or_else(|| invalid_binding("unknown confirmation evidence cannot be written"))?;
    let sections: serde_json::Map<String, Value> = SECTIONS
        .iter()
        .enumerate()
        .map(|(n, (_, name))| (name.to_string(), serde_json::json!(confirmation.sets[n])))
        .collect();
    let raw = serde_json::json!({
        "version": LEDGER_BINDING_VERSION,
        "registryId": binding.registry_id,
        "seq": binding.seq,
        "chainHead": binding.chain_head,
        "evidence": {
            "quarantinedFamilies": evidence.quarantined_families,
            "conflictedOrigins": evidence.conflicted_origins,
            "familyTests": evidence.family_tests,
        },
        "alpha": {
            "budgets": alpha.budgets,
            "reservations": alpha.reservations,
            "conflicts": alpha.conflicts,
        },
        "alphaUnknownOrigins": unknown,
        "confirmation": sections,
    })
    .to_string();
    if raw.len() > MAX_BYTES {
        return Err(invalid_binding("binding exceeds 8 MiB; never truncated"));
    }
    parse_binding_snapshot(&raw).map_err(|_| invalid_binding("invalid v5 binding evidence"))
}

pub(super) fn observe(
    tx: &Transaction<'_>,
    registry: &str,
) -> Result<ConfirmationEvidence, LedgerError> {
    let checked = super::super::confirmation::load_witnessed_in(tx, registry)?;
    let inventory = checked.inventory();
    let mut sets: [Vec<String>; 9] = Default::default();
    for (index, documents) in [
        (0, &inventory.batches),
        (1, &inventory.selections),
        (2, &inventory.operations),
        (3, &inventory.exposures),
        (4, &inventory.admissions),
        (5, &inventory.events),
        (7, &inventory.conflicts),
    ] {
        for raw in documents {
            // The original closed documents and their SQL projections were
            // already verified together; do not derive identities from counts.
            let value: Value = serde_json::from_str(raw)?;
            let id = value["id"]
                .as_str()
                .ok_or_else(|| invalid_binding("checked graph has no identity"))?;
            sets[index].push(id.to_owned());
        }
        sets[index].sort();
    }
    sets[6] = inventory
        .blobs
        .iter()
        .map(|b| b.sha256().to_owned())
        .collect();
    sets[6].sort();
    sets[8] = inventory.unknown_origins.clone();
    Ok(ConfirmationEvidence { sets })
}
