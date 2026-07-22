import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/gate-score-v1.json';
import { sha256Hex } from '../core/hashing';
import benchmarksSource from '../services/benchmarks.ts?raw';
import gateSource from '../services/gate.ts?raw';
import metricsCodecSource from '../services/metricsCodec.ts?raw';
import nonFiniteSource from '../services/nonFinite.ts?raw';
import scoreSource from '../services/score.ts?raw';
import strategySource from '../services/strategy.ts?raw';
import validationRecordSource from '../services/validationRecord.ts?raw';
import generatorSource from './gateScoreFixture.ts?raw';
import { canonicalizeFixtureSource, FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import {
  EXPECTED_FLOAT_ENCODING,
  SPECIAL_INPUT_NUMBER_ENCODING,
  buildGateScoreParityFixture,
} from './gateScoreFixture';

async function hashSource(source: string): Promise<string> {
  return `sha256:${await sha256Hex(canonicalizeFixtureSource(source))}`;
}

const GATE_ORDER = [
  'minTrades',
  'avgTradeReturn',
  'rollingConsistency',
  'maxDrawdown',
  'monthlyConcentration',
  'tradeConcentration',
  'benchmarkWins',
  'randomEntryPercentile',
];

const COMPONENT_ORDER = [
  'cagr',
  'sortino',
  'calmar',
  'regime',
  'profitFactor',
  'consistency',
];

const PENALTY_ORDER = ['complexity', 'turnover', 'dataMining'];

function findById<T extends { id: string }>(cases: T[], id: string): T {
  const found = cases.find((parityCase) => parityCase.id === id);
  if (!found) throw new Error(`missing fixture case ${id}`);
  return found;
}

function expectFiniteJson(value: unknown): void {
  if (typeof value === 'number') {
    expect(Number.isFinite(value)).toBe(true);
    expect(Object.is(value, -0)).toBe(false);
    if (!Number.isInteger(value)) {
      expect(value).toBe(Number(value.toPrecision(15)));
    }
    return;
  }
  if (Array.isArray(value)) {
    for (const child of value) expectFiniteJson(child);
    return;
  }
  if (value !== null && typeof value === 'object') {
    for (const child of Object.values(value)) expectFiniteJson(child);
  }
}

describe('RS-CORE Gate + Score parity fixture', () => {
  it('is exactly reproducible from canonical current TypeScript reference sources', async () => {
    const regenerated = buildGateScoreParityFixture({
      generator: await hashSource(generatorSource),
      gate: await hashSource(gateSource),
      score: await hashSource(scoreSource),
      strategy: await hashSource(strategySource),
      benchmarks: await hashSource(benchmarksSource),
      validationRecord: await hashSource(validationRecordSource),
      metricsCodec: await hashSource(metricsCodecSource),
      nonFinite: await hashSource(nonFiniteSource),
    });
    expect(regenerated).toEqual(fixture);
  });

  it('locks the envelope, numeric policy, and every success/error inventory', () => {
    expect(fixture.schemaVersion).toBe('rs-core-parity-fixture-v1');
    expect(fixture.fixtureVersion).toBe('gate-score-parity-v1');
    expect(fixture.contracts).toEqual({ metrics: 'metrics-v1', gate: 'gate-v1', score: 'score-v1' });
    expect(fixture.generator.sourceHashEncoding).toBe(FIXTURE_SOURCE_HASH_ENCODING);
    expect(fixture.numericEncoding).toEqual({
      specialInputNumbers: SPECIAL_INPUT_NUMBER_ENCODING,
      expectedTolerantFloats: EXPECTED_FLOAT_ENCODING,
    });
    expect(fixture.tolerance.default).toEqual({ absolute: 1e-12, relative: 1e-10 });
    const encodedFixture = JSON.stringify(fixture);
    for (const tag of ['positive_infinity', 'negative_infinity', 'nan', 'negative_zero']) {
      expect(encodedFixture).toContain(`"${tag}"`);
    }
    expect(encodedFixture).not.toContain('"train":');
    expect(encodedFixture).not.toContain('"test":');

    expect(fixture.complexityCases.map((parityCase) => parityCase.id)).toEqual([
      'complexity-ma-family',
      'complexity-ema-family',
      'complexity-price-slow-family-one-risk',
      'complexity-rsi-family-two-risks',
      'complexity-macd-family',
      'complexity-bollinger-family',
    ]);
    expect(fixture.gateCases.map((parityCase) => parityCase.id)).toEqual([
      'gate-default-pass',
      'gate-full-config-boundary-pass',
      'gate-partial-config-pass',
      'gate-min-trades-fail',
      'gate-unsafe-trade-count-fails-closed',
      'gate-nonfinite-trade-count-fails-closed',
      'gate-fractional-trade-count-fails-closed',
      'gate-negative-trade-count-fails-closed',
      'gate-avg-return-strict-tie',
      'gate-rolling-consistency-fail',
      'gate-max-drawdown-fail',
      'gate-monthly-concentration-fail',
      'gate-trade-concentration-fail',
      'gate-benchmark-strict-tie',
      'gate-random-percentile-fail',
      'gate-short-equity-fails-closed',
      'gate-nonpositive-profit-fails-closed',
      'gate-utc-month-boundary-pass',
      'gate-invalid-date-evidence-fails-closed',
      'gate-finite-ratio-overflow-fails-closed',
      'gate-nonfinite-statuses-fail-closed',
      'gate-nonfinite-derived-evidence-fails-closed',
    ]);
    expect(fixture.scoreCases.map((parityCase) => parityCase.id)).toEqual([
      'score-default-baseline',
      'score-partial-config-population-sigma-max-safe-n',
      'score-nonfinite-statuses-negative-zero-insufficient',
      'score-extreme-finite-months-and-clamps',
    ]);
    expect(fixture.gateErrorCases.map((parityCase) => parityCase.id)).toEqual([
      'gate-duplicate-benchmark',
      'gate-missing-benchmark',
      'gate-invalid-min-trades',
      'gate-fractional-min-trades',
      'gate-min-trades-above-safe-range',
      'gate-nonfinite-min-avg-return',
      'gate-invalid-rolling-window',
      'gate-fractional-rolling-window',
      'gate-rolling-window-above-safe-range',
      'gate-invalid-min-rolling-ratio',
      'gate-invalid-max-drawdown',
      'gate-invalid-monthly-contribution',
      'gate-invalid-single-trade-contribution',
      'gate-negative-percentile',
      'gate-invalid-percentile',
      'gate-nonfinite-percentile',
    ]);
    expect(fixture.gateErrorCases.map((parityCase) => parityCase.expectedErrorIncludes)).toEqual([
      'duplicate deterministic benchmark',
      'missing deterministic benchmark',
      'minTrades',
      'minTrades',
      'minTrades',
      'minAvgTradeReturn',
      'rollingWindowBars',
      'rollingWindowBars',
      'rollingWindowBars',
      'minRollingPositiveRatio',
      'maxDrawdown',
      'maxMonthlyContribution',
      'maxSingleTradeContribution',
      'minRandomEntryPercentile',
      'minRandomEntryPercentile',
      'minRandomEntryPercentile',
    ]);
    expect(fixture.scoreErrorCases.map((parityCase) => parityCase.id)).toEqual([
      'score-invalid-cap-zero',
      'score-invalid-profit-factor-cap',
      'score-invalid-negative-weight',
      'score-invalid-nonfinite-weight',
      'score-invalid-nonfinite-cap',
      'score-deferred-regime-weight',
      'score-tested-combinations-zero',
      'score-tested-combinations-fractional',
      'score-tested-combinations-above-safe-range',
      'score-unsupported-stoch-signal',
      'score-resolved-weight-aggregate-overflow',
    ]);
  });

  it('keeps the candidate surface params-only and covers all 12 supported signal ids', () => {
    const strategies = fixture.complexityCases.map((parityCase) => parityCase.input.strategy);
    const signalIds = strategies.flatMap((strategy) => [strategy.entrySig, strategy.exitSig]);
    expect(signalIds).toEqual([
      'maCrossUp', 'maCrossDown',
      'emaCrossUp', 'emaCrossDown',
      'priceAboveSlow', 'priceBelowSlow',
      'rsiOversold', 'rsiOverbought',
      'macdCrossUp', 'macdCrossDown',
      'bbLowerTouch', 'bbUpperTouch',
    ]);
    for (const strategy of strategies) {
      expect(Object.keys(strategy).sort()).toEqual(['entrySig', 'exitSig', 'slPct', 'tpPct']);
      expect(strategy).not.toHaveProperty('mode');
      expect(strategy).not.toHaveProperty('entryRules');
      expect(strategy).not.toHaveProperty('entryCode');
    }
    expect(fixture.complexityCases.map((parityCase) => parityCase.expected.units)).toEqual([
      8, 7, 8, 11, 9, 8,
    ]);
  });

  it('locks encoded Gate structure/order, isolated failures, boundaries, and non-finite statuses', () => {
    for (const parityCase of fixture.gateCases) {
      expect(parityCase.expected.version).toBe('gate-v1');
      expect(parityCase.expected.criteria.map((criterion) => criterion.id)).toEqual(GATE_ORDER);
      for (const criterion of parityCase.expected.criteria) {
        expect(['positive_infinity', 'negative_infinity', 'nan', null]).toContain(
          criterion.valueStatus,
        );
      }
      expectFiniteJson(parityCase.expected);
    }

    const isolated: Record<string, string[]> = {
      'gate-min-trades-fail': ['minTrades'],
      'gate-unsafe-trade-count-fails-closed': ['minTrades'],
      'gate-nonfinite-trade-count-fails-closed': ['minTrades'],
      'gate-fractional-trade-count-fails-closed': ['minTrades'],
      'gate-negative-trade-count-fails-closed': ['minTrades'],
      'gate-avg-return-strict-tie': ['avgTradeReturn'],
      'gate-rolling-consistency-fail': ['rollingConsistency'],
      'gate-max-drawdown-fail': ['maxDrawdown'],
      'gate-monthly-concentration-fail': ['monthlyConcentration'],
      'gate-trade-concentration-fail': ['tradeConcentration'],
      'gate-benchmark-strict-tie': ['benchmarkWins'],
      'gate-random-percentile-fail': ['randomEntryPercentile'],
    };
    for (const [id, expectedFailed] of Object.entries(isolated)) {
      const parityCase = findById(fixture.gateCases, id);
      expect(
        parityCase.expected.criteria.filter((criterion) => !criterion.pass).map((criterion) => criterion.id),
      ).toEqual(expectedFailed);
    }

    expect(findById(fixture.gateCases, 'gate-default-pass').expected.pass).toBe(true);
    expect(Object.keys(findById(fixture.gateCases, 'gate-full-config-boundary-pass').input.config!))
      .toHaveLength(8);
    expect(findById(fixture.gateCases, 'gate-partial-config-pass').input.config).toEqual({ minTrades: 5 });

    const utc = findById(fixture.gateCases, 'gate-utc-month-boundary-pass');
    expect(utc.expected.pass).toBe(true);
    expect(utc.expected.criteria.find((criterion) => criterion.id === 'monthlyConcentration')?.value)
      .toBe(0.5);

    const nonFiniteTradeCount = findById(
      fixture.gateCases,
      'gate-nonfinite-trade-count-fails-closed',
    );
    expect(nonFiniteTradeCount.expected.criteria.find((criterion) => criterion.id === 'minTrades'))
      .toMatchObject({ value: null, valueStatus: 'nan', pass: false });
    expect(
      findById(fixture.gateCases, 'gate-fractional-trade-count-fails-closed')
        .expected.criteria.find((criterion) => criterion.id === 'minTrades'),
    ).toMatchObject({ value: 36.5, valueStatus: null, pass: false });
    expect(
      findById(fixture.gateCases, 'gate-negative-trade-count-fails-closed')
        .expected.criteria.find((criterion) => criterion.id === 'minTrades'),
    ).toMatchObject({ value: -1, valueStatus: null, pass: false });

    const invalidDate = findById(fixture.gateCases, 'gate-invalid-date-evidence-fails-closed');
    expect(invalidDate.expected.criteria.find((criterion) => criterion.id === 'monthlyConcentration'))
      .toEqual({
        id: 'monthlyConcentration',
        pass: false,
        value: null,
        valueStatus: null,
        threshold: 0.4,
        detail: 'invalid trade exit-time evidence',
      });

    const ratioOverflow = findById(
      fixture.gateCases,
      'gate-finite-ratio-overflow-fails-closed',
    );
    expect(
      ratioOverflow.expected.criteria.filter((criterion) => !criterion.pass).map((criterion) => ({
        id: criterion.id,
        value: criterion.value,
        valueStatus: criterion.valueStatus,
        detail: 'detail' in criterion ? criterion.detail : undefined,
      })),
    ).toEqual([
      {
        id: 'monthlyConcentration',
        value: null,
        valueStatus: null,
        detail: 'non-finite profit evidence',
      },
      {
        id: 'tradeConcentration',
        value: null,
        valueStatus: null,
        detail: 'non-finite profit evidence',
      },
    ]);

    const statuses = findById(fixture.gateCases, 'gate-nonfinite-statuses-fail-closed');
    expect(statuses.expected.criteria.find((criterion) => criterion.id === 'avgTradeReturn'))
      .toMatchObject({ value: null, valueStatus: 'positive_infinity', pass: false });
    expect(statuses.expected.criteria.find((criterion) => criterion.id === 'maxDrawdown'))
      .toMatchObject({ value: null, valueStatus: 'negative_infinity', pass: false });
    expect(statuses.expected.criteria.find((criterion) => criterion.id === 'randomEntryPercentile'))
      .toMatchObject({ value: null, valueStatus: 'nan', pass: false });
  });

  it('locks complete JSON-safe score-v1 structures and the stable sqrt/log10 cases', () => {
    for (const parityCase of fixture.scoreCases) {
      const expected = parityCase.expected;
      expect(Object.keys(expected).sort()).toEqual([
        'components',
        'config',
        'formulaVersion',
        'penalties',
        'score',
        'segment',
        'testedCombinations',
      ]);
      expect(expected.formulaVersion).toBe('score-v1');
      expect(expected.segment).toBe('validation');
      expect(expected.components.map((entry) => entry.id)).toEqual(COMPONENT_ORDER);
      expect(expected.penalties.map((entry) => entry.id)).toEqual(PENALTY_ORDER);
      expect(JSON.parse(JSON.stringify(expected))).toEqual(expected);
      expectFiniteJson(expected);
    }

    const baseline = findById(fixture.scoreCases, 'score-default-baseline').expected;
    expect(baseline.score).toBeCloseTo(2.65, 12);

    const sigma = findById(
      fixture.scoreCases,
      'score-partial-config-population-sigma-max-safe-n',
    ).expected;
    const consistency = sigma.components.find((entry) => entry.id === 'consistency')!;
    expect(consistency.raw).toBe(1); // [-1,-1,1,1] population sigma, sqrt(1)
    expect(consistency.evidence).toEqual({ monthCount: 4, monthlyStdDev: 1 });
    expect(sigma.testedCombinations.n).toBe(Number.MAX_SAFE_INTEGER);
    expect(sigma.penalties.find((entry) => entry.id === 'dataMining')?.normalized).toBe(1);

    const nonFiniteCase = findById(
      fixture.scoreCases,
      'score-nonfinite-statuses-negative-zero-insufficient',
    );
    const nonFinite = nonFiniteCase.expected;
    expect(nonFinite.components.find((entry) => entry.id === 'cagr'))
      .toMatchObject({ raw: 0, rawStatus: 'finite', normalized: 0 });
    expect(nonFinite.components.find((entry) => entry.id === 'sortino'))
      .toMatchObject({ raw: null, rawStatus: 'positive_infinity', normalized: 1 });
    expect(nonFinite.components.find((entry) => entry.id === 'calmar'))
      .toMatchObject({ raw: null, rawStatus: 'invalid', normalized: 0 });
    expect(nonFinite.components.find((entry) => entry.id === 'profitFactor'))
      .toMatchObject({ raw: null, rawStatus: 'invalid', normalized: 0 });
    expect(nonFinite.components.find((entry) => entry.id === 'consistency'))
      .toMatchObject({ raw: null, rawStatus: 'insufficient', normalized: 0 });
    expect(nonFinite.penalties.find((entry) => entry.id === 'turnover'))
      .toMatchObject({ raw: null, rawStatus: 'positive_infinity', normalized: 1 });
    expect(nonFiniteCase.input.config?.weights).toEqual({
      cagr: 'negative_zero',
      sortino: 'negative_zero',
      calmar: 'negative_zero',
      regime: 'negative_zero',
      profitFactor: 'negative_zero',
      consistency: 'negative_zero',
      complexity: 'negative_zero',
      turnover: 'negative_zero',
      dataMining: 'negative_zero',
    });
    expect(Object.values(nonFinite.config.weights)).toEqual(Array(9).fill(0));
    for (const resolvedWeight of Object.values(nonFinite.config.weights)) {
      expect(Object.is(resolvedWeight, -0)).toBe(false);
    }
    expect([...nonFinite.components, ...nonFinite.penalties].map((entry) => entry.weight))
      .toEqual(Array(9).fill(0));
    expect([...nonFinite.components, ...nonFinite.penalties].map((entry) => entry.contribution))
      .toEqual(Array(9).fill(0));
    expect(nonFinite.score).toBe(0);
    expect(Object.is(nonFinite.score, -0)).toBe(false);

    const extreme = findById(fixture.scoreCases, 'score-extreme-finite-months-and-clamps').expected;
    const extremeConsistency = extreme.components.find((entry) => entry.id === 'consistency')!;
    expect(extremeConsistency.rawStatus).toBe('finite');
    expect(extremeConsistency.raw).toBeGreaterThan(Number.MAX_VALUE / 2);
    expect(extremeConsistency.raw).toBe(Number(extremeConsistency.raw!.toPrecision(15)));
    expect(Number.isFinite(extreme.score)).toBe(true);
  });
});
