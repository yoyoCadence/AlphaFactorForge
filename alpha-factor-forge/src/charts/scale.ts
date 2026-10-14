// Tiny pure helpers for chart scaling. Kept separate from the canvas component
// so the numeric logic is unit-testable (canvas drawing is not).

export interface Extent {
  min: number;
  max: number;
}

export interface BarWindow {
  start: number;
  end: number;
}

/** Min/max over finite values, ignoring NaN/Infinity. Degenerate inputs get a
 *  safe non-zero span so divisions by (max-min) never blow up. */
export function extentOf(values: number[]): Extent {
  let min = Infinity;
  let max = -Infinity;
  for (const v of values) {
    if (Number.isFinite(v)) {
      if (v < min) min = v;
      if (v > max) max = v;
    }
  }
  if (!Number.isFinite(min) || !Number.isFinite(max)) return { min: 0, max: 1 };
  if (min === max) {
    // Widen relative to the value: a fixed +/-1 swamps a sub-cent price (the
    // axis would run from about -1 to 1) and is invisible on a 60,000 one.
    const pad = min !== 0 ? Math.abs(min) * 0.01 : 1;
    return { min: min - pad, max: max + pad };
  }
  return { min, max };
}

export const PRICE_MIN_DECIMALS = 2;
export const PRICE_MAX_DECIMALS = 8;

/**
 * Decimal places for prices that are read together: the price axis, or one
 * bar's open/high/low/close (CHART-PRECISION-001). A fixed two reads `0.00`
 * for every price below a cent.
 *
 * The count is the larger of what the values' size needs (four significant
 * digits of the largest one) and what their spread needs (one digit finer than
 * a quarter of the range, which is the spacing of the axis gridlines), kept
 * within [PRICE_MIN_DECIMALS, PRICE_MAX_DECIMALS]. Prices from 10 upward keep
 * two decimals unless the range is narrow enough to need more.
 */
export function priceDecimals(min: number, max: number): number {
  const largest = Math.max(Math.abs(min), Math.abs(max));
  if (!Number.isFinite(largest) || largest === 0) return PRICE_MIN_DECIMALS;
  const fromSize = 3 - Math.floor(Math.log10(largest));
  const spread = Math.abs(max - min);
  const fromSpread = spread > 0 ? Math.ceil(-Math.log10(spread / 4)) + 1 : PRICE_MIN_DECIMALS;
  return Math.max(PRICE_MIN_DECIMALS, Math.min(PRICE_MAX_DECIMALS, Math.max(fromSize, fromSpread)));
}

/** Narrowest right-hand price gutter; fits labels of up to eight characters. */
export const PRICE_GUTTER_MIN_WIDTH = 54;

/**
 * Width in CSS pixels of the chart's right-hand price gutter for axis labels
 * of `labelLength` characters: about 5.4px per character of the 9px axis face,
 * plus the gap before the label and a margin. Derived from the character count
 * rather than measured, so the layout does not depend on which font loaded.
 */
export function priceGutterWidth(labelLength: number): number {
  return Math.max(PRICE_GUTTER_MIN_WIDTH, Math.ceil(labelLength * 5.4 + 10));
}

/** Expand an extent by `frac` on each side (e.g. 0.05 = 5% headroom). */
export function padExtent(e: Extent, frac: number): Extent {
  const d = (e.max - e.min) * frac;
  return { min: e.min - d, max: e.max + d };
}

/** Map a value in [min,max] to a pixel y in [top, top+height] (inverted: max
 *  at the top). Returns top for a zero span. */
export function valueToY(v: number, e: Extent, top: number, height: number): number {
  const span = e.max - e.min;
  if (span <= 0) return top;
  return top + (1 - (v - e.min) / span) * height;
}

/** The inclusive bar window [start, end] the chart should draw for bar replay.
 *  `upto` is the replay cursor (index of the last visible bar); null/undefined
 *  means "up to the latest bar" (i.e. no replay — the pre-6-1 behaviour). `end`
 *  is floored and clamped to [0, total-1]; the window shows up to `maxBars` bars
 *  ending at `end`. An empty series yields { start: 0, end: -1 } (draw nothing). */
export function replayWindow(
  total: number,
  upto: number | null | undefined,
  maxBars: number,
): { start: number; end: number } {
  if (total <= 0) return { start: 0, end: -1 };
  const last = total - 1;
  let end = upto == null ? last : Math.floor(upto);
  if (end < 0) end = 0;
  if (end > last) end = last;
  const cap = Math.max(1, Math.floor(maxBars));
  const n = Math.min(end + 1, cap);
  return { start: end + 1 - n, end };
}

/** Clamp a persisted chart window to the currently available bars. During bar
 *  replay the window follows the replay cursor while preserving its bar count,
 *  so zoom never reveals future candles and the playhead remains visible. */
export function reconcileBarWindow(
  window: BarWindow,
  total: number,
  upto?: number | null,
): BarWindow {
  if (total <= 0) return { start: 0, end: -1 };
  const last = total - 1;
  const limit = Math.max(0, Math.min(last, upto == null ? last : Math.floor(upto)));
  const requested = Math.max(1, Math.floor(window.end) - Math.floor(window.start) + 1);
  const count = Math.min(requested, limit + 1);

  if (upto != null) return { start: limit + 1 - count, end: limit };

  let start = Math.floor(window.start);
  start = Math.max(0, Math.min(limit + 1 - count, start));
  return { start, end: start + count - 1 };
}

/** Wheel-zoom an inclusive bar window, keeping the bar under the mouse at the
 *  same relative x position when bounds allow it. Negative delta zooms in;
 *  positive delta zooms out. The result is clamped to [0,boundsEnd] and to the
 *  configured min/max visible-bar counts. */
export function zoomBarWindow(
  window: BarWindow,
  anchor: number,
  deltaY: number,
  boundsEnd: number,
  minBars = 10,
  maxBars = 500,
): BarWindow {
  if (boundsEnd < 0) return { start: 0, end: -1 };
  const current = reconcileBarWindow(window, boundsEnd + 1);
  const count = current.end - current.start + 1;
  const maxCount = Math.max(1, Math.min(Math.floor(maxBars), boundsEnd + 1));
  const minCount = Math.max(1, Math.min(Math.floor(minBars), maxCount));
  if (deltaY === 0) return current;

  const factor = deltaY < 0 ? 0.8 : 1.25;
  const nextCount = Math.max(minCount, Math.min(maxCount, Math.round(count * factor)));
  if (nextCount === count) return current;

  const fixedAnchor = Math.max(current.start, Math.min(current.end, Math.floor(anchor)));
  const anchorRatio = (fixedAnchor - current.start + 0.5) / count;
  let start = Math.round(fixedAnchor + 0.5 - anchorRatio * nextCount);
  start = Math.max(0, Math.min(boundsEnd + 1 - nextCount, start));
  return { start, end: start + nextCount - 1 };
}

/** Shift an inclusive visible window by whole bars while preserving its size.
 *  Negative deltas reveal older bars; positive deltas reveal newer bars. The
 *  result is clamped to [0,boundsEnd], which is the replay cursor during replay
 *  so panning can never reveal future candles. */
export function panBarWindow(window: BarWindow, deltaBars: number, boundsEnd: number): BarWindow {
  if (boundsEnd < 0) return { start: 0, end: -1 };
  const current = reconcileBarWindow(window, boundsEnd + 1);
  const count = current.end - current.start + 1;
  const maxStart = Math.max(0, boundsEnd + 1 - count);
  const start = Math.max(0, Math.min(maxStart, current.start + Math.round(deltaBars)));
  return { start, end: start + count - 1 };
}

/** Inverse of the bar x-mapping (Slice 9 chart hover): the bar index under
 *  CSS-pixel `x`. `padL` is the left plot inset, `plotW` the plot width, `start`
 *  the first visible bar, `n` the visible bar count. Result is clamped to the
 *  visible window [start, start+n-1]. Pure. */
export function barAtX(x: number, padL: number, plotW: number, start: number, n: number): number {
  if (n <= 0) return start;
  const bw = plotW / n;
  const i = start + Math.floor((x - padL) / bw);
  return Math.max(start, Math.min(start + n - 1, i));
}

/** One autoplay step of the replay cursor (Slice 6-2): advance by one bar,
 *  clamped to the last bar. `atEnd` is true once the cursor has reached the last
 *  bar, so the caller can stop the timer. Pure. */
export function replayTick(cursor: number, total: number): { cursor: number; atEnd: boolean } {
  const last = Math.max(0, total - 1);
  const next = Math.min(cursor + 1, last);
  return { cursor: next, atEnd: next >= last };
}

/** A single trade leg to mark on the chart: a bar index + buy/sell direction. */
export interface TradeLeg {
  index: number;
  kind: 'buy' | 'sell';
  leg: 'entry' | 'exit';
}

/**
 * Flatten closed trades into chart legs. A trade has an entry and an exit; the
 * buy/sell direction follows the side, like a trading terminal:
 *   LONG  -> entry = buy,  exit = sell
 *   SHORT -> entry = sell, exit = buy
 * Times are mapped to bar indices via `timeToIndex` (a trade's entryTime/exitTime
 * are candle `t` values); legs whose time is not in the map are dropped. Pure.
 */
export function tradeLegs(
  trades: { entryTime: number; exitTime: number; side: 'LONG' | 'SHORT' }[],
  timeToIndex: Map<number, number>,
): TradeLeg[] {
  const out: TradeLeg[] = [];
  for (const t of trades) {
    const ei = timeToIndex.get(t.entryTime);
    const xi = timeToIndex.get(t.exitTime);
    if (ei != null) out.push({ index: ei, kind: t.side === 'LONG' ? 'buy' : 'sell', leg: 'entry' });
    if (xi != null) out.push({ index: xi, kind: t.side === 'LONG' ? 'sell' : 'buy', leg: 'exit' });
  }
  return out;
}

/** The open position at candle time `t` (Slice 6-3 replay readout): the side of a
 *  closed trade whose [entryTime, exitTime] covers `t`, else 'FLAT'. Bounds are
 *  inclusive so the entry and exit bars read as in-position. Pure. */
export function positionAtTime(
  trades: { entryTime: number; exitTime: number; side: 'LONG' | 'SHORT' }[],
  t: number,
): 'LONG' | 'SHORT' | 'FLAT' {
  for (const tr of trades) {
    if (tr.entryTime <= t && t <= tr.exitTime) return tr.side;
  }
  return 'FLAT';
}
