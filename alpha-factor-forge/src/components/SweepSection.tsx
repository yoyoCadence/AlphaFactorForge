// Parameter-sweep section, extracted from BacktestPanel (REF-001, move-only).
//
// Owns its own sweep UI state (open/axes/metric/result/appliedCell) and handlers
// (run/clear/apply). The strategy itself, the applied-key highlighting, and the
// candle loading stay in BacktestPanel; this section reaches them through props:
//   - liveContext      the ONE description of the inputs a sweep would run on
//   - ensureCandles()  loads candles if needed (mirrors the panel's run() path)
//   - onApplyCombo()   pushes the picked params onto the strategy + status line
//   - onClearApplied() clears the panel's applied-key highlight
//
// BUG-SWEEP-CONTEXT-001: a completed grid is held as an immutable artifact bound
// to the inputs it optimised over (see services/sweepArtifact.ts), and both the
// heatmap and every apply path read a render-derived gate rather than the raw
// state — so a Holdout toggle or a non-axis strategy edit removes them in the
// same render that accepts the edit.

import React, { useEffect, useMemo, useRef, useState } from 'react';
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
import {
  createSweepArtifact,
  describeSweepContext,
  sameSweepContext,
  sweepResultIsWritable,
  type CompletedSweep,
} from '../services/sweepArtifact';
import type { RunContext } from '../services/runArtifact';
import type { Candle as CoreCandle } from '../core/backtest';
import { HelpTip } from './HelpTip';
import { NumberInput } from './NumberInput';
import { makeStyles } from './panelStyles';
import { useTheme } from '../theme/ThemeProvider';
import { heatColor, heatTextColor } from '../charts/chartPaint';

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


/** One sweep axis on a single wrap-safe row: param picker + min / max / step.
 *  Inline (not a column) so the optional 2-D Y row can't overlap neighbours. */
function AxisEditor({ title, axis, onChange }: { title: string; axis: SweepAxisConfig; onChange: (a: SweepAxisConfig) => void }): React.ReactElement {
  const S = makeStyles(useTheme());
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
  const t = useTheme();
  const { xs, ys, grid, best, lo, hi, metric, xKey, yKey } = result;
  const is2d = yKey != null;
  const span = hi - lo;
  const cell: React.CSSProperties = {
    padding: '4px 6px', textAlign: 'center', fontFamily: t.font.mono, fontSize: 11, minWidth: 48,
  };
  const head: React.CSSProperties = { ...cell, background: t.button.ghostBg, color: t.color.ink, border: `1px solid ${t.color.cardBg}` };
  return (
    <div style={{ marginTop: 12, overflowX: 'auto' }}>
      <div style={{ fontSize: 11, color: t.color.muted, marginBottom: 6 }}>
        {/* 色階已由紅→黃→綠改為 token 單色階（冷 surface2 → 熱 accent），
            「越綠越佳」在多數皮膚會是錯的敘述，故改為描述強度。 */}
        熱力圖 · 顏色越強烈越佳（{SWEEP_METRIC_LABEL[metric]}）；橫軸 {SWEEP_PARAM_LABEL[xKey]}
        {is2d ? `、縱軸 ${SWEEP_PARAM_LABEL[yKey]}` : ''}。每格為指標值，括號為交易次數。
        {/* 「藍框」原本描述寫死的 #2f6df0；accent 現在隨皮膚變色，故移除顏色字眼。 */}
        <b>點任一格</b>即套用該組合 · <span style={{ color: t.color.ink }}>★ 最佳</span> · <span style={{ color: t.color.accent }}>✓ 已套用</span>。
      </div>
      <table style={{ borderCollapse: 'collapse' }}>
        <thead>
          <tr>
            <th style={{ ...head, color: t.color.muted }}>{is2d ? `${SWEEP_PARAM_LABEL[yKey]} \\ ${SWEEP_PARAM_LABEL[xKey]}` : SWEEP_PARAM_LABEL[xKey]}</th>
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
                // 原名 `t`，因本元件已取用 useTheme() 的 `t` 而更名，語意不變。
                const ratio = c.metric == null ? 0 : span > 0 ? (c.metric - lo) / span : 1;
                // 色階改由 chartPaint 的 token 版本產生：冷格 chart.surface2、熱格 chart.accent，
                // 明暗皮膚共用同一組。空值格直接用冷端（= surface2），不再是寫死的灰。
                const bg = c.metric == null ? t.chart.surface2 : heatColor(t.chart, ratio);
                // The hot end of the ramp IS chart.accent, which in several skins
                // is as dark as `ink` — the cell text has to flip with the fill or
                // the best combos become unreadable.
                const fg = c.metric == null ? t.chart.label : heatTextColor(t.chart, ratio);
                return (
                  <td
                    key={ci}
                    data-testid={`sweep-cell-${c.x}${is2d ? `-${c.y}` : ''}`}
                    onClick={() => onPick(c.x, c.y)}
                    title={`點擊套用 ${SWEEP_PARAM_LABEL[xKey]}=${c.x}${is2d ? ` · ${SWEEP_PARAM_LABEL[yKey]}=${c.y}` : ''}`}
                    style={{
                      ...cell,
                      background: bg,
                      color: fg,
                      cursor: 'pointer',
                      // Both markers key off `fg`, which already flips with the
                      // fill: the hot end of the ramp IS the accent, so an
                      // ink/accent border would vanish on exactly the cells the
                      // user is looking for. ★ / ✓已套用 keep the two apart.
                      border: isBest ? `2px solid ${fg}` : `1px solid ${t.color.cardBg}`,
                      outline: isApplied ? `3px solid ${fg}` : 'none',
                      outlineOffset: '-3px',
                      fontWeight: isBest || isApplied ? 700 : 500,
                    }}
                  >
                    <div>{fmtSweepMetric(metric, c.metric)}</div>
                    <div style={{ fontSize: 9, color: fg, opacity: 0.8 }}>({c.trades})</div>
                    {(isApplied || isBest) && (
                      <div style={{ fontSize: 9, fontWeight: 700, lineHeight: 1.2 }}>
                        {isApplied && <span data-testid="sweep-applied-marker" style={{ color: fg }}>✓已套用</span>}
                        {isBest && <span data-testid="sweep-best-marker" style={{ color: fg, marginLeft: isApplied ? 3 : 0 }}>★</span>}
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

export interface SweepSectionProps {
  /** BUG-SWEEP-CONTEXT-001 — the single description of the inputs a sweep started
   *  right now would optimise over: dataset identity + interval, the live
   *  strategy, and the Holdout split. It replaces the previous
   *  strat/interval/datasetSelected/holdout/holdoutPct props, because four
   *  independent props gave a completed grid nothing to compare itself against
   *  and let any one of them be forgotten. Null while the panel has no usable
   *  dataset, which fails every sweep action closed. */
  liveContext: RunContext | null;
  /** Load candles if the panel hasn't yet (mirrors run()'s empty-candles path). */
  ensureCandles: () => Promise<CoreCandle[]>;
  /** Push a picked param combo onto the strategy + status line (panel-owned). */
  onApplyCombo: (patch: Partial<Record<SweepParamKey, number>>, keys: SweepParamKey[], message: string) => void;
  /** Clear the panel's applied-key highlight (on sweep clear / rerun). */
  onClearApplied: () => void;
  /** Bump to clear the shown sweep result + applied cell (e.g. a strategy was
   *  loaded from the library, so the old heatmap no longer matches the strategy). */
  resetSignal: number;
  help: { sweep: string; runSweep: string; applyBest: string };
}

export function SweepSection({
  liveContext,
  ensureCandles,
  onApplyCombo,
  onClearApplied,
  resetSignal,
  help,
}: SweepSectionProps): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const [sweepOpen, setSweepOpen] = useState(false);
  const [sweepX, setSweepX] = useState<SweepAxisConfig>({ key: 'fastMA', min: 5, max: 20, step: 1 });
  const [sweepY, setSweepY] = useState<SweepAxisConfig>({ key: 'slowMA', min: 20, max: 40, step: 2 });
  const [sweepUse2d, setSweepUse2d] = useState(false);
  const [sweepMetric, setSweepMetric] = useState<SweepMetricId>('net');
  // BUG-SWEEP-CONTEXT-001 — a finished sweep is kept as an immutable artifact
  // bound to the inputs it optimised over, never as a bare grid.
  const [completedSweep, setCompletedSweep] = useState<CompletedSweep | null>(null);
  const [sweepErr, setSweepErr] = useState<string | null>(null);
  const [appliedCell, setAppliedCell] = useState<{ x: number; y: number | null } | null>(null);

  // One monotonically increasing token plus an owner ref, the same shape the
  // panel uses for dataset loads / runs: a completing sweep may only write while
  // it is still the newest one. Refs (not state) because a callback that already
  // ran must read the CURRENT owner, not its render closure's copy.
  const generationRef = useRef(0);
  const ownerRef = useRef(0);
  const [sweepingGen, setSweepingGen] = useState<number | null>(null);
  const sweeping = sweepingGen != null;

  // Clear the shown result when the panel signals a strategy load (the heatmap
  // was computed for the previous strategy). Mirrors the old inline reset in
  // loadSavedStrategy. The initial run (signal 0) is a harmless no-op. This is a
  // hard clear on top of the context gate below, matching the panel's own
  // loadSavedStrategy, which likewise drops `completed` outright.
  useEffect(() => {
    setCompletedSweep(null);
    setAppliedCell(null);
  }, [resetSignal]);

  const sweepConfig: SweepConfig = useMemo(
    () => ({ x: sweepX, y: sweepUse2d ? sweepY : null, metric: sweepMetric }),
    [sweepX, sweepY, sweepUse2d, sweepMetric],
  );
  const sweepCombos = countSweepCombos(sweepConfig);
  const sweepDupKey = sweepUse2d && sweepX.key === sweepY.key;
  const sweepTooMany = sweepCombos > SWEEP_MAX_COMBOS;

  // The inputs a sweep started right now would optimise over. Null until the
  // panel has a dataset whose candles are loaded, so "loading" and "load failed"
  // fail closed for every sweep action.
  const liveSweepContext = useMemo(
    () => (liveContext == null ? null : describeSweepContext({ run: liveContext, config: sweepConfig })),
    [liveContext, sweepConfig],
  );
  const liveSweepContextRef = useRef(liveSweepContext);
  liveSweepContextRef.current = liveSweepContext;

  // The completed sweep ONLY while it still describes the live inputs. The
  // heatmap, 套用最佳 and the per-cell apply all read this — never
  // `completedSweep` — so an invalidating edit removes them in the same render
  // that accepts it. The swept axes are masked out of the comparison, so
  // applying a cell (which writes exactly those axes) keeps the grid valid.
  const sweep = completedSweep != null && sameSweepContext(completedSweep.context, liveSweepContext) ? completedSweep : null;
  const staleSweep = completedSweep != null && sweep == null;
  const sweepResult = sweep?.result ?? null;
  // The Holdout split the sweep panel describes, read from the same live context
  // the run uses instead of a separate pair of props.
  const liveHoldout = liveContext?.range.holdout ?? null;

  // Any sweep-config edit invalidates a shown result: the visible controls would
  // otherwise describe a different sweep than the heatmap / 套用最佳 still acts on.
  // The applied-cell highlight is tied to that result, so it clears too.
  const clearSweep = () => {
    setCompletedSweep(null);
    setSweepErr(null);
    setAppliedCell(null);
    onClearApplied();
  };

  async function runSweep() {
    // The sweep executes the context, so the grid can never disagree about which
    // strategy / dataset / bars produced it.
    const run = liveContext;
    const context = liveSweepContext;
    if (run == null || context == null) {
      setSweepErr('請先選擇資料集');
      return;
    }
    generationRef.current += 1;
    const generation = generationRef.current;
    ownerRef.current = generation;
    setSweepingGen(generation);
    setSweepErr(null);
    setCompletedSweep(null);
    setAppliedCell(null);
    onClearApplied();
    // Let "掃描中…" paint before the (synchronous, up-to-256-backtest) run.
    await new Promise((r) => setTimeout(r, 20));
    try {
      const cs = await ensureCandles();
      if (!cs.length) throw new Error('此資料集沒有 K 線');
      // BUG-001: when holdout is on, optimise on the IN-SAMPLE segment only so
      // the out-of-sample tail stays untouched for honest validation. The range
      // now comes from the recorded context, which derives it from the same
      // holdoutSplitIndex run() uses — so the bars the grid is labelled with are
      // the bars it actually traded.
      const result = runParamSweep({
        candles: cs,
        strat: run.strategy,
        interval: context.dataset.interval,
        sweep: context.config,
        from: context.range.from,
        to: context.range.to,
      });
      // Both halves of the guard: still the newest sweep, and the inputs it was
      // started for are still the live ones. A superseded sweep is discarded
      // silently — it must leave neither a grid nor an error behind.
      if (!sweepResultIsWritable({ started: context, live: liveSweepContextRef.current, generation, owner: ownerRef.current })) return;
      setCompletedSweep(createSweepArtifact({ context, result }));
    } catch (e) {
      if (ownerRef.current !== generation) return;
      setSweepErr(e instanceof Error ? e.message : String(e));
    } finally {
      setSweepingGen((cur) => (cur === generation ? null : cur));
    }
  }

  // Apply any sweep cell's param combo to the strategy + mark it as applied.
  function applySweepCombo(r: SweepResult, x: number, y: number | null) {
    setAppliedCell({ x, y });
    const patch: Partial<Record<SweepParamKey, number>> = { [r.xKey]: x };
    const keys: SweepParamKey[] = [r.xKey];
    if (r.yKey != null && y != null) {
      patch[r.yKey] = y;
      keys.push(r.yKey);
    }
    const label = `${SWEEP_PARAM_LABEL[r.xKey]}=${x}${r.yKey != null && y != null ? ` · ${SWEEP_PARAM_LABEL[r.yKey]}=${y}` : ''}`;
    onApplyCombo(patch, keys, `已套用：${label}（記得再用樣本外驗證）`);
  }

  function applySweepBest() {
    const r = sweepResult;
    if (!r || !r.best) return;
    applySweepCombo(r, r.best.x, r.best.y);
  }

  return (
    <section style={{ ...S.card, marginTop: 12 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: sweepOpen ? 10 : 0, flexWrap: 'wrap' }}>
        <h2 style={{ ...S.h2, margin: 0 }}>參數掃描（最佳化）</h2>
        <HelpTip id="sweep" label="參數掃描" text={help.sweep} />
        <button data-testid="sweep-toggle" style={{ ...S.btnGhost, padding: '3px 10px' }} onClick={() => setSweepOpen((o) => !o)}>
          {sweepOpen ? '收合' : '展開'}
        </button>
        <span style={{ fontSize: 10, color: t.color.muted }}>選 1–2 個參數掃範圍，找最佳值（上限 {SWEEP_MAX_COMBOS} 組）。</span>
      </div>

      {sweepOpen && (
        <>
          {/* BUG-001: when holdout is on, the sweep optimises on the in-sample
              segment only — say so up front so the heatmap isn't misread as
              full-period. */}
          {liveHoldout && (
            <div data-testid="sweep-scope" style={{ fontSize: 11, color: t.color.accent, marginBottom: 10 }}>
              掃描範圍：僅樣本內（前 {100 - liveHoldout.pct}%）；末 {liveHoldout.pct}% 樣本外保留驗證，不參與最佳化。
            </div>
          )}
          {/* controls bar: metric + 2-D toggle + live combo count (axes get
              their own full-width rows below, so nothing can overlap) */}
          <div style={{ display: 'flex', gap: 16, alignItems: 'flex-end', flexWrap: 'wrap' }}>
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
              <span style={S.label}>最佳化指標</span>
              <select data-testid="sweep-metric" value={sweepMetric} onChange={(e) => { clearSweep(); setSweepMetric(e.target.value as SweepMetricId); }} style={{ ...S.input, fontSize: 11, minWidth: 120 }}>
                {SWEEP_METRIC_IDS.map((m) => <option key={m} value={m}>{SWEEP_METRIC_LABEL[m]}</option>)}
              </select>
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: t.color.muted }}>
              <input type="checkbox" data-testid="sweep-2d" checked={sweepUse2d} onChange={(e) => { clearSweep(); setSweepUse2d(e.target.checked); }} />
              第二維 Y（二維熱力圖）
            </label>
            <span data-testid="sweep-combos" style={{ fontSize: 11, color: sweepTooMany || sweepDupKey ? t.color.danger : t.color.muted }}>
              {sweepDupKey ? 'X / Y 參數需不同' : `組合數 ${sweepCombos}${sweepTooMany ? `（超過上限 ${SWEEP_MAX_COMBOS}）` : ''}`}
            </span>
          </div>

          <AxisEditor title="X 參數" axis={sweepX} onChange={(a) => { clearSweep(); setSweepX(a); }} />
          {sweepUse2d && <AxisEditor title="Y 參數" axis={sweepY} onChange={(a) => { clearSweep(); setSweepY(a); }} />}

          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 12, flexWrap: 'wrap' }}>
            <button data-testid="run-sweep" style={S.btn} onClick={runSweep} disabled={sweeping || sweepTooMany || sweepDupKey} aria-busy={sweeping}>
              {sweeping ? '掃描中…' : '▶ 執行掃描'}
            </button>
            <HelpTip id="run-sweep" label="執行掃描" text={help.runSweep} />
            {sweepResult?.best && (
              <>
                <button data-testid="apply-best" style={S.btnGhost} onClick={applySweepBest}>套用最佳：{sweepBestLabel(sweepResult)}</button>
                <HelpTip id="apply-best" label="套用最佳" text={help.applyBest} />
              </>
            )}
            <span style={{ fontSize: 10, color: t.color.warn }}>注意：歷史最佳常為過度擬合，務必再用樣本外驗證。</span>
          </div>

          {sweepErr && <div style={{ fontSize: 12, color: t.color.danger, marginTop: 8 }}>{sweepErr}</div>}
          {/* BUG-SWEEP-CONTEXT-001: say the grid is gone and why, rather than
              leaving a heatmap on screen that no longer describes these inputs. */}
          {staleSweep && (
            <div data-testid="sweep-stale" style={{ fontSize: 12, color: t.color.warn, marginTop: 8 }}>
              資料集、Holdout 或非掃描參數已變更 — 先前的掃描結果已失效（不會被套用）。請重新執行掃描。
            </div>
          )}
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
  );
}
