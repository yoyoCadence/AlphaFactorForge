// Parameter sweep (Slice 5b-1): vary 1–2 numeric strategy params over ranges,
// run a backtest per combo, and score each combo by a chosen metric. Pure
// (no IO), deterministic; safe in a Web Worker.
//
// Mirrors the legacy AlphaFactorForge.dc.html runSweep: each axis is capped at
// 64 values, total combos at 256; a combo only qualifies as "best" when it has
// trades > 0; non-finite profitFactor/calmar are guarded (Infinity -> 99). The
// swept value is patched onto the base strategy, so this respects whatever mode
// the strategy is in (params/blocks/code all derive indicators from these
// periods) — buildSignals runs through backtestRunner unchanged.

import { runParamsBacktest } from './backtestRunner';
import type { ParamsStrategy } from './strategy';
import type { Candle } from '../core/backtest';
import type { Metrics } from '../core/metrics';

/** Numeric strategy params a sweep may vary (mirrors the legacy axis list). */
export const SWEEP_PARAM_KEYS = [
  'fastMA',
  'slowMA',
  'emaPeriod',
  'rsiPeriod',
  'rsiBuy',
  'rsiSell',
  'macdFast',
  'macdSlow',
  'bbPeriod',
] as const;
export type SweepParamKey = (typeof SWEEP_PARAM_KEYS)[number];

/** Optimisation metrics. `dd` is stored as -maxDrawdown so higher is always
 *  better across every metric (simplifies best/color scaling). */
export const SWEEP_METRIC_IDS = ['net', 'sharpe', 'pf', 'winRate', 'calmar', 'dd'] as const;
export type SweepMetricId = (typeof SWEEP_METRIC_IDS)[number];

/** Per-axis cap (legacy: break once 64 values are generated). */
export const SWEEP_MAX_AXIS = 64;
/** Total-combo cap (legacy: reject when xs * ys > 256). */
export const SWEEP_MAX_COMBOS = 256;

export interface SweepAxisConfig {
  key: SweepParamKey;
  min: number;
  max: number;
  step: number;
}

export interface SweepConfig {
  x: SweepAxisConfig;
  /** optional second dimension (2-D heatmap); omit/null for a 1-D sweep. */
  y?: SweepAxisConfig | null;
  metric: SweepMetricId;
}

export interface SweepCell {
  x: number;
  /** null on a 1-D sweep. */
  y: number | null;
  /** the optimisation metric value, or null if the backtest produced none. */
  metric: number | null;
  trades: number;
}

export interface SweepBest {
  x: number;
  y: number | null;
  metric: number;
  trades: number;
}

export interface SweepResult {
  xs: number[];
  /** [null] on a 1-D sweep, else the y-axis values. */
  ys: (number | null)[];
  /** grid[rowIndex (ys)][colIndex (xs)]. */
  grid: SweepCell[][];
  metric: SweepMetricId;
  xKey: SweepParamKey;
  yKey: SweepParamKey | null;
  /** highest-metric combo with trades > 0, or null if none traded. */
  best: SweepBest | null;
  /** min / max metric across cells that produced a value (color scaling). */
  lo: number;
  hi: number;
}

/**
 * Inclusive value list from min to max stepping by |step| (>= 1 floor when 0).
 * Mirrors legacy mkRange: max < min -> [min]; rounds to 1e-6 to absorb float
 * drift; stops at SWEEP_MAX_AXIS values.
 */
export function buildAxisValues(min: number, max: number, step: number): number[] {
  const lo = Number(min);
  const hi = Number(max);
  if (!Number.isFinite(lo)) return [];
  if (!Number.isFinite(hi) || hi < lo) return [lo];
  const st = Math.abs(Number(step)) || 1;
  const out: number[] = [];
  for (let v = lo; v <= hi + 1e-9; v += st) {
    out.push(Math.round(v * 1e6) / 1e6);
    if (out.length >= SWEEP_MAX_AXIS) break;
  }
  return out;
}

/** Total combos a config would run (for pre-flight validation in the UI). */
export function countSweepCombos(cfg: SweepConfig): number {
  const xs = buildAxisValues(cfg.x.min, cfg.x.max, cfg.x.step);
  const ys = cfg.y ? buildAxisValues(cfg.y.min, cfg.y.max, cfg.y.step) : [null];
  return xs.length * ys.length;
}

/** Project a Metrics onto the chosen optimisation scalar (legacy guards). */
export function sweepMetricValue(m: Metrics, id: SweepMetricId): number {
  switch (id) {
    case 'net':
      return m.netReturn;
    case 'sharpe':
      return m.sharpe;
    case 'pf':
      return Number.isFinite(m.profitFactor) ? m.profitFactor : m.profitFactor > 0 ? 99 : 0;
    case 'winRate':
      return m.winRate;
    case 'calmar':
      return Number.isFinite(m.calmar) ? m.calmar : m.calmar > 0 ? 99 : 0;
    case 'dd':
      return -m.maxDrawdown;
  }
}

export interface RunParamSweepArgs {
  candles: Candle[];
  strat: ParamsStrategy;
  interval: string;
  sweep: SweepConfig;
  /** restrict each backtest to [from, to] (e.g. sweep in-sample only). */
  from?: number;
  to?: number;
}

/**
 * Run every param combination and return the scored grid. Deterministic.
 * Throws RangeError when the x-axis is empty or combos exceed SWEEP_MAX_COMBOS.
 * A single combo that throws (e.g. an invalid indicator period) yields a null
 * cell rather than failing the whole sweep.
 */
export function runParamSweep(args: RunParamSweepArgs): SweepResult {
  const { candles, strat, interval, sweep, from, to } = args;
  const xs = buildAxisValues(sweep.x.min, sweep.x.max, sweep.x.step);
  const ys = sweep.y ? buildAxisValues(sweep.y.min, sweep.y.max, sweep.y.step) : [null];

  if (xs.length === 0) throw new RangeError('掃描 X 軸範圍無效');
  const combos = xs.length * ys.length;
  if (combos > SWEEP_MAX_COMBOS) {
    throw new RangeError(`組合數 ${combos} 過多（上限 ${SWEEP_MAX_COMBOS}），請縮小範圍或加大間距。`);
  }

  const xKey = sweep.x.key;
  const yKey = sweep.y ? sweep.y.key : null;

  let best: SweepBest | null = null;
  let lo = Infinity;
  let hi = -Infinity;
  const grid: SweepCell[][] = [];

  for (const yv of ys) {
    const row: SweepCell[] = [];
    for (const xv of xs) {
      const variant: ParamsStrategy = { ...strat, [xKey]: xv };
      if (yKey != null && yv != null) variant[yKey] = yv;

      let metric: number | null = null;
      let trades = 0;
      try {
        const res = runParamsBacktest({ candles, strat: variant, interval, from, to });
        const v = sweepMetricValue(res.metrics, sweep.metric);
        metric = Number.isFinite(v) ? v : null;
        trades = res.metrics.tradeCount;
      } catch {
        metric = null;
        trades = 0;
      }

      if (metric != null) {
        if (metric < lo) lo = metric;
        if (metric > hi) hi = metric;
        if (trades > 0 && (best === null || metric > best.metric)) {
          best = { x: xv, y: yv, metric, trades };
        }
      }
      row.push({ x: xv, y: yv, metric, trades });
    }
    grid.push(row);
  }

  if (!Number.isFinite(lo)) lo = 0;
  if (!Number.isFinite(hi)) hi = 0;

  return { xs, ys, grid, metric: sweep.metric, xKey, yKey, best, lo, hi };
}
