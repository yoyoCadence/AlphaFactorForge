import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/rs-core/market-foundation-v1.json';
import { sha256Hex } from '../core/hashing';
import {
  ACTION_CODES,
  CALENDAR_RULE_IDS,
  COVERAGE_CODES,
  EXPECTED_RANGE_ISSUE_IDS,
  INSTRUMENT_ID_RULE_IDS,
  INTERVAL_MS,
  MARKET_FOUNDATION_VERSION,
  MAX_EXPECTED_BARS,
  SERIES_CONFLICT_CODES,
  TIME_UNIT_ISSUE_IDS,
  TIME_UNITS,
  auditCoverage,
  detectTimeUnit,
  expectedBarStarts,
  intervalMs,
  parseInstrumentId,
  seriesConflicts,
  validateCalendar,
  type SeriesIdentity,
  type SessionCalendar,
} from '../core/market-data/foundation';
import { canonicalizeFixtureSource, FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import { decodeFixtureNumber, type FixtureNumber } from './marketDataQualityFixture';
import generatorSource from './marketFoundationFixture.ts?raw';
import { buildMarketFoundationParityFixture } from './marketFoundationFixture';

async function hashSource(source: string): Promise<string> {
  return `sha256:${await sha256Hex(canonicalizeFixtureSource(source))}`;
}

const calendars = new Map<string, SessionCalendar>(
  (fixture.calendars as SessionCalendar[]).map((calendar) => [calendar.calendarId, calendar]),
);

const cases = fixture.cases as unknown as {
  instrumentIds: {
    id: string;
    value: string;
    expected: { parsed: { market: string; venue: string; symbol: string } | null; rule: string | null };
  }[];
  timeUnits: {
    id: string;
    timestamps: FixtureNumber[];
    expected: { unit: string | null; issue: string | null; index: number | null };
  }[];
  calendarValidation: {
    id: string;
    calendar: SessionCalendar;
    expected: { rule: string | null };
  }[];
  expectedRanges: {
    id: string;
    calendarId: string;
    interval: string;
    fromMs: number;
    toMsExclusive: number;
    listedFromMs: number | null;
    delistedAtMs: number | null;
    suspensions: { fromMs: number; toMsExclusive: number }[];
    expected: { timestamps: number[]; issue: string | null };
  }[];
  coverage: {
    id: string;
    interval: string;
    expected: number[];
    observed: number[];
    asOfMs: number;
    expectedReport: {
      events: unknown[];
      blocking: boolean;
      expectedCount: number;
      notDueCount: number;
      observedCount: number;
      matchedCount: number;
      issue: string | null;
    };
  }[];
  seriesCombination: {
    id: string;
    a: SeriesIdentity;
    b: SeriesIdentity;
    expected: { conflicts: string[] };
  }[];
};

describe('P06 market-foundation parity fixture', () => {
  it('is exactly reproducible from the canonical current generator source', async () => {
    const regenerated = buildMarketFoundationParityFixture({
      generator: await hashSource(generatorSource),
    });
    expect(regenerated).toEqual(fixture);
  });

  it('locks the envelope, the constants, and every inventory', () => {
    expect(fixture.schemaVersion).toBe('rs-core-parity-fixture-v1');
    expect(fixture.fixtureVersion).toBe('market-foundation-parity-v1');
    expect(fixture.contracts).toEqual({
      marketFoundation: 'market-foundation-v1',
      marketInstrument: 'market-instrument-v1',
      sessionCalendar: 'session-calendar-v1',
      marketDataQuality: 'market-data-quality-v1',
    });
    expect(fixture.generator.sourceHashEncoding).toBe(FIXTURE_SOURCE_HASH_ENCODING);
    expect(fixture.numericEncoding).toEqual({
      specialInputNumbers: 'explicit-numeric-status-v1',
      expectedNumericPolicy: 'exact-v1',
    });
    expect(fixture.constants).toEqual({
      minTimestampMs: 946_684_800_000,
      maxTimestampMsExclusive: 4_102_444_800_000,
      maxExpectedBars: MAX_EXPECTED_BARS,
    });
    expect(fixture.inventories.instrumentIdRuleIds).toEqual([...INSTRUMENT_ID_RULE_IDS]);
    expect(fixture.inventories.calendarRuleIds).toEqual([...CALENDAR_RULE_IDS]);
    expect(fixture.inventories.timeUnitIds).toEqual([...TIME_UNITS]);
    expect(fixture.inventories.timeUnitIssueIds).toEqual([...TIME_UNIT_ISSUE_IDS]);
    expect(fixture.inventories.expectedRangeIssueIds).toEqual([...EXPECTED_RANGE_ISSUE_IDS]);
    expect(fixture.inventories.coverageCodes).toEqual([...COVERAGE_CODES]);
    expect(fixture.inventories.actionCodes).toEqual([...ACTION_CODES]);
    expect(fixture.inventories.seriesConflictCodes).toEqual([...SERIES_CONFLICT_CODES]);
    expect(fixture.inventories.runtimeSpecificCalendarRuleIds).toEqual(['unknown_kind']);
  });

  it('shares one interval table with the implementation', () => {
    const table = Object.fromEntries(
      fixture.intervals.map((entry) => [entry.interval, entry.ms]),
    );
    expect(table).toEqual(INTERVAL_MS);
    for (const entry of fixture.intervals) {
      expect(intervalMs(entry.interval)).toBe(entry.ms);
    }
    expect(MARKET_FOUNDATION_VERSION).toBe(fixture.contracts.marketFoundation);
  });

  it('parses every instrument id exactly as specified', () => {
    for (const entry of cases.instrumentIds) {
      const result = parseInstrumentId(entry.value);
      expect({ parsed: result.id, rule: result.rule }, entry.id).toEqual(entry.expected);
    }
  });

  it('classifies every raw timestamp batch exactly as specified', () => {
    for (const entry of cases.timeUnits) {
      const verdict = detectTimeUnit(entry.timestamps.map(decodeFixtureNumber));
      expect(verdict, entry.id).toEqual(entry.expected);
    }
  });

  it('validates every calendar exactly as specified', () => {
    for (const entry of cases.calendarValidation) {
      expect(validateCalendar(entry.calendar), entry.id).toBe(entry.expected.rule);
    }
  });

  it('derives every expected range exactly as specified', () => {
    for (const entry of cases.expectedRanges) {
      const calendar = calendars.get(entry.calendarId);
      expect(calendar, `${entry.id}: calendar ${entry.calendarId}`).toBeDefined();
      const result = expectedBarStarts({
        calendar: calendar!,
        interval: entry.interval,
        fromMs: entry.fromMs,
        toMsExclusive: entry.toMsExclusive,
        listedFromMs: entry.listedFromMs,
        delistedAtMs: entry.delistedAtMs,
        suspensions: entry.suspensions,
      });
      expect(result, entry.id).toEqual(entry.expected);
    }
  });

  it('audits every coverage case exactly as specified', () => {
    for (const entry of cases.coverage) {
      const report = auditCoverage({
        interval: entry.interval,
        expected: entry.expected,
        observed: entry.observed,
        asOfMs: entry.asOfMs,
      });
      expect(report, entry.id).toEqual({
        version: MARKET_FOUNDATION_VERSION,
        ...entry.expectedReport,
      });
    }
  });

  it('answers every series-combination case exactly as specified', () => {
    for (const entry of cases.seriesCombination) {
      expect(seriesConflicts(entry.a, entry.b), entry.id).toEqual(entry.expected.conflicts);
    }
  });
});
