// Typed native-window + cross-window event boundary (Slice 8b-1).
// Browser/Vite mode is a no-op; production window creation remains a Rust
// command, and child-window state arrives only through typed Tauri events.

import { emitTo, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { ClosedTrade, Metrics } from '../core/metrics';
import type { Candle } from '../core/backtest';
import type { OverlayToggles } from '../charts/CandleChart';
import type { ParamsStrategy } from '../services/strategy';

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
}

export type MetricsWindowUpdate = MetricsWindowSnapshot | null;

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
    await emitTo(METRICS_WINDOW_LABEL, METRICS_SNAPSHOT_EVENT, snapshot);
  },
  onMetricsReady: async (handler: () => void): Promise<UnlistenFn> => {
    if (!isTauri()) return noopUnlisten();
    return listen(METRICS_READY_EVENT, handler);
  },
  onMetricsSnapshot: async (handler: (snapshot: MetricsWindowUpdate) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return noopUnlisten();
    return listen<MetricsWindowUpdate>(METRICS_SNAPSHOT_EVENT, (event) => handler(event.payload));
  },
  signalMetricsReady: async (): Promise<void> => {
    if (!isTauri()) return;
    await emitTo(MAIN_WINDOW_LABEL, METRICS_READY_EVENT);
  },
};
