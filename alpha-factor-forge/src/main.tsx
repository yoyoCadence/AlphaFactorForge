// App shell for the Tauri frontend. Boots the DB bridge, then hosts the ported
// UI. Slice 2 ships the single-strategy Backtest panel; later slices add the
// chart, sweep, replay, live, and library tabs (see tasks.md UI-port plan).
// The PR #1 bridge self-test lived here as a temporary harness and is now
// superseded by the real Backtest panel.

import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { db, isTauri } from './tauri-client/dataClient';
import { BacktestPanel } from './components/BacktestPanel';

function App(): React.ReactElement {
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
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column', background: '#eceae4', color: '#16150f', fontFamily: "'IBM Plex Sans', system-ui, sans-serif", fontSize: 13 }}>
      <header style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '0 14px', height: 50, background: '#fff', borderBottom: '1px solid #d6d2c8', flexShrink: 0 }}>
        <div style={{ width: 16, height: 16, background: '#16150f', transform: 'rotate(45deg)' }} />
        <div style={{ fontWeight: 700, fontSize: 15, letterSpacing: '0.01em' }}>
          ALPHAFACTORFORGE
          <span style={{ color: '#aaa599', fontWeight: 500, fontSize: 13 }}> /backtest</span>
        </div>
        <div style={{ flex: 1 }} />
        <div style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 11, color: '#8a8678' }}>{status}</div>
      </header>

      <main style={{ flex: 1, overflow: 'auto', padding: 16 }}>
        <BacktestPanel />
      </main>
    </div>
  );
}

const el = document.getElementById('root');
if (el) createRoot(el).render(<App />);
