import { describe, expect, it } from 'vitest';
import {
  MARKET_DATA_QUALITY_VERSION,
  MARKET_DATA_RULE_IDS,
  MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE,
  MIN_MARKET_TIMESTAMP_MS,
  assertMarketDataQuality,
  describeMarketDataIssue,
  firstMarketDataIssue,
  inspectMarketDataCandle,
  isRepresentableTimestamp,
  type MarketDataCandle,
} from './quality';

const validCandle = (overrides: Partial<MarketDataCandle> = {}): MarketDataCandle => ({
  timestamp: 1_721_001_600_000,
  open: 100,
  high: 103,
  low: 99,
  close: 102,
  volume: 10,
  ...overrides,
});

describe('market-data admission contract', () => {
  it('publishes the adjudicated constants and the ordered rule inventory', () => {
    expect(MARKET_DATA_QUALITY_VERSION).toBe('market-data-quality-v1');
    // 2000-01-01T00:00:00Z inclusive, 2100-01-01T00:00:00Z exclusive.
    expect(MIN_MARKET_TIMESTAMP_MS).toBe(Date.UTC(2000, 0, 1));
    expect(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE).toBe(Date.UTC(2100, 0, 1));
    expect(MIN_MARKET_TIMESTAMP_MS).toBe(946_684_800_000);
    expect(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE).toBe(4_102_444_800_000);
    expect([...MARKET_DATA_RULE_IDS]).toEqual([
      'timestamp_not_integer',
      'timestamp_out_of_range',
      'timestamp_not_representable',
      'price_not_positive',
      'volume_negative',
      'high_below_low',
      'ohlc_out_of_range',
    ]);
  });

  /**
   * Rule 3 has no fixture rejection row because it is unreachable by
   * construction, so the predicate is asserted directly instead. TypeScript and
   * Rust deliberately disagree OUTSIDE the product range: JavaScript's Date
   * limit is wider than chrono's UTC millisecond range, which is exactly why
   * representability cannot be a shared parity row.
   */
  it('asserts representability independently of the range rule', () => {
    expect(isRepresentableTimestamp(MIN_MARKET_TIMESTAMP_MS)).toBe(true);
    expect(isRepresentableTimestamp(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE)).toBe(true);
    // chrono cannot represent this; JavaScript can.
    expect(isRepresentableTimestamp(8_500_000_000_000_000)).toBe(true);
    // Beyond Date's own +-8.64e15 ms limit.
    expect(isRepresentableTimestamp(8_640_000_000_000_001)).toBe(false);
    expect(isRepresentableTimestamp(NaN)).toBe(false);
    expect(isRepresentableTimestamp(Infinity)).toBe(false);
  });

  it('never reaches rule 3 for a value that survived the range rule', () => {
    for (
      let timestamp = MIN_MARKET_TIMESTAMP_MS;
      timestamp < MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE;
      timestamp += 823_543_211
    ) {
      expect(isRepresentableTimestamp(timestamp)).toBe(true);
    }
    expect(isRepresentableTimestamp(MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE - 1)).toBe(true);
  });

  it('classifies non-finite fields by the first failing rule', () => {
    expect(inspectMarketDataCandle(0, validCandle({ timestamp: NaN }))?.rule)
      .toBe('timestamp_not_integer');
    expect(inspectMarketDataCandle(0, validCandle({ high: NaN }))?.rule)
      .toBe('price_not_positive');
    expect(inspectMarketDataCandle(0, validCandle({ low: -Infinity }))?.rule)
      .toBe('price_not_positive');
    expect(inspectMarketDataCandle(0, validCandle({ volume: NaN }))?.rule)
      .toBe('volume_negative');
    expect(inspectMarketDataCandle(0, validCandle({ volume: Infinity }))?.rule)
      .toBe('volume_negative');
    // Volume is non-negative, not strictly positive.
    expect(inspectMarketDataCandle(0, validCandle({ volume: 0 }))).toBeNull();
    expect(inspectMarketDataCandle(0, validCandle({ volume: -0 }))).toBeNull();
  });

  it('reports the failing index and stops at the first bad candle', () => {
    const issue = firstMarketDataIssue([
      validCandle(),
      validCandle({ timestamp: 1_721_005_200_000 }),
      validCandle({ timestamp: 1_721_008_800_000, volume: -1 }),
      validCandle({ timestamp: 1_721_012_400_000, high: 1, low: 2 }),
    ]);
    expect(issue).toEqual({ index: 2, timestamp: 1_721_008_800_000, rule: 'volume_negative' });
  });

  it('admits an empty slice, leaving emptiness to the identity layer', () => {
    // normalizeDatasetCandles already fails closed on an empty import; this
    // module must not duplicate or contradict that rule.
    expect(firstMarketDataIssue([])).toBeNull();
  });

  it('throws a stable technical message naming index, timestamp, and rule', () => {
    expect(() => assertMarketDataQuality([validCandle({ timestamp: 1_704_067_200 })]))
      .toThrow('market data rejected at candle 0 (timestamp 1704067200): timestamp_out_of_range');
    expect(describeMarketDataIssue({ index: 7, timestamp: 12, rule: 'high_below_low' }))
      .toBe('market data rejected at candle 7 (timestamp 12): high_below_low');
    expect(() => assertMarketDataQuality([validCandle()])).not.toThrow();
  });
});
