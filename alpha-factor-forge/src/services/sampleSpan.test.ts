import { describe, expect, it } from 'vitest';
import { SHORT_SAMPLE_DAYS, annualizationYears, displayDays, metricsSampleColumns, shortAnnualizationDays, shortSampleDays, spanDays } from './sampleSpan';
import { computeMetrics } from '../core/metrics';

const DAY = 86_400_000;
const START = Date.UTC(2024, 0, 1);

describe('engine annualization samples', () => {
  it.each([[364, 365], [365, 365], [366, 365], [8759, 8760], [8760, 8760], [8761, 8760]])(
    '%i bars against %i bars/year agrees with the metric engine boundary', (totalBars, barsPerYear) => {
      const sample = { totalBars, barsPerYear };
      const result = computeMetrics({ trades: [], equity: [{ time: START, equity: 1 }, { time: START + DAY, equity: 2 }], startEquity: 1, totalBars, barsPerYear });
      if (totalBars < barsPerYear) {
        expect(result.cagr).toBeGreaterThan(result.netReturn);
        expect(shortAnnualizationDays(sample)).not.toBeNull();
      } else {
        expect(result.cagr).toBeLessThanOrEqual(result.netReturn);
        expect(shortAnnualizationDays(sample)).toBeNull();
      }
    },
  );

  it('checks the short out-of-sample column of a long full run', () => {
    const samples = { full: { totalBars: 600, barsPerYear: 365 }, inSample: { totalBars: 420, barsPerYear: 365 }, outSample: { totalBars: 180, barsPerYear: 365 } };
    const columns = metricsSampleColumns(samples, true);
    expect(columns.filter((c) => shortAnnualizationDays(c.sample) != null).map((c) => c.label)).toEqual(['樣本外']);
    expect(shortAnnualizationDays(samples.outSample)).toBe(180);
    expect(metricsSampleColumns(samples, false).map((c) => c.label)).toEqual(['全期']);
  });

  it('keeps invalid or absent evidence unknown', () => {
    for (const sample of [undefined, null, { totalBars: 0, barsPerYear: 365 }, { totalBars: 1.5, barsPerYear: 365 }, { totalBars: 10, barsPerYear: 0 }, { totalBars: 10, barsPerYear: Infinity }, { totalBars: 10, barsPerYear: Number.MIN_VALUE }]) {
      expect(annualizationYears(sample)).toBeNull();
      expect(shortAnnualizationDays(sample)).toBeNull();
    }
    expect(metricsSampleColumns(undefined, true).every((c) => c.sample == null)).toBe(true);
  });
});

describe('spanDays', () => {
  it('measures the days between two epoch-ms bounds', () => {
    expect(spanDays(START, START + 10 * DAY)).toBe(10);
    expect(spanDays(START, START + DAY / 2)).toBe(0.5);
    expect(spanDays(START, START)).toBe(0);
  });

  it('is null for missing, non-finite or reversed bounds', () => {
    expect(spanDays(null, START)).toBeNull();
    expect(spanDays(START, undefined)).toBeNull();
    expect(spanDays(Number.NaN, START)).toBeNull();
    expect(spanDays(START, Number.POSITIVE_INFINITY)).toBeNull();
    expect(spanDays(START + DAY, START)).toBeNull();
  });
});

describe('shortSampleDays', () => {
  it('reports the seeded sample: 600 hourly bars span just under 25 days', () => {
    const lastBar = START + 599 * 3_600_000;
    const days = shortSampleDays(START, lastBar);
    expect(days).toBeCloseTo(24.958, 3);
    expect(displayDays(days as number)).toBe(25);
  });

  it('draws the line at exactly one year', () => {
    expect(SHORT_SAMPLE_DAYS).toBe(365);
    expect(shortSampleDays(START, START + 365 * DAY - 1)).not.toBeNull(); // just under
    expect(shortSampleDays(START, START + 365 * DAY)).toBeNull(); // exactly a year
    expect(shortSampleDays(START, START + 365 * DAY + 1)).toBeNull(); // just over
  });

  it('makes no claim when the span cannot be computed', () => {
    expect(shortSampleDays(null, START)).toBeNull();
    expect(shortSampleDays(START + DAY, START)).toBeNull();
  });
});

describe('displayDays', () => {
  it('rounds to whole days and never shows zero', () => {
    expect(displayDays(0)).toBe(1);
    expect(displayDays(0.2)).toBe(1);
    expect(displayDays(1.4)).toBe(1);
    expect(displayDays(1.5)).toBe(2);
    expect(displayDays(364.4)).toBe(364);
  });
});
