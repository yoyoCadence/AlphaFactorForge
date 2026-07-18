// Slice 7-1 — pure report / export formatters (no UI, no IO).
//
// Turn a finished backtest into a JSON report and a trades CSV string. These are
// deterministic string builders; ACTUALLY writing a file (a Tauri save dialog /
// fs command, or a browser download) is the next slice (7-2). No React/DOM/IO
// here so the formatting is unit-testable and reusable from either path.

import type { ParamsStrategy } from './strategy';
import type { BacktestResult } from '../core/backtest';
import type { ClosedTrade, Metrics } from '../core/metrics';
import { nonFiniteStatus, type NonFiniteStatus } from './nonFinite';

const APP = 'AlphaFactorForge';

export interface ReportDatasetMeta {
  symbol: string;
  interval: string;
  startTime?: number;
  endTime?: number;
}

export interface ReportInput {
  strategyName?: string;
  strategy: ParamsStrategy;
  dataset: ReportDatasetMeta;
  result: BacktestResult;
  /** Export timestamp (ms). Defaults to Date.now(); pass a fixed value in tests. */
  exportedAt?: number;
}

/** Metrics with every top-level numeric field narrowed to finite-or-null so
 *  the report is JSON-safe (schema 2, METRIC-001). */
export type ReportMetrics = {
  [K in keyof Metrics]: Metrics[K] extends number ? number | null : Metrics[K];
};

export interface ReportJson {
  app: string;
  schema: number;
  exportedAt: string;
  strategyName: string;
  strategy: ParamsStrategy;
  dataset: ReportDatasetMeta;
  metrics: ReportMetrics;
  /** Explicit status for every metric that is null above because it was
   *  non-finite (e.g. Sortino with no downside) — never rely on
   *  JSON.stringify's silent Infinity -> null conversion (METRIC-001). */
  metricsNonFinite: Partial<Record<keyof Metrics, NonFiniteStatus>>;
  tradeCount: number;
  trades: ClosedTrade[];
}

/** Encode metrics for JSON: non-finite numeric fields become null + an
 *  explicit status entry. monthlyReturns are equity ratios and stay finite by
 *  construction, so the record passes through unchanged. */
function encodeMetrics(metrics: Metrics): {
  values: ReportMetrics;
  nonFinite: Partial<Record<keyof Metrics, NonFiniteStatus>>;
} {
  const values: Record<string, unknown> = { ...metrics };
  const nonFinite: Partial<Record<keyof Metrics, NonFiniteStatus>> = {};
  for (const key of Object.keys(metrics) as (keyof Metrics)[]) {
    const v = metrics[key];
    if (typeof v !== 'number') continue;
    const status = nonFiniteStatus(v);
    if (status) {
      nonFinite[key] = status;
      values[key] = null;
    }
  }
  return { values: values as ReportMetrics, nonFinite };
}

/** Build the structured JSON report object. Pure. */
export function buildReport(input: ReportInput): ReportJson {
  const at = input.exportedAt ?? Date.now();
  const { values, nonFinite } = encodeMetrics(input.result.metrics);
  return {
    app: APP,
    schema: 2,
    exportedAt: new Date(at).toISOString(),
    strategyName: input.strategyName?.trim() || '(未命名)',
    strategy: input.strategy,
    dataset: input.dataset,
    metrics: values,
    metricsNonFinite: nonFinite,
    tradeCount: input.result.trades.length,
    trades: input.result.trades,
  };
}

/** Pretty-printed JSON report string. Pure. */
export function reportToJson(input: ReportInput): string {
  return JSON.stringify(buildReport(input), null, 2);
}

// One CSV column per round-trip-trade field, plus human-readable ISO times.
const CSV_COLUMNS: { header: string; get: (t: ClosedTrade) => string | number }[] = [
  { header: 'entry_time', get: (t) => t.entryTime },
  { header: 'entry_time_iso', get: (t) => new Date(t.entryTime).toISOString() },
  { header: 'exit_time', get: (t) => t.exitTime },
  { header: 'exit_time_iso', get: (t) => new Date(t.exitTime).toISOString() },
  { header: 'side', get: (t) => t.side },
  { header: 'entry_price', get: (t) => t.entryPrice },
  { header: 'exit_price', get: (t) => t.exitPrice },
  { header: 'pnl', get: (t) => t.pnl },
  { header: 'pnl_pct', get: (t) => t.pnlPct },
  { header: 'bars', get: (t) => t.bars },
];

/** Quote a CSV cell iff it contains a comma, quote, or newline (RFC-4180-ish). */
function csvCell(v: string | number): string {
  const s = String(v);
  return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/** Round-trip trades as a CSV string: a header row + one row per trade. Pure. */
export function tradesToCsv(trades: ClosedTrade[]): string {
  const header = CSV_COLUMNS.map((c) => c.header).join(',');
  const rows = trades.map((t) => CSV_COLUMNS.map((c) => csvCell(c.get(t))).join(','));
  return [header, ...rows].join('\n');
}

/** A filesystem-safe suggested filename, e.g. AlphaFactorForge_SAMPLE_1h_2026-07-01.json */
export function suggestedFilename(meta: ReportDatasetMeta, ext: 'json' | 'csv', at?: number): string {
  const date = new Date(at ?? Date.now()).toISOString().slice(0, 10);
  const safe = (s: string) => s.replace(/[^A-Za-z0-9_-]+/g, '-');
  return `${APP}_${safe(meta.symbol)}_${safe(meta.interval)}_${date}.${ext}`;
}
