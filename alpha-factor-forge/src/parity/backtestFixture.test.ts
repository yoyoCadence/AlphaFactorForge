import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/backtest-v1.json';
import { sha256Hex } from '../core/hashing';
import backtestSource from '../core/backtest/index.ts?raw';
import metricsSource from '../core/metrics/index.ts?raw';
import sampleDataSource from '../services/sampleData.ts?raw';
import generatorSource from './backtestFixture.ts?raw';
import { canonicalizeFixtureSource } from './indicatorFixture';
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
    expect(fixture.tolerance.default).toEqual({ absolute: 1e-12, relative: 1e-10 });

    const infinite = fixture.cases.find((c) => c.id === 'rising-no-downside-infinite-ratios')!;
    expect(infinite.expected.metrics.sortino).toBe('positive_infinity');
    expect(infinite.expected.metrics.calmar).toBe('positive_infinity');
    expect(infinite.expected.metrics.profitFactor).toBe('positive_infinity');

    const sample = fixture.cases.find((c) => c.id === 'sample-daily-long-nextopen-risk')!;
    expect(Object.keys(sample.expected.metrics.monthlyReturns as Record<string, number>).length)
      .toBeGreaterThanOrEqual(4);
    expect(fixture.errorCases).toHaveLength(3);
  });
});
