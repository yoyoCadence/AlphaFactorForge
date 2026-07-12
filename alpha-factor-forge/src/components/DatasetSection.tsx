// Dataset picker / import section, extracted from BacktestPanel (REF-003).
// Presentational: all state + handlers stay in the panel and arrive as props
// (the import parsing / sample loading cross-cut panel state). Move-only.
import React from 'react';
import type { Dataset } from '../tauri-client/commands';
import { HelpTip } from './HelpTip';
import { S } from './panelStyles';

export interface DatasetSectionProps {
  datasets: Dataset[];
  selId: number | null;
  busyData: boolean;
  importText: string;
  tauriAvailable: boolean;
  helpText: string;
  onSelectDataset: (id: number | null) => void;
  onLoadSample: () => void;
  onRefresh: () => void;
  onImportTextChange: (text: string) => void;
  onImportJson: () => void;
}

export function DatasetSection({
  datasets,
  selId,
  busyData,
  importText,
  tauriAvailable,
  helpText,
  onSelectDataset,
  onLoadSample,
  onRefresh,
  onImportTextChange,
  onImportJson,
}: DatasetSectionProps): React.ReactElement {
  return (
    <section style={S.card}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, margin: '0 0 8px' }}>
        <h2 style={{ ...S.h2, margin: 0 }}>資料集</h2>
        <HelpTip id="dataset" label="資料集" text={helpText} />
      </div>
      <select
        value={selId ?? ''}
        onChange={(e) => onSelectDataset(e.target.value ? Number(e.target.value) : null)}
        style={{ ...S.input, marginBottom: 8 }}
      >
        {datasets.length === 0 && <option value="">（尚無資料集）</option>}
        {datasets.map((d) => (
          <option key={d.id} value={d.id}>
            {d.symbol} · {d.interval} · {d.candle_count} 根
          </option>
        ))}
      </select>
      <div style={{ display: 'flex', gap: 6, marginBottom: 8 }}>
        <button data-testid="load-sample" style={S.btnGhost} onClick={onLoadSample} disabled={busyData || !tauriAvailable} aria-busy={busyData}>
          {busyData ? '處理中…' : '載入樣本資料'}
        </button>
        <button style={S.btnGhost} onClick={onRefresh} disabled={!tauriAvailable}>
          重新整理
        </button>
      </div>
      <textarea
        value={importText}
        onChange={(e) => onImportTextChange(e.target.value)}
        placeholder='貼上 K 線 JSON：[{ "t":.., "o":.., "h":.., "l":.., "c":.., "v":.. }, …] 或 { "symbol":"BTCUSDT","interval":"1h","candles":[…] }'
        rows={3}
        style={{ ...S.input, fontSize: 11, resize: 'vertical' }}
      />
      <button style={{ ...S.btnGhost, marginTop: 6 }} onClick={onImportJson} disabled={busyData || !importText.trim() || !tauriAvailable} aria-busy={busyData}>
        匯入 JSON
      </button>
    </section>
  );
}
