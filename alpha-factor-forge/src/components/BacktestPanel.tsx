// Slice 2 — single-strategy (params mode) backtest panel.
//
// Vertical slice: pick/import a dataset (SQLite) -> edit params-mode strategy ->
// run via the Slice 1 service (core/* under the hood) -> show metrics -> save
// the result (strategy_def + backtest_summary). No chart / sweep / replay /
// live / library yet — those are later slices. All persistence goes through
// tauri-client; all maths through core/* + src/services.

import React, { useCallback, useEffect, useState } from 'react';
import { db, isTauri, importDataset } from '../tauri-client/dataClient';
import type { Candle, Dataset } from '../tauri-client/commands';
import { defaultStrategy, OPERAND_IDS, type ParamsStrategy, type SignalId, type Rule, type RuleOp, type OperandId } from '../services/strategy';
import { SUPPORTED_SIGNALS } from '../services/strategySignals';
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
import { CandleChart, type OverlayToggles } from '../charts/CandleChart';
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

const OVERLAY_LABEL: Record<keyof OverlayToggles, string> = { ma: 'MA', ema: 'EMA', bb: 'BB', rsi: 'RSI', vol: '量' };

const OPERAND_LABEL: Record<OperandId, string> = {
  price: '價格', open: '開', high: '高', low: '低', volume: '量',
  maFast: '快線', maSlow: '慢線', ema: 'EMA', rsi: 'RSI',
  macd: 'MACD', macdSignal: 'MACD訊號', macdHist: 'MACD柱',
  bbUpper: '布林上', bbMid: '布林中', bbLower: '布林下',
};
const RULE_OPS: RuleOp[] = ['>', '<', '>=', '<=', 'crossUp', 'crossDown'];
const OP_LABEL: Record<RuleOp, string> = { '>': '>', '<': '<', '>=': '≥', '<=': '≤', crossUp: '上穿', crossDown: '下穿' };

const MODE_LABEL: Record<ParamsStrategy['mode'], string> = { params: '參數', blocks: '積木', code: '程式碼' };

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

/** One sweep axis: param picker + min / max / step. */
function AxisEditor({ title, axis, onChange }: { title: string; axis: SweepAxisConfig; onChange: (a: SweepAxisConfig) => void }): React.ReactElement {
  return (
    <div style={{ display: 'grid', gap: 4 }}>
      <span style={S.label}>{title}</span>
      <select value={axis.key} onChange={(e) => onChange({ ...axis, key: e.target.value as SweepParamKey })} style={{ ...S.input, fontSize: 11 }}>
        {SWEEP_PARAM_KEYS.map((k) => <option key={k} value={k}>{SWEEP_PARAM_LABEL[k]}</option>)}
      </select>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 4 }}>
        {([['min', '起'], ['max', '迄'], ['step', '間距']] as const).map(([k, lbl]) => (
          <label key={k} style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <span style={S.label}>{lbl}</span>
            <NumberInput value={axis[k]} onChange={(n) => onChange({ ...axis, [k]: n })} style={{ ...S.input, fontSize: 11 }} />
          </label>
        ))}
      </div>
    </div>
  );
}

/** Red→yellow→green heatmap of a sweep grid; best cell is outlined. */
function SweepHeatmap({ result }: { result: SweepResult }): React.ReactElement {
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
        {is2d ? `、縱軸 ${SWEEP_PARAM_LABEL[yKey]}` : ''}。每格為指標值，括號為交易次數；最佳格描黑框。
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
                const t = c.metric == null ? 0 : span > 0 ? (c.metric - lo) / span : 1;
                const bg = c.metric == null ? '#e8e6df' : heatColor(t);
                return (
                  <td
                    key={ci}
                    data-testid={isBest ? 'sweep-best-cell' : undefined}
                    title={`${SWEEP_PARAM_LABEL[xKey]}=${c.x}${is2d ? ` · ${SWEEP_PARAM_LABEL[yKey]}=${c.y}` : ''}`}
                    style={{ ...cell, background: bg, color: '#16150f', border: isBest ? '2px solid #16150f' : '1px solid #fff', fontWeight: isBest ? 700 : 500 }}
                  >
                    <div>{fmtSweepMetric(metric, c.metric)}</div>
                    <div style={{ fontSize: 9, color: '#3c3a30' }}>({c.trades})</div>
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

export function BacktestPanel(): React.ReactElement {
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [selId, setSelId] = useState<number | null>(null);
  const [strat, setStrat] = useState<ParamsStrategy>(defaultStrategy);
  const [stratName, setStratName] = useState('');
  const [result, setResult] = useState<BacktestResult | null>(null);
  const [running, setRunning] = useState(false);
  const [saving, setSaving] = useState(false);
  const [busyData, setBusyData] = useState(false);
  const [importText, setImportText] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [candles, setCandles] = useState<CoreCandle[]>([]);
  const [loadingCandles, setLoadingCandles] = useState(false);
  const [show, setShow] = useState<OverlayToggles>({ ma: true, ema: false, bb: false, rsi: true, vol: true });
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

  const selected = datasets.find((d) => d.id === selId) ?? null;
  const setNum = (key: NumKey, value: number) => setStrat((s) => ({ ...s, [key]: value }));

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

  const sweepConfig: SweepConfig = { x: sweepX, y: sweepUse2d ? sweepY : null, metric: sweepMetric };
  const sweepCombos = countSweepCombos(sweepConfig);
  const sweepDupKey = sweepUse2d && sweepX.key === sweepY.key;
  const sweepTooMany = sweepCombos > SWEEP_MAX_COMBOS;

  // Any sweep-config edit invalidates a shown result: the visible controls would
  // otherwise describe a different sweep than the heatmap / 套用最佳 still acts on.
  const clearSweep = () => {
    setSweepResult(null);
    setSweepErr(null);
  };

  async function runSweep() {
    if (!selected || selected.id == null) {
      setSweepErr('請先選擇資料集');
      return;
    }
    setSweeping(true);
    setSweepErr(null);
    setSweepResult(null);
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

  function applySweepBest() {
    const r = sweepResult;
    if (!r || !r.best) return;
    const best = r.best;
    setStrat((s) => {
      const next: ParamsStrategy = { ...s, [r.xKey]: best.x };
      if (r.yKey != null && best.y != null) next[r.yKey] = best.y;
      return next;
    });
    setMsg(`已套用最佳參數：${sweepBestLabel(r)}（記得再用樣本外驗證）`);
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

  return (
    <div>
      {err && <div style={{ ...S.card, borderColor: '#d23b2f', color: '#b23b2e', marginBottom: 12 }}>{err}</div>}
      {msg && <div style={{ ...S.card, borderColor: '#2d9f73', color: '#1f7a57', marginBottom: 12 }}>{msg}</div>}

      {candles.length > 0 && (
        <section style={{ ...S.card, marginBottom: 12 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 8, flexWrap: 'wrap' }}>
            <h2 style={{ ...S.h2, margin: 0 }}>圖表</h2>
            <div style={{ display: 'flex', gap: 10, fontSize: 11, color: '#8a8678' }}>
              {(['ma', 'ema', 'bb', 'rsi', 'vol'] as (keyof OverlayToggles)[]).map((k) => (
                <label key={k} style={{ display: 'flex', alignItems: 'center', gap: 3, cursor: 'pointer' }}>
                  <input type="checkbox" checked={show[k]} onChange={(e) => setShow((s) => ({ ...s, [k]: e.target.checked }))} />
                  {OVERLAY_LABEL[k]}
                </label>
              ))}
            </div>
            <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 11, color: '#aaa599' }}>
              {loadingCandles ? '載入中…' : `${selected?.symbol ?? ''} · ${candles.length} 根`}
            </span>
          </div>
          <CandleChart candles={candles} strat={strat} show={show} />
          <div style={{ display: 'flex', gap: 12, marginTop: 10, flexWrap: 'wrap', alignItems: 'flex-end', borderTop: '1px solid #efece5', paddingTop: 10 }}>
            {QUICK_FIELDS.map((f) => (
              <label key={f.key} style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <span style={S.label}>{f.label}</span>
                <NumberInput value={strat[f.key]} onChange={(n) => setNum(f.key, n)} style={{ ...S.input, width: 88 }} />
              </label>
            ))}
            <span style={{ fontSize: 10, color: '#aaa599', alignSelf: 'center' }}>調整即時重畫；完整參數見下方策略表單</span>
          </div>
        </section>
      )}

      <div style={S.panel}>
        {/* left column: data + strategy */}
        <div style={{ display: 'grid', gap: 12 }}>
          <section style={S.card}>
            <h2 style={S.h2}>資料集</h2>
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
              <button data-testid="load-sample" style={S.btnGhost} onClick={loadSample} disabled={busyData || !isTauri()}>
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
            <button style={{ ...S.btnGhost, marginTop: 6 }} onClick={importJson} disabled={busyData || !importText.trim() || !isTauri()}>
              匯入 JSON
            </button>
          </section>

          <section style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
              <h2 style={{ ...S.h2, margin: 0 }}>策略</h2>
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
                <label key={f.key} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={S.label}>{f.label}</span>
                  <NumberInput value={strat[f.key]} onChange={(n) => setNum(f.key, n)} style={S.input} />
                </label>
              ))}
            </div>

            <h2 style={{ ...S.h2, margin: '12px 0 8px' }}>執行模型</h2>
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

            <label style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 12, fontSize: 11, color: '#8a8678', flexWrap: 'wrap' }}>
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

            <button data-testid="run-backtest" style={{ ...S.btn, width: '100%', marginTop: 8 }} onClick={run} disabled={running || !selected}>
              {running ? '回測中…' : '▶ 執行回測'}
            </button>
          </section>
        </div>

        {/* right column: results */}
        <section style={S.card}>
          <h2 style={S.h2}>回測績效</h2>
          {!result && <p style={{ color: '#aaa599', fontSize: 12 }}>尚未回測 — 選資料集、設策略後按「執行回測」。</p>}
          {result && (
            <>
              <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: "'IBM Plex Mono', monospace", fontSize: 12 }}>
                {metricCols.length > 1 && (
                  <thead>
                    <tr style={{ borderBottom: '1px solid #d6d2c8' }}>
                      <th />
                      {metricCols.map((c) => (
                        <th key={c.label} data-testid={`col-${c.label}`} style={{ padding: '4px', textAlign: 'right', fontSize: 10, fontWeight: 600, color: '#8a8678' }}>{c.label}</th>
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

              <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 12 }}>
                <input
                  value={stratName}
                  onChange={(e) => setStratName(e.target.value)}
                  placeholder="策略名稱（可留空）"
                  style={{ ...S.input, flex: 1 }}
                />
                <button style={S.btn} onClick={save} disabled={saving}>
                  {saving ? '儲存中…' : '儲存結果'}
                </button>
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
            <button data-testid="sweep-toggle" style={{ ...S.btnGhost, padding: '3px 10px' }} onClick={() => setSweepOpen((o) => !o)}>
              {sweepOpen ? '收合' : '展開'}
            </button>
            <span style={{ fontSize: 10, color: '#aaa599' }}>選 1–2 個參數掃範圍，找最佳值（上限 {SWEEP_MAX_COMBOS} 組）。</span>
          </div>

          {sweepOpen && (
            <>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 200px', gap: 12, alignItems: 'start' }}>
                <AxisEditor title="X 參數" axis={sweepX} onChange={(a) => { clearSweep(); setSweepX(a); }} />
                <div>
                  <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: '#8a8678', marginBottom: 6 }}>
                    <input type="checkbox" data-testid="sweep-2d" checked={sweepUse2d} onChange={(e) => { clearSweep(); setSweepUse2d(e.target.checked); }} />
                    第二維 Y（二維熱力圖）
                  </label>
                  {sweepUse2d && <AxisEditor title="Y 參數" axis={sweepY} onChange={(a) => { clearSweep(); setSweepY(a); }} />}
                </div>
                <div style={{ display: 'grid', gap: 4 }}>
                  <span style={S.label}>最佳化指標</span>
                  <select data-testid="sweep-metric" value={sweepMetric} onChange={(e) => { clearSweep(); setSweepMetric(e.target.value as SweepMetricId); }} style={{ ...S.input, fontSize: 11 }}>
                    {SWEEP_METRIC_IDS.map((m) => <option key={m} value={m}>{SWEEP_METRIC_LABEL[m]}</option>)}
                  </select>
                  <span data-testid="sweep-combos" style={{ fontSize: 10, color: sweepTooMany || sweepDupKey ? '#b23b2e' : '#aaa599' }}>
                    {sweepDupKey ? 'X / Y 參數需不同' : `組合數 ${sweepCombos}${sweepTooMany ? `（超過上限 ${SWEEP_MAX_COMBOS}）` : ''}`}
                  </span>
                </div>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 10, flexWrap: 'wrap' }}>
                <button data-testid="run-sweep" style={S.btn} onClick={runSweep} disabled={sweeping || sweepTooMany || sweepDupKey}>
                  {sweeping ? '掃描中…' : '▶ 執行掃描'}
                </button>
                {sweepResult?.best && (
                  <button data-testid="apply-best" style={S.btnGhost} onClick={applySweepBest}>套用最佳：{sweepBestLabel(sweepResult)}</button>
                )}
                <span style={{ fontSize: 10, color: '#8a7a3a' }}>注意：歷史最佳常為過度擬合，務必再用樣本外驗證。</span>
              </div>

              {sweepErr && <div style={{ fontSize: 12, color: '#b23b2e', marginTop: 8 }}>{sweepErr}</div>}
              {sweepResult && <SweepHeatmap result={sweepResult} />}
            </>
          )}
        </section>
      )}
    </div>
  );
}
