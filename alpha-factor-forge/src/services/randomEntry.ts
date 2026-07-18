// BENCH-002: Random Entry Monte Carlo benchmark (STRATEGY_DISCOVERY.md §6).
//
// The key alpha test: a candidate must beat "random buys with the SAME
// exposure and holding time" or its return is just beta / time-in-market.
// Each simulated run places the candidate's trade count at seeded-random,
// non-overlapping positions in the same segment, with holding periods sampled
// (with replacement) from the candidate's own closed-trade `bars`, then runs
// the real backtest engine long-only / 100% sizing / close fill with the
// candidate's inherited costs.
//
// This slice returns the per-run netReturn distribution and the candidate's
// percentile (fraction of runs it STRICTLY beats). It deliberately renders no
// pass/fail verdict — the ">= 95th percentile" threshold belongs to the Gate
// slice. Deterministic: one mulberry32 stream, consumed in a fixed order
// (per run: k durations, then k + 1 gap weights). Pure; Web Worker safe.
//
// Recorded conventions (docs/benchmark-suite-contract.md, BENCH-002 section):
// sampled holding bars clamp to >= 1 (a same-bar candidate exit cannot be
// reproduced by close-fill signals); a trade that no longer fits the segment
// is clipped at the segment end (engine EOD settle) and later trades drop.

import {
  runBacktest,
  type BacktestConfig,
  type BacktestResult,
} from '../core/backtest';
import { barsPerYear, toExecCostFractions } from './backtestRunner';
import type { RunBenchmarksArgs } from './benchmarks';
import { mulberry32 } from './sampleData';

export const DEFAULT_RANDOM_ENTRY_RUNS = 200;
export const MAX_RANDOM_ENTRY_RUNS = 1000;

export interface RandomEntryArgs extends RunBenchmarksArgs {
  /** The candidate's result over the SAME candles × segment: its closed-trade
   *  `bars` form the holding-period pool and its netReturn is ranked. */
  candidateResult: BacktestResult;
  /** Explicit PRNG seed (non-negative safe integer). Same seed, same result. */
  seed: number;
  /** Simulated runs; explicit and capped. Default DEFAULT_RANDOM_ENTRY_RUNS. */
  runs?: number;
}

export interface PlannedRandomTrade {
  entryIdx: number;
  /** Exit-signal bar; null when the trade is clipped by the segment end and
   *  the engine's EOD settlement closes it instead. */
  exitIdx: number | null;
}

export interface RandomEntryBenchmark {
  runs: number;
  seed: number;
  /** Per-run net returns, in run order (deterministic given the seed). */
  netReturns: number[];
  candidateNetReturn: number;
  /** Percent of runs the candidate strictly beats, 0..100. */
  candidatePercentile: number;
}

/**
 * Plan one run's non-overlapping random trades inside [from, to] (inclusive).
 * Consumes `rand` in a fixed order: `tradeCount` durations, then
 * `tradeCount + 1` gap weights. Exported for direct unit testing.
 */
export function planRandomTrades(
  rand: () => number,
  from: number,
  to: number,
  holdingPool: number[],
  tradeCount: number,
): PlannedRandomTrade[] {
  const durations: number[] = [];
  for (let j = 0; j < tradeCount; j++) {
    durations.push(holdingPool[Math.floor(rand() * holdingPool.length)]);
  }
  const weights: number[] = [];
  for (let j = 0; j < tradeCount + 1; j++) weights.push(rand());

  const segmentBars = to - from + 1;
  const occupied = durations.reduce((sum, d) => sum + d + 1, 0);
  const free = Math.max(0, segmentBars - occupied);
  const totalWeight = weights.reduce((sum, w) => sum + w, 0);
  const gap = (j: number): number =>
    totalWeight > 0 ? Math.floor((weights[j] / totalWeight) * free) : 0;

  const planned: PlannedRandomTrade[] = [];
  let cursor = from + gap(0);
  for (let j = 0; j < tradeCount; j++) {
    if (cursor > to) break; // no room left — later trades drop
    const exitIdx = cursor + durations[j];
    if (exitIdx > to) {
      planned.push({ entryIdx: cursor, exitIdx: null }); // clipped: EOD settles
      break;
    }
    planned.push({ entryIdx: cursor, exitIdx });
    cursor = exitIdx + 1 + gap(j + 1);
  }
  return planned;
}

/**
 * Run the Random Entry Monte Carlo benchmark. Fails closed on an empty
 * series/segment, a candidate without closed trades, or invalid runs/seed.
 */
export function runRandomEntryBenchmark(args: RandomEntryArgs): RandomEntryBenchmark {
  const { candles, interval, costs, candidateResult } = args;
  const runs = args.runs ?? DEFAULT_RANDOM_ENTRY_RUNS;

  if (!Number.isSafeInteger(runs) || runs < 1 || runs > MAX_RANDOM_ENTRY_RUNS) {
    throw new RangeError(`runs must be an integer in [1, ${MAX_RANDOM_ENTRY_RUNS}]`);
  }
  if (!Number.isSafeInteger(args.seed) || args.seed < 0) {
    throw new RangeError('seed must be a non-negative safe integer');
  }
  if (candles.length === 0) {
    throw new RangeError('random entry benchmark needs a non-empty candle series');
  }
  const from = Math.max(0, args.from ?? 0);
  const to = Math.min(candles.length - 1, args.to ?? candles.length - 1);
  if (to < from) {
    throw new RangeError('random entry benchmark needs a non-empty [from, to] segment');
  }
  if (candidateResult.trades.length === 0) {
    throw new RangeError(
      'random entry benchmark needs at least one closed candidate trade for the holding-period pool',
    );
  }

  // Same-bar candidate exits (bars 0) cannot be reproduced with close-fill
  // signals, so the pool clamps to a minimum one-bar hold.
  const holdingPool = candidateResult.trades.map((t) => Math.max(1, t.bars));
  const tradeCount = candidateResult.trades.length;

  const { feePct, slippagePct, sizingPct } = toExecCostFractions({
    feePct: costs.feePct,
    slipPct: costs.slipPct,
    sizePct: 100,
  });
  const cfg: BacktestConfig = {
    exec: { direction: 'long', sizingPct, fillMode: 'close' },
    cost: { feePct, slippagePct },
    risk: {},
    barsPerYear: barsPerYear(interval),
    startEquity: args.startEquity,
    from: args.from,
    to: args.to,
  };

  const rand = mulberry32(args.seed);
  const netReturns: number[] = [];
  for (let run = 0; run < runs; run++) {
    const planned = planRandomTrades(rand, from, to, holdingPool, tradeCount);
    const entry = new Array<boolean>(candles.length).fill(false);
    const exit = new Array<boolean>(candles.length).fill(false);
    for (const t of planned) {
      entry[t.entryIdx] = true;
      if (t.exitIdx != null) exit[t.exitIdx] = true;
    }
    netReturns.push(runBacktest(candles, { entry, exit }, cfg).metrics.netReturn);
  }

  const candidateNetReturn = candidateResult.metrics.netReturn;
  const beaten = netReturns.filter((r) => r < candidateNetReturn).length;
  return {
    runs,
    seed: args.seed,
    netReturns,
    candidateNetReturn,
    candidatePercentile: (beaten / runs) * 100,
  };
}
