// Params-mode strategy -> entry/exit boolean signal arrays.
//
// Pure: candles + a ParamsStrategy in, two boolean[] out (aligned to candles).
// Indicators come from core/indicators (the canonical implementations); this
// module only wires comparisons. The boolean arrays feed core/backtest, which
// never sees indicators. Signal semantics mirror the legacy prototype's
// makeSig() params branch (AlphaFactorForge.dc.html).

import { sma, ema, rsi, macd, bbands, type Series } from '../core/indicators';
import type { Candle } from '../core/backtest';
import { OPERAND_IDS, type ParamsStrategy, type SignalId, type Rule, type RuleOp, type OperandId } from './strategy';

export interface SignalSeries {
  entry: boolean[];
  exit: boolean[];
}

type Operand = Series | number; // a series, or a constant threshold (e.g. rsiBuy)

const at = (x: Operand, i: number): number =>
  typeof x === 'number' ? x : i >= 0 && i < x.length ? x[i] : Number.NaN;

/** Evaluate one comparison at bar i. NaN (indicator warm-up) -> false.
 *  Needs the previous bar, so i < 1 is always false (matches legacy). */
function evalCond(left: Operand, op: RuleOp, right: Operand, i: number): boolean {
  if (i < 1) return false;
  const a = at(left, i);
  const b = at(right, i);
  if (!Number.isFinite(a) || !Number.isFinite(b)) return false;
  const pa = at(left, i - 1);
  const pb = at(right, i - 1);
  switch (op) {
    case '>':
      return a > b;
    case '<':
      return a < b;
    case '>=':
      return a >= b;
    case '<=':
      return a <= b;
    case 'crossUp':
      return Number.isFinite(pa) && Number.isFinite(pb) && pa <= pb && a > b;
    case 'crossDown':
      return Number.isFinite(pa) && Number.isFinite(pb) && pa >= pb && a < b;
  }
}

/** Signal ids implementable with the current core/indicators set. The two
 *  stoch* ids in SignalId are intentionally absent until core gains STOCH. */
export const SUPPORTED_SIGNALS: SignalId[] = [
  'maCrossUp',
  'maCrossDown',
  'emaCrossUp',
  'emaCrossDown',
  'priceAboveSlow',
  'priceBelowSlow',
  'rsiOversold',
  'rsiOverbought',
  'macdCrossUp',
  'macdCrossDown',
  'bbLowerTouch',
  'bbUpperTouch',
];

export function buildParamsSignals(candles: Candle[], strat: ParamsStrategy): SignalSeries {
  const n = candles.length;
  const closes = candles.map((c) => c.c);

  const maFast = sma(closes, strat.fastMA);
  const maSlow = sma(closes, strat.slowMA);
  const emaArr = ema(closes, strat.emaPeriod);
  const rsiArr = rsi(closes, strat.rsiPeriod);
  const m = macd(closes, strat.macdFast, strat.macdSlow, strat.macdSignal);
  const bb = bbands(closes, strat.bbPeriod, strat.bbMult);

  const evals: Partial<Record<SignalId, (i: number) => boolean>> = {
    maCrossUp: (i) => evalCond(maFast, 'crossUp', maSlow, i),
    maCrossDown: (i) => evalCond(maFast, 'crossDown', maSlow, i),
    emaCrossUp: (i) => evalCond(closes, 'crossUp', emaArr, i),
    emaCrossDown: (i) => evalCond(closes, 'crossDown', emaArr, i),
    priceAboveSlow: (i) => evalCond(closes, '>', maSlow, i),
    priceBelowSlow: (i) => evalCond(closes, '<', maSlow, i),
    rsiOversold: (i) => evalCond(rsiArr, 'crossUp', strat.rsiBuy, i),
    rsiOverbought: (i) => evalCond(rsiArr, 'crossDown', strat.rsiSell, i),
    macdCrossUp: (i) => evalCond(m.macd, 'crossUp', m.signal, i),
    macdCrossDown: (i) => evalCond(m.macd, 'crossDown', m.signal, i),
    bbLowerTouch: (i) => evalCond(closes, '<', bb.lower, i),
    bbUpperTouch: (i) => evalCond(closes, '>', bb.upper, i),
  };

  const pick = (id: SignalId): ((i: number) => boolean) => {
    const fn = evals[id];
    if (!fn) {
      throw new Error(
        `unsupported signal "${id}": stoch* signals await a core STOCH indicator (Phase B)`,
      );
    }
    return fn;
  };

  const entryFn = pick(strat.entrySig);
  const exitFn = pick(strat.exitSig);

  const entry = new Array<boolean>(n);
  const exit = new Array<boolean>(n);
  for (let i = 0; i < n; i++) {
    entry[i] = entryFn(i);
    exit[i] = exitFn(i);
  }
  return { entry, exit };
}

// ---------- blocks mode (Slice 4a) ----------

/** Compute every named operand series once for a strategy's periods. */
function resolveSeries(candles: Candle[], strat: ParamsStrategy): Record<OperandId, Series> {
  const closes = candles.map((c) => c.c);
  const m = macd(closes, strat.macdFast, strat.macdSlow, strat.macdSignal);
  const bb = bbands(closes, strat.bbPeriod, strat.bbMult);
  return {
    price: closes,
    open: candles.map((c) => c.o),
    high: candles.map((c) => c.h),
    low: candles.map((c) => c.l),
    volume: candles.map((c) => c.v),
    maFast: sma(closes, strat.fastMA),
    maSlow: sma(closes, strat.slowMA),
    ema: ema(closes, strat.emaPeriod),
    rsi: rsi(closes, strat.rsiPeriod),
    macd: m.macd,
    macdSignal: m.signal,
    macdHist: m.hist,
    bbUpper: bb.upper,
    bbMid: bb.middle,
    bbLower: bb.lower,
  };
}

/** Build signals from blocks-mode rule lists. Each list ANDs its rules; an
 *  empty list never fires (matches legacy). The right operand may be a series
 *  id or a numeric constant string; unknown non-numeric operands -> never true. */
export function buildBlocksSignals(candles: Candle[], strat: ParamsStrategy): SignalSeries {
  const n = candles.length;
  const series = resolveSeries(candles, strat);
  const known = new Set<string>(OPERAND_IDS);
  const operand = (name: string): Operand => {
    const key = name.trim();
    if (known.has(key)) return series[key as OperandId];
    if (!key) return Number.NaN; // blank/whitespace -> never compares true (avoid Number('') === 0)
    const num = Number(key);
    return Number.isFinite(num) ? num : Number.NaN;
  };
  const evalRule = (r: Rule, i: number) => evalCond(operand(r.l), r.op, operand(r.r), i);
  const all = (rules: Rule[], i: number) => rules.length > 0 && rules.every((rl) => evalRule(rl, i));

  const entry = new Array<boolean>(n);
  const exit = new Array<boolean>(n);
  for (let i = 0; i < n; i++) {
    entry[i] = all(strat.entryRules, i);
    exit[i] = all(strat.exitRules, i);
  }
  return { entry, exit };
}

/** Dispatch by strategy mode. (code mode arrives in Slice 4b.) */
export function buildSignals(candles: Candle[], strat: ParamsStrategy): SignalSeries {
  return strat.mode === 'blocks' ? buildBlocksSignals(candles, strat) : buildParamsSignals(candles, strat);
}
