import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/backtest-v1.json';
import { sha256Hex } from '../core/hashing';
import { runBacktest, type BacktestConfig, type Candle } from '../core/backtest';
import backtestSource from '../core/backtest/index.ts?raw';
import metricsSource from '../core/metrics/index.ts?raw';
import sampleDataSource from '../services/sampleData.ts?raw';
import generatorSource from './backtestFixture.ts?raw';
import { canonicalizeFixtureSource, FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import { buildBacktestParityFixture } from './backtestFixture';

async function hashSource(source: string): Promise<string> {
  return `sha256:${await sha256Hex(canonicalizeFixtureSource(source))}`;
}

describe('RS-CORE backtest parity fixture', () => {
  it('is exactly reproducible from the current TypeScript reference sources', async () => {
    const regenerated = buildBacktestParityFixture({
      generator: await hashSource(generatorSource),
      backtest: await hashSource(backtestSource),
      metrics: await hashSource(metricsSource),
      sampleData: await hashSource(sampleDataSource),
    });
    expect(regenerated).toEqual(fixture);
  });

  it('locks the tolerance policy and the METRIC-001 non-finite statuses', () => {
    expect(fixture.schemaVersion).toBe('rs-core-parity-fixture-v1');
    expect(fixture.fixtureVersion).toBe('backtest-parity-v1');
    expect(fixture.contracts.execution).toBe('backtest-execution-v1');
    expect(fixture.generator.sourceHashEncoding).toBe(FIXTURE_SOURCE_HASH_ENCODING);
    expect(fixture.tolerance.default).toEqual({ absolute: 1e-12, relative: 1e-10 });

    const infinite = fixture.cases.find((c) => c.id === 'rising-no-downside-infinite-ratios')!;
    expect(infinite.expected.metrics.sortino).toBe('positive_infinity');
    expect(infinite.expected.metrics.calmar).toBe('positive_infinity');
    expect(infinite.expected.metrics.profitFactor).toBe('positive_infinity');

    const sample = fixture.cases.find((c) => c.id === 'sample-daily-long-nextopen-risk')!;
    expect(Object.keys(sample.expected.metrics.monthlyReturns as Record<string, number>).length)
      .toBeGreaterThanOrEqual(4);
  });

  it('locks the exact case inventory including the empty boundaries', () => {
    expect(fixture.cases.map((c) => c.id)).toEqual([
      'long-close-two-roundtrips',
      'long-close-costs-partial-sizing',
      'short-close-win-and-loss',
      'both-close-reversals',
      'long-nextopen-pending-and-final-bar',
      'both-nextopen-reversal',
      'long-stoploss-gap-through',
      'long-takeprofit-then-gap-up',
      'short-stoploss-and-takeprofit',
      'stoploss-wins-ambiguous-bar',
      'full-sizing-budgets-entry-fee',
      'eod-settles-open-position',
      'from-to-subrange',
      'single-bar-from-equals-to',
      'no-trades-zero-metrics',
      'empty-candles-boundary',
      'inverted-range-empty-evaluation',
      'rising-no-downside-infinite-ratios',
      'sample-daily-long-nextopen-risk',
      'sample-daily-both-close',
    ]);
    const empty = fixture.cases.find((c) => c.id === 'empty-candles-boundary')!;
    expect(empty.expected.trades).toHaveLength(0);
    expect(empty.expected.equity).toHaveLength(0);
    expect(empty.expected.metrics.monthlyReturns).toEqual({});
  });

  it('the TS reference engine itself rejects every committed error case', () => {
    expect(fixture.errorCases).toHaveLength(3);
    for (const errorCase of fixture.errorCases) {
      const candles: Candle[] = errorCase.input.candles.map((candle) => ({
        t: candle.timestamp,
        o: candle.open,
        h: candle.high,
        l: candle.low,
        c: candle.close,
        v: candle.volume,
      }));
      let thrown: unknown = null;
      try {
        runBacktest(candles, errorCase.input.signals, errorCase.input.config as BacktestConfig);
      } catch (error) {
        thrown = error;
      }
      expect(thrown, errorCase.id).toBeInstanceOf(RangeError);
      expect((thrown as RangeError).message, errorCase.id).toContain(
        errorCase.expectedErrorIncludes,
      );
    }
  });
});
