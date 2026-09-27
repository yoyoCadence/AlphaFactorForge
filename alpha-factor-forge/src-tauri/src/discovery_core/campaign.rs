//! P12d-1: immutable declarations for campaign feasibility, not admission.
//!
//! This pure boundary freezes caller declarations before any research result
//! is available. P12d-2 must verify their provenance, bind runtime inputs and
//! read the current ledger. A content hash is not proof of eligibility.

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    backtest::EXECUTION_CONTRACT_VERSION,
    identity::{canonical_bytes, DATASET_HASH_VERSION},
    market_foundation::{interval_ms, parse_instrument_id, MARKET_SNAPSHOT_VERSION},
    metrics::METRICS_CONTRACT_VERSION,
    precision::{parse_precision_plan, PRECISION_MAX_COUNT, RESEARCH_PRECISION_VERSION},
    split::SPLIT_CONTRACT_VERSION,
    walk_forward::{precheck_walk_forward, WALK_FORWARD_EVIDENCE_VERSION, WALK_FORWARD_VERSION},
};

pub const CAMPAIGN_DECLARATION_VERSION: &str = "research-campaign-declaration-v1";
pub const CAMPAIGN_MAX_INSTRUMENTS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("campaign declaration: {0}")]
pub struct CampaignError(pub String);

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Declaration {
    contract_version: String,
    contracts: Contracts,
    sampling: Sampling,
    instruments: Vec<Instrument>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Contracts {
    dataset_identity: String,
    market_snapshot: String,
    execution: String,
    metrics: String,
    split: String,
    precision: String,
    trial_ledger: String,
    walk_forward: String,
    walk_forward_evidence: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Sampling {
    alpha_ppm: u64,
    max_relative_standard_error_ppm: u64,
    bootstrap_samples: u64,
    max_bootstrap_samples: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Instrument {
    instrument_id: String,
    listed_at_ms: u64,
    // Required even when null: missing listing metadata must not silently
    // acquire the meaning "no delisting declared".
    #[serde(deserialize_with = "required_nullable")]
    delisted_at_ms: Option<u64>,
    snapshot_id: String,
    dataset_hash: String,
    interval: String,
    from_ms: u64,
    to_ms: u64,
    sample_policy: SamplePolicy,
}

fn required_nullable<'de, D: Deserializer<'de>>(de: D) -> Result<Option<u64>, D::Error> {
    Option::<u64>::deserialize(de)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SamplePolicy {
    minimum_total_bars: u64,
    minimum_train_bars: u64,
    fold_validation_bars: u64,
    fold_count: u64,
    rationale: String,
}

/// Constructible only by validation. No deserializer or mutable access is
/// exposed: restoring a declaration must pass through `freeze_campaign`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenCampaignDeclaration {
    campaign_id: String,
    document: Value,
}

impl FrozenCampaignDeclaration {
    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn document(&self) -> &Value {
        &self.document
    }
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), CampaignError> {
    if condition {
        Ok(())
    } else {
        Err(CampaignError(message.into()))
    }
}

fn hash_is_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Strictly validate a declaration, sort its instrument set, then hash a
/// domain-separated canonical identity encoding. Does not read a database,
/// candles, results, ledger counts, time, or randomness; writes nothing.
pub fn freeze_campaign(raw: &Value) -> Result<FrozenCampaignDeclaration, CampaignError> {
    let mut declaration: Declaration =
        serde_json::from_value(raw.clone()).map_err(|e| CampaignError(e.to_string()))?;
    require(
        declaration.contract_version == CAMPAIGN_DECLARATION_VERSION,
        "unsupported contractVersion",
    )?;
    let contracts = &declaration.contracts;
    for (name, value, expected) in [
        (
            "datasetIdentity",
            &contracts.dataset_identity,
            DATASET_HASH_VERSION,
        ),
        (
            "marketSnapshot",
            &contracts.market_snapshot,
            MARKET_SNAPSHOT_VERSION,
        ),
        (
            "execution",
            &contracts.execution,
            EXECUTION_CONTRACT_VERSION,
        ),
        ("metrics", &contracts.metrics, METRICS_CONTRACT_VERSION),
        ("split", &contracts.split, SPLIT_CONTRACT_VERSION),
        (
            "precision",
            &contracts.precision,
            RESEARCH_PRECISION_VERSION,
        ),
        // The SQLite ledger is outside the pure library boundary.
        ("trialLedger", &contracts.trial_ledger, "trial-ledger-v1"),
        ("walkForward", &contracts.walk_forward, WALK_FORWARD_VERSION),
        (
            "walkForwardEvidence",
            &contracts.walk_forward_evidence,
            WALK_FORWARD_EVIDENCE_VERSION,
        ),
    ] {
        require(
            value == expected,
            format!("contracts.{name} must be {expected}"),
        )?;
    }

    // Reuse P12a's domains/cross-field validation, with synthetic counts ONLY
    // to validate the sampling declaration. Never evaluate these counts or
    // return them: real counts can come only from the current trial ledger.
    let sampling = &declaration.sampling;
    parse_precision_plan(&json!({
        "contractVersion": RESEARCH_PRECISION_VERSION,
        "correction": "holm",
        "alphaPpm": sampling.alpha_ppm,
        "maxRelativeStandardErrorPpm": sampling.max_relative_standard_error_ppm,
        "bootstrapSamples": sampling.bootstrap_samples,
        "maxBootstrapSamples": sampling.max_bootstrap_samples,
        "priorTrials": 0, "plannedTrials": 1, "testsPerTrial": 1
    }))
    .map_err(|e| CampaignError(e.to_string()))?;

    require(
        (1..=CAMPAIGN_MAX_INSTRUMENTS).contains(&declaration.instruments.len()),
        format!("instruments must contain 1..={CAMPAIGN_MAX_INSTRUMENTS} entries"),
    )?;
    let mut seen = BTreeSet::new();
    for instrument in &declaration.instruments {
        require(
            parse_instrument_id(&instrument.instrument_id).is_ok(),
            "invalid instrumentId",
        )?;
        require(
            seen.insert(&instrument.instrument_id),
            "duplicate instrumentId",
        )?;
        require(
            interval_ms(&instrument.interval).is_some(),
            "invalid interval",
        )?;
        require(hash_is_valid(&instrument.snapshot_id), "invalid snapshotId")?;
        require(
            instrument
                .dataset_hash
                .strip_prefix("dataset-content-v2:")
                .is_some_and(hash_is_valid),
            "invalid datasetHash",
        )?;
        require(
            instrument.to_ms <= PRECISION_MAX_COUNT
                && instrument.listed_at_ms <= instrument.from_ms
                && instrument.from_ms <= instrument.to_ms,
            "range must use safe timestamps and start on/after listing",
        )?;
        if let Some(delisted) = instrument.delisted_at_ms {
            require(
                delisted <= PRECISION_MAX_COUNT && instrument.to_ms < delisted,
                "range must end before the exclusive delisting timestamp",
            )?;
        }
        let policy = &instrument.sample_policy;
        require(
            (1..=PRECISION_MAX_COUNT).contains(&policy.minimum_total_bars),
            "minimumTotalBars must be a positive safe integer",
        )?;
        require(
            !policy.rationale.trim().is_empty()
                && policy.rationale.trim() == policy.rationale
                && policy.rationale.len() <= 1024,
            "rationale must be nonblank, trimmed and at most 1024 UTF-8 bytes",
        )?;
        // Validate P12c's declaration domains only. Total length and embargo
        // are not known until authoritative dataset/candidate admission.
        let _ = precheck_walk_forward(&json!({
            "contractVersion": WALK_FORWARD_VERSION,
            "totalBars": 0, "embargoBars": 0,
            "minimumTrainBars": policy.minimum_train_bars,
            "foldValidationBars": policy.fold_validation_bars,
            "foldCount": policy.fold_count
        }))
        .map_err(|e| CampaignError(e.to_string()))?;
    }
    declaration
        .instruments
        .sort_by(|a, b| a.instrument_id.cmp(&b.instrument_id));
    let document = serde_json::to_value(declaration).map_err(|e| CampaignError(e.to_string()))?;
    let encoded = canonical_bytes(&document).map_err(|e| CampaignError(e.to_string()))?;
    let mut digest = Sha256::new();
    digest.update(CAMPAIGN_DECLARATION_VERSION.as_bytes());
    digest.update([0]);
    digest.update(encoded);
    Ok(FrozenCampaignDeclaration {
        campaign_id: hex::encode(digest.finalize()),
        document,
    })
}

#[cfg(test)]
mod tests;
