// Backtest-results section, extracted from BacktestPanel (REF-003, move-only).
//
// Owns the shared metrics table (full / in-sample / out-of-sample columns), the
// report export (JSON/CSV), the Slice 8a in-app pop-out, and the Slice 8b native
// metrics-window snapshot sync. The completed run and the save action stay in
// BacktestPanel and arrive as props.
//
// BUG-RESULT-CONTEXT-001: this section reads ONE immutable `CompletedRun`. The
// strategy, dataset, interval, range, metrics, and trades it renders, saves, and
// exports all come from that artifact's snapshots — never from the live editor
// or the live dataset selection, which may already describe a different run.

import React, { useEffect, useRef, useState } from 'react';
import { files } from '../tauri-client/dataClient';
import { popoutWindows, type MetricsWindowSnapshot } from '../tauri-client/windowBridge';
import { reportToJson, suggestedFilename, tradesToCsv } from '../services/reportExport';
import type { CompletedRun } from '../services/runArtifact';
import { HelpTip } from './HelpTip';
import { FloatingPanel } from './FloatingPanel';
import { MetricsTable } from './MetricsTable';
import { PoppedOutNote } from './PoppedOutNote';
import { makeStyles } from './panelStyles';
import { useTheme } from '../theme/ThemeProvider';

export interface ResultsSectionProps {
  /** The completed run, or null when no finished backtest matches the live
   *  inputs. Null is the only "no result actions" signal this section needs. */
  artifact: CompletedRun | null;
  /** True when a run HAS completed but its inputs have since changed — the
   *  metrics disappeared on purpose and the user needs to be told why. */
  stale: boolean;
  /** Current strategy name (used to label the exported JSON report). A display
   *  label only: it changes neither the result nor the durable strategy
   *  identity, so renaming after a run must not force a re-run. */
  stratName: string;
  saving: boolean;
  onSave: () => void;
  /** The panel's setErr / setMsg (stable). */
  onError: (message: string | null) => void;
  onMessage: (message: string | null) => void;
  help: { metrics: string; save: string };
}

export function ResultsSection({
  artifact,
  stale,
  stratName,
  saving,
  onSave,
  onError,
  onMessage,
  help,
}: ResultsSectionProps): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const [exporting, setExporting] = useState<'json' | 'csv' | null>(null);
  const [exportNotice, setExportNotice] = useState<{ kind: 'busy' | 'done'; text: string } | null>(null);
  const [poppedMetrics, setPoppedMetrics] = useState(false);
  const [nativeMetricsOpened, setNativeMetricsOpened] = useState(false);
  const [openingNativeMetrics, setOpeningNativeMetrics] = useState(false);

  const metricsWindowSnapshot: MetricsWindowSnapshot | null = artifact
    ? {
        // Dataset labelling comes from the artifact, so the title can never
        // advertise a dataset the metrics were not computed on.
        title: `${artifact.context.dataset.symbol} · ${artifact.context.dataset.interval}`,
        full: artifact.result.metrics,
        ...(artifact.holdoutResult
          ? { inSample: artifact.holdoutResult.inSample.metrics, outSample: artifact.holdoutResult.outSample.metrics }
          : {}),
      }
    : null;
  const metricsWindowSnapshotRef = useRef(metricsWindowSnapshot);
  metricsWindowSnapshotRef.current = metricsWindowSnapshot;

  // The child registers its listener first, then requests the latest snapshot,
  // avoiding the same open-vs-listen race handled by the chart window.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    popoutWindows.onMetricsReady(() => {
      void popoutWindows.publishMetrics(metricsWindowSnapshotRef.current).catch((e) => !disposed && onError(String(e)));
    })
      .then((off) => {
        if (disposed) off();
        else unlisten = off;
      })
      .catch((e) => !disposed && onError(String(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Once opened, keep the native view aligned with later backtests and Holdout
  // changes. Existing windows also receive an immediate snapshot when focused.
  useEffect(() => {
    if (!nativeMetricsOpened) return;
    void popoutWindows.publishMetrics(metricsWindowSnapshotRef.current).catch((e) => onError(String(e)));
  }, [nativeMetricsOpened, artifact]); // eslint-disable-line react-hooks/exhaustive-deps

  async function openNativeMetricsWindow() {
    if (!metricsWindowSnapshotRef.current) return;
    setOpeningNativeMetrics(true);
    onError(null);
    try {
      await popoutWindows.openMetrics();
      setNativeMetricsOpened(true);
      // Re-read after the async open/focus operation: a new backtest, dataset
      // switch, or Holdout change may have replaced or cleared the result.
      await popoutWindows.publishMetrics(metricsWindowSnapshotRef.current);
      onMessage('已開啟原生績效視窗；可拖曳到其他螢幕。');
    } catch (e) {
      onError(`無法開啟原生績效視窗：${String(e)}`);
    } finally {
      setOpeningNativeMetrics(false);
    }
  }

  async function exportResult(ext: 'json' | 'csv') {
    if (!artifact) return;
    setExporting(ext);
    onError(null);
    onMessage(null);
    setExportNotice({ kind: 'busy', text: `正在準備 ${ext.toUpperCase()} 下載...` });
    try {
      const at = Date.now();
      const snapshot = artifact.context.dataset;
      const dataset = {
        symbol: snapshot.symbol,
        interval: snapshot.interval,
        startTime: snapshot.startTime,
        endTime: snapshot.endTime,
      };
      const contents = ext === 'json'
        ? reportToJson({ strategyName: stratName, strategy: artifact.context.strategy, dataset, result: artifact.result, exportedAt: at })
        : tradesToCsv(artifact.result.trades);
      const path = await files.saveReport(suggestedFilename(dataset, ext, at), contents);
      onMessage(`已匯出 ${ext.toUpperCase()}：${path}`);
      setExportNotice({ kind: 'done', text: `${ext.toUpperCase()} 下載完成：${path}` });
    } catch (e) {
      setExportNotice(null);
      onError(String(e));
    } finally {
      setExporting(null);
    }
  }

  return (
    <>
      <section style={S.card}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, margin: '0 0 8px' }}>
          <h2 style={{ ...S.h2, margin: 0 }}>回測績效</h2>
          <HelpTip id="metrics" label="回測績效" text={help.metrics} />
          {artifact && popoutWindows.isAvailable() && (
            <button data-testid="native-popout-metrics" title="另開可移到其他螢幕的原生視窗" style={{ ...S.btnGhost, padding: '3px 10px', marginLeft: 'auto' }} onClick={openNativeMetricsWindow} disabled={openingNativeMetrics} aria-busy={openingNativeMetrics}>
              {openingNativeMetrics ? '開啟中…' : '↗ 新視窗'}
            </button>
          )}
          {artifact && (
            <button data-testid="popout-metrics" title="放大到獨立面板" style={{ ...S.btnGhost, padding: '3px 10px', marginLeft: popoutWindows.isAvailable() ? 0 : 'auto' }} onClick={() => setPoppedMetrics((v) => !v)}>
              {poppedMetrics ? '⤡ 收合' : '⤢ 放大'}
            </button>
          )}
        </div>
        {!artifact && (
          stale
            ? (
              <p data-testid="result-stale" style={{ color: t.color.warn, fontSize: 12 }}>
                策略、資料集或 Holdout 設定已變更 — 先前的回測結果已失效（不會被存檔或匯出）。請重新執行回測。
              </p>
            )
            : <p style={{ color: t.color.faint, fontSize: 12 }}>尚未回測 — 選資料集、設策略後按「執行回測」。</p>
        )}
        {artifact && (
          <>
            {poppedMetrics ? <PoppedOutNote label="回測績效" onClose={() => setPoppedMetrics(false)} /> : metricsWindowSnapshot && <MetricsTable data={metricsWindowSnapshot} fontSize={12} />}

            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 12, flexWrap: 'wrap' }}>
              <button data-testid="export-json" style={S.btnGhost} onClick={() => exportResult('json')} disabled={exporting != null} aria-busy={exporting === 'json'}>
                {exporting === 'json' ? '匯出 JSON 中...' : '匯出 JSON'}
              </button>
              <button data-testid="export-csv" style={S.btnGhost} onClick={() => exportResult('csv')} disabled={exporting != null} aria-busy={exporting === 'csv'}>
                {exporting === 'csv' ? '匯出 CSV 中...' : '匯出 CSV'}
              </button>
              {exportNotice && (
                <span
                  aria-live="polite"
                  data-testid="export-status"
                  style={{
                    color: exportNotice.kind === 'done' ? t.color.ok : t.color.warn,
                    fontFamily: t.font.mono,
                    fontSize: 11,
                  }}
                >
                  {exportNotice.text}
                </span>
              )}
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 12 }}>
              <button data-testid="save-result" style={{ ...S.btn, flex: 1 }} onClick={onSave} disabled={saving} aria-busy={saving}>
                {saving ? '儲存中…' : '儲存結果'}
              </button>
              <HelpTip id="save" label="儲存結果" text={help.save} align="right" />
            </div>
            <p style={{ color: t.color.faint, fontSize: 11, marginTop: 8 }}>
              儲存會寫入 strategy_def + backtest_summary + trades（segment=full）。
            </p>
          </>
        )}
      </section>

      {/* Slice 8a metrics pop-out: non-modal floating panel rendering the metrics
          enlarged; the rest of the app stays usable while it is open. */}
      {poppedMetrics && artifact && (
        <FloatingPanel title="回測績效" testId="metrics-popout" initial={{ x: 220, y: 130, w: 460, h: 520 }} onClose={() => setPoppedMetrics(false)}>
          {() => metricsWindowSnapshot && <MetricsTable data={metricsWindowSnapshot} fontSize={15} />}
        </FloatingPanel>
      )}
    </>
  );
}
