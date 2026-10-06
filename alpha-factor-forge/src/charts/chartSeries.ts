// PERF-CHART-COMPUTE-001 — the candle chart's full-series data: indicator
// overlays and the trade-marker bar indices. They depend only on the candles,
// the overlay periods and toggles, and the trades — never on hover, the replay
// cursor, zoom or pan — so CandleChart memoizes them and every repaint reuses
// them. Indicators stay computed over the FULL series (correct warm-up); the
// drawing code slices the visible window. Pure: no React, DOM or IO.

import { sma, ema, bbands, rsi, type BbandsOut, type Series } from '../core/indicators';
import type { Candle } from '../core/backtest';
import type { ClosedTrade } from '../core/metrics';
import type { ParamsStrategy } from '../services/strategy';
import { tradeLegs, type TradeLeg } from './scale';

/** The strategy periods the overlays read. */
export type OverlayPeriods = Pick<ParamsStrategy, 'fastMA' | 'slowMA' | 'emaPeriod' | 'bbPeriod' | 'bbMult' | 'rsiPeriod'>;

/** The overlay toggles (CandleChart's `OverlayToggles`) that decide which
 *  indicator series exist. */
export interface IndicatorToggles {
  ma: boolean;
  ema: boolean;
  bb: boolean;
  rsi: boolean;
}

/** Full-series overlays; `null` when that overlay is off. */
export interface ChartSeries {
  maFast: Series | null;
  maSlow: Series | null;
  ema: Series | null;
  bb: BbandsOut | null;
  rsi: Series | null;
}

export function computeChartSeries(candles: Candle[], periods: OverlayPeriods, on: IndicatorToggles): ChartSeries {
  const closes = candles.map((c) => c.c);
  return {
    maFast: on.ma ? sma(closes, periods.fastMA) : null,
    maSlow: on.ma ? sma(closes, periods.slowMA) : null,
    ema: on.ema ? ema(closes, periods.emaPeriod) : null,
    bb: on.bb ? bbands(closes, periods.bbPeriod, periods.bbMult) : null,
    rsi: on.rsi ? rsi(closes, periods.rsiPeriod) : null,
  };
}

/** Entry/exit legs mapped to candle indices by exact open time; a leg whose
 *  time is not in `candles` is dropped (as the chart always did). */
export function tradeMarkers(candles: Candle[], trades: ClosedTrade[] | undefined): TradeLeg[] {
  if (!trades || trades.length === 0) return [];
  const timeToIndex = new Map<number, number>();
  for (let i = 0; i < candles.length; i++) timeToIndex.set(candles[i].t, i);
  return tradeLegs(trades, timeToIndex);
}
