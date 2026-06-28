// Orchestrates a single params-mode backtest: strategy -> signals -> engine.
//
// Bridges the legacy UI's units to core/backtest: legacy percentages
// (feePct 0.05, sizePct 100, slPct 0) -> fractions; interval string ->
// barsPerYear for annualised metrics. Pure (no IO); safe in a Web Worker.

import {
  runBacktest,
  type Candle,
  type BacktestConfig,
  type BacktestResult,
} from '../core/backtest';
import { buildParamsSignals } from './strategySignals';
import type { ParamsStrategy } from './strategy';

/** Approx. bars per year per interval, for CAGR/Sharpe annualisation. */
export const BARS_PER_YEAR: Record<string, number> = {
  '1m': 525_600,
  '3m': 175_200,
  '5m': 105_120,
  '15m': 35_040,
  '1h': 8_760,
  '4h': 2_190,
  '1d': 365,
};

/** Bars per year for an interval; unknown intervals fall back to daily. */
export function barsPerYear(interval: string): number {
  return BARS_PER_YEAR[interval] ?? 365;
}

export interface RunParamsBacktestArgs {
  candles: Candle[];
  strat: ParamsStrategy;
  interval: string;
  startEquity?: number;
  /** restrict to [from, to] candle index range (inclusive). */
  from?: number;
  to?: number;
}

/** Run a params-mode backtest end to end. Deterministic. */
export function runParamsBacktest(args: RunParamsBacktestArgs): BacktestResult {
  const { candles, strat, interval } = args;
  const signals = buildParamsSignals(candles, strat);

  const cfg: BacktestConfig = {
    exec: {
      direction: strat.direction,
      sizingPct: strat.sizePct / 100,
      fillMode: strat.fillMode,
    },
    cost: {
      feePct: strat.feePct / 100,
      slippagePct: strat.slipPct / 100,
    },
    risk: {
      stopLossPct: strat.slPct > 0 ? strat.slPct / 100 : undefined,
      takeProfitPct: strat.tpPct > 0 ? strat.tpPct / 100 : undefined,
    },
    barsPerYear: barsPerYear(interval),
    startEquity: args.startEquity,
    from: args.from,
    to: args.to,
  };

  return runBacktest(candles, signals, cfg);
}
