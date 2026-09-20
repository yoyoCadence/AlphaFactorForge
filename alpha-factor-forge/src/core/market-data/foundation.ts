// P06 — market foundation contract (`market-foundation-v1`, TypeScript side).
//
// `src-tauri/src/discovery_core/market_foundation.rs` is its exact Rust
// mirror; the two are held together by
// fixtures/rs-core/market-foundation-v1.json. Shapes and invariants:
// docs/market-foundation-v1.md. Upstream contract: docs/market-contract.md
// (`market-instrument-v1` / `market-provenance-v1` / `market-snapshot-v1`).
//
// This module answers four questions, and only these four:
//
//   1. Is this instrument id well formed, and may two series be combined?
//   2. Which unit are these raw timestamps in, and do they change mid-batch?
//   3. Which bars does the calendar say SHOULD exist in a range?
//   4. Which of them are missing, duplicated, unaligned, unexpected, or not
//      closed yet — and what is the actionable next step?
//
// It deliberately does NOT convert timestamps between units, repair series,
// drop bars, or decide qualification. A unit change is evidence that must be
// adjudicated (docs/market-contract.md §2), never something to silently
// normalise away.
//
// Relationship to `market-data-quality-v1` (./quality): that contract is the
// per-candle plausibility gate at admission and is unchanged. This one sits
// ABOVE it and is about a series' relationship to its instrument, calendar,
// and sources. The plausibility window is imported rather than restated so
// the two cannot drift.
//
// Pure module (src/core/* purity rule): no React, no DOM, no IO, and no
// zh-TW user-facing copy — callers own the wording they show a user.

import { MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE, MIN_MARKET_TIMESTAMP_MS } from './quality';

export const MARKET_FOUNDATION_VERSION = 'market-foundation-v1';
export const MARKET_INSTRUMENT_VERSION = 'market-instrument-v1';
export const SESSION_CALENDAR_VERSION = 'session-calendar-v1';
export const MARKET_PROVENANCE_VERSION = 'market-provenance-v1';
export const MARKET_SNAPSHOT_VERSION = 'market-snapshot-v1';

// ---------------------------------------------------------------- intervals

/**
 * Bar cadence in milliseconds (`market-interval-v1`). Strict on purpose: an
 * unknown interval is an error, never a silent daily fallback.
 *
 * This is NOT `barsPerYear` (src/services/backtestRunner.ts), which keeps its
 * documented legacy behaviour including the unknown-interval fallback and
 * `1d` -> 365. Cadence and annualisation are different questions; P08 versions
 * the annualisation one for ETFs without touching either of them here.
 */
export const INTERVAL_MS: Readonly<Record<string, number>> = Object.freeze({
  '1m': 60_000,
  '3m': 180_000,
  '5m': 300_000,
  '15m': 900_000,
  '1h': 3_600_000,
  '4h': 14_400_000,
  '1d': 86_400_000,
});

/** The cadence of `interval`, or null when this contract does not know it. */
export function intervalMs(interval: string): number | null {
  return Object.prototype.hasOwnProperty.call(INTERVAL_MS, interval)
    ? INTERVAL_MS[interval]
    : null;
}

// -------------------------------------------------------------- instruments

export const MARKETS = ['crypto', 'us-etf', 'tw-etf'] as const;
export type Market = (typeof MARKETS)[number];

export const ASSET_TYPES = ['spot-crypto', 'etf'] as const;
export type AssetType = (typeof ASSET_TYPES)[number];

export const PRICE_BASES = ['raw', 'adjusted-split', 'adjusted-total-return'] as const;
export type PriceBasis = (typeof PRICE_BASES)[number];

/** Stable rule ids for instrument-id parsing, evaluated in this order. */
export const INSTRUMENT_ID_RULE_IDS = [
  'instrument_id_shape',
  'unknown_market',
  'venue_not_normalized',
  'symbol_not_supported',
] as const;
export type InstrumentIdRule = (typeof INSTRUMENT_ID_RULE_IDS)[number];

export interface InstrumentId {
  market: Market;
  venue: string;
  symbol: string;
}

const VENUE_PATTERN = /^[a-z0-9-]+$/;
const SYMBOL_PATTERN = /^[A-Za-z0-9.-]+$/;

/**
 * Parse `<market>:<venue>:<symbol>` (docs/market-contract.md §1).
 *
 * The symbol keeps the venue's own spelling: `USDT` is never renamed `USD`,
 * and case is never folded, because a renamed quote currency is exactly the
 * kind of silent merge this contract exists to prevent.
 */
export function parseInstrumentId(
  value: string,
): { id: InstrumentId; rule: null } | { id: null; rule: InstrumentIdRule } {
  const parts = value.split(':');
  if (parts.length !== 3 || parts.some((part) => part.length === 0)) {
    return { id: null, rule: 'instrument_id_shape' };
  }
  const [market, venue, symbol] = parts;
  if (!(MARKETS as readonly string[]).includes(market)) {
    return { id: null, rule: 'unknown_market' };
  }
  if (!VENUE_PATTERN.test(venue)) return { id: null, rule: 'venue_not_normalized' };
  if (!SYMBOL_PATTERN.test(symbol)) return { id: null, rule: 'symbol_not_supported' };
  return { id: { market: market as Market, venue, symbol }, rule: null };
}

export function formatInstrumentId(id: InstrumentId): string {
  return `${id.market}:${id.venue}:${id.symbol}`;
}

// ------------------------------------------------------- source combination

export type SeriesRole = 'primary' | 'comparison';

/** What a series is, for the question "may these two be one series?". */
export interface SeriesIdentity {
  instrumentId: string;
  quote: string;
  interval: string;
  /** The retrieval source, e.g. `binance-archive`, `tiingo`, `csv`. */
  source: string;
  role: SeriesRole;
  priceBasis: PriceBasis;
}

/** Stable conflict codes, reported sorted. */
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
export type SeriesConflictCode = (typeof SERIES_CONFLICT_CODES)[number];

/**
 * The publisher a source id belongs to: `<origin>[-<endpoint>]`.
 *
 * `binance-archive` and `binance-rest` are two endpoints of ONE exchange
 * publishing its own market; `coinbase-rest` is a different exchange. The
 * distinction is what lets the contract say "the archive's gap may be
 * filled from the same exchange's REST endpoint" (docs/market-contract.md
 * §4, plan §5 P07) while still refusing to splice two exchanges together.
 */
export function sourceOrigin(source: string): string {
  const separator = source.indexOf('-');
  return separator < 0 ? source : source.slice(0, separator);
}

/** Whether two source ids are the same publisher. Empty is never a match. */
export function sourcesShareOrigin(a: string, b: string): boolean {
  const origin = sourceOrigin(a);
  return origin.length > 0 && origin === sourceOrigin(b);
}

/**
 * Why two series may not be combined into one tradable series — empty means
 * they may (docs/market-contract.md §6: never splice different venues or
 * quote currencies together, and a second source produces comparison
 * evidence rather than backfill).
 *
 * `source_mismatch` is reported for any two different source ids, including
 * two endpoints of the same publisher. It is a fact about the retrieval,
 * and whoever composes a series decides what to do with it: the snapshot
 * builder tolerates it when `sourcesShareOrigin` holds, and never otherwise.
 */
export function seriesConflicts(a: SeriesIdentity, b: SeriesIdentity): SeriesConflictCode[] {
  const found = new Set<SeriesConflictCode>();
  const left = parseInstrumentId(a.instrumentId);
  const right = parseInstrumentId(b.instrumentId);
  if (!left.id || !right.id) {
    found.add('invalid_instrument_id');
  } else {
    if (left.id.market !== right.id.market) found.add('market_mismatch');
    if (left.id.venue !== right.id.venue) found.add('venue_mismatch');
    if (left.id.symbol !== right.id.symbol) found.add('symbol_mismatch');
  }
  if (a.quote !== b.quote) found.add('quote_mismatch');
  if (a.interval !== b.interval) found.add('interval_mismatch');
  if (a.source !== b.source) found.add('source_mismatch');
  if (a.priceBasis !== b.priceBasis) found.add('price_basis_mismatch');
  if (a.role === 'comparison' || b.role === 'comparison') {
    found.add('comparison_role_not_combinable');
  }
  return [...found].sort();
}

// --------------------------------------------------------------- time units

export const TIME_UNITS = ['seconds', 'milliseconds', 'microseconds', 'nanoseconds'] as const;
export type TimeUnit = (typeof TIME_UNITS)[number];

export const TIME_UNIT_ISSUE_IDS = [
  'no_timestamps',
  'time_unit_mixed',
  'time_unit_unknown',
] as const;
export type TimeUnitIssue = (typeof TIME_UNIT_ISSUE_IDS)[number];

export interface TimeUnitVerdict {
  /** The single unit every timestamp is in, or null when there is none. */
  unit: TimeUnit | null;
  issue: TimeUnitIssue | null;
  /** The offending timestamp's index, or null. */
  index: number | null;
}

/**
 * The band of each unit, derived from the ONE plausibility window in
 * `market-data-quality-v1` so a change there moves every band with it.
 */
const UNIT_SCALE: Readonly<Record<TimeUnit, number>> = Object.freeze({
  seconds: 1 / 1000,
  milliseconds: 1,
  microseconds: 1000,
  nanoseconds: 1_000_000,
});

function unitOf(timestamp: number): TimeUnit | null {
  if (!Number.isFinite(timestamp)) return null;
  for (const unit of TIME_UNITS) {
    const scale = UNIT_SCALE[unit];
    if (
      timestamp >= MIN_MARKET_TIMESTAMP_MS * scale &&
      timestamp < MAX_MARKET_TIMESTAMP_MS_EXCLUSIVE * scale
    ) {
      return unit;
    }
  }
  return null;
}

/**
 * Which unit a raw batch of timestamps is in (docs/market-contract.md §4: the
 * Binance archive changed from milliseconds to microseconds, so a batch that
 * changes unit half way through must be visible rather than averaged away).
 *
 * Detection only. Nothing here converts: a `time_unit_mixed` batch is
 * evidence for adjudication, and a converted-by-guess series is exactly the
 * silent repair this contract forbids.
 */
export function detectTimeUnit(timestamps: readonly number[]): TimeUnitVerdict {
  if (timestamps.length === 0) return { unit: null, issue: 'no_timestamps', index: null };
  const first = unitOf(timestamps[0]);
  if (first === null) return { unit: null, issue: 'time_unit_unknown', index: 0 };
  for (let index = 1; index < timestamps.length; index++) {
    const unit = unitOf(timestamps[index]);
    if (unit === null) return { unit: null, issue: 'time_unit_unknown', index };
    if (unit !== first) return { unit: null, issue: 'time_unit_mixed', index };
  }
  return { unit: first, issue: null, index: null };
}

// ---------------------------------------------------------------- calendars

export const CALENDAR_KINDS = ['continuous', 'trading-days'] as const;
export type CalendarKind = (typeof CALENDAR_KINDS)[number];

export interface SessionCalendar {
  /** Carries its own version, e.g. `crypto-24x7-v1`, `nyse-v1`, `twse-v1`. */
  calendarId: string;
  kind: CalendarKind;
  /** IANA zone. Recorded for session semantics (P08); v1 range maths is UTC. */
  timezone: string;
  /** ISO weekdays (Mon = 1 ... Sun = 7), ascending and unique. */
  tradingWeekdays: number[];
  /** Non-trading dates, `YYYY-MM-DD`, ascending and unique. */
  holidays: string[];
  /** Short sessions, `YYYY-MM-DD`. Recorded in v1, not used by range maths. */
  earlyCloses: string[];
}

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
export type CalendarRule = (typeof CALENDAR_RULE_IDS)[number];

const CALENDAR_ID_PATTERN = /^[a-z0-9]+(-[a-z0-9]+)*-v[0-9]+$/;
const DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/;

const MS_PER_DAY = 86_400_000;

/** `YYYY-MM-DD` -> UTC midnight in epoch ms, or null when it is not a date. */
export function utcDateToMs(date: string): number | null {
  if (!DATE_PATTERN.test(date)) return null;
  const year = Number(date.slice(0, 4));
  const month = Number(date.slice(5, 7));
  const day = Number(date.slice(8, 10));
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const ms = Date.UTC(year, month - 1, day);
  if (!Number.isFinite(ms)) return null;
  // Rejects 2026-02-30 and friends: the round trip only survives real dates.
  const back = new Date(ms);
  if (
    back.getUTCFullYear() !== year ||
    back.getUTCMonth() + 1 !== month ||
    back.getUTCDate() !== day
  ) {
    return null;
  }
  return ms;
}

/** `YYYY-MM-DD` of a UTC midnight timestamp. */
export function utcDateOf(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10);
}

/** ISO weekday (Mon = 1 ... Sun = 7) of a UTC timestamp. */
export function isoWeekdayOfUtcMs(ms: number): number {
  const day = Math.floor(ms / MS_PER_DAY);
  // 1970-01-01 was a Thursday (ISO 4).
  return ((((day + 3) % 7) + 7) % 7) + 1;
}

/** The first calendar defect, or null. */
export function validateCalendar(calendar: SessionCalendar): CalendarRule | null {
  if (!CALENDAR_ID_PATTERN.test(calendar.calendarId)) return 'calendar_id_shape';
  if (!(CALENDAR_KINDS as readonly string[]).includes(calendar.kind)) return 'unknown_kind';
  if (calendar.timezone.trim().length === 0) return 'empty_timezone';
  if (calendar.kind === 'continuous') {
    if (calendar.tradingWeekdays.length > 0 || calendar.holidays.length > 0) {
      return 'continuous_carries_trading_days';
    }
  } else if (calendar.tradingWeekdays.length === 0) {
    return 'trading_days_without_weekdays';
  }
  for (const weekday of calendar.tradingWeekdays) {
    if (!Number.isInteger(weekday) || weekday < 1 || weekday > 7) return 'weekday_out_of_range';
  }
  for (let index = 1; index < calendar.tradingWeekdays.length; index++) {
    if (calendar.tradingWeekdays[index] <= calendar.tradingWeekdays[index - 1]) {
      return 'weekdays_not_sorted';
    }
  }
  for (const dates of [calendar.holidays, calendar.earlyCloses]) {
    for (const date of dates) {
      if (utcDateToMs(date) === null) return 'invalid_date';
    }
    for (let index = 1; index < dates.length; index++) {
      if (dates[index] <= dates[index - 1]) return 'dates_not_sorted';
    }
  }
  return null;
}

// ----------------------------------------------------------- expected range

export interface Suspension {
  fromMs: number;
  toMsExclusive: number;
}

export interface ExpectedRangeRequest {
  calendar: SessionCalendar;
  interval: string;
  /** Inclusive. */
  fromMs: number;
  /** Exclusive. */
  toMsExclusive: number;
  /** Inclusive listing time; null means "no recorded listing date". */
  listedFromMs?: number | null;
  /** Exclusive delisting time; null means "still listed". */
  delistedAtMs?: number | null;
  /** Halted periods; bars inside them are not expected. */
  suspensions?: readonly Suspension[];
}

export const EXPECTED_RANGE_ISSUE_IDS = [
  'unknown_interval',
  'invalid_calendar',
  'unsupported_interval_for_calendar',
  'invalid_range',
  'invalid_suspension',
  'range_too_large',
] as const;
export type ExpectedRangeIssue = (typeof EXPECTED_RANGE_ISSUE_IDS)[number];

/** A refused request returns its issue and no timestamps; never a guess. */
export interface ExpectedRangeResult {
  timestamps: number[];
  issue: ExpectedRangeIssue | null;
}

/** Defence against an unbounded range: ~114 years of hourly bars. */
export const MAX_EXPECTED_BARS = 1_000_000;

/**
 * Which bar starts the calendar says should exist in `[fromMs, toMsExclusive)`
 * (docs/market-contract.md §6 step 1). Weekends, holidays, halts, and
 * pre-listing/post-delisting time are NOT gaps, which is the whole point of
 * deriving the expectation from a versioned calendar instead of from the data.
 *
 * v1 supports intraday cadences on a `continuous` calendar and `1d` on a
 * `trading-days` calendar. A daily bar's timestamp is UTC midnight of the
 * trading date; intraday ETF sessions (and therefore local-time session
 * boundaries) are P08, which is why no timezone database is consulted here.
 */
export function expectedBarStarts(request: ExpectedRangeRequest): ExpectedRangeResult {
  const none = (issue: ExpectedRangeIssue): ExpectedRangeResult => ({ timestamps: [], issue });
  const cadence = intervalMs(request.interval);
  if (cadence === null) return none('unknown_interval');
  if (validateCalendar(request.calendar) !== null) return none('invalid_calendar');
  if (request.calendar.kind === 'trading-days' && cadence !== MS_PER_DAY) {
    return none('unsupported_interval_for_calendar');
  }
  if (
    !Number.isSafeInteger(request.fromMs) ||
    !Number.isSafeInteger(request.toMsExclusive) ||
    request.toMsExclusive <= request.fromMs
  ) {
    return none('invalid_range');
  }
  const suspensions = request.suspensions ?? [];
  for (const suspension of suspensions) {
    if (
      !Number.isSafeInteger(suspension.fromMs) ||
      !Number.isSafeInteger(suspension.toMsExclusive) ||
      suspension.toMsExclusive <= suspension.fromMs
    ) {
      return none('invalid_suspension');
    }
  }

  const listedFrom = request.listedFromMs ?? null;
  const delistedAt = request.delistedAtMs ?? null;
  const start = listedFrom === null ? request.fromMs : Math.max(request.fromMs, listedFrom);
  const end =
    delistedAt === null ? request.toMsExclusive : Math.min(request.toMsExclusive, delistedAt);
  if (end <= start) return { timestamps: [], issue: null };

  const suspended = (timestamp: number): boolean =>
    suspensions.some((period) => timestamp >= period.fromMs && timestamp < period.toMsExclusive);

  const timestamps: number[] = [];
  const first = Math.ceil(start / cadence) * cadence;
  for (let timestamp = first; timestamp < end; timestamp += cadence) {
    if (timestamps.length >= MAX_EXPECTED_BARS) return none('range_too_large');
    if (request.calendar.kind === 'trading-days') {
      if (!request.calendar.tradingWeekdays.includes(isoWeekdayOfUtcMs(timestamp))) continue;
      if (request.calendar.holidays.includes(utcDateOf(timestamp))) continue;
    }
    if (suspended(timestamp)) continue;
    timestamps.push(timestamp);
  }
  return { timestamps, issue: null };
}

// ----------------------------------------------------------- coverage audit

export const SEVERITIES = ['blocking', 'degraded', 'info'] as const;
export type Severity = (typeof SEVERITIES)[number];

export const COVERAGE_CODES = [
  'out_of_order',
  'duplicate_timestamp',
  'unaligned_timestamp',
  'unexpected_bar',
  'missing_bar',
  'unclosed_bar',
] as const;
export type CoverageCode = (typeof COVERAGE_CODES)[number];

/** What a user can actually do about an event; UI owns the wording. */
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
export type ActionCode = (typeof ACTION_CODES)[number];

export interface CoverageEvent {
  code: CoverageCode;
  severity: Severity;
  /** Inclusive first offending bar start. */
  rangeStart: number;
  /** Inclusive last offending bar start (equal to rangeStart for singletons). */
  rangeEnd: number;
  /** Bars covered by this event (a merged gap counts every missing bar). */
  count: number;
  action: ActionCode;
}

export interface CoverageRequest {
  interval: string;
  /** Ascending and unique — `expectedBarStarts` output. */
  expected: readonly number[];
  /** As retrieved or stored, in any order. */
  observed: readonly number[];
  /** The data cut: a bar closing after this is not due yet. */
  asOfMs: number;
}

export interface CoverageReport {
  version: string;
  events: CoverageEvent[];
  blocking: boolean;
  expectedCount: number;
  /** Expected bars whose close is after `asOfMs`; not due, so never missing. */
  notDueCount: number;
  observedCount: number;
  matchedCount: number;
  issue: ExpectedRangeIssue | null;
}

const COVERAGE_ACTIONS: Readonly<Record<CoverageCode, ActionCode>> = Object.freeze({
  out_of_order: 'resort_source',
  duplicate_timestamp: 'deduplicate_source',
  unaligned_timestamp: 'verify_time_unit',
  unexpected_bar: 'review_calendar',
  missing_bar: 'refetch_range',
  unclosed_bar: 'wait_for_close',
});

function event(
  code: CoverageCode,
  rangeStart: number,
  rangeEnd: number,
  count: number,
): CoverageEvent {
  // Every coverage defect blocks: a series is admitted whole or not at all,
  // exactly as `market-data-quality-v1` admits a dataset whole.
  return { code, severity: 'blocking', rangeStart, rangeEnd, count, action: COVERAGE_ACTIONS[code] };
}

/**
 * Compare what the calendar expects with what a source actually delivered
 * (docs/market-contract.md §6 step 2). Nothing is repaired, dropped, or
 * forward-filled: the report is evidence, and a blocking report keeps the
 * series out of a snapshot until a human or an adapter resolves it.
 */
export function auditCoverage(request: CoverageRequest): CoverageReport {
  const cadence = intervalMs(request.interval);
  const base: CoverageReport = {
    version: MARKET_FOUNDATION_VERSION,
    events: [],
    blocking: false,
    expectedCount: request.expected.length,
    notDueCount: 0,
    observedCount: request.observed.length,
    matchedCount: 0,
    issue: null,
  };
  if (cadence === null) return { ...base, issue: 'unknown_interval', blocking: true };

  const events: CoverageEvent[] = [];
  const observed = [...request.observed];
  // Non-decreasing is "ordered": a repeated timestamp is a duplicate, which
  // has its own code, so it must not also be reported as bad ordering.
  let ordered = true;
  for (let index = 1; index < observed.length; index++) {
    if (observed[index] < observed[index - 1]) {
      ordered = false;
      break;
    }
  }
  if (!ordered) {
    observed.sort((a, b) => a - b);
    events.push(event('out_of_order', observed[0], observed[observed.length - 1], observed.length));
  }

  const expectedSet = new Set(request.expected);
  const seen = new Map<number, number>();
  for (const timestamp of observed) {
    seen.set(timestamp, (seen.get(timestamp) ?? 0) + 1);
  }

  let matched = 0;
  for (const [timestamp, count] of [...seen.entries()].sort((a, b) => a[0] - b[0])) {
    if (count > 1) events.push(event('duplicate_timestamp', timestamp, timestamp, count));
    if (((timestamp % cadence) + cadence) % cadence !== 0) {
      events.push(event('unaligned_timestamp', timestamp, timestamp, 1));
      continue;
    }
    if (!expectedSet.has(timestamp)) {
      events.push(event('unexpected_bar', timestamp, timestamp, 1));
      continue;
    }
    matched += 1;
    if (timestamp + cadence > request.asOfMs) {
      events.push(event('unclosed_bar', timestamp, timestamp, 1));
    }
  }

  // Merge contiguous missing bars (contiguous in the EXPECTED grid, so a
  // weekend inside a gap does not split the report into two rows).
  let notDue = 0;
  let gapStart: number | null = null;
  let gapEnd = 0;
  let gapCount = 0;
  const flush = (): void => {
    if (gapStart !== null) events.push(event('missing_bar', gapStart, gapEnd, gapCount));
    gapStart = null;
    gapCount = 0;
  };
  for (const timestamp of request.expected) {
    if (timestamp + cadence > request.asOfMs) {
      notDue += 1;
      flush();
      continue;
    }
    if (seen.has(timestamp)) {
      flush();
      continue;
    }
    if (gapStart === null) gapStart = timestamp;
    gapEnd = timestamp;
    gapCount += 1;
  }
  flush();

  // Deterministic in both runtimes: by first offending bar, then by the
  // declared code order. (Code RANK, not the code text — string collation is
  // not guaranteed to agree between JavaScript and Rust.)
  events.sort(
    (a, b) =>
      a.rangeStart - b.rangeStart ||
      COVERAGE_CODES.indexOf(a.code) - COVERAGE_CODES.indexOf(b.code),
  );
  return {
    ...base,
    events,
    blocking: events.some((entry) => entry.severity === 'blocking'),
    notDueCount: notDue,
    matchedCount: matched,
  };
}
