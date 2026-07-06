// Slice 2 — single-strategy (params mode) backtest panel.
//
// Vertical slice: pick/import a dataset (SQLite) -> edit params-mode strategy ->
// run via the Slice 1 service (core/* under the hood) -> show metrics -> save
// the result (strategy_def + backtest_summary). No chart / sweep / replay /
// live / library yet — those are later slices. All persistence goes through
// tauri-client; all maths through core/* + src/services.

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { db, files, isTauri, importDataset } from '../tauri-client/dataClient';
import type { Candle, Dataset } from '../tauri-client/commands';
import { defaultStrategy, OPERAND_IDS, type ParamsStrategy, type SignalId, type Rule, type RuleOp, type OperandId } from '../services/strategy';
import { SUPPORTED_SIGNALS, buildSignals } from '../services/strategySignals';
import { compileExpression } from '../services/exprInterpreter';
import { runParamsBacktest } from '../services/backtestRunner';
import {
  runParamSweep,
  countSweepCombos,
  SWEEP_PARAM_KEYS,
  SWEEP_METRIC_IDS,
  SWEEP_MAX_COMBOS,
  type SweepAxisConfig,
  type SweepConfig,
  type SweepMetricId,
  type SweepParamKey,
  type SweepResult,
} from '../services/paramSweep';
import { toCoreCandles } from '../services/candleAdapter';
import { makeSampleCandles } from '../services/sampleData';
import { buildStrategyDef } from '../services/strategyRecord';
import { metricsToBacktestSummary } from '../services/metricsMapper';
import { reportToJson, suggestedFilename, tradesToCsv } from '../services/reportExport';
import { CandleChart, type OverlayToggles } from '../charts/CandleChart';
import { replayTick, positionAtTime } from '../charts/scale';
import { HelpTip } from './HelpTip';
import { FloatingPanel } from './FloatingPanel';
import type { BacktestResult, Candle as CoreCandle } from '../core/backtest';
import type { Metrics } from '../core/metrics';

type NumKey =
  | 'fastMA' | 'slowMA' | 'emaPeriod' | 'rsiPeriod' | 'rsiBuy' | 'rsiSell'
  | 'macdFast' | 'macdSlow' | 'macdSignal' | 'bbPeriod' | 'bbMult'
  | 'feePct' | 'slipPct' | 'sizePct' | 'slPct' | 'tpPct';

const IND_FIELDS: { key: NumKey; label: string }[] = [
  { key: 'fastMA', label: '快線 MA' },
  { key: 'slowMA', label: '慢線 MA' },
  { key: 'emaPeriod', label: 'EMA' },
  { key: 'rsiPeriod', label: 'RSI 週期' },
  { key: 'rsiBuy', label: 'RSI 買' },
  { key: 'rsiSell', label: 'RSI 賣' },
  { key: 'macdFast', label: 'MACD 快' },
  { key: 'macdSlow', label: 'MACD 慢' },
  { key: 'macdSignal', label: 'MACD 訊號' },
  { key: 'bbPeriod', label: 'BB 週期' },
  { key: 'bbMult', label: 'BB 倍數' },
];

// The overlay-driving periods most often tweaked while reading the chart —
// surfaced as a quick row right under it. Same `strat` state as the full form.
const QUICK_FIELDS: { key: NumKey; label: string }[] = [
  { key: 'fastMA', label: '快線 MA' },
  { key: 'slowMA', label: '慢線 MA' },
  { key: 'emaPeriod', label: 'EMA' },
  { key: 'rsiPeriod', label: 'RSI 週期' },
];

const EXEC_FIELDS: { key: NumKey; label: string }[] = [
  { key: 'feePct', label: '手續費 %' },
  { key: 'slipPct', label: '滑價 %' },
  { key: 'sizePct', label: '部位 %' },
  { key: 'slPct', label: '停損 %' },
  { key: 'tpPct', label: '停利 %' },
];

const SIG_LABEL: Record<SignalId, string> = {
  maCrossUp: 'MA 金叉', maCrossDown: 'MA 死叉',
  emaCrossUp: '價格上穿 EMA', emaCrossDown: '價格下穿 EMA',
  priceAboveSlow: '價格 > 慢線', priceBelowSlow: '價格 < 慢線',
  rsiOversold: 'RSI 超賣上穿', rsiOverbought: 'RSI 超買下穿',
  macdCrossUp: 'MACD 金叉', macdCrossDown: 'MACD 死叉',
  bbLowerTouch: '觸布林下軌', bbUpperTouch: '觸布林上軌',
  stochOversold: 'Stoch 超賣(未支援)', stochOverbought: 'Stoch 超買(未支援)',
};

const OVERLAY_LABEL: Record<keyof OverlayToggles, string> = { ma: 'MA', ema: 'EMA', bb: 'BB', rsi: 'RSI', vol: '量', trades: '買賣' };

const OPERAND_LABEL: Record<OperandId, string> = {
  price: '價格', open: '開', high: '高', low: '低', volume: '量',
  maFast: '快線', maSlow: '慢線', ema: 'EMA', rsi: 'RSI',
  macd: 'MACD', macdSignal: 'MACD訊號', macdHist: 'MACD柱',
  bbUpper: '布林上', bbMid: '布林中', bbLower: '布林下',
};
const RULE_OPS: RuleOp[] = ['>', '<', '>=', '<=', 'crossUp', 'crossDown'];
const OP_LABEL: Record<RuleOp, string> = { '>': '>', '<': '<', '>=': '≥', '<=': '≤', crossUp: '上穿', crossDown: '下穿' };

const MODE_LABEL: Record<ParamsStrategy['mode'], string> = { params: '參數', blocks: '積木', code: '程式碼' };

const POS_LABEL: Record<'LONG' | 'SHORT' | 'FLAT', string> = { LONG: '多', SHORT: '空', FLAT: '空手' };

// Slice 5c — short explanations shown by the "?" HelpTip markers. Kept as one
// map so the copy is easy to review/edit without hunting through the JSX.
const HELP: Record<string, string> = {
  dataset: '選擇或匯入 K 線資料集：載入內建樣本、貼上 JSON 匯入，或選既有資料集（SQLite）。回測與掃描都以此資料為輸入。',
  strategy: '定義進出場邏輯。參數＝挑現成訊號；積木＝用運算元組規則；程式碼＝手動撰寫安全運算式（AI 不會使用此模式）。',
  exec: '回測的成交假設：手續費、滑價、部位大小、停損／停利、方向（做多／做空／雙向），以及成交價（當根收盤或次根開盤）。',
  holdout: '把最後 N% 的 K 線留作樣本外（out-of-sample）。回測會同時列出全期／樣本內／樣本外，用來檢查是否過度擬合。',
  metrics: '策略在此資料集上的表現：淨報酬、CAGR、最大回撤、Sharpe／Sortino／Calmar、勝率、交易數、獲利因子等。',
  sweep: `自動改變 1–2 個參數掃過設定範圍，用熱力圖找較佳組合（上限 ${SWEEP_MAX_COMBOS} 組）。注意：歷史最佳常過度擬合，務必再用樣本外驗證。`,
  run: '以目前策略與執行模型，在選定資料集上跑一次回測；結果顯示於右側「回測績效」。',
  save: '把策略與這次回測摘要寫入資料庫（strategy_def + backtest_summary，segment=full），經由 metricsToBacktestSummary()。',
  runSweep: `對每個參數組合各回測一次並畫成熱力圖（上限 ${SWEEP_MAX_COMBOS} 組）；掃描期間畫面顯示「掃描中…」。`,
  applyBest: '把最佳組合的參數套回策略表單（也可直接點熱力圖任一格套用該格的組合）。',
  replay: '回放模式：用滑桿或 ◀ / ▶ 一根一根前進，或按 ⏵ 自動播放（速度 1×–4×）；圖表只畫到目前這根，並顯示此根的進出場訊號與持倉（持倉依上次回測），之後的 K 線與買賣點會被隱藏，像重播當時看到的行情。',
};

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

function pct(x: number): string {
  return `${(x * 100).toFixed(2)}%`;
}
function num(x: number): string {
  return Number.isFinite(x) ? x.toFixed(2) : '—';
}

const SWEEP_PARAM_LABEL: Record<SweepParamKey, string> = {
  fastMA: '快線MA', slowMA: '慢線MA', emaPeriod: 'EMA週期', rsiPeriod: 'RSI週期',
  rsiBuy: 'RSI買', rsiSell: 'RSI賣', macdFast: 'MACD快', macdSlow: 'MACD慢', bbPeriod: '布林週期',
};
const SWEEP_METRIC_LABEL: Record<SweepMetricId, string> = {
  net: '淨報酬', sharpe: '夏普', pf: '獲利因子', winRate: '勝率', calmar: '卡瑪', dd: '最小回撤',
};

/** Sweep metrics stored as ratios/percent (net/winRate/dd) render as %. */
function fmtSweepMetric(metric: SweepMetricId, v: number | null): string {
  if (v == null) return '—';
  return metric === 'net' || metric === 'winRate' || metric === 'dd'
    ? `${(v * 100).toFixed(1)}%`
    : v.toFixed(2);
}

function sweepBestLabel(r: SweepResult): string {
  if (!r.best) return '';
  const x = `${SWEEP_PARAM_LABEL[r.xKey]}=${r.best.x}`;
  const y = r.yKey != null && r.best.y != null ? ` · ${SWEEP_PARAM_LABEL[r.yKey]}=${r.best.y}` : '';
  return `${x}${y} → ${fmtSweepMetric(r.metric, r.best.metric)}（${r.best.trades} 筆）`;
}

/** Heatmap color for t in [0,1]: 0 = red, 0.5 = yellow, 1 = green. */
function heatColor(t: number): string {
  const u = Math.max(0, Math.min(1, t));
  const lerp = (a: number, b: number, k: number) => Math.round(a + (b - a) * k);
  if (u < 0.5) {
    const k = u / 0.5;
    return `rgb(${lerp(192, 241, k)},${lerp(57, 196, k)},${lerp(43, 15, k)})`;
  }
  const k = (u - 0.5) / 0.5;
  return `rgb(${lerp(241, 31, k)},${lerp(196, 138, k)},${lerp(15, 91, k)})`;
}

/** Read a finite number from one of several candidate keys, else throw. */
function pickNum(o: Record<string, unknown>, keys: string[]): number {
  for (const k of keys) {
    const v = o[k];
    if (typeof v === 'number' && Number.isFinite(v)) return v;
  }
  throw new Error('K 線欄位需為數字（t/o/h/l/c/v 或 timestamp/open/high/low/close/volume）');
}

function normalizeCandle(x: unknown): Candle {
  const o = (x ?? {}) as Record<string, unknown>;
  return {
    timestamp: pickNum(o, ['timestamp', 't']),
    open: pickNum(o, ['open', 'o']),
    high: pickNum(o, ['high', 'h']),
    low: pickNum(o, ['low', 'l']),
    close: pickNum(o, ['close', 'c']),
    volume: pickNum(o, ['volume', 'v']),
  };
}

const S = {
  panel: { display: 'grid', gridTemplateColumns: '380px 1fr', gap: 16, alignItems: 'start' } as React.CSSProperties,
  card: { border: '1px solid #d6d2c8', background: '#fff', padding: 12 } as React.CSSProperties,
  h2: { fontSize: 12, fontWeight: 700, margin: '0 0 8px', letterSpacing: '0.04em', color: '#16150f' } as React.CSSProperties,
  label: { fontSize: 10, color: '#8a8678' } as React.CSSProperties,
  input: {
    width: '100%', padding: '5px 7px', border: '1px solid #d6d2c8', background: '#fff',
    fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, color: '#16150f', outline: 'none',
  } as React.CSSProperties,
  btn: {
    fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, fontWeight: 600,
    padding: '6px 10px', border: '1px solid #16150f', background: '#16150f', color: '#fff', cursor: 'pointer',
  } as React.CSSProperties,
  btnGhost: {
    fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, fontWeight: 600,
    padding: '6px 10px', border: '1px solid #d6d2c8', background: '#efece5', color: '#16150f', cursor: 'pointer',
  } as React.CSSProperties,
  grid3: { display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 7 } as React.CSSProperties,
};

/** Editable AND-list of blocks-mode rules (left operand · op · right). */
function RuleRows({ title, rules, onChange }: { title: string; rules: Rule[]; onChange: (rules: Rule[]) => void }): React.ReactElement {
  const update = (i: number, patch: Partial<Rule>) => onChange(rules.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));
  const add = () => onChange([...rules, { l: 'price', op: '>', r: 'maSlow' }]);
  const remove = (i: number) => onChange(rules.filter((_, idx) => idx !== i));
  return (
    <div style={{ marginBottom: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
        <span style={S.label}>{title}（全部成立才觸發）</span>
        <button style={{ ...S.btnGhost, padding: '2px 8px' }} onClick={add}>＋ 規則</button>
      </div>
      {rules.length === 0 && <div style={{ fontSize: 11, color: '#aaa599' }}>（無規則 → 不觸發）</div>}
      {rules.map((r, i) => (
        <div key={i} style={{ display: 'grid', gridTemplateColumns: '1fr 58px 1fr 22px', gap: 4, marginBottom: 4 }}>
          <select value={r.l} onChange={(e) => update(i, { l: e.target.value as OperandId })} style={{ ...S.input, fontSize: 11 }}>
            {OPERAND_IDS.map((id) => <option key={id} value={id}>{OPERAND_LABEL[id]}</option>)}
          </select>
          <select value={r.op} onChange={(e) => update(i, { op: e.target.value as RuleOp })} style={{ ...S.input, fontSize: 11, padding: '5px 2px' }}>
            {RULE_OPS.map((op) => <option key={op} value={op}>{OP_LABEL[op]}</option>)}
          </select>
          <input list="operand-list" value={r.r} onChange={(e) => update(i, { r: e.target.value })} placeholder="series 或數字" style={{ ...S.input, fontSize: 11 }} />
          <button onClick={() => remove(i)} title="刪除" style={{ ...S.btnGhost, padding: '2px 0' }}>×</button>
        </div>
      ))}
    </div>
  );
}

/** A code-mode expression field with live (per-keystroke) interpreter validation. */
function CodeField({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }): React.ReactElement {
  let err: string | null = null;
  try {
    compileExpression(value, OPERAND_IDS);
  } catch (e) {
    err = e instanceof Error ? e.message : String(e);
  }
  return (
    <label style={{ display: 'flex', flexDirection: 'column', gap: 3, marginBottom: 8 }}>
      <span style={S.label}>{label}</span>
      <textarea
        value={value}
        onChange={(e) => onChange(e.target.value)}
        rows={2}
        spellCheck={false}
        style={{ ...S.input, fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, resize: 'vertical', borderColor: err ? '#d23b2f' : '#d6d2c8' }}
      />
      {err && <span style={{ fontSize: 10, color: '#b23b2e' }}>{err}</span>}
    </label>
  );
}

/** Numeric input that allows clearing / partial edits. Keeps a draft string
 *  while focused (so backspace doesn't snap to 0 or a clamp), propagates the
 *  number live only when the draft is a valid number, and normalises/clamps on
 *  blur. min/max clamp on blur only — not while typing. */
function NumberInput({
  value,
  onChange,
  min,
  max,
  style,
}: {
  value: number;
  onChange: (n: number) => void;
  min?: number;
  max?: number;
  style?: React.CSSProperties;
}): React.ReactElement {
  const [draft, setDraft] = useState(String(value));

  // Re-sync when the value changes externally, but don't clobber an in-progress
  // edit that already parses to the same number (e.g. "5." while typing "5.5").
  useEffect(() => {
    if (parseFloat(draft) !== value) setDraft(String(value));
  }, [value]); // eslint-disable-line react-hooks/exhaustive-deps

  const clamp = (n: number) => {
    let v = n;
    if (min != null) v = Math.max(min, v);
    if (max != null) v = Math.min(max, v);
    return v;
  };

  return (
    <input
      type="number"
      value={draft}
      onChange={(e) => {
        const raw = e.target.value;
        setDraft(raw);
        const n = parseFloat(raw);
        if (raw !== '' && Number.isFinite(n)) onChange(n); // live, unclamped; empty/partial stays in the field
      }}
      onBlur={() => {
        const n = parseFloat(draft);
        const v = clamp(Number.isFinite(n) ? n : value);
        onChange(v);
        setDraft(String(v));
      }}
      style={style}
    />
  );
}

/** One sweep axis on a single wrap-safe row: param picker + min / max / step.
 *  Inline (not a column) so the optional 2-D Y row can't overlap neighbours. */
function AxisEditor({ title, axis, onChange }: { title: string; axis: SweepAxisConfig; onChange: (a: SweepAxisConfig) => void }): React.ReactElement {
  return (
    <div style={{ display: 'flex', gap: 10, alignItems: 'flex-end', flexWrap: 'wrap', marginTop: 8 }}>
      <label style={{ display: 'flex', flexDirection: 'column', gap: 3, minWidth: 150 }}>
        <span style={S.label}>{title}</span>
        <select value={axis.key} onChange={(e) => onChange({ ...axis, key: e.target.value as SweepParamKey })} style={{ ...S.input, fontSize: 11 }}>
          {SWEEP_PARAM_KEYS.map((k) => <option key={k} value={k}>{SWEEP_PARAM_LABEL[k]}</option>)}
        </select>
      </label>
      {([['min', '起'], ['max', '迄'], ['step', '間距']] as const).map(([k, lbl]) => (
        <label key={k} style={{ display: 'flex', flexDirection: 'column', gap: 3, width: 76 }}>
          <span style={S.label}>{lbl}</span>
          <NumberInput value={axis[k]} onChange={(n) => onChange({ ...axis, [k]: n })} style={{ ...S.input, fontSize: 11 }} />
        </label>
      ))}
    </div>
  );
}

/** Red→yellow→green heatmap of a sweep grid. Every cell is clickable to apply
 *  its param combo; the best cell is outlined (★) and the applied cell is ringed
 *  (✓), so the user always sees which combo is currently on the strategy. */
function SweepHeatmap({
  result,
  applied,
  onPick,
}: {
  result: SweepResult;
  applied: { x: number; y: number | null } | null;
  onPick: (x: number, y: number | null) => void;
}): React.ReactElement {
  const { xs, ys, grid, best, lo, hi, metric, xKey, yKey } = result;
  const is2d = yKey != null;
  const span = hi - lo;
  const cell: React.CSSProperties = {
    padding: '4px 6px', textAlign: 'center', fontFamily: "'IBM Plex Mono', monospace", fontSize: 11, minWidth: 48,
  };
  const head: React.CSSProperties = { ...cell, background: '#efece5', color: '#16150f', border: '1px solid #fff' };
  return (
    <div style={{ marginTop: 12, overflowX: 'auto' }}>
      <div style={{ fontSize: 11, color: '#8a8678', marginBottom: 6 }}>
        熱力圖 · 顏色越綠越佳（{SWEEP_METRIC_LABEL[metric]}）；橫軸 {SWEEP_PARAM_LABEL[xKey]}
        {is2d ? `、縱軸 ${SWEEP_PARAM_LABEL[yKey]}` : ''}。每格為指標值，括號為交易次數。
        <b>點任一格</b>即套用該組合 · <span style={{ color: '#16150f' }}>★ 最佳</span> · <span style={{ color: '#2f6df0' }}>✓ 已套用（藍框）</span>。
      </div>
      <table style={{ borderCollapse: 'collapse' }}>
        <thead>
          <tr>
            <th style={{ ...head, color: '#8a8678' }}>{is2d ? `${SWEEP_PARAM_LABEL[yKey]} \\ ${SWEEP_PARAM_LABEL[xKey]}` : SWEEP_PARAM_LABEL[xKey]}</th>
            {xs.map((x) => <th key={x} style={head}>{x}</th>)}
          </tr>
        </thead>
        <tbody>
          {grid.map((row, ri) => (
            <tr key={ri}>
              <th style={head}>{is2d ? String(ys[ri]) : ''}</th>
              {row.map((c, ci) => {
                const isBest = best != null && c.x === best.x && c.y === best.y && c.metric === best.metric && c.trades > 0;
                const isApplied = applied != null && applied.x === c.x && applied.y === c.y;
                const t = c.metric == null ? 0 : span > 0 ? (c.metric - lo) / span : 1;
                const bg = c.metric == null ? '#e8e6df' : heatColor(t);
                return (
                  <td
                    key={ci}
                    data-testid={`sweep-cell-${c.x}${is2d ? `-${c.y}` : ''}`}
                    onClick={() => onPick(c.x, c.y)}
                    title={`點擊套用 ${SWEEP_PARAM_LABEL[xKey]}=${c.x}${is2d ? ` · ${SWEEP_PARAM_LABEL[yKey]}=${c.y}` : ''}`}
                    style={{
                      ...cell,
                      background: bg,
                      color: '#16150f',
                      cursor: 'pointer',
                      border: isBest ? '2px solid #16150f' : '1px solid #fff',
                      outline: isApplied ? '3px solid #2f6df0' : 'none',
                      outlineOffset: '-3px',
                      fontWeight: isBest || isApplied ? 700 : 500,
                    }}
                  >
                    <div>{fmtSweepMetric(metric, c.metric)}</div>
                    <div style={{ fontSize: 9, color: '#3c3a30' }}>({c.trades})</div>
                    {(isApplied || isBest) && (
                      <div style={{ fontSize: 9, fontWeight: 700, lineHeight: 1.2 }}>
                        {isApplied && <span data-testid="sweep-applied-marker" style={{ color: '#2f6df0' }}>✓已套用</span>}
                        {isBest && <span data-testid="sweep-best-marker" style={{ color: '#16150f', marginLeft: isApplied ? 3 : 0 }}>★</span>}
                      </div>
                    )}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** Inline stand-in shown where the chart / metrics normally sit while that
 *  section is popped out into a FloatingPanel (Slice 8a). */
function PoppedOutNote({ label, onClose }: { label: string; onClose: () => void }): React.ReactElement {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '18px 12px', background: '#f4f2ec', border: '1px dashed #cfccc4', color: '#8a8678', fontSize: 12 }}>
      {label}已彈出放大檢視。
      <button style={{ ...S.btnGhost, padding: '2px 8px' }} onClick={onClose}>收合</button>
    </div>
  );
}

export function BacktestPanel(): React.ReactElement {
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [selId, setSelId] = useState<number | null>(null);
  const [strat, setStrat] = useState<ParamsStrategy>(defaultStrategy);
  const [stratName, setStratName] = useState('');
  const [result, setResult] = useState<BacktestResult | null>(null);
  const [running, setRunning] = useState(false);
  const [saving, setSaving] = useState(false);
  const [exporting, setExporting] = useState<'json' | 'csv' | null>(null);
  const [exportNotice, setExportNotice] = useState<{ kind: 'busy' | 'done'; text: string } | null>(null);
  const [busyData, setBusyData] = useState(false);
  const [importText, setImportText] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [candles, setCandles] = useState<CoreCandle[]>([]);
  const [loadingCandles, setLoadingCandles] = useState(false);
  const [show, setShow] = useState<OverlayToggles>({ ma: true, ema: false, bb: false, rsi: true, vol: true, trades: true });
  // Bar replay (Slice 6-1): step a cursor through the loaded candles; the chart
  // clips to bars [.., cursor]. Cursor resets to the latest bar when candles change.
  const [replayOn, setReplayOn] = useState(false);
  const [replayCursor, setReplayCursor] = useState(0);
  const [replayPlaying, setReplayPlaying] = useState(false); // autoplay (Slice 6-2)
  const [replaySpeed, setReplaySpeed] = useState(1); // 1× / 2× / 4×
  // Pop-out (Slice 8a): enlarge 圖表 / 回測績效 into a floating resizable panel.
  const [poppedChart, setPoppedChart] = useState(false);
  const [poppedMetrics, setPoppedMetrics] = useState(false);
  const [hoverBar, setHoverBar] = useState<number | null>(null); // chart hover (Slice 9)
  const [holdout, setHoldout] = useState(false);
  const [holdoutPct, setHoldoutPct] = useState(30); // last N% of bars = out-of-sample
  const [holdoutResult, setHoldoutResult] = useState<{ inSample: BacktestResult; outSample: BacktestResult } | null>(null);
  // Parameter sweep (Slice 5b-2): vary 1–2 params over ranges via the 5b-1 engine.
  const [sweepOpen, setSweepOpen] = useState(false);
  const [sweepX, setSweepX] = useState<SweepAxisConfig>({ key: 'fastMA', min: 5, max: 20, step: 1 });
  const [sweepY, setSweepY] = useState<SweepAxisConfig>({ key: 'slowMA', min: 20, max: 40, step: 2 });
  const [sweepUse2d, setSweepUse2d] = useState(false);
  const [sweepMetric, setSweepMetric] = useState<SweepMetricId>('net');
  const [sweeping, setSweeping] = useState(false);
  const [sweepResult, setSweepResult] = useState<SweepResult | null>(null);
  const [sweepErr, setSweepErr] = useState<string | null>(null);
  const [appliedCell, setAppliedCell] = useState<{ x: number; y: number | null } | null>(null);
  // Which strategy params the last sweep-apply set — highlighted in the form +
  // chart quick row so the user sees what the heatmap selection changed. A param
  // drops out of the set the moment it is hand-edited (no longer "from sweep").
  const [appliedKeys, setAppliedKeys] = useState<NumKey[]>([]);

  const refresh = useCallback(async () => {
    const ds = await db.getDatasets();
    setDatasets(ds);
    setSelId((prev) => prev ?? ds[0]?.id ?? null);
  }, []);

  useEffect(() => {
    if (isTauri()) refresh().catch((e) => setErr(String(e)));
  }, [refresh]);

  // Load candles for the chart whenever the selected dataset changes.
  useEffect(() => {
    const ds = datasets.find((d) => d.id === selId) ?? null;
    if (!isTauri() || !ds || ds.id == null) {
      setCandles([]);
      return;
    }
    let cancelled = false;
    setLoadingCandles(true);
    db.getCandles(ds.id, ds.start_time, ds.end_time)
      .then((cs) => {
        if (!cancelled) {
          setCandles(toCoreCandles(cs));
          setResult(null);
        }
      })
      .catch((e) => !cancelled && setErr(String(e)))
      .finally(() => !cancelled && setLoadingCandles(false));
    return () => {
      cancelled = true;
    };
  }, [selId, datasets]);

  // Keep the replay cursor at the latest bar whenever the candle set changes
  // (dataset switch / import), so it's always in bounds and starts "at now".
  useEffect(() => {
    setReplayCursor(Math.max(0, candles.length - 1));
    setReplayPlaying(false);
  }, [candles]);

  // Autoplay tick (Slice 6-2): while playing, advance the cursor one bar every
  // 400/speed ms. The interval is re-created only when play/speed/candles change
  // (a functional update reads the latest cursor), so playback stays smooth.
  useEffect(() => {
    if (!replayOn || !replayPlaying) return;
    const id = setInterval(() => {
      setReplayCursor((c) => replayTick(c, candles.length).cursor);
    }, 400 / replaySpeed);
    return () => clearInterval(id);
  }, [replayOn, replayPlaying, replaySpeed, candles.length]);

  // Stop autoplay once the cursor reaches the last bar (kept out of the tick
  // updater so state stays pure / StrictMode-safe).
  useEffect(() => {
    if (replayPlaying && replayCursor >= candles.length - 1) setReplayPlaying(false);
  }, [replayPlaying, replayCursor, candles.length]);

  // Live signal series for the replay readout (Slice 6-3): entry/exit condition
  // per bar via the same buildSignals the backtest uses. Memoized over
  // candles+strat so it isn't recomputed on every autoplay tick (only the
  // cursor moves); a code-mode parse error yields null (readout hides).
  const signalSeries = useMemo(() => {
    if (candles.length === 0) return null;
    try {
      return buildSignals(candles, strat);
    } catch {
      return null;
    }
  }, [candles, strat]);

  // Play/pause; starting from the last bar restarts replay from the first.
  const toggleReplayPlay = () => {
    if (replayPlaying) {
      setReplayPlaying(false);
      return;
    }
    if (replayCursor >= candles.length - 1) setReplayCursor(0);
    setReplayPlaying(true);
  };

  const selected = datasets.find((d) => d.id === selId) ?? null;
  const setNum = (key: NumKey, value: number) => {
    setStrat((s) => ({ ...s, [key]: value }));
    setAppliedKeys((ks) => (ks.includes(key) ? ks.filter((k) => k !== key) : ks));
  };

  // Highlight styling for a param that the last sweep-apply set (blue accent).
  const isAppliedKey = (key: NumKey) => appliedKeys.includes(key);
  const appliedInputStyle = (key: NumKey, base: React.CSSProperties): React.CSSProperties =>
    isAppliedKey(key) ? { ...base, borderColor: '#2f6df0', background: '#eef4ff' } : base;
  const appliedLabelStyle = (key: NumKey): React.CSSProperties =>
    isAppliedKey(key) ? { ...S.label, color: '#2f6df0', fontWeight: 700 } : S.label;

  async function loadSample() {
    setBusyData(true);
    setErr(null);
    setMsg(null);
    try {
      const candles = makeSampleCandles({ count: 600 });
      const id = await importDataset({ exchange: 'sample', symbol: 'SAMPLE', interval: '1h', source: 'sample', candles });
      await refresh();
      setSelId(id);
      setMsg('已載入樣本資料（SAMPLE · 1h · 600 根；僅供測試管線）');
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusyData(false);
    }
  }

  async function importJson() {
    setBusyData(true);
    setErr(null);
    setMsg(null);
    try {
      const raw: unknown = JSON.parse(importText);
      const rec = (raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {});
      const arr = Array.isArray(raw) ? raw : rec.candles;
      if (!Array.isArray(arr) || arr.length === 0) throw new Error('JSON 需為非空的 K 線陣列，或 { candles: [...] }');
      const candles = arr.map(normalizeCandle);
      const symbol = typeof rec.symbol === 'string' ? rec.symbol : 'IMPORT';
      const interval = typeof rec.interval === 'string' ? rec.interval : '1h';
      const id = await importDataset({ exchange: 'import', symbol, interval, source: 'import', candles });
      await refresh();
      setSelId(id);
      setImportText('');
      setMsg(`已匯入 ${candles.length} 根 K 線（${symbol} · ${interval}）`);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusyData(false);
    }
  }

  async function run() {
    if (!selected || selected.id == null) {
      setErr('請先選擇資料集');
      return;
    }
    setRunning(true);
    setErr(null);
    setMsg(null);
    setResult(null);
    setHoldoutResult(null);
    try {
      let cs = candles;
      if (!cs.length) {
        cs = toCoreCandles(await db.getCandles(selected.id, selected.start_time, selected.end_time));
        setCandles(cs);
      }
      if (!cs.length) throw new Error('此資料集沒有 K 線');
      const interval = selected.interval;
      setResult(runParamsBacktest({ candles: cs, strat, interval }));
      if (holdout) {
        // Same candles (so indicators keep full history); from/to restrict which
        // bars are traded -> proper in-sample vs out-of-sample split.
        const nn = cs.length;
        const split = Math.max(1, Math.min(nn - 1, Math.floor(nn * (1 - holdoutPct / 100))));
        const inSample = runParamsBacktest({ candles: cs, strat, interval, from: 0, to: split - 1 });
        const outSample = runParamsBacktest({ candles: cs, strat, interval, from: split, to: nn - 1 });
        setHoldoutResult({ inSample, outSample });
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setRunning(false);
    }
  }

  async function save() {
    if (!selected || selected.id == null || !result) return;
    setSaving(true);
    setErr(null);
    try {
      const def = await buildStrategyDef(strat, stratName);
      const strategyId = await db.saveStrategy(def);
      const summary = metricsToBacktestSummary(result.metrics, {
        strategyId,
        datasetId: selected.id,
        segment: 'full',
        startTime: selected.start_time,
        endTime: selected.end_time,
      });
      await db.saveBacktestResult(summary);
      setMsg(`已存檔：strategy #${strategyId}（type=${def.type}）· dataset #${selected.id} · ${result.trades.length} trades`);
    } catch (e) {
      setErr(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function exportResult(ext: 'json' | 'csv') {
    if (!selected || !result) return;
    setExporting(ext);
    setErr(null);
    setMsg(null);
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
      setMsg(`已匯出 ${ext.toUpperCase()}：${path}`);
      setExportNotice({ kind: 'done', text: `${ext.toUpperCase()} 下載完成：${path}` });
    } catch (e) {
      setExportNotice(null);
      setErr(String(e));
    } finally {
      setExporting(null);
    }
  }

  const sweepConfig: SweepConfig = { x: sweepX, y: sweepUse2d ? sweepY : null, metric: sweepMetric };
  const sweepCombos = countSweepCombos(sweepConfig);
  const sweepDupKey = sweepUse2d && sweepX.key === sweepY.key;
  const sweepTooMany = sweepCombos > SWEEP_MAX_COMBOS;

  // Any sweep-config edit invalidates a shown result: the visible controls would
  // otherwise describe a different sweep than the heatmap / 套用最佳 still acts on.
  // The applied-cell highlight is tied to that result, so it clears too.
  const clearSweep = () => {
    setSweepResult(null);
    setSweepErr(null);
    setAppliedCell(null);
    setAppliedKeys([]);
  };

  async function runSweep() {
    if (!selected || selected.id == null) {
      setSweepErr('請先選擇資料集');
      return;
    }
    setSweeping(true);
    setSweepErr(null);
    setSweepResult(null);
    setAppliedCell(null);
    setAppliedKeys([]);
    // Let "掃描中…" paint before the (synchronous, up-to-256-backtest) run.
    await new Promise((r) => setTimeout(r, 20));
    try {
      let cs = candles;
      if (!cs.length) {
        cs = toCoreCandles(await db.getCandles(selected.id, selected.start_time, selected.end_time));
        setCandles(cs);
      }
      if (!cs.length) throw new Error('此資料集沒有 K 線');
      setSweepResult(runParamSweep({ candles: cs, strat, interval: selected.interval, sweep: sweepConfig }));
    } catch (e) {
      setSweepErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSweeping(false);
    }
  }

  // Apply any sweep cell's param combo to the strategy + mark it as applied.
  function applySweepCombo(r: SweepResult, x: number, y: number | null) {
    setStrat((s) => {
      const next: ParamsStrategy = { ...s, [r.xKey]: x };
      if (r.yKey != null && y != null) next[r.yKey] = y;
      return next;
    });
    setAppliedCell({ x, y });
    const keys: NumKey[] = [r.xKey];
    if (r.yKey != null && y != null) keys.push(r.yKey);
    setAppliedKeys(keys);
    const label = `${SWEEP_PARAM_LABEL[r.xKey]}=${x}${r.yKey != null && y != null ? ` · ${SWEEP_PARAM_LABEL[r.yKey]}=${y}` : ''}`;
    setMsg(`已套用：${label}（記得再用樣本外驗證）`);
  }

  function applySweepBest() {
    const r = sweepResult;
    if (!r || !r.best) return;
    applySweepCombo(r, r.best.x, r.best.y);
  }

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

  // Bar-info readout (Slice 9): the "active" bar is the hovered bar if hovering,
  // else the replay cursor when replay is on, else none. Its OHLC + entry/exit
  // condition + position (from the last backtest's trades) feed the 此根資訊 row,
  // so pointing at any bar shows its info in ANY mode, not just at the cursor.
  const activeBar =
    hoverBar != null && hoverBar >= 0 && hoverBar < candles.length
      ? hoverBar
      : replayOn && candles.length > 0
        ? Math.min(replayCursor, candles.length - 1)
        : null;
  const activeCandle = activeBar != null ? candles[activeBar] : null;
  const liveEntry = activeBar != null && signalSeries ? !!signalSeries.entry[activeBar] : false;
  const liveExit = activeBar != null && signalSeries ? !!signalSeries.exit[activeBar] : false;
  const livePosition = activeBar != null && result ? positionAtTime(result.trades, candles[activeBar].t) : null;
  const posText = livePosition ? POS_LABEL[livePosition] : '—（回測後顯示）';
  const posColor = livePosition === 'LONG' ? '#1f7a57' : livePosition === 'SHORT' ? '#b23b2e' : '#8a8678';

  // Chart / metrics content, factored out so it can render inline OR (Slice 8a)
  // enlarged inside a FloatingPanel. Same state either way -> edits reflow live.
  const renderChart = (chartHeight: number) => (
    <CandleChart candles={candles} strat={strat} show={show} trades={result?.trades} upto={replayOn ? replayCursor : undefined} onHoverBar={setHoverBar} height={chartHeight} />
  );
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

  return (
    <div>
      {err && <div style={{ ...S.card, borderColor: '#d23b2f', color: '#b23b2e', marginBottom: 12 }}>{err}</div>}
      {msg && <div style={{ ...S.card, borderColor: '#2d9f73', color: '#1f7a57', marginBottom: 12 }}>{msg}</div>}

      {candles.length > 0 && (
        <section style={{ ...S.card, marginBottom: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 8, flexWrap: 'wrap' }}>
            <h2 style={{ ...S.h2, margin: 0 }}>圖表</h2>
            <div style={{ display: 'flex', gap: 10, fontSize: 11, color: '#8a8678' }}>
              {(['ma', 'ema', 'bb', 'rsi', 'vol', 'trades'] as (keyof OverlayToggles)[]).map((k) => (
                <label key={k} style={{ display: 'flex', alignItems: 'center', gap: 3, cursor: 'pointer' }}>
                  <input type="checkbox" checked={show[k]} onChange={(e) => setShow((s) => ({ ...s, [k]: e.target.checked }))} />
                  {OVERLAY_LABEL[k]}
                </label>
              ))}
            </div>
            <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 11, color: '#aaa599' }}>
              {loadingCandles ? '載入中…' : `${selected?.symbol ?? ''} · ${candles.length} 根`}
            </span>
            <button data-testid="popout-chart" title="放大到獨立面板" style={{ ...S.btnGhost, padding: '3px 10px', marginLeft: 'auto' }} onClick={() => setPoppedChart((v) => !v)}>
              {poppedChart ? '⤡ 收合' : '⤢ 放大'}
            </button>
          </div>
          {poppedChart ? <PoppedOutNote label="圖表" onClose={() => setPoppedChart(false)} /> : renderChart(360)}

          <div style={{ display: 'flex', gap: 8, marginTop: 10, flexWrap: 'wrap', alignItems: 'center', borderTop: '1px solid #efece5', paddingTop: 10 }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: '#8a8678' }}>
              <input type="checkbox" data-testid="replay-toggle" checked={replayOn} onChange={(e) => { setReplayOn(e.target.checked); if (!e.target.checked) setReplayPlaying(false); }} />
              回放模式
            </label>
            <HelpTip id="replay" label="回放" text={HELP.replay} />
            {replayOn && (
              <>
                <button data-testid="replay-reset" style={{ ...S.btnGhost, padding: '3px 8px' }} title="回到最新" onClick={() => setReplayCursor(Math.max(0, candles.length - 1))}>⏮</button>
                <button data-testid="replay-back" style={{ ...S.btnGhost, padding: '3px 8px' }} title="上一根" onClick={() => setReplayCursor((c) => Math.max(0, c - 1))}>◀</button>
                <button data-testid="replay-play" style={{ ...S.btnGhost, padding: '3px 8px' }} title={replayPlaying ? '暫停' : '播放'} onClick={toggleReplayPlay}>{replayPlaying ? '⏸' : '⏵'}</button>
                <input
                  type="range"
                  data-testid="replay-cursor"
                  min={0}
                  max={Math.max(0, candles.length - 1)}
                  value={Math.min(replayCursor, candles.length - 1)}
                  onChange={(e) => setReplayCursor(Number(e.target.value))}
                  style={{ flex: 1, minWidth: 140 }}
                />
                <button data-testid="replay-fwd" style={{ ...S.btnGhost, padding: '3px 8px' }} title="下一根" onClick={() => setReplayCursor((c) => Math.min(candles.length - 1, c + 1))}>▶</button>
                <select data-testid="replay-speed" value={replaySpeed} onChange={(e) => setReplaySpeed(Number(e.target.value))} title="播放速度" style={{ ...S.input, width: 56, fontSize: 11, padding: '3px 4px' }}>
                  <option value={1}>1×</option>
                  <option value={2}>2×</option>
                  <option value={4}>4×</option>
                </select>
                <span data-testid="replay-readout" style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 11, color: '#16150f', minWidth: 120 }}>
                  第 {Math.min(replayCursor, candles.length - 1) + 1} / {candles.length} 根
                </span>
              </>
            )}
          </div>

          {activeBar != null && activeCandle && (
            <div data-testid="bar-info" style={{ display: 'flex', gap: 12, marginTop: 6, flexWrap: 'wrap', alignItems: 'center', fontFamily: "'IBM Plex Mono', monospace", fontSize: 11 }}>
              <span style={{ color: '#8a8678' }}>第 {activeBar + 1} 根{hoverBar != null ? '（游標）' : ''}</span>
              <span style={{ color: '#3c3a30' }}>開 {activeCandle.o.toFixed(2)} 高 {activeCandle.h.toFixed(2)} 低 {activeCandle.l.toFixed(2)} 收 {activeCandle.c.toFixed(2)} · 量 {activeCandle.v.toFixed(0)}</span>
              <span style={{ color: liveEntry ? '#1f7a57' : '#aaa599', fontWeight: liveEntry ? 700 : 400 }}>進場 {liveEntry ? '✓ 成立' : '✗'}</span>
              <span style={{ color: liveExit ? '#b23b2e' : '#aaa599', fontWeight: liveExit ? 700 : 400 }}>出場 {liveExit ? '✓ 成立' : '✗'}</span>
              <span style={{ color: '#8a8678' }}>持倉 <b data-testid="bar-position" style={{ color: posColor }}>{posText}</b></span>
            </div>
          )}

          <div style={{ display: 'flex', gap: 12, marginTop: 10, flexWrap: 'wrap', alignItems: 'flex-end', borderTop: '1px solid #efece5', paddingTop: 10 }}>
            {QUICK_FIELDS.map((f) => (
              <label key={f.key} data-testid={isAppliedKey(f.key) ? `quick-applied-${f.key}` : undefined} style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <span style={appliedLabelStyle(f.key)}>{isAppliedKey(f.key) ? `✓ ${f.label}` : f.label}</span>
                <NumberInput value={strat[f.key]} onChange={(n) => setNum(f.key, n)} style={appliedInputStyle(f.key, { ...S.input, width: 88 })} />
              </label>
            ))}
            <span style={{ fontSize: 10, color: '#aaa599', alignSelf: 'center' }}>調整即時重畫；完整參數見下方策略表單（<span style={{ color: '#2f6df0' }}>✓ 藍框</span>＝由掃描套用）</span>
          </div>
        </section>
      )}

      <div style={S.panel}>
        {/* left column: data + strategy */}
        <div style={{ display: 'grid', gap: 12 }}>
          <section style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, margin: '0 0 8px' }}>
              <h2 style={{ ...S.h2, margin: 0 }}>資料集</h2>
              <HelpTip id="dataset" label="資料集" text={HELP.dataset} />
            </div>
            <select
              value={selId ?? ''}
              onChange={(e) => setSelId(e.target.value ? Number(e.target.value) : null)}
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
              <button data-testid="load-sample" style={S.btnGhost} onClick={loadSample} disabled={busyData || !isTauri()} aria-busy={busyData}>
                {busyData ? '處理中…' : '載入樣本資料'}
              </button>
              <button style={S.btnGhost} onClick={() => refresh().catch((e) => setErr(String(e)))} disabled={!isTauri()}>
                重新整理
              </button>
            </div>
            <textarea
              value={importText}
              onChange={(e) => setImportText(e.target.value)}
              placeholder='貼上 K 線 JSON：[{ "t":.., "o":.., "h":.., "l":.., "c":.., "v":.. }, …] 或 { "symbol":"BTCUSDT","interval":"1h","candles":[…] }'
              rows={3}
              style={{ ...S.input, fontSize: 11, resize: 'vertical' }}
            />
            <button style={{ ...S.btnGhost, marginTop: 6 }} onClick={importJson} disabled={busyData || !importText.trim() || !isTauri()} aria-busy={busyData}>
              匯入 JSON
            </button>
          </section>

          <section style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
              <h2 style={{ ...S.h2, margin: 0 }}>策略</h2>
              <HelpTip id="strategy" label="策略" text={HELP.strategy} />
              <div style={{ display: 'flex', gap: 2 }}>
                {(['params', 'blocks', 'code'] as const).map((mode) => (
                  <button
                    key={mode}
                    onClick={() => setStrat((s) => ({ ...s, mode }))}
                    style={{
                      ...S.btnGhost,
                      padding: '3px 10px',
                      background: strat.mode === mode ? '#16150f' : '#efece5',
                      color: strat.mode === mode ? '#fff' : '#16150f',
                      borderColor: strat.mode === mode ? '#16150f' : '#d6d2c8',
                    }}
                  >
                    {MODE_LABEL[mode]}
                  </button>
                ))}
              </div>
            </div>

            {strat.mode === 'params' && (
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 7, marginBottom: 8 }}>
                <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={S.label}>進場訊號</span>
                  <select value={strat.entrySig} onChange={(e) => setStrat((s) => ({ ...s, entrySig: e.target.value as SignalId }))} style={S.input}>
                    {SUPPORTED_SIGNALS.map((id) => (
                      <option key={id} value={id}>{SIG_LABEL[id]}</option>
                    ))}
                  </select>
                </label>
                <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={S.label}>出場訊號</span>
                  <select value={strat.exitSig} onChange={(e) => setStrat((s) => ({ ...s, exitSig: e.target.value as SignalId }))} style={S.input}>
                    {SUPPORTED_SIGNALS.map((id) => (
                      <option key={id} value={id}>{SIG_LABEL[id]}</option>
                    ))}
                  </select>
                </label>
              </div>
            )}
            {strat.mode === 'blocks' && (
              <>
                <datalist id="operand-list">
                  {OPERAND_IDS.map((id) => <option key={id} value={id} />)}
                </datalist>
                <RuleRows title="進場規則" rules={strat.entryRules} onChange={(rules) => setStrat((s) => ({ ...s, entryRules: rules }))} />
                <RuleRows title="出場規則" rules={strat.exitRules} onChange={(rules) => setStrat((s) => ({ ...s, exitRules: rules }))} />
              </>
            )}
            {strat.mode === 'code' && (
              <div style={{ marginBottom: 8 }}>
                <CodeField label="進場條件 (entry)" value={strat.entryCode} onChange={(v) => setStrat((s) => ({ ...s, entryCode: v }))} />
                <CodeField label="出場條件 (exit)" value={strat.exitCode} onChange={(v) => setStrat((s) => ({ ...s, exitCode: v }))} />
                <div style={{ fontSize: 10, color: '#8a8678', lineHeight: 1.5 }}>
                  變數：{OPERAND_IDS.join(' ')}
                  <br />
                  函式：prev(x) · crossUp(a,b) · crossDown(a,b)　運算子：+ - * / &gt; &lt; &gt;= &lt;= == != &amp;&amp; || !
                </div>
                <div style={{ fontSize: 10, color: '#aaa599', marginTop: 4 }}>
                  code 模式為手動專用，AI 不會使用；以安全直譯器求值（無 eval）。
                </div>
              </div>
            )}

            <div style={S.grid3}>
              {IND_FIELDS.map((f) => (
                <label key={f.key} data-testid={isAppliedKey(f.key) ? `applied-${f.key}` : undefined} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={appliedLabelStyle(f.key)}>{isAppliedKey(f.key) ? `✓ ${f.label}` : f.label}</span>
                  <NumberInput value={strat[f.key]} onChange={(n) => setNum(f.key, n)} style={appliedInputStyle(f.key, S.input)} />
                </label>
              ))}
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: 6, margin: '12px 0 8px' }}>
              <h2 style={{ ...S.h2, margin: 0 }}>執行模型</h2>
              <HelpTip id="exec" label="執行模型" text={HELP.exec} />
            </div>
            <div style={S.grid3}>
              {EXEC_FIELDS.map((f) => (
                <label key={f.key} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={S.label}>{f.label}</span>
                  <NumberInput value={strat[f.key]} onChange={(n) => setNum(f.key, n)} style={S.input} />
                </label>
              ))}
              <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                <span style={S.label}>方向</span>
                <select value={strat.direction} onChange={(e) => setStrat((s) => ({ ...s, direction: e.target.value as ParamsStrategy['direction'] }))} style={S.input}>
                  <option value="long">做多</option>
                  <option value="short">做空</option>
                  <option value="both">雙向</option>
                </select>
              </label>
              <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                <span style={S.label}>成交價</span>
                <select value={strat.fillMode} onChange={(e) => setStrat((s) => ({ ...s, fillMode: e.target.value as ParamsStrategy['fillMode'] }))} style={S.input}>
                  <option value="close">當根收盤</option>
                  <option value="nextOpen">次根開盤</option>
                </select>
              </label>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 12, flexWrap: 'wrap' }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: '#8a8678', flexWrap: 'wrap' }}>
                <input
                  type="checkbox"
                  data-testid="holdout-toggle"
                  checked={holdout}
                  onChange={(e) => {
                    const checked = e.target.checked;
                    setHoldout(checked);
                    if (!checked) setHoldoutResult(null);
                  }}
                />
                Holdout 樣本外驗證
                {holdout && (
                  <>
                    <span style={{ color: '#cfccc4' }}>·</span>末
                    <NumberInput
                      value={holdoutPct}
                      min={5}
                      max={90}
                      onChange={(n) => {
                        setHoldoutPct(n);
                        setHoldoutResult(null); // stale split no longer matches the new %
                      }}
                      style={{ ...S.input, width: 52, fontSize: 11, padding: '3px 5px' }}
                    />
                    % 為樣本外
                  </>
                )}
              </label>
              <HelpTip id="holdout" label="Holdout" text={HELP.holdout} />
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 8 }}>
              <button data-testid="run-backtest" style={{ ...S.btn, flex: 1 }} onClick={run} disabled={running || !selected} aria-busy={running}>
                {running ? '回測中…' : '▶ 執行回測'}
              </button>
              <HelpTip id="run" label="執行回測" text={HELP.run} align="right" />
            </div>
          </section>
        </div>

        {/* right column: results */}
        <section style={S.card}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6, margin: '0 0 8px' }}>
            <h2 style={{ ...S.h2, margin: 0 }}>回測績效</h2>
            <HelpTip id="metrics" label="回測績效" text={HELP.metrics} />
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
                <input
                  value={stratName}
                  onChange={(e) => setStratName(e.target.value)}
                  placeholder="策略名稱（可留空）"
                  style={{ ...S.input, flex: 1 }}
                />
                <button style={S.btn} onClick={save} disabled={saving} aria-busy={saving}>
                  {saving ? '儲存中…' : '儲存結果'}
                </button>
                <HelpTip id="save" label="儲存結果" text={HELP.save} align="right" />
              </div>
              <p style={{ color: '#aaa599', fontSize: 11, marginTop: 8 }}>
                儲存會寫入 strategy_def + backtest_summary（segment=full），經由 metricsToBacktestSummary()。
              </p>
            </>
          )}
        </section>
      </div>

      {candles.length > 0 && (
        <section style={{ ...S.card, marginTop: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: sweepOpen ? 10 : 0, flexWrap: 'wrap' }}>
            <h2 style={{ ...S.h2, margin: 0 }}>參數掃描（最佳化）</h2>
            <HelpTip id="sweep" label="參數掃描" text={HELP.sweep} />
            <button data-testid="sweep-toggle" style={{ ...S.btnGhost, padding: '3px 10px' }} onClick={() => setSweepOpen((o) => !o)}>
              {sweepOpen ? '收合' : '展開'}
            </button>
            <span style={{ fontSize: 10, color: '#aaa599' }}>選 1–2 個參數掃範圍，找最佳值（上限 {SWEEP_MAX_COMBOS} 組）。</span>
          </div>

          {sweepOpen && (
            <>
              {/* controls bar: metric + 2-D toggle + live combo count (axes get
                  their own full-width rows below, so nothing can overlap) */}
              <div style={{ display: 'flex', gap: 16, alignItems: 'flex-end', flexWrap: 'wrap' }}>
                <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={S.label}>最佳化指標</span>
                  <select data-testid="sweep-metric" value={sweepMetric} onChange={(e) => { clearSweep(); setSweepMetric(e.target.value as SweepMetricId); }} style={{ ...S.input, fontSize: 11, minWidth: 120 }}>
                    {SWEEP_METRIC_IDS.map((m) => <option key={m} value={m}>{SWEEP_METRIC_LABEL[m]}</option>)}
                  </select>
                </label>
                <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: '#8a8678' }}>
                  <input type="checkbox" data-testid="sweep-2d" checked={sweepUse2d} onChange={(e) => { clearSweep(); setSweepUse2d(e.target.checked); }} />
                  第二維 Y（二維熱力圖）
                </label>
                <span data-testid="sweep-combos" style={{ fontSize: 11, color: sweepTooMany || sweepDupKey ? '#b23b2e' : '#aaa599' }}>
                  {sweepDupKey ? 'X / Y 參數需不同' : `組合數 ${sweepCombos}${sweepTooMany ? `（超過上限 ${SWEEP_MAX_COMBOS}）` : ''}`}
                </span>
              </div>

              <AxisEditor title="X 參數" axis={sweepX} onChange={(a) => { clearSweep(); setSweepX(a); }} />
              {sweepUse2d && <AxisEditor title="Y 參數" axis={sweepY} onChange={(a) => { clearSweep(); setSweepY(a); }} />}

              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 12, flexWrap: 'wrap' }}>
                <button data-testid="run-sweep" style={S.btn} onClick={runSweep} disabled={sweeping || sweepTooMany || sweepDupKey} aria-busy={sweeping}>
                  {sweeping ? '掃描中…' : '▶ 執行掃描'}
                </button>
                <HelpTip id="run-sweep" label="執行掃描" text={HELP.runSweep} />
                {sweepResult?.best && (
                  <>
                    <button data-testid="apply-best" style={S.btnGhost} onClick={applySweepBest}>套用最佳：{sweepBestLabel(sweepResult)}</button>
                    <HelpTip id="apply-best" label="套用最佳" text={HELP.applyBest} />
                  </>
                )}
                <span style={{ fontSize: 10, color: '#8a7a3a' }}>注意：歷史最佳常為過度擬合，務必再用樣本外驗證。</span>
              </div>

              {sweepErr && <div style={{ fontSize: 12, color: '#b23b2e', marginTop: 8 }}>{sweepErr}</div>}
              {sweepResult && (
                <SweepHeatmap
                  result={sweepResult}
                  applied={appliedCell}
                  onPick={(x, y) => applySweepCombo(sweepResult, x, y)}
                />
              )}
            </>
          )}
        </section>
      )}

      {/* Slice 8a pop-outs: non-modal floating panels rendering the same content
          enlarged; the left-column controls stay usable while these are open. */}
      {poppedChart && candles.length > 0 && (
        <FloatingPanel title="圖表" testId="chart-popout" initial={{ x: 430, y: 70, w: 800, h: 540 }} onClose={() => setPoppedChart(false)}>
          {(s) => renderChart(s.h)}
        </FloatingPanel>
      )}
      {poppedMetrics && result && (
        <FloatingPanel title="回測績效" testId="metrics-popout" initial={{ x: 220, y: 130, w: 460, h: 520 }} onClose={() => setPoppedMetrics(false)}>
          {() => renderMetricsTable(15)}
        </FloatingPanel>
      )}
    </div>
  );
}
