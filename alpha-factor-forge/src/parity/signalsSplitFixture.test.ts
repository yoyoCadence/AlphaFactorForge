import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/signals-split-v1.json';
import { sha256Hex } from '../core/hashing';
import { planValidationSplit } from '../core/validation/split';
import { deriveEmbargoBars } from '../services/embargo';
import { buildParamsSignals } from '../services/strategySignals';
import { defaultStrategy, type ParamsStrategy } from '../services/strategy';
import strategySignalsSource from '../services/strategySignals.ts?raw';
import splitSource from '../core/validation/split.ts?raw';
import embargoSource from '../services/embargo.ts?raw';
import strategySource from '../services/strategy.ts?raw';
import indicatorsSource from '../core/indicators/index.ts?raw';
import sampleDataSource from '../services/sampleData.ts?raw';
import generatorSource from './signalsSplitFixture.ts?raw';
import { canonicalizeFixtureSource, FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import { buildSignalsSplitParityFixture } from './signalsSplitFixture';

async function hashSource(source: string): Promise<string> {
  return `sha256:${await sha256Hex(canonicalizeFixtureSource(source))}`;
}

const asStrategy = (config: (typeof fixture)['signalCases'][number]['input']['config']): ParamsStrategy => ({
  ...defaultStrategy(),
  ...config,
  mode: 'params',
} as ParamsStrategy);

describe('RS-CORE signals/split parity fixture', () => {
  it('is exactly reproducible from the current TypeScript reference sources', async () => {
    const regenerated = buildSignalsSplitParityFixture({
      generator: await hashSource(generatorSource),
      strategySignals: await hashSource(strategySignalsSource),
      split: await hashSource(splitSource),
      embargo: await hashSource(embargoSource),
      strategy: await hashSource(strategySource),
      indicators: await hashSource(indicatorsSource),
      sampleData: await hashSource(sampleDataSource),
    });
    expect(regenerated).toEqual(fixture);
  });

  it('locks the exact case inventories', () => {
    expect(fixture.fixtureVersion).toBe('signals-split-parity-v1');
    expect(fixture.generator.sourceHashEncoding).toBe(FIXTURE_SOURCE_HASH_ENCODING);
    expect(fixture.signalCases.map((c) => c.id)).toEqual([
      'hand-ma-cross-exact-index',
      'sample-ma-cross',
      'sample-ema-cross',
      'sample-price-vs-slow',
      'sample-rsi-thresholds',
      'sample-macd-cross',
      'sample-bollinger-touch',
    ]);
    expect(fixture.splitCases.map((c) => c.id)).toEqual([
      'split-minimal-five',
      'split-residue-0',
      'split-residue-1',
      'split-residue-2',
      'split-residue-3',
      'split-residue-4',
      'split-with-embargo',
      'split-embargo-residue',
      'split-max-safe-integer',
    ]);
    expect(fixture.embargoCases.map((c) => c.id)).toEqual([
      'embargo-default-ma-cross',
      'embargo-macd-exit',
      'embargo-rsi-pair',
      'embargo-bollinger-pair',
      'embargo-price-vs-slow',
      'embargo-with-allowance',
      'embargo-unused-period-ignored',
    ]);
    expect(fixture.signalErrorCases.map((c) => c.id)).toEqual(['signals-stoch-unsupported']);
    expect(fixture.splitErrorCases.map((c) => c.id)).toEqual([
      'split-too-few-usable-bars',
      'split-embargo-consumes-bars',
      'split-negative-total',
      'split-negative-embargo',
    ]);
    expect(fixture.embargoErrorCases.map((c) => c.id)).toEqual([
      'embargo-stoch-unsupported',
      'embargo-invalid-used-period',
      'embargo-negative-allowance',
    ]);
  });

  it('the TS reference itself rejects every committed error case', () => {
    for (const errorCase of fixture.signalErrorCases) {
      const candles = errorCase.input.candles.map((candle) => ({
        t: candle.timestamp,
        o: candle.open,
        h: candle.high,
        l: candle.low,
        c: candle.close,
        v: candle.volume,
      }));
      expect(
        () => buildParamsSignals(candles, asStrategy(errorCase.input.config)),
        errorCase.id,
      ).toThrow(new RegExp(errorCase.expectedErrorIncludes));
    }
    for (const errorCase of fixture.splitErrorCases) {
      expect(
        () => planValidationSplit(errorCase.input.totalBars, errorCase.input.embargoBars),
        errorCase.id,
      ).toThrow(new RegExp(errorCase.expectedErrorIncludes));
    }
    for (const errorCase of fixture.embargoErrorCases) {
      expect(
        () =>
          deriveEmbargoBars(
            asStrategy(errorCase.input.config),
            errorCase.input.holdingAllowanceBars,
          ),
        errorCase.id,
      ).toThrow(new RegExp(errorCase.expectedErrorIncludes));
    }
  });
});
