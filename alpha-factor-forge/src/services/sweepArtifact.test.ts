import { describe, it, expect } from 'vitest';
import {
  SWEEP_CONTEXT_VERSION,
  createSweepArtifact,
  describeSweepBasis,
  describeSweepContext,
  normalizeSweepConfig,
  sameSweepContext,
  sweepContextKey,
  sweepRangeFromRunRange,
  sweepResultIsWritable,
  sweptParamKeys,
  type SweepContext,
} from './sweepArtifact';
import { describeRunContext, type RunDatasetSnapshot } from './runArtifact';
import { holdoutSplitIndex } from './holdout';
import { defaultStrategy, type ParamsStrategy } from './strategy';
import type { SweepConfig, SweepResult } from './paramSweep';

const DATASET: RunDatasetSnapshot = {
  id: 7,
  hash: 'dataset-content-v2:aa',
  symbol: 'SAMPLE',
  interval: '1h',
  startTime: 1_700_000_000_000,
  endTime: 1_700_003_600_000,
  barCount: 600,
};

const CONFIG_1D: SweepConfig = { x: { key: 'fastMA', min: 5, max: 20, step: 1 }, y: null, metric: 'net' };
const CONFIG_2D: SweepConfig = {
  x: { key: 'fastMA', min: 5, max: 20, step: 1 },
  y: { key: 'slowMA', min: 20, max: 40, step: 2 },
  metric: 'net',
};

function context(overrides: {
  dataset?: Partial<RunDatasetSnapshot>;
  strategy?: Partial<ParamsStrategy>;
  holdout?: boolean;
  holdoutPct?: number;
  config?: SweepConfig;
} = {}): SweepContext {
  return describeSweepContext({
    run: describeRunContext({
      dataset: { ...DATASET, ...overrides.dataset },
      strategy: { ...defaultStrategy(), ...overrides.strategy },
      holdout: overrides.holdout ?? false,
      holdoutPct: overrides.holdoutPct ?? 30,
    }),
    config: overrides.config ?? CONFIG_1D,
  });
}

function result(bestX = 7): SweepResult {
  return {
    xs: [5, 6, 7],
    ys: [null],
    grid: [[
      { x: 5, y: null, metric: 0.1, trades: 4 },
      { x: 6, y: null, metric: null, trades: 0 },
      { x: 7, y: null, metric: 0.4, trades: 6 },
    ]],
    metric: 'net',
    xKey: 'fastMA',
    yKey: null,
    best: { x: bestX, y: null, metric: 0.4, trades: 6 },
    lo: 0.1,
    hi: 0.4,
  };
}

describe('sweepRangeFromRunRange', () => {
  it('covers every bar and records no split when holdout is off', () => {
    const run = describeRunContext({ dataset: DATASET, strategy: defaultStrategy(), holdout: false, holdoutPct: 30 });
    expect(sweepRangeFromRunRange(run.range)).toEqual({ from: 0, to: 599, holdout: null });
  });

  it('optimises the in-sample segment only, on the shared holdout boundary', () => {
    const run = describeRunContext({ dataset: DATASET, strategy: defaultStrategy(), holdout: true, holdoutPct: 30 });
    const split = holdoutSplitIndex(600, 30);
    expect(split).toBe(420);
    // The out-of-sample tail [420, 599] is NOT part of what the sweep optimised.
    expect(sweepRangeFromRunRange(run.range)).toEqual({ from: 0, to: split - 1, holdout: { pct: 30, splitIndex: split } });
  });
});

describe('normalizeSweepConfig / sweptParamKeys', () => {
  it('keys an absent second axis the same as an explicit null', () => {
    const absent = { x: CONFIG_1D.x, metric: 'net' } as SweepConfig;
    expect(normalizeSweepConfig(absent)).toEqual(normalizeSweepConfig(CONFIG_1D));
    expect(sweepContextKey(context({ config: absent }))).toBe(sweepContextKey(context({ config: CONFIG_1D })));
  });

  it('lists the swept axes sorted and de-duplicated', () => {
    expect(sweptParamKeys(CONFIG_1D)).toEqual(['fastMA']);
    expect(sweptParamKeys(CONFIG_2D)).toEqual(['fastMA', 'slowMA']);
    expect(sweptParamKeys({ ...CONFIG_2D, y: { ...CONFIG_2D.x } })).toEqual(['fastMA']);
  });
});

describe('describeSweepBasis', () => {
  it('removes the swept axes and keeps every other strategy field', () => {
    const basis = describeSweepBasis(defaultStrategy(), CONFIG_2D);
    expect(basis.swept).toEqual(['fastMA', 'slowMA']);
    expect(basis.fixed).not.toHaveProperty('fastMA');
    expect(basis.fixed).not.toHaveProperty('slowMA');
    expect(basis.fixed.feePct).toBe(0.05);
    expect(basis.fixed.entryRules).toEqual([{ l: 'maFast', op: 'crossUp', r: 'maSlow' }]);
    // Every non-swept field of the strategy is present, so nothing can be
    // forgotten when ParamsStrategy grows a field.
    const expected = Object.keys(defaultStrategy()).filter((k) => k !== 'fastMA' && k !== 'slowMA').sort();
    expect(Object.keys(basis.fixed).sort()).toEqual(expected);
  });
});

describe('sweepContextKey / sameSweepContext', () => {
  it('treats identical inputs as the same sweep', () => {
    expect(sameSweepContext(context(), context())).toBe(true);
    expect(sweepContextKey(context())).toContain(SWEEP_CONTEXT_VERSION);
  });

  it('never matches a missing context, so "no dataset" and "not loaded" fail closed', () => {
    expect(sameSweepContext(context(), null)).toBe(false);
    expect(sameSweepContext(null, context())).toBe(false);
    expect(sameSweepContext(null, null)).toBe(false);
  });

  // The intentional case: a cell click writes exactly the swept axes, so it must
  // NOT invalidate the grid it was picked from (task acceptance criterion 4).
  it('stays valid when a swept axis value changes (applying a cell)', () => {
    expect(sameSweepContext(context(), context({ strategy: { fastMA: 7 } }))).toBe(true);
    expect(sameSweepContext(
      context({ config: CONFIG_2D }),
      context({ config: CONFIG_2D, strategy: { fastMA: 7, slowMA: 34 } }),
    )).toBe(true);
  });

  it('invalidates when a parameter that is NOT an axis of this sweep changes', () => {
    // slowMA is swept in the 2-D config above, so the 1-D sweep must reject it.
    expect(sameSweepContext(context(), context({ strategy: { slowMA: 34 } }))).toBe(false);
  });

  it.each([
    ['an execution cost', { strategy: { feePct: 0.2 } }],
    ['the slippage', { strategy: { slipPct: 0.5 } }],
    ['the position size', { strategy: { sizePct: 50 } }],
    ['a risk field', { strategy: { slPct: 2 } }],
    ['the direction', { strategy: { direction: 'short' as const } }],
    ['the fill mode', { strategy: { fillMode: 'nextOpen' as const } }],
    ['the strategy mode', { strategy: { mode: 'blocks' as const } }],
    ['a params signal', { strategy: { entrySig: 'rsiOversold' as const } }],
    ['a nested blocks rule', { strategy: { entryRules: [{ l: 'rsi' as const, op: '>' as const, r: '70' }] } }],
    ['a code expression', { strategy: { entryCode: 'price > maSlow' } }],
    ['a non-axis indicator period', { strategy: { rsiPeriod: 21 } }],
    ['the dataset id', { dataset: { id: 8 } }],
    ['the dataset content hash', { dataset: { hash: 'dataset-content-v2:bb' } }],
    ['the interval', { dataset: { interval: '1d' } }],
    ['the symbol', { dataset: { symbol: 'BTCUSDT' } }],
    ['the loaded bar count', { dataset: { barCount: 599 } }],
    ['the dataset time range', { dataset: { startTime: 1 } }],
  ])('invalidates when %s changes', (_label, overrides) => {
    expect(sameSweepContext(context(), context(overrides))).toBe(false);
  });

  // The audit's shortest reproduction: a full-period grid must not survive the
  // Holdout toggle, or 套用最佳 tunes on the out-of-sample tail.
  it('invalidates when holdout is toggled or its percentage changes', () => {
    expect(sameSweepContext(context({ holdout: false }), context({ holdout: true }))).toBe(false);
    expect(sameSweepContext(
      context({ holdout: true, holdoutPct: 30 }),
      context({ holdout: true, holdoutPct: 40 }),
    )).toBe(false);
    expect(sameSweepContext(
      context({ holdout: true, holdoutPct: 30 }),
      context({ holdout: true, holdoutPct: 30 }),
    )).toBe(true);
  });

  it.each([
    ['the x axis parameter', { x: { key: 'emaPeriod' as const, min: 5, max: 20, step: 1 } }],
    ['the x axis start', { x: { key: 'fastMA' as const, min: 6, max: 20, step: 1 } }],
    ['the x axis end', { x: { key: 'fastMA' as const, min: 5, max: 21, step: 1 } }],
    ['the x axis step', { x: { key: 'fastMA' as const, min: 5, max: 20, step: 2 } }],
    ['the optimisation metric', { metric: 'sharpe' as const }],
    ['adding a second axis', { y: CONFIG_2D.y }],
  ] as [string, Partial<SweepConfig>][])('invalidates when %s changes', (_label, patch) => {
    expect(sameSweepContext(context(), context({ config: { ...CONFIG_1D, ...patch } }))).toBe(false);
  });

  it('invalidates when the second axis range changes', () => {
    expect(sameSweepContext(
      context({ config: CONFIG_2D }),
      context({ config: { ...CONFIG_2D, y: { key: 'slowMA', min: 20, max: 40, step: 4 } } }),
    )).toBe(false);
  });
});

describe('createSweepArtifact', () => {
  it('freezes the grid and detaches it from the caller', () => {
    const source = result();
    const artifact = createSweepArtifact({ context: context(), result: source });

    source.grid[0][0].metric = -99;
    source.best = null;

    expect(artifact.result.grid[0][0].metric).toBe(0.1);
    expect(artifact.result.best).toEqual({ x: 7, y: null, metric: 0.4, trades: 6 });
    expect(() => {
      artifact.result.grid[0].push({ x: 9, y: null, metric: 1, trades: 1 });
    }).toThrow(TypeError);
  });

  it('still describes the context it was built from', () => {
    const built = context({ holdout: true, holdoutPct: 30 });
    const artifact = createSweepArtifact({ context: built, result: result() });
    expect(sameSweepContext(artifact.context, built)).toBe(true);
    expect(artifact.context.range).toEqual({ from: 0, to: 419, holdout: { pct: 30, splitIndex: 420 } });
  });
});

describe('sweepResultIsWritable', () => {
  const started = context();

  it('accepts the newest sweep while its inputs are still live', () => {
    expect(sweepResultIsWritable({ started, live: context(), generation: 3, owner: 3 })).toBe(true);
  });

  it('discards a superseded sweep even when the inputs still match', () => {
    expect(sweepResultIsWritable({ started, live: context(), generation: 3, owner: 4 })).toBe(false);
  });

  it('discards a late completion whose inputs have changed', () => {
    expect(sweepResultIsWritable({
      started,
      live: context({ holdout: true }),
      generation: 3,
      owner: 3,
    })).toBe(false);
  });

  it('discards a completion that has no live inputs left at all', () => {
    expect(sweepResultIsWritable({ started, live: null, generation: 3, owner: 3 })).toBe(false);
  });
});
