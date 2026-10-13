import { describe, expect, it } from 'vitest';
import { SHORT_SAMPLE_DAYS, displayDays, shortSampleDays, spanDays } from './sampleSpan';

const DAY = 86_400_000;
const START = Date.UTC(2024, 0, 1);

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
