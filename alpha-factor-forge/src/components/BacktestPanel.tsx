// Slice 2 — single-strategy (params mode) backtest panel.
//
// Vertical slice: pick/import a dataset (SQLite) -> edit params-mode strategy ->
// run via the Slice 1 service (core/* under the hood) -> show metrics -> save
// the result (strategy_def + backtest_summary). No chart / sweep / replay /
// live / library yet — those are later slices. All persistence goes through
// tauri-client; all maths through core/* + src/services.

import React, { useCallback, useEffect, useState } from 'react';
import { db, isTauri, importDataset } from '../tauri-client/dataClient';
import type { Candle, Dataset, StrategyDef } from '../tauri-client/commands';
import { defaultStrategy, OPERAND_IDS, type ParamsStrategy, type SignalId, type Rule, type RuleOp, type OperandId } from '../services/strategy';
import { SUPPORTED_SIGNALS } from '../services/strategySignals';
import { compileExpression } from '../services/exprInterpreter';
import { runParamsBacktest } from '../services/backtestRunner';
import { SWEEP_MAX_COMBOS } from '../services/paramSweep';
import { holdoutSplitIndex } from '../services/holdout';
import { toCoreCandles } from '../services/candleAdapter';
import { makeSampleCandles } from '../services/sampleData';
import { buildStrategyDef } from '../services/strategyRecord';
import { strategyFromDef } from '../services/strategyLibrary';
import { metricsToBacktestSummary } from '../services/metricsMapper';
import { HelpTip } from './HelpTip';
import { NumberInput } from './NumberInput';
import { SweepSection } from './SweepSection';
import { ChartSection } from './ChartSection';
import { DatasetSection } from './DatasetSection';
import { ResultsSection } from './ResultsSection';
import { S } from './panelStyles';
import type { NumKey } from './panelTypes';
import type { BacktestResult, Candle as CoreCandle } from '../core/backtest';

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

const OPERAND_LABEL: Record<OperandId, string> = {
  price: '價格', open: '開', high: '高', low: '低', volume: '量',
  maFast: '快線', maSlow: '慢線', ema: 'EMA', rsi: 'RSI',
  macd: 'MACD', macdSignal: 'MACD訊號', macdHist: 'MACD柱',
  bbUpper: '布林上', bbMid: '布林中', bbLower: '布林下',
};
const RULE_OPS: RuleOp[] = ['>', '<', '>=', '<=', 'crossUp', 'crossDown'];
const OP_LABEL: Record<RuleOp, string> = { '>': '>', '<': '<', '>=': '≥', '<=': '≤', crossUp: '上穿', crossDown: '下穿' };

const MODE_LABEL: Record<ParamsStrategy['mode'], string> = { params: '參數', blocks: '積木', code: '程式碼' };

// Slice 5c — short explanations shown by the "?" HelpTip markers. Kept as one
// map so the copy is easy to review/edit without hunting through the JSX.
const HELP: Record<string, string> = {
  dataset: '選擇或匯入 K 線資料集：載入內建樣本、貼上 JSON 匯入，或選既有資料集（SQLite）。回測與掃描都以此資料為輸入。',
  strategy: '定義進出場邏輯。參數＝挑現成訊號；積木＝用運算元組規則；程式碼＝手動撰寫安全運算式（AI 不會使用此模式）。',
  exec: '回測的成交假設：手續費、滑價、部位大小、停損／停利、方向（做多／做空／雙向），以及成交價（當根收盤或次根開盤）。',
  holdout: '把最後 N% 的 K 線留作樣本外（out-of-sample）。回測會同時列出全期／樣本內／樣本外，用來檢查是否過度擬合。',
  metrics: '策略在此資料集上的表現：淨報酬、CAGR、最大回撤、Sharpe／Sortino／Calmar、勝率、交易數、獲利因子等。',
  sweep: `自動改變 1–2 個參數掃過設定範圍，用熱力圖找較佳組合（上限 ${SWEEP_MAX_COMBOS} 組）。注意：歷史最佳常過度擬合，務必再用樣本外驗證。開啟 Holdout 時，掃描只使用樣本內資料（末段樣本外不參與最佳化）。`,
  run: '以目前策略與執行模型，在選定資料集上跑一次回測；結果顯示於右側「回測績效」。',
  save: '把策略與這次回測摘要寫入資料庫（strategy_def + backtest_summary，segment=full），經由 metricsToBacktestSummary()。',
  runSweep: `對每個參數組合各回測一次並畫成熱力圖（上限 ${SWEEP_MAX_COMBOS} 組）；掃描期間畫面顯示「掃描中…」。`,
  applyBest: '把最佳組合的參數套回策略表單（也可直接點熱力圖任一格套用該格的組合）。',
  replay: '回放模式：用滑桿或 ◀ / ▶ 一根一根前進，或按 ⏵ 自動播放（速度 1×–4×）；圖表只畫到目前這根，並顯示此根的進出場訊號與持倉（持倉依上次回測），之後的 K 線與買賣點會被隱藏，像重播當時看到的行情。',
};

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

export function BacktestPanel(): React.ReactElement {
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [selId, setSelId] = useState<number | null>(null);
  const [strat, setStrat] = useState<ParamsStrategy>(defaultStrategy);
  const [stratName, setStratName] = useState('');
  const [savedStrategies, setSavedStrategies] = useState<StrategyDef[]>([]);
  const [savedStrategyId, setSavedStrategyId] = useState<number | null>(null);
  const [loadingStrategies, setLoadingStrategies] = useState(false);
  const [result, setResult] = useState<BacktestResult | null>(null);
  const [running, setRunning] = useState(false);
  const [saving, setSaving] = useState(false);
  const [busyData, setBusyData] = useState(false);
  const [importText, setImportText] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [candles, setCandles] = useState<CoreCandle[]>([]);
  const [loadingCandles, setLoadingCandles] = useState(false);
  const [holdout, setHoldout] = useState(false);
  const [holdoutPct, setHoldoutPct] = useState(30); // last N% of bars = out-of-sample
  const [holdoutResult, setHoldoutResult] = useState<{ inSample: BacktestResult; outSample: BacktestResult } | null>(null);
  // Which strategy params the last sweep-apply set — highlighted in the form +
  // chart quick row so the user sees what the heatmap selection changed. A param
  // drops out of the set the moment it is hand-edited (no longer "from sweep").
  // Owned here (the strategy form + chart quick row read it); SweepSection sets
  // it via onApplyCombo and clears it via onClearApplied.
  const [appliedKeys, setAppliedKeys] = useState<NumKey[]>([]);
  // Bumped to tell SweepSection to drop its shown result (e.g. on strategy load).
  const [sweepResetSignal, setSweepResetSignal] = useState(0);

  const refresh = useCallback(async () => {
    const ds = await db.getDatasets();
    setDatasets(ds);
    setSelId((prev) => prev ?? ds[0]?.id ?? null);
  }, []);

  const refreshStrategies = useCallback(async () => {
    setLoadingStrategies(true);
    try {
      const rows = await db.getStrategies();
      setSavedStrategies(rows);
      setSavedStrategyId((current) => current != null && rows.some((row) => row.id === current) ? current : null);
      return rows;
    } finally {
      setLoadingStrategies(false);
    }
  }, []);

  useEffect(() => {
    if (isTauri()) {
      Promise.all([refresh(), refreshStrategies()]).catch((e) => setErr(String(e)));
    }
  }, [refresh, refreshStrategies]);

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
        const split = holdoutSplitIndex(nn, holdoutPct);
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
      await refreshStrategies();
      setSavedStrategyId(strategyId);
      setMsg(`已存檔：strategy #${strategyId}（type=${def.type}）· dataset #${selected.id} · ${result.trades.length} trades`);
    } catch (e) {
      setErr(String(e));
    } finally {
      setSaving(false);
    }
  }

  function loadSavedStrategy() {
    const def = savedStrategies.find((row) => row.id === savedStrategyId);
    if (!def) return;
    setErr(null);
    try {
      const loaded = strategyFromDef(def);
      setStrat(loaded);
      setStratName(def.name);
      setResult(null);
      setHoldoutResult(null);
      setSweepResetSignal((n) => n + 1); // clear the (now-stale) sweep heatmap in SweepSection
      setAppliedKeys([]);
      setMsg(`已載入策略：${def.name}（${def.type}）；請重新執行回測。`);
    } catch (e) {
      setErr(`無法載入「${def.name}」：${String(e)}`);
    }
  }

  // Load candles if the panel hasn't yet (mirrors run()'s empty-candles path),
  // so a sweep can run even before the chart-load effect has populated them.
  // Passed to SweepSection (REF-001).
  const ensureCandles = async (): Promise<CoreCandle[]> => {
    let cs = candles;
    if (!cs.length && selected && selected.id != null) {
      cs = toCoreCandles(await db.getCandles(selected.id, selected.start_time, selected.end_time));
      setCandles(cs);
    }
    return cs;
  };

  return (
    <div>
      {err && <div style={{ ...S.card, borderColor: '#d23b2f', color: '#b23b2e', marginBottom: 12 }}>{err}</div>}
      {msg && <div style={{ ...S.card, borderColor: '#2d9f73', color: '#1f7a57', marginBottom: 12 }}>{msg}</div>}

      <ChartSection
        candles={candles}
        strat={strat}
        result={result}
        selected={selected}
        loadingCandles={loadingCandles}
        appliedKeys={appliedKeys}
        onChangeParam={setNum}
        onError={setErr}
        onMessage={setMsg}
        helpReplayText={HELP.replay}
      />

      <div style={S.panel}>
        {/* left column: data + strategy */}
        <div style={{ display: 'grid', gap: 12 }}>
          <DatasetSection
            datasets={datasets}
            selId={selId}
            busyData={busyData}
            importText={importText}
            tauriAvailable={isTauri()}
            helpText={HELP.dataset}
            onSelectDataset={setSelId}
            onLoadSample={loadSample}
            onRefresh={() => refresh().catch((e) => setErr(String(e)))}
            onImportTextChange={setImportText}
            onImportJson={importJson}
          />

          <section style={S.card}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
              <h2 style={{ ...S.h2, margin: 0 }}>策略</h2>
              <HelpTip id="strategy" label="策略" text={HELP.strategy} />
              <div style={{ display: 'flex', gap: 2 }}>
                {(['params', 'blocks', 'code'] as const).map((mode) => (
                  <button
                    key={mode}
                    data-testid={`strategy-mode-${mode}`}
                    aria-pressed={strat.mode === mode}
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

            <div style={{ display: 'flex', alignItems: 'flex-end', gap: 6, marginBottom: 10, paddingBottom: 10, borderBottom: '1px solid #efece5', flexWrap: 'wrap' }}>
              <label style={{ display: 'flex', flex: 1, minWidth: 160, flexDirection: 'column', gap: 3 }}>
                <span style={S.label}>目前策略名稱</span>
                <input
                  data-testid="strategy-name"
                  value={stratName}
                  onChange={(e) => setStratName(e.target.value)}
                  placeholder="策略名稱（可留空）"
                  style={S.input}
                />
              </label>
              <label style={{ display: 'flex', flex: 1, minWidth: 180, flexDirection: 'column', gap: 3 }}>
                <span style={S.label}>策略庫（SQLite）</span>
                <select
                  data-testid="strategy-library-select"
                  value={savedStrategyId ?? ''}
                  onChange={(e) => setSavedStrategyId(e.target.value ? Number(e.target.value) : null)}
                  style={S.input}
                  disabled={loadingStrategies}
                >
                  <option value="">{loadingStrategies ? '讀取中…' : savedStrategies.length === 0 ? '尚無已存策略' : '選擇已存策略'}</option>
                  {savedStrategies.filter((row) => row.id != null).map((row) => (
                    <option key={row.id} value={row.id} disabled={!['params', 'blocks', 'code'].includes(row.type)}>
                      {row.name} · {row.type}
                    </option>
                  ))}
                </select>
              </label>
              <button data-testid="load-strategy" style={S.btnGhost} onClick={loadSavedStrategy} disabled={savedStrategyId == null || loadingStrategies}>
                載入
              </button>
              <button
                data-testid="refresh-strategies"
                title="重新讀取策略庫"
                style={S.btnGhost}
                onClick={() => refreshStrategies().catch((e) => setErr(String(e)))}
                disabled={loadingStrategies}
                aria-busy={loadingStrategies}
              >
                重新整理
              </button>
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

        {/* right column: results (metrics table + export + save + metrics pop-out) */}
        <ResultsSection
          result={result}
          holdout={holdout}
          holdoutResult={holdoutResult}
          selected={selected}
          strat={strat}
          stratName={stratName}
          saving={saving}
          onSave={save}
          onError={setErr}
          onMessage={setMsg}
          help={{ metrics: HELP.metrics, save: HELP.save }}
        />
      </div>

      {candles.length > 0 && (
        <SweepSection
          strat={strat}
          interval={selected?.interval ?? ''}
          datasetSelected={selected != null && selected.id != null}
          holdout={holdout}
          holdoutPct={holdoutPct}
          ensureCandles={ensureCandles}
          onApplyCombo={(patch, keys, message) => {
            setStrat((s) => ({ ...s, ...patch }));
            setAppliedKeys(keys);
            setMsg(message);
          }}
          onClearApplied={() => setAppliedKeys([])}
          resetSignal={sweepResetSignal}
          help={{ sweep: HELP.sweep, runSweep: HELP.runSweep, applyBest: HELP.applyBest }}
        />
      )}

    </div>
  );
}
