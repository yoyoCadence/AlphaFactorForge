import { describe, expect, it } from 'vitest';
import { SKIN_ORDER, THEMES, type ChartTheme } from './theme';

// The chart draws MA fast, MA slow and EMA at the same time, over candles that
// are already `up`/`down` coloured. Adding a skin is meant to be "copy the
// nearest theme and change the tokens", which makes it easy to leave two line
// colours identical (or leave the Bollinger envelope indistinguishable from the
// grid it sits on) and only find out by staring at a chart. These lock the
// separation numerically instead.

/** '#rgb' / '#rrggbb' -> [r,g,b]; null for the rgba() values some skins use. */
function rgb(value: string): [number, number, number] | null {
  if (!value.startsWith('#')) return null;
  const s = value.slice(1);
  const n = s.length === 3 ? s.split('').map((c) => c + c).join('') : s;
  return [parseInt(n.slice(0, 2), 16), parseInt(n.slice(2, 4), 16), parseInt(n.slice(4, 6), 16)];
}

function distance(a: string, b: string): number | null {
  const x = rgb(a);
  const y = rgb(b);
  if (!x || !y) return null; // translucent token — contrast is against whatever is behind it
  return Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]);
}

/** Below this two lines read as "the same colour" at 1–2px stroke width. */
const MIN_LINE_SEPARATION = 60;
/** The envelope may be quiet, but not so quiet it merges into the gridlines. */
const MIN_GRID_SEPARATION = 40;

describe.each(SKIN_ORDER)('skin %s chart palette', (id) => {
  const C: ChartTheme = THEMES[id].chart;

  it('keeps the three overlay lines apart from each other', () => {
    const lines: [string, string][] = [
      ['ma1', C.ma1],
      ['ma2', C.ma2],
      ['ema', C.ema],
    ];
    for (let i = 0; i < lines.length; i++) {
      for (let j = i + 1; j < lines.length; j++) {
        const d = distance(lines[i][1], lines[j][1]);
        if (d == null) continue;
        expect(d, `${lines[i][0]} vs ${lines[j][0]}`).toBeGreaterThanOrEqual(MIN_LINE_SEPARATION);
      }
    }
  });

  it('keeps the EMA line off the candle up/down colours', () => {
    for (const [name, value] of [['up', C.up], ['down', C.down]] as const) {
      const d = distance(C.ema, value);
      if (d == null) continue;
      expect(d, `ema vs ${name}`).toBeGreaterThanOrEqual(MIN_LINE_SEPARATION);
    }
  });

  it('keeps the Bollinger envelope readable against the grid', () => {
    const d = distance(C.bb, C.grid);
    if (d == null) return; // aurora-glass paints both as rgba over a gradient
    expect(d).toBeGreaterThanOrEqual(MIN_GRID_SEPARATION);
  });
});
