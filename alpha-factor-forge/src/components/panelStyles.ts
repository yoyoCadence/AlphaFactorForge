// Shared inline-style tokens for the backtest panel and its extracted sections
// (SweepSection, …). Kept as one object so the terminal-dense look stays
// consistent across the split. Verbatim from the original BacktestPanel `S`.
import type * as React from 'react';

export const S = {
  panel: { display: 'grid', gridTemplateColumns: '380px 1fr', gap: 16, alignItems: 'start' } as React.CSSProperties,
  card: { border: '1px solid #d6d2c8', background: '#fff', padding: 12 } as React.CSSProperties,
  h2: { fontSize: 12, fontWeight: 700, margin: '0 0 8px', letterSpacing: '0.04em', color: '#16150f' } as React.CSSProperties,
  label: { fontSize: 10, color: '#8a8678' } as React.CSSProperties,
  input: {
    width: '100%', padding: '5px 7px', border: '1px solid #d6d2c8', background: '#fff',
    fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, color: '#16150f', outline: 'none',
  } as React.CSSProperties,
  btn: {
    fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, fontWeight: 600,
    padding: '6px 10px', border: '1px solid #16150f', background: '#16150f', color: '#fff', cursor: 'pointer',
  } as React.CSSProperties,
  btnGhost: {
    fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, fontWeight: 600,
    padding: '6px 10px', border: '1px solid #d6d2c8', background: '#efece5', color: '#16150f', cursor: 'pointer',
  } as React.CSSProperties,
  grid3: { display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 7 } as React.CSSProperties,
};
