//! `gate-v1`: pure Rust parity port of the hard elimination Gate in
//! `src/services/gate.ts`.
//!
//! This module only judges already-computed candidate and benchmark evidence.
//! It performs no backtests and has no Tauri, runner, database, thread, event,
//! or UI dependency. The lightweight views keep the Gate independent from
//! persistence DTOs while adapters project the existing Rust computation
//! results into the exact evidence it reads.

use serde::{Deserialize, Serialize};

use super::backtest::BacktestResult;
use super::benchmarks::{BenchmarkRun, DETERMINISTIC_BENCHMARK_IDS};
use super::metrics::EquityPoint;
use super::random_entry::RandomEntryBenchmark;

pub const GATE_CONTRACT_VERSION: &str = "gate-v1";
const JS_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const JS_DATE_TIME_CLIP_MS: f64 = 8_640_000_000_000_000.0;
const MILLIS_PER_DAY: i64 = 86_400_000;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateConfig {
    pub min_trades: i64,
    pub min_avg_trade_return: f64,
    pub rolling_window_bars: i64,
    pub min_rolling_positive_ratio: f64,
    pub max_drawdown: f64,
    pub max_monthly_contribution: f64,
    pub max_single_trade_contribution: f64,
    pub min_random_entry_percentile: f64,
}

pub const DEFAULT_GATE_CONFIG: GateConfig = GateConfig {
    min_trades: 30,
    min_avg_trade_return: 0.0,
    rolling_window_bars: 30,
    min_rolling_positive_ratio: 0.55,
    max_drawdown: 0.35,
    max_monthly_contribution: 0.40,
    max_single_trade_contribution: 0.25,
    min_random_entry_percentile: 95.0,
};

impl Default for GateConfig {
    fn default() -> Self {
        DEFAULT_GATE_CONFIG
    }
}

/// TypeScript's `Partial<GateConfig>` boundary. Integer-valued fields remain
/// `f64` here so NaN, fractional values, and the JavaScript safe-integer edge
/// can be rejected with the reference implementation's own error messages.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateConfigOverrides {
    #[serde(default)]
    pub min_trades: Option<f64>,
    #[serde(default)]
    pub min_avg_trade_return: Option<f64>,
    #[serde(default)]
    pub rolling_window_bars: Option<f64>,
    #[serde(default)]
    pub min_rolling_positive_ratio: Option<f64>,
    #[serde(default)]
    pub max_drawdown: Option<f64>,
    #[serde(default)]
    pub max_monthly_contribution: Option<f64>,
    #[serde(default)]
    pub max_single_trade_contribution: Option<f64>,
    #[serde(default)]
    pub min_random_entry_percentile: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateTradeEvidence {
    pub exit_time: f64,
    pub pnl: f64,
}

#[derive(Clone, Debug)]
pub struct GateCandidateView<'a> {
    pub trades: Vec<GateTradeEvidence>,
    pub equity: &'a [EquityPoint],
    pub net_return: f64,
    /// Deliberately `f64`: parity must retain malformed NaN, fractional, and
    /// above-MAX_SAFE inputs until the Gate fails them closed.
    pub trade_count: f64,
    pub avg_trade_return: f64,
    pub max_drawdown: f64,
}

impl<'a> From<&'a BacktestResult> for GateCandidateView<'a> {
    fn from(result: &'a BacktestResult) -> Self {
        Self {
            trades: result
                .trades
                .iter()
                .map(|trade| GateTradeEvidence {
                    exit_time: trade.exit_time as f64,
                    pnl: trade.pnl,
                })
                .collect(),
            equity: &result.equity,
            net_return: result.metrics.net_return,
            trade_count: result.metrics.trade_count as f64,
            avg_trade_return: result.metrics.avg_trade_return,
            max_drawdown: result.metrics.max_drawdown,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GateBenchmarkView<'a> {
    pub id: &'a str,
    pub net_return: f64,
}

impl<'a> From<&'a BenchmarkRun> for GateBenchmarkView<'a> {
    fn from(run: &'a BenchmarkRun) -> Self {
        Self {
            id: run.id,
            net_return: run.result.metrics.net_return,
        }
    }
}

pub fn benchmark_views(runs: &[BenchmarkRun]) -> Vec<GateBenchmarkView<'_>> {
    runs.iter().map(GateBenchmarkView::from).collect()
}

#[derive(Clone, Copy, Debug)]
pub struct GateRandomEntryView {
    pub candidate_percentile: f64,
}

impl From<&RandomEntryBenchmark> for GateRandomEntryView {
    fn from(result: &RandomEntryBenchmark) -> Self {
        Self {
            candidate_percentile: result.candidate_percentile,
        }
    }
}

pub struct EvaluateGateArgs<'a> {
    pub candidate: GateCandidateView<'a>,
    pub benchmarks: &'a [GateBenchmarkView<'a>],
    pub random_entry: GateRandomEntryView,
    pub config: Option<GateConfigOverrides>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GateCriterionId {
    #[serde(rename = "minTrades")]
    MinTrades,
    #[serde(rename = "avgTradeReturn")]
    AvgTradeReturn,
    #[serde(rename = "rollingConsistency")]
    RollingConsistency,
    #[serde(rename = "maxDrawdown")]
    MaxDrawdown,
    #[serde(rename = "monthlyConcentration")]
    MonthlyConcentration,
    #[serde(rename = "tradeConcentration")]
    TradeConcentration,
    #[serde(rename = "benchmarkWins")]
    BenchmarkWins,
    #[serde(rename = "randomEntryPercentile")]
    RandomEntryPercentile,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateCriterion {
    pub id: GateCriterionId,
    pub pass: bool,
    pub value: Option<f64>,
    pub threshold: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GateVerdict {
    pub pass: bool,
    pub criteria: Vec<GateCriterion>,
    pub config: GateConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GateValueStatus {
    #[serde(rename = "positive_infinity")]
    PositiveInfinity,
    #[serde(rename = "negative_infinity")]
    NegativeInfinity,
    #[serde(rename = "nan")]
    Nan,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodedGateCriterion {
    pub id: GateCriterionId,
    pub pass: bool,
    /// Finite observed value, or null for insufficient/non-finite evidence.
    pub value: Option<f64>,
    /// Null for finite and insufficient values; exact status when non-finite.
    pub value_status: Option<GateValueStatus>,
    pub threshold: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EncodedGateVerdict {
    pub version: &'static str,
    pub pass: bool,
    pub criteria: Vec<EncodedGateCriterion>,
    pub config: GateConfig,
}

pub fn encode_gate_verdict(verdict: &GateVerdict) -> EncodedGateVerdict {
    let criteria = verdict
        .criteria
        .iter()
        .map(|criterion| {
            let (value, value_status) = match criterion.value {
                None => (None, None),
                Some(value) if value.is_finite() => (Some(value), None),
                Some(value) if value.is_nan() => (None, Some(GateValueStatus::Nan)),
                Some(value) if value.is_sign_positive() => {
                    (None, Some(GateValueStatus::PositiveInfinity))
                }
                Some(_) => (None, Some(GateValueStatus::NegativeInfinity)),
            };
            EncodedGateCriterion {
                id: criterion.id,
                pass: criterion.pass,
                value,
                value_status,
                threshold: criterion.threshold,
                detail: criterion.detail.clone(),
            }
        })
        .collect();

    EncodedGateVerdict {
        version: GATE_CONTRACT_VERSION,
        pass: verdict.pass,
        criteria,
        config: verdict.config,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GateError(pub String);

impl std::fmt::Display for GateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GateError {}

fn is_safe_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0 && value.abs() <= JS_MAX_SAFE_INTEGER
}

fn is_positive_safe_integer(value: f64) -> bool {
    is_safe_integer(value) && value >= 1.0
}

fn assert_fraction(value: f64, name: &str) -> Result<(), GateError> {
    if !value.is_finite() || value <= 0.0 || value > 1.0 {
        return Err(GateError(format!("{name} must be a fraction in (0, 1]")));
    }
    Ok(())
}

pub fn resolve_gate_config(
    overrides: Option<&GateConfigOverrides>,
) -> Result<GateConfig, GateError> {
    let overrides = overrides.copied().unwrap_or_default();
    let min_trades = overrides
        .min_trades
        .unwrap_or(DEFAULT_GATE_CONFIG.min_trades as f64);
    let min_avg_trade_return = overrides
        .min_avg_trade_return
        .unwrap_or(DEFAULT_GATE_CONFIG.min_avg_trade_return);
    let rolling_window_bars = overrides
        .rolling_window_bars
        .unwrap_or(DEFAULT_GATE_CONFIG.rolling_window_bars as f64);
    let min_rolling_positive_ratio = overrides
        .min_rolling_positive_ratio
        .unwrap_or(DEFAULT_GATE_CONFIG.min_rolling_positive_ratio);
    let max_drawdown = overrides
        .max_drawdown
        .unwrap_or(DEFAULT_GATE_CONFIG.max_drawdown);
    let max_monthly_contribution = overrides
        .max_monthly_contribution
        .unwrap_or(DEFAULT_GATE_CONFIG.max_monthly_contribution);
    let max_single_trade_contribution = overrides
        .max_single_trade_contribution
        .unwrap_or(DEFAULT_GATE_CONFIG.max_single_trade_contribution);
    let min_random_entry_percentile = overrides
        .min_random_entry_percentile
        .unwrap_or(DEFAULT_GATE_CONFIG.min_random_entry_percentile);

    // Keep the validation order and messages exactly aligned with TypeScript.
    if !is_positive_safe_integer(min_trades) {
        return Err(GateError("minTrades must be a positive integer".into()));
    }
    if !min_avg_trade_return.is_finite() {
        return Err(GateError(
            "minAvgTradeReturn must be a finite number".into(),
        ));
    }
    if !is_positive_safe_integer(rolling_window_bars) {
        return Err(GateError(
            "rollingWindowBars must be a positive integer".into(),
        ));
    }
    assert_fraction(min_rolling_positive_ratio, "minRollingPositiveRatio")?;
    assert_fraction(max_drawdown, "maxDrawdown")?;
    assert_fraction(max_monthly_contribution, "maxMonthlyContribution")?;
    assert_fraction(max_single_trade_contribution, "maxSingleTradeContribution")?;
    if !min_random_entry_percentile.is_finite()
        || !(0.0..=100.0).contains(&min_random_entry_percentile)
    {
        return Err(GateError(
            "minRandomEntryPercentile must be in [0, 100]".into(),
        ));
    }

    Ok(GateConfig {
        min_trades: min_trades as i64,
        min_avg_trade_return,
        rolling_window_bars: rolling_window_bars as i64,
        min_rolling_positive_ratio,
        max_drawdown,
        max_monthly_contribution,
        max_single_trade_contribution,
        min_random_entry_percentile,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RollingEvidenceError {
    NonFinite,
    Insufficient,
}

fn rolling_positive_ratio_evidence(
    equity: &[EquityPoint],
    window_bars: usize,
) -> Result<f64, RollingEvidenceError> {
    if equity.iter().any(|point| !point.equity.is_finite()) {
        return Err(RollingEvidenceError::NonFinite);
    }
    let windows = equity
        .len()
        .checked_sub(window_bars)
        .ok_or(RollingEvidenceError::Insufficient)?;
    if windows < 1 {
        return Err(RollingEvidenceError::Insufficient);
    }
    let positive = (0..windows)
        .filter(|index| equity[index + window_bars].equity > equity[*index].equity)
        .count();
    Ok(positive as f64 / windows as f64)
}

/// Fraction of step-1 windows whose ending equity is strictly above their
/// starting equity. Any non-finite point or insufficient history returns None.
pub fn rolling_positive_ratio(equity: &[EquityPoint], window_bars: usize) -> Option<f64> {
    rolling_positive_ratio_evidence(equity, window_bars).ok()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContributionError {
    NonFiniteProfit,
    NonPositiveProfit,
}

fn max_contribution(values: impl IntoIterator<Item = f64>) -> Result<f64, ContributionError> {
    let mut total = 0.0;
    let mut largest = 0.0f64;
    for value in values {
        if !value.is_finite() {
            return Err(ContributionError::NonFiniteProfit);
        }
        total += value;
        largest = largest.max(value);
    }
    if !total.is_finite() {
        return Err(ContributionError::NonFiniteProfit);
    }
    if total <= 0.0 {
        return Err(ContributionError::NonPositiveProfit);
    }
    let contribution = largest / total;
    if !contribution.is_finite() {
        return Err(ContributionError::NonFiniteProfit);
    }
    Ok(contribution)
}

/// Mirror JavaScript `new Date(epochMs)` + UTC year/month. Date's TimeClip
/// truncates fractional milliseconds toward zero and rejects non-finite values
/// or magnitudes above 8.64e15 ms.
fn utc_month(epoch_ms: f64) -> Option<String> {
    if !epoch_ms.is_finite() || epoch_ms.abs() > JS_DATE_TIME_CLIP_MS {
        return None;
    }

    let days_since_epoch = (epoch_ms.trunc() as i64).div_euclid(MILLIS_PER_DAY);

    // Howard Hinnant's civil_from_days, using the proleptic Gregorian calendar
    // that ECMAScript Date uses. The input range is only +/-100,000,000 days.
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    Some(format!("{year}-{month:02}"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MonthlyContributionError {
    InvalidExitTime,
    Contribution(ContributionError),
}

fn monthly_contribution(trades: &[GateTradeEvidence]) -> Result<f64, MonthlyContributionError> {
    let months: Vec<String> = trades
        .iter()
        .map(|trade| utc_month(trade.exit_time).ok_or(MonthlyContributionError::InvalidExitTime))
        .collect::<Result<_, _>>()?;

    // Vec preserves the TypeScript Map's first-seen month order, including
    // the floating-point accumulation order used by the final total.
    let mut monthly: Vec<(String, f64)> = Vec::new();
    for (trade, month) in trades.iter().zip(months) {
        if !trade.pnl.is_finite() {
            return Err(MonthlyContributionError::Contribution(
                ContributionError::NonFiniteProfit,
            ));
        }
        if let Some((_, pnl)) = monthly.iter_mut().find(|(key, _)| key == &month) {
            *pnl += trade.pnl;
            if !pnl.is_finite() {
                return Err(MonthlyContributionError::Contribution(
                    ContributionError::NonFiniteProfit,
                ));
            }
        } else {
            monthly.push((month, trade.pnl));
        }
    }
    max_contribution(monthly.into_iter().map(|(_, pnl)| pnl))
        .map_err(MonthlyContributionError::Contribution)
}

pub fn evaluate_gate(args: &EvaluateGateArgs<'_>) -> Result<GateVerdict, GateError> {
    let config = resolve_gate_config(args.config.as_ref())?;

    let duplicates: Vec<&str> = DETERMINISTIC_BENCHMARK_IDS
        .iter()
        .copied()
        .filter(|id| {
            args.benchmarks
                .iter()
                .filter(|benchmark| benchmark.id == *id)
                .count()
                > 1
        })
        .collect();
    if !duplicates.is_empty() {
        return Err(GateError(format!(
            "duplicate deterministic benchmark(s): {}",
            duplicates.join(", ")
        )));
    }

    let missing: Vec<&str> = DETERMINISTIC_BENCHMARK_IDS
        .iter()
        .copied()
        .filter(|id| !args.benchmarks.iter().any(|benchmark| benchmark.id == *id))
        .collect();
    if !missing.is_empty() {
        return Err(GateError(format!(
            "missing deterministic benchmark(s): {}",
            missing.join(", ")
        )));
    }

    let candidate = &args.candidate;
    let mut criteria = Vec::with_capacity(8);

    criteria.push(GateCriterion {
        id: GateCriterionId::MinTrades,
        pass: is_safe_integer(candidate.trade_count)
            && candidate.trade_count >= 0.0
            && candidate.trade_count >= config.min_trades as f64,
        value: Some(candidate.trade_count),
        threshold: config.min_trades as f64,
        detail: None,
    });

    criteria.push(GateCriterion {
        id: GateCriterionId::AvgTradeReturn,
        pass: candidate.avg_trade_return.is_finite()
            && candidate.avg_trade_return > config.min_avg_trade_return,
        value: Some(candidate.avg_trade_return),
        threshold: config.min_avg_trade_return,
        detail: None,
    });

    let rolling_evidence = usize::try_from(config.rolling_window_bars)
        .map_err(|_| RollingEvidenceError::Insufficient)
        .and_then(|window| rolling_positive_ratio_evidence(candidate.equity, window));
    let (rolling, rolling_detail) = match rolling_evidence {
        Ok(value) => (Some(value), None),
        Err(RollingEvidenceError::NonFinite) => {
            (None, Some("non-finite equity evidence".to_string()))
        }
        Err(RollingEvidenceError::Insufficient) => (
            None,
            Some(format!(
                "equity curve shorter than one {}-bar window",
                config.rolling_window_bars
            )),
        ),
    };
    criteria.push(GateCriterion {
        id: GateCriterionId::RollingConsistency,
        pass: rolling.is_some_and(|value| value >= config.min_rolling_positive_ratio),
        value: rolling,
        threshold: config.min_rolling_positive_ratio,
        detail: rolling_detail,
    });

    criteria.push(GateCriterion {
        id: GateCriterionId::MaxDrawdown,
        pass: candidate.max_drawdown.is_finite() && candidate.max_drawdown <= config.max_drawdown,
        value: Some(candidate.max_drawdown),
        threshold: config.max_drawdown,
        detail: None,
    });

    let (monthly, monthly_detail) = match monthly_contribution(&candidate.trades) {
        Ok(value) => (Some(value), None),
        Err(MonthlyContributionError::InvalidExitTime) => {
            (None, Some("invalid trade exit-time evidence".to_string()))
        }
        Err(MonthlyContributionError::Contribution(ContributionError::NonFiniteProfit)) => {
            (None, Some("non-finite profit evidence".to_string()))
        }
        Err(MonthlyContributionError::Contribution(ContributionError::NonPositiveProfit)) => (
            None,
            Some("no positive total profit to attribute".to_string()),
        ),
    };
    criteria.push(GateCriterion {
        id: GateCriterionId::MonthlyConcentration,
        pass: monthly.is_some_and(|value| value <= config.max_monthly_contribution),
        value: monthly,
        threshold: config.max_monthly_contribution,
        detail: monthly_detail,
    });

    let (per_trade, per_trade_detail) =
        match max_contribution(candidate.trades.iter().map(|trade| trade.pnl)) {
            Ok(value) => (Some(value), None),
            Err(ContributionError::NonFiniteProfit) => {
                (None, Some("non-finite profit evidence".to_string()))
            }
            Err(ContributionError::NonPositiveProfit) => (
                None,
                Some("no positive total profit to attribute".to_string()),
            ),
        };
    criteria.push(GateCriterion {
        id: GateCriterionId::TradeConcentration,
        pass: per_trade.is_some_and(|value| value <= config.max_single_trade_contribution),
        value: per_trade,
        threshold: config.max_single_trade_contribution,
        detail: per_trade_detail,
    });

    let lost: Vec<&str> = DETERMINISTIC_BENCHMARK_IDS
        .iter()
        .copied()
        .filter(|id| {
            let benchmark_return = args
                .benchmarks
                .iter()
                .find(|benchmark| benchmark.id == *id)
                .expect("complete unique benchmark set validated")
                .net_return;
            !(candidate.net_return.is_finite()
                && benchmark_return.is_finite()
                && candidate.net_return > benchmark_return)
        })
        .collect();
    criteria.push(GateCriterion {
        id: GateCriterionId::BenchmarkWins,
        pass: lost.is_empty(),
        value: Some((DETERMINISTIC_BENCHMARK_IDS.len() - lost.len()) as f64),
        threshold: DETERMINISTIC_BENCHMARK_IDS.len() as f64,
        detail: (!lost.is_empty()).then(|| format!("not beaten: {}", lost.join(", "))),
    });

    let percentile = args.random_entry.candidate_percentile;
    criteria.push(GateCriterion {
        id: GateCriterionId::RandomEntryPercentile,
        pass: percentile.is_finite() && percentile >= config.min_random_entry_percentile,
        value: Some(percentile),
        threshold: config.min_random_entry_percentile,
        detail: None,
    });

    Ok(GateVerdict {
        pass: criteria.iter().all(|criterion| criterion.pass),
        criteria,
        config,
    })
}
