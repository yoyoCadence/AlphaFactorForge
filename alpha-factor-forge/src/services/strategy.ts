// Strategy config for the UI port. One flat object (like the legacy defStrat),
// with `mode` selecting how signals are built:
//   - 'params' (Slice 1): pick a built-in entry/exit SignalId.
//   - 'blocks' (Slice 4a): AND-lists of {operand, op, operand|const} rules.
// 'code' mode (Slice 4b) will be added with a SAFE whitelist-AST interpreter —
// never new Function/eval — and stays manual-only.
//
// Percentages use the legacy UNIT convention (feePct 0.05 = 0.05%, sizePct 100
// = all-in, slPct 0 = off); backtestRunner converts these to fractions.

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

/** Comparison operators for blocks-mode rules (superset of the params ops). */
export type RuleOp = '>' | '<' | '>=' | '<=' | 'crossUp' | 'crossDown';

/** Named operands a blocks rule can reference. Limited to series the current
 *  core indicator set can produce (stoch/atr/volMa await later indicators). */
export const OPERAND_IDS = [
  'price', 'open', 'high', 'low', 'volume',
  'maFast', 'maSlow', 'ema', 'rsi',
  'macd', 'macdSignal', 'macdHist',
  'bbUpper', 'bbMid', 'bbLower',
] as const;
export type OperandId = (typeof OPERAND_IDS)[number];

/** One blocks-mode condition: `l op r`, where `r` is an operand id OR a numeric
 *  constant written as a string (e.g. "70"). All rules in a list AND together. */
export interface Rule {
  l: OperandId;
  op: RuleOp;
  r: string;
}

export interface ParamsStrategy {
  mode: 'params' | 'blocks';
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
  // params-mode signals
  entrySig: SignalId;
  exitSig: SignalId;
  // blocks-mode rules (AND within each list)
  entryRules: Rule[];
  exitRules: Rule[];
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
    entryRules: [{ l: 'maFast', op: 'crossUp', r: 'maSlow' }],
    exitRules: [{ l: 'maFast', op: 'crossDown', r: 'maSlow' }],
    slPct: 0,
    tpPct: 0,
    feePct: 0.05,
    slipPct: 0.02,
    sizePct: 100,
    fillMode: 'close',
    direction: 'long',
  };
}
