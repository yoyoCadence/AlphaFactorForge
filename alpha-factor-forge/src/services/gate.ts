// GATE-001: hard elimination gate (STRATEGY_DISCOVERY.md §5.1).
//
// A candidate must pass EVERY criterion before Score/ranking ever sees it.
// This module only judges — it computes nothing new about the strategy and
// runs no backtests. Inputs are the candidate's segment result plus the
// complete §6 benchmark outputs for the SAME candles × segment (the caller
// owns running them; a validation-segment result is the intended input, and
// the hidden Test segment must never be fed through ranking-time gates).
//
// Thresholds are explicit configuration; DEFAULT_GATE_CONFIG records the
// §5.1 defaults. Missing evidence never passes: an unverifiable criterion
// (too little equity history, no positive total profit, missing benchmark)
// fails closed. Pure — no IO/UI/state.

import type { BacktestResult } from '../core/backtest';
import { DETERMINISTIC_BENCHMARK_IDS, type BenchmarkRun } from './benchmarks';
import type { RandomEntryBenchmark } from './randomEntry';

/** Version recorded with persisted Gate verdicts; see docs/gate-contract.md. */
export const GATE_CONTRACT_VERSION = 'gate-v1';

export interface GateConfig {
  /** Minimum closed trades in the segment (>= threshold passes). */
  minTrades: number;
  /** Cost-adjusted mean trade return must be STRICTLY above this. */
  minAvgTradeReturn: number;
  /** Rolling-return window length in bars (equity-curve windows, step 1). */
  rollingWindowBars: number;
  /** Fraction of rolling windows with a positive return (>= passes). */
  minRollingPositiveRatio: number;
  /** Max peak-to-trough drawdown as a positive fraction (<= passes). */
  maxDrawdown: number;
  /** Any single month may contribute at most this fraction of total profit. */
  maxMonthlyContribution: number;
  /** Any single trade may contribute at most this fraction of total profit. */
  maxSingleTradeContribution: number;
  /** Candidate must reach this Random Entry percentile (>= passes). */
  minRandomEntryPercentile: number;
}

/** §5.1 defaults. rollingWindowBars is a recorded v1 convention — the doc
 *  fixes the 55% ratio but not the window length. */
export const DEFAULT_GATE_CONFIG: GateConfig = {
  minTrades: 30,
  minAvgTradeReturn: 0,
  rollingWindowBars: 30,
  minRollingPositiveRatio: 0.55,
  maxDrawdown: 0.35,
  maxMonthlyContribution: 0.4,
  maxSingleTradeContribution: 0.25,
  minRandomEntryPercentile: 95,
};

export type GateCriterionId =
  | 'minTrades'
  | 'avgTradeReturn'
  | 'rollingConsistency'
  | 'maxDrawdown'
  | 'monthlyConcentration'
  | 'tradeConcentration'
  | 'benchmarkWins'
  | 'randomEntryPercentile';

export interface GateCriterion {
  id: GateCriterionId;
  pass: boolean;
  /** Observed value; null when the evidence was insufficient (fails closed). */
  value: number | null;
  threshold: number;
  detail?: string;
}

export interface GateVerdict {
  /** True only when every criterion passed. */
  pass: boolean;
  /** Fixed §5.1 order, one entry per criterion. */
  criteria: GateCriterion[];
  /** The exact thresholds judged with (record alongside the verdict). */
  config: GateConfig;
}

export interface EvaluateGateArgs {
  /** The candidate's result over the judged segment (Validation intended). */
  candidateResult: BacktestResult;
  /** All four deterministic §6 benchmarks over the same candles × segment. */
  benchmarks: BenchmarkRun[];
  /** The Random Entry Monte Carlo output for the same candidate/segment. */
  randomEntry: RandomEntryBenchmark;
  /** Threshold overrides; unspecified fields keep the §5.1 defaults. */
  config?: Partial<GateConfig>;
}

function assertFraction(value: number, name: string): void {
  if (!Number.isFinite(value) || value <= 0 || value > 1) {
    throw new RangeError(`${name} must be a fraction in (0, 1]`);
  }
}

function validateConfig(cfg: GateConfig): void {
  if (!Number.isSafeInteger(cfg.minTrades) || cfg.minTrades < 1) {
    throw new RangeError('minTrades must be a positive integer');
  }
  if (!Number.isFinite(cfg.minAvgTradeReturn)) {
    throw new RangeError('minAvgTradeReturn must be a finite number');
  }
  if (!Number.isSafeInteger(cfg.rollingWindowBars) || cfg.rollingWindowBars < 1) {
    throw new RangeError('rollingWindowBars must be a positive integer');
  }
  assertFraction(cfg.minRollingPositiveRatio, 'minRollingPositiveRatio');
  assertFraction(cfg.maxDrawdown, 'maxDrawdown');
  assertFraction(cfg.maxMonthlyContribution, 'maxMonthlyContribution');
  assertFraction(cfg.maxSingleTradeContribution, 'maxSingleTradeContribution');
  if (
    !Number.isFinite(cfg.minRandomEntryPercentile) ||
    cfg.minRandomEntryPercentile < 0 ||
    cfg.minRandomEntryPercentile > 100
  ) {
    throw new RangeError('minRandomEntryPercentile must be in [0, 100]');
  }
}

/** Fraction of rolling `windowBars`-bar equity windows with a positive
 *  return (step 1). Null when the curve is too short to form one window or
 *  contains a non-finite equity value. */
export function rollingPositiveRatio(
  equity: BacktestResult['equity'],
  windowBars: number,
): number | null {
  if (equity.some((point) => !Number.isFinite(point.equity))) return null;
  const windows = equity.length - windowBars;
  if (windows < 1) return null;
  let positive = 0;
  for (let i = 0; i < windows; i++) {
    if (equity[i + windowBars].equity > equity[i].equity) positive++;
  }
  return positive / windows;
}

const utcMonth = (epochMs: number): string | null => {
  const d = new Date(epochMs);
  if (!Number.isFinite(epochMs) || Number.isNaN(d.getTime())) return null;
  return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, '0')}`;
};

interface ContributionEvidence {
  value: number | null;
  detail?: string;
}

/** Largest single positive contribution as a fraction of total profit.
 *  Null when an input/total is non-finite or total profit is not positive —
 *  concentration is then unverifiable and the criterion fails closed. */
function maxContribution(pnls: number[]): ContributionEvidence {
  let total = 0;
  for (const pnl of pnls) {
    if (!Number.isFinite(pnl)) {
      return { value: null, detail: 'non-finite profit evidence' };
    }
    total += pnl;
    if (!Number.isFinite(total)) {
      return { value: null, detail: 'non-finite profit evidence' };
    }
  }
  if (total <= 0) {
    return { value: null, detail: 'no positive total profit to attribute' };
  }
  const largest = pnls.reduce((current, pnl) => Math.max(current, pnl), 0);
  const value = largest / total;
  return Number.isFinite(value)
    ? { value }
    : { value: null, detail: 'non-finite profit evidence' };
}

function monthlyContribution(trades: BacktestResult['trades']): ContributionEvidence {
  const months = trades.map((trade) => utcMonth(trade.exitTime));
  if (months.some((month) => month === null)) {
    return { value: null, detail: 'invalid trade exit-time evidence' };
  }

  const byMonth = new Map<string, number>();
  for (let index = 0; index < trades.length; index++) {
    const month = months[index]!;
    byMonth.set(month, (byMonth.get(month) ?? 0) + trades[index].pnl);
  }
  return maxContribution([...byMonth.values()]);
}

/**
 * Judge the §5.1 hard gate. Throws on invalid config or when the deterministic
 * benchmark set has missing/duplicate ids; every other evidence problem fails
 * the affected criterion instead of throwing.
 */
export function evaluateGate(args: EvaluateGateArgs): GateVerdict {
  const config: GateConfig = { ...DEFAULT_GATE_CONFIG, ...args.config };
  validateConfig(config);

  const benchmarkCounts = new Map<BenchmarkRun['id'], number>();
  for (const benchmark of args.benchmarks) {
    benchmarkCounts.set(benchmark.id, (benchmarkCounts.get(benchmark.id) ?? 0) + 1);
  }
  const duplicates = DETERMINISTIC_BENCHMARK_IDS.filter(
    (id) => (benchmarkCounts.get(id) ?? 0) > 1,
  );
  if (duplicates.length > 0) {
    throw new RangeError(`duplicate deterministic benchmark(s): ${duplicates.join(', ')}`);
  }

  const byId = new Map(args.benchmarks.map((b) => [b.id, b]));
  const missing = DETERMINISTIC_BENCHMARK_IDS.filter((id) => !byId.has(id));
  if (missing.length > 0) {
    throw new RangeError(`missing deterministic benchmark(s): ${missing.join(', ')}`);
  }

  const { metrics, trades, equity } = args.candidateResult;
  const criteria: GateCriterion[] = [];

  criteria.push({
    id: 'minTrades',
    pass:
      Number.isSafeInteger(metrics.tradeCount) &&
      metrics.tradeCount >= 0 &&
      metrics.tradeCount >= config.minTrades,
    value: metrics.tradeCount,
    threshold: config.minTrades,
  });

  criteria.push({
    id: 'avgTradeReturn',
    pass:
      Number.isFinite(metrics.avgTradeReturn) &&
      metrics.avgTradeReturn > config.minAvgTradeReturn,
    value: metrics.avgTradeReturn,
    threshold: config.minAvgTradeReturn,
  });

  const hasNonFiniteEquity = equity.some((point) => !Number.isFinite(point.equity));
  const rolling = rollingPositiveRatio(equity, config.rollingWindowBars);
  criteria.push({
    id: 'rollingConsistency',
    pass: rolling != null && rolling >= config.minRollingPositiveRatio,
    value: rolling,
    threshold: config.minRollingPositiveRatio,
    ...(rolling == null
      ? {
          detail: hasNonFiniteEquity
            ? 'non-finite equity evidence'
            : `equity curve shorter than one ${config.rollingWindowBars}-bar window`,
        }
      : {}),
  });

  criteria.push({
    id: 'maxDrawdown',
    pass: Number.isFinite(metrics.maxDrawdown) && metrics.maxDrawdown <= config.maxDrawdown,
    value: metrics.maxDrawdown,
    threshold: config.maxDrawdown,
  });

  const monthly = monthlyContribution(trades);
  criteria.push({
    id: 'monthlyConcentration',
    pass: monthly.value != null && monthly.value <= config.maxMonthlyContribution,
    value: monthly.value,
    threshold: config.maxMonthlyContribution,
    ...(monthly.detail ? { detail: monthly.detail } : {}),
  });

  const perTrade = maxContribution(trades.map((t) => t.pnl));
  criteria.push({
    id: 'tradeConcentration',
    pass: perTrade.value != null && perTrade.value <= config.maxSingleTradeContribution,
    value: perTrade.value,
    threshold: config.maxSingleTradeContribution,
    ...(perTrade.detail ? { detail: perTrade.detail } : {}),
  });

  const lost = DETERMINISTIC_BENCHMARK_IDS.filter((id) => {
    const benchmarkReturn = byId.get(id)!.result.metrics.netReturn;
    return !(
      Number.isFinite(metrics.netReturn) &&
      Number.isFinite(benchmarkReturn) &&
      metrics.netReturn > benchmarkReturn
    );
  });
  criteria.push({
    id: 'benchmarkWins',
    pass: lost.length === 0,
    value: DETERMINISTIC_BENCHMARK_IDS.length - lost.length,
    threshold: DETERMINISTIC_BENCHMARK_IDS.length,
    ...(lost.length > 0 ? { detail: `not beaten: ${lost.join(', ')}` } : {}),
  });

  criteria.push({
    id: 'randomEntryPercentile',
    pass:
      Number.isFinite(args.randomEntry.candidatePercentile) &&
      args.randomEntry.candidatePercentile >= config.minRandomEntryPercentile,
    value: args.randomEntry.candidatePercentile,
    threshold: config.minRandomEntryPercentile,
  });

  return { pass: criteria.every((c) => c.pass), criteria, config };
}
