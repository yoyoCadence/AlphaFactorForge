// SKELETON — minimal frontend shell that BOOTS in Tauri and verifies the
// command bridge (DB init + dataset list). The full AlphaFactorForge research
// workspace UI (Dashboard / Chart / Builder / Discovery / Results / AI Lab /
// Data Manager / Settings) is ported in later — see TODO.md "UI port".

import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { db, isTauri, type Dataset } from './tauri-client/commands';

function App(): React.ReactElement {
  const [status, setStatus] = useState<string>('booting…');
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauri()) {
      setStatus('running OUTSIDE Tauri (browser) — backend commands unavailable');
      return;
    }
    (async () => {
      try {
        setStatus(await db.init());
        setDatasets(await db.getDatasets());
      } catch (e) {
        setErr(String(e));
        setStatus('bridge error');
      }
    })();
  }, []);

  return (
    <div style={{ fontFamily: 'ui-monospace, monospace', padding: 24, color: '#16150f' }}>
      <h1 style={{ fontSize: 18, margin: '0 0 4px' }}>AlphaFactorForge — Automated Indicator Discovery Workstation</h1>
      <p style={{ color: '#6b6862', margin: '0 0 16px' }}>status: {status}</p>
      {err && <pre style={{ color: '#b23b2e', whiteSpace: 'pre-wrap' }}>{err}</pre>}
      <h2 style={{ fontSize: 13 }}>datasets ({datasets.length})</h2>
      <ul>
        {datasets.map((d) => (
          <li key={d.id}>
            {d.symbol} · {d.interval} · {d.candle_count} candles
          </li>
        ))}
        {!datasets.length && <li style={{ color: '#aaa' }}>none yet — import via Data Manager (TODO)</li>}
      </ul>
    </div>
  );
}

const el = document.getElementById('root');
if (el) createRoot(el).render(<App />);
