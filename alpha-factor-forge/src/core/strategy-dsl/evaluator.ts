// Pure evaluator for already-admitted `strategy-dsl-v1` trees.

import type { Candle, Signals } from '../backtest';
import { atr, ema, highest, lowest, roc, rsi, sma, stddev, wma } from '../indicators';
import type { ExprNode, ParamSpec, PriceSource, StrategyDSL } from './schema';
import { validateDSL } from './validator';

export interface DslEvaluation {
  signals: Signals;
  maxLookbackBars: number;
  nodeCount: number;
  parameterCount: number;
}

type Evaluated =
  | { type: 'number'; values: number[] }
  | { type: 'boolean'; values: boolean[] };

function source(candles: readonly Candle[], id: PriceSource): number[] {
  return candles.map((candle) => {
    if (id === 'OPEN') return candle.o;
    if (id === 'HIGH') return candle.h;
    if (id === 'LOW') return candle.l;
    if (id === 'VOLUME') return candle.v;
    if (id === 'HLC3') return (candle.h + candle.l + candle.c) / 3;
    return candle.c;
  });
}

function parameterValues(dsl: StrategyDSL): Map<string, number> {
  return new Map(Object.entries(dsl.params).map(([name, spec]) => [
    name,
    typeof spec === 'number' ? spec : (spec as ParamSpec).default,
  ]));
}

function scalar(value: number | string | undefined, values: ReadonlyMap<string, number>): number {
  if (typeof value === 'number') return value;
  return values.get((value as string).slice(1)) as number;
}

function numeric(result: Evaluated): number[] {
  if (result.type !== 'number') throw new TypeError('validated DSL type invariant failed');
  return result.values;
}

function logical(result: Evaluated): boolean[] {
  if (result.type !== 'boolean') throw new TypeError('validated DSL type invariant failed');
  return result.values;
}

function evaluate(
  node: ExprNode,
  candles: readonly Candle[],
  values: ReadonlyMap<string, number>,
): Evaluated {
  if ('ind' in node) {
    if (['CLOSE', 'OPEN', 'HIGH', 'LOW', 'HLC3'].includes(node.ind)) {
      return { type: 'number', values: source(candles, node.ind as PriceSource) };
    }
    const length = scalar(node.len, values);
    if (node.ind === 'ATR') {
      return {
        type: 'number',
        values: atr(source(candles, 'HIGH'), source(candles, 'LOW'), source(candles, 'CLOSE'), length),
      };
    }
    const input = source(candles, node.src as PriceSource);
    if (node.ind === 'EMA') return { type: 'number', values: ema(input, length) };
    if (node.ind === 'SMA') return { type: 'number', values: sma(input, length) };
    if (node.ind === 'WMA') return { type: 'number', values: wma(input, length) };
    if (node.ind === 'RSI') return { type: 'number', values: rsi(input, length) };
    if (node.ind === 'ROC') return { type: 'number', values: roc(input, length) };
    if (node.ind === 'STDDEV') return { type: 'number', values: stddev(input, length) };
    if (node.ind === 'HIGHEST') return { type: 'number', values: highest(input, length) };
    return { type: 'number', values: lowest(input, length) };
  }

  if (node.op === 'CONST') {
    return { type: 'number', values: new Array(candles.length).fill(scalar(node.v, values)) };
  }
  const children = (node.args ?? []).map((child) => evaluate(child, candles, values));
  if (node.op === 'SHIFT') {
    const offset = scalar(node.n, values);
    if (children[0].type === 'boolean') {
      const input = logical(children[0]);
      return { type: 'boolean', values: input.map((_, index) => index >= offset && input[index - offset]) };
    }
    const input = numeric(children[0]);
    return { type: 'number', values: input.map((_, index) => index >= offset ? input[index - offset] : Number.NaN) };
  }
  if (node.op === 'RISING' || node.op === 'FALLING') {
    const input = numeric(children[0]);
    const periods = scalar(node.n, values);
    return {
      type: 'boolean',
      values: input.map((_, index) => {
        if (index < periods) return false;
        for (let offset = 0; offset < periods; offset++) {
          const current = input[index - offset];
          const previous = input[index - offset - 1];
          if (!Number.isFinite(current) || !Number.isFinite(previous)) return false;
          if (node.op === 'RISING' ? current <= previous : current >= previous) return false;
        }
        return true;
      }),
    };
  }
  if (node.op === 'AND' || node.op === 'OR') {
    const left = logical(children[0]);
    const right = logical(children[1]);
    return { type: 'boolean', values: left.map((value, index) => node.op === 'AND' ? value && right[index] : value || right[index]) };
  }
  if (node.op === 'NOT') {
    return { type: 'boolean', values: logical(children[0]).map((value) => !value) };
  }
  const left = numeric(children[0]);
  const right = children[1] ? numeric(children[1]) : [];
  if (['GT', 'LT', 'GTE', 'LTE', 'CROSS_UP', 'CROSS_DOWN'].includes(node.op)) {
    return {
      type: 'boolean',
      values: left.map((a, index) => {
        const b = right[index];
        if (!Number.isFinite(a) || !Number.isFinite(b)) return false;
        if (node.op === 'GT') return a > b;
        if (node.op === 'LT') return a < b;
        if (node.op === 'GTE') return a >= b;
        if (node.op === 'LTE') return a <= b;
        if (index === 0 || !Number.isFinite(left[index - 1]) || !Number.isFinite(right[index - 1])) return false;
        return node.op === 'CROSS_UP'
          ? left[index - 1] <= right[index - 1] && a > b
          : left[index - 1] >= right[index - 1] && a < b;
      }),
    };
  }
  return {
    type: 'number',
    values: left.map((a, index) => {
      if (node.op === 'ABS') return Number.isFinite(a) ? Math.abs(a) : Number.NaN;
      const b = right[index];
      if (!Number.isFinite(a) || !Number.isFinite(b)) return Number.NaN;
      if (node.op === 'ADD') return a + b;
      if (node.op === 'SUB') return a - b;
      if (node.op === 'MUL') return a * b;
      if (node.op === 'DIV') return b === 0 ? Number.NaN : a / b;
      if (node.op === 'MIN') return Math.min(a, b);
      if (node.op === 'MAX') return Math.max(a, b);
      const high = numeric(children[2])[index];
      return Number.isFinite(high) ? Math.min(Math.max(a, b), high) : Number.NaN;
    }),
  };
}

export function evaluateDSL(candles: readonly Candle[], input: unknown): DslEvaluation {
  const validation = validateDSL(input);
  if (!validation.ok) throw new RangeError(`invalid ${validation.errors.join('; ')}`);
  const dsl = input as StrategyDSL;
  const values = parameterValues(dsl);
  const entry = evaluate(dsl.entry, candles, values);
  const exit = evaluate(dsl.exit, candles, values);
  return {
    signals: { entry: logical(entry), exit: logical(exit) },
    maxLookbackBars: validation.maxLookbackBars,
    nodeCount: validation.nodeCount,
    parameterCount: validation.parameterCount,
  };
}
