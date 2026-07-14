import React, { useEffect, useState } from 'react';
import { popoutWindows, type MetricsWindowSnapshot } from '../tauri-client/windowBridge';
import { MetricsTable } from './MetricsTable';

export function MetricsPopoutWindow(): React.ReactElement {
  const [snapshot, setSnapshot] = useState<MetricsWindowSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    popoutWindows.onMetricsSnapshot((incoming) => {
      if (!disposed) setSnapshot(incoming);
    })
      .then((off) => {
        if (disposed) {
          off();
          return;
        }
        unlisten = off;
        return popoutWindows.signalMetricsReady();
      })
      .catch((e) => !disposed && setError(String(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden', background: '#eceae4', color: '#16150f', fontFamily: "'IBM Plex Sans', system-ui, sans-serif" }}>
      <header style={{ height: 46, flexShrink: 0, display: 'flex', alignItems: 'center', gap: 10, padding: '0 14px', background: '#fff', borderBottom: '1px solid #d6d2c8' }}>
        <div style={{ width: 12, height: 12, background: '#16150f', transform: 'rotate(45deg)' }} />
        <strong>ALPHAFACTORFORGE /metrics</strong>
        <span style={{ marginLeft: 'auto', color: '#8a8678', fontFamily: "'IBM Plex Mono', monospace", fontSize: 11 }}>{snapshot?.title ?? '等待回測結果…'}</span>
      </header>
      <main style={{ flex: 1, minHeight: 0, overflow: 'auto', padding: 16 }}>
        {error && <div style={{ padding: 12, color: '#b23b2e' }}>{error}</div>}
        {!error && !snapshot && <div data-testid="metrics-window-loading" style={{ padding: 12, color: '#8a8678' }}>正在等待主視窗的績效資料…</div>}
        {!error && snapshot && (
          <section style={{ padding: 16, background: '#fff', border: '1px solid #d6d2c8', borderRadius: 4 }}>
            <MetricsTable data={snapshot} fontSize={15} />
          </section>
        )}
      </main>
    </div>
  );
}
