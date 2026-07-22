// SCORE-001 acceptance tests — one per reviewer checklist item in the PR #61
// handoff Resolution.

import { describe, it, expect } from 'vitest';
import type { Metrics } from '../core/metrics';
import { defaultStrategy, type ParamsStrategy } from './strategy';
import type { ValidationRunResult } from './validationRun';
import {
  DEFAULT_SCORE_CONFIG,
  SCORE_FORMULA_VERSION,
  complexityUnits,
  scoreCandidate,
} from './score';

const zeroMetrics = (): Metrics => ({
  netReturn: 0,
  cagr: 0,
  maxDrawdown: 0,
  sharpe: 0,
  sortino: 0,
  calmar: 0,
  winRate: 0,
  tradeCount: 0,
  profitFactor: 0,
  avgTradeReturn: 0,
  medianTradeReturn: 0,
  avgHoldingBars: 0,
  exposure: 0,
  turnover: 0,
  largestWin: 0,
  largestLoss: 0,
  consecutiveLosses: 0,
  monthlyReturns: {},
});

const threeCalmMonths = { '2024-01': 0.02, '2024-02': 0.02, '2024-03': 0.02 };

const vMetrics = (over: Partial<Metrics> = {}): Metrics => ({
  ...zeroMetrics(),
  cagr: 0.5,
  sortino: 2.5,
  calmar: 10,
  profitFactor: 2,
  turnover: 0.05,
  monthlyReturns: threeCalmMonths,
  ...over,
});

const vRun = (m: Metrics): Pick<ValidationRunResult, 'validation'> => ({
  validation: { trades: [], equity: [], metrics: m },
});

const baseArgs = (metricsOver: Partial<Metrics> = {}) => ({
  validationRun: vRun(vMetrics(metricsOver)),
  strat: defaultStrategy(),
  testedCombinations: 100,
});

const entry = (b: ReturnType<typeof scoreCandidate>, id: string) =>
  [...b.components, ...b.penalties].find((e) => e.id === id)!;

describe('scoreCandidate — hand-computed baseline', () => {
  it('produces the exact score-v1 weighted sum with the default config', () => {
    const b = scoreCandidate(baseArgs());
    // components: cagr .5/1=.5, sortino 2.5/5=.5, calmar 10/5->1, regime 0,
    // pf (2-1)/2=.5, consistency sigma 0 -> 1  => 3.5
    // penalties: complexity 8/40*.5=.1, turnover .05/.1*.5=.25,
    // dataMining log10(100)/4*1=.5              => .85
    expect(b.score).toBeCloseTo(2.65, 12);
    expect(b.formulaVersion).toBe(SCORE_FORMULA_VERSION);
    expect(b.segment).toBe('validation');
    expect(b.components.map((c) => c.id)).toEqual([
      'cagr', 'sortino', 'calmar', 'regime', 'profitFactor', 'consistency',
    ]);
    expect(b.penalties.map((p) => p.id)).toEqual(['complexity', 'turnover', 'dataMining']);
    expect(entry(b, 'complexity').raw).toBe(8);
    expect(entry(b, 'dataMining').normalized).toBeCloseTo(0.5, 12);
  });

  it('is deterministic: same input/config -> identical breakdown', () => {
    expect(scoreCandidate(baseArgs())).toEqual(scoreCandidate(baseArgs()));
  });

  it('records the exact resolved config, including overrides', () => {
    const b = scoreCandidate({ ...baseArgs(), config: { caps: { sortino: 10 } } });
    expect(b.config.caps.sortino).toBe(10);
    expect(b.config.caps.cagr).toBe(DEFAULT_SCORE_CONFIG.caps.cagr);
    expect(entry(b, 'sortino').normalized).toBeCloseTo(0.25, 12);
  });
});

describe('scoreCandidate — config and input validation', () => {
  it('rejects invalid caps, weights, and testedCombinations', () => {
    expect(() => scoreCandidate({ ...baseArgs(), config: { caps: { cagr: 0 } } })).toThrow(RangeError);
    expect(() => scoreCandidate({ ...baseArgs(), config: { caps: { turnover: -1 } } })).toThrow(RangeError);
    expect(() => scoreCandidate({ ...baseArgs(), config: { caps: { profitFactor: 1 } } })).toThrow(RangeError);
    expect(() => scoreCandidate({ ...baseArgs(), config: { weights: { calmar: -0.1 } } })).toThrow(RangeError);
    expect(() => scoreCandidate({ ...baseArgs(), config: { weights: { cagr: Infinity } } })).toThrow(RangeError);
    expect(() => scoreCandidate({ ...baseArgs(), testedCombinations: 0 })).toThrow(RangeError);
    expect(() => scoreCandidate({ ...baseArgs(), testedCombinations: 1.5 })).toThrow(RangeError);
  });

  it('rejects a non-zero weight on the deferred regime component', () => {
    expect(() =>
      scoreCandidate({ ...baseArgs(), config: { weights: { regime: 0.5 } } }),
    ).toThrow(/REGIME-001/);
    const regime = entry(scoreCandidate(baseArgs()), 'regime');
    expect(regime).toMatchObject({
      raw: null, rawStatus: 'deferred', normalized: null, weight: 0, contribution: 0,
    });
  });
});

describe('scoreCandidate — non-finite and insufficient evidence', () => {
  it('gives positive Infinity full credit and survives a JSON round-trip', () => {
    const b = scoreCandidate(baseArgs({ sortino: Infinity }));
    const sortino = entry(b, 'sortino');
    expect(sortino).toMatchObject({ raw: null, rawStatus: 'positive_infinity', normalized: 1 });
    expect(JSON.parse(JSON.stringify(b))).toEqual(b); // fully JSON-safe
  });

  it('fails closed on NaN / negative Infinity / missing months; score stays finite', () => {
    const b = scoreCandidate(
      baseArgs({ sortino: NaN, calmar: -Infinity, monthlyReturns: { '2024-01': 0.02 } }),
    );
    expect(entry(b, 'sortino')).toMatchObject({ raw: null, rawStatus: 'invalid', normalized: 0 });
    expect(entry(b, 'calmar')).toMatchObject({ raw: null, rawStatus: 'invalid', normalized: 0 });
    expect(entry(b, 'consistency')).toMatchObject({
      raw: null, rawStatus: 'insufficient', normalized: 0,
    });
    expect(entry(b, 'consistency').evidence).toEqual({ monthCount: 1, monthlyStdDev: null });
    expect(Number.isFinite(b.score)).toBe(true);
  });

  it('consistency: zero sigma scores 1, higher sigma scores lower, 2 months is insufficient', () => {
    const at = (monthlyReturns: Record<string, number>) =>
      entry(scoreCandidate(baseArgs({ monthlyReturns })), 'consistency');
    expect(at(threeCalmMonths).normalized).toBe(1);
    const low = at({ a: 0.02, b: 0.021, c: 0.019 }).normalized!;
    const high = at({ a: 0.2, b: -0.2, c: 0.2 }).normalized!;
    expect(low).toBeGreaterThan(high);
    expect(at({ a: 0.02, b: 0.02 }).rawStatus).toBe('insufficient');
    expect(at(threeCalmMonths).evidence?.monthCount).toBe(3);
  });

  it('canonicalizes negative zero so the complete breakdown round-trips through JSON', () => {
    const zeroWeights = {
      cagr: -0,
      sortino: 0,
      calmar: 0,
      regime: -0,
      profitFactor: 0,
      consistency: 0,
      complexity: 0,
      turnover: 0,
      dataMining: 0,
    };
    const b = scoreCandidate({
      ...baseArgs({ cagr: -0 }),
      testedCombinations: 1,
      config: { weights: zeroWeights },
    });

    expect(Object.is(entry(b, 'cagr').raw, 0)).toBe(true);
    expect(Object.is(b.config.weights.cagr, 0)).toBe(true);
    expect(Object.is(b.config.weights.regime, 0)).toBe(true);
    expect(Object.is(b.score, 0)).toBe(true);
    expect(JSON.parse(JSON.stringify(b))).toEqual(b);
  });

  it('keeps population sigma finite for extreme finite monthly returns', () => {
    const b = scoreCandidate(baseArgs({
      monthlyReturns: {
        '2024-01': Number.MAX_VALUE,
        '2024-02': -Number.MAX_VALUE,
        '2024-03': 0,
      },
    }));
    const consistency = entry(b, 'consistency');

    expect(consistency.rawStatus).toBe('finite');
    expect(consistency.raw).not.toBeNull();
    expect(Number.isFinite(consistency.raw!)).toBe(true);
    expect(consistency.raw!).toBeGreaterThan(Number.MAX_VALUE / 2);
    expect(consistency.evidence?.monthlyStdDev).toBe(consistency.raw!);
    expect(JSON.parse(JSON.stringify(b))).toEqual(b);
  });
});

describe('scoreCandidate — finite score guarantee', () => {
  it('rejects finite weights whose aggregate contributions overflow', () => {
    expect(() => scoreCandidate({
      ...baseArgs(),
      config: {
        weights: {
          calmar: Number.MAX_VALUE,
          consistency: Number.MAX_VALUE,
        },
      },
    })).toThrow('resolved score weights produce a non-finite score');
  });
});

describe('complexityUnits — canonical cross-mode parity (Resolution D4)', () => {
  it('semantically equivalent MA-cross strategies yield identical units', () => {
    const params = defaultStrategy(); // maCrossUp / maCrossDown
    const blocks: ParamsStrategy = { ...defaultStrategy(), mode: 'blocks' }; // maFast cross maSlow rules
    const code: ParamsStrategy = {
      ...defaultStrategy(),
      mode: 'code',
      entryCode: 'crossUp(maFast, maSlow)',
      exitCode: 'crossDown(maFast, maSlow)',
    };
    const p = complexityUnits(params);
    expect(p).toEqual(complexityUnits(blocks));
    expect(p).toEqual(complexityUnits(code));
    // 6 decision nodes (2 signals x [op + 2 operands]) + 2 distinct params + 0 risk
    expect(p).toEqual({ units: 8, decisionNodes: 6, indicatorParams: 2, riskRules: 0 });
  });

  it('counts enabled SL/TP risk rules and AND connectors; fee/slip/size never count', () => {
    expect(complexityUnits({ ...defaultStrategy(), slPct: 2, tpPct: 3 }).units).toBe(10);
    expect(complexityUnits({ ...defaultStrategy(), feePct: 0.5, slipPct: 0.5, sizePct: 50 }).units).toBe(8);
    const twoRules: ParamsStrategy = {
      ...defaultStrategy(),
      mode: 'blocks',
      entryRules: [
        { l: 'maFast', op: 'crossUp', r: 'maSlow' },
        { l: 'rsi', op: '<', r: '30' },
      ],
      exitRules: [{ l: 'maFast', op: 'crossDown', r: 'maSlow' }],
    };
    // entry 3+3+1 connector, exit 3 = 10 nodes; fields fastMA/slowMA/rsiPeriod = 3
    expect(complexityUnits(twoRules)).toEqual({
      units: 13, decisionNodes: 10, indicatorParams: 3, riskRules: 0,
    });
  });

  it('fails closed on unsupported stoch signals and invalid code', () => {
    expect(() => complexityUnits({ ...defaultStrategy(), entrySig: 'stochOversold' })).toThrow(/stoch/i);
    expect(() =>
      complexityUnits({ ...defaultStrategy(), mode: 'code', entryCode: 'rsi >' }),
    ).toThrow(/invalid expression/);
  });
});

describe('scoreCandidate — lineage N and segment discipline', () => {
  it('the same final N gives the same dataMining entry regardless of call order', () => {
    const a1 = entry(scoreCandidate({ ...baseArgs(), testedCombinations: 500 }), 'dataMining');
    const other = scoreCandidate({ ...baseArgs({ cagr: 0.1 }), testedCombinations: 500 });
    const a2 = entry(scoreCandidate({ ...baseArgs(), testedCombinations: 500 }), 'dataMining');
    expect(a1).toEqual(a2);
    expect(entry(other, 'dataMining')).toEqual(a1);
    expect(a1.evidence).toEqual({ n: 500, basis: 'lineage-final-unique' });
  });

  it('never reads the Train or Test segments', () => {
    const run = vRun(vMetrics()) as Record<string, unknown>;
    Object.defineProperty(run, 'train', { get() { throw new Error('train was read'); } });
    Object.defineProperty(run, 'test', { get() { throw new Error('test was read'); } });
    const b = scoreCandidate({
      validationRun: run as unknown as Pick<ValidationRunResult, 'validation'>,
      strat: defaultStrategy(),
      testedCombinations: 1,
    });
    expect(Number.isFinite(b.score)).toBe(true);
  });
});
