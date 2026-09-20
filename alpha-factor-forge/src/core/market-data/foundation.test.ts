import { describe, expect, it } from 'vitest';
import {
  MARKET_FOUNDATION_VERSION,
  MAX_EXPECTED_BARS,
  auditCoverage,
  detectTimeUnit,
  expectedBarStarts,
  formatInstrumentId,
  intervalMs,
  isoWeekdayOfUtcMs,
  parseInstrumentId,
  seriesConflicts,
  sourceOrigin,
  sourcesShareOrigin,
  utcDateToMs,
  validateCalendar,
  type SeriesIdentity,
  type SessionCalendar,
} from './foundation';

const HOUR = 3_600_000;
const DAY = 86_400_000;
/** 2024-07-15T00:00:00Z, a Monday. */
const T0 = 1_721_001_600_000;

const crypto247: SessionCalendar = {
  calendarId: 'crypto-24x7-v1',
  kind: 'continuous',
  timezone: 'UTC',
  tradingWeekdays: [],
  holidays: [],
  earlyCloses: [],
};

const weekdays: SessionCalendar = {
  calendarId: 'test-weekdays-v1',
  kind: 'trading-days',
  timezone: 'America/New_York',
  tradingWeekdays: [1, 2, 3, 4, 5],
  holidays: ['2024-07-18'],
  earlyCloses: [],
};

describe('market-foundation intervals', () => {
  it('is strict: an unknown interval has no cadence instead of a daily fallback', () => {
    expect(intervalMs('1h')).toBe(HOUR);
    expect(intervalMs('1d')).toBe(DAY);
    expect(intervalMs('2h')).toBeNull();
    expect(intervalMs('')).toBeNull();
    // Inherited Object prototype keys must not answer as intervals.
    expect(intervalMs('toString')).toBeNull();
  });
});

describe('instrument ids', () => {
  it('parses market, venue, and symbol and keeps the venue spelling', () => {
    expect(parseInstrumentId('crypto:binance:BTCUSDT')).toEqual({
      id: { market: 'crypto', venue: 'binance', symbol: 'BTCUSDT' },
      rule: null,
    });
    expect(parseInstrumentId('tw-etf:twse:0050').id?.symbol).toBe('0050');
    expect(parseInstrumentId('us-etf:nyse-arca:SPY').id?.venue).toBe('nyse-arca');
    expect(formatInstrumentId({ market: 'crypto', venue: 'binance', symbol: 'BTCUSDT' })).toBe(
      'crypto:binance:BTCUSDT',
    );
  });

  it('reports the first failing rule', () => {
    expect(parseInstrumentId('crypto:binance').rule).toBe('instrument_id_shape');
    expect(parseInstrumentId('crypto::BTCUSDT').rule).toBe('instrument_id_shape');
    expect(parseInstrumentId('crypto:binance:BTC:USDT').rule).toBe('instrument_id_shape');
    expect(parseInstrumentId('fx:oanda:EURUSD').rule).toBe('unknown_market');
    expect(parseInstrumentId('crypto:Binance:BTCUSDT').rule).toBe('venue_not_normalized');
    expect(parseInstrumentId('crypto:binance:BTC USDT').rule).toBe('symbol_not_supported');
  });
});

describe('series combination', () => {
  const binance: SeriesIdentity = {
    instrumentId: 'crypto:binance:BTCUSDT',
    quote: 'USDT',
    interval: '1h',
    source: 'binance-archive',
    role: 'primary',
    priceBasis: 'raw',
  };

  it('allows two identical primary series from the same source', () => {
    expect(seriesConflicts(binance, { ...binance })).toEqual([]);
  });

  it('never splices a different venue, quote, interval, basis, or source together', () => {
    expect(
      seriesConflicts(binance, {
        ...binance,
        instrumentId: 'crypto:coinbase:BTC-USD',
        quote: 'USD',
        source: 'coinbase-rest',
      }),
    ).toEqual(['quote_mismatch', 'source_mismatch', 'symbol_mismatch', 'venue_mismatch']);
    expect(seriesConflicts(binance, { ...binance, interval: '1d' })).toEqual(['interval_mismatch']);
    expect(seriesConflicts(binance, { ...binance, priceBasis: 'adjusted-split' })).toEqual([
      'price_basis_mismatch',
    ]);
  });

  it('keeps a comparison source as evidence rather than a component', () => {
    expect(seriesConflicts(binance, { ...binance, role: 'comparison' })).toEqual([
      'comparison_role_not_combinable',
    ]);
  });

  it('reports a source mismatch even between two endpoints of one publisher', () => {
    // The fact is reported; whether it disqualifies a composition is the
    // caller's decision, and `sourcesShareOrigin` is how it decides.
    expect(seriesConflicts(binance, { ...binance, source: 'binance-rest' })).toEqual([
      'source_mismatch',
    ]);
    expect(sourcesShareOrigin('binance-archive', 'binance-rest')).toBe(true);
    expect(sourcesShareOrigin('binance-archive', 'coinbase-rest')).toBe(false);
    expect(sourceOrigin('binance-archive')).toBe('binance');
    expect(sourceOrigin('tiingo')).toBe('tiingo');
    expect(sourcesShareOrigin('', '')).toBe(false);
  });

  it('refuses an unparsable id without guessing what was meant', () => {
    expect(seriesConflicts(binance, { ...binance, instrumentId: 'binance-BTCUSDT' })).toEqual([
      'invalid_instrument_id',
    ]);
  });
});

describe('time units', () => {
  it('names the unit of a consistent batch', () => {
    expect(detectTimeUnit([T0, T0 + HOUR])).toEqual({
      unit: 'milliseconds',
      issue: null,
      index: null,
    });
    expect(detectTimeUnit([T0 / 1000]).unit).toBe('seconds');
    expect(detectTimeUnit([T0 * 1000]).unit).toBe('microseconds');
    expect(detectTimeUnit([T0 * 1_000_000]).unit).toBe('nanoseconds');
  });

  it('reports the index where the Binance archive changes unit mid-batch', () => {
    expect(detectTimeUnit([T0, T0 + HOUR, (T0 + 2 * HOUR) * 1000])).toEqual({
      unit: null,
      issue: 'time_unit_mixed',
      index: 2,
    });
  });

  it('refuses to classify what is in no known band', () => {
    expect(detectTimeUnit([])).toEqual({ unit: null, issue: 'no_timestamps', index: null });
    expect(detectTimeUnit([0])).toEqual({ unit: null, issue: 'time_unit_unknown', index: 0 });
    expect(detectTimeUnit([T0, Number.NaN]).issue).toBe('time_unit_unknown');
  });
});

describe('calendars', () => {
  it('accepts the two shapes and rejects everything else by rule', () => {
    expect(validateCalendar(crypto247)).toBeNull();
    expect(validateCalendar(weekdays)).toBeNull();
    expect(validateCalendar({ ...crypto247, calendarId: 'crypto-24x7' })).toBe('calendar_id_shape');
    expect(validateCalendar({ ...crypto247, timezone: ' ' })).toBe('empty_timezone');
    expect(validateCalendar({ ...crypto247, holidays: ['2024-07-18'] })).toBe(
      'continuous_carries_trading_days',
    );
    expect(validateCalendar({ ...weekdays, tradingWeekdays: [] })).toBe(
      'trading_days_without_weekdays',
    );
    expect(validateCalendar({ ...weekdays, tradingWeekdays: [0, 1] })).toBe('weekday_out_of_range');
    expect(validateCalendar({ ...weekdays, tradingWeekdays: [2, 1] })).toBe('weekdays_not_sorted');
    expect(validateCalendar({ ...weekdays, holidays: ['2024-02-30'] })).toBe('invalid_date');
    expect(validateCalendar({ ...weekdays, holidays: ['2024-07-19', '2024-07-18'] })).toBe(
      'dates_not_sorted',
    );
  });

  it('maps dates and weekdays without a timezone database', () => {
    expect(utcDateToMs('2024-07-15')).toBe(T0);
    expect(utcDateToMs('2024-13-01')).toBeNull();
    expect(isoWeekdayOfUtcMs(T0)).toBe(1);
    expect(isoWeekdayOfUtcMs(T0 + 5 * DAY)).toBe(6);
    expect(isoWeekdayOfUtcMs(0)).toBe(4);
  });
});

describe('expected bar starts', () => {
  it('walks a continuous calendar on the cadence grid', () => {
    expect(
      expectedBarStarts({
        calendar: crypto247,
        interval: '1h',
        fromMs: T0,
        toMsExclusive: T0 + 3 * HOUR,
      }),
    ).toEqual({ timestamps: [T0, T0 + HOUR, T0 + 2 * HOUR], issue: null });
  });

  it('excludes weekends, holidays, halts, and time outside the listing', () => {
    const result = expectedBarStarts({
      calendar: weekdays,
      interval: '1d',
      fromMs: T0,
      toMsExclusive: T0 + 7 * DAY,
      suspensions: [{ fromMs: T0 + DAY, toMsExclusive: T0 + 2 * DAY }],
    });
    // Mon 15th expected, Tue 16th halted, Thu 18th a holiday, Sat/Sun absent.
    expect(result.timestamps).toEqual([T0, T0 + 2 * DAY, T0 + 4 * DAY]);

    expect(
      expectedBarStarts({
        calendar: weekdays,
        interval: '1d',
        fromMs: T0,
        toMsExclusive: T0 + 7 * DAY,
        listedFromMs: T0 + 2 * DAY,
        delistedAtMs: T0 + 4 * DAY,
      }).timestamps,
    ).toEqual([T0 + 2 * DAY]);
  });

  it('refuses rather than guessing', () => {
    expect(expectedBarStarts({ calendar: crypto247, interval: '2h', fromMs: T0, toMsExclusive: T0 + DAY }).issue).toBe(
      'unknown_interval',
    );
    expect(
      expectedBarStarts({ calendar: weekdays, interval: '1h', fromMs: T0, toMsExclusive: T0 + DAY })
        .issue,
    ).toBe('unsupported_interval_for_calendar');
    expect(
      expectedBarStarts({ calendar: crypto247, interval: '1h', fromMs: T0, toMsExclusive: T0 })
        .issue,
    ).toBe('invalid_range');
    expect(
      expectedBarStarts({
        calendar: crypto247,
        interval: '1h',
        fromMs: T0,
        toMsExclusive: T0 + DAY,
        suspensions: [{ fromMs: T0 + DAY, toMsExclusive: T0 }],
      }).issue,
    ).toBe('invalid_suspension');
    expect(
      expectedBarStarts({
        calendar: crypto247,
        interval: '1m',
        fromMs: T0,
        toMsExclusive: T0 + (MAX_EXPECTED_BARS + 1) * 60_000,
      }),
    ).toEqual({ timestamps: [], issue: 'range_too_large' });
  });

  it('returns nothing, with no issue, when the listing does not reach the range', () => {
    expect(
      expectedBarStarts({
        calendar: crypto247,
        interval: '1h',
        fromMs: T0,
        toMsExclusive: T0 + HOUR,
        listedFromMs: T0 + 10 * DAY,
      }),
    ).toEqual({ timestamps: [], issue: null });
  });
});

describe('coverage audit', () => {
  const expected = [T0, T0 + HOUR, T0 + 2 * HOUR, T0 + 3 * HOUR];
  const asOfMs = T0 + 4 * HOUR;

  it('reports a clean series as complete', () => {
    const report = auditCoverage({ interval: '1h', expected, observed: expected, asOfMs });
    expect(report).toEqual({
      version: MARKET_FOUNDATION_VERSION,
      events: [],
      blocking: false,
      expectedCount: 4,
      notDueCount: 0,
      observedCount: 4,
      matchedCount: 4,
      issue: null,
    });
  });

  it('merges contiguous missing bars into one actionable range', () => {
    const report = auditCoverage({
      interval: '1h',
      expected,
      observed: [T0, T0 + 3 * HOUR],
      asOfMs,
    });
    expect(report.events).toEqual([
      {
        code: 'missing_bar',
        severity: 'blocking',
        rangeStart: T0 + HOUR,
        rangeEnd: T0 + 2 * HOUR,
        count: 2,
        action: 'refetch_range',
      },
    ]);
    expect(report.blocking).toBe(true);
    expect(report.matchedCount).toBe(2);
  });

  it('separates duplicates, misalignment, unexpected bars, and bad ordering', () => {
    const report = auditCoverage({
      interval: '1h',
      expected,
      observed: [
        T0 + HOUR,
        T0,
        T0,
        T0 + 2 * HOUR,
        T0 + 3 * HOUR + 1,
        T0 + 4 * HOUR,
        T0 + 3 * HOUR,
      ],
      asOfMs,
    });
    // Events are ordered by (rangeStart, declared code order); the duplicate
    // and the ordering event both start at T0, so the code rank decides.
    expect(report.events.map((entry) => [entry.code, entry.count])).toEqual([
      ['out_of_order', 7],
      ['duplicate_timestamp', 2],
      ['unaligned_timestamp', 1],
      ['unexpected_bar', 1],
    ]);
    expect(report.events.map((entry) => entry.action)).toEqual([
      'resort_source',
      'deduplicate_source',
      'verify_time_unit',
      'review_calendar',
    ]);
  });

  it('never calls a bar that has not closed yet missing, and flags one that arrived early', () => {
    const early = auditCoverage({
      interval: '1h',
      expected,
      observed: [T0, T0 + HOUR, T0 + 2 * HOUR, T0 + 3 * HOUR],
      asOfMs: T0 + 3 * HOUR + 1,
    });
    expect(early.notDueCount).toBe(1);
    expect(early.events).toEqual([
      {
        code: 'unclosed_bar',
        severity: 'blocking',
        rangeStart: T0 + 3 * HOUR,
        rangeEnd: T0 + 3 * HOUR,
        count: 1,
        action: 'wait_for_close',
      },
    ]);

    const waiting = auditCoverage({
      interval: '1h',
      expected,
      observed: [T0, T0 + HOUR, T0 + 2 * HOUR],
      asOfMs: T0 + 3 * HOUR + 1,
    });
    expect(waiting.events).toEqual([]);
    expect(waiting.blocking).toBe(false);
    expect(waiting.notDueCount).toBe(1);
  });

  it('blocks on an interval it cannot measure', () => {
    const report = auditCoverage({ interval: '2h', expected, observed: expected, asOfMs });
    expect(report.issue).toBe('unknown_interval');
    expect(report.blocking).toBe(true);
    expect(report.events).toEqual([]);
  });
});
