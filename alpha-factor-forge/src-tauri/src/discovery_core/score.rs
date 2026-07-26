//! `score-v1`: pure Rust parity port of the params-only branch of
//! `src/services/score.ts`.
//!
//! Per the runner Resolution D2, discovery v1 accepts params strategies only.
//! Blocks and code-mode expression complexity deliberately stay TypeScript-only.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::backtest::BacktestResult;

pub const SCORE_FORMULA_VERSION: &str = "score-v1";
pub const JS_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

// ---------- config ----------

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreCaps {
    pub cagr: f64,
    pub sortino: f64,
    pub calmar: f64,
    pub profit_factor: f64,
    pub consistency_sigma_scale: f64,
    pub complexity_units: f64,
    pub turnover: f64,
    pub data_mining_log10: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreWeights {
    pub cagr: f64,
    pub sortino: f64,
    pub calmar: f64,
    pub regime: f64,
    pub profit_factor: f64,
    pub consistency: f64,
    pub complexity: f64,
    pub turnover: f64,
    pub data_mining: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ScoreConfig {
    pub caps: ScoreCaps,
    pub weights: ScoreWeights,
}

pub const DEFAULT_SCORE_CONFIG: ScoreConfig = ScoreConfig {
    caps: ScoreCaps {
        cagr: 1.0,
        sortino: 5.0,
        calmar: 5.0,
        profit_factor: 3.0,
        consistency_sigma_scale: 10.0,
        complexity_units: 40.0,
        turnover: 0.1,
        data_mining_log10: 4.0,
    },
    weights: ScoreWeights {
        cagr: 1.0,
        sortino: 1.0,
        calmar: 1.0,
        regime: 0.0,
        profit_factor: 1.0,
        consistency: 1.0,
        complexity: 0.5,
        turnover: 0.5,
        data_mining: 1.0,
    },
};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScoreCapsOverride {
    pub cagr: Option<f64>,
    pub sortino: Option<f64>,
    pub calmar: Option<f64>,
    pub profit_factor: Option<f64>,
    pub consistency_sigma_scale: Option<f64>,
    pub complexity_units: Option<f64>,
    pub turnover: Option<f64>,
    pub data_mining_log10: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScoreWeightsOverride {
    pub cagr: Option<f64>,
    pub sortino: Option<f64>,
    pub calmar: Option<f64>,
    pub regime: Option<f64>,
    pub profit_factor: Option<f64>,
    pub consistency: Option<f64>,
    pub complexity: Option<f64>,
    pub turnover: Option<f64>,
    pub data_mining: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScoreConfigOverride {
    pub caps: Option<ScoreCapsOverride>,
    pub weights: Option<ScoreWeightsOverride>,
}

// ---------- input projections ----------

/// The params fields SCORE-001 actually reads for canonical complexity.
/// Its type cannot represent blocks/code candidates.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParamsStrategyProjection {
    pub entry_sig: String,
    pub exit_sig: String,
    pub sl_pct: f64,
    pub tp_pct: f64,
}

/// Read-only result projection. It carries no segment identity: the future
/// runner must construct it from Validation only. This module never executes
/// or selects Train/Test segments.
#[derive(Clone, Copy, Debug)]
pub struct ScoreCandidateView<'a> {
    pub cagr: f64,
    pub sortino: f64,
    pub calmar: f64,
    pub profit_factor: f64,
    pub turnover: f64,
    pub monthly_returns: &'a BTreeMap<String, f64>,
    pub closed_trade_count: usize,
    pub total_bars: usize,
}

impl<'a> From<&'a BacktestResult> for ScoreCandidateView<'a> {
    fn from(result: &'a BacktestResult) -> Self {
        Self {
            cagr: result.metrics.cagr,
            sortino: result.metrics.sortino,
            calmar: result.metrics.calmar,
            profit_factor: result.metrics.profit_factor,
            turnover: result.metrics.turnover,
            monthly_returns: &result.metrics.monthly_returns,
            closed_trade_count: result.trades.len(),
            total_bars: result.equity.len(),
        }
    }
}

pub struct ScoreCandidateArgs<'a> {
    pub candidate: ScoreCandidateView<'a>,
    pub strategy: &'a ParamsStrategyProjection,
    /// Kept as f64 until validation to mirror Number.isSafeInteger failures.
    pub tested_combinations: f64,
    pub config: Option<&'a ScoreConfigOverride>,
}

// ---------- output ----------

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScoreComponentId {
    Cagr,
    Sortino,
    Calmar,
    Regime,
    ProfitFactor,
    Consistency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScorePenaltyId {
    Complexity,
    Turnover,
    DataMining,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RawStatus {
    Finite,
    PositiveInfinity,
    Insufficient,
    Invalid,
    Deferred,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyEvidence {
    pub month_count: usize,
    pub monthly_std_dev: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexityEvidence {
    pub decision_nodes: usize,
    pub indicator_params: usize,
    pub risk_rules: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnoverEvidence {
    pub proxy: &'static str,
    pub closed_trade_count: usize,
    pub total_bars: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DataMiningEvidence {
    pub n: u64,
    pub basis: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ScoreEvidence {
    Consistency(ConsistencyEvidence),
    Complexity(ComplexityEvidence),
    Turnover(TurnoverEvidence),
    DataMining(DataMiningEvidence),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreEntry<Id> {
    pub id: Id,
    pub raw: Option<f64>,
    pub raw_status: RawStatus,
    pub normalized: Option<f64>,
    pub weight: f64,
    pub contribution: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ScoreEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoreSegment {
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TestedCombinationsEvidence {
    pub n: u64,
    pub basis: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreBreakdown {
    pub formula_version: &'static str,
    pub segment: ScoreSegment,
    pub score: f64,
    pub components: Vec<ScoreEntry<ScoreComponentId>>,
    pub penalties: Vec<ScoreEntry<ScorePenaltyId>>,
    pub config: ScoreConfig,
    pub tested_combinations: TestedCombinationsEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreError(pub String);

impl std::fmt::Display for ScoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ScoreError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComplexityUnits {
    pub units: usize,
    pub decision_nodes: usize,
    pub indicator_params: usize,
    pub risk_rules: usize,
}

// ---------- config resolution and validation ----------

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

fn resolve_config(overrides: Option<&ScoreConfigOverride>) -> ScoreConfig {
    let mut config = DEFAULT_SCORE_CONFIG;
    if let Some(overrides) = overrides {
        if let Some(caps) = overrides.caps {
            if let Some(value) = caps.cagr {
                config.caps.cagr = value;
            }
            if let Some(value) = caps.sortino {
                config.caps.sortino = value;
            }
            if let Some(value) = caps.calmar {
                config.caps.calmar = value;
            }
            if let Some(value) = caps.profit_factor {
                config.caps.profit_factor = value;
            }
            if let Some(value) = caps.consistency_sigma_scale {
                config.caps.consistency_sigma_scale = value;
            }
            if let Some(value) = caps.complexity_units {
                config.caps.complexity_units = value;
            }
            if let Some(value) = caps.turnover {
                config.caps.turnover = value;
            }
            if let Some(value) = caps.data_mining_log10 {
                config.caps.data_mining_log10 = value;
            }
        }
        if let Some(weights) = overrides.weights {
            if let Some(value) = weights.cagr {
                config.weights.cagr = value;
            }
            if let Some(value) = weights.sortino {
                config.weights.sortino = value;
            }
            if let Some(value) = weights.calmar {
                config.weights.calmar = value;
            }
            if let Some(value) = weights.regime {
                config.weights.regime = value;
            }
            if let Some(value) = weights.profit_factor {
                config.weights.profit_factor = value;
            }
            if let Some(value) = weights.consistency {
                config.weights.consistency = value;
            }
            if let Some(value) = weights.complexity {
                config.weights.complexity = value;
            }
            if let Some(value) = weights.turnover {
                config.weights.turnover = value;
            }
            if let Some(value) = weights.data_mining {
                config.weights.data_mining = value;
            }
        }
    }

    config.weights.cagr = canonical_zero(config.weights.cagr);
    config.weights.sortino = canonical_zero(config.weights.sortino);
    config.weights.calmar = canonical_zero(config.weights.calmar);
    config.weights.regime = canonical_zero(config.weights.regime);
    config.weights.profit_factor = canonical_zero(config.weights.profit_factor);
    config.weights.consistency = canonical_zero(config.weights.consistency);
    config.weights.complexity = canonical_zero(config.weights.complexity);
    config.weights.turnover = canonical_zero(config.weights.turnover);
    config.weights.data_mining = canonical_zero(config.weights.data_mining);
    config
}

fn validate_config(config: &ScoreConfig) -> Result<(), ScoreError> {
    let caps = [
        ("cagr", config.caps.cagr),
        ("sortino", config.caps.sortino),
        ("calmar", config.caps.calmar),
        ("profitFactor", config.caps.profit_factor),
        ("consistencySigmaScale", config.caps.consistency_sigma_scale),
        ("complexityUnits", config.caps.complexity_units),
        ("turnover", config.caps.turnover),
        ("dataMiningLog10", config.caps.data_mining_log10),
    ];
    for (name, value) in caps {
        if !value.is_finite() || value <= 0.0 {
            return Err(ScoreError(format!("cap {name} must be finite and > 0")));
        }
    }
    if config.caps.profit_factor <= 1.0 {
        return Err(ScoreError(
            "cap profitFactor must be > 1 (1 is the break-even floor)".to_string(),
        ));
    }

    let weights = [
        ("cagr", config.weights.cagr),
        ("sortino", config.weights.sortino),
        ("calmar", config.weights.calmar),
        ("regime", config.weights.regime),
        ("profitFactor", config.weights.profit_factor),
        ("consistency", config.weights.consistency),
        ("complexity", config.weights.complexity),
        ("turnover", config.weights.turnover),
        ("dataMining", config.weights.data_mining),
    ];
    for (name, value) in weights {
        if !value.is_finite() || value < 0.0 {
            return Err(ScoreError(format!("weight {name} must be finite and >= 0")));
        }
    }
    if config.weights.regime != 0.0 {
        return Err(ScoreError(
            "regime weight must stay 0 until REGIME-001 implements the regime classifier"
                .to_string(),
        ));
    }
    Ok(())
}

fn tested_combinations(value: f64) -> Result<u64, ScoreError> {
    if !value.is_finite() || value.fract() != 0.0 || value < 1.0 || value > JS_MAX_SAFE_INTEGER {
        return Err(ScoreError(
            "testedCombinations must be a positive safe integer (pass 1 for manual one-offs)"
                .to_string(),
        ));
    }
    Ok(value as u64)
}

// ---------- normalization ----------

fn clamp01(value: f64) -> f64 {
    if value <= 0.0 {
        0.0
    } else if value >= 1.0 {
        1.0
    } else {
        value
    }
}

fn ratio_entry<Id, Normalize>(id: Id, raw: f64, weight: f64, normalize: Normalize) -> ScoreEntry<Id>
where
    Normalize: FnOnce(f64) -> f64,
{
    if raw.is_nan() || raw == f64::NEG_INFINITY {
        return ScoreEntry {
            id,
            raw: None,
            raw_status: RawStatus::Invalid,
            normalized: Some(0.0),
            weight,
            contribution: 0.0,
            evidence: None,
        };
    }
    if raw == f64::INFINITY {
        return ScoreEntry {
            id,
            raw: None,
            raw_status: RawStatus::PositiveInfinity,
            normalized: Some(1.0),
            weight,
            contribution: weight,
            evidence: None,
        };
    }
    let finite_raw = canonical_zero(raw);
    let normalized = clamp01(normalize(finite_raw));
    ScoreEntry {
        id,
        raw: Some(finite_raw),
        raw_status: RawStatus::Finite,
        normalized: Some(normalized),
        weight,
        contribution: weight * normalized,
        evidence: None,
    }
}

fn consistency_entry(
    monthly_returns: &BTreeMap<String, f64>,
    weight: f64,
    sigma_scale: f64,
) -> ScoreEntry<ScoreComponentId> {
    let months: Vec<f64> = monthly_returns
        .values()
        .copied()
        .filter(|value| value.is_finite())
        .collect();
    if months.len() < 3 {
        return ScoreEntry {
            id: ScoreComponentId::Consistency,
            raw: None,
            raw_status: RawStatus::Insufficient,
            normalized: Some(0.0),
            weight,
            contribution: 0.0,
            evidence: Some(ScoreEvidence::Consistency(ConsistencyEvidence {
                month_count: months.len(),
                monthly_std_dev: None,
            })),
        };
    }

    let scale = months
        .iter()
        .fold(0.0f64, |largest, value| largest.max(value.abs()));
    let scaled: Vec<f64> = if scale == 0.0 {
        months
    } else {
        months.iter().map(|value| value / scale).collect()
    };
    let mean = scaled.iter().fold(0.0, |sum, value| sum + value) / scaled.len() as f64;
    let variance = scaled.iter().fold(0.0, |sum, value| {
        let difference = value - mean;
        sum + difference * difference
    }) / scaled.len() as f64;
    let scaled_sigma = variance.sqrt();
    let sigma = canonical_zero(scaled_sigma * if scale == 0.0 { 1.0 } else { scale });
    if !mean.is_finite() || !variance.is_finite() || !scaled_sigma.is_finite() || !sigma.is_finite()
    {
        return ScoreEntry {
            id: ScoreComponentId::Consistency,
            raw: None,
            raw_status: RawStatus::Invalid,
            normalized: Some(0.0),
            weight,
            contribution: 0.0,
            evidence: Some(ScoreEvidence::Consistency(ConsistencyEvidence {
                month_count: scaled.len(),
                monthly_std_dev: None,
            })),
        };
    }

    let normalized = 1.0 / (1.0 + sigma_scale * sigma);
    ScoreEntry {
        id: ScoreComponentId::Consistency,
        raw: Some(sigma),
        raw_status: RawStatus::Finite,
        normalized: Some(normalized),
        weight,
        contribution: weight * normalized,
        evidence: Some(ScoreEvidence::Consistency(ConsistencyEvidence {
            month_count: scaled.len(),
            monthly_std_dev: Some(sigma),
        })),
    }
}

// ---------- params-only complexity ----------

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum IndicatorParameter {
    FastMa,
    SlowMa,
    EmaPeriod,
    RsiPeriod,
    RsiBuy,
    RsiSell,
    MacdFast,
    MacdSlow,
    MacdSignal,
    BbPeriod,
    BbMult,
}

fn add_signal_parameters(
    id: &str,
    parameters: &mut BTreeSet<IndicatorParameter>,
) -> Result<(), ScoreError> {
    match id {
        "maCrossUp" | "maCrossDown" => {
            parameters.insert(IndicatorParameter::FastMa);
            parameters.insert(IndicatorParameter::SlowMa);
        }
        "emaCrossUp" | "emaCrossDown" => {
            parameters.insert(IndicatorParameter::EmaPeriod);
        }
        "priceAboveSlow" | "priceBelowSlow" => {
            parameters.insert(IndicatorParameter::SlowMa);
        }
        "rsiOversold" => {
            parameters.insert(IndicatorParameter::RsiPeriod);
            parameters.insert(IndicatorParameter::RsiBuy);
        }
        "rsiOverbought" => {
            parameters.insert(IndicatorParameter::RsiPeriod);
            parameters.insert(IndicatorParameter::RsiSell);
        }
        "macdCrossUp" | "macdCrossDown" => {
            parameters.insert(IndicatorParameter::MacdFast);
            parameters.insert(IndicatorParameter::MacdSlow);
            parameters.insert(IndicatorParameter::MacdSignal);
        }
        "bbLowerTouch" | "bbUpperTouch" => {
            parameters.insert(IndicatorParameter::BbPeriod);
            parameters.insert(IndicatorParameter::BbMult);
        }
        unsupported => {
            return Err(ScoreError(format!(
                "unsupported signal \"{unsupported}\": stoch* signals await a core STOCH indicator (Phase B)"
            )));
        }
    }
    Ok(())
}

pub fn complexity_units(
    strategy: &ParamsStrategyProjection,
) -> Result<ComplexityUnits, ScoreError> {
    let mut parameters = BTreeSet::new();
    add_signal_parameters(&strategy.entry_sig, &mut parameters)?;
    add_signal_parameters(&strategy.exit_sig, &mut parameters)?;
    let decision_nodes = 6;
    let risk_rules = usize::from(strategy.sl_pct > 0.0) + usize::from(strategy.tp_pct > 0.0);
    Ok(ComplexityUnits {
        units: decision_nodes + parameters.len() + risk_rules,
        decision_nodes,
        indicator_params: parameters.len(),
        risk_rules,
    })
}

// ---------- scoring ----------

pub fn score_candidate(args: &ScoreCandidateArgs<'_>) -> Result<ScoreBreakdown, ScoreError> {
    let config = resolve_config(args.config);
    validate_config(&config)?;
    let n = tested_combinations(args.tested_combinations)?;
    let candidate = args.candidate;
    let caps = config.caps;
    let weights = config.weights;

    let components = vec![
        ratio_entry(
            ScoreComponentId::Cagr,
            candidate.cagr,
            weights.cagr,
            |value| value / caps.cagr,
        ),
        ratio_entry(
            ScoreComponentId::Sortino,
            candidate.sortino,
            weights.sortino,
            |value| value / caps.sortino,
        ),
        ratio_entry(
            ScoreComponentId::Calmar,
            candidate.calmar,
            weights.calmar,
            |value| value / caps.calmar,
        ),
        ScoreEntry {
            id: ScoreComponentId::Regime,
            raw: None,
            raw_status: RawStatus::Deferred,
            normalized: None,
            weight: weights.regime,
            contribution: 0.0,
            evidence: None,
        },
        ratio_entry(
            ScoreComponentId::ProfitFactor,
            candidate.profit_factor,
            weights.profit_factor,
            |value| (value - 1.0) / (caps.profit_factor - 1.0),
        ),
        consistency_entry(
            candidate.monthly_returns,
            weights.consistency,
            caps.consistency_sigma_scale,
        ),
    ];

    let complexity = complexity_units(args.strategy)?;
    let complexity_normalized = clamp01(complexity.units as f64 / caps.complexity_units);
    let complexity_entry = ScoreEntry {
        id: ScorePenaltyId::Complexity,
        raw: Some(complexity.units as f64),
        raw_status: RawStatus::Finite,
        normalized: Some(complexity_normalized),
        weight: weights.complexity,
        contribution: weights.complexity * complexity_normalized,
        evidence: Some(ScoreEvidence::Complexity(ComplexityEvidence {
            decision_nodes: complexity.decision_nodes,
            indicator_params: complexity.indicator_params,
            risk_rules: complexity.risk_rules,
        })),
    };

    let mut turnover_entry = ratio_entry(
        ScorePenaltyId::Turnover,
        candidate.turnover,
        weights.turnover,
        |value| value / caps.turnover,
    );
    turnover_entry.evidence = Some(ScoreEvidence::Turnover(TurnoverEvidence {
        proxy: "closedTrades/totalBars@v1",
        closed_trade_count: candidate.closed_trade_count,
        total_bars: candidate.total_bars,
    }));

    let data_mining_normalized = clamp01((n as f64).log10() / caps.data_mining_log10);
    let data_mining_entry = ScoreEntry {
        id: ScorePenaltyId::DataMining,
        raw: Some(n as f64),
        raw_status: RawStatus::Finite,
        normalized: Some(data_mining_normalized),
        weight: weights.data_mining,
        contribution: weights.data_mining * data_mining_normalized,
        evidence: Some(ScoreEvidence::DataMining(DataMiningEvidence {
            n,
            basis: "lineage-final-unique",
        })),
    };
    let penalties = vec![complexity_entry, turnover_entry, data_mining_entry];

    let positive = components
        .iter()
        .fold(0.0, |sum, component| sum + component.contribution);
    let penalty = penalties
        .iter()
        .fold(0.0, |sum, entry| sum + entry.contribution);
    let score = positive - penalty;
    if !positive.is_finite() || !penalty.is_finite() || !score.is_finite() {
        return Err(ScoreError(
            "resolved score weights produce a non-finite score".to_string(),
        ));
    }

    Ok(ScoreBreakdown {
        formula_version: SCORE_FORMULA_VERSION,
        segment: ScoreSegment::Validation,
        score: canonical_zero(score),
        components,
        penalties,
        config,
        tested_combinations: TestedCombinationsEvidence {
            n,
            basis: "lineage-final-unique",
        },
    })
}
