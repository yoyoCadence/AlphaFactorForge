import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultStrategy } from '../services/strategy';
import {
  mergeChartSnapshot,
  popoutWindows,
  type ChartWindowSnapshot,
  type MetricsWindowSnapshot,
  type MetricsWindowUpdate,
} from './windowBridge';

const tauriMocks = vi.hoisted(() => ({
  emitTo: vi.fn(),
  listen: vi.fn(),
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriMocks.invoke,
  isTauri: () => true,
}));

vi.mock('@tauri-apps/api/event', () => ({
  emitTo: tauriMocks.emitTo,
  listen: tauriMocks.listen,
}));

beforeEach(() => {
  vi.clearAllMocks();
});

function snapshot(datasetKey: string, close: number): ChartWindowSnapshot {
  return {
    datasetKey,
    title: datasetKey,
    candles: [{ t: 1, o: close, h: close, l: close, c: close, v: 1 }],
    strat: defaultStrategy(),
    show: { ma: true, ema: false, bb: false, rsi: true, vol: true, trades: true },
    trades: [],
  };
}

describe('mergeChartSnapshot', () => {
  it('preserves candle identity for updates to the same dataset', () => {
    const current = snapshot('same', 10);
    const incoming = { ...snapshot('same', 99), title: 'updated title', upto: 0 };
    const merged = mergeChartSnapshot(current, incoming);
    expect(merged.candles).toBe(current.candles);
    expect(merged.title).toBe('updated title');
    expect(merged.upto).toBe(0);
  });

  it('accepts a new candle array when the dataset changes', () => {
    const incoming = snapshot('new', 20);
    expect(mergeChartSnapshot(snapshot('old', 10), incoming)).toBe(incoming);
    expect(mergeChartSnapshot(null, incoming)).toBe(incoming);
  });
});

describe('metrics window updates', () => {
  it('transports both a clear update and the latest snapshot', async () => {
    let eventHandler: ((event: { payload: MetricsWindowUpdate }) => void) | undefined;
    tauriMocks.listen.mockImplementation(async (_event, handler) => {
      eventHandler = handler;
      return () => undefined;
    });
    const received: MetricsWindowUpdate[] = [];
    await popoutWindows.onMetricsSnapshot((update) => received.push(update));

    eventHandler?.({ payload: null });
    const latest = { title: 'latest', full: { tradeCount: 3 } } as MetricsWindowSnapshot;
    eventHandler?.({ payload: latest });
    await popoutWindows.publishMetrics(null);
    await popoutWindows.publishMetrics(latest);

    expect(received).toEqual([null, latest]);
    expect(tauriMocks.emitTo).toHaveBeenNthCalledWith(1, 'metrics-popout-window', 'aff:metrics-snapshot', null);
    expect(tauriMocks.emitTo).toHaveBeenNthCalledWith(2, 'metrics-popout-window', 'aff:metrics-snapshot', latest);
  });
});
