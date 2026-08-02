import React, { useEffect, useState } from 'react';
import { popoutWindows, type MetricsWindowSnapshot } from '../tauri-client/windowBridge';
import { MetricsTable } from './MetricsTable';
import { useTheme } from '../theme/ThemeProvider';

export function MetricsPopoutWindow(): React.ReactElement {
  const t = useTheme();
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
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden', background: t.color.bg, color: t.color.ink, fontFamily: t.font.sans }}>
      <header style={{ height: 46, flexShrink: 0, display: 'flex', alignItems: 'center', gap: 10, padding: '0 14px', background: t.header.bg, borderBottom: `1px solid ${t.header.border}` }}>
        <div style={{ width: 12, height: 12, background: t.header.markBg, transform: `rotate(${t.header.markRotate})`, borderRadius: t.header.markRadius }} />
        <strong>ALPHAFACTORFORGE /metrics</strong>
        <span style={{ marginLeft: 'auto', color: t.header.muted, fontFamily: t.font.mono, fontSize: 11 }}>{snapshot?.title ?? '等待回測結果…'}</span>
      </header>
      <main style={{ flex: 1, minHeight: 0, overflow: 'auto', padding: 16 }}>
        {error && <div style={{ padding: 12, color: t.color.danger }}>{error}</div>}
        {!error && !snapshot && <div data-testid="metrics-window-loading" style={{ padding: 12, color: t.color.muted }}>正在等待主視窗的績效資料…</div>}
        {!error && snapshot && (
          <section style={{ padding: 16, background: t.color.cardBg, border: `${t.shape.borderWidth}px ${t.shape.borderStyle} ${t.color.line}`, borderRadius: 4 }}>
            <MetricsTable data={snapshot} fontSize={15} />
          </section>
        )}
      </main>
    </div>
  );
}
