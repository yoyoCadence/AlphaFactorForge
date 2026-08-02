import React, { useEffect, useState } from 'react';
import { CandleChart } from '../charts/CandleChart';
import {
  mergeChartSnapshot,
  popoutWindows,
  type ChartWindowSnapshot,
} from '../tauri-client/windowBridge';
import { useTheme } from '../theme/ThemeProvider';

export function ChartPopoutWindow(): React.ReactElement {
  const t = useTheme();
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
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden', background: t.color.bg, color: t.color.ink, fontFamily: t.font.sans }}>
      <header style={{ height: 46, flexShrink: 0, display: 'flex', alignItems: 'center', gap: 10, padding: '0 14px', background: t.header.bg, borderBottom: `1px solid ${t.header.border}` }}>
        <div style={{ width: 12, height: 12, background: t.header.markBg, transform: `rotate(${t.header.markRotate})`, borderRadius: t.header.markRadius }} />
        <strong>ALPHAFACTORFORGE /chart</strong>
        <span style={{ marginLeft: 'auto', color: t.header.muted, fontFamily: t.font.mono, fontSize: 11 }}>{snapshot?.title ?? '等待主視窗資料…'}</span>
      </header>
      <main style={{ flex: 1, minHeight: 0, padding: 8 }}>
        {error && <div style={{ padding: 12, color: t.color.danger }}>{error}</div>}
        {!error && !snapshot && <div data-testid="chart-window-loading" style={{ padding: 12, color: t.color.muted }}>正在同步圖表資料…</div>}
        {!error && snapshot && snapshot.candles.length === 0 && <div style={{ padding: 12, color: t.color.muted }}>主視窗尚未載入資料集。</div>}
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
