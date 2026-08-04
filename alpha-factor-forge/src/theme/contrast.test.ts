import { describe, expect, it } from 'vitest';
import { SKIN_ORDER, THEMES, type Theme } from './theme';

// Text contrast is the one skin property that cannot be eyeballed reliably —
// a muted grey that reads fine on the skin you designed fails on the next one.
// The original handoff flagged two skins as below AA; an audit found three, and
// the same value was also sitting in `header.muted` and `chart.label`, so a fix
// applied to `color.muted` alone would have left the header status line and the
// canvas axis labels behind. These lock the outcome for every skin at once.

function channels(colour: string): [number, number, number] | null {
  if (colour.startsWith('#')) {
    const h = colour.slice(1);
    const n = h.length === 3 ? h.split('').map((c) => c + c).join('') : h;
    return [0, 2, 4].map((i) => parseInt(n.slice(i, i + 2), 16)) as [number, number, number];
  }
  return null; // rgba() / gradient: contrast depends on what is behind it
}

function relativeLuminance(rgb: [number, number, number]): number {
  const [r, g, b] = rgb.map((v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Contrast ratio, or null when either colour is not an opaque hex. */
function contrast(fg: string, bg: string): number | null {
  const a = channels(fg);
  const b = channels(bg);
  if (!a || !b) return null;
  const [hi, lo] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** WCAG AA for normal text. Every token below carries text at 10-13px. */
const AA = 4.5;

/** `faint` is the de-emphasised half of a pair — the entry/exit ✗ against a bold
 *  coloured ✓, and the `/backtest` suffix after the product name. It is not
 *  carrying prose any more (that moved to `muted`), so the UI-component bar
 *  applies rather than the body-text one. Kept apart from `muted` on purpose:
 *  lifting it to AA would put the two on the same level and flatten the
 *  ink → muted → faint hierarchy into two steps. */
const DE_EMPHASIS = 3.0;

/** swiss-forge's identity colour is a signal red (#e63329); white on it is
 *  4.31:1. That is a property of the design's own primary, not of the token
 *  port — raising it means picking a different red, which is a design decision
 *  and not one to make inside a contrast fix. Named here so it stays visible
 *  instead of being quietly dropped from the suite. */
const ACCENT_PAIR_BELOW_AA: ReadonlySet<string> = new Set(['swiss-forge']);
const ACCENT_PAIR_CASES = new Set(['accentInk on accent', 'primaryInk on primaryBg']);

describe.each(SKIN_ORDER)('skin %s contrast', (id) => {
  const t: Theme = THEMES[id];

  const cases: [string, string, string][] = [
    ['ink on cardBg', t.color.ink, t.color.cardBg],
    ['muted on cardBg', t.color.muted, t.color.cardBg],
    ['muted on bg', t.color.muted, t.color.bg],
    ['header.muted on header.bg', t.header.muted, t.header.bg],
    ['chart.label on chart.bg', t.chart.label, t.chart.bg],
    // The sweep-applied field keeps `ink` from S.input on the accent wash.
    ['ink on accentWash', t.color.ink, t.color.accentWash],
    ['accentInk on accent', t.color.accentInk, t.color.accent],
    ['primaryInk on primaryBg', t.button.primaryInk, t.button.primaryBg],
  ];

  it.each(cases)('%s meets AA', (what, fg, bg) => {
    const ratio = contrast(fg, bg);
    if (ratio == null) return; // translucent skin surface — nothing to assert against
    if (ACCENT_PAIR_BELOW_AA.has(id) && ACCENT_PAIR_CASES.has(what)) {
      // Still asserted, just at the level the design's red actually reaches, so
      // a regression below it is caught even though AA is out of reach.
      expect(ratio).toBeGreaterThanOrEqual(4.3);
      return;
    }
    expect(ratio).toBeGreaterThanOrEqual(AA);
  });

  // faint survives on exactly two surfaces: the chart card (the entry/exit
  // crosses) and the header (the /backtest suffix).
  it.each([
    ['faint on cardBg', t.color.faint, t.color.cardBg],
    ['faint on header.bg', t.color.faint, t.header.bg],
  ])('%s stays perceivable', (_what, fg, bg) => {
    const ratio = contrast(fg, bg);
    if (ratio == null) return;
    expect(ratio).toBeGreaterThanOrEqual(DE_EMPHASIS);
  });

  it('keeps three distinct steps of ink -> muted -> faint', () => {
    const ink = contrast(t.color.ink, t.color.cardBg);
    const muted = contrast(t.color.muted, t.color.cardBg);
    const faint = contrast(t.color.faint, t.color.cardBg);
    if (ink == null || muted == null || faint == null) return;
    // Each step has to stay visibly weaker than the one above it. Without this,
    // a future contrast fix could quietly raise `faint` to `muted`'s level and
    // leave the palette with two tones pretending to be three.
    expect(ink / muted).toBeGreaterThanOrEqual(1.5);
    expect(muted / faint).toBeGreaterThanOrEqual(1.2);
  });
});
