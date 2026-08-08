// Theme -> CSS custom properties.
//
// Drop-in target: alpha-factor-forge/src/theme/themeCss.ts
//
// Why both CSS vars and the TS object: src/styles.css (global button/focus
// rules) and any future stylesheet can only read vars, while the inline-style
// components read the Theme object. One source, two consumers — never two
// copies of a hex code.

import type { Theme } from './theme';

export function themeToCssVars(t: Theme): Record<string, string> {
  const c = t.color, f = t.font, s = t.shape, b = t.button, h = t.header, k = t.track, w = t.workspaceBackground;
  return {
    '--afs-bg': c.bg,
    '--afs-surface2': c.surface2,
    '--afs-card-bg': c.cardBg,
    '--afs-ink': c.ink,
    '--afs-muted': c.muted,
    '--afs-faint': c.faint,
    '--afs-line': c.line,
    '--afs-line2': c.line2,
    '--afs-accent': c.accent,
    '--afs-accent-ink': c.accentInk,
    '--afs-accent-wash': c.accentWash,
    '--afs-ok': c.ok,
    '--afs-warn': c.warn,
    '--afs-danger': c.danger,

    '--afs-workspace-image': w.image,
    '--afs-workspace-scrim': w.scrim,
    '--afs-workspace-position': w.position,
    '--afs-workspace-size': w.size,
    '--afs-workspace-repeat': w.repeat,

    '--afs-sans': f.sans,
    '--afs-mono': f.mono,
    '--afs-fs': f.size + 'px',
    '--afs-label-fs': f.labelSize + 'px',
    '--afs-title-fs': f.titleSize + 'px',
    '--afs-label-tt': f.labelTransform,
    '--afs-label-ls': f.labelTracking,
    '--afs-label-w': String(f.labelWeight),
    '--afs-title-tt': f.titleTransform,
    '--afs-title-ls': f.titleTracking,
    '--afs-title-w': String(f.titleWeight),

    '--afs-r': s.radius + 'px',
    '--afs-r-sm': s.radiusSm + 'px',
    '--afs-bw': s.borderWidth + 'px',
    '--afs-bstyle': s.borderStyle,
    '--afs-pad': s.pad + 'px',
    '--afs-gap': s.gap + 'px',
    '--afs-shadow': s.shadow,
    '--afs-card-filter': s.backdrop ?? 'none',
    '--afs-field-bg': s.fieldBg,
    '--afs-field-r': s.fieldRadius + 'px',
    '--afs-btn-r': s.btnRadius + 'px',
    '--afs-btn-pad': s.btnPad,
    '--afs-btn-tt': s.btnTransform,
    '--afs-btn-w': String(s.btnWeight),
    '--afs-btn-shadow': s.btnShadow,
    '--afs-chip-r': s.chipRadius + 'px',
    '--afs-tab-r': s.tabRadius + 'px',
    '--afs-row-line': s.rowLine,

    '--afs-primary-bg': b.primaryBg,
    '--afs-primary-ink': b.primaryInk,
    '--afs-ghost-bg': b.ghostBg,
    '--afs-ghost-ink': b.ghostInk,
    '--afs-ghost-line': b.ghostLine,

    '--afs-header-bg': h.bg,
    '--afs-header-ink': h.ink,
    '--afs-header-muted': h.muted,
    '--afs-header-h': h.height + 'px',
    '--afs-header-border': h.border,
    '--afs-mark-bg': h.markBg,
    '--afs-mark-r': h.markRadius,
    '--afs-mark-rot': h.markRotate,

    '--afs-track-bg': k.bg,
    '--afs-track-pad': k.pad + 'px',
    '--afs-track-r': k.radius + 'px',
    '--afs-track-gap': k.gap + 'px',
    '--afs-track-ub': k.underline,

    '--afs-idx-display': t.sectionIndex,
  };
}

/** Write the vars onto an element (app root in the Tauri window, or
 *  document.documentElement for pop-out windows that render standalone). */
export function applyTheme(el: HTMLElement, t: Theme): void {
  const vars = themeToCssVars(t);
  Object.keys(vars).forEach((k) => el.style.setProperty(k, vars[k]));
  el.dataset.skin = t.id;
  el.dataset.mode = t.mode;
}
