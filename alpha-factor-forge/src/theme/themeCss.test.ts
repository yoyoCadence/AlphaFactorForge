import { describe, expect, it } from 'vitest';
import { SKIN_ORDER, THEMES } from './theme';
import { themeToCssVars } from './themeCss';

// styles.css reads the --afs-* variables with the forge-paper values as
// fallbacks, so a skin that silently fails to emit one does not break — it
// quietly renders forge-paper's value instead, which is the hardest kind of
// skin bug to notice. Lock the key set across every theme.

const BASELINE = Object.keys(themeToCssVars(THEMES['forge-paper'])).sort();

describe('themeToCssVars', () => {
  it('emits a non-trivial set of variables', () => {
    expect(BASELINE.length).toBeGreaterThan(40);
    expect(BASELINE.every((k) => k.startsWith('--afs-'))).toBe(true);
    expect(BASELINE).not.toContain('--afs-surface');
  });

  it.each(SKIN_ORDER)('%s emits exactly the same keys', (id) => {
    expect(Object.keys(themeToCssVars(THEMES[id])).sort()).toEqual(BASELINE);
  });

  it.each(SKIN_ORDER)('%s leaves no variable empty', (id) => {
    for (const [key, value] of Object.entries(themeToCssVars(THEMES[id]))) {
      expect(value, key).toBeTruthy();
    }
  });

  it.each(SKIN_ORDER)('%s emits the workspace and accent-wash variables', (id) => {
    const vars = themeToCssVars(THEMES[id]);
    expect(vars['--afs-accent-wash']).toBe(THEMES[id].color.accentWash);
    expect(vars['--afs-workspace-image']).toBe(THEMES[id].workspaceBackground.image);
    expect(vars['--afs-workspace-scrim']).toBe(THEMES[id].workspaceBackground.scrim);
    expect(vars['--afs-workspace-position']).toBe(THEMES[id].workspaceBackground.position);
    expect(vars['--afs-workspace-size']).toBe(THEMES[id].workspaceBackground.size);
    expect(vars['--afs-workspace-repeat']).toBe(THEMES[id].workspaceBackground.repeat);
  });
});
