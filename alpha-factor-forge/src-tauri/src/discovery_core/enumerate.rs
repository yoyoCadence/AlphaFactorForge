//! `discovery-enumeration-v1`: deterministic params-only candidate
//! enumeration, ported from `src/services/candidateEnumeration.ts`.
//!
//! The plan is the run's lineage: it fixes which hypotheses exist, Score's
//! data-mining `N`, and every candidate's stable index. It therefore depends
//! only on config content — never on map iteration, thread scheduling,
//! completion order, or a SQLite row id.
//!
//! Pure: no Tauri, rusqlite, threads, events, UI, or Test-segment execution.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Number, Value};

use super::config::{
    axis_values, ConfigError, DiscoveryBase, ResolvedDiscoveryConfig, DISCOVERY_ENUMERATION_VERSION,
};
use super::identity::strategy_hash;
use super::seed::{derive_discovery_seed, DeriveSeedArgs};

/// Cross-field validity rules, in the reference's fixed order.
pub const DISCOVERY_VALIDITY_RULE_IDS: [&str; 3] =
    ["fastMA<slowMA", "macdFast<macdSlow", "rsiBuy<rsiSell"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumerateError(pub String);

impl std::fmt::Display for EnumerateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for EnumerateError {}

impl From<ConfigError> for EnumerateError {
    fn from(error: ConfigError) -> Self {
        EnumerateError(error.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumerationCounts {
    pub raw: i64,
    pub pruned_invalid: i64,
    pub duplicates: i64,
    pub final_unique: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateSeeds {
    pub random_entry: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumeratedCandidate {
    pub index: i64,
    pub strategy_hash: String,
    pub base_id: String,
    pub applied_axes: BTreeMap<String, f64>,
    pub strategy: Value,
    pub seeds: CandidateSeeds,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestedCombinations {
    pub n: i64,
    pub basis: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatePlan {
    pub contract_version: String,
    pub dataset_content_hash: String,
    pub root_seed: u32,
    pub counts: EnumerationCounts,
    pub candidates: Vec<EnumeratedCandidate>,
    pub tested_combinations: TestedCombinations,
}

fn number_field(strategy: &Value, key: &str) -> f64 {
    strategy
        .get(key)
        .and_then(Value::as_f64)
        .unwrap_or(f64::NAN)
}

/// First violated rule id in fixed order, or `None` for a valid hypothesis.
///
/// The negated comparisons are deliberate and mirror the TypeScript reference:
/// a non-finite field must be treated as INVALID (pruned). `a >= b` is false
/// for NaN and would admit such a combination as a valid hypothesis.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn candidate_validity(strategy: &Value) -> Option<&'static str> {
    if !(number_field(strategy, "fastMA") < number_field(strategy, "slowMA")) {
        return Some(DISCOVERY_VALIDITY_RULE_IDS[0]);
    }
    if !(number_field(strategy, "macdFast") < number_field(strategy, "macdSlow")) {
        return Some(DISCOVERY_VALIDITY_RULE_IDS[1]);
    }
    if !(number_field(strategy, "rsiBuy") < number_field(strategy, "rsiSell")) {
        return Some(DISCOVERY_VALIDITY_RULE_IDS[2]);
    }
    None
}

#[derive(Clone, Debug)]
struct Combination {
    base_id: String,
    applied_axes: BTreeMap<String, f64>,
    strategy: Value,
}

fn patch(strategy: &Value, key: &str, value: f64) -> Result<Value, EnumerateError> {
    let mut object: Map<String, Value> = strategy
        .as_object()
        .cloned()
        .ok_or_else(|| EnumerateError("candidate strategy must be an object".into()))?;
    let number = Number::from_f64(value)
        .ok_or_else(|| EnumerateError(format!("axis value for {key} must be finite")))?;
    object.insert(key.to_string(), Value::Number(number));
    Ok(Value::Object(object))
}

/// Row-major odometer over declared axes: the LAST axis varies fastest.
fn combinations_for_base(base: &DiscoveryBase) -> Result<Vec<Combination>, EnumerateError> {
    let mut combinations = vec![Combination {
        base_id: base.id.clone(),
        applied_axes: BTreeMap::new(),
        strategy: base.strategy.clone(),
    }];
    for axis in &base.axes {
        let values = axis_values(axis)?;
        let mut next = Vec::with_capacity(combinations.len() * values.len());
        for combination in &combinations {
            for value in &values {
                let mut applied_axes = combination.applied_axes.clone();
                applied_axes.insert(axis.key.to_string(), *value);
                next.push(Combination {
                    base_id: combination.base_id.clone(),
                    applied_axes,
                    strategy: patch(&combination.strategy, axis.key, *value)?,
                });
            }
        }
        combinations = next;
    }
    Ok(combinations)
}

/// Raw Cartesian product across every base, with an explicit safe-integer
/// guard so an over-cap grid cannot overflow past the cap check.
pub fn raw_combination_count(bases: &[DiscoveryBase]) -> Result<i64, EnumerateError> {
    let mut total: f64 = 0.0;
    const JS_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
    for base in bases {
        let mut product: f64 = 1.0;
        for axis in &base.axes {
            product *= axis_values(axis)?.len() as f64;
            if product > JS_MAX_SAFE_INTEGER {
                return Err(EnumerateError(format!(
                    "base \"{}\" raw combination count is not a safe integer",
                    base.id
                )));
            }
        }
        total += product;
        if total > JS_MAX_SAFE_INTEGER {
            return Err(EnumerateError(
                "raw combination count is not a safe integer".into(),
            ));
        }
    }
    Ok(total as i64)
}

/// Enumerate, prune, deduplicate, hash-sort, index, and seed every candidate.
pub fn enumerate_candidates(
    config: &ResolvedDiscoveryConfig,
) -> Result<CandidatePlan, EnumerateError> {
    let raw = raw_combination_count(&config.bases)?;
    if raw > config.caps.candidates {
        return Err(EnumerateError(format!(
            "raw combination count {raw} exceeds the candidate cap {}",
            config.caps.candidates
        )));
    }

    let mut pruned_invalid: i64 = 0;
    let mut duplicates: i64 = 0;
    // BTreeMap keeps hash order without a separate sort; `strategy-v2` ids are
    // ASCII, so byte order matches the reference's string comparison.
    let mut by_hash: BTreeMap<String, Combination> = BTreeMap::new();

    for base in &config.bases {
        for combination in combinations_for_base(base)? {
            if candidate_validity(&combination.strategy).is_some() {
                pruned_invalid += 1;
                continue;
            }
            let fee_pct = number_field(&combination.strategy, "feePct");
            let slip_pct = number_field(&combination.strategy, "slipPct");
            let hash = strategy_hash(&combination.strategy, fee_pct, slip_pct)
                .map_err(|error| EnumerateError(error.0))?;
            if by_hash.contains_key(&hash) {
                duplicates += 1;
                continue;
            }
            by_hash.insert(hash, combination);
        }
    }

    if by_hash.is_empty() {
        return Err(EnumerateError(
            "enumeration produced no valid candidates".into(),
        ));
    }

    let mut candidates = Vec::with_capacity(by_hash.len());
    for (index, (hash, combination)) in by_hash.into_iter().enumerate() {
        let random_entry = derive_discovery_seed(&DeriveSeedArgs {
            root_seed: f64::from(config.root_seed),
            dataset_content_hash: &config.dataset.content_hash,
            strategy_hash: &hash,
            purpose: "random-entry",
        })
        .map_err(|error| EnumerateError(error.0))?;
        candidates.push(EnumeratedCandidate {
            index: index as i64,
            strategy_hash: hash,
            base_id: combination.base_id,
            applied_axes: combination.applied_axes,
            strategy: combination.strategy,
            seeds: CandidateSeeds { random_entry },
        });
    }

    let final_unique = candidates.len() as i64;
    if pruned_invalid + duplicates + final_unique != raw {
        return Err(EnumerateError(
            "enumeration counters do not reconcile with the raw product".into(),
        ));
    }

    Ok(CandidatePlan {
        contract_version: DISCOVERY_ENUMERATION_VERSION.to_string(),
        dataset_content_hash: config.dataset.content_hash.clone(),
        root_seed: config.root_seed,
        counts: EnumerationCounts {
            raw,
            pruned_invalid,
            duplicates,
            final_unique,
        },
        candidates,
        tested_combinations: TestedCombinations {
            n: final_unique,
            basis: "lineage-final-unique".to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validity_rules_apply_in_the_fixed_order() {
        let strategy = serde_json::json!({
            "fastMA": 9, "slowMA": 21,
            "macdFast": 12, "macdSlow": 26,
            "rsiBuy": 30, "rsiSell": 70
        });
        assert_eq!(candidate_validity(&strategy), None);

        let mut both = strategy.clone();
        both["fastMA"] = serde_json::json!(21);
        both["rsiBuy"] = serde_json::json!(70);
        assert_eq!(candidate_validity(&both), Some("fastMA<slowMA"));

        let mut macd = strategy.clone();
        macd["macdFast"] = serde_json::json!(26);
        assert_eq!(candidate_validity(&macd), Some("macdFast<macdSlow"));
    }
}
