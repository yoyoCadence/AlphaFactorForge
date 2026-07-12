// Chart section, extracted from BacktestPanel (REF-002, move-only).
//
// Owns everything chart-related: the candlestick canvas + overlay toggles, bar
// replay (cursor / autoplay / speed), the Slice 9 hover "此根資訊" readout, the
// overlay-driving quick param row, the Slice 8a in-app pop-out, AND the Slice 8b
// native OS-window mirror (snapshot + cursor sync). The strategy, the backtest
// result, the applied-key highlight source, and the top-level error/status
// banners stay in BacktestPanel and arrive as props.
//
// This component is rendered UNCONDITIONALLY by the panel (the visible <section>
// gates itself on candles.length) so the native-window "ready" listener registers
// exactly when it did as inline panel code — regardless of whether candles are
// loaded yet. Behaviour is identical to the pre-extraction inline block; every
// data-testid is preserved.

import React, { useEffect, useMemo, useRef, useState } from 'react';
import { CandleChart, type OverlayToggles } from '../charts/CandleChart';
import { replayTick, positionAtTime } from '../charts/scale';
import { buildSignals } from '../services/strategySignals';
import { popoutWindows, type ChartWindowSnapshot } from '../tauri-client/windowBridge';
import type { ParamsStrategy } from '../services/strategy';
import type { BacktestResult, Candle as CoreCandle } from '../core/backtest';
import type { Dataset } from '../tauri-client/commands';
import { HelpTip } from './HelpTip';
import { FloatingPanel } from './FloatingPanel';
import { PoppedOutNote } from './PoppedOutNote';
import { NumberInput } from './NumberInput';
import { S } from './panelStyles';
import type { NumKey } from './panelTypes';

// The overlay-driving periods most often tweaked while reading the chart —
// surfaced as a quick row right under it. Same `strat` as the full panel form.
const QUICK_FIELDS: { key: NumKey; label: string }[] = [
  { key: 'fastMA', label: '快線 MA' },
  { key: 'slowMA', label: '慢線 MA' },
  { key: 'emaPeriod', label: 'EMA' },
  { key: 'rsiPeriod', label: 'RSI 週期' },
];

const OVERLAY_LABEL: Record<keyof OverlayToggles, string> = { ma: 'MA', ema: 'EMA', bb: 'BB', rsi: 'RSI', vol: '量', trades: '買賣' };

const POS_LABEL: Record<'LONG' | 'SHORT' | 'FLAT', string> = { LONG: '多', SHORT: '空', FLAT: '空手' };

export interface ChartSectionProps {
  candles: CoreCandle[];
  strat: ParamsStrategy;
  result: BacktestResult | null;
  selected: Dataset | null;
  loadingCandles: boolean;
  /** Keys the last sweep-apply set — drives the quick row's blue ✓ highlight. */
  appliedKeys: NumKey[];
  /** Edit a numeric strategy param (mirrors the panel form; drops the key from
   *  appliedKeys). Same handler the panel passes to its own fields. */
  onChangeParam: (key: NumKey, value: number) => void;
  /** The panel's setErr (stable). Native-window failures surface through it. */
  onError: (message: string | null) => void;
  /** The panel's setMsg (stable). */
  onMessage: (message: string | null) => void;
  helpReplayText: string;
}

export function ChartSection({
  candles,
  strat,
  result,
  selected,
  loadingCandles,
  appliedKeys,
  onChangeParam,
  onError,
  onMessage,
  helpReplayText,
}: ChartSectionProps): React.ReactElement {
  const [show, setShow] = useState<OverlayToggles>({ ma: true, ema: false, bb: false, rsi: true, vol: true, trades: true });
  // Bar replay (Slice 6-1): step a cursor through the loaded candles; the chart
  // clips to bars [.., cursor]. Cursor resets to the latest bar when candles change.
  const [replayOn, setReplayOn] = useState(false);
  const [replayCursor, setReplayCursor] = useState(0);
  const [replayPlaying, setReplayPlaying] = useState(false); // autoplay (Slice 6-2)
  const [replaySpeed, setReplaySpeed] = useState(1); // 1× / 2× / 4×
  // Pop-out (Slice 8a): enlarge the chart into a floating resizable panel.
  const [poppedChart, setPoppedChart] = useState(false);
  const [nativeChartOpened, setNativeChartOpened] = useState(false);
  const [openingNativeChart, setOpeningNativeChart] = useState(false);
  const [hoverBar, setHoverBar] = useState<number | null>(null); // chart hover (Slice 9)

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

  const chartWindowSnapshot: ChartWindowSnapshot = {
    datasetKey: selected?.dataset_hash ?? `none:${candles.length}`,
    title: selected ? `${selected.symbol} · ${selected.interval}` : '尚無資料集',
    candles,
    strat,
    show,
    trades: result?.trades ?? [],
    upto: replayOn ? replayCursor : undefined,
  };
  const chartWindowSnapshotRef = useRef(chartWindowSnapshot);
  chartWindowSnapshotRef.current = chartWindowSnapshot;

  // Child window registers its listeners first, then asks the main window for
  // the latest complete snapshot. This avoids an open-vs-listen race.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    popoutWindows.onChartReady(() => {
      void popoutWindows.publishChart(chartWindowSnapshotRef.current).catch((e) => !disposed && onError(String(e)));
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

  // Full snapshots are for substantive chart changes. Replay ticks use the
  // small cursor event below so autoplay never re-sends the full candle array.
  useEffect(() => {
    if (!nativeChartOpened) return;
    void popoutWindows.publishChart(chartWindowSnapshotRef.current).catch((e) => onError(String(e)));
  }, [nativeChartOpened, candles, strat, show, result?.trades, selected]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!nativeChartOpened) return;
    void popoutWindows.publishChartCursor({ upto: replayOn ? replayCursor : undefined }).catch((e) => onError(String(e)));
  }, [nativeChartOpened, replayOn, replayCursor]); // eslint-disable-line react-hooks/exhaustive-deps

  async function openNativeChartWindow() {
    setOpeningNativeChart(true);
    onError(null);
    try {
      await popoutWindows.openChart();
      setNativeChartOpened(true);
      // Existing windows are focused rather than recreated, so push immediately;
      // a newly created window also requests this state through the ready event.
      await popoutWindows.publishChart(chartWindowSnapshotRef.current);
      onMessage('已開啟原生圖表視窗；可拖曳到其他螢幕。');
    } catch (e) {
      onError(`無法開啟原生圖表視窗：${String(e)}`);
    } finally {
      setOpeningNativeChart(false);
    }
  }

  // Highlight styling for a param that the last sweep-apply set (blue accent).
  const isAppliedKey = (key: NumKey) => appliedKeys.includes(key);
  const appliedInputStyle = (key: NumKey, base: React.CSSProperties): React.CSSProperties =>
    isAppliedKey(key) ? { ...base, borderColor: '#2f6df0', background: '#eef4ff' } : base;
  const appliedLabelStyle = (key: NumKey): React.CSSProperties =>
    isAppliedKey(key) ? { ...S.label, color: '#2f6df0', fontWeight: 700 } : S.label;

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

  // Chart content, factored out so it can render inline OR (Slice 8a) enlarged
  // inside a FloatingPanel. Same state either way -> edits reflow live.
  const renderChart = (chartHeight: number) => (
    <CandleChart candles={candles} strat={strat} show={show} trades={result?.trades} upto={replayOn ? replayCursor : undefined} onHoverBar={setHoverBar} height={chartHeight} />
  );

  return (
    <>
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
            {popoutWindows.isAvailable() && (
              <button data-testid="native-popout-chart" title="另開可移到其他螢幕的原生視窗" style={{ ...S.btnGhost, padding: '3px 10px', marginLeft: 'auto' }} onClick={openNativeChartWindow} disabled={openingNativeChart} aria-busy={openingNativeChart}>
                {openingNativeChart ? '開啟中…' : '↗ 新視窗'}
              </button>
            )}
            <button data-testid="popout-chart" title="放大到獨立面板" style={{ ...S.btnGhost, padding: '3px 10px', marginLeft: popoutWindows.isAvailable() ? 0 : 'auto' }} onClick={() => setPoppedChart((v) => !v)}>
              {poppedChart ? '⤡ 收合' : '⤢ 放大'}
            </button>
          </div>
          {poppedChart ? <PoppedOutNote label="圖表" onClose={() => setPoppedChart(false)} /> : renderChart(360)}

          <div style={{ display: 'flex', gap: 8, marginTop: 10, flexWrap: 'wrap', alignItems: 'center', borderTop: '1px solid #efece5', paddingTop: 10 }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: '#8a8678' }}>
              <input type="checkbox" data-testid="replay-toggle" checked={replayOn} onChange={(e) => { setReplayOn(e.target.checked); if (!e.target.checked) setReplayPlaying(false); }} />
              回放模式
            </label>
            <HelpTip id="replay" label="回放" text={helpReplayText} />
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
                <NumberInput value={strat[f.key]} onChange={(n) => onChangeParam(f.key, n)} style={appliedInputStyle(f.key, { ...S.input, width: 88 })} />
              </label>
            ))}
            <span style={{ fontSize: 10, color: '#aaa599', alignSelf: 'center' }}>調整即時重畫；完整參數見下方策略表單（<span style={{ color: '#2f6df0' }}>✓ 藍框</span>＝由掃描套用）</span>
          </div>
        </section>
      )}

      {/* Slice 8a pop-out: non-modal floating panel rendering the same chart
          enlarged; the left-column controls stay usable while this is open. */}
      {poppedChart && candles.length > 0 && (
        <FloatingPanel title="圖表" testId="chart-popout" initial={{ x: 430, y: 70, w: 800, h: 540 }} onClose={() => setPoppedChart(false)}>
          {(s) => renderChart(s.h)}
        </FloatingPanel>
      )}
    </>
  );
}
