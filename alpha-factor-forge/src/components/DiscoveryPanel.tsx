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
//   2. ALL event ordering lives in `services/discoveryFeed.ts`, never here. Its
//      applied sequence is forward-only and kept PER STATE SLICE — one for
//      status/counts, one for the result list — because progress is throttled and
//      results are not, so a single counter for all three channels would make a
//      coalesced progress tick that lands after a newer result look stale and
//      drop its counts. That single-counter design was tried, rejected in the
//      PR #102 review, and must not be reintroduced; the reducer also buffers
//      events that arrive before the run id is known, which is the only way the
//      results of a run that finishes before `start_discovery` returns survive.

import React, { useEffect, useMemo, useReducer, useRef, useState } from 'react';
import { discovery, discoveryEvents, runtime, runtimeEvents } from '../tauri-client/dataClient';
import type { HostMode, HostStatus } from '../tauri-client/commands';
import {
  createThrottle,
  type DiscoveryProgressEvent,
  type RunStatus,
} from '../tauri-client/events';
import { isCommandError } from '../services/researchCommand';
import {
  INITIAL_DISCOVERY_FEED,
  isTerminalStatus,
  reduceDiscoveryFeed,
} from '../services/discoveryFeed';
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

type PendingAction = 'start' | 'pause' | 'resume' | 'cancel' | 'refresh' | 'host';

/** P04b: what the host-mode badge says. */
const HOST_LABEL: Record<HostMode, string> = {
  'desktop-embedded': '桌面內建',
  'desktop-connect': '背景 service',
  switching: '切換中',
};

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
  // All ordering lives in the pure reducer (see services/discoveryFeed.ts). The
  // first version kept the run id and applied sequence in refs re-derived from
  // state each render, which the PR #102 review proved cannot be monotonic:
  // ordering that depends on render timing is not ordering. `dispatch` is stable,
  // so the subscription callbacks never capture stale values either.
  const [feed, dispatch] = useReducer(reduceDiscoveryFeed, INITIAL_DISCOVERY_FEED);
  const { run, results, stale } = feed;
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [err, setErr] = useState<string | null>(null);
  // P04b: which host the desktop is. `null` until the backend answers (and
  // forever outside Tauri), so nothing about hosts is shown where nothing can
  // be switched.
  const [host, setHost] = useState<HostStatus | null>(null);
  const [serviceReachable, setServiceReachable] = useState(true);
  // The run the panel is following, readable from subscription callbacks
  // (which are registered once and must not capture a stale `run`).
  const runIdRef = useRef<number | null>(null);
  runIdRef.current = run?.runId ?? null;

  const combinations = useMemo(() => {
    try {
      return axisValues(axis).length;
    } catch {
      // An invalid axis is reported by Start through the shared parser; the
      // preview simply has nothing to show until then.
      return null;
    }
  }, [axis]);

  // One subscription set for the panel's lifetime. Progress is throttled because
  // a run emits one per candidate; the throttle is cancelled on unmount so a
  // queued trailing call cannot fire into a dead component.
  useEffect(() => {
    const applyProgress = createThrottle(
      (event: DiscoveryProgressEvent) => dispatch({ type: 'progress', event }),
      PROGRESS_THROTTLE_MS,
    );
    let disposed = false;
    const unlisteners: (() => void)[] = [];

    const reportDropped = (): void => dispatch({ type: 'dropped' });

    void (async () => {
      // Registered one at a time, not through Promise.all: a rejection there
      // discards the unlisten functions of the subscriptions that DID resolve,
      // leaking them, and surfaces only as an unhandled rejection (PR #102
      // review). Here every success is recorded before the next attempt, so a
      // partial failure can still be cleaned up and reported.
      const register: (() => Promise<() => void>)[] = [
        () => discoveryEvents.onProgress((event) => applyProgress.call(event), reportDropped),
        () => discoveryEvents.onResult((event) => dispatch({ type: 'result', event }), reportDropped),
        () => discoveryEvents.onDone((event) => {
          // Terminal state is applied immediately: it must never sit behind a
          // throttle window. Cancelling the throttle drops whatever progress tick
          // was still queued — including, on a fast run, the FINAL counts — so the
          // authoritative numbers are then re-read from the database rather than
          // reconstructed from the events that happened to survive coalescing.
          applyProgress.cancel();
          dispatch({ type: 'done', event });
          void discovery.progress(event.runId)
            .then((snapshot) => dispatch({ type: 'snapshot', snapshot, adopt: false }))
            // The status from the event is already applied; only the counts stay
            // uncertain, which is exactly what the stale notice tells the user.
            .catch(() => dispatch({ type: 'dropped' }));
        }, reportDropped),
      ];
      try {
        for (const subscribe of register) {
          const unlisten = await subscribe();
          if (disposed) {
            unlisten();
            for (const previous of unlisteners) previous();
            unlisteners.length = 0;
            return;
          }
          unlisteners.push(unlisten);
        }
      } catch (error) {
        for (const unlisten of unlisteners) unlisten();
        unlisteners.length = 0;
        // A panel with no event feed is not broken, only blind: the database is
        // still readable, so say so instead of failing silently.
        setErr(`事件訂閱失敗：${error instanceof Error ? error.message : String(error)}`);
        dispatch({ type: 'dropped' });
      }
    })();

    return () => {
      disposed = true;
      applyProgress.cancel();
      for (const unlisten of unlisteners) unlisten();
    };
  }, []);

  // The database is the source of truth: adopt whatever run already exists before
  // trusting any event. Startup recovery turns an orphaned run into `paused`, and
  // this is how the panel rediscovers it after a reload.
  useEffect(() => {
    let cancelled = false;
    discovery.getActiveRun()
      .then((snapshot) => {
        if (cancelled || snapshot == null) return;
        dispatch({ type: 'snapshot', snapshot, adopt: true });
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

  // P04b: the host mode, and the reconnect it implies. A mode change (either
  // direction), a service that comes back, or the bridge saying the view may
  // be behind (`runtime://resnapshot`: a ledger gap, a version that moved
  // with no row, an undeliverable row) all mean the run may have moved while
  // this window was not following it, so the snapshot is re-read before any
  // further event is trusted (contract §3: snapshot, then cursor). The
  // active run is read first; when there is none — the followed run has
  // ENDED, which is exactly the case a missing Done row leaves behind — the
  // followed run's own snapshot is read instead, so a stale "running" never
  // outlives the database's "completed".
  useEffect(() => {
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const resnapshot = async (): Promise<void> => {
      const active = await discovery.getActiveRun();
      if (disposed) return;
      if (active != null) {
        dispatch({ type: 'snapshot', snapshot: active, adopt: true });
        return;
      }
      const followed = runIdRef.current;
      if (followed == null) return;
      const snapshot = await discovery.progress(followed);
      if (!disposed) dispatch({ type: 'snapshot', snapshot, adopt: false });
    };
    const resnapshotReporting = (): void => {
      resnapshot().catch((error) => {
        if (!disposed) setErr(String(error));
      });
    };
    runtime.hostStatus()
      .then((status) => {
        if (!disposed) setHost(status);
      })
      .catch(() => {
        // No host backend (plain Vite without ?mock=1): the badge stays hidden.
      });
    const keep = (subscription: Promise<() => void>): void => {
      subscription
        .then((stop) => {
          if (disposed) stop();
          else unlisteners.push(stop);
        })
        .catch(() => {
          // Same as above: nothing to subscribe to outside Tauri.
        });
    };
    keep(runtimeEvents.onHostChanged((event) => {
      if (disposed) return;
      setHost((previous) => (previous == null ? previous : { ...previous, hostMode: event.hostMode, detail: null }));
      setServiceReachable(event.serviceReachable);
      if (!event.serviceReachable) {
        setErr(`背景 service 失聯：${event.reason ?? '無回應'}（重新尋找端點中）`);
        return;
      }
      setErr(null);
      resnapshotReporting();
    }));
    keep(runtimeEvents.onResnapshotNeeded((event) => {
      if (disposed) return;
      onMessage(`背景 service 的事件可能有缺口（${event.reason}），已重新讀取進度`);
      resnapshotReporting();
    }));
    return () => {
      disposed = true;
      for (const stop of unlisteners) stop();
    };
    // Mount-time subscriptions; onMessage identity must not re-subscribe.
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
    // Stop following the previous run BEFORE the command is sent. The backend
    // emits its first progress event and spawns the coordinator before
    // `start_discovery` returns the run id, so a short run can emit results —
    // even finish — while this promise is still pending. Those events are
    // buffered by the reducer and drained when the snapshot below adopts the run;
    // a snapshot alone could not recover them, because no command returns result
    // history (PR #102 review).
    dispatch({ type: 'starting' });
    const runId = await discovery.start(envelope);
    const snapshot = await discovery.progress(runId);
    dispatch({ type: 'snapshot', snapshot, adopt: true });
    onMessage(`已啟動探索任務 #${runId}（seed ${rootSeed}）`);
  });

  const refresh = (): Promise<void> => act('refresh', async () => {
    if (run == null) return;
    const snapshot = await discovery.progress(run.runId);
    dispatch({ type: 'snapshot', snapshot, adopt: false });
  });

  const lifecycle = (action: 'pause' | 'resume' | 'cancel'): Promise<void> => act(action, async () => {
    if (run == null) return;
    if (action === 'pause') await discovery.pause(run.runId);
    if (action === 'resume') await discovery.resume(run.runId);
    if (action === 'cancel') await discovery.cancel(run.runId);
    // Re-read rather than assume: the backend decides whether the transition
    // took, and a drained pause can land after the command returns. The reducer
    // drops this read if events have already moved past it.
    dispatch({ type: 'snapshot', snapshot: await discovery.progress(run.runId), adopt: false });
  });

  // P04b: hand the workspace to the background service, or take it back. The
  // backend rejects with a `CommandError` whose message already names the
  // mode it fell back to; the status is re-read either way so the badge
  // never shows a mode the backend is not in.
  const switchHost = (): Promise<void> => act('host', async () => {
    if (host == null) return;
    const entering = host.hostMode === 'desktop-embedded';
    try {
      const status = entering ? await runtime.enterBackgroundMode() : await runtime.exitBackgroundMode();
      setHost(status);
      onMessage(entering
        ? '研究已交給背景 service；關閉視窗不會中斷進行中的探索'
        : '已停止背景 service，研究回到桌面內建模式');
    } catch (error) {
      runtime.hostStatus().then(setHost).catch(() => undefined);
      throw new Error(isCommandError(error) ? error.message : String(error));
    }
  });

  const active = run != null && !isTerminalStatus(run.status);
  const busy = pending != null;
  const canStart = liveContext != null && !active && !busy;
  const hostLost = host?.hostMode === 'desktop-connect' && !serviceReachable;
  const serviceMissing = host?.hostMode === 'desktop-embedded' && !host.serviceExecutablePresent;

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
        {host != null && (
          <span
            data-testid="host-mode"
            data-host-mode={host.hostMode}
            title={host.hostMode === 'desktop-connect'
              ? '研究在背景 service 程序中執行；關閉本視窗不會中斷'
              : host.hostMode === 'desktop-embedded'
                ? '研究在本視窗的程序中執行；關閉視窗會在 checkpoint 停下'
                : host.detail ?? undefined}
            style={{ fontSize: 11, color: hostLost ? t.color.danger : t.color.muted, marginLeft: 'auto' }}
          >
            宿主：{HOST_LABEL[host.hostMode]}{hostLost ? '（失聯）' : ''}
          </span>
        )}
        {host != null && host.hostMode !== 'switching' && (
          <button
            data-testid="host-mode-toggle"
            style={{ ...S.btnGhost, padding: '3px 10px' }}
            onClick={switchHost}
            disabled={busy || serviceMissing}
            aria-busy={pending === 'host'}
            title={serviceMissing ? `找不到 service 程式：${host.serviceExecutable}` : undefined}
          >
            {pending === 'host' ? '切換中…' : host.hostMode === 'desktop-connect' ? '收回桌面' : '在背景繼續'}
          </button>
        )}
        {host?.hostMode === 'switching' && host.detail != null && (
          <span data-testid="host-detail" style={{ fontSize: 11, color: t.color.danger }}>{host.detail}</span>
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
