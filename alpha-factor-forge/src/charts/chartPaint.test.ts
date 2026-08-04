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

// WCAG AA for normal text. Reachable only because the ink pair is pure
// black/white: the design mock's #111111 / #f2f2f2 tops out at ~4.09:1 on a
// mid-tone fill, and the ramp does land there (4.112 at broadsheet t=0.77).
// Measured floor with the current pair is 4.587, so this has ~0.09 of headroom
// — a new skin whose accent lands mid-tone is exactly what this catches.
const MIN_CONTRAST = 4.5;

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
