import { describe, expect, it } from 'vitest';
import { bbands, ema, rsi, sma } from '../core/indicators';
import type { Candle } from '../core/backtest';
import type { ClosedTrade } from '../core/metrics';
import { computeChartSeries, tradeMarkers } from './chartSeries';

// PERF-CHART-COMPUTE-001 moved the chart's full-series computation out of the
// per-repaint draw() so it can be memoized. These pin that the extracted
// functions produce exactly what draw() used to compute inline.

const candles: Candle[] = Array.from({ length: 120 }, (_, i) => {
  const o = 100 + 10 * Math.sin(i / 7);
  const c = 100 + 10 * Math.sin((i + 1) / 7);
  return { t: 1_700_000_000_000 + i * 3_600_000, o, h: Math.max(o, c) + 1, l: Math.min(o, c) - 1, c, v: 10 + i };
});
const closes = candles.map((c) => c.c);
const periods = { fastMA: 5, slowMA: 20, emaPeriod: 9, bbPeriod: 20, bbMult: 2, rsiPeriod: 14 };

describe('computeChartSeries', () => {
  it('matches the indicator functions over the full close series', () => {
    const series = computeChartSeries(candles, periods, { ma: true, ema: true, bb: true, rsi: true });
    expect(series.maFast).toEqual(sma(closes, 5));
    expect(series.maSlow).toEqual(sma(closes, 20));
    expect(series.ema).toEqual(ema(closes, 9));
    expect(series.bb).toEqual(bbands(closes, 20, 2));
    expect(series.rsi).toEqual(rsi(closes, 14));
  });

  it('leaves a switched-off overlay null', () => {
    expect(computeChartSeries(candles, periods, { ma: false, ema: false, bb: false, rsi: false })).toEqual({
      maFast: null,
      maSlow: null,
      ema: null,
      bb: null,
      rsi: null,
    });
    const onlyRsi = computeChartSeries(candles, periods, { ma: false, ema: false, bb: false, rsi: true });
    expect(onlyRsi.rsi).toEqual(rsi(closes, 14));
    expect(onlyRsi.maFast).toBeNull();
  });
});

describe('tradeMarkers', () => {
  const trade = (entryBar: number, exitBar: number, side: 'LONG' | 'SHORT'): ClosedTrade =>
    ({ entryTime: candles[entryBar].t, exitTime: candles[exitBar].t, side }) as ClosedTrade;

  it('maps entry/exit times to bar indices with the side-dependent marker', () => {
    expect(tradeMarkers(candles, [trade(3, 8, 'LONG'), trade(10, 15, 'SHORT')])).toEqual([
      { index: 3, kind: 'buy', leg: 'entry' },
      { index: 8, kind: 'sell', leg: 'exit' },
      { index: 10, kind: 'sell', leg: 'entry' },
      { index: 15, kind: 'buy', leg: 'exit' },
    ]);
  });

  it('drops legs whose time is not a loaded bar, and returns nothing without trades', () => {
    const offGrid = { entryTime: candles[2].t + 1, exitTime: candles[4].t, side: 'LONG' } as ClosedTrade;
    expect(tradeMarkers(candles, [offGrid])).toEqual([{ index: 4, kind: 'sell', leg: 'exit' }]);
    expect(tradeMarkers(candles, [])).toEqual([]);
    expect(tradeMarkers(candles, undefined)).toEqual([]);
  });
});
