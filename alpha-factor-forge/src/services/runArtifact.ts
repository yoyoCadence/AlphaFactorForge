// BUG-RESULT-CONTEXT-001 — the immutable completed-run artifact plus the single
// canonical definition of "these are the same run inputs".
//
// A finished backtest only means something together with the exact inputs that
// produced it. The panel used to hold a bare `BacktestResult`, so editing the
// strategy or switching dataset left the old metrics/trades on screen and let
// Save/Export persist them under the NEW inputs.
//
// What this module guarantees:
//   - ONE comparison point. Every "is this result still valid?" decision goes
//     through `runContextKey` / `sameRunContext`; React handlers never compare
//     strategies or datasets field by field, so no call site can forget one.
//   - DEEP, FROZEN snapshots. `createRunArtifact` clones what it is handed, so a
//     later edit to the live editor strategy can never reach a completed run —
//     the decoupling standard PERSIST-001 set for persisted evidence.
//   - ONE range definition. The traded range is derived from the same
//     `holdoutSplitIndex` the run itself uses, so the recorded range is the
//     range that actually ran.
//
// Pure: no React, DOM, IO, or persistence. The strategy NAME is deliberately not
// part of the context — it is a display label that cannot change a result or the
// durable strategy-v2 identity, so renaming must never force a re-run.

import { canonicalize } from '../core/hashing';
import type { BacktestResult } from '../core/backtest';
import { holdoutSplitIndex } from './holdout';
import type { ParamsStrategy } from './strategy';

/** Bump when the field set that defines "same run inputs" changes. */
export const RUN_CONTEXT_VERSION = 'run-context-v1';

/** The identity of the dataset a run consumed. Datasets are immutable once
 *  imported, so id + content hash pin the candle bytes; the rest is the derived
 *  metadata Save/Export must reproduce without re-reading the live selection. */
export interface RunDatasetSnapshot {
  id: number;
  hash: string;
  symbol: string;
  interval: string;
  startTime: number;
  endTime: number;
  /** Bars actually loaded for the run. */
  barCount: number;
}

/** The holdout split a run used: in-sample is [0, splitIndex - 1], out-of-sample
 *  is [splitIndex, to]. Null when the run was full-period only. */
export interface RunHoldoutSplit {
  pct: number;
  splitIndex: number;
}

/** Inclusive bar-index bounds the run traded, plus its holdout split. */
export interface RunRange {
  from: number;
  to: number;
  holdout: RunHoldoutSplit | null;
}

/** Everything that determines what a backtest produces. Two contexts with the
 *  same key describe the same deterministic run. */
export interface RunContext {
  dataset: RunDatasetSnapshot;
  strategy: ParamsStrategy;
  range: RunRange;
}

export interface RunHoldoutResult {
  inSample: BacktestResult;
  outSample: BacktestResult;
}

/** A backtest bound to the inputs that produced it. Frozen end to end. */
export interface CompletedRun {
  context: RunContext;
  /** Durable `strategy-v2` identity of `context.strategy`, computed by the same
   *  helper Save persists with, so the artifact can never disagree with its row. */
  strategyHash: string;
  result: BacktestResult;
  holdoutResult: RunHoldoutResult | null;
}

export interface DescribeRunContextInput {
  dataset: RunDatasetSnapshot;
  strategy: ParamsStrategy;
  holdout: boolean;
  holdoutPct: number;
}

export interface CreateRunArtifactInput {
  context: RunContext;
  strategyHash: string;
  result: BacktestResult;
  holdoutResult: RunHoldoutResult | null;
}

/** Stable key for "the candles currently loaded belong to THIS dataset".
 *  Readiness must always be stored with this key, never as a bare array. */
export function datasetCandleKey(id: number, hash: string): string {
  return `${id}#${hash}`;
}

/** The exact bar range a run traded. Shares `holdoutSplitIndex` with the runner
 *  and the sweep, so the recorded split can never drift from the executed one. */
export function describeRunRange(barCount: number, holdout: boolean, holdoutPct: number): RunRange {
  if (!Number.isSafeInteger(barCount) || barCount < 1) {
    throw new Error(`run range needs at least one bar, received ${barCount}`);
  }
  if (!holdout) return { from: 0, to: barCount - 1, holdout: null };
  if (!Number.isFinite(holdoutPct)) {
    throw new Error(`holdout percentage must be finite, received ${holdoutPct}`);
  }
  return {
    from: 0,
    to: barCount - 1,
    holdout: { pct: holdoutPct, splitIndex: holdoutSplitIndex(barCount, holdoutPct) },
  };
}

/** Build the immutable description of one set of run inputs. Used for BOTH the
 *  live editor state and the finished run, so the two are always comparable. */
export function describeRunContext(input: DescribeRunContextInput): RunContext {
  return freezeDeep({
    dataset: cloneDeep(input.dataset),
    strategy: cloneDeep(input.strategy),
    range: describeRunRange(input.dataset.barCount, input.holdout, input.holdoutPct),
  });
}

/** Canonical string identity of a run context. Object key order cannot change
 *  it (core/hashing sorts deeply), so it is safe to compare structurally
 *  different-but-equivalent snapshots. */
export function runContextKey(context: RunContext): string {
  return canonicalize({
    version: RUN_CONTEXT_VERSION,
    dataset: context.dataset,
    strategy: context.strategy,
    range: context.range,
  });
}

/** The ONE equality used to decide whether a completed run still describes the
 *  live inputs. A missing context is never equal to anything, so "not loaded
 *  yet" and "load failed" both fail closed. */
export function sameRunContext(a: RunContext | null | undefined, b: RunContext | null | undefined): boolean {
  if (a == null || b == null) return false;
  return runContextKey(a) === runContextKey(b);
}

/** Bind a finished backtest to its context. Every part is deep-cloned and
 *  frozen, so nothing the caller keeps editing can reach the artifact. */
export function createRunArtifact(input: CreateRunArtifactInput): CompletedRun {
  if (!input.strategyHash) throw new Error('a completed run requires a durable strategy identity');
  return freezeDeep({
    context: describeRunContext({
      dataset: input.context.dataset,
      strategy: input.context.strategy,
      holdout: input.context.range.holdout != null,
      holdoutPct: input.context.range.holdout?.pct ?? 0,
    }),
    strategyHash: input.strategyHash,
    result: cloneDeep(input.result),
    holdoutResult: input.holdoutResult == null ? null : cloneDeep(input.holdoutResult),
  });
}

/** structuredClone, not a JSON round-trip: metrics legitimately carry Infinity
 *  (e.g. profit factor with no losing trade) and JSON would turn it into null. */
function cloneDeep<T>(value: T): T {
  return structuredClone(value);
}

function freezeDeep<T>(value: T): T {
  if (value != null && typeof value === 'object' && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const inner of Object.values(value as Record<string, unknown>)) freezeDeep(inner);
  }
  return value;
}
