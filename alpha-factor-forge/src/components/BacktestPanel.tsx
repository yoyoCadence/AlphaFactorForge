// Slice 2 — single-strategy (params mode) backtest panel.
//
// Vertical slice: pick/import a dataset (SQLite) -> edit params-mode strategy ->
// run via the Slice 1 service (core/* under the hood) -> show metrics -> save
// the result (strategy_def + backtest_summary + trades). No chart / sweep / replay /
// live / library yet — those are later slices. All persistence goes through
// tauri-client; all maths through core/* + src/services.

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { db, isTauri, importDataset } from '../tauri-client/dataClient';
import type { Candle, Dataset, StrategyDef } from '../tauri-client/commands';
import { defaultStrategy, type ParamsStrategy } from '../services/strategy';
import { runParamsBacktest } from '../services/backtestRunner';
import { SWEEP_MAX_COMBOS } from '../services/paramSweep';
import {
  createRunArtifact,
  datasetCandleKey,
  describeRunContext,
  sameRunContext,
  type CompletedRun,
  type RunContext,
} from '../services/runArtifact';
import { toCoreCandles } from '../services/candleAdapter';
import { makeSampleCandles } from '../services/sampleData';
import { buildStrategyDef } from '../services/strategyRecord';
import { strategyFromDef } from '../services/strategyLibrary';
import { metricsToBacktestSummary } from '../services/metricsMapper';
import { tradesToRows } from '../services/tradesMapper';
import { SweepSection } from './SweepSection';
import { ChartSection } from './ChartSection';
import { DatasetSection } from './DatasetSection';
import { ResultsSection } from './ResultsSection';
import { StrategySection } from './StrategySection';
import { makeStyles } from './panelStyles';
import { useTheme } from '../theme/ThemeProvider';
import type { NumKey } from './panelTypes';
import type { Candle as CoreCandle } from '../core/backtest';

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
  save: '把策略、回測摘要與交易明細寫入資料庫（strategy_def + backtest_summary + trades，segment=full）。',
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

/** Stable "no candles" identity: the derived candle list must keep one reference
 *  across renders, or every consumer's candle-keyed effect re-fires each time. */
const NO_CANDLES: CoreCandle[] = [];

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

export function BacktestPanel(): React.ReactElement {
  const S = makeStyles(useTheme());
  const [datasets, setDatasets] = useState<Dataset[]>([]);
  const [selId, setSelId] = useState<number | null>(null);
  const [strat, setStrat] = useState<ParamsStrategy>(defaultStrategy);
  const [stratName, setStratName] = useState('');
  const [savedStrategies, setSavedStrategies] = useState<StrategyDef[]>([]);
  const [savedStrategyId, setSavedStrategyId] = useState<number | null>(null);
  const [loadingStrategies, setLoadingStrategies] = useState(false);
  // BUG-RESULT-CONTEXT-001 — a finished backtest is kept as an immutable
  // artifact bound to the inputs that produced it, never as a bare result.
  const [completed, setCompleted] = useState<CompletedRun | null>(null);
  const [saving, setSaving] = useState(false);
  const [busyData, setBusyData] = useState(false);
  const [importText, setImportText] = useState('');
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  // Candle readiness is stored WITH the dataset identity it was loaded for, so
  // another dataset can never adopt a leftover array (repro B in the audit).
  const [loadedCandles, setLoadedCandles] = useState<{ key: string; rows: CoreCandle[] } | null>(null);
  const [holdout, setHoldout] = useState(false);
  const [holdoutPct, setHoldoutPct] = useState(30); // last N% of bars = out-of-sample
  // Which strategy params the last sweep-apply set — highlighted in the form +
  // chart quick row so the user sees what the heatmap selection changed. A param
  // drops out of the set the moment it is hand-edited (no longer "from sweep").
  // Owned here (the strategy form + chart quick row read it); SweepSection sets
  // it via onApplyCombo and clears it via onClearApplied.
  const [appliedKeys, setAppliedKeys] = useState<NumKey[]>([]);
  // Bumped to tell SweepSection to drop its shown result (e.g. on strategy load).
  const [sweepResetSignal, setSweepResetSignal] = useState(0);

  // BUG-RESULT-CONTEXT-001 — one monotonically increasing generation token for
  // dataset loads and backtest runs. Async work captures the token it started
  // with; the "owner" refs say which token may still write. A newer job of the
  // same kind, or a result-affecting edit, moves the owner and the older job's
  // write is dropped. Refs (not state) because a callback that already ran must
  // read the CURRENT owner, not the value captured in its render closure.
  const generationRef = useRef(0);
  const loadOwnerRef = useRef(0);
  const runOwnerRef = useRef(0);
  const [loadingGen, setLoadingGen] = useState<number | null>(null);
  const [runningGen, setRunningGen] = useState<number | null>(null);
  const loadingCandles = loadingGen != null;
  const running = runningGen != null;

  const nextGeneration = useCallback(() => {
    generationRef.current += 1;
    return generationRef.current;
  }, []);

  /** A result-affecting input changed. Any in-flight RUN is now computing for
   *  inputs the user has already replaced, so it loses the right to write.
   *  Candle loads are untouched: candles depend on the dataset, not on the
   *  strategy or holdout. Every such handler must call this BEFORE it accepts
   *  the new value, so no code path can edit around the guard. */
  const invalidateRun = useCallback(() => {
    runOwnerRef.current = nextGeneration();
  }, [nextGeneration]);

  /** The one strategy-mutation entry point: mode, signals, rules, code, periods,
   *  exec/risk fields, library load, and Sweep apply all route through it. */
  const changeStrategy = useCallback((update: React.SetStateAction<ParamsStrategy>) => {
    invalidateRun();
    setStrat(update);
  }, [invalidateRun]);

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

  const loadedCandlesRef = useRef(loadedCandles);
  loadedCandlesRef.current = loadedCandles;

  /** Select a dataset. The completed run is dropped in the same synchronous
   *  update as the selection, and candle readiness stops applying the moment
   *  the selected identity changes (the key check below), so there is no render
   *  in which the previous dataset's candles/result are usable under the new
   *  one. Claiming the LOAD slot is left to the effect, which is the only place
   *  that starts one — claiming it here would orphan the load it triggers. */
  const selectDataset = useCallback((id: number | null) => {
    runOwnerRef.current = nextGeneration();
    setSelId(id);
    setCompleted(null);
  }, [nextGeneration]);

  // Load candles for the selected dataset. Readiness is keyed by dataset
  // identity, so a response may only be written while it still owns the load
  // slot; every pass claims that slot, which is what orphans a request whose
  // dataset is no longer selected. Datasets are immutable once imported, so an
  // identity that is already loaded is never re-fetched.
  useEffect(() => {
    const ds = datasets.find((d) => d.id === selId) ?? null;
    if (!isTauri() || !ds || ds.id == null) return;
    const key = datasetCandleKey(ds.id, ds.dataset_hash);
    const gen = nextGeneration();
    loadOwnerRef.current = gen;
    if (loadedCandlesRef.current?.key === key) {
      setLoadingGen(null); // nothing is in flight FOR THIS dataset any more
      return;
    }
    // A different dataset is coming: any in-flight run is for the previous one.
    runOwnerRef.current = gen;
    setLoadingGen(gen);
    db.getCandles(ds.id, ds.start_time, ds.end_time)
      .then((cs) => {
        if (loadOwnerRef.current !== gen) return; // superseded: a newer selection owns the slot
        setLoadedCandles({ key, rows: toCoreCandles(cs) });
      })
      .catch((e) => {
        if (loadOwnerRef.current !== gen) return;
        // A rejected load leaves this dataset unready, so Run/Save/Export stay disabled.
        setErr(String(e));
      })
      .finally(() => setLoadingGen((cur) => (cur === gen ? null : cur)));
  }, [selId, datasets, nextGeneration]);

  const selected = datasets.find((d) => d.id === selId) ?? null;
  const datasetKey = selected && selected.id != null ? datasetCandleKey(selected.id, selected.dataset_hash) : null;
  // Candles are only visible once they are proven to belong to the SELECTED
  // dataset — a non-empty array is never enough on its own.
  const candles = datasetKey != null && loadedCandles?.key === datasetKey ? loadedCandles.rows : NO_CANDLES;

  // The inputs a run started right now would use. Null until a dataset's candles
  // are actually loaded, which is what makes "loading" and "load failed" fail
  // closed for every result action.
  const liveContext: RunContext | null = useMemo(() => {
    if (!selected || selected.id == null || candles.length === 0) return null;
    return describeRunContext({
      dataset: {
        id: selected.id,
        hash: selected.dataset_hash,
        symbol: selected.symbol,
        interval: selected.interval,
        startTime: selected.start_time,
        endTime: selected.end_time,
        barCount: candles.length,
      },
      strategy: strat,
      holdout,
      holdoutPct,
    });
  }, [selected, candles, strat, holdout, holdoutPct]);
  const liveContextRef = useRef(liveContext);
  liveContextRef.current = liveContext;

  // The completed run ONLY while it still describes the live inputs. Everything
  // that renders, saves, or exports a result reads this — never `completed` —
  // so an edit invalidates those actions in the same render that accepts it.
  const artifact = completed != null && sameRunContext(completed.context, liveContext) ? completed : null;
  const staleResult = completed != null && artifact == null;

  // Set a numeric strategy param + drop it from the sweep applied-highlight (it's
  // now a hand edit). Passed to ChartSection (quick row) and StrategySection.
  const setNum = (key: NumKey, value: number) => {
    changeStrategy((s) => ({ ...s, [key]: value }));
    setAppliedKeys((ks) => (ks.includes(key) ? ks.filter((k) => k !== key) : ks));
  };

  async function loadSample() {
    setBusyData(true);
    setErr(null);
    setMsg(null);
    try {
      const candles = makeSampleCandles({ count: 600 });
      const id = await importDataset({ exchange: 'sample', symbol: 'SAMPLE', interval: '1h', source: 'sample', candles });
      await refresh();
      selectDataset(id);
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
      selectDataset(id);
      setImportText('');
      setMsg(`已匯入 ${candles.length} 根 K 線（${symbol} · ${interval}）`);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusyData(false);
    }
  }

  async function run() {
    // The run executes the context, so nothing downstream can disagree about
    // which strategy / dataset / range produced the result.
    const context = liveContext;
    if (!context) {
      setErr(selected == null ? '請先選擇資料集' : '此資料集沒有 K 線');
      return;
    }
    const gen = nextGeneration();
    runOwnerRef.current = gen;
    setRunningGen(gen);
    setErr(null);
    setMsg(null);
    setCompleted(null);
    try {
      const { strategy, dataset, range } = context;
      const interval = dataset.interval;
      const result = runParamsBacktest({ candles, strat: strategy, interval });
      const holdoutResult = range.holdout
        // Same candles (so indicators keep full history); from/to restrict which
        // bars are traded -> proper in-sample vs out-of-sample split.
        ? {
            inSample: runParamsBacktest({ candles, strat: strategy, interval, from: range.from, to: range.holdout.splitIndex - 1 }),
            outSample: runParamsBacktest({ candles, strat: strategy, interval, from: range.holdout.splitIndex, to: range.to }),
          }
        : null;
      // Durable identity comes from the same helper Save persists with, so an
      // artifact can never carry an identity that disagrees with its stored row.
      const strategyHash = (await buildStrategyDef(strategy, stratName)).strategy_hash;
      // Both halves of the guard: still the newest run, and the inputs it was
      // started for are still the live ones.
      if (runOwnerRef.current !== gen || !sameRunContext(context, liveContextRef.current)) return;
      setCompleted(createRunArtifact({ context, strategyHash, result, holdoutResult }));
    } catch (e) {
      if (runOwnerRef.current !== gen) return;
      setErr(String(e));
    } finally {
      setRunningGen((cur) => (cur === gen ? null : cur));
    }
  }

  async function save() {
    if (!artifact) return;
    setSaving(true);
    setErr(null);
    try {
      const { dataset, strategy } = artifact.context;
      // The strategy NAME is the only live editor value Save reads: it is a
      // display label that changes neither the result nor the strategy-v2
      // identity, which the equality check below proves before any write.
      const def = await buildStrategyDef(strategy, stratName);
      if (def.strategy_hash !== artifact.strategyHash) {
        throw new Error('策略識別碼與此回測結果不符，請重新執行回測後再儲存');
      }
      const strategyId = await db.saveStrategy(def);
      const summary = metricsToBacktestSummary(artifact.result.metrics, {
        strategyId,
        datasetId: dataset.id,
        segment: 'full',
        startTime: dataset.startTime,
        endTime: dataset.endTime,
      });
      await db.saveBacktestResult(summary, tradesToRows(artifact.result.trades));
      await refreshStrategies();
      setSavedStrategyId(strategyId);
      setMsg(`已存檔：strategy #${strategyId}（type=${def.type}）· dataset #${dataset.id} · ${artifact.result.trades.length} trades`);
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
      changeStrategy(loaded);
      setStratName(def.name);
      setCompleted(null);
      setSweepResetSignal((n) => n + 1); // clear the (now-stale) sweep heatmap in SweepSection
      setAppliedKeys([]);
      setMsg(`已載入策略：${def.name}（${def.type}）；請重新執行回測。`);
    } catch (e) {
      setErr(`無法載入「${def.name}」：${String(e)}`);
    }
  }

  // Candles proven to belong to the SELECTED dataset. This used to fetch and
  // adopt any non-empty array, which let a sweep run on the previous dataset's
  // candles; the section is only mounted once these are loaded, so the sweep now
  // reads exactly what the panel reads. Passed to SweepSection (REF-001).
  const ensureCandles = async (): Promise<CoreCandle[]> => candles;

  return (
    <div>
      {err && <div style={S.banner('error')}>{err}</div>}
      {msg && <div style={S.banner('ok')}>{msg}</div>}

      <ChartSection
        candles={candles}
        strat={strat}
        result={artifact?.result ?? null}
        selected={selected}
        loadingCandles={loadingCandles}
        appliedKeys={appliedKeys}
        onChangeParam={setNum}
        onError={setErr}
        onMessage={setMsg}
        helpReplayText={HELP.replay}
      />

      <div className="backtest-layout" data-testid="backtest-layout" style={S.panel}>
        {/* left column: data + strategy */}
        <div style={{ display: 'grid', gap: 12, minWidth: 0 }}>
          <DatasetSection
            datasets={datasets}
            selId={selId}
            busyData={busyData}
            importText={importText}
            tauriAvailable={isTauri()}
            helpText={HELP.dataset}
            onSelectDataset={selectDataset}
            onLoadSample={loadSample}
            onRefresh={() => refresh().catch((e) => setErr(String(e)))}
            onImportTextChange={setImportText}
            onImportJson={importJson}
          />

          <StrategySection
            strat={strat}
            onStratChange={changeStrategy}
            stratName={stratName}
            onStratNameChange={setStratName}
            savedStrategies={savedStrategies}
            savedStrategyId={savedStrategyId}
            loadingStrategies={loadingStrategies}
            onSelectSaved={setSavedStrategyId}
            onLoadStrategy={loadSavedStrategy}
            onRefreshStrategies={() => refreshStrategies().catch((e) => setErr(String(e)))}
            appliedKeys={appliedKeys}
            onChangeParam={setNum}
            holdout={holdout}
            onHoldoutToggle={(checked) => {
              invalidateRun(); // the split is part of the run context
              setHoldout(checked);
            }}
            holdoutPct={holdoutPct}
            onHoldoutPctChange={(n) => {
              invalidateRun(); // a stale split no longer matches the new %
              setHoldoutPct(n);
            }}
            running={running}
            canRun={liveContext != null && !loadingCandles}
            onRun={run}
            help={{ strategy: HELP.strategy, exec: HELP.exec, holdout: HELP.holdout, run: HELP.run }}
          />
        </div>

        {/* right column: results (metrics table + export + save + metrics pop-out) */}
        <ResultsSection
          artifact={artifact}
          stale={staleResult}
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
            // Apply Best / any heatmap cell is a strategy edit like the rest
            // (audit repro C): it must invalidate the previous completed run.
            changeStrategy((s) => ({ ...s, ...patch }));
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
