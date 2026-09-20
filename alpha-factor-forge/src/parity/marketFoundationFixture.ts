// TypeScript-reference builder for the P06 market-foundation parity fixture
// (`market-foundation-v1`). Pure and deterministic;
// scripts/generate-market-foundation-fixtures.ts owns file IO.
//
// This module is DELIBERATELY independent of src/core/market-data/foundation.ts.
// Every expectation below is authored from docs/market-contract.md and
// docs/market-foundation-v1.md, not recorded from a running implementation, so
// the fixture is a contract both runtimes are measured against rather than a
// snapshot of whichever one was written first. Nothing here may import the
// implementation.
//
// The acceptance list this fixture exists for (plan §5, P06): time-unit
// change, missing bars, duplicates, source confusion, and — in the storage
// half, which is Rust-only and therefore not in this file — revisions.

import { FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import {
  SPECIAL_INPUT_NUMBER_ENCODING,
  type FixtureNumber,
} from './marketDataQualityFixture';

export const PARITY_FIXTURE_SCHEMA_VERSION = 'rs-core-parity-fixture-v1';
export const MARKET_FOUNDATION_FIXTURE_VERSION = 'market-foundation-parity-v1';
export const MARKET_FOUNDATION_CONTRACT_VERSION = 'market-foundation-v1';
export const MARKET_INSTRUMENT_CONTRACT_VERSION = 'market-instrument-v1';
export const SESSION_CALENDAR_CONTRACT_VERSION = 'session-calendar-v1';
export const MARKET_DATA_QUALITY_CONTRACT_VERSION = 'market-data-quality-v1';
export const EXPECTED_NUMERIC_POLICY = 'exact-v1';

/** The plausibility window every time-unit band is derived from. */
export const MIN_MARKET_TIMESTAMP_MS = 946_684_800_000;
export const MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE = 4_102_444_800_000;
export const MAX_EXPECTED_BARS = 1_000_000;

export const INTERVALS: readonly (readonly [string, number])[] = [
  ['1m', 60_000],
  ['3m', 180_000],
  ['5m', 300_000],
  ['15m', 900_000],
  ['1h', 3_600_000],
  ['4h', 14_400_000],
  ['1d', 86_400_000],
];

export const MARKET_IDS = ['crypto', 'us-etf', 'tw-etf'] as const;
export const ASSET_TYPE_IDS = ['spot-crypto', 'etf'] as const;
export const PRICE_BASIS_IDS = ['raw', 'adjusted-split', 'adjusted-total-return'] as const;
export const CALENDAR_KIND_IDS = ['continuous', 'trading-days'] as const;
export const SEVERITY_IDS = ['blocking', 'degraded', 'info'] as const;

export const INSTRUMENT_ID_RULE_IDS = [
  'instrument_id_shape',
  'unknown_market',
  'venue_not_normalized',
  'symbol_not_supported',
] as const;

export const CALENDAR_RULE_IDS = [
  'calendar_id_shape',
  'unknown_kind',
  'empty_timezone',
  'continuous_carries_trading_days',
  'trading_days_without_weekdays',
  'weekday_out_of_range',
  'weekdays_not_sorted',
  'invalid_date',
  'dates_not_sorted',
] as const;

/**
 * `unknown_kind` cannot be produced by the Rust side: there the kind is an
 * enum, so an unknown value never reaches the validator. It therefore has no
 * shared fixture row and is unit-tested per language, exactly as
 * `timestamp_not_representable` is in `market-data-quality-v1`.
 */
export const RUNTIME_SPECIFIC_CALENDAR_RULE_IDS = ['unknown_kind'] as const;

export const TIME_UNIT_IDS = ['seconds', 'milliseconds', 'microseconds', 'nanoseconds'] as const;
export const TIME_UNIT_ISSUE_IDS = [
  'no_timestamps',
  'time_unit_mixed',
  'time_unit_unknown',
] as const;

export const EXPECTED_RANGE_ISSUE_IDS = [
  'unknown_interval',
  'invalid_calendar',
  'unsupported_interval_for_calendar',
  'invalid_range',
  'invalid_suspension',
  'range_too_large',
] as const;

/** Declared order IS the event sort rank; see docs/market-foundation-v1.md §4. */
export const COVERAGE_CODES = [
  'out_of_order',
  'duplicate_timestamp',
  'unaligned_timestamp',
  'unexpected_bar',
  'missing_bar',
  'unclosed_bar',
] as const;

export const ACTION_CODES = [
  'refetch_range',
  'deduplicate_source',
  'resort_source',
  'verify_time_unit',
  'wait_for_close',
  'review_calendar',
  'separate_sources',
  'confirm_costs',
  'verify_corporate_actions',
  'rebuild_from_revision',
  'record_availability',
] as const;

/**
 * Actions no coverage report can ask for: they belong to the storage half
 * (snapshot composition, cost confirmation, corporate actions, revisions),
 * which is Rust-only. Asserted absent rather than left unstated.
 */
export const STORAGE_ONLY_ACTION_CODES = [
  'separate_sources',
  'confirm_costs',
  'verify_corporate_actions',
  'rebuild_from_revision',
  'record_availability',
] as const;

export const SERIES_CONFLICT_CODES = [
  'comparison_role_not_combinable',
  'interval_mismatch',
  'invalid_instrument_id',
  'market_mismatch',
  'price_basis_mismatch',
  'quote_mismatch',
  'source_mismatch',
  'symbol_mismatch',
  'venue_mismatch',
] as const;

type InstrumentIdRule = (typeof INSTRUMENT_ID_RULE_IDS)[number];
type CalendarRule = (typeof CALENDAR_RULE_IDS)[number];
type TimeUnitId = (typeof TIME_UNIT_IDS)[number];
type TimeUnitIssue = (typeof TIME_UNIT_ISSUE_IDS)[number];
type ExpectedRangeIssue = (typeof EXPECTED_RANGE_ISSUE_IDS)[number];
type CoverageCode = (typeof COVERAGE_CODES)[number];
type ActionCode = (typeof ACTION_CODES)[number];
type SeriesConflictCode = (typeof SERIES_CONFLICT_CODES)[number];

export interface FixtureCalendar {
  calendarId: string;
  kind: (typeof CALENDAR_KIND_IDS)[number];
  timezone: string;
  tradingWeekdays: number[];
  holidays: string[];
  earlyCloses: string[];
}

export interface FixtureSeriesIdentity {
  instrumentId: string;
  quote: string;
  interval: string;
  source: string;
  role: 'primary' | 'comparison';
  priceBasis: (typeof PRICE_BASIS_IDS)[number];
}

export interface FixtureCoverageEvent {
  code: CoverageCode;
  severity: (typeof SEVERITY_IDS)[number];
  rangeStart: number;
  rangeEnd: number;
  count: number;
  action: ActionCode;
}

// ---------------------------------------------------------- authored inputs

const MINUTE = 60_000;
const HOUR = 3_600_000;
const DAY = 86_400_000;
/** 2024-07-15T00:00:00Z — a Monday, and a UTC midnight. */
const T0 = 1_721_001_600_000;

const CRYPTO_247: FixtureCalendar = {
  calendarId: 'crypto-24x7-v1',
  kind: 'continuous',
  timezone: 'UTC',
  tradingWeekdays: [],
  holidays: [],
  earlyCloses: [],
};

/**
 * A weekday calendar with one holiday on Thursday 2024-07-18. Named
 * `fixture-…` because it is authored test data, NOT a claim about any real
 * exchange's holidays — those arrive with P09/P10 from their own sources.
 */
const FIXTURE_WEEKDAYS: FixtureCalendar = {
  calendarId: 'fixture-weekdays-v1',
  kind: 'trading-days',
  timezone: 'America/New_York',
  tradingWeekdays: [1, 2, 3, 4, 5],
  holidays: ['2024-07-18'],
  earlyCloses: ['2024-07-19'],
};

const BROKEN_CALENDAR: FixtureCalendar = {
  ...CRYPTO_247,
  calendarId: 'crypto-24x7',
};

const CALENDARS: FixtureCalendar[] = [CRYPTO_247, FIXTURE_WEEKDAYS, BROKEN_CALENDAR];

// ---------------------------------------------------------------- case sets

export interface InstrumentIdCase {
  id: string;
  value: string;
  expected:
    | { parsed: { market: string; venue: string; symbol: string }; rule: null }
    | { parsed: null; rule: InstrumentIdRule };
}

function instrumentIdCases(): InstrumentIdCase[] {
  const accept = (id: string, value: string, market: string, venue: string, symbol: string): InstrumentIdCase => ({
    id,
    value,
    expected: { parsed: { market, venue, symbol }, rule: null },
  });
  const reject = (id: string, value: string, rule: InstrumentIdRule): InstrumentIdCase => ({
    id,
    value,
    expected: { parsed: null, rule },
  });
  return [
    accept('crypto-binance-btcusdt', 'crypto:binance:BTCUSDT', 'crypto', 'binance', 'BTCUSDT'),
    // USDT stays USDT: a renamed quote currency is a silent merge.
    accept('crypto-binance-ethusdt', 'crypto:binance:ETHUSDT', 'crypto', 'binance', 'ETHUSDT'),
    accept('us-etf-nyse-arca-spy', 'us-etf:nyse-arca:SPY', 'us-etf', 'nyse-arca', 'SPY'),
    accept('tw-etf-twse-0050', 'tw-etf:twse:0050', 'tw-etf', 'twse', '0050'),
    accept('crypto-coinbase-btc-usd', 'crypto:coinbase:BTC-USD', 'crypto', 'coinbase', 'BTC-USD'),
    reject('two-fields-rejected', 'crypto:binance', 'instrument_id_shape'),
    reject('empty-venue-rejected', 'crypto::BTCUSDT', 'instrument_id_shape'),
    reject('four-fields-rejected', 'crypto:binance:BTC:USDT', 'instrument_id_shape'),
    reject('unknown-market-rejected', 'fx:oanda:EURUSD', 'unknown_market'),
    reject('uppercase-venue-rejected', 'crypto:Binance:BTCUSDT', 'venue_not_normalized'),
    reject('symbol-with-space-rejected', 'crypto:binance:BTC USDT', 'symbol_not_supported'),
  ];
}

export interface TimeUnitCase {
  id: string;
  timestamps: FixtureNumber[];
  expected: { unit: TimeUnitId | null; issue: TimeUnitIssue | null; index: number | null };
}

function timeUnitCases(): TimeUnitCase[] {
  return [
    {
      id: 'milliseconds-batch',
      timestamps: [T0, T0 + HOUR, T0 + 2 * HOUR],
      expected: { unit: 'milliseconds', issue: null, index: null },
    },
    {
      id: 'seconds-batch',
      timestamps: [T0 / 1000, T0 / 1000 + 3600],
      expected: { unit: 'seconds', issue: null, index: null },
    },
    {
      id: 'microseconds-batch',
      timestamps: [T0 * 1000],
      expected: { unit: 'microseconds', issue: null, index: null },
    },
    {
      id: 'nanoseconds-batch',
      timestamps: [T0 * 1_000_000],
      expected: { unit: 'nanoseconds', issue: null, index: null },
    },
    {
      // The archive change the contract names (docs/market-contract.md §4):
      // milliseconds for two bars, then microseconds.
      id: 'archive-switches-to-microseconds-mid-batch',
      timestamps: [T0, T0 + HOUR, (T0 + 2 * HOUR) * 1000],
      expected: { unit: null, issue: 'time_unit_mixed', index: 2 },
    },
    {
      id: 'epoch-zero-is-in-no-band',
      timestamps: [0],
      expected: { unit: null, issue: 'time_unit_unknown', index: 0 },
    },
    {
      id: 'non-finite-is-in-no-band',
      timestamps: [T0, 'nan'],
      expected: { unit: null, issue: 'time_unit_unknown', index: 1 },
    },
    {
      id: 'empty-batch',
      timestamps: [],
      expected: { unit: null, issue: 'no_timestamps', index: null },
    },
  ];
}

export interface CalendarValidationCase {
  id: string;
  calendar: FixtureCalendar;
  expected: { rule: CalendarRule | null };
}

function calendarValidationCases(): CalendarValidationCase[] {
  const at = (id: string, calendar: FixtureCalendar, rule: CalendarRule | null): CalendarValidationCase => ({
    id,
    calendar,
    expected: { rule },
  });
  return [
    at('continuous-accepted', CRYPTO_247, null),
    at('trading-days-accepted', FIXTURE_WEEKDAYS, null),
    at('id-without-a-version-rejected', BROKEN_CALENDAR, 'calendar_id_shape'),
    at('blank-timezone-rejected', { ...CRYPTO_247, timezone: ' ' }, 'empty_timezone'),
    at(
      'continuous-with-holidays-rejected',
      { ...CRYPTO_247, holidays: ['2024-07-18'] },
      'continuous_carries_trading_days',
    ),
    at(
      'trading-days-without-weekdays-rejected',
      { ...FIXTURE_WEEKDAYS, tradingWeekdays: [] },
      'trading_days_without_weekdays',
    ),
    at(
      'weekday-zero-rejected',
      { ...FIXTURE_WEEKDAYS, tradingWeekdays: [0, 1, 2] },
      'weekday_out_of_range',
    ),
    at(
      'weekdays-unsorted-rejected',
      { ...FIXTURE_WEEKDAYS, tradingWeekdays: [2, 1] },
      'weekdays_not_sorted',
    ),
    at(
      'impossible-date-rejected',
      { ...FIXTURE_WEEKDAYS, holidays: ['2024-02-30'] },
      'invalid_date',
    ),
    at(
      'holidays-unsorted-rejected',
      { ...FIXTURE_WEEKDAYS, holidays: ['2024-07-19', '2024-07-18'] },
      'dates_not_sorted',
    ),
  ];
}

export interface ExpectedRangeCase {
  id: string;
  calendarId: string;
  interval: string;
  fromMs: number;
  toMsExclusive: number;
  listedFromMs: number | null;
  delistedAtMs: number | null;
  suspensions: { fromMs: number; toMsExclusive: number }[];
  expected: { timestamps: number[]; issue: ExpectedRangeIssue | null };
}

function expectedRangeCases(): ExpectedRangeCase[] {
  const base = {
    calendarId: CRYPTO_247.calendarId,
    interval: '1h',
    fromMs: T0,
    toMsExclusive: T0 + 3 * HOUR,
    listedFromMs: null,
    delistedAtMs: null,
    suspensions: [],
  };
  return [
    {
      ...base,
      id: 'crypto-hourly-three-bars',
      expected: { timestamps: [T0, T0 + HOUR, T0 + 2 * HOUR], issue: null },
    },
    {
      // A range that starts mid-bar begins at the next grid point; the
      // half-open bar before it is not invented.
      ...base,
      id: 'crypto-hourly-range-starts-mid-bar',
      fromMs: T0 + 1,
      toMsExclusive: T0 + 2 * HOUR,
      expected: { timestamps: [T0 + HOUR], issue: null },
    },
    {
      ...base,
      id: 'weekdays-skip-weekend-holiday-and-halt',
      calendarId: FIXTURE_WEEKDAYS.calendarId,
      interval: '1d',
      toMsExclusive: T0 + 7 * DAY,
      suspensions: [{ fromMs: T0 + DAY, toMsExclusive: T0 + 2 * DAY }],
      // Mon 15 kept, Tue 16 halted, Wed 17 kept, Thu 18 a holiday,
      // Fri 19 kept, Sat 20 and Sun 21 are not trading days.
      expected: { timestamps: [T0, T0 + 2 * DAY, T0 + 4 * DAY], issue: null },
    },
    {
      ...base,
      id: 'listing-window-clamps-the-range',
      calendarId: FIXTURE_WEEKDAYS.calendarId,
      interval: '1d',
      toMsExclusive: T0 + 7 * DAY,
      listedFromMs: T0 + 2 * DAY,
      delistedAtMs: T0 + 4 * DAY,
      expected: { timestamps: [T0 + 2 * DAY], issue: null },
    },
    {
      ...base,
      id: 'a-listing-after-the-range-is-empty-without-a-defect',
      listedFromMs: T0 + 10 * DAY,
      expected: { timestamps: [], issue: null },
    },
    {
      ...base,
      id: 'unknown-interval-refused',
      interval: '2h',
      expected: { timestamps: [], issue: 'unknown_interval' },
    },
    {
      ...base,
      id: 'invalid-calendar-refused',
      calendarId: BROKEN_CALENDAR.calendarId,
      expected: { timestamps: [], issue: 'invalid_calendar' },
    },
    {
      ...base,
      id: 'intraday-on-a-trading-days-calendar-refused',
      calendarId: FIXTURE_WEEKDAYS.calendarId,
      expected: { timestamps: [], issue: 'unsupported_interval_for_calendar' },
    },
    {
      ...base,
      id: 'empty-range-refused',
      toMsExclusive: T0,
      expected: { timestamps: [], issue: 'invalid_range' },
    },
    {
      ...base,
      id: 'backwards-suspension-refused',
      suspensions: [{ fromMs: T0 + DAY, toMsExclusive: T0 }],
      expected: { timestamps: [], issue: 'invalid_suspension' },
    },
    {
      ...base,
      id: 'oversized-range-refused',
      interval: '1m',
      toMsExclusive: T0 + (MAX_EXPECTED_BARS + 1) * MINUTE,
      expected: { timestamps: [], issue: 'range_too_large' },
    },
  ];
}

export interface CoverageCase {
  id: string;
  interval: string;
  expected: number[];
  observed: number[];
  asOfMs: number;
  expectedReport: {
    events: FixtureCoverageEvent[];
    blocking: boolean;
    expectedCount: number;
    notDueCount: number;
    observedCount: number;
    matchedCount: number;
    issue: ExpectedRangeIssue | null;
  };
}

const COVERAGE_ACTIONS: Record<CoverageCode, ActionCode> = {
  out_of_order: 'resort_source',
  duplicate_timestamp: 'deduplicate_source',
  unaligned_timestamp: 'verify_time_unit',
  unexpected_bar: 'review_calendar',
  missing_bar: 'refetch_range',
  unclosed_bar: 'wait_for_close',
};

function blocking(
  code: CoverageCode,
  rangeStart: number,
  rangeEnd: number,
  count: number,
): FixtureCoverageEvent {
  return { code, severity: 'blocking', rangeStart, rangeEnd, count, action: COVERAGE_ACTIONS[code] };
}

function coverageCases(): CoverageCase[] {
  const hourly = [T0, T0 + HOUR, T0 + 2 * HOUR, T0 + 3 * HOUR];
  const closed = T0 + 4 * HOUR;
  // Friday 19th, Monday 22nd, Tuesday 23rd on the weekday calendar.
  const daily = [T0 + 4 * DAY, T0 + 7 * DAY, T0 + 8 * DAY];
  return [
    {
      id: 'complete-series',
      interval: '1h',
      expected: hourly,
      observed: hourly,
      asOfMs: closed,
      expectedReport: {
        events: [],
        blocking: false,
        expectedCount: 4,
        notDueCount: 0,
        observedCount: 4,
        matchedCount: 4,
        issue: null,
      },
    },
    {
      id: 'one-missing-bar',
      interval: '1h',
      expected: hourly,
      observed: [T0, T0 + HOUR, T0 + 3 * HOUR],
      asOfMs: closed,
      expectedReport: {
        events: [blocking('missing_bar', T0 + 2 * HOUR, T0 + 2 * HOUR, 1)],
        blocking: true,
        expectedCount: 4,
        notDueCount: 0,
        observedCount: 3,
        matchedCount: 3,
        issue: null,
      },
    },
    {
      id: 'contiguous-gap-is-one-actionable-range',
      interval: '1h',
      expected: hourly,
      observed: [T0, T0 + 3 * HOUR],
      asOfMs: closed,
      expectedReport: {
        events: [blocking('missing_bar', T0 + HOUR, T0 + 2 * HOUR, 2)],
        blocking: true,
        expectedCount: 4,
        notDueCount: 0,
        observedCount: 2,
        matchedCount: 2,
        issue: null,
      },
    },
    {
      // Contiguity is in the EXPECTED grid: the weekend between Friday and
      // Monday does not split one gap into two.
      id: 'a-weekend-inside-a-gap-does-not-split-it',
      interval: '1d',
      expected: daily,
      observed: [T0 + 8 * DAY],
      asOfMs: T0 + 9 * DAY,
      expectedReport: {
        events: [blocking('missing_bar', T0 + 4 * DAY, T0 + 7 * DAY, 2)],
        blocking: true,
        expectedCount: 3,
        notDueCount: 0,
        observedCount: 1,
        matchedCount: 1,
        issue: null,
      },
    },
    {
      id: 'duplicate-timestamp',
      interval: '1h',
      expected: hourly,
      observed: [T0, T0 + HOUR, T0 + HOUR, T0 + 2 * HOUR, T0 + 3 * HOUR],
      asOfMs: closed,
      expectedReport: {
        events: [blocking('duplicate_timestamp', T0 + HOUR, T0 + HOUR, 2)],
        blocking: true,
        expectedCount: 4,
        notDueCount: 0,
        observedCount: 5,
        matchedCount: 4,
        issue: null,
      },
    },
    {
      // A timestamp one millisecond off the grid is not "close enough": it is
      // the signature of a unit or offset mistake, so it is never matched to
      // the bar it resembles — which also leaves that bar missing.
      id: 'unaligned-timestamp-leaves-its-bar-missing',
      interval: '1h',
      expected: hourly,
      observed: [T0, T0 + HOUR, T0 + 2 * HOUR, T0 + 3 * HOUR + 1],
      asOfMs: closed,
      expectedReport: {
        events: [
          blocking('missing_bar', T0 + 3 * HOUR, T0 + 3 * HOUR, 1),
          blocking('unaligned_timestamp', T0 + 3 * HOUR + 1, T0 + 3 * HOUR + 1, 1),
        ],
        blocking: true,
        expectedCount: 4,
        notDueCount: 0,
        observedCount: 4,
        matchedCount: 3,
        issue: null,
      },
    },
    {
      id: 'a-bar-on-a-non-trading-day-is-unexpected',
      interval: '1d',
      // Monday 15th and Wednesday 17th, with Tuesday a non-trading day.
      expected: [T0, T0 + 2 * DAY],
      observed: [T0, T0 + DAY, T0 + 2 * DAY],
      asOfMs: T0 + 3 * DAY,
      expectedReport: {
        events: [blocking('unexpected_bar', T0 + DAY, T0 + DAY, 1)],
        blocking: true,
        expectedCount: 2,
        notDueCount: 0,
        observedCount: 3,
        matchedCount: 2,
        issue: null,
      },
    },
    {
      // Both events start at T0; the declared code rank decides the order.
      id: 'out-of-order-and-duplicate-at-the-same-bar',
      interval: '1h',
      expected: hourly,
      observed: [T0 + HOUR, T0, T0, T0 + 2 * HOUR, T0 + 3 * HOUR],
      asOfMs: closed,
      expectedReport: {
        events: [
          blocking('out_of_order', T0, T0 + 3 * HOUR, 5),
          blocking('duplicate_timestamp', T0, T0, 2),
        ],
        blocking: true,
        expectedCount: 4,
        notDueCount: 0,
        observedCount: 5,
        matchedCount: 4,
        issue: null,
      },
    },
    {
      id: 'a-bar-that-has-not-closed-yet-is-reported-not-matched-away',
      interval: '1h',
      expected: hourly,
      observed: hourly,
      asOfMs: T0 + 3 * HOUR + 1,
      expectedReport: {
        events: [blocking('unclosed_bar', T0 + 3 * HOUR, T0 + 3 * HOUR, 1)],
        blocking: true,
        expectedCount: 4,
        notDueCount: 1,
        observedCount: 4,
        matchedCount: 4,
        issue: null,
      },
    },
    {
      id: 'a-bar-that-is-not-due-yet-is-never-missing',
      interval: '1h',
      expected: hourly,
      observed: [T0, T0 + HOUR, T0 + 2 * HOUR],
      asOfMs: T0 + 3 * HOUR + 1,
      expectedReport: {
        events: [],
        blocking: false,
        expectedCount: 4,
        notDueCount: 1,
        observedCount: 3,
        matchedCount: 3,
        issue: null,
      },
    },
    {
      id: 'an-interval-the-contract-cannot-measure-blocks',
      interval: '2h',
      expected: hourly,
      observed: hourly,
      asOfMs: closed,
      expectedReport: {
        events: [],
        blocking: true,
        expectedCount: 4,
        notDueCount: 0,
        observedCount: 4,
        matchedCount: 0,
        issue: 'unknown_interval',
      },
    },
  ];
}

export interface SourceOriginCase {
  id: string;
  a: string;
  b: string;
  expected: { origin: string; shareOrigin: boolean };
}

/**
 * `<origin>[-<endpoint>]`: two endpoints of one exchange are one publisher,
 * two exchanges are not. This is what allows the archive's tail to be
 * filled from the same exchange's REST endpoint without allowing a second
 * exchange to be spliced in (plan §5 P07, docs/market-contract.md §4).
 */
function sourceOriginCases(): SourceOriginCase[] {
  return [
    {
      id: 'two-endpoints-of-one-exchange',
      a: 'binance-archive',
      b: 'binance-rest',
      expected: { origin: 'binance', shareOrigin: true },
    },
    {
      id: 'two-exchanges-are-never-one-publisher',
      a: 'binance-archive',
      b: 'coinbase-rest',
      expected: { origin: 'binance', shareOrigin: false },
    },
    {
      id: 'an-origin-without-an-endpoint',
      a: 'tiingo',
      b: 'tiingo',
      expected: { origin: 'tiingo', shareOrigin: true },
    },
    {
      id: 'an-origin-matches-its-own-endpointed-form',
      a: 'binance',
      b: 'binance-archive',
      expected: { origin: 'binance', shareOrigin: true },
    },
    {
      id: 'an-empty-source-is-nobody',
      a: '',
      b: '',
      expected: { origin: '', shareOrigin: false },
    },
    {
      id: 'a-leading-separator-is-nobody',
      a: '-rest',
      b: '-archive',
      expected: { origin: '', shareOrigin: false },
    },
  ];
}

export interface SeriesCombinationCase {
  id: string;
  a: FixtureSeriesIdentity;
  b: FixtureSeriesIdentity;
  expected: { conflicts: SeriesConflictCode[] };
}

function seriesCombinationCases(): SeriesCombinationCase[] {
  const binance: FixtureSeriesIdentity = {
    instrumentId: 'crypto:binance:BTCUSDT',
    quote: 'USDT',
    interval: '1h',
    source: 'binance-archive',
    role: 'primary',
    priceBasis: 'raw',
  };
  return [
    {
      id: 'same-primary-source-combinable',
      a: binance,
      b: { ...binance },
      expected: { conflicts: [] },
    },
    {
      // The named source-confusion case: the same asset at another venue, in
      // another quote currency, from another source. Never one series.
      id: 'binance-and-coinbase-are-never-one-series',
      a: binance,
      b: {
        ...binance,
        instrumentId: 'crypto:coinbase:BTC-USD',
        quote: 'USD',
        source: 'coinbase-rest',
      },
      expected: {
        conflicts: ['quote_mismatch', 'source_mismatch', 'symbol_mismatch', 'venue_mismatch'],
      },
    },
    {
      id: 'a-different-market-conflicts-on-every-axis',
      a: binance,
      b: {
        instrumentId: 'us-etf:nyse-arca:SPY',
        quote: 'USD',
        interval: '1d',
        source: 'tiingo',
        role: 'primary',
        priceBasis: 'adjusted-split',
      },
      expected: {
        conflicts: [
          'interval_mismatch',
          'market_mismatch',
          'price_basis_mismatch',
          'quote_mismatch',
          'source_mismatch',
          'symbol_mismatch',
          'venue_mismatch',
        ],
      },
    },
    {
      id: 'two-intervals-are-two-series',
      a: binance,
      b: { ...binance, interval: '1d' },
      expected: { conflicts: ['interval_mismatch'] },
    },
    {
      id: 'raw-and-adjusted-are-two-series',
      a: binance,
      b: { ...binance, priceBasis: 'adjusted-total-return' },
      expected: { conflicts: ['price_basis_mismatch'] },
    },
    {
      id: 'a-comparison-source-is-evidence-not-a-component',
      a: binance,
      b: { ...binance, role: 'comparison' },
      expected: { conflicts: ['comparison_role_not_combinable'] },
    },
    {
      id: 'an-unparsable-id-is-refused-not-guessed',
      a: binance,
      b: { ...binance, instrumentId: 'binance-BTCUSDT' },
      expected: { conflicts: ['invalid_instrument_id'] },
    },
  ];
}

// ------------------------- build-time self-checks (no implementation used)

function uniqueIds(ids: string[], group: string): void {
  if (new Set(ids).size !== ids.length) {
    throw new Error(`market-foundation fixture has duplicate case ids in ${group}`);
  }
}

function assertMatrixCoverage(cases: {
  instrumentIds: InstrumentIdCase[];
  timeUnits: TimeUnitCase[];
  calendarValidation: CalendarValidationCase[];
  expectedRanges: ExpectedRangeCase[];
  coverage: CoverageCase[];
  seriesCombination: SeriesCombinationCase[];
  sourceOrigins: SourceOriginCase[];
}): void {
  uniqueIds(cases.instrumentIds.map((entry) => entry.id), 'instrumentIds');
  uniqueIds(cases.timeUnits.map((entry) => entry.id), 'timeUnits');
  uniqueIds(cases.calendarValidation.map((entry) => entry.id), 'calendarValidation');
  uniqueIds(cases.expectedRanges.map((entry) => entry.id), 'expectedRanges');
  uniqueIds(cases.coverage.map((entry) => entry.id), 'coverage');
  uniqueIds(cases.seriesCombination.map((entry) => entry.id), 'seriesCombination');

  const missing = (what: string, id: string): never => {
    throw new Error(`market-foundation fixture is missing a ${what} row for ${id}`);
  };

  for (const rule of INSTRUMENT_ID_RULE_IDS) {
    if (!cases.instrumentIds.some((entry) => entry.expected.rule === rule)) {
      missing('instrument-id rejection', rule);
    }
  }
  if (!cases.instrumentIds.some((entry) => entry.expected.rule === null)) {
    missing('instrument-id acceptance', 'any');
  }
  for (const unit of TIME_UNIT_IDS) {
    if (!cases.timeUnits.some((entry) => entry.expected.unit === unit)) missing('time-unit', unit);
  }
  for (const issue of TIME_UNIT_ISSUE_IDS) {
    if (!cases.timeUnits.some((entry) => entry.expected.issue === issue)) {
      missing('time-unit issue', issue);
    }
  }
  for (const rule of CALENDAR_RULE_IDS) {
    const covered = cases.calendarValidation.some((entry) => entry.expected.rule === rule);
    const runtimeSpecific = (RUNTIME_SPECIFIC_CALENDAR_RULE_IDS as readonly string[]).includes(rule);
    if (covered === runtimeSpecific) {
      throw new Error(
        runtimeSpecific
          ? `${rule} cannot be produced by both runtimes and must not have a shared row`
          : `market-foundation fixture is missing a calendar rejection row for ${rule}`,
      );
    }
  }
  for (const issue of EXPECTED_RANGE_ISSUE_IDS) {
    if (!cases.expectedRanges.some((entry) => entry.expected.issue === issue)) {
      missing('expected-range issue', issue);
    }
  }
  const producedCodes = new Set(
    cases.coverage.flatMap((entry) => entry.expectedReport.events.map((event) => event.code)),
  );
  for (const code of COVERAGE_CODES) {
    if (!producedCodes.has(code)) missing('coverage event', code);
  }
  const producedActions = new Set(
    cases.coverage.flatMap((entry) => entry.expectedReport.events.map((event) => event.action)),
  );
  for (const action of STORAGE_ONLY_ACTION_CODES) {
    if (producedActions.has(action)) {
      throw new Error(`${action} belongs to the storage half and must not appear in a report`);
    }
  }
  const producedConflicts = new Set(
    cases.seriesCombination.flatMap((entry) => entry.expected.conflicts),
  );
  for (const code of SERIES_CONFLICT_CODES) {
    if (!producedConflicts.has(code)) missing('series conflict', code);
  }
  if (!cases.seriesCombination.some((entry) => entry.expected.conflicts.length === 0)) {
    missing('combinable series', 'any');
  }
  uniqueIds(cases.sourceOrigins.map((entry) => entry.id), 'sourceOrigins');
  if (!cases.sourceOrigins.some((entry) => entry.expected.shareOrigin)) {
    missing('shared-origin', 'any');
  }
  if (!cases.sourceOrigins.some((entry) => !entry.expected.shareOrigin)) {
    missing('distinct-origin', 'any');
  }

  // Every expected report is internally consistent, independently of any
  // implementation: sorted events, sane counts, and blocking iff an event is.
  for (const entry of cases.coverage) {
    const report = entry.expectedReport;
    const rank = (code: CoverageCode): number => COVERAGE_CODES.indexOf(code);
    for (let index = 1; index < report.events.length; index++) {
      const previous = report.events[index - 1];
      const current = report.events[index];
      const ordered =
        previous.rangeStart < current.rangeStart ||
        (previous.rangeStart === current.rangeStart && rank(previous.code) < rank(current.code));
      if (!ordered) throw new Error(`coverage case ${entry.id} lists events out of contract order`);
    }
    for (const event of report.events) {
      if (event.rangeEnd < event.rangeStart || event.count < 1) {
        throw new Error(`coverage case ${entry.id} has an impossible event range`);
      }
      if (event.action !== COVERAGE_ACTIONS[event.code]) {
        throw new Error(`coverage case ${entry.id} pairs ${event.code} with the wrong action`);
      }
    }
    const blocks = report.events.some((event) => event.severity === 'blocking');
    if (report.blocking !== (blocks || report.issue !== null)) {
      throw new Error(`coverage case ${entry.id} disagrees with its own blocking flag`);
    }
    if (report.expectedCount !== entry.expected.length) {
      throw new Error(`coverage case ${entry.id} miscounts its expected bars`);
    }
    if (report.observedCount !== entry.observed.length) {
      throw new Error(`coverage case ${entry.id} miscounts its observed bars`);
    }
  }

  // Every referenced calendar is defined, and every defined one is referenced.
  const defined = new Set(CALENDARS.map((calendar) => calendar.calendarId));
  const referenced = new Set(cases.expectedRanges.map((entry) => entry.calendarId));
  for (const id of referenced) {
    if (!defined.has(id)) throw new Error(`expected-range cases reference unknown calendar ${id}`);
  }
  for (const id of defined) {
    if (!referenced.has(id)) throw new Error(`calendar ${id} is defined but never used`);
  }
}

export interface FixtureSourceHashes {
  generator: string;
}

export function buildMarketFoundationParityFixture(sourceHashes: FixtureSourceHashes) {
  const cases = {
    instrumentIds: instrumentIdCases(),
    timeUnits: timeUnitCases(),
    calendarValidation: calendarValidationCases(),
    expectedRanges: expectedRangeCases(),
    coverage: coverageCases(),
    seriesCombination: seriesCombinationCases(),
    sourceOrigins: sourceOriginCases(),
  };
  assertMatrixCoverage(cases);
  return {
    schemaVersion: PARITY_FIXTURE_SCHEMA_VERSION,
    fixtureVersion: MARKET_FOUNDATION_FIXTURE_VERSION,
    contracts: {
      marketFoundation: MARKET_FOUNDATION_CONTRACT_VERSION,
      marketInstrument: MARKET_INSTRUMENT_CONTRACT_VERSION,
      sessionCalendar: SESSION_CALENDAR_CONTRACT_VERSION,
      marketDataQuality: MARKET_DATA_QUALITY_CONTRACT_VERSION,
    },
    generator: {
      command: 'npm run fixtures:market-foundation',
      referenceRuntime: 'typescript',
      sourceHashEncoding: FIXTURE_SOURCE_HASH_ENCODING,
      sourceHashes,
    },
    numericEncoding: {
      specialInputNumbers: SPECIAL_INPUT_NUMBER_ENCODING,
      expectedNumericPolicy: EXPECTED_NUMERIC_POLICY,
    },
    constants: {
      minTimestampMs: MIN_MARKET_TIMESTAMP_MS,
      maxTimestampMsExclusive: MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE,
      maxExpectedBars: MAX_EXPECTED_BARS,
    },
    intervals: INTERVALS.map(([interval, ms]) => ({ interval, ms })),
    inventories: {
      markets: [...MARKET_IDS],
      assetTypes: [...ASSET_TYPE_IDS],
      priceBases: [...PRICE_BASIS_IDS],
      calendarKinds: [...CALENDAR_KIND_IDS],
      severities: [...SEVERITY_IDS],
      instrumentIdRuleIds: [...INSTRUMENT_ID_RULE_IDS],
      calendarRuleIds: [...CALENDAR_RULE_IDS],
      runtimeSpecificCalendarRuleIds: [...RUNTIME_SPECIFIC_CALENDAR_RULE_IDS],
      timeUnitIds: [...TIME_UNIT_IDS],
      timeUnitIssueIds: [...TIME_UNIT_ISSUE_IDS],
      expectedRangeIssueIds: [...EXPECTED_RANGE_ISSUE_IDS],
      coverageCodes: [...COVERAGE_CODES],
      actionCodes: [...ACTION_CODES],
      storageOnlyActionCodes: [...STORAGE_ONLY_ACTION_CODES],
      seriesConflictCodes: [...SERIES_CONFLICT_CODES],
    },
    calendars: CALENDARS,
    tolerance: {
      // Classification, not computation: every leaf compares exactly.
      policy: EXPECTED_NUMERIC_POLICY,
      exact: [
        'schema, fixture, and contract versions',
        'case ids and inventory order',
        'instrument-id parses and rule ids',
        'time-unit verdicts and the index a batch changes unit at',
        'calendar rule ids',
        'expected bar timestamps, refusal issues, and their emptiness',
        'coverage event codes, severities, ranges, counts, actions, and order',
        'series conflict codes and their sorted order',
      ],
    },
    cases,
  };
}

export type MarketFoundationParityFixture = ReturnType<
  typeof buildMarketFoundationParityFixture
>;
