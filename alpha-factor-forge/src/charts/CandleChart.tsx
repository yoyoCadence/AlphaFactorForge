// Slice 3 — candlestick canvas with indicator overlays.
//
// Static fit-to-width render of the latest `maxBars` candles: price pane
// (candles + MA fast/slow + EMA + Bollinger), a volume strip, and an RSI
// subpanel. Indicators come from core/indicators (computed over the full series
// so warm-up is correct, then drawn over the visible window). An optional `upto`
// cursor clips the window to bars [.., upto] for bar replay (a dashed playhead
// marks it). Pan/zoom is deferred to a later slice. Pure drawing — no IO/state.

import React, { useEffect, useRef, useState } from 'react';
import { sma, ema, bbands, rsi, type Series } from '../core/indicators';
import type { Candle as CoreCandle } from '../core/backtest';
import type { ClosedTrade } from '../core/metrics';
import type { ParamsStrategy } from '../services/strategy';
import { extentOf, padExtent, valueToY, tradeLegs, replayWindow } from './scale';

export interface OverlayToggles {
  ma: boolean;
  ema: boolean;
  bb: boolean;
  rsi: boolean;
  vol: boolean;
  trades: boolean;
}

export interface CandleChartProps {
  candles: CoreCandle[];
  strat: ParamsStrategy;
  show: OverlayToggles;
  /** entry/exit markers from the latest backtest (drawn when show.trades). */
  trades?: ClosedTrade[];
  /** Bar-replay cursor: when set, draw only candles up to this index (inclusive)
   *  and mark it as the current bar. Undefined = show the latest bars (no replay). */
  upto?: number;
  height?: number;
  maxBars?: number;
}

const COL = {
  up: '#2d9f73',
  down: '#d23b2f',
  grid: '#efece5',
  axis: '#8a8678',
  maFast: '#2563eb',
  maSlow: '#f59e0b',
  ema: '#7c3aed',
  bb: '#b9b4a8',
  rsi: '#16150f',
  playhead: '#2f6df0',
};

export function CandleChart({ candles, strat, show, trades, upto, height = 360, maxBars = 500 }: CandleChartProps): React.ReactElement {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) setWidth(e.contentRect.width);
    });
    ro.observe(el);
    setWidth(el.clientWidth);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || width <= 0 || candles.length === 0) return;
    draw(canvas, width, height, candles, strat, show, maxBars, trades, upto);
  }, [candles, strat, show, width, height, maxBars, trades, upto]);

  return (
    <div ref={wrapRef} style={{ width: '100%' }}>
      <canvas ref={canvasRef} style={{ width: '100%', height, display: 'block' }} />
    </div>
  );
}

/** Filled triangle marker with a white outline. `apexY` is the point nearest
 *  the candle; 'up' points up (buy, sits below the bar), 'down' points down. */
function drawMarker(ctx: CanvasRenderingContext2D, x: number, apexY: number, dir: 'up' | 'down', color: string): void {
  const s = 4; // half base width
  const hgt = 7;
  ctx.beginPath();
  if (dir === 'up') {
    ctx.moveTo(x, apexY);
    ctx.lineTo(x - s, apexY + hgt);
    ctx.lineTo(x + s, apexY + hgt);
  } else {
    ctx.moveTo(x, apexY);
    ctx.lineTo(x - s, apexY - hgt);
    ctx.lineTo(x + s, apexY - hgt);
  }
  ctx.closePath();
  ctx.fillStyle = color;
  ctx.fill();
  ctx.lineWidth = 1;
  ctx.strokeStyle = '#fff';
  ctx.stroke();
}

function draw(
  canvas: HTMLCanvasElement,
  w: number,
  h: number,
  candles: CoreCandle[],
  strat: ParamsStrategy,
  show: OverlayToggles,
  maxBars: number,
  trades?: ClosedTrade[],
  upto?: number,
): void {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(h * dpr);
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = '#fff';
  ctx.fillRect(0, 0, w, h);

  const padL = 6;
  const padR = 54;
  const padTop = 6;
  const padBottom = 6;
  const gap = 6;
  const plotW = w - padL - padR;

  const volH = show.vol ? 46 : 0;
  const rsiH = show.rsi ? 64 : 0;
  const priceH = h - padTop - padBottom - (show.vol ? volH + gap : 0) - (show.rsi ? rsiH + gap : 0);
  const priceTop = padTop;
  const volTop = priceTop + priceH + gap;
  const rsiTop = (show.vol ? volTop + volH : priceTop + priceH) + gap;

  // Visible bar window. Without `upto` this is the latest `maxBars` bars (the
  // pre-replay behaviour); with `upto` it ends at the replay cursor. `end` is
  // inclusive; indicators are still computed over the full series below.
  const { start, end } = replayWindow(candles.length, upto, maxBars);
  const n = end - start + 1;
  const bw = plotW / n;
  const xc = (i: number) => padL + (i - start + 0.5) * bw;

  const closes = candles.map((c) => c.c);
  const maFast = show.ma ? sma(closes, strat.fastMA) : null;
  const maSlow = show.ma ? sma(closes, strat.slowMA) : null;
  const emaArr = show.ema ? ema(closes, strat.emaPeriod) : null;
  const bb = show.bb ? bbands(closes, strat.bbPeriod, strat.bbMult) : null;
  const rsiArr = show.rsi ? rsi(closes, strat.rsiPeriod) : null;

  // price extent over the visible window (candles + overlays)
  const vals: number[] = [];
  for (let i = start; i <= end; i++) {
    vals.push(candles[i].h, candles[i].l);
    for (const s of [maFast, maSlow, emaArr, bb?.upper, bb?.lower]) {
      if (s && Number.isFinite(s[i])) vals.push(s[i]);
    }
  }
  const ext = padExtent(extentOf(vals), 0.05);
  const py = (p: number) => valueToY(p, ext, priceTop, priceH);

  // price grid + right-axis labels
  ctx.font = "10px 'IBM Plex Mono', monospace";
  ctx.textBaseline = 'middle';
  ctx.strokeStyle = COL.grid;
  ctx.fillStyle = COL.axis;
  ctx.lineWidth = 1;
  const ticks = 4;
  for (let t = 0; t <= ticks; t++) {
    const v = ext.min + ((ext.max - ext.min) * t) / ticks;
    const y = py(v);
    ctx.beginPath();
    ctx.moveTo(padL, y);
    ctx.lineTo(padL + plotW, y);
    ctx.stroke();
    ctx.fillText(v.toFixed(2), padL + plotW + 4, y);
  }

  // candles
  const bodyW = Math.max(1, bw * 0.7);
  for (let i = start; i <= end; i++) {
    const c = candles[i];
    const x = xc(i);
    const up = c.c >= c.o;
    ctx.strokeStyle = up ? COL.up : COL.down;
    ctx.fillStyle = up ? COL.up : COL.down;
    // wick
    ctx.beginPath();
    ctx.moveTo(x, py(c.h));
    ctx.lineTo(x, py(c.l));
    ctx.stroke();
    // body
    const yo = py(c.o);
    const ycl = py(c.c);
    const top = Math.min(yo, ycl);
    const bh = Math.max(1, Math.abs(ycl - yo));
    ctx.fillRect(x - bodyW / 2, top, bodyW, bh);
  }

  const polyline = (s: Series | null | undefined, color: string) => {
    if (!s) return;
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.25;
    ctx.beginPath();
    let started = false;
    for (let i = start; i <= end; i++) {
      const v = s[i];
      if (!Number.isFinite(v)) {
        started = false;
        continue;
      }
      const x = xc(i);
      const y = py(v);
      if (!started) {
        ctx.moveTo(x, y);
        started = true;
      } else {
        ctx.lineTo(x, y);
      }
    }
    ctx.stroke();
  };

  if (bb) {
    polyline(bb.upper, COL.bb);
    polyline(bb.middle, COL.bb);
    polyline(bb.lower, COL.bb);
  }
  polyline(maFast, COL.maFast);
  polyline(maSlow, COL.maSlow);
  polyline(emaArr, COL.ema);

  // trade markers: buy ▲ below the low (green), sell ▼ above the high (red)
  if (show.trades && trades && trades.length) {
    const timeToIndex = new Map<number, number>();
    for (let i = 0; i < candles.length; i++) timeToIndex.set(candles[i].t, i);
    for (const lg of tradeLegs(trades, timeToIndex)) {
      if (lg.index < start || lg.index > end) continue;
      const c = candles[lg.index];
      const x = xc(lg.index);
      if (lg.kind === 'buy') drawMarker(ctx, x, py(c.l) + 4, 'up', COL.up);
      else drawMarker(ctx, x, py(c.h) - 4, 'down', COL.down);
    }
  }

  // volume strip
  if (show.vol) {
    let maxVol = 0;
    for (let i = start; i <= end; i++) maxVol = Math.max(maxVol, candles[i].v);
    if (maxVol > 0) {
      for (let i = start; i <= end; i++) {
        const c = candles[i];
        const bh = (c.v / maxVol) * volH;
        ctx.fillStyle = c.c >= c.o ? COL.up : COL.down;
        ctx.globalAlpha = 0.5;
        ctx.fillRect(xc(i) - bodyW / 2, volTop + volH - bh, bodyW, bh);
        ctx.globalAlpha = 1;
      }
    }
    ctx.fillStyle = COL.axis;
    ctx.fillText('VOL', padL + plotW + 4, volTop + 6);
  }

  // RSI subpanel
  if (show.rsi && rsiArr) {
    const ry = (v: number) => rsiTop + (1 - v / 100) * rsiH;
    ctx.strokeStyle = COL.grid;
    for (const g of [30, 70]) {
      const y = ry(g);
      ctx.beginPath();
      ctx.moveTo(padL, y);
      ctx.lineTo(padL + plotW, y);
      ctx.stroke();
      ctx.fillStyle = COL.axis;
      ctx.fillText(String(g), padL + plotW + 4, y);
    }
    ctx.strokeStyle = COL.rsi;
    ctx.lineWidth = 1.25;
    ctx.beginPath();
    let started = false;
    for (let i = start; i <= end; i++) {
      const v = rsiArr[i];
      if (!Number.isFinite(v)) {
        started = false;
        continue;
      }
      const x = xc(i);
      const y = ry(v);
      if (!started) {
        ctx.moveTo(x, y);
        started = true;
      } else {
        ctx.lineTo(x, y);
      }
    }
    ctx.stroke();
    ctx.fillStyle = COL.axis;
    ctx.fillText('RSI', padL + 2, rsiTop + 6);
  }

  // replay playhead: a dashed vertical guide at the current (last visible) bar
  if (upto != null) {
    const x = xc(end);
    ctx.save();
    ctx.strokeStyle = COL.playhead;
    ctx.globalAlpha = 0.6;
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 3]);
    ctx.beginPath();
    ctx.moveTo(x, padTop);
    ctx.lineTo(x, h - padBottom);
    ctx.stroke();
    ctx.restore();
  }
}
