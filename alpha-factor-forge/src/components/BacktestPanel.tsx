// Slice 2 — single-strategy (params mode) backtest panel.
//
// Vertical slice: pick/import a dataset (SQLite) -> edit params-mode strategy ->
// run via the Slice 1 service (core/* under the hood) -> show metrics -> save
// the result (strategy_def + backtest_summary). No chart / sweep / replay /
// live / library yet — those are later slices. All persistence goes through
// tauri-client; all maths through core/* + src/services.

import React, { useCallback, useEffect, useState } from 'react';
import { db, isTauri, type Candle, type Dataset } from '../tauri-client/commands';
import { importDataset } from '../tauri-client/dbClient';
import { defaultStrategy, OPERAND_IDS, type ParamsStrategy, type SignalId, type Rule, type RuleOp, type OperandId } from '../services/strategy';
import { SUPPORTED_SIGNALS } from '../services/strategySignals';
import { runParamsBacktest } from '../services/backtestRunner';
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
    try {
      let cs = candles;
      if (!cs.length) {
        cs = toCoreCandles(await db.getCandles(selected.id, selected.start_time, selected.end_time));
        setCandles(cs);
      }
      if (!cs.length) throw new Error('此資料集沒有 K 線');
      setResult(runParamsBacktest({ candles: cs, strat, interval: selected.interval }));
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
      setMsg(`已存檔：strategy #${strategyId} · dataset #${selected.id} · ${result.trades.length} trades`);
    } catch (e) {
      setErr(String(e));
    } finally {
      setSaving(false);
    }
  }

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
                <input
                  type="number"
                  value={strat[f.key]}
                  onChange={(e) => setNum(f.key, parseFloat(e.target.value) || 0)}
                  style={{ ...S.input, width: 88 }}
                />
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
              <button style={S.btnGhost} onClick={loadSample} disabled={busyData || !isTauri()}>
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
                {(['params', 'blocks'] as const).map((mode) => (
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
                    {mode === 'params' ? '參數' : '積木'}
                  </button>
                ))}
              </div>
            </div>

            {strat.mode === 'params' ? (
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
            ) : (
              <>
                <datalist id="operand-list">
                  {OPERAND_IDS.map((id) => <option key={id} value={id} />)}
                </datalist>
                <RuleRows title="進場規則" rules={strat.entryRules} onChange={(rules) => setStrat((s) => ({ ...s, entryRules: rules }))} />
                <RuleRows title="出場規則" rules={strat.exitRules} onChange={(rules) => setStrat((s) => ({ ...s, exitRules: rules }))} />
              </>
            )}

            <div style={S.grid3}>
              {IND_FIELDS.map((f) => (
                <label key={f.key} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={S.label}>{f.label}</span>
                  <input
                    type="number"
                    value={strat[f.key]}
                    onChange={(e) => setNum(f.key, parseFloat(e.target.value) || 0)}
                    style={S.input}
                  />
                </label>
              ))}
            </div>

            <h2 style={{ ...S.h2, margin: '12px 0 8px' }}>執行模型</h2>
            <div style={S.grid3}>
              {EXEC_FIELDS.map((f) => (
                <label key={f.key} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={S.label}>{f.label}</span>
                  <input
                    type="number"
                    value={strat[f.key]}
                    onChange={(e) => setNum(f.key, parseFloat(e.target.value) || 0)}
                    style={S.input}
                  />
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

            <button style={{ ...S.btn, width: '100%', marginTop: 12 }} onClick={run} disabled={running || !selected}>
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
                <tbody>
                  {METRIC_ROWS.map((r) => (
                    <tr key={r.label} style={{ borderBottom: '1px solid #efece5' }}>
                      <td style={{ padding: '5px 4px', color: '#8a8678' }}>{r.label}</td>
                      <td style={{ padding: '5px 4px', textAlign: 'right', fontWeight: 600 }}>{r.fmt(result.metrics)}</td>
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
    </div>
  );
}
