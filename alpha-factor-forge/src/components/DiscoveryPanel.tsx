// RUNNER-UI-001b-2 — the discovery runner panel.
//
// The backend runner has been merged since RUNNER-EXEC-001 with no way to reach
// it. This panel starts a run from the workspace's live dataset and strategy,
// adopts a run that already exists, and shows progress, results, and lifecycle
// controls. It owns no contract of its own:
//   - the envelope comes from `buildDiscoveryConfig` (b-1), which pre-validates
//     with the same admission parser the backend mirrors, so an invalid run fails
//     here and no run row is ever created;
//   - events arrive already parsed and version-checked (slice a), and an
//     unparseable payload is REPORTED, never rendered as `undefined`.
//
// Two rules shape everything below:
//   1. The DATABASE is the source of truth for progress. `getActiveRun()` on
//      mount is the primary read — startup recovery can have left a paused run —
//      and events are a fast path layered on top of that snapshot. A re-query
//      control exists precisely because the fast path can drop a payload.
//   2. ONE monotonic sequence guard covers all three channels. An event that is
//      not newer than what the panel holds is ignored, which is also what stops a
//      coalesced progress tick from overwriting the terminal status that `done`
//      already delivered.

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { discovery, discoveryEvents } from '../tauri-client/dataClient';
import type { DiscoveryProgressSnapshot } from '../tauri-client/commands';
import {
  TERMINAL_RUN_STATUSES,
  createThrottle,
  type DiscoveryProgressCounts,
  type DiscoveryResultEvent,
  type RunStatus,
} from '../tauri-client/events';
import {
  DISCOVERY_AXIS_KEYS,
  axisValues,
  type DiscoveryAxis,
  type DiscoveryAxisKey,
} from '../services/discoveryConfig';
import { buildDiscoveryConfig, randomRootSeed } from '../services/discoveryRunConfig';
import type { RunContext } from '../services/runArtifact';
import { HelpTip } from './HelpTip';
import { NumberInput } from './NumberInput';
import { makeStyles } from './panelStyles';
import { useTheme } from '../theme/ThemeProvider';

/** Progress updates are coalesced to this window; results are not, because each
 *  one is a distinct row rather than a replacement value. */
const PROGRESS_THROTTLE_MS = 300;
/** Rolling window of shown results. The full history is the Results Explorer's
 *  job (a separate Phase B task), not this panel's. */
const MAX_SHOWN_RESULTS = 20;

const STATUS_LABEL: Record<RunStatus, string> = {
  idle: '閒置',
  running: '執行中',
  paused: '已暫停',
  completed: '已完成',
  failed: '失敗',
  cancelled: '已取消',
};

const AXIS_LABEL: Record<DiscoveryAxisKey, string> = {
  fastMA: '快線MA', slowMA: '慢線MA', emaPeriod: 'EMA週期', rsiPeriod: 'RSI週期',
  rsiBuy: 'RSI買', rsiSell: 'RSI賣', macdFast: 'MACD快', macdSlow: 'MACD慢',
  macdSignal: 'MACD訊號', bbPeriod: '布林週期', bbMult: '布林倍數',
  slPct: '停損%', tpPct: '停利%',
};

/** What the panel knows about the run it is following. */
interface FollowedRun {
  runId: number;
  name: string;
  status: RunStatus;
  counts: DiscoveryProgressCounts;
  bestStrategyId: number | null;
  errorMessage: string | null;
  /** Highest event sequence already applied; the single ordering guard. */
  lastSequence: number;
}

type PendingAction = 'start' | 'pause' | 'resume' | 'cancel' | 'refresh';

function isTerminal(status: RunStatus): boolean {
  return TERMINAL_RUN_STATUSES.includes(status);
}

function fromSnapshot(snapshot: DiscoveryProgressSnapshot): FollowedRun {
  return {
    runId: snapshot.runId,
    name: snapshot.name,
    status: snapshot.status,
    counts: snapshot.counts,
    bestStrategyId: snapshot.bestStrategyId,
    errorMessage: snapshot.errorMessage,
    lastSequence: snapshot.lastEventSequence,
  };
}

export interface DiscoveryPanelProps {
  /** The workspace's live inputs; null until a dataset's candles are loaded, which
   *  is what keeps Start disabled while there is nothing to run on. */
  liveContext: RunContext | null;
  onMessage: (message: string) => void;
}

export function DiscoveryPanel({ liveContext, onMessage }: DiscoveryPanelProps): React.ReactElement {
  const t = useTheme();
  const S = makeStyles(t);
  const [open, setOpen] = useState(false);
  const [axis, setAxis] = useState<DiscoveryAxis>({ key: 'fastMA', min: 5, max: 11, step: 3 });
  const [holdingAllowanceBars, setHoldingAllowanceBars] = useState(0);
  const [rootSeed, setRootSeed] = useState(() => randomRootSeed());
  const [run, setRun] = useState<FollowedRun | null>(null);
  const [results, setResults] = useState<DiscoveryResultEvent[]>([]);
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [err, setErr] = useState<string | null>(null);
  /** True once a payload was dropped: the view may be behind the database. */
  const [stale, setStale] = useState(false);

  // Refs, not state: the subscription callbacks are created once and must read
  // the CURRENT run/sequence, not the values captured in their first render.
  const runIdRef = useRef<number | null>(null);
  const sequenceRef = useRef(0);
  // The name only ever comes from a snapshot, so events keep whatever the last
  // snapshot reported instead of inventing a label.
  const runNameRef = useRef('');
  runIdRef.current = run?.runId ?? null;
  sequenceRef.current = run?.lastSequence ?? 0;
  runNameRef.current = run?.name ?? '';

  const combinations = useMemo(() => {
    try {
      return axisValues(axis).length;
    } catch {
      // An invalid axis is reported by Start through the shared parser; the
      // preview simply has nothing to show until then.
      return null;
    }
  }, [axis]);

  /** Accept an event only if it belongs to the followed run and is newer than
   *  everything already applied. */
  const accepts = useCallback((runId: number, sequence: number): boolean => {
    if (runIdRef.current !== runId) return false;
    if (sequence <= sequenceRef.current) return false;
    sequenceRef.current = sequence;
    return true;
  }, []);

  // One subscription set for the panel's lifetime. Progress is throttled because
  // a run emits one per candidate; the throttle is cancelled on unmount so a
  // queued trailing call cannot fire into a dead component.
  useEffect(() => {
    const applyProgress = createThrottle((next: FollowedRun) => setRun(next), PROGRESS_THROTTLE_MS);
    let disposed = false;
    const unlisteners: (() => void)[] = [];

    const reportDropped = (): void => setStale(true);

    void (async () => {
      const subscriptions = await Promise.all([
        discoveryEvents.onProgress((event) => {
          if (!accepts(event.runId, event.sequence)) return;
          applyProgress.call({
            runId: event.runId,
            name: runNameRef.current,
            status: event.status,
            counts: event.counts,
            bestStrategyId: event.bestStrategyId,
            errorMessage: null,
            lastSequence: event.sequence,
          });
        }, reportDropped),
        discoveryEvents.onResult((event) => {
          if (!accepts(event.runId, event.sequence)) return;
          setResults((prev) => [event, ...prev].slice(0, MAX_SHOWN_RESULTS));
        }, reportDropped),
        discoveryEvents.onDone((event) => {
          if (!accepts(event.runId, event.sequence)) return;
          // Terminal state is applied immediately: it must never sit behind a
          // throttle window. Cancelling the throttle drops whatever progress tick
          // was still queued — including, on a fast run, the FINAL counts — so the
          // authoritative numbers are then re-read from the database rather than
          // reconstructed from the events that happened to survive coalescing.
          // This is the panel's first rule doing real work, not a formality.
          applyProgress.cancel();
          setRun((prev) => prev == null ? prev : {
            ...prev,
            status: event.status,
            bestStrategyId: event.bestStrategyId ?? prev.bestStrategyId,
            errorMessage: event.errorMessage,
            lastSequence: event.sequence,
          });
          void discovery.progress(event.runId)
            .then((snapshot) => setRun(fromSnapshot(snapshot)))
            // The status from the event is already applied; only the counts stay
            // uncertain, which is exactly what the stale notice tells the user.
            .catch(() => setStale(true));
        }, reportDropped),
      ]);
      if (disposed) {
        for (const unlisten of subscriptions) unlisten();
        return;
      }
      unlisteners.push(...subscriptions);
    })();

    return () => {
      disposed = true;
      applyProgress.cancel();
      for (const unlisten of unlisteners) unlisten();
    };
  }, [accepts]);

  // The database is the source of truth: adopt whatever run already exists before
  // trusting any event. Startup recovery turns an orphaned run into `paused`, and
  // this is how the panel rediscovers it after a reload.
  useEffect(() => {
    let cancelled = false;
    discovery.getActiveRun()
      .then((snapshot) => {
        if (cancelled || snapshot == null) return;
        setRun(fromSnapshot(snapshot));
        onMessage(`已接續既有的探索任務：${snapshot.name}（${STATUS_LABEL[snapshot.status]}）`);
      })
      .catch((error) => {
        if (!cancelled) setErr(String(error));
      });
    return () => {
      cancelled = true;
    };
    // Adoption is a mount-time read; onMessage identity must not re-trigger it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function act(action: PendingAction, work: () => Promise<void>): Promise<void> {
    setPending(action);
    setErr(null);
    try {
      await work();
    } catch (error) {
      setErr(error instanceof Error ? error.message : String(error));
    } finally {
      setPending(null);
    }
  }

  const start = (): Promise<void> => act('start', async () => {
    if (liveContext == null) throw new Error('請先選擇資料集');
    // Built and validated before any command is called (b-1), so a bad axis,
    // seed, or strategy fails here with the admission parser's own message.
    const { envelope } = buildDiscoveryConfig({
      dataset: { id: liveContext.dataset.id, hash: liveContext.dataset.hash },
      strategy: liveContext.strategy,
      options: { axes: [axis], holdingAllowanceBars, rootSeed },
      logicalCores: logicalCores(),
    });
    const runId = await discovery.start(envelope);
    setResults([]);
    setStale(false);
    sequenceRef.current = 0;
    const snapshot = await discovery.progress(runId);
    setRun(fromSnapshot(snapshot));
    onMessage(`已啟動探索任務 #${runId}（seed ${rootSeed}）`);
  });

  const refresh = (): Promise<void> => act('refresh', async () => {
    if (run == null) return;
    const snapshot = await discovery.progress(run.runId);
    setRun(fromSnapshot(snapshot));
    setStale(false);
  });

  const lifecycle = (action: 'pause' | 'resume' | 'cancel'): Promise<void> => act(action, async () => {
    if (run == null) return;
    if (action === 'pause') await discovery.pause(run.runId);
    if (action === 'resume') await discovery.resume(run.runId);
    if (action === 'cancel') await discovery.cancel(run.runId);
    // Re-read rather than assume: the backend decides whether the transition
    // took, and a drained pause can land after the command returns.
    setRun(fromSnapshot(await discovery.progress(run.runId)));
  });

  const active = run != null && !isTerminal(run.status);
  const busy = pending != null;
  const canStart = liveContext != null && !active && !busy;

  return (
    <section style={{ ...S.card, marginTop: 12 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: open ? 10 : 0, flexWrap: 'wrap' }}>
        <h2 style={{ ...S.h2, margin: 0 }}>策略探索（Discovery）</h2>
        <HelpTip
          id="discovery"
          label="策略探索"
          text="以目前策略為基礎，在後端逐一驗證一個參數軸上的候選策略：每個候選都跑 Train／Validation 分段、基準比較與 Gate／Score，結果寫入資料庫。Test 段永不執行。"
        />
        <button data-testid="discovery-toggle" style={{ ...S.btnGhost, padding: '3px 10px' }} onClick={() => setOpen((o) => !o)}>
          {open ? '收合' : '展開'}
        </button>
        {run != null && (
          <span data-testid="discovery-status" style={{ fontSize: 11, color: t.color.muted }}>
            {run.name} · {STATUS_LABEL[run.status]}
          </span>
        )}
      </div>

      {open && (
        <>
          <div style={{ display: 'flex', gap: 12, alignItems: 'flex-end', flexWrap: 'wrap' }}>
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3, minWidth: 150 }}>
              <span style={S.label}>掃描參數</span>
              <select
                data-testid="discovery-axis-key"
                value={axis.key}
                onChange={(e) => setAxis((a) => ({ ...a, key: e.target.value as DiscoveryAxisKey }))}
                style={{ ...S.input, fontSize: 11 }}
              >
                {DISCOVERY_AXIS_KEYS.map((key) => <option key={key} value={key}>{AXIS_LABEL[key]}</option>)}
              </select>
            </label>
            {([['min', '起'], ['max', '迄'], ['step', '間距']] as const).map(([field, label]) => (
              <label key={field} style={{ display: 'flex', flexDirection: 'column', gap: 3, width: 76 }}>
                <span style={S.label}>{label}</span>
                <NumberInput
                  value={axis[field]}
                  onChange={(n) => setAxis((a) => ({ ...a, [field]: n }))}
                  style={{ ...S.input, fontSize: 11 }}
                />
              </label>
            ))}
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3, width: 110 }}>
              <span style={S.label}>持倉寬限(根)</span>
              <NumberInput value={holdingAllowanceBars} min={0} step={1} onChange={setHoldingAllowanceBars} style={{ ...S.input, fontSize: 11 }} />
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', gap: 3, width: 150 }}>
              {/* Displayed, not hidden: the whole Random Entry distribution derives
                  from this number, so reproducing a run means re-entering it. */}
              <span style={S.label}>隨機種子</span>
              <NumberInput value={rootSeed} min={0} step={1} onChange={setRootSeed} style={{ ...S.input, fontSize: 11 }} />
            </label>
            <button data-testid="discovery-reseed" style={{ ...S.btnGhost, padding: '3px 10px' }} onClick={() => setRootSeed(randomRootSeed())}>
              重新產生
            </button>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 12, flexWrap: 'wrap' }}>
            <button data-testid="discovery-start" style={S.btn} onClick={start} disabled={!canStart} aria-busy={pending === 'start'}>
              {pending === 'start' ? '啟動中…' : '▶ 啟動探索'}
            </button>
            <button data-testid="discovery-pause" style={S.btnGhost} onClick={() => lifecycle('pause')} disabled={busy || run == null || run.status !== 'running'}>
              ⏸ 暫停
            </button>
            <button data-testid="discovery-resume" style={S.btnGhost} onClick={() => lifecycle('resume')} disabled={busy || run == null || run.status !== 'paused'}>
              ⏵ 繼續
            </button>
            <button data-testid="discovery-cancel" style={S.btnGhost} onClick={() => lifecycle('cancel')} disabled={busy || !active}>
              ✕ 取消
            </button>
            <button data-testid="discovery-refresh" style={S.btnGhost} onClick={refresh} disabled={busy || run == null}>
              重新查詢
            </button>
            <span data-testid="discovery-combos" style={{ fontSize: 11, color: t.color.muted }}>
              {combinations == null ? '參數範圍無效' : `候選 ${combinations} 組`}
            </span>
          </div>

          {liveContext == null && (
            <div style={{ fontSize: 11, color: t.color.muted, marginTop: 8 }}>
              請先選擇並載入資料集，探索才能取得資料集識別碼。
            </div>
          )}
          {err && <div data-testid="discovery-error" style={{ fontSize: 12, color: t.color.danger, marginTop: 8 }}>{err}</div>}
          {/* A dropped payload is surfaced, never swallowed: the database still
              holds the truth, so the user is told to re-query rather than left
              looking at a silently frozen view. */}
          {stale && (
            <div data-testid="discovery-stale" style={{ fontSize: 12, color: t.color.warn, marginTop: 8 }}>
              有一筆進度事件無法解析已被丟棄，畫面可能落後於資料庫。請按「重新查詢」。
            </div>
          )}

          {run != null && (
            <div data-testid="discovery-progress" style={{ marginTop: 12, fontSize: 12, fontFamily: t.font.mono }}>
              <div>
                狀態 <b>{STATUS_LABEL[run.status]}</b> · 完成 {run.counts.completedCandidates}/{run.counts.totalCandidates}
                {run.counts.failedCandidates > 0 && ` · 失敗 ${run.counts.failedCandidates}`}
                {run.counts.skippedCandidates > 0 && ` · 略過 ${run.counts.skippedCandidates}`}
              </div>
              {run.bestStrategyId != null && (
                <div data-testid="discovery-best">目前最佳策略 #{run.bestStrategyId}</div>
              )}
              {run.errorMessage != null && (
                <div data-testid="discovery-run-error" style={{ color: t.color.danger }}>{run.errorMessage}</div>
              )}
            </div>
          )}

          {results.length > 0 && (
            <table data-testid="discovery-results" style={{ marginTop: 12, borderCollapse: 'collapse', fontSize: 11, fontFamily: t.font.mono }}>
              <thead>
                <tr>
                  {['候選', '策略', 'Gate', 'Score'].map((head) => (
                    <th key={head} style={{ ...S.tableHead, padding: '3px 8px', textAlign: 'left' }}>{head}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {results.map((result) => (
                  <tr key={result.sequence} data-testid={`discovery-result-${result.candidateIndex}`}>
                    <td style={{ padding: '3px 8px' }}>#{result.candidateIndex}</td>
                    <td style={{ padding: '3px 8px' }}>#{result.strategyId}</td>
                    <td style={{ padding: '3px 8px', color: result.gatePassed ? t.color.accent : t.color.muted }}>
                      {result.gatePassed ? '通過' : '未通過'}
                    </td>
                    {/* Gate-failed candidates have no score by contract, not a zero. */}
                    <td style={{ padding: '3px 8px' }}>{result.score == null ? '—' : result.score.toFixed(4)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      )}
    </section>
  );
}

/** Used only for b-1's local validation; the backend resolves concurrency with
 *  its own core count, which is why the envelope never sends a number. */
function logicalCores(): number {
  const cores = typeof navigator !== 'undefined' ? navigator.hardwareConcurrency : undefined;
  return Number.isSafeInteger(cores) && (cores as number) > 0 ? (cores as number) : 4;
}
