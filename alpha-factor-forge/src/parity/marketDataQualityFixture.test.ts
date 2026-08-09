import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/market-data-quality-v1.json';
import { sha256Hex } from '../core/hashing';
import {
  MARKET_DATA_QUALITY_VERSION,
  MARKET_DATA_RULE_IDS,
  MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE,
  MIN_MARKET_TIMESTAMP_MS,
  firstMarketDataIssue,
  type MarketDataCandle,
} from '../core/market-data/quality';
import { canonicalizeFixtureSource, FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import generatorSource from './marketDataQualityFixture.ts?raw';
import {
  buildMarketDataQualityParityFixture,
  decodeFixtureNumber,
  type FixtureCandle,
} from './marketDataQualityFixture';

async function hashSource(source: string): Promise<string> {
  return `sha256:${await sha256Hex(canonicalizeFixtureSource(source))}`;
}

function decodeCandle(candle: FixtureCandle): MarketDataCandle {
  return {
    timestamp: decodeFixtureNumber(candle.timestamp),
    open: decodeFixtureNumber(candle.open),
    high: decodeFixtureNumber(candle.high),
    low: decodeFixtureNumber(candle.low),
    close: decodeFixtureNumber(candle.close),
    volume: decodeFixtureNumber(candle.volume),
  };
}

const cases = fixture.cases as unknown as {
  id: string;
  candles: FixtureCandle[];
  expected: { accepted: boolean; rule?: string; index?: number };
}[];

describe('DATA-QUALITY-001 market-data admission parity fixture', () => {
  it('is exactly reproducible from the canonical current generator source', async () => {
    const regenerated = buildMarketDataQualityParityFixture({
      generator: await hashSource(generatorSource),
    });
    expect(regenerated).toEqual(fixture);
  });

  it('locks the envelope, the adjudicated constants, and the rule inventory', () => {
    expect(fixture.schemaVersion).toBe('rs-core-parity-fixture-v1');
    expect(fixture.fixtureVersion).toBe('market-data-quality-parity-v1');
    expect(fixture.contracts).toEqual({
      marketDataQuality: 'market-data-quality-v1',
      candle: 'ohlcv-candle-v1',
    });
    expect(fixture.generator.sourceHashEncoding).toBe(FIXTURE_SOURCE_HASH_ENCODING);
    expect(fixture.numericEncoding).toEqual({
      specialInputNumbers: 'explicit-numeric-status-v1',
      expectedNumericPolicy: 'exact-v1',
    });
    expect(fixture.constants).toEqual({
      minTimestampMs: 946_684_800_000,
      maxTimestampMsExclusive: 4_102_444_800_000,
    });
    expect(fixture.ruleIds).toEqual([
      'timestamp_not_integer',
      'timestamp_out_of_range',
      'timestamp_not_representable',
      'price_not_positive',
      'volume_negative',
      'high_below_low',
      'ohlc_out_of_range',
    ]);
    expect(fixture.reachableRuleIds).toEqual([
      'timestamp_not_integer',
      'timestamp_out_of_range',
      'price_not_positive',
      'volume_negative',
      'high_below_low',
      'ohlc_out_of_range',
    ]);
    expect(fixture.unreachableRuleIds).toEqual(['timestamp_not_representable']);
    expect(cases.map((entry) => entry.id)).toEqual([
      'valid-three-candle-dataset',
      'valid-single-candle-zero-volume',
      'valid-flat-bar',
      'boundary-min-timestamp-accepted',
      'boundary-max-exclusive-minus-one-accepted',
      'boundary-min-minus-one-rejected',
      'boundary-max-exclusive-rejected',
      'audit-epoch-seconds-timestamp-rejected',
      'runner-regression-js-safe-timestamp-rejected',
      'timestamp-fractional-rejected',
      'timestamp-unsafe-magnitude-rejected',
      'timestamp-nan-rejected',
      'timestamp-infinite-rejected',
      'price-zero-low-rejected',
      'price-negative-open-rejected',
      'price-nan-high-rejected',
      'volume-negative-rejected',
      'volume-nan-rejected',
      'high-below-low-rejected',
      'ohlc-open-above-high-rejected',
      'ohlc-close-below-low-rejected',
      'audit-inverted-ohlc-negative-volume-reports-volume-first',
      'first-failing-candle-wins-over-later-defect',
    ]);
  });

  it('shares one set of constants and rule ids with the TypeScript validator', () => {
    expect(MIN_MARKET_TIMESTAMP_MS).toBe(fixture.constants.minTimestampMs);
    expect(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE).toBe(fixture.constants.maxTimestampMsExclusive);
    expect([...MARKET_DATA_RULE_IDS]).toEqual(fixture.ruleIds);
    expect(MARKET_DATA_QUALITY_VERSION).toBe(fixture.contracts.marketDataQuality);
  });

  it('classifies every fixture row exactly as the fixture specifies', () => {
    for (const entry of cases) {
      const issue = firstMarketDataIssue(entry.candles.map(decodeCandle));
      if (entry.expected.accepted) {
        expect(issue, `${entry.id} must be admitted`).toBeNull();
        continue;
      }
      expect(issue, `${entry.id} must be rejected`).not.toBeNull();
      expect({ rule: issue!.rule, index: issue!.index }, entry.id).toEqual({
        rule: entry.expected.rule,
        index: entry.expected.index,
      });
    }
  });

  it('covers every reachable rule id with at least one rejection row', () => {
    const observed = new Set(
      cases
        .filter((entry) => !entry.expected.accepted)
        .map((entry) => firstMarketDataIssue(entry.candles.map(decodeCandle))!.rule),
    );
    expect([...fixture.reachableRuleIds].every((rule) => observed.has(rule as never))).toBe(true);
    expect(observed.has('timestamp_not_representable')).toBe(false);
  });
});
