// BENCH-001: deterministic benchmark suite (STRATEGY_DISCOVERY.md §6).
//
// Every candidate strategy must eventually beat these baselines on the SAME
// candles × segment before it can promote. This slice only PRODUCES the four
// deterministic benchmark results through the existing backtest pipeline;
// the Gate comparison rules and the Random Entry Monte Carlo benchmark
// (matched holding period + percentile convention) are later slices.
//
// V1 conventions (recorded in docs/benchmark-suite-contract.md):
//   - Benchmarks inherit the candidate's fee/slippage so the comparison is
//     cost-fair, but always run long-only, 100% sizing, close fill, no SL/TP.
//   - smaCross is the doc's standard 50/200; timeframe-adapted periods are
//     deferred. Short segments where SMA200 never warms up simply produce
//     0 trades — the Gate slice owns how that is judged.
//   - Buy & Hold enters at the first tested bar's close and holds until the
//     engine's EOD settlement at the segment end (hand-built signals).
// Pure (no IO); safe in a Web Worker. Deterministic: fixed suite order.

import {
  runBacktest,
  type Candle,
  type BacktestConfig,
  type BacktestResult,
} from '../core/backtest';
import { barsPerYear, runParamsBacktest, toExecCostFractions } from './backtestRunner';
import { defaultStrategy, type ParamsStrategy } from './strategy';

export type DeterministicBenchmarkId =
  | 'buyHold'
  | 'smaCross'
  | 'rsiReversion'
  | 'bollingerReversion';

/** Fixed, deterministic suite order. */
export const DETERMINISTIC_BENCHMARK_IDS: readonly DeterministicBenchmarkId[] = [
  'buyHold',
  'smaCross',
  'rsiReversion',
  'bollingerReversion',
] as const;

/** Candidate exec costs the benchmarks inherit (legacy percent units,
 *  e.g. feePct 0.05 = 0.05% — same convention as ParamsStrategy). */
export interface BenchmarkCosts {
  feePct: number;
  slipPct: number;
}

export interface RunBenchmarksArgs {
  /** Candles ordered oldest to newest (same series the candidate uses). */
  candles: Candle[];
  interval: string;
  costs: BenchmarkCosts;
  startEquity?: number;
  /** restrict to [from, to] candle index range (inclusive), like the engine. */
  from?: number;
  to?: number;
}

export interface BenchmarkRun {
  id: DeterministicBenchmarkId;
  /** The exact strategy backtested; null for buyHold (hand-built signals). */
  strat: ParamsStrategy | null;
  result: BacktestResult;
}

/** Shared benchmark execution model: long-only, all-in, close fill, no risk
 *  exits; only the candidate's costs vary. */
function benchmarkBase(costs: BenchmarkCosts): ParamsStrategy {
  return {
    ...defaultStrategy(),
    mode: 'params',
    direction: 'long',
    fillMode: 'close',
    sizePct: 100,
    slPct: 0,
    tpPct: 0,
    feePct: costs.feePct,
    slipPct: costs.slipPct,
  };
}

/** The exact signal-based strategy one benchmark id runs (doc §6 definitions). */
export function benchmarkStrategy(
  id: Exclude<DeterministicBenchmarkId, 'buyHold'>,
  costs: BenchmarkCosts,
): ParamsStrategy {
  const base = benchmarkBase(costs);
  switch (id) {
    case 'smaCross':
      return { ...base, fastMA: 50, slowMA: 200, entrySig: 'maCrossUp', exitSig: 'maCrossDown' };
    case 'rsiReversion':
      return {
        ...base,
        rsiPeriod: 14,
        rsiBuy: 30,
        rsiSell: 70,
        entrySig: 'rsiOversold',
        exitSig: 'rsiOverbought',
      };
    case 'bollingerReversion':
      return { ...base, bbPeriod: 20, bbMult: 2, entrySig: 'bbLowerTouch', exitSig: 'bbUpperTouch' };
  }
}

/** Buy & Hold: enter at the first tested bar's close, never signal an exit;
 *  the engine's EOD settlement closes the position at the segment end. */
function runBuyHold(args: RunBenchmarksArgs): BacktestResult {
  const { candles, interval, costs } = args;
  const n = candles.length;
  const from = Math.max(0, args.from ?? 0);
  const { feePct, slippagePct, sizingPct } = toExecCostFractions({
    feePct: costs.feePct,
    slipPct: costs.slipPct,
    sizePct: 100,
  });

  const entry = new Array<boolean>(n).fill(false);
  if (from < n) entry[from] = true;
  const exit = new Array<boolean>(n).fill(false);

  const cfg: BacktestConfig = {
    exec: { direction: 'long', sizingPct, fillMode: 'close' },
    cost: { feePct, slippagePct },
    risk: {},
    barsPerYear: barsPerYear(interval),
    startEquity: args.startEquity,
    from: args.from,
    to: args.to,
  };
  return runBacktest(candles, { entry, exit }, cfg);
}

/**
 * Run the four deterministic benchmarks over one candles × segment, in the
 * fixed DETERMINISTIC_BENCHMARK_IDS order. Fails closed on an empty series —
 * a benchmark over nothing cannot anchor any comparison.
 */
export function runDeterministicBenchmarks(args: RunBenchmarksArgs): BenchmarkRun[] {
  if (args.candles.length === 0) {
    throw new RangeError('benchmarks need a non-empty candle series');
  }
  return DETERMINISTIC_BENCHMARK_IDS.map((id) => {
    if (id === 'buyHold') {
      return { id, strat: null, result: runBuyHold(args) };
    }
    const strat = benchmarkStrategy(id, args.costs);
    const result = runParamsBacktest({
      candles: args.candles,
      strat,
      interval: args.interval,
      startEquity: args.startEquity,
      from: args.from,
      to: args.to,
    });
    return { id, strat, result };
  });
}
