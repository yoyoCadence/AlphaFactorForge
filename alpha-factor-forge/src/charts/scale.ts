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
