//! `discovery-config-v1`: strict input parsing for a discovery run, ported
//! from `src/services/discoveryConfig.ts` (PR #66 Resolution D2 + D4).
//!
//! This is the run's only admission gate. Parsing walks the raw JSON by hand
//! instead of deriving `Deserialize`, because the reference implementation's
//! path-qualified messages and its exact rejection ORDER are part of the
//! contract: an operator who fixes the first reported problem must see the
//! same next problem in both languages.
//!
//! Pure: no Tauri, rusqlite, threads, events, UI, or Test-segment execution.

use serde::Serialize;
use serde_json::{Map, Value};

use super::backtest::EXECUTION_CONTRACT_VERSION;
use super::benchmarks::BENCHMARK_CONTRACT_VERSION;
use super::embargo::EMBARGO_CONTRACT_VERSION;
use super::gate::{resolve_gate_config, GateConfig, GateConfigOverrides};
use super::identity::{DATASET_HASH_VERSION, STRATEGY_HASH_VERSION};
use super::metrics::METRICS_CONTRACT_VERSION;
use super::random_entry::RANDOM_ENTRY_CONTRACT_VERSION;
use super::score::{ScoreCaps, ScoreConfig, ScoreWeights, SCORE_FORMULA_VERSION};
use super::seed::DISCOVERY_SEED_VERSION;
use super::split::SPLIT_CONTRACT_VERSION;

pub const DISCOVERY_CONFIG_VERSION: &str = "discovery-config-v1";
pub const DISCOVERY_PRESET_VERSION: &str = "discovery-preset-v1";
pub const DISCOVERY_ENUMERATION_VERSION: &str = "discovery-enumeration-v1";

pub const DISCOVERY_DEFAULT_CANDIDATE_CAP: i64 = 256;
pub const DISCOVERY_HARD_CANDIDATE_CAP: i64 = 4096;
pub const DISCOVERY_MAX_AXIS_VALUES: usize = 64;

const JS_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const MAX_U32: i64 = 4_294_967_295;
/// `MAX_RANDOM_ENTRY_RUNS` from `src/services/randomEntry.ts`.
const MAX_RANDOM_ENTRY_RUNS: i64 = 1000;
const BASE_ID_PATTERN: &str = "^[a-z0-9][a-z0-9-]*$";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

fn fail<T>(message: String) -> Result<T, ConfigError> {
    Err(ConfigError(message))
}

// ---------- parameter domains ----------

#[derive(Clone, Copy, PartialEq)]
enum NumericDomain {
    Period,
    Level,
    Positive,
    /// Bounded at 100, not merely at 0: these legacy percent units are divided
    /// by 100 before the engine's normalized-fraction check, which rejects
    /// anything above 1. Admitting 101 would queue a run guaranteed to throw
    /// once a job executes.
    Percent,
    SizePercent,
}

/// Every numeric `ParamsStrategy` field with its domain, in declaration order.
const NUMERIC_PARAM_DOMAINS: [(&str, NumericDomain); 16] = [
    ("fastMA", NumericDomain::Period),
    ("slowMA", NumericDomain::Period),
    ("emaPeriod", NumericDomain::Period),
    ("rsiPeriod", NumericDomain::Period),
    ("rsiBuy", NumericDomain::Level),
    ("rsiSell", NumericDomain::Level),
    ("macdFast", NumericDomain::Period),
    ("macdSlow", NumericDomain::Period),
    ("macdSignal", NumericDomain::Period),
    ("bbPeriod", NumericDomain::Period),
    ("bbMult", NumericDomain::Positive),
    ("slPct", NumericDomain::Percent),
    ("tpPct", NumericDomain::Percent),
    ("feePct", NumericDomain::Percent),
    ("slipPct", NumericDomain::Percent),
    ("sizePct", NumericDomain::SizePercent),
];

/// Hypothesis axes only: `feePct`, `slipPct`, and `sizePct` are execution
/// model and are deliberately absent.
///
/// A closed enum, not a string: `&'static str` would only promise that the
/// text outlives the program — ANY literal satisfies it, so a caller could
/// still build an axis naming a non-whitelisted key. Illegal axis keys are
/// unrepresentable here, and `parse` is the only way in.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum AxisKey {
    #[serde(rename = "fastMA")]
    FastMa,
    #[serde(rename = "slowMA")]
    SlowMa,
    #[serde(rename = "emaPeriod")]
    EmaPeriod,
    #[serde(rename = "rsiPeriod")]
    RsiPeriod,
    #[serde(rename = "rsiBuy")]
    RsiBuy,
    #[serde(rename = "rsiSell")]
    RsiSell,
    #[serde(rename = "macdFast")]
    MacdFast,
    #[serde(rename = "macdSlow")]
    MacdSlow,
    #[serde(rename = "macdSignal")]
    MacdSignal,
    #[serde(rename = "bbPeriod")]
    BbPeriod,
    #[serde(rename = "bbMult")]
    BbMult,
    #[serde(rename = "slPct")]
    SlPct,
    #[serde(rename = "tpPct")]
    TpPct,
}

impl AxisKey {
    /// The whitelist itself, in the reference's declared order.
    pub const ALL: [AxisKey; 13] = [
        AxisKey::FastMa,
        AxisKey::SlowMa,
        AxisKey::EmaPeriod,
        AxisKey::RsiPeriod,
        AxisKey::RsiBuy,
        AxisKey::RsiSell,
        AxisKey::MacdFast,
        AxisKey::MacdSlow,
        AxisKey::MacdSignal,
        AxisKey::BbPeriod,
        AxisKey::BbMult,
        AxisKey::SlPct,
        AxisKey::TpPct,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AxisKey::FastMa => "fastMA",
            AxisKey::SlowMa => "slowMA",
            AxisKey::EmaPeriod => "emaPeriod",
            AxisKey::RsiPeriod => "rsiPeriod",
            AxisKey::RsiBuy => "rsiBuy",
            AxisKey::RsiSell => "rsiSell",
            AxisKey::MacdFast => "macdFast",
            AxisKey::MacdSlow => "macdSlow",
            AxisKey::MacdSignal => "macdSignal",
            AxisKey::BbPeriod => "bbPeriod",
            AxisKey::BbMult => "bbMult",
            AxisKey::SlPct => "slPct",
            AxisKey::TpPct => "tpPct",
        }
    }

    /// The ONLY constructor from untrusted text.
    pub fn parse(value: &str) -> Option<AxisKey> {
        AxisKey::ALL.into_iter().find(|key| key.as_str() == value)
    }
}

/// Whitelist as strings, derived from `AxisKey::ALL` so the two cannot drift.
pub fn discovery_axis_keys() -> [&'static str; 13] {
    let mut out = [""; 13];
    let mut index = 0;
    while index < AxisKey::ALL.len() {
        out[index] = AxisKey::ALL[index].as_str();
        index += 1;
    }
    out
}

pub const DISCOVERY_SUPPORTED_SIGNAL_IDS: [&str; 12] = [
    "maCrossUp",
    "maCrossDown",
    "emaCrossUp",
    "emaCrossDown",
    "priceAboveSlow",
    "priceBelowSlow",
    "rsiOversold",
    "rsiOverbought",
    "macdCrossUp",
    "macdCrossDown",
    "bbLowerTouch",
    "bbUpperTouch",
];

const FILL_MODES: [&str; 2] = ["close", "nextOpen"];
const DIRECTIONS: [&str; 3] = ["long", "short", "both"];

const STRATEGY_KEYS: [&str; 25] = [
    "mode",
    "fastMA",
    "slowMA",
    "emaPeriod",
    "rsiPeriod",
    "rsiBuy",
    "rsiSell",
    "macdFast",
    "macdSlow",
    "macdSignal",
    "bbPeriod",
    "bbMult",
    "entrySig",
    "exitSig",
    "entryRules",
    "exitRules",
    "entryCode",
    "exitCode",
    "slPct",
    "tpPct",
    "feePct",
    "slipPct",
    "sizePct",
    "fillMode",
    "direction",
];

const GATE_KEYS: [&str; 8] = [
    "minTrades",
    "minAvgTradeReturn",
    "rollingWindowBars",
    "minRollingPositiveRatio",
    "maxDrawdown",
    "maxMonthlyContribution",
    "maxSingleTradeContribution",
    "minRandomEntryPercentile",
];

const SCORE_CAP_KEYS: [&str; 8] = [
    "cagr",
    "sortino",
    "calmar",
    "profitFactor",
    "consistencySigmaScale",
    "complexityUnits",
    "turnover",
    "dataMiningLog10",
];

const SCORE_WEIGHT_KEYS: [&str; 9] = [
    "cagr",
    "sortino",
    "calmar",
    "regime",
    "profitFactor",
    "consistency",
    "complexity",
    "turnover",
    "dataMining",
];

const ENVELOPE_KEYS: [&str; 13] = [
    "envelopeVersion",
    "contracts",
    "dataset",
    "bases",
    "embargo",
    "execution",
    "benchmarkCosts",
    "randomEntry",
    "gateConfig",
    "scoreConfig",
    "rootSeed",
    "caps",
    "maxConcurrency",
];

// ---------- resolved shapes ----------

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryContractVersions {
    pub strategy_hash: String,
    pub dataset_hash: String,
    pub split: String,
    pub embargo: String,
    pub backtest: String,
    pub metrics: String,
    pub benchmarks: String,
    pub random_entry: String,
    pub gate: String,
    pub score: String,
    pub seed: String,
    pub enumeration: String,
}

pub fn discovery_contract_versions() -> DiscoveryContractVersions {
    DiscoveryContractVersions {
        strategy_hash: STRATEGY_HASH_VERSION.into(),
        dataset_hash: DATASET_HASH_VERSION.into(),
        split: SPLIT_CONTRACT_VERSION.into(),
        embargo: EMBARGO_CONTRACT_VERSION.into(),
        backtest: EXECUTION_CONTRACT_VERSION.into(),
        metrics: METRICS_CONTRACT_VERSION.into(),
        benchmarks: BENCHMARK_CONTRACT_VERSION.into(),
        random_entry: RANDOM_ENTRY_CONTRACT_VERSION.into(),
        gate: super::gate::GATE_CONTRACT_VERSION.into(),
        score: SCORE_FORMULA_VERSION.into(),
        seed: DISCOVERY_SEED_VERSION.into(),
        enumeration: DISCOVERY_ENUMERATION_VERSION.into(),
    }
}

fn contract_entries(versions: &DiscoveryContractVersions) -> Vec<(&'static str, &str)> {
    // Sorted by key, matching the reference's sorted iteration order.
    let mut entries: Vec<(&'static str, &str)> = vec![
        ("backtest", versions.backtest.as_str()),
        ("benchmarks", versions.benchmarks.as_str()),
        ("datasetHash", versions.dataset_hash.as_str()),
        ("embargo", versions.embargo.as_str()),
        ("enumeration", versions.enumeration.as_str()),
        ("gate", versions.gate.as_str()),
        ("metrics", versions.metrics.as_str()),
        ("randomEntry", versions.random_entry.as_str()),
        ("score", versions.score.as_str()),
        ("seed", versions.seed.as_str()),
        ("split", versions.split.as_str()),
        ("strategyHash", versions.strategy_hash.as_str()),
    ];
    entries.sort_by(|left, right| left.0.cmp(right.0));
    entries
}

/// One grid axis. This is the SINGLE representation: the audit view that gets
/// serialized into the run record and the value the enumerator walks are the
/// same field, so a recorded config can never describe a different grid from
/// the one that actually produced the candidates.
///
/// `key` is a closed `AxisKey` enum, so an axis naming a non-whitelisted key
/// cannot be constructed at all — not even by a caller inside this crate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryAxis {
    pub key: AxisKey,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryBase {
    pub id: String,
    pub preset_version: String,
    /// The validated strategy object, kept as JSON so it hashes byte-for-byte
    /// like the TypeScript reference's own object.
    pub strategy: Value,
    pub axes: Vec<DiscoveryAxis>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetRef {
    pub id: i64,
    pub content_hash: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbargoInput {
    pub holding_allowance_bars: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionInput {
    pub start_equity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkCostsInput {
    pub fee_pct: f64,
    pub slip_pct: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomEntryInput {
    pub runs: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapsInput {
    pub candidates: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedConcurrency {
    pub requested: Option<f64>,
    pub resolved: i64,
    pub logical_cores: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDiscoveryConfig {
    pub envelope_version: String,
    pub contracts: DiscoveryContractVersions,
    pub dataset: DatasetRef,
    pub bases: Vec<DiscoveryBase>,
    pub embargo: EmbargoInput,
    pub execution: ExecutionInput,
    pub benchmark_costs: BenchmarkCostsInput,
    pub random_entry: RandomEntryInput,
    pub gate_config: GateConfig,
    pub score_config: ScoreConfig,
    pub root_seed: u32,
    pub caps: CapsInput,
    pub concurrency: ResolvedConcurrency,
}

// ---------- strict readers ----------

fn require_object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ConfigError> {
    value
        .as_object()
        .ok_or_else(|| ConfigError(format!("{path} must be an object")))
}

fn require_exact_keys(
    object: &Map<String, Value>,
    path: &str,
    keys: &[&str],
) -> Result<(), ConfigError> {
    for key in keys {
        if !object.contains_key(*key) {
            return fail(format!("{path} is missing key \"{key}\""));
        }
    }
    // serde_json's default map is sorted, matching the reference's sorted scan.
    let mut present: Vec<&String> = object.keys().collect();
    present.sort();
    for key in present {
        if !keys.contains(&key.as_str()) {
            return fail(format!("{path} has unknown key \"{key}\""));
        }
    }
    Ok(())
}

fn require_number(object: &Map<String, Value>, path: &str, key: &str) -> Result<f64, ConfigError> {
    match object.get(key).and_then(Value::as_f64) {
        Some(value) if value.is_finite() => Ok(value),
        _ => fail(format!("{path}.{key} must be a finite number")),
    }
}

fn require_string<'a>(
    object: &'a Map<String, Value>,
    path: &str,
    key: &str,
) -> Result<&'a str, ConfigError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ConfigError(format!("{path}.{key} must be a string")))
}

fn require_array<'a>(
    object: &'a Map<String, Value>,
    path: &str,
    key: &str,
) -> Result<&'a Vec<Value>, ConfigError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| ConfigError(format!("{path}.{key} must be an array")))
}

fn require_literal<'a>(
    object: &'a Map<String, Value>,
    path: &str,
    key: &str,
    allowed: &[&str],
) -> Result<&'a str, ConfigError> {
    let value = require_string(object, path, key)?;
    if !allowed.contains(&value) {
        return fail(format!(
            "{path}.{key} must be one of {}",
            allowed.join(", ")
        ));
    }
    Ok(value)
}

fn is_safe_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0 && value.abs() <= JS_MAX_SAFE_INTEGER
}

fn require_integer_in_range(
    value: f64,
    path: &str,
    min: i64,
    max: i64,
) -> Result<i64, ConfigError> {
    if !is_safe_integer(value) || value < min as f64 || value > max as f64 {
        return fail(format!("{path} must be an integer in [{min}, {max}]"));
    }
    Ok(value as i64)
}

fn numeric_domain(key: &str) -> Option<NumericDomain> {
    NUMERIC_PARAM_DOMAINS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, domain)| *domain)
}

/// Mirrors `checkNumericParam`; returns the reference's problem text.
pub fn check_numeric_param(key: &str, value: f64) -> Option<String> {
    if !value.is_finite() {
        return Some(format!("{key} must be a finite number"));
    }
    match numeric_domain(key) {
        Some(NumericDomain::Period) => {
            if is_safe_integer(value) && value >= 1.0 {
                None
            } else {
                Some(format!("{key} must be an integer >= 1"))
            }
        }
        Some(NumericDomain::Level) => {
            if (0.0..=100.0).contains(&value) {
                None
            } else {
                Some(format!("{key} must be in [0, 100]"))
            }
        }
        Some(NumericDomain::Positive) => {
            if value > 0.0 {
                None
            } else {
                Some(format!("{key} must be > 0"))
            }
        }
        Some(NumericDomain::Percent) => {
            if (0.0..=100.0).contains(&value) {
                None
            } else {
                Some(format!("{key} must be in [0, 100]"))
            }
        }
        Some(NumericDomain::SizePercent) => {
            if value > 0.0 && value <= 100.0 {
                None
            } else {
                Some(format!("{key} must be in (0, 100]"))
            }
        }
        None => Some(format!("{key} is not a numeric strategy parameter")),
    }
}

// ---------- strategy preset ----------

fn parse_strategy(value: &Value, path: &str) -> Result<Value, ConfigError> {
    let object = require_object(value, path)?;
    require_exact_keys(object, path, &STRATEGY_KEYS)?;

    // Resolution D2: params only. Blocks and AI DSL are later phases; code
    // mode is permanently excluded from discovery.
    let mode = require_string(object, path, "mode")?;
    if mode != "params" {
        return fail(format!(
            "{path}.mode must be \"params\" (discovery v1 enumerates params-mode candidates only)"
        ));
    }

    for (key, _) in NUMERIC_PARAM_DOMAINS {
        let parsed = require_number(object, path, key)?;
        if let Some(problem) = check_numeric_param(key, parsed) {
            return fail(format!("{path}.{problem}"));
        }
    }

    require_literal(object, path, "entrySig", &DISCOVERY_SUPPORTED_SIGNAL_IDS)?;
    require_literal(object, path, "exitSig", &DISCOVERY_SUPPORTED_SIGNAL_IDS)?;
    require_literal(object, path, "fillMode", &FILL_MODES)?;
    require_literal(object, path, "direction", &DIRECTIONS)?;

    // Dormant in params mode but part of `strategy-v2` identity: present and
    // well-typed, contents never interpreted.
    require_array(object, path, "entryRules")?;
    require_array(object, path, "exitRules")?;
    require_string(object, path, "entryCode")?;
    require_string(object, path, "exitCode")?;

    Ok(value.clone())
}

// ---------- axes ----------

fn parse_axis(value: &Value, path: &str) -> Result<DiscoveryAxis, ConfigError> {
    let object = require_object(value, path)?;
    require_exact_keys(object, path, &["key", "min", "max", "step"])?;
    let key = require_literal(object, path, "key", &discovery_axis_keys())?;
    let key = AxisKey::parse(key).expect("axis key was whitelisted above");
    let min = require_number(object, path, "min")?;
    let max = require_number(object, path, "max")?;
    let step = require_number(object, path, "step")?;
    if step <= 0.0 {
        return fail(format!("{path}.step must be > 0"));
    }
    if max < min {
        return fail(format!("{path}.max must be >= min"));
    }
    if numeric_domain(key.as_str()) == Some(NumericDomain::Period) {
        for (name, bound) in [("min", min), ("max", max), ("step", step)] {
            if !is_safe_integer(bound) {
                let key = key.as_str();
                return fail(format!(
                    "{path}.{name} must be an integer for the integer axis \"{key}\""
                ));
            }
        }
    }
    Ok(DiscoveryAxis {
        key,
        min,
        max,
        step,
    })
}

/// Inclusive `min + i*step` values. Multiplication, never accumulation: the
/// TypeScript reference locks the exact binary drift this produces.
pub fn axis_values(axis: &DiscoveryAxis) -> Result<Vec<f64>, ConfigError> {
    let mut values = Vec::new();
    let mut i = 0u32;
    loop {
        let value = axis.min + f64::from(i) * axis.step;
        // Negated comparison is deliberate and mirrors the TypeScript
        // reference: a NaN `value` must TERMINATE the loop. `value > axis.max`
        // is false for NaN and would spin forever.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !(value <= axis.max) {
            break;
        }
        if values.len() >= DISCOVERY_MAX_AXIS_VALUES {
            return fail(format!(
                "axis \"{}\" produces more than {DISCOVERY_MAX_AXIS_VALUES} values",
                axis.key.as_str()
            ));
        }
        values.push(value);
        i += 1;
    }
    if values.is_empty() {
        return fail(format!("axis \"{}\" produces no values", axis.key.as_str()));
    }
    Ok(values)
}

fn is_valid_base_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    })
}

fn parse_base(value: &Value, path: &str) -> Result<DiscoveryBase, ConfigError> {
    let object = require_object(value, path)?;
    require_exact_keys(object, path, &["id", "presetVersion", "strategy", "axes"])?;
    let id = require_string(object, path, "id")?;
    if !is_valid_base_id(id) {
        return fail(format!("{path}.id must match {BASE_ID_PATTERN}"));
    }
    let preset_version = require_string(object, path, "presetVersion")?;
    if preset_version != DISCOVERY_PRESET_VERSION {
        return fail(format!(
            "{path}.presetVersion must be \"{DISCOVERY_PRESET_VERSION}\""
        ));
    }
    let strategy = parse_strategy(
        object.get("strategy").unwrap_or(&Value::Null),
        &format!("{path}.strategy"),
    )?;

    let raw_axes = require_array(object, path, "axes")?;
    let mut axes: Vec<DiscoveryAxis> = Vec::new();
    for (index, raw_axis) in raw_axes.iter().enumerate() {
        let axis_path = format!("{path}.axes[{index}]");
        let axis = parse_axis(raw_axis, &axis_path)?;
        if axes.iter().any(|seen| seen.key == axis.key) {
            return fail(format!(
                "{axis_path} repeats axis key \"{}\"",
                axis.key.as_str()
            ));
        }
        for generated in axis_values(&axis)? {
            if let Some(problem) = check_numeric_param(axis.key.as_str(), generated) {
                return fail(format!("{axis_path} generates an invalid value: {problem}"));
            }
        }
        axes.push(axis);
    }

    Ok(DiscoveryBase {
        id: id.to_string(),
        preset_version: DISCOVERY_PRESET_VERSION.to_string(),
        strategy,
        axes,
    })
}

// ---------- gate / score configs ----------

fn parse_gate_config(value: &Value, path: &str) -> Result<GateConfig, ConfigError> {
    let object = require_object(value, path)?;
    require_exact_keys(object, path, &GATE_KEYS)?;
    let overrides = GateConfigOverrides {
        min_trades: Some(require_number(object, path, "minTrades")?),
        min_avg_trade_return: Some(require_number(object, path, "minAvgTradeReturn")?),
        rolling_window_bars: Some(require_number(object, path, "rollingWindowBars")?),
        min_rolling_positive_ratio: Some(require_number(object, path, "minRollingPositiveRatio")?),
        max_drawdown: Some(require_number(object, path, "maxDrawdown")?),
        max_monthly_contribution: Some(require_number(object, path, "maxMonthlyContribution")?),
        max_single_trade_contribution: Some(require_number(
            object,
            path,
            "maxSingleTradeContribution",
        )?),
        min_random_entry_percentile: Some(require_number(
            object,
            path,
            "minRandomEntryPercentile",
        )?),
    };
    // The Gate module owns these messages; reuse it so admission and judgment
    // can never disagree about what a legal threshold is.
    resolve_gate_config(Some(&overrides)).map_err(|error| ConfigError(error.0))
}

fn parse_score_config(value: &Value, path: &str) -> Result<ScoreConfig, ConfigError> {
    let object = require_object(value, path)?;
    require_exact_keys(object, path, &["caps", "weights"])?;
    let caps_path = format!("{path}.caps");
    let caps_object = require_object(object.get("caps").unwrap_or(&Value::Null), &caps_path)?;
    require_exact_keys(caps_object, &caps_path, &SCORE_CAP_KEYS)?;
    let weights_path = format!("{path}.weights");
    let weights_object =
        require_object(object.get("weights").unwrap_or(&Value::Null), &weights_path)?;
    require_exact_keys(weights_object, &weights_path, &SCORE_WEIGHT_KEYS)?;

    let mut cap_values = [0.0f64; 8];
    for (index, key) in SCORE_CAP_KEYS.iter().enumerate() {
        let parsed = require_number(caps_object, &caps_path, key)?;
        if parsed <= 0.0 {
            return fail(format!("cap {key} must be finite and > 0"));
        }
        cap_values[index] = parsed;
    }
    let caps = ScoreCaps {
        cagr: cap_values[0],
        sortino: cap_values[1],
        calmar: cap_values[2],
        profit_factor: cap_values[3],
        consistency_sigma_scale: cap_values[4],
        complexity_units: cap_values[5],
        turnover: cap_values[6],
        data_mining_log10: cap_values[7],
    };
    if caps.profit_factor <= 1.0 {
        return fail("cap profitFactor must be > 1 (1 is the break-even floor)".into());
    }

    let mut weight_values = [0.0f64; 9];
    for (index, key) in SCORE_WEIGHT_KEYS.iter().enumerate() {
        let parsed = require_number(weights_object, &weights_path, key)?;
        if parsed < 0.0 {
            return fail(format!("weight {key} must be finite and >= 0"));
        }
        weight_values[index] = parsed;
    }
    let weights = ScoreWeights {
        cagr: weight_values[0],
        sortino: weight_values[1],
        calmar: weight_values[2],
        regime: weight_values[3],
        profit_factor: weight_values[4],
        consistency: weight_values[5],
        complexity: weight_values[6],
        turnover: weight_values[7],
        data_mining: weight_values[8],
    };
    if weights.regime != 0.0 {
        return fail(
            "regime weight must stay 0 until REGIME-001 implements the regime classifier".into(),
        );
    }

    Ok(ScoreConfig { caps, weights })
}

// ---------- concurrency ----------

/// Resolution D4: default `max(1, logicalCores - 1)`; an override must fit
/// `1..=logicalCores`. Concurrency affects performance only.
pub fn resolve_concurrency(requested: Option<f64>, logical_cores: f64) -> Result<i64, ConfigError> {
    if !is_safe_integer(logical_cores) || logical_cores < 1.0 {
        return fail("logicalCores must be an integer >= 1".into());
    }
    let cores = logical_cores as i64;
    match requested {
        None => Ok(std::cmp::max(1, cores - 1)),
        Some(value) => {
            if !is_safe_integer(value) || value < 1.0 || value > logical_cores {
                return fail(format!("maxConcurrency must be an integer in [1, {cores}]"));
            }
            Ok(value as i64)
        }
    }
}

// ---------- envelope ----------

/// Parse and resolve one `discovery-config-v1` envelope.
pub fn parse_discovery_config(
    value: &Value,
    logical_cores: f64,
) -> Result<ResolvedDiscoveryConfig, ConfigError> {
    let path = "discoveryConfig";
    let object = require_object(value, path)?;
    require_exact_keys(object, path, &ENVELOPE_KEYS)?;

    let envelope_version = require_string(object, path, "envelopeVersion")?;
    if envelope_version != DISCOVERY_CONFIG_VERSION {
        return fail(format!(
            "{path}.envelopeVersion must be \"{DISCOVERY_CONFIG_VERSION}\""
        ));
    }

    let contracts = discovery_contract_versions();
    let contracts_path = format!("{path}.contracts");
    let contracts_object = require_object(
        object.get("contracts").unwrap_or(&Value::Null),
        &contracts_path,
    )?;
    let entries = contract_entries(&contracts);
    let contract_keys: Vec<&str> = entries.iter().map(|(key, _)| *key).collect();
    require_exact_keys(contracts_object, &contracts_path, &contract_keys)?;
    for (key, expected) in &entries {
        let recorded = require_string(contracts_object, &contracts_path, key)?;
        if recorded != *expected {
            return fail(format!(
                "{contracts_path}.{key} must be \"{expected}\" (recorded \"{recorded}\")"
            ));
        }
    }

    let dataset_path = format!("{path}.dataset");
    let dataset_object =
        require_object(object.get("dataset").unwrap_or(&Value::Null), &dataset_path)?;
    require_exact_keys(dataset_object, &dataset_path, &["id", "contentHash"])?;
    let dataset_id = require_integer_in_range(
        require_number(dataset_object, &dataset_path, "id")?,
        &format!("{dataset_path}.id"),
        1,
        JS_MAX_SAFE_INTEGER as i64,
    )?;
    let content_hash = require_string(dataset_object, &dataset_path, "contentHash")?;
    if !super::seed::is_durable_identity(content_hash, DATASET_HASH_VERSION) {
        return fail(format!(
            "{dataset_path}.contentHash must be a durable {DATASET_HASH_VERSION} identity"
        ));
    }

    let raw_bases = require_array(object, path, "bases")?;
    if raw_bases.is_empty() {
        return fail(format!(
            "{path}.bases must contain at least one base preset"
        ));
    }
    let mut bases: Vec<DiscoveryBase> = Vec::new();
    for (index, raw_base) in raw_bases.iter().enumerate() {
        let base_path = format!("{path}.bases[{index}]");
        let base = parse_base(raw_base, &base_path)?;
        if bases.iter().any(|seen| seen.id == base.id) {
            return fail(format!("{base_path} repeats base id \"{}\"", base.id));
        }
        bases.push(base);
    }

    let embargo_path = format!("{path}.embargo");
    let embargo_object =
        require_object(object.get("embargo").unwrap_or(&Value::Null), &embargo_path)?;
    require_exact_keys(embargo_object, &embargo_path, &["holdingAllowanceBars"])?;
    let holding_allowance_bars = require_integer_in_range(
        require_number(embargo_object, &embargo_path, "holdingAllowanceBars")?,
        &format!("{embargo_path}.holdingAllowanceBars"),
        0,
        JS_MAX_SAFE_INTEGER as i64,
    )?;

    let execution_path = format!("{path}.execution");
    let execution_object = require_object(
        object.get("execution").unwrap_or(&Value::Null),
        &execution_path,
    )?;
    require_exact_keys(execution_object, &execution_path, &["startEquity"])?;
    let start_equity = require_number(execution_object, &execution_path, "startEquity")?;
    if start_equity <= 0.0 {
        return fail(format!("{execution_path}.startEquity must be > 0"));
    }

    let costs_path = format!("{path}.benchmarkCosts");
    let costs_object = require_object(
        object.get("benchmarkCosts").unwrap_or(&Value::Null),
        &costs_path,
    )?;
    require_exact_keys(costs_object, &costs_path, &["feePct", "slipPct"])?;
    let benchmark_costs = BenchmarkCostsInput {
        fee_pct: require_number(costs_object, &costs_path, "feePct")?,
        slip_pct: require_number(costs_object, &costs_path, "slipPct")?,
    };
    for (key, value) in [
        ("feePct", benchmark_costs.fee_pct),
        ("slipPct", benchmark_costs.slip_pct),
    ] {
        if let Some(problem) = check_numeric_param(key, value) {
            return fail(format!("{costs_path}.{problem}"));
        }
    }
    for (index, base) in bases.iter().enumerate() {
        let fee = base.strategy["feePct"].as_f64().unwrap_or(f64::NAN);
        let slip = base.strategy["slipPct"].as_f64().unwrap_or(f64::NAN);
        if fee != benchmark_costs.fee_pct || slip != benchmark_costs.slip_pct {
            return fail(format!(
                "{path}.benchmarkCosts must match bases[{index}] costs (feePct {fee}, slipPct {slip})"
            ));
        }
    }

    let random_entry_path = format!("{path}.randomEntry");
    let random_entry_object = require_object(
        object.get("randomEntry").unwrap_or(&Value::Null),
        &random_entry_path,
    )?;
    require_exact_keys(random_entry_object, &random_entry_path, &["runs"])?;
    let runs = require_integer_in_range(
        require_number(random_entry_object, &random_entry_path, "runs")?,
        &format!("{random_entry_path}.runs"),
        1,
        MAX_RANDOM_ENTRY_RUNS,
    )?;

    let gate_config = parse_gate_config(
        object.get("gateConfig").unwrap_or(&Value::Null),
        &format!("{path}.gateConfig"),
    )?;
    let score_config = parse_score_config(
        object.get("scoreConfig").unwrap_or(&Value::Null),
        &format!("{path}.scoreConfig"),
    )?;

    let root_seed = require_integer_in_range(
        require_number(object, path, "rootSeed")?,
        &format!("{path}.rootSeed"),
        0,
        MAX_U32,
    )? as u32;

    let caps_path = format!("{path}.caps");
    let caps_object = require_object(object.get("caps").unwrap_or(&Value::Null), &caps_path)?;
    require_exact_keys(caps_object, &caps_path, &["candidates"])?;
    let candidates = require_integer_in_range(
        require_number(caps_object, &caps_path, "candidates")?,
        &format!("{caps_path}.candidates"),
        1,
        DISCOVERY_HARD_CANDIDATE_CAP,
    )?;

    let requested_concurrency = match object.get("maxConcurrency") {
        Some(Value::Null) => None,
        Some(Value::Number(number)) => match number.as_f64() {
            Some(value) if value.is_finite() => Some(value),
            _ => {
                return fail(format!(
                    "{path}.maxConcurrency must be a finite number or null"
                ))
            }
        },
        _ => return fail(format!("{path}.maxConcurrency must be a number or null")),
    };
    let resolved = resolve_concurrency(requested_concurrency, logical_cores)?;

    Ok(ResolvedDiscoveryConfig {
        envelope_version: DISCOVERY_CONFIG_VERSION.to_string(),
        contracts,
        dataset: DatasetRef {
            id: dataset_id,
            content_hash: content_hash.to_string(),
        },
        bases,
        embargo: EmbargoInput {
            holding_allowance_bars,
        },
        execution: ExecutionInput { start_equity },
        benchmark_costs,
        random_entry: RandomEntryInput { runs },
        gate_config,
        score_config,
        root_seed,
        caps: CapsInput { candidates },
        concurrency: ResolvedConcurrency {
            requested: requested_concurrency,
            resolved,
            logical_cores: logical_cores as i64,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_versions_come_from_the_owning_modules() {
        let versions = discovery_contract_versions();
        assert_eq!(versions.gate, "gate-v1");
        assert_eq!(versions.score, "score-v1");
        assert_eq!(versions.split, "validation-split-v1");
        assert_eq!(versions.enumeration, DISCOVERY_ENUMERATION_VERSION);
        assert_eq!(contract_entries(&versions).len(), 12);
    }

    #[test]
    fn concurrency_defaults_and_bounds_match_the_reference() {
        assert_eq!(resolve_concurrency(None, 1.0).unwrap(), 1);
        assert_eq!(resolve_concurrency(None, 16.0).unwrap(), 15);
        assert_eq!(resolve_concurrency(Some(4.0), 4.0).unwrap(), 4);
        assert!(resolve_concurrency(Some(5.0), 4.0).is_err());
        assert!(resolve_concurrency(None, 0.0).is_err());
    }

    #[test]
    fn base_id_pattern_accepts_only_lowercase_hyphenated_ids() {
        assert!(is_valid_base_id("ma-cross"));
        assert!(is_valid_base_id("grid1"));
        assert!(!is_valid_base_id(""));
        assert!(!is_valid_base_id("-lead"));
        assert!(!is_valid_base_id("MA-cross"));
        assert!(!is_valid_base_id("ma cross"));
    }
}
