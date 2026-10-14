import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultStrategy } from '../services/strategy';
import type { Metrics } from '../core/metrics';
import {
  decodeMetricsSnapshot,
  encodeMetricsSnapshot,
  mergeChartSnapshot,
  popoutWindows,
  type ChartWindowSnapshot,
  type MetricsWindowSnapshot,
  type MetricsWindowUpdate,
  type MetricsWindowWire,
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
    show: { ma: true, ema: false, bb: false, rsi: true, vol: true, trades: true, volUpDown: false },
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

function metrics(over: Partial<Metrics> = {}): Metrics {
  return {
    netReturn: 0.1, cagr: 0.2, maxDrawdown: 0.05, sharpe: 1.5, sortino: 2, calmar: 4,
    winRate: 0.5, tradeCount: 3, profitFactor: 1.8, avgTradeReturn: 0.01, medianTradeReturn: 0.01,
    avgHoldingBars: 4, exposure: 0.4, turnover: 0.02, largestWin: 0.05, largestLoss: -0.02,
    consecutiveLosses: 1, monthlyReturns: { '2024-01': 0.1 },
    ...over,
  };
}

describe('metrics window updates', () => {
  it('transports the completed samples without aliasing the sender or wire', () => {
    const sent: MetricsWindowSnapshot = {
      title: 'long full / short holdout', full: metrics(), inSample: metrics(), outSample: metrics(),
      samples: { full: { totalBars: 600, barsPerYear: 365 }, inSample: { totalBars: 420, barsPerYear: 365 }, outSample: { totalBars: 180, barsPerYear: 365 } },
    };
    const wire = encodeMetricsSnapshot(sent);
    expect(wire.samples).toEqual(sent.samples);
    const arrived = decodeMetricsSnapshot(JSON.parse(JSON.stringify(wire)));
    expect(arrived).toEqual(sent);
    wire.samples!.full.totalBars = 1;
    expect(sent.samples!.full.totalBars).toBe(600);
    arrived.samples!.outSample!.totalBars = 1;
    expect(wire.samples!.outSample!.totalBars).toBe(180);
  });

  it('transports both a clear update and the latest snapshot', async () => {
    let eventHandler: ((event: { payload: MetricsWindowWire | null }) => void) | undefined;
    tauriMocks.listen.mockImplementation(async (_event, handler) => {
      eventHandler = handler;
      return () => undefined;
    });
    const received: MetricsWindowUpdate[] = [];
    await popoutWindows.onMetricsSnapshot((update) => received.push(update));

    const latest: MetricsWindowSnapshot = { title: 'latest', full: metrics() };
    await popoutWindows.publishMetrics(null);
    await popoutWindows.publishMetrics(latest);

    expect(tauriMocks.emitTo).toHaveBeenNthCalledWith(1, 'metrics-popout-window', 'aff:metrics-snapshot', null);
    const [label, event, wire] = tauriMocks.emitTo.mock.calls[1];
    expect([label, event]).toEqual(['metrics-popout-window', 'aff:metrics-snapshot']);

    // Deliver what was emitted, the way the event system does: as JSON.
    eventHandler?.({ payload: null });
    eventHandler?.({ payload: JSON.parse(JSON.stringify(wire)) });
    expect(received).toEqual([null, latest]);
  });

  // METRIC-DISPLAY-001: a bare Metrics object lost its infinite ratios to
  // JSON, so the pop-out showed a dash where the main window showed a value.
  it('carries non-finite metrics across the JSON boundary', async () => {
    let eventHandler: ((event: { payload: MetricsWindowWire | null }) => void) | undefined;
    tauriMocks.listen.mockImplementation(async (_event, handler) => {
      eventHandler = handler;
      return () => undefined;
    });
    const received: MetricsWindowUpdate[] = [];
    await popoutWindows.onMetricsSnapshot((update) => received.push(update));

    const sent: MetricsWindowSnapshot = {
      title: 'no losing trade',
      full: metrics({ sortino: Number.POSITIVE_INFINITY, calmar: Number.POSITIVE_INFINITY, profitFactor: Number.POSITIVE_INFINITY }),
      inSample: metrics({ cagr: Number.NaN }),
      outSample: metrics({ sharpe: Number.NEGATIVE_INFINITY }),
    };
    await popoutWindows.publishMetrics(sent);
    const wire = tauriMocks.emitTo.mock.calls[0][2] as MetricsWindowWire;

    // On the wire every number is finite or null, with an explicit status.
    const json = JSON.stringify(wire);
    expect(json).not.toMatch(/Infinity|NaN/);
    expect(wire.full.values.sortino).toBeNull();
    expect(wire.full.nonFinite).toEqual({ sortino: 'positive_infinity', calmar: 'positive_infinity', profitFactor: 'positive_infinity' });

    eventHandler?.({ payload: JSON.parse(json) });
    const arrived = received[0] as MetricsWindowSnapshot;
    expect(arrived.full.sortino).toBe(Number.POSITIVE_INFINITY);
    expect(arrived.full.calmar).toBe(Number.POSITIVE_INFINITY);
    expect(arrived.full.profitFactor).toBe(Number.POSITIVE_INFINITY);
    expect(arrived.inSample?.cagr).toBeNaN();
    expect(arrived.outSample?.sharpe).toBe(Number.NEGATIVE_INFINITY);
    expect(arrived).toEqual(sent);
  });

  it('omits the holdout columns when the run has none', () => {
    const wire = encodeMetricsSnapshot({ title: 'full only', full: metrics() });
    expect(Object.keys(wire).sort()).toEqual(['full', 'title']);
    expect(Object.keys(decodeMetricsSnapshot(wire)).sort()).toEqual(['full', 'title']);
  });

  it('leaves a null without a status as null rather than inventing a value', () => {
    const wire = encodeMetricsSnapshot({ title: 't', full: metrics() });
    (wire.full.values as { sharpe: number | null }).sharpe = null;
    expect(decodeMetricsSnapshot(wire).full.sharpe).toBeNull();
  });

  it('does not alias the wire payload', () => {
    const wire = encodeMetricsSnapshot({ title: 't', full: metrics() });
    const decoded = decodeMetricsSnapshot(wire);
    decoded.full.monthlyReturns['2024-02'] = 1;
    expect(wire.full.values.monthlyReturns).toEqual({ '2024-01': 0.1 });
  });
});
