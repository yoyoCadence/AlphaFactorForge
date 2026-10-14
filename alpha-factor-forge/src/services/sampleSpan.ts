// METRIC-DISPLAY-001 — how much calendar time a result covers.
//
// CAGR, Sharpe, Sortino and Calmar are annualized. On a sample much shorter
// than a year that is an extrapolation: 600 hourly bars with a 14% net return
// read as a 620% CAGR. Nothing here changes a metric; it only tells the reader
// when the figures beside it were scaled up from a short sample. Pure.

const MS_PER_DAY = 86_400_000;

/** Display metadata only: the same tested-bar count and annualization basis
 *  the completed result used. Calendar timestamp gaps cannot establish it. */
export interface AnnualizationSample {
  totalBars: number;
  barsPerYear: number;
}

export interface AnnualizationColumn {
  label: string;
  sample: AnnualizationSample | null;
}

export interface MetricsSamples {
  full: AnnualizationSample;
  inSample?: AnnualizationSample;
  outSample?: AnnualizationSample;
}

/** Shares computeMetrics' years = totalBars / barsPerYear definition. Invalid
 *  or absent evidence stays unknown; an exact year never gets the warning. */
export function annualizationYears(sample: AnnualizationSample | null | undefined): number | null {
  if (!sample || !Number.isSafeInteger(sample.totalBars) || sample.totalBars < 1
    || !Number.isFinite(sample.barsPerYear) || sample.barsPerYear <= 0) return null;
  const years = sample.totalBars / sample.barsPerYear;
  return Number.isFinite(years) && years > 0 ? years : null;
}

/** Whole-calendar-day equivalent of the engine's annualization basis. This is
 *  not the first-to-last timestamp span (which excludes the final bar). */
export function shortAnnualizationDays(sample: AnnualizationSample | null | undefined): number | null {
  const years = annualizationYears(sample);
  return years != null && years < 1 ? years * SHORT_SAMPLE_DAYS : null;
}

export function metricsSampleColumns(samples: MetricsSamples | undefined, holdout: boolean): AnnualizationColumn[] {
  return [
    { label: '全期', sample: samples?.full ?? null },
    ...(holdout ? [
      { label: '樣本內', sample: samples?.inSample ?? null },
      { label: '樣本外', sample: samples?.outSample ?? null },
    ] : []),
  ];
}

/** Below this many days, annualized figures get the short-sample note. */
export const SHORT_SAMPLE_DAYS = 365;

/** Days between two epoch-ms bounds (fractional), or null when the bounds are
 *  missing, non-finite, or reversed. */
export function spanDays(startMs: number | null | undefined, endMs: number | null | undefined): number | null {
  if (typeof startMs !== 'number' || typeof endMs !== 'number') return null;
  if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs < startMs) return null;
  return (endMs - startMs) / MS_PER_DAY;
}

/** The span in days when it is shorter than a year, else null. A span that
 *  cannot be computed is null too: no claim is made about it either way. */
export function shortSampleDays(startMs: number | null | undefined, endMs: number | null | undefined): number | null {
  const days = spanDays(startMs, endMs);
  return days != null && days < SHORT_SAMPLE_DAYS ? days : null;
}

/** The whole-day figure shown to the reader; a sub-day sample still says 1. */
export function displayDays(days: number): number {
  return Math.max(1, Math.round(days));
}
