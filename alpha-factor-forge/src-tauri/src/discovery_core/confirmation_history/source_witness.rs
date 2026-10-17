//! Immutable format snapshots and source trial anchors. The opaque archive is
//! not validated here: neither shape nor a hash authenticates source facts.
use super::{admission, hex, ordered, BindingSnapshot, HistoricalPrefix, HistoryRecordError};
use crate::discovery_core::{
    confirmation_contracts::{validate_registered_source, RegisteredSource},
    precision::PRECISION_MAX_COUNT,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const SOURCE_WITNESS_VERSION: &str = "research-confirmation-source-witness-v1";
pub const MAX_SOURCE_WITNESS_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireWitness {
    contract_version: String,
    trial_export_text: String,
    registered_source: RegisteredSource,
}

/// Private fields and no unchecked deserialization. Only the outer wire shape,
/// source domains and canonical bytes are checked; backend reconstruction must
/// verify the complete nested trial snapshot before supplying graph host facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceWitnessSnapshot {
    raw: String,
    sha256: String,
    wire: WireWitness,
}
impl SourceWitnessSnapshot {
    pub fn text(&self) -> &str {
        &self.raw
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn trial_export_text(&self) -> &str {
        &self.wire.trial_export_text
    }
    pub fn source_claims(&self) -> &RegisteredSource {
        &self.wire.registered_source
    }
}
fn canonical<T: Serialize>(value: &T) -> Result<String, HistoryRecordError> {
    let mut value = serde_json::to_value(value).map_err(|_| HistoryRecordError::Blob)?;
    value.sort_all_objects();
    serde_json::to_string(&value).map_err(|_| HistoryRecordError::Blob)
}
fn check_wire(w: &WireWitness) -> Result<(), HistoryRecordError> {
    if w.contract_version != SOURCE_WITNESS_VERSION {
        return Err(HistoryRecordError::Version);
    }
    if w.trial_export_text.is_empty() || !w.trial_export_text.ends_with('\n') {
        return Err(HistoryRecordError::Blob);
    }
    if w.trial_export_text.len() > MAX_SOURCE_WITNESS_BYTES {
        return Err(HistoryRecordError::TooLarge);
    }
    validate_registered_source(&w.registered_source).map_err(|_| HistoryRecordError::Binding)
}
fn snapshot(raw: String, wire: WireWitness) -> SourceWitnessSnapshot {
    SourceWitnessSnapshot {
        sha256: hex::encode(Sha256::digest(raw.as_bytes())),
        raw,
        wire,
    }
}
pub fn parse_source_witness(raw: &str) -> Result<SourceWitnessSnapshot, HistoryRecordError> {
    if raw.len() > MAX_SOURCE_WITNESS_BYTES {
        return Err(HistoryRecordError::TooLarge);
    }
    // Decode original text first; a Value routing step would hide duplicates.
    let wire: WireWitness = serde_json::from_str(raw).map_err(|_| HistoryRecordError::Malformed)?;
    check_wire(&wire)?;
    if canonical(&wire)? != raw {
        return Err(HistoryRecordError::Malformed);
    }
    Ok(snapshot(raw.into(), wire))
}
pub fn freeze_source_witness(
    trial_export_text: &str,
    registered_source: &RegisteredSource,
) -> Result<SourceWitnessSnapshot, HistoryRecordError> {
    if trial_export_text.len() > MAX_SOURCE_WITNESS_BYTES {
        return Err(HistoryRecordError::TooLarge);
    }
    validate_registered_source(registered_source).map_err(|_| HistoryRecordError::Binding)?;
    let wire = WireWitness {
        contract_version: SOURCE_WITNESS_VERSION.into(),
        trial_export_text: trial_export_text.into(),
        registered_source: registered_source.clone(),
    };
    check_wire(&wire)?;
    let raw = canonical(&wire)?;
    if raw.len() > MAX_SOURCE_WITNESS_BYTES {
        return Err(HistoryRecordError::TooLarge);
    }
    Ok(snapshot(raw, wire))
}

/// Domain-checked host metadata, not source authentication or a current permit.
/// Alpha/confirmation state is outside a trial anchor. Original comparison is
/// exact even when a newer current registry could cover a smaller old HWM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTrialAnchor {
    prefix: HistoricalPrefix,
    quarantined_families: Vec<String>,
    conflicted_origins: Vec<String>,
    family_tests: BTreeMap<String, u64>,
}
impl SourceTrialAnchor {
    pub fn from_host_facts(
        prefix: HistoricalPrefix,
        quarantined_families: Vec<String>,
        conflicted_origins: Vec<String>,
        family_tests: BTreeMap<String, u64>,
    ) -> Result<Self, HistoryRecordError> {
        if !hex(&prefix.registry_id, 32)
            || prefix.seq > PRECISION_MAX_COUNT
            || !hex(&prefix.chain_head, 64)
            || (prefix.seq == 0 && prefix.chain_head != admission::genesis(&prefix.registry_id))
            || !ordered(&quarantined_families)
            || !quarantined_families.iter().all(|v| admission::family(v))
            || !ordered(&conflicted_origins)
            || !conflicted_origins.iter().all(|v| hex(v, 32))
            || !family_tests
                .iter()
                .all(|(k, v)| admission::family(k) && (1..=PRECISION_MAX_COUNT).contains(v))
        {
            return Err(HistoryRecordError::Binding);
        }
        let result = Self {
            prefix,
            quarantined_families,
            conflicted_origins,
            family_tests,
        };
        // Same 8 MiB bound as an original source/binding. This does not add a
        // family-count policy or require fabricated alpha/confirmation fields.
        let size = serde_json::to_vec(&serde_json::json!({
            "registryId":result.prefix.registry_id,"seq":result.prefix.seq,"chainHead":result.prefix.chain_head,
            "evidence":{"quarantinedFamilies":result.quarantined_families,"conflictedOrigins":result.conflicted_origins,"familyTests":result.family_tests}
        })).map_err(|_| HistoryRecordError::Blob)?.len();
        if size > MAX_SOURCE_WITNESS_BYTES {
            return Err(HistoryRecordError::TooLarge);
        }
        Ok(result)
    }
    pub fn prefix(&self) -> &HistoricalPrefix {
        &self.prefix
    }
    pub fn quarantined_families(&self) -> &[String] {
        &self.quarantined_families
    }
    pub fn conflicted_origins(&self) -> &[String] {
        &self.conflicted_origins
    }
    pub fn family_tests(&self) -> &BTreeMap<String, u64> {
        &self.family_tests
    }
    pub fn matches_binding(&self, binding: &BindingSnapshot) -> bool {
        self.prefix.registry_id == binding.registry_id()
            && self.prefix.seq == binding.seq()
            && self.prefix.chain_head == binding.chain_head()
            && self.quarantined_families == binding.quarantined_families()
            && self.conflicted_origins == binding.conflicted_origins()
            && &self.family_tests == binding.family_tests()
    }
}

#[cfg(test)]
mod tests;
