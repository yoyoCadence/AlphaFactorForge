//! Bounded consistency selectors from original roots, never source authority.
//! Parsed numeric values use the existing legacy codec; original bytes survive.

use std::io::{self, Write};

use alpha_factor_forge::discovery_core::{
    campaign::{freeze_campaign, CAMPAIGN_DECLARATION_VERSION},
    config::{axis_values, parse_discovery_config, ResolvedDiscoveryConfig},
    enumerate::enumerate_candidates,
    seed::is_durable_identity,
};
use serde::Serialize;
use serde_json::Value;

use super::{hash, invalid, RootRowsObservation, MAX_TOTAL_JSON_NODES, MAX_TOTAL_TEXT_BYTES};
use crate::error::AppResult;

const MAX_CANDIDATES: usize = 4096;
const JS_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

#[derive(Clone, Eq, PartialEq)]
pub(super) struct CandidateKey {
    index: i64,
    strategy_hash: String,
}

impl CandidateKey {
    pub(super) fn index(&self) -> i64 {
        self.index
    }
    pub(super) fn strategy_hash(&self) -> &str {
        &self.strategy_hash
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct SourceSelection {
    snapshot_id: String,
    dataset_id: i64,
    dataset_hash: String,
    interval: String,
    candidates: Vec<CandidateKey>,
}

impl SourceSelection {
    pub(super) fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }
    pub(super) fn dataset_id(&self) -> i64 {
        self.dataset_id
    }
    pub(super) fn dataset_hash(&self) -> &str {
        &self.dataset_hash
    }
    pub(super) fn interval(&self) -> &str {
        &self.interval
    }
    pub(super) fn candidates(&self) -> &[CandidateKey] {
        &self.candidates
    }
}

fn string<'a>(value: &'a Value, key: &str) -> AppResult<&'a str> {
    value.get(key).and_then(Value::as_str).ok_or_else(invalid)
}

/// A deterministic validation input, not the current/historical CPU count.
fn parser_cores(raw: &Value) -> AppResult<f64> {
    match raw.get("maxConcurrency") {
        Some(Value::Null) => Ok(1.0),
        Some(Value::Number(number)) => {
            let cores = number.as_f64().ok_or_else(invalid)?;
            if !cores.is_finite()
                || cores.fract() != 0.0
                || !(1.0..=JS_MAX_SAFE_INTEGER).contains(&cores)
            {
                return Err(invalid());
            }
            Ok(cores)
        }
        _ => Err(invalid()),
    }
}

#[derive(Clone, Eq, PartialEq)]
struct ScratchUsage {
    bytes: usize,
    nodes: usize,
}

impl ScratchUsage {
    fn add_product(&mut self, product: usize, bytes: usize, nodes: usize) -> AppResult<()> {
        let bytes = product
            .checked_mul(bytes)
            .and_then(|v| self.bytes.checked_add(v))
            .ok_or_else(invalid)?;
        let nodes = product
            .checked_mul(nodes)
            .and_then(|v| self.nodes.checked_add(v))
            .ok_or_else(invalid)?;
        if bytes > MAX_TOTAL_TEXT_BYTES || nodes > MAX_TOTAL_JSON_NODES {
            return Err(invalid());
        }
        self.bytes = bytes;
        self.nodes = nodes;
        Ok(())
    }
}

// An in-memory counting sink: no output bytes, file handle or IO side effect.
struct ByteCount(usize);

impl Write for ByteCount {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let total = self
            .0
            .checked_add(bytes.len())
            .filter(|v| *v <= MAX_TOTAL_TEXT_BYTES)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "selection_size_invalid"))?;
        self.0 = total;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialized_len(value: &impl Serialize) -> AppResult<usize> {
    let mut count = ByteCount(0);
    serde_json::to_writer(&mut count, value).map_err(|_| invalid())?;
    Ok(count.0)
}

fn value_nodes(value: &Value) -> AppResult<usize> {
    let mut total = 1usize;
    match value {
        Value::Array(values) => {
            for child in values {
                total = total.checked_add(value_nodes(child)?).ok_or_else(invalid)?;
            }
        }
        Value::Object(values) => {
            for child in values.values() {
                total = total.checked_add(value_nodes(child)?).ok_or_else(invalid)?;
            }
        }
        _ => {}
    }
    if total > MAX_TOTAL_JSON_NODES {
        return Err(invalid());
    }
    Ok(total)
}

/// Bound the whole raw expansion before the existing odometer clones anything.
fn scratch_preflight(config: &ResolvedDiscoveryConfig) -> AppResult<ScratchUsage> {
    let cap = usize::try_from(config.caps.candidates).map_err(|_| invalid())?;
    if cap == 0 || cap > MAX_CANDIDATES {
        return Err(invalid());
    }
    let mut products = Vec::new();
    let mut raw = 0usize;
    // Refuse raw count first, even when pruning/deduplication would shrink it.
    for base in &config.bases {
        let mut product = 1usize;
        for axis in &base.axes {
            let count = axis_values(axis).map_err(|_| invalid())?.len();
            product = product.checked_mul(count).ok_or_else(invalid)?;
        }
        raw = raw.checked_add(product).ok_or_else(invalid)?;
        if raw > cap {
            return Err(invalid());
        }
        products.push(product);
    }
    if raw == 0 {
        return Err(invalid());
    }
    let mut used = ScratchUsage { bytes: 0, nodes: 0 };
    for (base, product) in config.bases.iter().zip(products) {
        let strategy_bytes = serialized_len(&base.strategy)?;
        let strategy_nodes = value_nodes(&base.strategy)?;
        let id_bytes = serialized_len(&base.id)?;
        let mut axes_bytes = 0usize;
        for axis in &base.axes {
            let key_bytes = serialized_len(&axis.key.as_str())?;
            let mut max_value_bytes = 0usize;
            for value in axis_values(axis).map_err(|_| invalid())? {
                // serde_json would serialize nonfinite f64 as null; refuse it.
                let number = serde_json::Number::from_f64(value).ok_or_else(invalid)?;
                max_value_bytes = max_value_bytes.max(serialized_len(&number)?);
            }
            axes_bytes = axes_bytes
                .checked_add(key_bytes)
                .and_then(|v| v.checked_add(max_value_bytes))
                .and_then(|v| v.checked_add(3))
                .ok_or_else(invalid)?;
        }
        let bytes = axes_bytes
            .checked_mul(2)
            .and_then(|v| v.checked_add(2))
            .and_then(|v| v.checked_add(strategy_bytes))
            .and_then(|v| v.checked_add(id_bytes))
            .ok_or_else(invalid)?;
        let nodes = base
            .axes
            .len()
            .checked_mul(2)
            .and_then(|v| v.checked_add(2))
            .and_then(|v| v.checked_add(strategy_nodes))
            .ok_or_else(invalid)?;
        used.add_product(product, bytes, nodes)?;
    }
    Ok(used)
}

#[cfg(test)]
thread_local! {
    static ENUM_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static DENY_ENUM: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) fn derive_selection(roots: &RootRowsObservation) -> AppResult<SourceSelection> {
    if roots.campaign.version != CAMPAIGN_DECLARATION_VERSION {
        return Err(invalid());
    }
    let declaration: Value =
        serde_json::from_slice(roots.campaign.document_json.as_bytes()).map_err(|_| invalid())?;
    let frozen = freeze_campaign(&declaration).map_err(|_| invalid())?;
    if frozen.campaign_id() != roots.campaign.campaign_id
        || frozen.campaign_id() != roots.admission.campaign_id
        || frozen.campaign_id() != roots.locator.campaign_id
    {
        return Err(invalid());
    }
    let bindings = frozen
        .document()
        .get("instruments")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    let binding = bindings
        .iter()
        .find(|binding| {
            binding.get("instrumentId").and_then(Value::as_str)
                == Some(roots.admission.instrument_id.as_str())
        })
        .ok_or_else(invalid)?;
    let snapshot_id = string(binding, "snapshotId")?;
    let dataset_hash = string(binding, "datasetHash")?;
    let interval = string(binding, "interval")?;
    if !hash(snapshot_id) || interval.is_empty() || interval.len() > 4096 {
        return Err(invalid());
    }
    let raw: Value =
        serde_json::from_slice(roots.run.config_json.as_bytes()).map_err(|_| invalid())?;
    let config = parse_discovery_config(&raw, parser_cores(&raw)?).map_err(|_| invalid())?;
    if config.dataset.content_hash != dataset_hash {
        return Err(invalid());
    }
    scratch_preflight(&config)?;
    #[cfg(test)]
    {
        ENUM_ENTRIES.with(|value| value.set(value.get() + 1));
        assert!(
            !DENY_ENUM.with(std::cell::Cell::get),
            "Cartesian enumeration must not run before resource refusal"
        );
    }
    let plan = enumerate_candidates(&config).map_err(|_| invalid())?;
    let total = plan
        .counts
        .pruned_invalid
        .checked_add(plan.counts.duplicates)
        .and_then(|v| v.checked_add(plan.counts.final_unique));
    if plan.candidates.is_empty()
        || plan.candidates.len() > MAX_CANDIDATES
        || usize::try_from(plan.counts.final_unique).ok() != Some(plan.candidates.len())
        || plan.counts.raw <= 0
        || plan.counts.raw > config.caps.candidates
        || plan.counts.pruned_invalid < 0
        || plan.counts.duplicates < 0
        || total != Some(plan.counts.raw)
        || plan.tested_combinations.n != plan.counts.final_unique
        || plan.tested_combinations.basis != "lineage-final-unique"
        || plan.dataset_content_hash != dataset_hash
        || plan.root_seed != config.root_seed
        || plan.contract_version != config.contracts.enumeration
    {
        return Err(invalid());
    }
    let mut candidates = Vec::new();
    for (index, candidate) in plan.candidates.into_iter().enumerate() {
        if candidate.index != index as i64
            || !is_durable_identity(&candidate.strategy_hash, "strategy-v2")
        {
            return Err(invalid());
        }
        candidates.push(CandidateKey {
            index: candidate.index,
            strategy_hash: candidate.strategy_hash,
        });
    }
    Ok(SourceSelection {
        snapshot_id: snapshot_id.into(),
        dataset_id: config.dataset.id,
        dataset_hash: config.dataset.content_hash,
        interval: interval.into(),
        candidates,
    })
}

#[cfg(test)]
mod tests;
