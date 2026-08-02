// Canvas painting helpers driven by ChartTheme.
//
// Drop-in target: alpha-factor-forge/src/charts/chartPaint.ts
//
// ChartSection currently hard-codes candle/MA/grid colours inside its draw
// pass. Replace those literals with `const C = useTheme().chart` and call these
// helpers — `C.style` is what makes each skin's chart read differently
// (filled bodies / hollow / OHLC bars / outline / soft), not just recoloured.
//
// Pure canvas + numbers: no React, no DOM lookups, unit-testable like core/*.

import type { ChartTheme } from '../theme/theme';

export interface Bar { open: number; high: number; low: number; close: number }

export interface Geom {
  /** Pixel centre of bar i. */
  x: (i: number) => number;
  /** Pixel y of a price. */
  y: (price: number) => number;
  /** Body width in px. */
  barWidth: number;
}

export function paintGrid(
  g: CanvasRenderingContext2D,
  C: ChartTheme,
  box: { left: number; right: number; top: number; bottom: number },
  hi: number,
  lo: number,
  lines = 4,
  /** Axis label formatter. The default drops the decimals, which is unreadable
   *  for any instrument priced below ~10 (every gridline would render "0"), so
   *  the price pane passes its own. Added to the handoff helper for that reason;
   *  everything else in this file is the handoff version verbatim. */
  format: (value: number) => string = (value) => value.toFixed(0),
): void {
  g.save();
  g.strokeStyle = C.grid;
  g.lineWidth = 1;
  if (C.dash) g.setLineDash(C.dash);
  g.font = `9px ${C.label ? 'monospace' : 'monospace'}`;
  g.textBaseline = 'middle';
  for (let i = 0; i <= lines; i++) {
    const y = Math.round(box.top + (box.bottom - box.top) * (i / lines)) + 0.5;
    g.beginPath();
    g.moveTo(box.left, y);
    g.lineTo(box.right, y);
    g.stroke();
    g.setLineDash([]);
    g.fillStyle = C.label;
    g.textAlign = 'left';
    g.fillText(format(hi - (hi - lo) * (i / lines)), box.right + 8, y);
    if (C.dash) g.setLineDash(C.dash);
  }
  g.restore();
}

/** Whole-series renderers ('line' / 'area'). Call INSTEAD of paintBars when
 *  `C.style` is one of them — those skins show no per-bar marks at all. */
export function paintSeries(
  g: CanvasRenderingContext2D,
  C: ChartTheme,
  bars: Bar[],
  geom: Geom,
  baselineY: number,
): void {
  const path = () => {
    g.beginPath();
    bars.forEach((k, i) => {
      const x = geom.x(i);
      const y = geom.y(k.close);
      if (i === 0) g.moveTo(x, y);
      else g.lineTo(x, y);
    });
  };
  if (C.style === 'area') {
    const grad = g.createLinearGradient(0, 0, 0, baselineY);
    grad.addColorStop(0, C.up);
    grad.addColorStop(1, 'rgba(0,0,0,0)');
    g.save();
    g.globalAlpha = 0.22;
    path();
    g.lineTo(geom.x(bars.length - 1), baselineY);
    g.lineTo(geom.x(0), baselineY);
    g.closePath();
    g.fillStyle = grad;
    g.fill();
    g.restore();
  }
  g.strokeStyle = C.up;
  g.lineWidth = C.lw + 0.6;
  g.lineJoin = 'round';
  path();
  g.stroke();
}

export function paintBars(g: CanvasRenderingContext2D, C: ChartTheme, bars: Bar[], geom: Geom): void {
  const w = geom.barWidth;
  bars.forEach((k, i) => {
    const up = k.close >= k.open;
    const col = up ? C.up : C.down;
    const cx = geom.x(i);
    const yo = geom.y(k.open);
    const yc = geom.y(k.close);
    const top = Math.min(yo, yc);
    const bh = Math.max(1, Math.abs(yc - yo));
    const x = cx - w / 2;

    if (C.style === 'stick') {
      // Engraved newsprint bar: hairline high-low with a close tick to the right.
      g.strokeStyle = col;
      g.lineWidth = 1;
      g.beginPath();
      g.moveTo(Math.round(cx) + 0.5, geom.y(k.high));
      g.lineTo(Math.round(cx) + 0.5, geom.y(k.low));
      g.stroke();
      g.lineWidth = 2;
      g.beginPath();
      g.moveTo(Math.round(cx) + 0.5, yc);
      g.lineTo(Math.round(cx) + 0.5 + w / 2 + 1, yc);
      g.stroke();
      return;
    }

    if (C.style === 'block') {
      // Brutalist: fat body, hard black keyline, no wick.
      const bh2 = Math.max(3, Math.abs(yc - yo));
      g.fillStyle = col;
      g.fillRect(x, top, w, bh2);
      g.strokeStyle = '#000000';
      g.lineWidth = 1.5;
      g.strokeRect(x, top, w, bh2);
      return;
    }

    if (C.style === 'bar') {
      g.strokeStyle = col;
      g.lineWidth = C.lw;
      g.beginPath();
      g.moveTo(cx, geom.y(k.high));
      g.lineTo(cx, geom.y(k.low));
      g.moveTo(x, yo);
      g.lineTo(cx, yo);
      g.moveTo(cx, yc);
      g.lineTo(cx + w / 2, yc);
      g.stroke();
      return;
    }

    g.strokeStyle = col;
    g.lineWidth = 1;
    g.beginPath();
    g.moveTo(Math.round(cx) + 0.5, geom.y(k.high));
    g.lineTo(Math.round(cx) + 0.5, geom.y(k.low));
    g.stroke();

    if (C.style === 'hollow' && up) {
      g.lineWidth = C.lw;
      g.strokeRect(x, top, w, bh);
    } else if (C.style === 'outline') {
      g.lineWidth = C.lw;
      g.strokeRect(x + 0.5, top + 0.5, w - 1, bh);
    } else if (C.style === 'soft') {
      g.fillStyle = col;
      g.globalAlpha = 0.9;
      if (typeof g.roundRect === 'function') {
        g.beginPath();
        g.roundRect(x, top, w, bh, Math.min(2, w / 3));
        g.fill();
      } else {
        g.fillRect(x, top, w, bh);
      }
      g.globalAlpha = 1;
    } else {
      g.fillStyle = col;
      g.fillRect(x, top, w, bh);
    }
  });
}

export function paintLine(
  g: CanvasRenderingContext2D,
  C: ChartTheme,
  values: (number | null)[],
  geom: Geom,
  color: string,
  dash?: number[],
): void {
  g.save();
  g.strokeStyle = color;
  g.lineWidth = C.lw;
  if (dash) g.setLineDash(dash);
  g.beginPath();
  let started = false;
  values.forEach((v, i) => {
    if (v == null) return;
    const x = geom.x(i);
    const y = geom.y(v);
    if (!started) {
      g.moveTo(x, y);
      started = true;
    } else {
      g.lineTo(x, y);
    }
  });
  g.stroke();
  g.restore();
}

/** Heatmap cell fill: cold = surface2, hot = accent. Keeps the sweep heatmap
 *  legible on light and dark skins without a second palette. */
export function heatColor(C: ChartTheme, t: number): string {
  const clamp = Math.max(0, Math.min(1, t));
  const A = hexToRgb(C.surface2);
  const B = hexToRgb(C.accent);
  return `rgb(${A.map((v, i) => Math.round(v + (B[i] - v) * clamp * 0.92)).join(',')})`;
}

export function heatTextColor(C: ChartTheme, t: number): string {
  return t > 0.55 ? C.accentInk : C.label;
}

function hexToRgb(hex: string): number[] {
  const s = hex.replace('#', '');
  const n = s.length === 3 ? s.split('').map((c) => c + c).join('') : s;
  return [parseInt(n.slice(0, 2), 16), parseInt(n.slice(2, 4), 16), parseInt(n.slice(4, 6), 16)];
}
