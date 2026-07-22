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
import { buildSignals } from './strategySignals';
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
  return Object.prototype.hasOwnProperty.call(BARS_PER_YEAR, interval)
    ? BARS_PER_YEAR[interval]
    : 365;
}

export interface ExecCostFractions {
  feePct: number; // per side, fraction, >= 0
  slippagePct: number; // per side, fraction, >= 0
  sizingPct: number; // fraction of equity, clamped to [0.01, 1]
}

/**
 * Legacy execution-model unit conversion + clamping, mirroring
 * AlphaFactorForge.dc.html (runBacktestCore):
 *   fee  = Math.max(0, feePct||0) / 100
 *   slip = Math.max(0, slipPct||0) / 100
 *   size = Math.min(1, Math.max(0.01, (sizePct||100) / 100))
 * Clamping matters for edge inputs: negative fee/slip must NOT become a rebate,
 * and sizePct 0 falls back to 100% (then the 0.01 floor / 1.0 cap apply).
 */
export function toExecCostFractions(
  strat: Pick<ParamsStrategy, 'feePct' | 'slipPct' | 'sizePct'>,
): ExecCostFractions {
  return {
    feePct: Math.max(0, strat.feePct || 0) / 100,
    slippagePct: Math.max(0, strat.slipPct || 0) / 100,
    sizingPct: Math.min(1, Math.max(0.01, (strat.sizePct || 100) / 100)),
  };
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
  const signals = buildSignals(candles, strat);
  const { feePct, slippagePct, sizingPct } = toExecCostFractions(strat);

  const cfg: BacktestConfig = {
    exec: {
      direction: strat.direction,
      sizingPct,
      fillMode: strat.fillMode,
    },
    cost: {
      feePct,
      slippagePct,
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
