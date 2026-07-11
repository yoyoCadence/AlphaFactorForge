// Slice 3 — candlestick canvas with indicator overlays.
//
// Fit-to-width / wheel-zoom render of the latest `maxBars` candles: price pane
// (candles + MA fast/slow + EMA + Bollinger), a volume strip, and an RSI
// subpanel. Indicators come from core/indicators (computed over the full series
// so warm-up is correct, then drawn over the visible window). An optional `upto`
// cursor clips the window to bars [.., upto] for bar replay (a dashed playhead
// marks it). Slice 10 adds cursor-anchored wheel zoom, reset, and drag-pan with
// replay-safe bounds. Pure drawing — no IO.

import React, { useEffect, useRef, useState } from 'react';
import { sma, ema, bbands, rsi, type Series } from '../core/indicators';
import type { Candle as CoreCandle } from '../core/backtest';
import type { ClosedTrade } from '../core/metrics';
import type { ParamsStrategy } from '../services/strategy';
import { extentOf, padExtent, valueToY, tradeLegs, replayWindow, barAtX, reconcileBarWindow, zoomBarWindow, panBarWindow, type BarWindow } from './scale';

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
  /** Called with the bar index under the mouse on hover, or null on leave
   *  (Slice 9). Drives the shared 「此根資訊」 readout in BacktestPanel. */
  onHoverBar?: (index: number | null) => void;
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
  crosshair: '#3c3a30',
};

export function CandleChart({ candles, strat, show, trades, upto, onHoverBar, height = 360, maxBars = 500 }: CandleChartProps): React.ReactElement {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(0);
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);
  const [viewWindow, setViewWindow] = useState<BarWindow | null>(null);
  const [followReplay, setFollowReplay] = useState(true);
  const [dragging, setDragging] = useState(false);
  const dragRef = useRef<{ pointerId: number; startX: number; window: BarWindow; barWidth: number; moved: boolean } | null>(null);
  // Latest bar geometry, written by draw(), read by the hover handler to map a
  // mouse x back to a bar index (avoids re-deriving the layout on every move).
  const layoutRef = useRef<Layout | null>(null);

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

  const replayMode = upto != null;
  useEffect(() => {
    // A dataset change or entering/leaving replay starts from a predictable fit
    // window. Replay cursor movement itself preserves the zoom level below.
    setViewWindow(null);
    setFollowReplay(true);
    setDragging(false);
    dragRef.current = null;
  }, [candles, replayMode, maxBars]);

  const fittedWindow = replayWindow(candles.length, upto, maxBars);
  const last = candles.length - 1;
  const boundsEnd = upto == null ? last : Math.max(0, Math.min(last, Math.floor(upto)));
  const visibleWindow = viewWindow
    ? replayMode && followReplay
      ? reconcileBarWindow(viewWindow, candles.length, upto)
      : reconcileBarWindow(viewWindow, boundsEnd + 1)
    : fittedWindow;
  const visibleCount = Math.max(0, visibleWindow.end - visibleWindow.start + 1);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || width <= 0 || candles.length === 0) {
      layoutRef.current = null;
      return;
    }
    layoutRef.current = draw(canvas, width, height, candles, strat, show, visibleWindow, trades, upto, hoverIndex);
  }, [candles, strat, show, width, height, trades, upto, hoverIndex, visibleWindow.start, visibleWindow.end]);

  const updateHover = (clientX: number) => {
    const lay = layoutRef.current;
    const canvas = canvasRef.current;
    if (!lay || !canvas) return;
    const x = clientX - canvas.getBoundingClientRect().left;
    const idx = barAtX(x, lay.padL, lay.plotW, lay.start, lay.n);
    if (idx !== hoverIndex) {
      setHoverIndex(idx);
      onHoverBar?.(idx);
    }
  };
  const handleLeave = () => {
    if (hoverIndex !== null) {
      setHoverIndex(null);
      onHoverBar?.(null);
    }
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const lay = layoutRef.current;
    if (e.button !== 0 || viewWindow == null || !lay || lay.n <= 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    dragRef.current = {
      pointerId: e.pointerId,
      startX: e.clientX,
      window: { start: lay.start, end: lay.end },
      barWidth: lay.plotW / lay.n,
      moved: false,
    };
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== e.pointerId) {
      updateHover(e.clientX);
      return;
    }
    const dx = e.clientX - drag.startX;
    if (!drag.moved && Math.abs(dx) < 4) {
      updateHover(e.clientX);
      return;
    }
    e.preventDefault();
    if (!drag.moved) {
      drag.moved = true;
      setDragging(true);
      setHoverIndex(null);
      onHoverBar?.(null);
    }
    // Dragging content right reveals older bars; dragging left reveals newer.
    const next = panBarWindow(drag.window, -dx / drag.barWidth, boundsEnd);
    setViewWindow(next);
    if (replayMode) setFollowReplay(next.end === boundsEnd);
  };

  const finishPointer = (e: React.PointerEvent<HTMLCanvasElement>, cancelled = false) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
    dragRef.current = null;
    setDragging(false);
    if (cancelled) handleLeave();
    else updateHover(e.clientX);
  };

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    // React/Chromium may register wheel delegation as passive, which makes
    // preventDefault() ineffective and scrolls the whole page while zooming.
    // Bind directly with passive:false so chart zoom exclusively consumes it.
    const handleWheel = (e: WheelEvent) => {
      const lay = layoutRef.current;
      if (!lay || lay.n <= 0) return;
      e.preventDefault();
      const x = e.clientX - canvas.getBoundingClientRect().left;
      const anchor = barAtX(x, lay.padL, lay.plotW, lay.start, lay.n);
      const next = zoomBarWindow({ start: lay.start, end: lay.end }, anchor, e.deltaY, boundsEnd, 10, maxBars);
      const isFit = next.start === fittedWindow.start && next.end === fittedWindow.end;
      setViewWindow(isFit ? null : next);
      if (isFit) setFollowReplay(true);
      else if (replayMode && next.end === boundsEnd) setFollowReplay(true);
    };
    canvas.addEventListener('wheel', handleWheel, { passive: false });
    return () => canvas.removeEventListener('wheel', handleWheel);
  }, [boundsEnd, replayMode, maxBars, fittedWindow.start, fittedWindow.end]);

  return (
    <div ref={wrapRef} style={{ width: '100%', position: 'relative' }}>
      <canvas
        ref={canvasRef}
        data-testid="candle-canvas"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={finishPointer}
        onPointerCancel={(e) => finishPointer(e, true)}
        onPointerLeave={() => { if (!dragRef.current) handleLeave(); }}
        style={{ width: '100%', height, display: 'block', cursor: dragging ? 'grabbing' : viewWindow != null ? 'grab' : 'crosshair', touchAction: 'none', userSelect: 'none' }}
      />
      <div style={{ position: 'absolute', top: 8, right: 62, display: 'flex', alignItems: 'center', gap: 5, padding: '2px 4px', background: 'rgba(255,255,255,0.88)', border: '1px solid #efece5', fontFamily: "'IBM Plex Mono', monospace", fontSize: 10 }}>
        <span data-testid="chart-zoom-status" data-window-start={visibleWindow.start} data-window-end={visibleWindow.end} data-dragging={dragging} aria-live="polite">顯示 {visibleCount} 根</span>
        <button
          type="button"
          data-testid="chart-zoom-reset"
          title="重置為自動適配"
          disabled={viewWindow == null}
          onClick={() => { setViewWindow(null); setFollowReplay(true); }}
          style={{ padding: '1px 5px', border: '1px solid #d6d2c8', background: '#f4f2ec', color: '#3c3a30', font: 'inherit', cursor: viewWindow == null ? 'default' : 'pointer' }}
        >
          重置
        </button>
      </div>
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

/** Bar geometry returned by draw() so the hover handler can invert x -> bar. */
interface Layout {
  padL: number;
  plotW: number;
  start: number;
  end: number;
  n: number;
}

function draw(
  canvas: HTMLCanvasElement,
  w: number,
  h: number,
  candles: CoreCandle[],
  strat: ParamsStrategy,
  show: OverlayToggles,
  visibleWindow: BarWindow,
  trades: ClosedTrade[] | undefined,
  upto: number | undefined,
  hoverIndex: number | null,
): Layout {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(h * dpr);
  const ctx = canvas.getContext('2d');
  if (!ctx) return { padL: 0, plotW: 0, start: 0, end: -1, n: 0 };
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

  // Visible bar window is owned by the component's fit/zoom state. `end` is
  // inclusive; indicators are still computed over the full series below.
  const { start, end } = visibleWindow;
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

  // replay playhead: draw only when the actual cursor is inside a panned window.
  const playhead = upto == null ? null : Math.max(0, Math.min(candles.length - 1, Math.floor(upto)));
  if (playhead != null && playhead >= start && playhead <= end) {
    const x = xc(playhead);
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

  // hover crosshair: a dashed vertical guide at the bar under the mouse
  if (hoverIndex != null && hoverIndex >= start && hoverIndex <= end) {
    const x = xc(hoverIndex);
    ctx.save();
    ctx.strokeStyle = COL.crosshair;
    ctx.globalAlpha = 0.85;
    ctx.lineWidth = 1;
    ctx.setLineDash([2, 2]);
    ctx.beginPath();
    ctx.moveTo(x, padTop);
    ctx.lineTo(x, h - padBottom);
    ctx.stroke();
    ctx.restore();
  }

  return { padL, plotW, start, end, n };
}
