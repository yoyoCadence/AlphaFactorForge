import React, { useEffect, useState } from 'react';
import { CandleChart } from '../charts/CandleChart';
import {
  mergeChartSnapshot,
  popoutWindows,
  type ChartWindowSnapshot,
} from '../tauri-client/windowBridge';

export function ChartPopoutWindow(): React.ReactElement {
  const [snapshot, setSnapshot] = useState<ChartWindowSnapshot | null>(null);
  const [height, setHeight] = useState(() => Math.max(320, globalThis.innerHeight - 54));
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const resize = () => setHeight(Math.max(320, globalThis.innerHeight - 54));
    globalThis.addEventListener('resize', resize);
    return () => globalThis.removeEventListener('resize', resize);
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlistenSnapshot: (() => void) | undefined;
    let unlistenCursor: (() => void) | undefined;
    Promise.all([
      popoutWindows.onChartSnapshot((incoming) => {
        if (!disposed) setSnapshot((current) => mergeChartSnapshot(current, incoming));
      }),
      popoutWindows.onChartCursor((cursor) => {
        if (!disposed) setSnapshot((current) => current ? { ...current, upto: cursor.upto } : current);
      }),
    ])
      .then(([snapshotOff, cursorOff]) => {
        if (disposed) {
          snapshotOff();
          cursorOff();
          return;
        }
        unlistenSnapshot = snapshotOff;
        unlistenCursor = cursorOff;
        return popoutWindows.signalChartReady();
      })
      .catch((e) => !disposed && setError(String(e)));
    return () => {
      disposed = true;
      unlistenSnapshot?.();
      unlistenCursor?.();
    };
  }, []);

  return (
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden', background: '#eceae4', color: '#16150f', fontFamily: "'IBM Plex Sans', system-ui, sans-serif" }}>
      <header style={{ height: 46, flexShrink: 0, display: 'flex', alignItems: 'center', gap: 10, padding: '0 14px', background: '#fff', borderBottom: '1px solid #d6d2c8' }}>
        <div style={{ width: 12, height: 12, background: '#16150f', transform: 'rotate(45deg)' }} />
        <strong>ALPHAFACTORFORGE /chart</strong>
        <span style={{ marginLeft: 'auto', color: '#8a8678', fontFamily: "'IBM Plex Mono', monospace", fontSize: 11 }}>{snapshot?.title ?? '等待主視窗資料…'}</span>
      </header>
      <main style={{ flex: 1, minHeight: 0, padding: 8 }}>
        {error && <div style={{ padding: 12, color: '#b23b2e' }}>{error}</div>}
        {!error && !snapshot && <div data-testid="chart-window-loading" style={{ padding: 12, color: '#8a8678' }}>正在同步圖表資料…</div>}
        {!error && snapshot && snapshot.candles.length === 0 && <div style={{ padding: 12, color: '#8a8678' }}>主視窗尚未載入資料集。</div>}
        {!error && snapshot && snapshot.candles.length > 0 && (
          <CandleChart
            candles={snapshot.candles}
            strat={snapshot.strat}
            show={snapshot.show}
            trades={snapshot.trades}
            upto={snapshot.upto}
            height={height}
          />
        )}
      </main>
    </div>
  );
}
