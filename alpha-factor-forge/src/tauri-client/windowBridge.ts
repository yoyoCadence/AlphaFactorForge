// Typed native-window + cross-window event boundary (Slice 8b-1).
// Browser/Vite mode is a no-op; production window creation remains a Rust
// command, and child-window state arrives only through typed Tauri events.

import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { ClosedTrade, Metrics } from '../core/metrics';
import type { Candle } from '../core/backtest';
import type { OverlayToggles } from '../charts/CandleChart';
import type { ParamsStrategy } from '../services/strategy';
import { encodeMetrics, type EncodedMetrics } from '../services/metricsCodec';
import type { NonFiniteStatus } from '../services/nonFinite';
import type { MetricsSamples } from '../services/sampleSpan';

export const MAIN_WINDOW_LABEL = 'main';
export const CHART_WINDOW_LABEL = 'chart-popout-window';
export const METRICS_WINDOW_LABEL = 'metrics-popout-window';
const CHART_READY_EVENT = 'aff:chart-ready';
const CHART_SNAPSHOT_EVENT = 'aff:chart-snapshot';
const CHART_CURSOR_EVENT = 'aff:chart-cursor';
const METRICS_READY_EVENT = 'aff:metrics-ready';
const METRICS_SNAPSHOT_EVENT = 'aff:metrics-snapshot';

export interface ChartWindowSnapshot {
  datasetKey: string;
  title: string;
  candles: Candle[];
  strat: ParamsStrategy;
  show: OverlayToggles;
  trades: ClosedTrade[];
  upto?: number;
}

export interface ChartCursorUpdate {
  upto?: number;
}

export interface MetricsWindowSnapshot {
  title: string;
  full: Metrics;
  inSample?: Metrics;
  outSample?: Metrics;
  /** Optional for older callers; absence is displayed as unknown. */
  samples?: MetricsSamples;
}

export type MetricsWindowUpdate = MetricsWindowSnapshot | null;

/** The metrics snapshot as it crosses the window boundary. The event payload is
 *  JSON, which turns an infinite Sortino / Calmar / profit factor into null, so
 *  the pop-out would show a dash where the main window shows the value
 *  (METRIC-DISPLAY-001). Each column travels through the shared metrics codec
 *  and is restored on arrival. */
export interface MetricsWindowWire {
  title: string;
  full: EncodedMetrics;
  inSample?: EncodedMetrics;
  outSample?: EncodedMetrics;
  samples?: MetricsSamples;
}

const cloneSamples = (samples: MetricsSamples): MetricsSamples => ({
  full: { ...samples.full },
  ...(samples.inSample ? { inSample: { ...samples.inSample } } : {}),
  ...(samples.outSample ? { outSample: { ...samples.outSample } } : {}),
});

const NON_FINITE_VALUE: Record<NonFiniteStatus, number> = {
  positive_infinity: Number.POSITIVE_INFINITY,
  negative_infinity: Number.NEGATIVE_INFINITY,
  nan: Number.NaN,
};

/** The inverse of the shared `encodeMetrics`: every field with a status gets
 *  its non-finite number back; a null without a status stays null. It lives
 *  here, not in `metricsCodec.ts`, because that file is a pinned reference
 *  source of the Gate/Score parity fixture and this is a display concern. */
function decodeMetrics(encoded: EncodedMetrics): Metrics {
  const values: Record<string, unknown> = {
    ...encoded.values,
    monthlyReturns: { ...encoded.values.monthlyReturns },
  };
  for (const key of Object.keys(encoded.nonFinite) as (keyof Metrics)[]) {
    const status = encoded.nonFinite[key];
    if (status) values[key] = NON_FINITE_VALUE[status];
  }
  return values as unknown as Metrics;
}

export function encodeMetricsSnapshot(snapshot: MetricsWindowSnapshot): MetricsWindowWire {
  return {
    title: snapshot.title,
    full: encodeMetrics(snapshot.full),
    ...(snapshot.samples ? { samples: cloneSamples(snapshot.samples) } : {}),
    ...(snapshot.inSample ? { inSample: encodeMetrics(snapshot.inSample) } : {}),
    ...(snapshot.outSample ? { outSample: encodeMetrics(snapshot.outSample) } : {}),
  };
}

export function decodeMetricsSnapshot(wire: MetricsWindowWire): MetricsWindowSnapshot {
  return {
    title: wire.title,
    full: decodeMetrics(wire.full),
    ...(wire.samples ? { samples: cloneSamples(wire.samples) } : {}),
    ...(wire.inSample ? { inSample: decodeMetrics(wire.inSample) } : {}),
    ...(wire.outSample ? { outSample: decodeMetrics(wire.outSample) } : {}),
  };
}

/** Preserve the large candle array when only strategy/overlay/replay state was
 *  re-emitted for the same dataset. This also prevents CandleChart from treating
 *  every event as a dataset change and resetting its local zoom/pan window. */
export function mergeChartSnapshot(
  current: ChartWindowSnapshot | null,
  incoming: ChartWindowSnapshot,
): ChartWindowSnapshot {
  if (!current || current.datasetKey !== incoming.datasetKey) return incoming;
  return { ...incoming, candles: current.candles };
}

const noopUnlisten = (): UnlistenFn => () => undefined;

export const popoutWindows = {
  isAvailable: () => isTauri(),
  openChart: async (): Promise<void> => {
    if (!isTauri()) return;
    await invoke<void>('open_popout_window', { kind: 'chart' });
  },
  publishChart: async (snapshot: ChartWindowSnapshot): Promise<void> => {
    if (!isTauri()) return;
    await emitTo(CHART_WINDOW_LABEL, CHART_SNAPSHOT_EVENT, snapshot);
  },
  publishChartCursor: async (cursor: ChartCursorUpdate): Promise<void> => {
    if (!isTauri()) return;
    await emitTo(CHART_WINDOW_LABEL, CHART_CURSOR_EVENT, cursor);
  },
  onChartReady: async (handler: () => void): Promise<UnlistenFn> => {
    if (!isTauri()) return noopUnlisten();
    return listen(CHART_READY_EVENT, handler);
  },
  onChartSnapshot: async (handler: (snapshot: ChartWindowSnapshot) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return noopUnlisten();
    return listen<ChartWindowSnapshot>(CHART_SNAPSHOT_EVENT, (event) => handler(event.payload));
  },
  onChartCursor: async (handler: (cursor: ChartCursorUpdate) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return noopUnlisten();
    return listen<ChartCursorUpdate>(CHART_CURSOR_EVENT, (event) => handler(event.payload));
  },
  signalChartReady: async (): Promise<void> => {
    if (!isTauri()) return;
    await emitTo(MAIN_WINDOW_LABEL, CHART_READY_EVENT);
  },
  openMetrics: async (): Promise<void> => {
    if (!isTauri()) return;
    await invoke<void>('open_popout_window', { kind: 'metrics' });
  },
  publishMetrics: async (snapshot: MetricsWindowUpdate): Promise<void> => {
    if (!isTauri()) return;
    await emitTo(METRICS_WINDOW_LABEL, METRICS_SNAPSHOT_EVENT, snapshot == null ? null : encodeMetricsSnapshot(snapshot));
  },
  onMetricsReady: async (handler: () => void): Promise<UnlistenFn> => {
    if (!isTauri()) return noopUnlisten();
    return listen(METRICS_READY_EVENT, handler);
  },
  onMetricsSnapshot: async (handler: (snapshot: MetricsWindowUpdate) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return noopUnlisten();
    return listen<MetricsWindowWire | null>(METRICS_SNAPSHOT_EVENT, (event) =>
      handler(event.payload == null ? null : decodeMetricsSnapshot(event.payload)));
  },
  signalMetricsReady: async (): Promise<void> => {
    if (!isTauri()) return;
    await emitTo(MAIN_WINDOW_LABEL, METRICS_READY_EVENT);
  },
};
