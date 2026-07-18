// VAL-002: run the VAL-001 split plan through the existing backtest pipeline.
//
// Plans planValidationSplit(candles.length, embargoBars), then backtests the
// Train and Validation ranges via runParamsBacktest `from`/`to`. Signals and
// indicators still see the full candle history (the same causal pattern as the
// Holdout split): core indicators only look backward, so earlier bars are
// legitimate warm-up history, and the embargo gaps keep the evaluated segments
// from touching.
//
// Test discipline: the hidden Test segment is planned but NEVER backtested
// here. ValidationRunResult deliberately has no `test` field, so no Test
// metrics exist for generation, tuning, ranking, or prompts. Pure (no IO);
// safe in a Web Worker.

import type { Candle, BacktestResult } from '../core/backtest';
import {
  planValidationSplit,
  type InclusiveBarRange,
  type ValidationSplitPlan,
} from '../core/validation/split';
import { runParamsBacktest } from './backtestRunner';
import type { ParamsStrategy } from './strategy';

export interface ValidationRunArgs {
  /** Candles ordered oldest to newest (the split contract's precondition). */
  candles: Candle[];
  strat: ParamsStrategy;
  interval: string;
  /** Caller-derived equal gap between segments; see the VAL-001 contract. */
  embargoBars: number;
  startEquity?: number;
}

export interface ValidationRunResult {
  /** The exact deterministic plan both segments were run with (audit trail). */
  plan: ValidationSplitPlan;
  train: BacktestResult;
  validation: BacktestResult;
  // No `test` field by design — the hidden Test segment is not run in v1.
}

/**
 * Backtest the Train and Validation segments of the v1 split plan.
 *
 * Invalid or insufficient input fails closed: planValidationSplit throws
 * (RangeError) before any backtest runs.
 */
export function runValidationBacktests(args: ValidationRunArgs): ValidationRunResult {
  const { candles, strat, interval, embargoBars, startEquity } = args;
  const plan = planValidationSplit(candles.length, embargoBars);
  const runRange = (range: InclusiveBarRange): BacktestResult =>
    runParamsBacktest({ candles, strat, interval, startEquity, from: range.from, to: range.to });

  return {
    plan,
    train: runRange(plan.train),
    validation: runRange(plan.validation),
  };
}
