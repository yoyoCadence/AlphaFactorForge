// Params-mode strategy shape for the UI port (Slice 1).
//
// Mirrors the params subset of the legacy prototype's defStrat() (see
// AlphaFactorForge.dc.html). Percentages use the legacy UNIT convention
// (feePct 0.05 = 0.05%, sizePct 100 = all-in, slPct 0 = off); backtestRunner
// converts these to the fractions core/backtest expects. blocks/code modes
// arrive in a later slice; code mode stays manual-only.

import type { Direction, FillMode } from '../core/backtest';

/** Built-in entry/exit signal ids (params mode). `stoch*` await a core STOCH
 *  indicator (Phase B) and are rejected at runtime until then. */
export type SignalId =
  | 'maCrossUp'
  | 'maCrossDown'
  | 'emaCrossUp'
  | 'emaCrossDown'
  | 'priceAboveSlow'
  | 'priceBelowSlow'
  | 'rsiOversold'
  | 'rsiOverbought'
  | 'macdCrossUp'
  | 'macdCrossDown'
  | 'bbLowerTouch'
  | 'bbUpperTouch'
  | 'stochOversold'
  | 'stochOverbought';

export interface ParamsStrategy {
  mode: 'params';
  // indicator periods
  fastMA: number;
  slowMA: number;
  emaPeriod: number;
  rsiPeriod: number;
  rsiBuy: number;
  rsiSell: number;
  macdFast: number;
  macdSlow: number;
  macdSignal: number;
  bbPeriod: number;
  bbMult: number;
  // signals
  entrySig: SignalId;
  exitSig: SignalId;
  // risk (legacy percent units; 0 = off)
  slPct: number;
  tpPct: number;
  // execution + cost (legacy percent units)
  feePct: number;
  slipPct: number;
  sizePct: number;
  fillMode: FillMode;
  direction: Direction;
}

/** Defaults copied verbatim from the legacy defStrat() params subset. */
export function defaultStrategy(): ParamsStrategy {
  return {
    mode: 'params',
    fastMA: 9,
    slowMA: 21,
    emaPeriod: 50,
    rsiPeriod: 14,
    rsiBuy: 30,
    rsiSell: 70,
    macdFast: 12,
    macdSlow: 26,
    macdSignal: 9,
    bbPeriod: 20,
    bbMult: 2,
    entrySig: 'maCrossUp',
    exitSig: 'maCrossDown',
    slPct: 0,
    tpPct: 0,
    feePct: 0.05,
    slipPct: 0.02,
    sizePct: 100,
    fillMode: 'close',
    direction: 'long',
  };
}
