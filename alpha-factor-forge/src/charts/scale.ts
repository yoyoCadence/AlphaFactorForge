// Tiny pure helpers for chart scaling. Kept separate from the canvas component
// so the numeric logic is unit-testable (canvas drawing is not).

export interface Extent {
  min: number;
  max: number;
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
  if (min === max) return { min: min - 1, max: max + 1 };
  return { min, max };
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
