// App shell for the Tauri frontend. Boots the DB bridge, then hosts the ported
// UI. Slice 2 ships the single-strategy Backtest panel; later slices add the
// chart, sweep, replay, live, and library tabs (see tasks.md UI-port plan).
// The PR #1 bridge self-test lived here as a temporary harness and is now
// superseded by the real Backtest panel.

import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { db, isTauri } from './tauri-client/dataClient';
import { BacktestPanel } from './components/BacktestPanel';
import { ChartPopoutWindow } from './components/ChartPopoutWindow';
import { MetricsPopoutWindow } from './components/MetricsPopoutWindow';
import { SkinPicker } from './components/SkinPicker';
import { ThemeProvider, useTheme } from './theme/ThemeProvider';
import './styles.css';

function App(): React.ReactElement {
  const t = useTheme();
  const [status, setStatus] = useState<string>('booting…');

  useEffect(() => {
    if (!isTauri()) {
      setStatus('running OUTSIDE Tauri (browser) — backend commands unavailable');
      return;
    }
    db.init()
      .then((s) => setStatus(s))
      .catch((e) => setStatus(`bridge error: ${String(e)}`));
  }, []);

  return (
    <div className="app-shell" style={{ height: '100vh', display: 'flex', flexDirection: 'column', background: 'transparent', color: t.color.ink, fontFamily: t.font.sans, fontSize: t.font.size }}>
      {/* Header height is a skin token (46–68px): the broadsheet skin's masthead
          and the terminal skin's tight bar are part of their design, not spare
          padding. Safe to vary because <main> below is flex:1. */}
      <header className="app-header" style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '0 14px', height: t.header.height, background: t.header.bg, borderBottom: `1px solid ${t.header.border}`, flexShrink: 0 }}>
        <div style={{ width: 16, height: 16, background: t.header.markBg, transform: `rotate(${t.header.markRotate})`, borderRadius: t.header.markRadius }} />
        <div style={{ fontWeight: 700, fontSize: 15, letterSpacing: '0.01em', color: t.header.ink }}>
          ALPHAFACTORFORGE
          <span style={{ color: t.color.faint, fontWeight: 500, fontSize: 13 }}> /backtest</span>
        </div>
        <div style={{ flex: 1 }} />
        <div className="app-header-status" title={status} style={{ minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontFamily: t.font.mono, fontSize: t.font.labelSize, color: t.header.muted }}>{status}</div>
        <SkinPicker />
      </header>

      <main className="app-workspace" style={{ flex: 1, overflow: 'auto', padding: 16 }}>
        <BacktestPanel />
      </main>
    </div>
  );
}

const el = document.getElementById('root');
const childWindow = new URLSearchParams(globalThis.location?.search ?? '').get('window');
const root = childWindow === 'chart'
  ? <ChartPopoutWindow />
  : childWindow === 'metrics'
    ? <MetricsPopoutWindow />
    : <App />;
// Skin PR-A: every tree (main window + both pop-outs) mounts under the same
// provider so the theme tokens / font loading are identical across windows.
if (el) createRoot(el).render(<ThemeProvider>{root}</ThemeProvider>);
