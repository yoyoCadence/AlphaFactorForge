import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/indicators-v1.json';
import { sha256Hex } from '../core/hashing';
import indicatorSource from '../core/indicators/index.ts?raw';
import sampleDataSource from '../services/sampleData.ts?raw';
import generatorSource from './indicatorFixture.ts?raw';
import {
  buildIndicatorParityFixture,
  canonicalizeFixtureSource,
  FIXTURE_SOURCE_HASH_ENCODING,
} from './indicatorFixture';

async function hashSource(source: string): Promise<string> {
  return `sha256:${await sha256Hex(canonicalizeFixtureSource(source))}`;
}

describe('RS-CORE indicator parity fixture', () => {
  it('canonicalizes checkout line endings before source hashing', () => {
    expect(canonicalizeFixtureSource('first\r\nsecond\rthird\n')).toBe(
      'first\nsecond\nthird\n',
    );
  });

  it('is exactly reproducible from the current TypeScript reference sources', async () => {
    const regenerated = buildIndicatorParityFixture({
      generator: await hashSource(generatorSource),
      indicators: await hashSource(indicatorSource),
      sampleData: await hashSource(sampleDataSource),
    });
    expect(regenerated).toEqual(fixture);
  });

  it('declares the reviewable tolerance and exact warm-up contract', () => {
    expect(fixture.generator.sourceHashEncoding).toBe(FIXTURE_SOURCE_HASH_ENCODING);
    expect(fixture.tolerance.default).toEqual({ absolute: 1e-12, relative: 1e-10 });
    const parityCase = fixture.cases[0];
    const length = parityCase.input.candles.length;
    expect(length).toBe(48);
    expect(parityCase.expected.sma).toHaveLength(length);
    expect(parityCase.expected.macd.signal).toHaveLength(length);
    expect(parityCase.expected.bbands.upper).toHaveLength(length);
    expect(parityCase.expected.sma.slice(0, 6)).toEqual(Array(6).fill(null));
    expect(parityCase.expected.rsi.slice(0, 14)).toEqual(Array(14).fill(null));
    expect(parityCase.expected.macd.signal.slice(0, 33)).toEqual(Array(33).fill(null));
  });
});
