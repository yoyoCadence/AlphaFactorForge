// Themed replacement for src/components/panelStyles.ts.
//
// Drop-in target: alpha-factor-forge/src/components/panelStyles.ts
//
// The exported shape is IDENTICAL to today's `S` object, plus new members, so
// the port is mechanical:
//
//   -import { S } from './panelStyles';
//   +import { makeStyles } from './panelStyles';
//   +const S = makeStyles(useTheme());          // inside the component body
//
// Everything else in each section component stays as-is. Memoise per theme so
// the object identity is stable across renders.

import type * as React from 'react';
import type { Theme } from '../theme/theme';

export interface PanelStyles {
  panel: React.CSSProperties;
  card: React.CSSProperties;
  h2: React.CSSProperties;
  label: React.CSSProperties;
  input: React.CSSProperties;
  btn: React.CSSProperties;
  btnGhost: React.CSSProperties;
  grid3: React.CSSProperties;
  /** New: two-state controls (interval chips, indicator toggles, tabs). */
  chip: (on: boolean) => React.CSSProperties;
  tab: (on: boolean) => React.CSSProperties;
  tabTrack: React.CSSProperties;
  /** New: table row rule + muted cell, so MetricsTable stops hard-coding hexes. */
  tableRow: React.CSSProperties;
  tableHead: React.CSSProperties;
  /** New: banner colours for the err/msg cards in BacktestPanel. */
  banner: (kind: 'error' | 'ok' | 'warn') => React.CSSProperties;
}

const cache = new WeakMap<Theme, PanelStyles>();

export function makeStyles(t: Theme): PanelStyles {
  const hit = cache.get(t);
  if (hit) return hit;

  const { color: c, font: f, shape: s, button: b } = t;
  const border = `${s.borderWidth}px ${s.borderStyle} ${c.line}`;

  const base: React.CSSProperties = {
    fontFamily: f.mono,
    fontSize: f.size,
    fontWeight: s.btnWeight,
    textTransform: s.btnTransform,
    letterSpacing: f.labelTracking,
    padding: s.btnPad,
    borderRadius: s.btnRadius,
    boxShadow: s.btnShadow,
    cursor: 'pointer',
  };

  const out: PanelStyles = {
    panel: {
      display: 'grid',
      gridTemplateColumns: 'minmax(300px, 380px) minmax(0, 1fr)',
      gap: s.gap,
      alignItems: 'start',
      width: '100%',
      minWidth: 0,
    },
    card: {
      border,
      borderRadius: s.radius,
      background: c.cardBg,
      boxShadow: s.shadow,
      backdropFilter: s.backdrop,
      WebkitBackdropFilter: s.backdrop,
      padding: s.pad,
    },
    h2: {
      fontFamily: f.sans,
      fontSize: f.titleSize,
      fontWeight: f.titleWeight,
      letterSpacing: f.titleTracking,
      textTransform: f.titleTransform,
      color: c.ink,
      margin: '0 0 8px',
    },
    label: {
      fontFamily: f.sans,
      fontSize: f.labelSize,
      fontWeight: f.labelWeight,
      letterSpacing: f.labelTracking,
      textTransform: f.labelTransform,
      color: c.muted,
    },
    input: {
      width: '100%',
      padding: '5px 7px',
      border: `${s.borderWidth}px solid ${c.line}`,
      borderRadius: s.fieldRadius,
      background: s.fieldBg,
      fontFamily: f.mono,
      fontSize: f.size,
      color: c.ink,
    },
    btn: { ...base, border: `${s.borderWidth}px solid ${b.primaryBg}`, background: b.primaryBg, color: b.primaryInk },
    btnGhost: { ...base, border: `${s.borderWidth}px solid ${b.ghostLine}`, background: b.ghostBg, color: b.ghostInk },
    grid3: { display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 7 },

    chip: (on) => {
      const st = on ? t.states.chip.on : t.states.chip.off;
      return {
        ...base,
        padding: '5px 11px',
        fontSize: 11,
        borderRadius: s.chipRadius,
        background: st.bg,
        color: st.fg,
        border: `${s.borderWidth}px solid ${st.bd}`,
      };
    },
    tab: (on) => {
      const st = on ? t.states.tab.on : t.states.tab.off;
      return {
        flex: 1,
        padding: '8px 6px',
        fontFamily: f.sans,
        fontSize: f.size,
        fontWeight: 600,
        letterSpacing: f.labelTracking,
        textTransform: f.labelTransform,
        background: st.bg,
        color: st.fg,
        border: st.bd,
        borderBottom: st.ub ?? st.bd,
        borderRadius: s.tabRadius,
        cursor: 'pointer',
      };
    },
    tabTrack: {
      display: 'flex',
      gap: t.track.gap,
      background: t.track.bg,
      padding: t.track.pad,
      borderRadius: t.track.radius,
      borderBottom: t.track.underline,
    },
    tableRow: { borderBottom: `1px solid ${s.rowLine}` },
    tableHead: {
      padding: 4,
      textAlign: 'right',
      fontSize: f.labelSize,
      fontWeight: 600,
      letterSpacing: f.labelTracking,
      textTransform: f.labelTransform,
      color: c.muted,
    },
    banner: (kind) => ({
      border: `${s.borderWidth}px ${s.borderStyle} ${kind === 'error' ? c.danger : kind === 'ok' ? c.ok : c.warn}`,
      borderRadius: s.radius,
      background: c.cardBg,
      color: kind === 'error' ? c.danger : kind === 'ok' ? c.ok : c.warn,
      padding: s.pad,
      marginBottom: s.gap,
    }),
  };

  cache.set(t, out);
  return out;
}
