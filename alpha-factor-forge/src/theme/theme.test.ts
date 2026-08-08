import { describe, expect, it } from 'vitest';
import { DEFAULT_SKIN, SKIN_ORDER, THEMES, getTheme, isSkinId, type ChartTheme } from './theme';

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

const RASTER_BACKGROUND_SKINS = ['forge-paper', 'atelier-warm', 'signal-orange', 'broadsheet', 'aurora-glass'] as const;
const PATTERN_BACKGROUND_SKINS = ['midnight-tape', 'blueprint'] as const;
const FLAT_BACKGROUND_SKINS = ['swiss-forge', 'brutal-yellow', 'frost-grey'] as const;

describe('workspace background contract', () => {
  it.each(RASTER_BACKGROUND_SKINS)('%s uses a local WebP beneath a readability scrim', (id) => {
    const background = THEMES[id].workspaceBackground;
    expect(background.image).toMatch(/^url\(".*\.webp"\)$/);
    expect(background.image).not.toMatch(/https?:|data:/);
    expect(background.scrim).toContain('gradient');
    expect(background.size).toContain('cover');
  });

  it.each(PATTERN_BACKGROUND_SKINS)('%s uses a lightweight CSS pattern', (id) => {
    const background = THEMES[id].workspaceBackground;
    expect(background.image).toContain('gradient');
    expect(background.image).not.toContain('url(');
  });

  it.each(FLAT_BACKGROUND_SKINS)('%s stays intentionally flat', (id) => {
    expect(THEMES[id].workspaceBackground.image).toBe('none');
    expect(THEMES[id].workspaceBackground.scrim).toBe('none');
  });

  it('classifies every skin and keeps each workspace base colour opaque', () => {
    expect([
      ...RASTER_BACKGROUND_SKINS,
      ...PATTERN_BACKGROUND_SKINS,
      ...FLAT_BACKGROUND_SKINS,
    ].sort()).toEqual([...SKIN_ORDER].sort());
    for (const id of SKIN_ORDER) expect(THEMES[id].color.bg, id).toMatch(/^#[0-9a-f]{6}$/i);
  });
});

describe('skin id validation', () => {
  it('accepts only registered ids and falls back safely', () => {
    expect(isSkinId('blueprint')).toBe(true);
    expect(isSkinId('future-skin')).toBe(false);
    expect(isSkinId(null)).toBe(false);
    expect(getTheme('future-skin')).toBe(THEMES[DEFAULT_SKIN]);
  });
});
