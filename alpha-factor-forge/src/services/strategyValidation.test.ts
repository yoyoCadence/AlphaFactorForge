import { describe, it, expect } from 'vitest';
import {
  HARD_VALIDATED_PARAM_KEYS,
  LEGACY_CLAMPED_PARAM_KEYS,
  STRATEGY_PARAM_RULES_VERSION,
  assertStrategyParams,
  validateStrategyParams,
  type HardValidatedParamKey,
} from './strategyValidation';
import { checkNumericParam } from './discoveryConfig';
import { defaultStrategy, type ParamsStrategy } from './strategy';
import { runParamsBacktest } from './backtestRunner';
import { buildStrategyDef } from './strategyRecord';
import { runParamSweep } from './paramSweep';
import type { Candle } from '../core/backtest';

function strategy(overrides: Partial<ParamsStrategy> = {}): ParamsStrategy {
  return { ...defaultStrategy(), ...overrides };
}

/** Deterministic rising/falling series, long enough for the default periods. */
function candles(count = 120): Candle[] {
  return Array.from({ length: count }, (_, i) => {
    const base = 100 + Math.sin(i / 6) * 8 + i * 0.15;
    return { t: 1_700_000_000_000 + i * 3_600_000, o: base, h: base + 1, l: base - 1, c: base + 0.4, v: 10 + i };
  });
}

describe('parameter classification', () => {
  it('covers every numeric ParamsStrategy field exactly once', () => {
    const numericKeys = Object.entries(defaultStrategy())
      .filter(([, value]) => typeof value === 'number')
      .map(([key]) => key)
      .sort();
    const classified = [...HARD_VALIDATED_PARAM_KEYS, ...LEGACY_CLAMPED_PARAM_KEYS].sort();
    // A new numeric strategy field must be classified as hard-validated or as
    // owned by the legacy execution-cost clamping — never silently neither.
    expect(classified).toEqual(numericKeys);
    expect(new Set(classified).size).toBe(classified.length);
  });

  it('hard-validates the eleven indicator-grid fields', () => {
    expect([...HARD_VALIDATED_PARAM_KEYS]).toEqual([
      'fastMA', 'slowMA', 'emaPeriod', 'rsiPeriod', 'rsiBuy', 'rsiSell',
      'macdFast', 'macdSlow', 'macdSignal', 'bbPeriod', 'bbMult',
    ]);
    expect(STRATEGY_PARAM_RULES_VERSION).toBe('strategy-params-v1');
  });
});

describe('validateStrategyParams', () => {
  it('accepts the shipped defaults with no issue and no warning', () => {
    expect(validateStrategyParams(defaultStrategy())).toEqual({ ok: true, issues: [], warnings: [] });
  });

  const PERIOD_KEYS: HardValidatedParamKey[] = [
    'fastMA', 'slowMA', 'emaPeriod', 'rsiPeriod', 'macdFast', 'macdSlow', 'macdSignal', 'bbPeriod',
  ];

  it.each(PERIOD_KEYS)('rejects a zero, negative, fractional, non-finite or unsafe %s', (key) => {
    for (const bad of [0, -1, 2.5, Number.NaN, Number.POSITIVE_INFINITY, Number.MAX_SAFE_INTEGER + 1]) {
      const result = validateStrategyParams(strategy({ [key]: bad }));
      expect(result.ok, `${key}=${bad} must be rejected`).toBe(false);
      expect(result.issues.map((i) => i.key)).toContain(key);
    }
    expect(validateStrategyParams(strategy({ [key]: 1 })).ok).toBe(true);
  });

  it.each(['rsiBuy', 'rsiSell'] as const)('keeps %s inside the RSI 0-100 range', (key) => {
    for (const bad of [-1, 101, Number.NaN]) {
      expect(validateStrategyParams(strategy({ [key]: bad })).ok).toBe(false);
    }
    // The bounds themselves are legal, and a fractional level is meaningful.
    for (const good of [0, 100, 30.5]) {
      expect(validateStrategyParams(strategy({ rsiBuy: 0, rsiSell: 100, [key]: good })).ok).toBe(true);
    }
  });

  it('requires a positive Bollinger multiplier', () => {
    expect(validateStrategyParams(strategy({ bbMult: 0 })).ok).toBe(false);
    expect(validateStrategyParams(strategy({ bbMult: -2 })).ok).toBe(false);
    expect(validateStrategyParams(strategy({ bbMult: 0.5 })).ok).toBe(true);
  });

  // The single-rule-set guarantee: this module decides the wording, never the
  // verdict. If someone re-implements a rule here, this fails.
  it('agrees with checkNumericParam for every hard-validated key', () => {
    const battery = [0, -0, 1, -1, 2.5, 14, 100, 101, -100, 0.1, Number.NaN, Number.POSITIVE_INFINITY, Number.MAX_SAFE_INTEGER + 1];
    for (const key of HARD_VALIDATED_PARAM_KEYS) {
      for (const value of battery) {
        const flagged = validateStrategyParams(strategy({ [key]: value })).issues.some((i) => i.key === key);
        expect(flagged, `${key}=${value}`).toBe(checkNumericParam(key, value) != null);
      }
    }
  });

  it('reports every offending field, naming the key and its current value', () => {
    const result = validateStrategyParams(strategy({ fastMA: 2.5, bbPeriod: 0 }));
    expect(result.issues.map((i) => i.key)).toEqual(['fastMA', 'bbPeriod']);
    expect(result.issues[0].message).toContain('fastMA');
    expect(result.issues[0].message).toContain('2.5');
    expect(result.issues[0].value).toBe(2.5);
  });

  // The execution model keeps its documented legacy clamping (sizePct 0 = 100%,
  // negative fee clamps to 0), which has its own committed tests.
  it.each(LEGACY_CLAMPED_PARAM_KEYS)('leaves the legacy-clamped %s alone', (key) => {
    for (const value of [0, -1, 150]) {
      expect(validateStrategyParams(strategy({ [key]: value })).ok, `${key}=${value}`).toBe(true);
    }
  });
});

describe('cross-field warnings', () => {
  it.each([
    ['fastMA<slowMA', { fastMA: 30, slowMA: 21 }],
    ['macdFast<macdSlow', { macdFast: 30, macdSlow: 26 }],
    ['rsiBuy<rsiSell', { rsiBuy: 70, rsiSell: 30 }],
  ] as [string, Partial<ParamsStrategy>][])('warns about %s without blocking', (rule, overrides) => {
    const result = validateStrategyParams(strategy(overrides));
    // Computable, merely dubious: it must stay runnable and saveable.
    expect(result.ok).toBe(true);
    expect(result.issues).toEqual([]);
    expect(result.warnings.map((w) => w.rule)).toEqual([rule]);
  });

  it('stays quiet while a hard rule is failing, so the real error is not buried', () => {
    const result = validateStrategyParams(strategy({ fastMA: 0, slowMA: 21 }));
    expect(result.ok).toBe(false);
    expect(result.warnings).toEqual([]);
  });
});

describe('assertStrategyParams', () => {
  it('throws a RangeError naming the first offending field', () => {
    expect(() => assertStrategyParams(strategy({ slowMA: 0, bbMult: 0 }))).toThrow(RangeError);
    expect(() => assertStrategyParams(strategy({ slowMA: 0, bbMult: 0 }))).toThrow(/slowMA/);
  });

  it('passes a valid strategy through untouched', () => {
    const strat = defaultStrategy();
    expect(() => assertStrategyParams(strat)).not.toThrow();
    expect(strat).toEqual(defaultStrategy());
  });
});

describe('mount points', () => {
  it('rejects a fractional period at the execution boundary instead of reporting zero trades', () => {
    const rows = candles();
    // Before this gate, sma(closes, 2.5) indexed values[i - 2.5] -> undefined ->
    // NaN for every bar, and this call returned a confident 0-trade result.
    expect(() => runParamsBacktest({ candles: rows, strat: strategy({ fastMA: 2.5 }), interval: '1h' }))
      .toThrow(/fastMA/);
    expect(runParamsBacktest({ candles: rows, strat: defaultStrategy(), interval: '1h' }).metrics).toBeDefined();
  });

  it('rejects at the persistence boundary before a durable identity is computed', async () => {
    await expect(buildStrategyDef(strategy({ rsiPeriod: 0 }), 'bad')).rejects.toThrow(/rsiPeriod/);
    const def = await buildStrategyDef(defaultStrategy(), 'good');
    expect(def.strategy_hash).toMatch(/^strategy-v2:/);
  });

  it('turns an unevaluable sweep cell into an empty cell rather than a zero-trade score', () => {
    const result = runParamSweep({
      candles: candles(),
      strat: defaultStrategy(),
      interval: '1h',
      // A fractional step makes every other value fractional: 5, 5.5, 6, 6.5 …
      sweep: { x: { key: 'fastMA', min: 5, max: 6.5, step: 0.5 }, y: null, metric: 'net' },
    });
    const cells = result.grid[0];
    expect(cells.map((c) => c.x)).toEqual([5, 5.5, 6, 6.5]);
    expect(cells.filter((c) => c.x % 1 !== 0).every((c) => c.metric === null && c.trades === 0)).toBe(true);
    // The integer columns still evaluate, so the sweep degrades per cell only.
    expect(cells.filter((c) => c.x % 1 === 0).every((c) => c.metric !== null || c.trades === 0)).toBe(true);
  });
});
