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

  it('pins the authored edge cases by exact id and their warm-up semantics', () => {
    expect(fixture.cases.map((c) => c.id)).toEqual([
      'sample-seed-42-48-bars',
      'flat-30-bars',
      'single-bar',
      'period-one-12-bars',
      'periods-longer-than-series',
    ]);
    for (const c of fixture.cases) {
      const n = c.input.candles.length;
      for (const series of [c.expected.sma, c.expected.rsi, c.expected.macd.hist, c.expected.bbands.lower, c.expected.roc]) {
        expect(series, c.id).toHaveLength(n);
      }
    }
    const byId = Object.fromEntries(fixture.cases.map((c) => [c.id, c]));
    const flat = byId['flat-30-bars'].expected;
    expect(flat.stddev.filter((v) => v !== null)).toEqual(Array(21).fill(0));
    expect(flat.bbands.upper.slice(19)).toEqual(flat.bbands.middle.slice(19));
    expect(flat.rsi.slice(14)).toEqual(Array(16).fill(100));
    const single = byId['single-bar'].expected;
    expect(single.trueRange).toEqual([2]);
    expect([single.sma, single.ema, single.rsi, single.atr, single.roc]).toEqual(Array(5).fill([null]));
    const one = byId['period-one-12-bars'];
    expect(one.expected.sma).toEqual(one.input.candles.map((candle) => candle.close));
    expect(one.expected.stddev.slice(0)).toEqual(Array(12).fill(0));
    const longer = byId['periods-longer-than-series'].expected;
    for (const series of [longer.sma, longer.ema, longer.wma, longer.rsi, longer.macd.macd, longer.atr, longer.bbands.middle, longer.stddev, longer.highest, longer.lowest, longer.roc]) {
      expect(series).toEqual(Array(5).fill(null));
    }
  });
});
