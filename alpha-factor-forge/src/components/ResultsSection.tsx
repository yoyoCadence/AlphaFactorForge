// Backtest-results section, extracted from BacktestPanel (REF-003, move-only).
//
// Owns the metrics table (full / in-sample / out-of-sample columns), the report
// export (JSON/CSV) with its status line, the Slice 8a metrics pop-out, and their
// local state (exporting / exportNotice / poppedMetrics). The backtest result,
// holdout split, strategy, and the save action stay in BacktestPanel and arrive
// as props. Behaviour is identical to the pre-extraction inline block.

import React, { useState } from 'react';
import type { BacktestResult } from '../core/backtest';
import type { Metrics } from '../core/metrics';
import type { ParamsStrategy } from '../services/strategy';
import type { Dataset } from '../tauri-client/commands';
import { files } from '../tauri-client/dataClient';
import { reportToJson, suggestedFilename, tradesToCsv } from '../services/reportExport';
import { HelpTip } from './HelpTip';
import { FloatingPanel } from './FloatingPanel';
import { PoppedOutNote } from './PoppedOutNote';
import { S } from './panelStyles';

function pct(x: number): string {
  return `${(x * 100).toFixed(2)}%`;
}
function num(x: number): string {
  return Number.isFinite(x) ? x.toFixed(2) : '—';
}

const METRIC_ROWS: { label: string; fmt: (m: Metrics) => string }[] = [
  { label: '淨報酬', fmt: (m) => pct(m.netReturn) },
  { label: 'CAGR', fmt: (m) => pct(m.cagr) },
  { label: '最大回撤', fmt: (m) => pct(m.maxDrawdown) },
  { label: 'Sharpe', fmt: (m) => num(m.sharpe) },
  { label: 'Sortino', fmt: (m) => num(m.sortino) },
  { label: 'Calmar', fmt: (m) => num(m.calmar) },
  { label: '勝率', fmt: (m) => pct(m.winRate) },
  { label: '交易數', fmt: (m) => String(m.tradeCount) },
  { label: 'Profit Factor', fmt: (m) => (Number.isFinite(m.profitFactor) ? num(m.profitFactor) : '∞') },
  { label: '平均每筆', fmt: (m) => pct(m.avgTradeReturn) },
  { label: '曝險', fmt: (m) => pct(m.exposure) },
  { label: '換手', fmt: (m) => num(m.turnover) },
];

export interface ResultsSectionProps {
  result: BacktestResult | null;
  holdout: boolean;
  holdoutResult: { inSample: BacktestResult; outSample: BacktestResult } | null;
  selected: Dataset | null;
  strat: ParamsStrategy;
  /** Current strategy name (used to label the exported JSON report). The name
   *  input itself lives in the strategy card, not here. */
  stratName: string;
  saving: boolean;
  onSave: () => void;
  /** The panel's setErr / setMsg (stable). */
  onError: (message: string | null) => void;
  onMessage: (message: string | null) => void;
  help: { metrics: string; save: string };
}

export function ResultsSection({
  result,
  holdout,
  holdoutResult,
  selected,
  strat,
  stratName,
  saving,
  onSave,
  onError,
  onMessage,
  help,
}: ResultsSectionProps): React.ReactElement {
  const [exporting, setExporting] = useState<'json' | 'csv' | null>(null);
  const [exportNotice, setExportNotice] = useState<{ kind: 'busy' | 'done'; text: string } | null>(null);
  const [poppedMetrics, setPoppedMetrics] = useState(false);

  // Columns for the metrics table: a single full-period column, or three
  // (full / in-sample / out-of-sample) when holdout produced a split.
  const metricCols = result
    ? holdout && holdoutResult
      ? [
          { label: '全期', metrics: result.metrics },
          { label: '樣本內', metrics: holdoutResult.inSample.metrics },
          { label: '樣本外', metrics: holdoutResult.outSample.metrics },
        ]
      : [{ label: '', metrics: result.metrics }]
    : [];

  // Metrics table content, factored out so it can render inline OR (Slice 8a)
  // enlarged inside a FloatingPanel. Same state either way -> edits reflow live.
  const renderMetricsTable = (fontSize: number) => (
    <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: "'IBM Plex Mono', monospace", fontSize }}>
      {metricCols.length > 1 && (
        <thead>
          <tr style={{ borderBottom: '1px solid #d6d2c8' }}>
            <th />
            {metricCols.map((c) => (
              <th key={c.label} data-testid={`col-${c.label}`} style={{ padding: '4px', textAlign: 'right', fontSize: fontSize - 2, fontWeight: 600, color: '#8a8678' }}>{c.label}</th>
            ))}
          </tr>
        </thead>
      )}
      <tbody>
        {METRIC_ROWS.map((r) => (
          <tr key={r.label} style={{ borderBottom: '1px solid #efece5' }}>
            <td style={{ padding: '5px 4px', color: '#8a8678' }}>{r.label}</td>
            {metricCols.map((c) => (
              <td key={c.label} style={{ padding: '5px 4px', textAlign: 'right', fontWeight: 600 }}>{r.fmt(c.metrics)}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );

  async function exportResult(ext: 'json' | 'csv') {
    if (!selected || !result) return;
    setExporting(ext);
    onError(null);
    onMessage(null);
    setExportNotice({ kind: 'busy', text: `正在準備 ${ext.toUpperCase()} 下載...` });
    try {
      const at = Date.now();
      const dataset = {
        symbol: selected.symbol,
        interval: selected.interval,
        startTime: selected.start_time,
        endTime: selected.end_time,
      };
      const contents = ext === 'json'
        ? reportToJson({ strategyName: stratName, strategy: strat, dataset, result, exportedAt: at })
        : tradesToCsv(result.trades);
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
          {result && (
            <button data-testid="popout-metrics" title="放大到獨立面板" style={{ ...S.btnGhost, padding: '3px 10px', marginLeft: 'auto' }} onClick={() => setPoppedMetrics((v) => !v)}>
              {poppedMetrics ? '⤡ 收合' : '⤢ 放大'}
            </button>
          )}
        </div>
        {!result && <p style={{ color: '#aaa599', fontSize: 12 }}>尚未回測 — 選資料集、設策略後按「執行回測」。</p>}
        {result && (
          <>
            {poppedMetrics ? <PoppedOutNote label="回測績效" onClose={() => setPoppedMetrics(false)} /> : renderMetricsTable(12)}

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
                    color: exportNotice.kind === 'done' ? '#1f7a57' : '#8a7a3a',
                    fontFamily: "'IBM Plex Mono', monospace",
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
            <p style={{ color: '#aaa599', fontSize: 11, marginTop: 8 }}>
              儲存會寫入 strategy_def + backtest_summary（segment=full），經由 metricsToBacktestSummary()。
            </p>
          </>
        )}
      </section>

      {/* Slice 8a metrics pop-out: non-modal floating panel rendering the metrics
          enlarged; the rest of the app stays usable while it is open. */}
      {poppedMetrics && result && (
        <FloatingPanel title="回測績效" testId="metrics-popout" initial={{ x: 220, y: 130, w: 460, h: 520 }} onClose={() => setPoppedMetrics(false)}>
          {() => renderMetricsTable(15)}
        </FloatingPanel>
      )}
    </>
  );
}
