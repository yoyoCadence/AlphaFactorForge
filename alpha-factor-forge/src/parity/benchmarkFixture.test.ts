import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/benchmark-v1.json';
import { sha256Hex } from '../core/hashing';
import { mulberry32 } from '../services/sampleData';
import benchmarksSource from '../services/benchmarks.ts?raw';
import randomEntrySource from '../services/randomEntry.ts?raw';
import backtestRunnerSource from '../services/backtestRunner.ts?raw';
import strategySignalsSource from '../services/strategySignals.ts?raw';
import strategySource from '../services/strategy.ts?raw';
import indicatorsSource from '../core/indicators/index.ts?raw';
import backtestSource from '../core/backtest/index.ts?raw';
import metricsSource from '../core/metrics/index.ts?raw';
import sampleDataSource from '../services/sampleData.ts?raw';
import nonFiniteSource from '../services/nonFinite.ts?raw';
import generatorSource from './benchmarkFixture.ts?raw';
import parityEncodeSource from './parityEncode.ts?raw';
import { canonicalizeFixtureSource, FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import { buildBenchmarkParityFixture } from './benchmarkFixture';

async function hashSource(source: string): Promise<string> {
  return `sha256:${await sha256Hex(canonicalizeFixtureSource(source))}`;
}

describe('RS-CORE benchmark parity fixture', () => {
  it('is exactly reproducible from the current TypeScript reference sources', async () => {
    const regenerated = buildBenchmarkParityFixture({
      generator: await hashSource(generatorSource),
      parityEncode: await hashSource(parityEncodeSource),
      nonFinite: await hashSource(nonFiniteSource),
      benchmarks: await hashSource(benchmarksSource),
      randomEntry: await hashSource(randomEntrySource),
      backtestRunner: await hashSource(backtestRunnerSource),
      strategySignals: await hashSource(strategySignalsSource),
      strategy: await hashSource(strategySource),
      indicators: await hashSource(indicatorsSource),
      backtest: await hashSource(backtestSource),
      metrics: await hashSource(metricsSource),
      sampleData: await hashSource(sampleDataSource),
    });
    expect(regenerated).toEqual(fixture);
  });

  it('locks the exact case inventories and the PRNG raw-u32 contract', () => {
    expect(fixture.fixtureVersion).toBe('benchmark-parity-v1');
    expect(fixture.generator.sourceHashEncoding).toBe(FIXTURE_SOURCE_HASH_ENCODING);
    expect(fixture.contracts.candle).toBe('ohlcv-candle-v1');
    expect(fixture.contracts.execution).toBe('backtest-execution-v1');
    expect(fixture.contracts.prng).toBe('mulberry32-v1');
    expect(fixture.contracts.metrics).toBe('metrics-v1');
    expect(fixture.contracts.signals).toBe('params-signals-v1');
    expect(fixture.contracts.benchmarks).toBe('benchmark-suite-v1');
    expect(fixture.contracts.randomEntry).toBe('random-entry-v1');
    expect(fixture.tolerance.default).toEqual({ absolute: 1e-12, relative: 1e-10 });

    expect(fixture.prngCases.map((c) => c.id)).toEqual([
      'prng-seed-42',
      'prng-seed-7',
      'prng-seed-u32-max',
      'prng-seed-123',
      'prng-seed-truncated-2pow32-plus-123',
    ]);
    expect(fixture.suiteCases.map((c) => c.id)).toEqual([
      'suite-small-no-cost',
      'suite-sample-daily-costs',
      'suite-sma-cross-trades',
      'suite-subrange-prototype-key-interval',
    ]);
    expect(fixture.plannerCases.map((c) => c.id)).toEqual([
      'planner-basic',
      'planner-clip-and-drop',
    ]);
    expect(fixture.randomEntryCases.map((c) => c.id)).toEqual([
      'random-entry-fake-candidate',
      'random-entry-real-candidate',
      'random-entry-zero-bar-clamp-subrange',
      'random-entry-flat-tie-default-runs',
      'random-entry-min-seed-min-runs',
      'random-entry-max-seed-max-runs',
    ]);
    expect(fixture.benchmarkErrorCases.map((c) => c.id)).toEqual(['benchmarks-empty-candles']);
    expect(fixture.randomEntryErrorCases.map((c) => c.id)).toEqual([
      'random-entry-zero-runs',
      'random-entry-runs-above-cap',
      'random-entry-negative-seed',
      'random-entry-seed-above-safe-range',
      'random-entry-empty-candles',
      'random-entry-inverted-segment',
      'random-entry-no-candidate-trades',
    ]);
  });

  it('the committed raw u32 sequences match a fresh mulberry32 stream exactly', () => {
    for (const prngCase of fixture.prngCases) {
      const next = mulberry32(prngCase.input.seed);
      const expected = Array.from({ length: prngCase.input.count }, () => next() * 4_294_967_296);
      expect(prngCase.expected.rawU32, prngCase.id).toEqual(expected);
      for (const value of prngCase.expected.rawU32) {
        expect(Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff).toBe(true);
      }
    }
    const plain = fixture.prngCases.find((c) => c.id === 'prng-seed-123')!;
    const truncated = fixture.prngCases.find(
      (c) => c.id === 'prng-seed-truncated-2pow32-plus-123',
    )!;
    expect(truncated.expected.rawU32).toEqual(plain.expected.rawU32);
  });
});
