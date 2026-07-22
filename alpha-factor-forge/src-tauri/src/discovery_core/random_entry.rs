//! `random-entry-v1`: pure Rust parity port of the Random Entry Monte Carlo
//! benchmark in `src/services/randomEntry.ts` (BENCH-002).
//!
//! One `mulberry32` stream consumed in the exact reference order (per run: k
//! durations, then k + 1 gap weights), placing the candidate's trade count at
//! random non-overlapping positions with holding periods resampled from the
//! candidate's own closed-trade `bars`. Long-only, 100% sizing, close fill,
//! inherited costs. Fails closed on the same inputs as the reference.

use serde::Deserialize;

use super::backtest::{
    run_backtest, BacktestConfig, BacktestError, CostModel, Direction, ExecutionModel, FillMode,
    Signals,
};
use super::benchmarks::{bars_per_year, BenchmarkCosts};
use super::metrics::ClosedTrade;
use super::prng::Mulberry32;
use super::types::Candle;

pub const RANDOM_ENTRY_CONTRACT_VERSION: &str = "random-entry-v1";
pub const DEFAULT_RANDOM_ENTRY_RUNS: i64 = 200;
pub const MAX_RANDOM_ENTRY_RUNS: i64 = 1000;
const JS_MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlannedRandomTrade {
    pub entry_idx: i64,
    /// `None` when the trade is clipped by the segment end (EOD settles it).
    pub exit_idx: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomEntryCandidate {
    pub trades: Vec<ClosedTrade>,
    pub net_return: f64,
}

pub struct RandomEntryArgs<'a> {
    pub candles: &'a [Candle],
    pub interval: &'a str,
    pub costs: BenchmarkCosts,
    pub candidate: &'a RandomEntryCandidate,
    pub seed: i64,
    pub runs: Option<i64>,
    pub start_equity: Option<f64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

pub struct RandomEntryBenchmark {
    pub runs: i64,
    pub seed: i64,
    pub net_returns: Vec<f64>,
    pub candidate_net_return: f64,
    pub candidate_percentile: f64,
}

/// Plan one run's non-overlapping random trades inside `[from, to]`
/// (inclusive). Consumes `rng` in the fixed order: `trade_count` durations,
/// then `trade_count + 1` gap weights.
pub(super) fn plan_random_trades(
    rng: &mut Mulberry32,
    from: i64,
    to: i64,
    holding_pool: &[i64],
    trade_count: i64,
) -> Vec<PlannedRandomTrade> {
    let mut durations: Vec<i64> = Vec::with_capacity(trade_count.max(0) as usize);
    for _ in 0..trade_count {
        let index = (rng.next_f64() * holding_pool.len() as f64).floor() as usize;
        durations.push(holding_pool[index]);
    }
    let mut weights: Vec<f64> = Vec::with_capacity((trade_count + 1).max(0) as usize);
    for _ in 0..(trade_count + 1) {
        weights.push(rng.next_f64());
    }

    let segment_bars = to - from + 1;
    let occupied: i64 = durations.iter().map(|d| d + 1).sum();
    let free = (segment_bars - occupied).max(0);
    let total_weight: f64 = weights.iter().sum();
    let gap = |j: usize| -> i64 {
        if total_weight > 0.0 {
            (weights[j] / total_weight * free as f64).floor() as i64
        } else {
            0
        }
    };

    let mut planned = Vec::new();
    let mut cursor = from + gap(0);
    for (j, duration) in durations.iter().enumerate() {
        if cursor > to {
            break; // no room left — later trades drop
        }
        let exit_idx = cursor + *duration;
        if exit_idx > to {
            planned.push(PlannedRandomTrade {
                entry_idx: cursor,
                exit_idx: None,
            }); // clipped: EOD settles
            break;
        }
        planned.push(PlannedRandomTrade {
            entry_idx: cursor,
            exit_idx: Some(exit_idx),
        });
        cursor = exit_idx + 1 + gap(j + 1);
    }
    planned
}

fn exec_cost_fractions(costs: BenchmarkCosts) -> (f64, f64, f64) {
    (
        costs.fee_pct.max(0.0) / 100.0,
        costs.slip_pct.max(0.0) / 100.0,
        1.0,
    )
}

/// Run the Random Entry Monte Carlo benchmark. Fails closed on an empty
/// series/segment, a candidate without closed trades, or invalid runs/seed.
pub fn run_random_entry_benchmark(
    args: &RandomEntryArgs<'_>,
) -> Result<RandomEntryBenchmark, BacktestError> {
    let runs = args.runs.unwrap_or(DEFAULT_RANDOM_ENTRY_RUNS);
    if !(1..=MAX_RANDOM_ENTRY_RUNS).contains(&runs) {
        return Err(BacktestError(format!(
            "runs must be an integer in [1, {MAX_RANDOM_ENTRY_RUNS}]"
        )));
    }
    if !(0..=JS_MAX_SAFE_INTEGER).contains(&args.seed) {
        return Err(BacktestError(
            "seed must be a non-negative safe integer".into(),
        ));
    }
    if args.candles.is_empty() {
        return Err(BacktestError(
            "random entry benchmark needs a non-empty candle series".into(),
        ));
    }
    let last_index = args.candles.len() as i64 - 1;
    let from = args.from.unwrap_or(0).max(0);
    let to = args.to.unwrap_or(last_index).min(last_index);
    if to < from {
        return Err(BacktestError(
            "random entry benchmark needs a non-empty [from, to] segment".into(),
        ));
    }
    if args.candidate.trades.is_empty() {
        return Err(BacktestError(
            "random entry benchmark needs at least one closed candidate trade for the holding-period pool".into(),
        ));
    }

    // Same-bar candidate exits (bars 0) cannot be reproduced with close-fill
    // signals, so the pool clamps to a minimum one-bar hold.
    let holding_pool: Vec<i64> = args
        .candidate
        .trades
        .iter()
        .map(|t| t.bars.max(1))
        .collect();
    let trade_count = args.candidate.trades.len() as i64;

    let (fee_pct, slippage_pct, sizing_pct) = exec_cost_fractions(args.costs);
    let cfg = BacktestConfig {
        exec: ExecutionModel {
            direction: Direction::Long,
            sizing_pct,
            fill_mode: FillMode::Close,
        },
        cost: CostModel {
            fee_pct,
            slippage_pct,
        },
        risk: None,
        start_equity: args.start_equity,
        bars_per_year: bars_per_year(args.interval),
        from: args.from,
        to: args.to,
    };

    let mut rng = Mulberry32::from_truncated(args.seed);
    let mut net_returns = Vec::with_capacity(runs as usize);
    for _ in 0..runs {
        let planned = plan_random_trades(&mut rng, from, to, &holding_pool, trade_count);
        let mut entry = vec![false; args.candles.len()];
        let mut exit = vec![false; args.candles.len()];
        for trade in &planned {
            entry[trade.entry_idx as usize] = true;
            if let Some(exit_idx) = trade.exit_idx {
                exit[exit_idx as usize] = true;
            }
        }
        let result = run_backtest(args.candles, &Signals { entry, exit }, &cfg)?;
        net_returns.push(result.metrics.net_return);
    }

    let candidate_net_return = args.candidate.net_return;
    let beaten = net_returns
        .iter()
        .filter(|value| **value < candidate_net_return)
        .count() as f64;
    Ok(RandomEntryBenchmark {
        runs,
        seed: args.seed,
        net_returns,
        candidate_net_return,
        candidate_percentile: beaten / runs as f64 * 100.0,
    })
}
