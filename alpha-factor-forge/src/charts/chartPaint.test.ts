import { describe, expect, it } from 'vitest';
import { SKIN_ORDER, THEMES } from '../theme/theme';
import { heatColor, heatTextColor } from './chartPaint';

// The sweep heatmap fills each cell by mixing the skin's `surface2` toward its
// `accent`. `accent` is near-black in some skins and a bright green or cyan in
// others, so the same ramp position needs opposite ink depending on the skin —
// which is why heatTextColor reads the luminance of the fill it actually
// produced. Assert the outcome (readable text) rather than the mechanism.

function channels(rgbOrHex: string): [number, number, number] {
  if (rgbOrHex.startsWith('rgb')) {
    const [r, g, b] = rgbOrHex.slice(4, -1).split(',').map(Number);
    return [r, g, b];
  }
  const s = rgbOrHex.slice(1);
  const n = s.length === 3 ? s.split('').map((c) => c + c).join('') : s;
  return [parseInt(n.slice(0, 2), 16), parseInt(n.slice(2, 4), 16), parseInt(n.slice(4, 6), 16)];
}

/** WCAG relative luminance. */
function relativeLuminance(colour: string): number {
  const [r, g, b] = channels(colour).map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const RAMP_POSITIONS = [0, 0.25, 0.5, 0.54, 0.56, 0.75, 0.77, 1];

// Not 4.5 (AA). The design fixes the two inks at #111111 / #f2f2f2, and for a
// mid-tone fill sitting between them the best either can do is ~4.09:1 — a
// mathematical property of that pair, not a tuning miss. Measured worst case
// over all ten skins at 1% ramp steps is 4.112 (broadsheet, t=0.77). Swapping
// the pair for pure #000000 / #ffffff would raise the floor to 4.587 and clear
// AA; that is an owner call on the design values, not a code fix.
const MIN_CONTRAST = 4.0;

describe.each(SKIN_ORDER)('skin %s heatmap', (id) => {
  const C = THEMES[id].chart;

  it.each(RAMP_POSITIONS)('cell text is readable at t=%s', (t) => {
    const fill = heatColor(C, t);
    const ink = heatTextColor(C, t);
    expect(contrast(fill, ink), `${fill} on ${ink}`).toBeGreaterThanOrEqual(MIN_CONTRAST);
  });

  it('the ramp actually moves between its ends', () => {
    expect(heatColor(C, 0)).not.toEqual(heatColor(C, 1));
  });
});
