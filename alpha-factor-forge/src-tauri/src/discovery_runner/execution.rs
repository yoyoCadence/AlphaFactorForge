//! Pure execution boundary for one discovery candidate.
//!
//! This module deliberately owns no threads, SQLite connection, Tauri state,
//! or event emitter. A coordinator supplies one already-admitted candidate and
//! immutable candle slice; the result is the complete owned persistence bundle
//! that `db::discovery::commit_candidate_assessment` can commit atomically.
//!
//! The hidden Test segment is planned for audit lineage but never executed.

use std::fmt;

use alpha_factor_forge::discovery_core::{
    backtest::{
        run_backtest, BacktestConfig, BacktestResult, CostModel, Direction, ExecutionModel,
        FillMode, RiskModel, EXECUTION_CONTRACT_VERSION,
    },
    benchmarks::{
        bars_per_year, run_deterministic_benchmarks, BenchmarkCosts, BenchmarkRun,
        RunBenchmarksArgs, BENCHMARK_CONTRACT_VERSION, DETERMINISTIC_BENCHMARK_IDS,
    },
    config::ResolvedDiscoveryConfig,
    embargo::{derive_embargo_bars, EmbargoDerivation},
    enumerate::EnumeratedCandidate,
    gate::{
        benchmark_views, encode_gate_verdict, evaluate_gate, EvaluateGateArgs, GateCandidateView,
        GateConfig, GateConfigOverrides, GateRandomEntryView, GateVerdict, GATE_CONTRACT_VERSION,
    },
    identity::strategy_hash,
    metrics::{ClosedTrade, Metrics, TradeSide, METRICS_CONTRACT_VERSION},
    random_entry::{
        run_random_entry_benchmark, RandomEntryArgs, RandomEntryBenchmark, RandomEntryCandidate,
    },
    score::{
        score_candidate, ParamsStrategyProjection, ScoreBreakdown, ScoreCandidateArgs,
        ScoreCandidateView, ScoreCapsOverride, ScoreConfig, ScoreConfigOverride,
        ScoreWeightsOverride, SCORE_FORMULA_VERSION,
    },
    signals::{build_params_signals, ParamsSignalConfig},
    split::{plan_validation_split, ValidationSplitPlan},
    types::Candle,
};
use chrono::{TimeZone, Utc};
use serde::Serialize;
use serde_json::{Map, Number, Value};

use crate::db::{
    repositories::{BacktestSummary, TradeRow, ValidationRecordRow},
    validation_record::{BENCHMARK_RECORD_VERSION, VALIDATION_RECORD_VERSION},
};
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// Durable dataset metadata already revalidated by the coordinator at start.
///
/// It is repeated here so a candidate cannot accidentally be composed against
/// a different run config while sharing the same candle allocation.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionDataset<'a> {
    pub id: i64,
    pub content_hash: &'a str,
    pub interval: &'a str,
}

/// Everything needed to execute one candidate assessment.
///
/// `tested_combinations` is the candidate plan's lineage-final unique count.
/// It is explicit because neither `ResolvedDiscoveryConfig` nor one
/// `EnumeratedCandidate` carries that plan-level value; recomputing the whole
/// enumeration inside every worker would turn a run into O(N²) admission work.
pub struct ExecuteCandidateArgs<'a> {
    pub config: &'a ResolvedDiscoveryConfig,
    pub candidate: &'a EnumeratedCandidate,
    pub tested_combinations: i64,
    pub strategy_id: i64,
    pub dataset: ExecutionDataset<'a>,
    pub candles: &'a [Candle],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateResultDigest {
    pub gate_passed: bool,
    pub score: Option<f64>,
}

/// Owned output ready for the runner store's one-candidate transaction.
pub struct CandidateExecutionOutput {
    pub train_summary: BacktestSummary,
    pub train_trades: Vec<TradeRow>,
    pub validation_summary: BacktestSummary,
    pub validation_trades: Vec<TradeRow>,
    pub record: ValidationRecordRow,
    pub digest: CandidateResultDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateExecutionError(pub String);

impl fmt::Display for CandidateExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CandidateExecutionError {}

fn fail<T>(message: impl Into<String>) -> Result<T, CandidateExecutionError> {
    Err(CandidateExecutionError(message.into()))
}

fn context(error: impl fmt::Display, label: &str) -> CandidateExecutionError {
    CandidateExecutionError(format!("{label}: {error}"))
}

/// Protect the still-infallible `metrics::monthly_returns` implementation.
///
/// Every evaluated equity/trade timestamp originates from a candle, so
/// validating the Train-through-Validation slice before signal or backtest
/// work proves the later UTC conversion cannot reach its historical
/// `.expect()` panic. Hidden Test candles are deliberately outside this slice.
fn validate_timestamps(candles: &[Candle]) -> Result<(), CandidateExecutionError> {
    for (index, candle) in candles.iter().enumerate() {
        if Utc
            .timestamp_millis_opt(candle.timestamp)
            .single()
            .is_none()
        {
            return fail(format!(
                "candles[{index}].timestamp {} is outside chrono's UTC millisecond range",
                candle.timestamp
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct CandidateStrategy {
    signal: ParamsSignalConfig,
    sl_pct: f64,
    tp_pct: f64,
    fee_pct: f64,
    slip_pct: f64,
    size_pct: f64,
    fill_mode: FillMode,
    direction: Direction,
}

impl CandidateStrategy {
    fn parse(value: &Value) -> Result<Self, CandidateExecutionError> {
        let object = value.as_object().ok_or_else(|| {
            CandidateExecutionError("candidate strategy must be an object".into())
        })?;
        let mode = string_field(object, "mode")?;
        if mode != "params" {
            return fail(format!(
                "candidate strategy mode must be params (received {mode})"
            ));
        }

        let signal = ParamsSignalConfig {
            fast_ma: period_field(object, "fastMA")?,
            slow_ma: period_field(object, "slowMA")?,
            ema_period: period_field(object, "emaPeriod")?,
            rsi_period: period_field(object, "rsiPeriod")?,
            rsi_buy: number_field(object, "rsiBuy")?,
            rsi_sell: number_field(object, "rsiSell")?,
            macd_fast: period_field(object, "macdFast")?,
            macd_slow: period_field(object, "macdSlow")?,
            macd_signal: period_field(object, "macdSignal")?,
            bb_period: period_field(object, "bbPeriod")?,
            bb_mult: number_field(object, "bbMult")?,
            entry_sig: string_field(object, "entrySig")?.to_string(),
            exit_sig: string_field(object, "exitSig")?.to_string(),
        };

        let fill_mode = match string_field(object, "fillMode")? {
            "close" => FillMode::Close,
            "nextOpen" => FillMode::NextOpen,
            other => return fail(format!("unsupported candidate fillMode \"{other}\"")),
        };
        let direction = match string_field(object, "direction")? {
            "long" => Direction::Long,
            "short" => Direction::Short,
            "both" => Direction::Both,
            other => return fail(format!("unsupported candidate direction \"{other}\"")),
        };

        Ok(Self {
            signal,
            sl_pct: number_field(object, "slPct")?,
            tp_pct: number_field(object, "tpPct")?,
            fee_pct: number_field(object, "feePct")?,
            slip_pct: number_field(object, "slipPct")?,
            size_pct: number_field(object, "sizePct")?,
            fill_mode,
            direction,
        })
    }

    fn backtest_config(
        &self,
        interval: &str,
        start_equity: f64,
        from: i64,
        to: i64,
    ) -> BacktestConfig {
        BacktestConfig {
            exec: ExecutionModel {
                direction: self.direction,
                sizing_pct: self.size_pct / 100.0,
                fill_mode: self.fill_mode,
            },
            cost: CostModel {
                fee_pct: self.fee_pct / 100.0,
                slippage_pct: self.slip_pct / 100.0,
            },
            risk: Some(RiskModel {
                stop_loss_pct: (self.sl_pct > 0.0).then_some(self.sl_pct / 100.0),
                take_profit_pct: (self.tp_pct > 0.0).then_some(self.tp_pct / 100.0),
            }),
            start_equity: Some(start_equity),
            bars_per_year: bars_per_year(interval),
            from: Some(from),
            to: Some(to),
        }
    }

    fn score_projection(&self) -> ParamsStrategyProjection {
        ParamsStrategyProjection {
            entry_sig: self.signal.entry_sig.clone(),
            exit_sig: self.signal.exit_sig.clone(),
            sl_pct: self.sl_pct,
            tp_pct: self.tp_pct,
        }
    }
}

fn string_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, CandidateExecutionError> {
    object.get(key).and_then(Value::as_str).ok_or_else(|| {
        CandidateExecutionError(format!("candidate strategy.{key} must be a string"))
    })
}

fn number_field(object: &Map<String, Value>, key: &str) -> Result<f64, CandidateExecutionError> {
    let value = object.get(key).and_then(Value::as_f64).ok_or_else(|| {
        CandidateExecutionError(format!("candidate strategy.{key} must be a number"))
    })?;
    if !value.is_finite() {
        return fail(format!("candidate strategy.{key} must be finite"));
    }
    Ok(value)
}

fn period_field(object: &Map<String, Value>, key: &str) -> Result<usize, CandidateExecutionError> {
    let value = number_field(object, key)?;
    if value < 1.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return fail(format!(
            "candidate strategy.{key} must be a positive integer representable as usize"
        ));
    }
    Ok(value as usize)
}

fn gate_overrides(config: GateConfig) -> GateConfigOverrides {
    GateConfigOverrides {
        min_trades: Some(config.min_trades as f64),
        min_avg_trade_return: Some(config.min_avg_trade_return),
        rolling_window_bars: Some(config.rolling_window_bars as f64),
        min_rolling_positive_ratio: Some(config.min_rolling_positive_ratio),
        max_drawdown: Some(config.max_drawdown),
        max_monthly_contribution: Some(config.max_monthly_contribution),
        max_single_trade_contribution: Some(config.max_single_trade_contribution),
        min_random_entry_percentile: Some(config.min_random_entry_percentile),
    }
}

fn score_overrides(config: ScoreConfig) -> ScoreConfigOverride {
    ScoreConfigOverride {
        caps: Some(ScoreCapsOverride {
            cagr: Some(config.caps.cagr),
            sortino: Some(config.caps.sortino),
            calmar: Some(config.caps.calmar),
            profit_factor: Some(config.caps.profit_factor),
            consistency_sigma_scale: Some(config.caps.consistency_sigma_scale),
            complexity_units: Some(config.caps.complexity_units),
            turnover: Some(config.caps.turnover),
            data_mining_log10: Some(config.caps.data_mining_log10),
        }),
        weights: Some(ScoreWeightsOverride {
            cagr: Some(config.weights.cagr),
            sortino: Some(config.weights.sortino),
            calmar: Some(config.weights.calmar),
            regime: Some(config.weights.regime),
            profit_factor: Some(config.weights.profit_factor),
            consistency: Some(config.weights.consistency),
            complexity: Some(config.weights.complexity),
            turnover: Some(config.weights.turnover),
            data_mining: Some(config.weights.data_mining),
        }),
    }
}

fn finite_number(value: f64, path: &str) -> Result<Value, CandidateExecutionError> {
    if !value.is_finite() {
        return fail(format!("{path} must be finite for JSON persistence"));
    }
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| CandidateExecutionError(format!("{path} is not JSON-safe")))
}

fn metric_status(value: f64) -> &'static str {
    if value.is_nan() {
        "nan"
    } else if value.is_sign_positive() {
        "positive_infinity"
    } else {
        "negative_infinity"
    }
}

fn insert_metric(
    values: &mut Map<String, Value>,
    statuses: &mut Map<String, Value>,
    key: &str,
    value: f64,
) {
    if value.is_finite() {
        // `from_f64` cannot fail after the finite check.
        values.insert(
            key.to_string(),
            Value::Number(Number::from_f64(value).expect("finite metric")),
        );
    } else {
        values.insert(key.to_string(), Value::Null);
        statuses.insert(
            key.to_string(),
            Value::String(metric_status(value).to_string()),
        );
    }
}

/// Rust mirror of `services/metricsCodec.ts::encodeMetrics`.
fn encode_metrics(metrics: &Metrics) -> Result<Value, CandidateExecutionError> {
    let mut values = Map::new();
    let mut statuses = Map::new();
    insert_metric(&mut values, &mut statuses, "netReturn", metrics.net_return);
    insert_metric(&mut values, &mut statuses, "cagr", metrics.cagr);
    insert_metric(
        &mut values,
        &mut statuses,
        "maxDrawdown",
        metrics.max_drawdown,
    );
    insert_metric(&mut values, &mut statuses, "sharpe", metrics.sharpe);
    insert_metric(&mut values, &mut statuses, "sortino", metrics.sortino);
    insert_metric(&mut values, &mut statuses, "calmar", metrics.calmar);
    insert_metric(&mut values, &mut statuses, "winRate", metrics.win_rate);
    values.insert(
        "tradeCount".into(),
        Value::Number(Number::from(metrics.trade_count)),
    );
    insert_metric(
        &mut values,
        &mut statuses,
        "profitFactor",
        metrics.profit_factor,
    );
    insert_metric(
        &mut values,
        &mut statuses,
        "avgTradeReturn",
        metrics.avg_trade_return,
    );
    insert_metric(
        &mut values,
        &mut statuses,
        "medianTradeReturn",
        metrics.median_trade_return,
    );
    insert_metric(
        &mut values,
        &mut statuses,
        "avgHoldingBars",
        metrics.avg_holding_bars,
    );
    insert_metric(&mut values, &mut statuses, "exposure", metrics.exposure);
    insert_metric(&mut values, &mut statuses, "turnover", metrics.turnover);
    insert_metric(
        &mut values,
        &mut statuses,
        "largestWin",
        metrics.largest_win,
    );
    insert_metric(
        &mut values,
        &mut statuses,
        "largestLoss",
        metrics.largest_loss,
    );
    values.insert(
        "consecutiveLosses".into(),
        Value::Number(Number::from(metrics.consecutive_losses)),
    );

    // The TypeScript codec clones monthlyReturns without a second status
    // channel, then its recursive JSON guard rejects a non-finite leaf.
    let mut monthly = Map::new();
    for (month, value) in &metrics.monthly_returns {
        monthly.insert(
            month.clone(),
            finite_number(*value, &format!("metrics.monthlyReturns.{month}"))?,
        );
    }
    values.insert("monthlyReturns".into(), Value::Object(monthly));

    let mut encoded = Map::new();
    encoded.insert("values".into(), Value::Object(values));
    encoded.insert("nonFinite".into(), Value::Object(statuses));
    Ok(Value::Object(encoded))
}

fn encode_random_entry(benchmark: &RandomEntryBenchmark) -> Result<Value, CandidateExecutionError> {
    let mut net_returns = Vec::with_capacity(benchmark.net_returns.len());
    for (index, value) in benchmark.net_returns.iter().enumerate() {
        net_returns.push(finite_number(
            *value,
            &format!("benchmark.randomEntry.netReturns[{index}]"),
        )?);
    }
    let mut record = Map::new();
    record.insert("runs".into(), Value::Number(Number::from(benchmark.runs)));
    record.insert("seed".into(), Value::Number(Number::from(benchmark.seed)));
    record.insert("netReturns".into(), Value::Array(net_returns));
    record.insert(
        "candidateNetReturn".into(),
        finite_number(
            benchmark.candidate_net_return,
            "benchmark.randomEntry.candidateNetReturn",
        )?,
    );
    record.insert(
        "candidatePercentile".into(),
        finite_number(
            benchmark.candidate_percentile,
            "benchmark.randomEntry.candidatePercentile",
        )?,
    );
    Ok(Value::Object(record))
}

fn to_json_value(value: &impl Serialize, label: &str) -> Result<Value, CandidateExecutionError> {
    serde_json::to_value(value).map_err(|error| context(error, label))
}

fn build_benchmark_record(
    config: &ResolvedDiscoveryConfig,
    dataset: ExecutionDataset<'_>,
    plan: &ValidationSplitPlan,
    benchmarks: &[BenchmarkRun],
    random_entry: &RandomEntryBenchmark,
) -> Result<Value, CandidateExecutionError> {
    let mut entries = Vec::with_capacity(DETERMINISTIC_BENCHMARK_IDS.len());
    for id in DETERMINISTIC_BENCHMARK_IDS {
        let run = benchmarks.iter().find(|run| run.id == id).ok_or_else(|| {
            CandidateExecutionError(format!("missing deterministic benchmark {id}"))
        })?;
        let mut entry = Map::new();
        entry.insert("id".into(), Value::String(id.to_string()));
        entry.insert(
            "strat".into(),
            match &run.strat {
                Some(strategy) => to_json_value(strategy, "benchmark strategy")?,
                None => Value::Null,
            },
        );
        entry.insert("metrics".into(), encode_metrics(&run.result.metrics)?);
        entries.push(Value::Object(entry));
    }

    let mut range = Map::new();
    range.insert(
        "from".into(),
        Value::Number(Number::from(plan.validation.from)),
    );
    range.insert("to".into(), Value::Number(Number::from(plan.validation.to)));

    let mut costs = Map::new();
    costs.insert(
        "feePct".into(),
        finite_number(config.benchmark_costs.fee_pct, "benchmark.costs.feePct")?,
    );
    costs.insert(
        "slipPct".into(),
        finite_number(config.benchmark_costs.slip_pct, "benchmark.costs.slipPct")?,
    );

    let mut record = Map::new();
    record.insert(
        "version".into(),
        Value::String(BENCHMARK_RECORD_VERSION.into()),
    );
    record.insert(
        "benchmarkContract".into(),
        Value::String(BENCHMARK_CONTRACT_VERSION.into()),
    );
    record.insert("interval".into(), Value::String(dataset.interval.into()));
    record.insert("validationRange".into(), Value::Object(range));
    record.insert(
        "startEquity".into(),
        finite_number(config.execution.start_equity, "benchmark.startEquity")?,
    );
    record.insert("costs".into(), Value::Object(costs));
    record.insert("benchmarks".into(), Value::Array(entries));
    record.insert("randomEntry".into(), encode_random_entry(random_entry)?);
    Ok(Value::Object(record))
}

fn summary_from_result(
    result: &BacktestResult,
    strategy_id: i64,
    dataset_id: i64,
    segment: &str,
) -> Result<BacktestSummary, CandidateExecutionError> {
    let first = result
        .equity
        .first()
        .ok_or_else(|| CandidateExecutionError(format!("{segment} equity must not be empty")))?;
    let last = result
        .equity
        .last()
        .ok_or_else(|| CandidateExecutionError(format!("{segment} equity must not be empty")))?;
    let metrics = &result.metrics;
    let finite = |value: f64| value.is_finite().then_some(value);

    Ok(BacktestSummary {
        id: None,
        strategy_id,
        dataset_id,
        segment: segment.to_string(),
        start_time: first.time,
        end_time: last.time,
        net_return: finite(metrics.net_return),
        cagr: finite(metrics.cagr),
        max_drawdown: finite(metrics.max_drawdown),
        sharpe: finite(metrics.sharpe),
        sortino: finite(metrics.sortino),
        calmar: finite(metrics.calmar),
        win_rate: finite(metrics.win_rate),
        trade_count: Some(metrics.trade_count),
        profit_factor: finite(metrics.profit_factor),
        avg_trade_return: finite(metrics.avg_trade_return),
        median_trade_return: finite(metrics.median_trade_return),
        exposure: finite(metrics.exposure),
        turnover: finite(metrics.turnover),
        largest_win: finite(metrics.largest_win),
        largest_loss: finite(metrics.largest_loss),
        consecutive_losses: Some(metrics.consecutive_losses),
        gate_passed: None,
        score: None,
        score_breakdown_json: None,
        benchmark_result_json: None,
        created_at: None,
    })
}

fn trade_rows(trades: &[ClosedTrade]) -> Vec<TradeRow> {
    trades
        .iter()
        .map(|trade| TradeRow {
            entry_time: trade.entry_time,
            exit_time: trade.exit_time,
            side: match trade.side {
                TradeSide::Long => "LONG",
                TradeSide::Short => "SHORT",
            }
            .to_string(),
            entry_price: trade.entry_price,
            exit_price: trade.exit_price,
            pnl: trade.pnl,
            pnl_pct: trade.pnl_pct,
            reason: None,
        })
        .collect()
}

struct RecordParts<'a> {
    strategy_id: i64,
    strategy_hash: &'a str,
    dataset: ExecutionDataset<'a>,
    embargo: &'a EmbargoDerivation,
    plan: &'a ValidationSplitPlan,
    train: &'a BacktestResult,
    validation: &'a BacktestResult,
    benchmark: &'a Value,
    gate: &'a GateVerdict,
    score: Option<&'a ScoreBreakdown>,
    tested_combinations: i64,
}

fn build_validation_record(parts: &RecordParts<'_>) -> Result<Value, CandidateExecutionError> {
    if parts.gate.pass != parts.score.is_some() {
        return fail("Gate pass state must exactly match Score presence");
    }
    if let Some(score) = parts.score {
        if score.tested_combinations.n != parts.tested_combinations as u64 {
            return fail("testedCombinations must match the Score evidence");
        }
    }

    let score_value = match parts.score {
        Some(score) => to_json_value(score, "score breakdown")?,
        None => Value::Null,
    };
    let encoded_gate = encode_gate_verdict(parts.gate);

    let mut contracts = Map::new();
    contracts.insert(
        "execution".into(),
        Value::String(EXECUTION_CONTRACT_VERSION.into()),
    );
    contracts.insert(
        "benchmark".into(),
        Value::String(BENCHMARK_CONTRACT_VERSION.into()),
    );
    contracts.insert(
        "metrics".into(),
        Value::String(METRICS_CONTRACT_VERSION.into()),
    );
    contracts.insert("gate".into(), Value::String(GATE_CONTRACT_VERSION.into()));
    contracts.insert(
        "score".into(),
        parts
            .score
            .map(|_| Value::String(SCORE_FORMULA_VERSION.into()))
            .unwrap_or(Value::Null),
    );

    let mut tested = Map::new();
    tested.insert(
        "n".into(),
        Value::Number(Number::from(parts.tested_combinations)),
    );
    tested.insert("basis".into(), Value::String("lineage-final-unique".into()));

    let mut record = Map::new();
    record.insert(
        "version".into(),
        Value::String(VALIDATION_RECORD_VERSION.into()),
    );
    record.insert("contracts".into(), Value::Object(contracts));
    record.insert(
        "strategyId".into(),
        Value::Number(Number::from(parts.strategy_id)),
    );
    record.insert(
        "strategyHash".into(),
        Value::String(parts.strategy_hash.into()),
    );
    record.insert(
        "datasetId".into(),
        Value::Number(Number::from(parts.dataset.id)),
    );
    record.insert(
        "datasetHash".into(),
        Value::String(parts.dataset.content_hash.into()),
    );
    record.insert(
        "embargo".into(),
        to_json_value(parts.embargo, "embargo record")?,
    );
    record.insert("splitPlan".into(), to_json_value(parts.plan, "split plan")?);
    record.insert("trainMetrics".into(), encode_metrics(&parts.train.metrics)?);
    record.insert(
        "validationMetrics".into(),
        encode_metrics(&parts.validation.metrics)?,
    );
    record.insert("benchmark".into(), parts.benchmark.clone());
    record.insert(
        "gate".into(),
        to_json_value(&encoded_gate, "encoded Gate verdict")?,
    );
    record.insert("gatePassed".into(), Value::Bool(parts.gate.pass));
    record.insert("score".into(), score_value);
    record.insert("testedCombinations".into(), Value::Object(tested));
    Ok(Value::Object(record))
}

/// Execute one candidate through the complete Train/Validation pipeline and
/// compose the immutable persistence bundle. No Test backtest is ever built.
pub fn execute_candidate(
    args: &ExecuteCandidateArgs<'_>,
) -> Result<CandidateExecutionOutput, CandidateExecutionError> {
    if args.strategy_id < 1 {
        return fail("strategy_id must be positive");
    }
    if args.dataset.id != args.config.dataset.id {
        return fail(format!(
            "dataset id mismatch: execution metadata has {}, config records {}",
            args.dataset.id, args.config.dataset.id
        ));
    }
    if args.dataset.content_hash != args.config.dataset.content_hash {
        return fail("dataset content hash does not match discovery config");
    }
    if !(1..=JS_MAX_SAFE_INTEGER).contains(&args.tested_combinations) {
        return fail("tested_combinations must be a positive JavaScript safe integer");
    }

    let total_bars = i64::try_from(args.candles.len())
        .map_err(|_| CandidateExecutionError("candle count exceeds i64".into()))?;
    let strategy = CandidateStrategy::parse(&args.candidate.strategy)?;
    if strategy.fee_pct != args.config.benchmark_costs.fee_pct
        || strategy.slip_pct != args.config.benchmark_costs.slip_pct
    {
        return fail("candidate costs do not match the resolved benchmark costs");
    }
    let computed_hash = strategy_hash(
        &args.candidate.strategy,
        strategy.fee_pct,
        strategy.slip_pct,
    )
    .map_err(|error| context(error, "candidate strategy identity"))?;
    if computed_hash != args.candidate.strategy_hash {
        return fail("candidate strategy content does not match its strategy_hash");
    }

    let embargo = derive_embargo_bars(&strategy.signal, args.config.embargo.holding_allowance_bars)
        .map_err(|error| context(error, "derive embargo"))?;
    let plan = plan_validation_split(total_bars, embargo.embargo_bars)
        .map_err(|error| context(error, "plan validation split"))?;
    let evaluation_len = usize::try_from(plan.validation.to)
        .ok()
        .and_then(|index| index.checked_add(1))
        .ok_or_else(|| {
            CandidateExecutionError(
                "validation range cannot be represented as a candle slice".into(),
            )
        })?;
    let evaluation_candles = args.candles.get(..evaluation_len).ok_or_else(|| {
        CandidateExecutionError("validation range exceeds the supplied candles".into())
    })?;

    // This MUST precede every evaluated path that can reach compute_metrics.
    // The hidden Test suffix is neither scanned nor passed to compute.
    validate_timestamps(evaluation_candles)?;
    let signals = build_params_signals(evaluation_candles, &strategy.signal)
        .map_err(|error| context(error, "build candidate signals"))?;

    let run_segment = |from: i64, to: i64| {
        run_backtest(
            evaluation_candles,
            &signals,
            &strategy.backtest_config(
                args.dataset.interval,
                args.config.execution.start_equity,
                from,
                to,
            ),
        )
        .map_err(|error| context(error, "run candidate backtest"))
    };
    let train = run_segment(plan.train.from, plan.train.to)?;
    let validation = run_segment(plan.validation.from, plan.validation.to)?;

    let benchmark_costs = BenchmarkCosts {
        fee_pct: args.config.benchmark_costs.fee_pct,
        slip_pct: args.config.benchmark_costs.slip_pct,
    };
    let benchmarks = run_deterministic_benchmarks(&RunBenchmarksArgs {
        candles: evaluation_candles,
        interval: args.dataset.interval,
        costs: benchmark_costs,
        start_equity: Some(args.config.execution.start_equity),
        from: Some(plan.validation.from),
        to: Some(plan.validation.to),
    })
    .map_err(|error| context(error, "run deterministic benchmarks"))?;

    // Do not manufacture evidence for a zero-trade candidate: the benchmark's
    // existing fail-closed error is the run failure evidence.
    let random_candidate = RandomEntryCandidate {
        trades: validation.trades.clone(),
        net_return: validation.metrics.net_return,
    };
    let random_entry = run_random_entry_benchmark(&RandomEntryArgs {
        candles: evaluation_candles,
        interval: args.dataset.interval,
        costs: benchmark_costs,
        candidate: &random_candidate,
        seed: i64::from(args.candidate.seeds.random_entry),
        runs: Some(args.config.random_entry.runs),
        start_equity: Some(args.config.execution.start_equity),
        from: Some(plan.validation.from),
        to: Some(plan.validation.to),
    })
    .map_err(|error| context(error, "run Random Entry benchmark"))?;

    let views = benchmark_views(&benchmarks);
    let gate = evaluate_gate(&EvaluateGateArgs {
        candidate: GateCandidateView::from(&validation),
        benchmarks: &views,
        random_entry: GateRandomEntryView::from(&random_entry),
        config: Some(gate_overrides(args.config.gate_config)),
    })
    .map_err(|error| context(error, "evaluate Gate"))?;

    let score_config = score_overrides(args.config.score_config);
    let score_projection = strategy.score_projection();
    let score = if gate.pass {
        Some(
            score_candidate(&ScoreCandidateArgs {
                candidate: ScoreCandidateView::from(&validation),
                strategy: &score_projection,
                tested_combinations: args.tested_combinations as f64,
                config: Some(&score_config),
            })
            .map_err(|error| context(error, "score candidate"))?,
        )
    } else {
        None
    };

    let benchmark_record =
        build_benchmark_record(args.config, args.dataset, &plan, &benchmarks, &random_entry)?;
    let validation_record = build_validation_record(&RecordParts {
        strategy_id: args.strategy_id,
        strategy_hash: &args.candidate.strategy_hash,
        dataset: args.dataset,
        embargo: &embargo,
        plan: &plan,
        train: &train,
        validation: &validation,
        benchmark: &benchmark_record,
        gate: &gate,
        score: score.as_ref(),
        tested_combinations: args.tested_combinations,
    })?;

    let score_value = score.as_ref().map(|breakdown| breakdown.score);
    let score_json = score
        .as_ref()
        .map(|breakdown| {
            serde_json::to_string(breakdown)
                .map_err(|error| context(error, "serialize score breakdown"))
        })
        .transpose()?;
    let benchmark_json = serde_json::to_string(&benchmark_record)
        .map_err(|error| context(error, "serialize benchmark record"))?;
    let record_json = serde_json::to_string(&validation_record)
        .map_err(|error| context(error, "serialize validation record"))?;

    let train_summary = summary_from_result(&train, args.strategy_id, args.dataset.id, "train")?;
    let mut validation_summary =
        summary_from_result(&validation, args.strategy_id, args.dataset.id, "validation")?;
    validation_summary.gate_passed = Some(gate.pass);
    validation_summary.score = score_value;
    validation_summary.score_breakdown_json = score_json;
    validation_summary.benchmark_result_json = Some(benchmark_json);

    let record = ValidationRecordRow {
        id: None,
        strategy_id: args.strategy_id,
        dataset_id: args.dataset.id,
        record_version: VALIDATION_RECORD_VERSION.into(),
        gate_passed: gate.pass,
        score: score_value,
        record_json,
        created_at: None,
    };

    Ok(CandidateExecutionOutput {
        train_summary,
        train_trades: trade_rows(&train.trades),
        validation_summary,
        validation_trades: trade_rows(&validation.trades),
        record,
        digest: CandidateResultDigest {
            gate_passed: gate.pass,
            score: score_value,
        },
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::repositories::validate_validation_bundle;
    use alpha_factor_forge::discovery_core::{
        config::parse_discovery_config, enumerate::enumerate_candidates,
    };
    use serde_json::json;

    const RUNNER_CONFIG_FIXTURE: &str =
        include_str!("../../../fixtures/rs-core/runner-config-v1.json");

    fn test_config_and_candidate() -> (ResolvedDiscoveryConfig, EnumeratedCandidate, i64) {
        let fixture: Value =
            serde_json::from_str(RUNNER_CONFIG_FIXTURE).expect("parse runner config fixture");
        let mut input = fixture["enumerationCases"][0]["input"].clone();
        input["embargo"]["holdingAllowanceBars"] = json!(0);
        input["randomEntry"]["runs"] = json!(5);
        input["gateConfig"]["maxMonthlyContribution"] = json!(1);
        input["bases"][0]["axes"] = json!([]);
        input["bases"][0]["strategy"]["fastMA"] = json!(1);
        input["bases"][0]["strategy"]["slowMA"] = json!(2);
        input["bases"][0]["strategy"]["entrySig"] = json!("priceBelowSlow");
        input["bases"][0]["strategy"]["exitSig"] = json!("priceAboveSlow");

        let config = parse_discovery_config(&input, 4.0).expect("admit test config");
        let plan = enumerate_candidates(&config).expect("enumerate test candidate");
        assert_eq!(plan.candidates.len(), 1);
        (
            config,
            plan.candidates[0].clone(),
            plan.tested_combinations.n,
        )
    }

    fn alternating_candles(count: usize) -> Vec<Candle> {
        let start = 1_577_836_800_000i64;
        (0..count)
            .map(|index| {
                let close = if index % 2 == 0 { 110.0 } else { 90.0 };
                Candle {
                    timestamp: start + index as i64 * 86_400_000,
                    open: close,
                    high: close + 1.0,
                    low: close - 1.0,
                    close,
                    volume: 1_000.0,
                }
            })
            .collect()
    }

    pub(crate) fn representative_output() -> CandidateExecutionOutput {
        let (config, candidate, tested_combinations) = test_config_and_candidate();
        let candles = alternating_candles(600);
        execute_candidate(&ExecuteCandidateArgs {
            config: &config,
            candidate: &candidate,
            tested_combinations,
            strategy_id: 11,
            dataset: ExecutionDataset {
                id: config.dataset.id,
                content_hash: &config.dataset.content_hash,
                interval: "1d",
            },
            candles: &candles,
        })
        .expect("execute representative candidate")
    }

    #[test]
    fn rejects_a_chrono_invalid_timestamp_before_execution() {
        let (config, candidate, tested_combinations) = test_config_and_candidate();
        let mut candles = alternating_candles(600);
        candles[0].timestamp = i64::MAX;
        let result = execute_candidate(&ExecuteCandidateArgs {
            config: &config,
            candidate: &candidate,
            tested_combinations,
            strategy_id: 1,
            dataset: ExecutionDataset {
                id: config.dataset.id,
                content_hash: &config.dataset.content_hash,
                interval: "1d",
            },
            candles: &candles,
        });
        let error = match result {
            Ok(_) => panic!("timestamp must fail closed"),
            Err(error) => error,
        };
        assert!(error.0.contains("outside chrono's UTC millisecond range"));
    }

    #[test]
    fn hidden_test_timestamp_is_not_read_by_candidate_execution() {
        let (config, candidate, tested_combinations) = test_config_and_candidate();
        let mut candles = alternating_candles(600);
        candles.last_mut().expect("hidden Test candle").timestamp = i64::MAX;

        execute_candidate(&ExecuteCandidateArgs {
            config: &config,
            candidate: &candidate,
            tested_combinations,
            strategy_id: 11,
            dataset: ExecutionDataset {
                id: config.dataset.id,
                content_hash: &config.dataset.content_hash,
                interval: "1d",
            },
            candles: &candles,
        })
        .expect("hidden Test candle content must not affect Train/Validation execution");
    }

    #[test]
    fn rejects_tested_combinations_above_the_javascript_safe_integer_boundary() {
        let (config, candidate, _) = test_config_and_candidate();
        let candles = alternating_candles(600);
        let result = execute_candidate(&ExecuteCandidateArgs {
            config: &config,
            candidate: &candidate,
            tested_combinations: JS_MAX_SAFE_INTEGER + 1,
            strategy_id: 11,
            dataset: ExecutionDataset {
                id: config.dataset.id,
                content_hash: &config.dataset.content_hash,
                interval: "1d",
            },
            candles: &candles,
        });
        let error = match result {
            Ok(_) => panic!("unsafe testedCombinations must fail before Gate or Score"),
            Err(error) => error,
        };

        assert!(error.0.contains("positive JavaScript safe integer"));
    }

    #[test]
    fn produced_bundle_passes_the_persist_validator() {
        let (config, candidate, tested_combinations) = test_config_and_candidate();
        let candles = alternating_candles(600);
        let output = execute_candidate(&ExecuteCandidateArgs {
            config: &config,
            candidate: &candidate,
            tested_combinations,
            strategy_id: 11,
            dataset: ExecutionDataset {
                id: config.dataset.id,
                content_hash: &config.dataset.content_hash,
                interval: "1d",
            },
            candles: &candles,
        })
        .expect("execute representative candidate");

        validate_validation_bundle(
            &output.train_summary,
            &output.validation_summary,
            &output.record,
        )
        .expect("composer output must satisfy PERSIST-001");
        assert_eq!(
            output.validation_summary.score.is_some(),
            output.digest.gate_passed
        );
        assert_eq!(output.record.score, output.digest.score);
        assert!(output
            .record
            .record_json
            .contains("\"version\":\"validation-record-v2\""));
    }

    fn collect_required_field_paths(value: &Value, path: &str, paths: &mut Vec<String>) {
        match value {
            Value::Object(object) => {
                // Map entries are evidence values, not statically named DTO fields.
                if path.ends_with("/monthlyReturns") || path.ends_with("/nonFinite") {
                    return;
                }
                for (key, child) in object {
                    let child_path = format!("{path}/{key}");
                    paths.push(child_path.clone());
                    collect_required_field_paths(child, &child_path, paths);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    collect_required_field_paths(child, &format!("{path}/{index}"), paths);
                }
            }
            _ => {}
        }
    }

    fn remove_object_field(value: &mut Value, pointer: &str) {
        let (parent_pointer, key) = pointer.rsplit_once('/').expect("field JSON pointer");
        value
            .pointer_mut(parent_pointer)
            .and_then(Value::as_object_mut)
            .expect("field parent object")
            .remove(key)
            .expect("required field exists");
    }

    #[test]
    fn every_present_v2_dto_field_is_required_before_persistence() {
        let output = representative_output();
        let envelope: Value = serde_json::from_str(&output.record.record_json).unwrap();
        let mut paths = Vec::new();
        collect_required_field_paths(&envelope, "", &mut paths);
        assert!(
            paths.len() > 150,
            "the mutation matrix must cover the full nested DTO"
        );

        for path in paths {
            let mut mutant = envelope.clone();
            remove_object_field(&mut mutant, &path);
            let record = ValidationRecordRow {
                id: None,
                strategy_id: output.record.strategy_id,
                dataset_id: output.record.dataset_id,
                record_version: output.record.record_version.clone(),
                gate_passed: output.record.gate_passed,
                score: output.record.score,
                record_json: serde_json::to_string(&mutant).unwrap(),
                created_at: None,
            };
            assert!(
                validate_validation_bundle(
                    &output.train_summary,
                    &output.validation_summary,
                    &record,
                )
                .is_err(),
                "removing required field {path} must fail closed"
            );
        }
    }

    #[test]
    fn rejects_legacy_and_unknown_record_versions_on_new_writes() {
        for version in ["validation-record-v1", "validation-record-v999"] {
            let output = representative_output();
            let record = ValidationRecordRow {
                id: None,
                strategy_id: output.record.strategy_id,
                dataset_id: output.record.dataset_id,
                record_version: version.into(),
                gate_passed: output.record.gate_passed,
                score: output.record.score,
                record_json: output.record.record_json,
                created_at: None,
            };
            let error = validate_validation_bundle(
                &output.train_summary,
                &output.validation_summary,
                &record,
            )
            .expect_err("unsupported write version must fail closed");
            let message = error.to_string();
            assert!(
                message.contains("validation-record") || message.contains("unsupported"),
                "version dispatch error should be explicit: {message}"
            );
        }
    }
}
