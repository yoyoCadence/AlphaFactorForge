//! Strict runtime contract for immutable validation audit records.
//!
//! `validation-record-v1` remains readable database history, but it predates
//! an explicit metrics formula pin. New writes must use v2 and pass this full
//! DTO before any transaction opens.

use std::collections::{BTreeMap, BTreeSet};

use alpha_factor_forge::discovery_core::{
    backtest::EXECUTION_CONTRACT_VERSION,
    benchmarks::{BENCHMARK_CONTRACT_VERSION, DETERMINISTIC_BENCHMARK_IDS},
    gate::GATE_CONTRACT_VERSION,
    identity::{DATASET_HASH_VERSION, STRATEGY_HASH_VERSION},
    metrics::METRICS_CONTRACT_VERSION,
    score::SCORE_FORMULA_VERSION,
    seed::is_durable_identity,
};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::repositories::BacktestSummary;

pub const VALIDATION_RECORD_VERSION: &str = "validation-record-v2";
pub const LEGACY_VALIDATION_RECORD_VERSION: &str = "validation-record-v1";
pub const BENCHMARK_RECORD_VERSION: &str = "bench-record-v1";
const TESTED_COMBINATIONS_BASIS: &str = "lineage-final-unique";
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Clone, Debug)]
pub(super) struct RequiredNullable<T>(pub Option<T>);

impl<'de, T> Deserialize<'de> for RequiredNullable<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Nullable<T> {
            Value(T),
            Null(()),
        }

        Nullable::<T>::deserialize(deserializer).map(|value| match value {
            Nullable::Value(value) => Self(Some(value)),
            Nullable::Null(()) => Self(None),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ValidationRecordV2 {
    version: String,
    contracts: RecordContracts,
    pub strategy_id: i64,
    strategy_hash: String,
    pub dataset_id: i64,
    dataset_hash: String,
    embargo: EmbargoDto,
    split_plan: SplitPlanDto,
    pub train_metrics: EncodedMetricsDto,
    pub validation_metrics: EncodedMetricsDto,
    pub benchmark: BenchmarkRecordDto,
    pub gate: GateDto,
    pub gate_passed: bool,
    pub score: RequiredNullable<ScoreDto>,
    tested_combinations: TestedCombinationsDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordContracts {
    execution: String,
    benchmark: String,
    metrics: String,
    gate: String,
    score: RequiredNullable<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EmbargoDto {
    embargo_bars: i64,
    max_signal_lookback_bars: i64,
    holding_allowance_bars: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct RangeDto {
    from: i64,
    to: i64,
    count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SplitPlanDto {
    total_bars: i64,
    usable_bars: i64,
    embargo_bars: i64,
    train: RangeDto,
    train_validation_embargo: RequiredNullable<RangeDto>,
    validation: RangeDto,
    validation_test_embargo: RequiredNullable<RangeDto>,
    test: RangeDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum NonFiniteStatus {
    PositiveInfinity,
    NegativeInfinity,
    Nan,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MetricValuesDto {
    net_return: RequiredNullable<f64>,
    cagr: RequiredNullable<f64>,
    max_drawdown: RequiredNullable<f64>,
    sharpe: RequiredNullable<f64>,
    sortino: RequiredNullable<f64>,
    calmar: RequiredNullable<f64>,
    win_rate: RequiredNullable<f64>,
    trade_count: RequiredNullable<f64>,
    profit_factor: RequiredNullable<f64>,
    avg_trade_return: RequiredNullable<f64>,
    median_trade_return: RequiredNullable<f64>,
    avg_holding_bars: RequiredNullable<f64>,
    exposure: RequiredNullable<f64>,
    turnover: RequiredNullable<f64>,
    largest_win: RequiredNullable<f64>,
    largest_loss: RequiredNullable<f64>,
    consecutive_losses: RequiredNullable<f64>,
    monthly_returns: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EncodedMetricsDto {
    values: MetricValuesDto,
    non_finite: BTreeMap<String, NonFiniteStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BenchmarkRecordDto {
    version: String,
    benchmark_contract: String,
    interval: String,
    validation_range: BenchmarkRangeDto,
    start_equity: RequiredNullable<f64>,
    costs: BenchmarkCostsDto,
    benchmarks: Vec<BenchmarkEntryDto>,
    random_entry: RandomEntryDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkRangeDto {
    from: i64,
    to: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BenchmarkCostsDto {
    fee_pct: f64,
    slip_pct: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkRuleDto {
    l: String,
    op: String,
    r: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BenchmarkStrategyDto {
    mode: String,
    #[serde(rename = "fastMA")]
    fast_ma: usize,
    #[serde(rename = "slowMA")]
    slow_ma: usize,
    ema_period: usize,
    rsi_period: usize,
    rsi_buy: f64,
    rsi_sell: f64,
    macd_fast: usize,
    macd_slow: usize,
    macd_signal: usize,
    bb_period: usize,
    bb_mult: f64,
    entry_sig: String,
    exit_sig: String,
    entry_rules: Vec<BenchmarkRuleDto>,
    exit_rules: Vec<BenchmarkRuleDto>,
    entry_code: String,
    exit_code: String,
    sl_pct: f64,
    tp_pct: f64,
    fee_pct: f64,
    slip_pct: f64,
    size_pct: f64,
    fill_mode: String,
    direction: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkEntryDto {
    id: String,
    strat: RequiredNullable<BenchmarkStrategyDto>,
    metrics: EncodedMetricsDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RandomEntryDto {
    runs: i64,
    seed: i64,
    net_returns: Vec<f64>,
    candidate_net_return: f64,
    candidate_percentile: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum GateValueStatusDto {
    PositiveInfinity,
    NegativeInfinity,
    Nan,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateConfigDto {
    min_trades: i64,
    min_avg_trade_return: f64,
    rolling_window_bars: i64,
    min_rolling_positive_ratio: f64,
    max_drawdown: f64,
    max_monthly_contribution: f64,
    max_single_trade_contribution: f64,
    min_random_entry_percentile: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GateCriterionDto {
    id: String,
    pass: bool,
    value: RequiredNullable<f64>,
    value_status: RequiredNullable<GateValueStatusDto>,
    threshold: f64,
    #[serde(default)]
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GateDto {
    version: String,
    pass: bool,
    criteria: Vec<GateCriterionDto>,
    config: GateConfigDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScoreCapsDto {
    cagr: f64,
    sortino: f64,
    calmar: f64,
    profit_factor: f64,
    consistency_sigma_scale: f64,
    complexity_units: f64,
    turnover: f64,
    data_mining_log10: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScoreWeightsDto {
    cagr: f64,
    sortino: f64,
    calmar: f64,
    regime: f64,
    profit_factor: f64,
    consistency: f64,
    complexity: f64,
    turnover: f64,
    data_mining: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreConfigDto {
    caps: ScoreCapsDto,
    weights: ScoreWeightsDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConsistencyEvidenceDto {
    month_count: usize,
    monthly_std_dev: RequiredNullable<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ComplexityEvidenceDto {
    decision_nodes: usize,
    indicator_params: usize,
    risk_rules: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TurnoverEvidenceDto {
    proxy: String,
    closed_trade_count: usize,
    total_bars: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataMiningEvidenceDto {
    n: u64,
    basis: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ScoreEvidenceDto {
    Consistency(ConsistencyEvidenceDto),
    Complexity(ComplexityEvidenceDto),
    Turnover(TurnoverEvidenceDto),
    DataMining(DataMiningEvidenceDto),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScoreEntryDto {
    id: String,
    raw: RequiredNullable<f64>,
    raw_status: String,
    normalized: RequiredNullable<f64>,
    weight: f64,
    contribution: f64,
    #[serde(default)]
    evidence: Option<ScoreEvidenceDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ScoreDto {
    formula_version: String,
    segment: String,
    pub score: f64,
    components: Vec<ScoreEntryDto>,
    penalties: Vec<ScoreEntryDto>,
    config: ScoreConfigDto,
    tested_combinations: TestedCombinationsDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestedCombinationsDto {
    n: u64,
    basis: String,
}

fn require(condition: bool, message: String) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message)
    }
}

fn validate_range(range: &RangeDto, label: &str) -> Result<(), String> {
    require(
        range.from >= 0,
        format!("{label}.from must be non-negative"),
    )?;
    require(
        range.to >= range.from,
        format!("{label}.to must be >= from"),
    )?;
    require(
        range.count == range.to - range.from + 1,
        format!("{label}.count must match its inclusive bounds"),
    )
}

fn validate_split(split: &SplitPlanDto, embargo: &EmbargoDto) -> Result<(), String> {
    require(
        embargo.max_signal_lookback_bars >= 1 && embargo.holding_allowance_bars >= 0,
        "embargo derivation values are outside their contract".into(),
    )?;
    require(
        embargo
            .max_signal_lookback_bars
            .checked_add(embargo.holding_allowance_bars)
            == Some(embargo.embargo_bars),
        "embargoBars must equal lookback plus holding allowance".into(),
    )?;
    require(
        split.embargo_bars == embargo.embargo_bars,
        "split embargo must match derivation".into(),
    )?;
    require(
        (0..=JS_MAX_SAFE_INTEGER).contains(&split.total_bars)
            && (0..=JS_MAX_SAFE_INTEGER).contains(&split.usable_bars),
        "split bar counts must be JavaScript safe integers".into(),
    )?;
    for (range, label) in [
        (&split.train, "splitPlan.train"),
        (&split.validation, "splitPlan.validation"),
        (&split.test, "splitPlan.test"),
    ] {
        validate_range(range, label)?;
    }
    let train_gap = split.train_validation_embargo.0.as_ref();
    let validation_gap = split.validation_test_embargo.0.as_ref();
    require(
        train_gap.is_some() == (split.embargo_bars > 0)
            && validation_gap.is_some() == (split.embargo_bars > 0),
        "split embargo ranges must exist exactly when embargoBars is positive".into(),
    )?;
    if let Some(range) = train_gap {
        validate_range(range, "splitPlan.trainValidationEmbargo")?;
        require(
            range.count == split.embargo_bars,
            "train/validation embargo count mismatch".into(),
        )?;
    }
    if let Some(range) = validation_gap {
        validate_range(range, "splitPlan.validationTestEmbargo")?;
        require(
            range.count == split.embargo_bars,
            "validation/test embargo count mismatch".into(),
        )?;
    }
    let mut cursor = 0;
    for range in [
        Some(&split.train),
        train_gap,
        Some(&split.validation),
        validation_gap,
        Some(&split.test),
    ]
    .into_iter()
    .flatten()
    {
        require(
            range.from == cursor,
            "split ranges must be contiguous and ordered".into(),
        )?;
        cursor = range.to + 1;
    }
    require(
        cursor == split.total_bars,
        "split ranges must cover totalBars exactly".into(),
    )?;
    require(
        split.usable_bars == split.train.count + split.validation.count + split.test.count,
        "usableBars must equal Train + Validation + Test counts".into(),
    )
}

impl EncodedMetricsDto {
    fn numeric_fields(&self) -> [(&'static str, &RequiredNullable<f64>); 17] {
        let values = &self.values;
        [
            ("netReturn", &values.net_return),
            ("cagr", &values.cagr),
            ("maxDrawdown", &values.max_drawdown),
            ("sharpe", &values.sharpe),
            ("sortino", &values.sortino),
            ("calmar", &values.calmar),
            ("winRate", &values.win_rate),
            ("tradeCount", &values.trade_count),
            ("profitFactor", &values.profit_factor),
            ("avgTradeReturn", &values.avg_trade_return),
            ("medianTradeReturn", &values.median_trade_return),
            ("avgHoldingBars", &values.avg_holding_bars),
            ("exposure", &values.exposure),
            ("turnover", &values.turnover),
            ("largestWin", &values.largest_win),
            ("largestLoss", &values.largest_loss),
            ("consecutiveLosses", &values.consecutive_losses),
        ]
    }

    fn validate(&self, label: &str) -> Result<(), String> {
        let allowed: BTreeSet<&str> = self.numeric_fields().iter().map(|(key, _)| *key).collect();
        for key in self.non_finite.keys() {
            require(
                allowed.contains(key.as_str()),
                format!("{label}.nonFinite contains unknown metric {key}"),
            )?;
        }
        for (key, value) in self.numeric_fields() {
            require(
                value.0.is_none() == self.non_finite.contains_key(key),
                format!("{label}.{key} null/status encoding is inconsistent"),
            )?;
        }
        for (key, value) in [
            ("tradeCount", self.values.trade_count.0),
            ("consecutiveLosses", self.values.consecutive_losses.0),
        ] {
            if let Some(value) = value {
                require(
                    value >= 0.0 && value.fract() == 0.0 && value <= JS_MAX_SAFE_INTEGER as f64,
                    format!("{label}.{key} must be a non-negative safe integer"),
                )?;
            }
        }
        for (month, value) in &self.values.monthly_returns {
            let bytes = month.as_bytes();
            let valid_key = bytes.len() == 7
                && bytes[4] == b'-'
                && bytes
                    .iter()
                    .enumerate()
                    .all(|(index, byte)| index == 4 || byte.is_ascii_digit())
                && month[5..]
                    .parse::<u8>()
                    .is_ok_and(|value| (1..=12).contains(&value));
            require(
                valid_key,
                format!("{label}.monthlyReturns contains invalid UTC month {month}"),
            )?;
            require(
                value.is_finite(),
                format!("{label}.monthlyReturns.{month} must be finite"),
            )?;
        }
        Ok(())
    }

    pub(super) fn validate_summary(
        &self,
        summary: &BacktestSummary,
        label: &str,
    ) -> Result<(), String> {
        let values = &self.values;
        for (name, encoded, persisted) in [
            ("netReturn", values.net_return.0, summary.net_return),
            ("cagr", values.cagr.0, summary.cagr),
            ("maxDrawdown", values.max_drawdown.0, summary.max_drawdown),
            ("sharpe", values.sharpe.0, summary.sharpe),
            ("sortino", values.sortino.0, summary.sortino),
            ("calmar", values.calmar.0, summary.calmar),
            ("winRate", values.win_rate.0, summary.win_rate),
            (
                "tradeCount",
                values.trade_count.0,
                summary.trade_count.map(|value| value as f64),
            ),
            (
                "profitFactor",
                values.profit_factor.0,
                summary.profit_factor,
            ),
            (
                "avgTradeReturn",
                values.avg_trade_return.0,
                summary.avg_trade_return,
            ),
            (
                "medianTradeReturn",
                values.median_trade_return.0,
                summary.median_trade_return,
            ),
            ("exposure", values.exposure.0, summary.exposure),
            ("turnover", values.turnover.0, summary.turnover),
            ("largestWin", values.largest_win.0, summary.largest_win),
            ("largestLoss", values.largest_loss.0, summary.largest_loss),
            (
                "consecutiveLosses",
                values.consecutive_losses.0,
                summary.consecutive_losses.map(|value| value as f64),
            ),
        ] {
            let equal = match (encoded, persisted) {
                (Some(left), Some(right)) => {
                    (left - right).abs()
                        <= f64::EPSILON * 4.0 * left.abs().max(right.abs()).max(1.0)
                }
                (None, None) => true,
                _ => false,
            };
            require(
                equal,
                format!(
                    "{label}.{name} must equal the immutable metric snapshot ({encoded:?} != {persisted:?})"
                ),
            )?;
        }
        Ok(())
    }
}

fn validate_strategy(
    strategy: &BenchmarkStrategyDto,
    costs: &BenchmarkCostsDto,
) -> Result<(), String> {
    require(
        strategy.mode == "params",
        "benchmark strategy mode must be params".into(),
    )?;
    for (name, period) in [
        ("fastMA", strategy.fast_ma),
        ("slowMA", strategy.slow_ma),
        ("emaPeriod", strategy.ema_period),
        ("rsiPeriod", strategy.rsi_period),
        ("macdFast", strategy.macd_fast),
        ("macdSlow", strategy.macd_slow),
        ("macdSignal", strategy.macd_signal),
        ("bbPeriod", strategy.bb_period),
    ] {
        require(
            period > 0,
            format!("benchmark strategy {name} must be positive"),
        )?;
    }
    for (name, value) in [
        ("rsiBuy", strategy.rsi_buy),
        ("rsiSell", strategy.rsi_sell),
        ("bbMult", strategy.bb_mult),
        ("slPct", strategy.sl_pct),
        ("tpPct", strategy.tp_pct),
        ("feePct", strategy.fee_pct),
        ("slipPct", strategy.slip_pct),
        ("sizePct", strategy.size_pct),
    ] {
        require(
            value.is_finite(),
            format!("benchmark strategy {name} must be finite"),
        )?;
    }
    for (name, value) in [
        ("entrySig", strategy.entry_sig.as_str()),
        ("exitSig", strategy.exit_sig.as_str()),
        ("entryCode", strategy.entry_code.as_str()),
        ("exitCode", strategy.exit_code.as_str()),
    ] {
        require(
            !value.is_empty(),
            format!("benchmark strategy {name} must not be empty"),
        )?;
    }
    for rule in strategy.entry_rules.iter().chain(&strategy.exit_rules) {
        require(
            !rule.l.is_empty() && !rule.op.is_empty() && !rule.r.is_empty(),
            "benchmark rule fields must not be empty".into(),
        )?;
    }
    require(
        strategy.fee_pct == costs.fee_pct && strategy.slip_pct == costs.slip_pct,
        "benchmark strategy costs must equal benchmark costs".into(),
    )?;
    require(
        strategy.size_pct == 100.0
            && strategy.sl_pct == 0.0
            && strategy.tp_pct == 0.0
            && strategy.fill_mode == "close"
            && strategy.direction == "long",
        "benchmark strategy execution settings must match benchmark-suite-v1".into(),
    )
}

impl BenchmarkRecordDto {
    fn validate(&self, split: &SplitPlanDto) -> Result<(), String> {
        require(
            self.version == BENCHMARK_RECORD_VERSION,
            "unsupported benchmark record version".into(),
        )?;
        require(
            self.benchmark_contract == BENCHMARK_CONTRACT_VERSION,
            "benchmark contract version mismatch".into(),
        )?;
        require(
            !self.interval.is_empty(),
            "benchmark interval must not be empty".into(),
        )?;
        require(
            self.validation_range.from == split.validation.from
                && self.validation_range.to == split.validation.to,
            "benchmark validation range must equal splitPlan.validation".into(),
        )?;
        if let Some(start_equity) = self.start_equity.0 {
            require(
                start_equity > 0.0,
                "benchmark startEquity must be positive".into(),
            )?;
        }
        require(
            self.costs.fee_pct >= 0.0 && self.costs.slip_pct >= 0.0,
            "benchmark costs must be non-negative".into(),
        )?;
        require(
            self.benchmarks.len() == DETERMINISTIC_BENCHMARK_IDS.len(),
            "benchmark record must contain exactly four deterministic benchmarks".into(),
        )?;
        for (index, expected_id) in DETERMINISTIC_BENCHMARK_IDS.iter().enumerate() {
            let entry = &self.benchmarks[index];
            require(
                entry.id == *expected_id,
                "deterministic benchmark ids/order mismatch".into(),
            )?;
            require(
                entry.strat.0.is_none() == (*expected_id == "buyHold"),
                format!("benchmark {expected_id} strategy nullability is invalid"),
            )?;
            if let Some(strategy) = &entry.strat.0 {
                validate_strategy(strategy, &self.costs)?;
            }
            entry
                .metrics
                .validate(&format!("benchmark.{expected_id}.metrics"))?;
        }
        let random = &self.random_entry;
        require(
            random.runs >= 1 && random.net_returns.len() == random.runs as usize,
            "Random Entry distribution length must equal runs".into(),
        )?;
        require(
            (0..=u32::MAX as i64).contains(&random.seed),
            "Random Entry seed must be uint32".into(),
        )?;
        require(
            (0.0..=100.0).contains(&random.candidate_percentile)
                && random.candidate_net_return.is_finite(),
            "Random Entry candidate evidence is invalid".into(),
        )
    }
}

impl GateDto {
    fn validate(&self) -> Result<(), String> {
        require(
            self.version == GATE_CONTRACT_VERSION,
            "Gate contract version mismatch".into(),
        )?;
        let cfg = &self.config;
        require(
            cfg.min_trades >= 1 && cfg.rolling_window_bars >= 1,
            "Gate integer config values must be positive".into(),
        )?;
        for (name, value) in [
            ("minRollingPositiveRatio", cfg.min_rolling_positive_ratio),
            ("maxDrawdown", cfg.max_drawdown),
            ("maxMonthlyContribution", cfg.max_monthly_contribution),
            (
                "maxSingleTradeContribution",
                cfg.max_single_trade_contribution,
            ),
        ] {
            require(
                (0.0..=1.0).contains(&value),
                format!("Gate {name} must be in [0, 1]"),
            )?;
        }
        require(
            (0.0..=100.0).contains(&cfg.min_random_entry_percentile)
                && cfg.min_avg_trade_return.is_finite(),
            "Gate numeric config is invalid".into(),
        )?;
        let expected = [
            ("minTrades", cfg.min_trades as f64),
            ("avgTradeReturn", cfg.min_avg_trade_return),
            ("rollingConsistency", cfg.min_rolling_positive_ratio),
            ("maxDrawdown", cfg.max_drawdown),
            ("monthlyConcentration", cfg.max_monthly_contribution),
            ("tradeConcentration", cfg.max_single_trade_contribution),
            ("benchmarkWins", DETERMINISTIC_BENCHMARK_IDS.len() as f64),
            ("randomEntryPercentile", cfg.min_random_entry_percentile),
        ];
        require(
            self.criteria.len() == expected.len(),
            "Gate must contain exactly eight criteria".into(),
        )?;
        for (index, (criterion, (expected_id, expected_threshold))) in
            self.criteria.iter().zip(expected).enumerate()
        {
            require(
                criterion.id == expected_id,
                "Gate criterion ids/order mismatch".into(),
            )?;
            require(
                criterion.threshold == expected_threshold,
                format!("Gate criterion {expected_id} threshold/config mismatch"),
            )?;
            require(
                criterion.value.0.is_none() || criterion.value_status.0.is_none(),
                format!("Gate criterion {expected_id} cannot have value and valueStatus"),
            )?;
            if criterion.value_status.0.is_some() {
                require(
                    criterion.value.0.is_none(),
                    format!("Gate criterion {expected_id} non-finite status requires null value"),
                )?;
            }
            if let Some(detail) = &criterion.detail {
                require(
                    !detail.is_empty(),
                    "Gate criterion detail must not be empty".into(),
                )?;
            }
            let computed_pass = criterion.value.0.is_some_and(|value| match index {
                0 | 2 | 6 | 7 => value >= criterion.threshold,
                1 => value > criterion.threshold,
                3..=5 => value <= criterion.threshold,
                _ => false,
            });
            require(
                criterion.pass == computed_pass,
                format!("Gate criterion {expected_id} pass/value/threshold mismatch"),
            )?;
        }
        require(
            self.pass == self.criteria.iter().all(|criterion| criterion.pass),
            "Gate pass must equal the conjunction of all criteria".into(),
        )
    }
}

fn validate_score_entry(entry: &ScoreEntryDto, expected_id: &str) -> Result<(), String> {
    require(
        entry.id == expected_id,
        "Score entry ids/order mismatch".into(),
    )?;
    require(
        entry.weight >= 0.0,
        format!("Score {expected_id} weight must be non-negative"),
    )?;
    if let Some(normalized) = entry.normalized.0 {
        require(
            (0.0..=1.0).contains(&normalized),
            format!("Score {expected_id} normalized value must be in [0, 1]"),
        )?;
    }
    match entry.raw_status.as_str() {
        "finite" => require(
            entry.raw.0.is_some() && entry.normalized.0.is_some(),
            format!("Score {expected_id} finite raw/normalized evidence is missing"),
        )?,
        "positive_infinity" => require(
            entry.raw.0.is_none() && entry.normalized.0 == Some(1.0),
            format!("Score {expected_id} positive infinity encoding is invalid"),
        )?,
        "insufficient" | "invalid" => require(
            entry.raw.0.is_none() && entry.normalized.0 == Some(0.0),
            format!("Score {expected_id} insufficient/invalid encoding is invalid"),
        )?,
        "deferred" => require(
            entry.raw.0.is_none() && entry.normalized.0.is_none(),
            format!("Score {expected_id} deferred encoding is invalid"),
        )?,
        _ => return Err(format!("Score {expected_id} has unknown rawStatus")),
    }
    require(
        entry.contribution == entry.weight * entry.normalized.0.unwrap_or(0.0),
        format!("Score {expected_id} contribution is inconsistent"),
    )
}

impl ScoreDto {
    fn validate(&self, tested: &TestedCombinationsDto) -> Result<(), String> {
        require(
            self.formula_version == SCORE_FORMULA_VERSION,
            "Score formula version mismatch".into(),
        )?;
        require(
            self.segment == "validation",
            "Score segment must be validation".into(),
        )?;
        require(
            self.tested_combinations.n == tested.n
                && self.tested_combinations.basis == tested.basis,
            "Score testedCombinations must equal record evidence".into(),
        )?;
        let component_ids = [
            "cagr",
            "sortino",
            "calmar",
            "regime",
            "profitFactor",
            "consistency",
        ];
        let penalty_ids = ["complexity", "turnover", "dataMining"];
        require(
            self.components.len() == component_ids.len(),
            "Score must contain six components".into(),
        )?;
        require(
            self.penalties.len() == penalty_ids.len(),
            "Score must contain three penalties".into(),
        )?;
        for (entry, id) in self.components.iter().zip(component_ids) {
            validate_score_entry(entry, id)?;
        }
        for (entry, id) in self.penalties.iter().zip(penalty_ids) {
            validate_score_entry(entry, id)?;
        }
        require(
            self.components[3].raw_status == "deferred",
            "regime component must remain deferred".into(),
        )?;
        match self.components[5].evidence.as_ref() {
            Some(ScoreEvidenceDto::Consistency(evidence)) => require(
                evidence.monthly_std_dev.0 == self.components[5].raw.0
                    && evidence.month_count <= JS_MAX_SAFE_INTEGER as usize,
                "consistency evidence is inconsistent".into(),
            )?,
            _ => return Err("consistency component requires consistency evidence".into()),
        }
        match self.penalties[0].evidence.as_ref() {
            Some(ScoreEvidenceDto::Complexity(evidence)) => {
                let units = evidence
                    .decision_nodes
                    .checked_add(evidence.indicator_params)
                    .and_then(|value| value.checked_add(evidence.risk_rules));
                require(
                    units.is_some_and(|value| value > 0),
                    "complexity evidence must contain a safe positive unit count".into(),
                )?;
            }
            _ => return Err("complexity penalty requires complexity evidence".into()),
        }
        match self.penalties[1].evidence.as_ref() {
            Some(ScoreEvidenceDto::Turnover(evidence)) => require(
                evidence.proxy == "closedTrades/totalBars@v1"
                    && evidence.total_bars > 0
                    && evidence.closed_trade_count <= JS_MAX_SAFE_INTEGER as usize,
                "turnover evidence is invalid".into(),
            )?,
            _ => return Err("turnover penalty requires turnover evidence".into()),
        }
        match self.penalties[2].evidence.as_ref() {
            Some(ScoreEvidenceDto::DataMining(evidence)) => require(
                evidence.n == tested.n && evidence.basis == TESTED_COMBINATIONS_BASIS,
                "data-mining evidence must equal record testedCombinations".into(),
            )?,
            _ => return Err("dataMining penalty requires data-mining evidence".into()),
        }
        for entry in self.components.iter().take(5) {
            if entry.id != "regime" {
                require(
                    entry.evidence.is_none(),
                    format!("Score {} has unexpected evidence", entry.id),
                )?;
            }
        }
        let caps = &self.config.caps;
        for (name, value) in [
            ("cagr", caps.cagr),
            ("sortino", caps.sortino),
            ("calmar", caps.calmar),
            ("profitFactor", caps.profit_factor),
            ("consistencySigmaScale", caps.consistency_sigma_scale),
            ("complexityUnits", caps.complexity_units),
            ("turnover", caps.turnover),
            ("dataMiningLog10", caps.data_mining_log10),
        ] {
            require(value > 0.0, format!("Score cap {name} must be positive"))?;
        }
        let weights = &self.config.weights;
        let expected_weights = [
            weights.cagr,
            weights.sortino,
            weights.calmar,
            weights.regime,
            weights.profit_factor,
            weights.consistency,
            weights.complexity,
            weights.turnover,
            weights.data_mining,
        ];
        for (entry, expected) in self
            .components
            .iter()
            .chain(&self.penalties)
            .zip(expected_weights)
        {
            require(
                entry.weight == expected,
                "Score entry weight/config mismatch".into(),
            )?;
        }
        require(
            weights.regime == 0.0,
            "Score regime weight must remain zero".into(),
        )?;
        let computed = self
            .components
            .iter()
            .map(|entry| entry.contribution)
            .sum::<f64>()
            - self
                .penalties
                .iter()
                .map(|entry| entry.contribution)
                .sum::<f64>();
        require(
            self.score == computed,
            "Score total does not match contributions".into(),
        )
    }
}

impl ValidationRecordV2 {
    fn validate(&self) -> Result<(), String> {
        require(
            self.version == VALIDATION_RECORD_VERSION,
            "record envelope version mismatch".into(),
        )?;
        require(
            self.strategy_id > 0 && self.dataset_id > 0,
            "record ids must be positive".into(),
        )?;
        require(
            is_durable_identity(&self.strategy_hash, STRATEGY_HASH_VERSION),
            "strategyHash must be a durable strategy-v2 identity".into(),
        )?;
        require(
            is_durable_identity(&self.dataset_hash, DATASET_HASH_VERSION),
            "datasetHash must be a durable dataset-content-v2 identity".into(),
        )?;
        let contracts = &self.contracts;
        require(
            contracts.execution == EXECUTION_CONTRACT_VERSION
                && contracts.benchmark == BENCHMARK_CONTRACT_VERSION
                && contracts.metrics == METRICS_CONTRACT_VERSION
                && contracts.gate == GATE_CONTRACT_VERSION,
            "record contract versions are inconsistent".into(),
        )?;
        require(
            self.tested_combinations.n >= 1
                && self.tested_combinations.n <= JS_MAX_SAFE_INTEGER as u64
                && self.tested_combinations.basis == TESTED_COMBINATIONS_BASIS,
            "testedCombinations evidence is invalid".into(),
        )?;
        validate_split(&self.split_plan, &self.embargo)?;
        self.train_metrics.validate("trainMetrics")?;
        self.validation_metrics.validate("validationMetrics")?;
        self.benchmark.validate(&self.split_plan)?;
        self.gate.validate()?;
        require(
            self.gate_passed == self.gate.pass,
            "record gatePassed must equal Gate verdict".into(),
        )?;
        match (&contracts.score.0, &self.score.0, self.gate_passed) {
            (Some(contract), Some(score), true) => {
                require(
                    contract == SCORE_FORMULA_VERSION,
                    "Score contract version mismatch".into(),
                )?;
                score.validate(&self.tested_combinations)?;
            }
            (None, None, false) => {}
            _ => return Err("Gate, Score contract, and Score snapshot are inconsistent".into()),
        }
        Ok(())
    }
}

/// Explicit write-version dispatch. Legacy v1 remains readable through the
/// row APIs but cannot be appended after metrics-v2 became the active formula.
pub(super) fn parse_new_record(
    record_version: &str,
    record_json: &str,
) -> Result<ValidationRecordV2, String> {
    match record_version {
        VALIDATION_RECORD_VERSION => {
            let value: Value = serde_json::from_str(record_json)
                .map_err(|error| format!("record_json must be valid JSON: {error}"))?;
            let record: ValidationRecordV2 = serde_json::from_value(value)
                .map_err(|error| format!("invalid validation-record-v2 DTO: {error}"))?;
            record.validate()?;
            Ok(record)
        }
        LEGACY_VALIDATION_RECORD_VERSION => Err(
            "validation-record-v1 is legacy read-only evidence with an unknown metrics formula"
                .into(),
        ),
        other => Err(format!("unsupported validation record version {other}")),
    }
}
