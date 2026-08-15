// BUG-SWEEP-CONTEXT-001 — the immutable completed-sweep artifact plus the single
// canonical definition of "these are the same sweep inputs".
//
// A parameter sweep only means something together with the bars it optimised
// over. The section used to hold a bare `SweepResult` that was cleared on a
// sweep-config edit and on a library load, and on nothing else — so a grid
// computed with Holdout OFF stayed on screen (and stayed appliable) after
// Holdout was switched ON, which quietly tunes the strategy on the very bars
// the out-of-sample segment exists to protect.
//
// What this module guarantees:
//   - ONE comparison point. Every "is this sweep still valid?" decision goes
//     through `sweepContextKey` / `sameSweepContext`; the section never compares
//     datasets, strategies, or Holdout fields field by field.
//   - ONE range definition. The optimised range is derived from the run range
//     the panel already computes, so it shares `holdoutSplitIndex` with the
//     interactive backtest and with the sweep's own `from`/`to`.
//   - MASKED axes. The swept parameters are removed from the compared basis, so
//     applying a cell — which by definition writes exactly those parameters —
//     cannot invalidate the grid it came from, while an edit to ANY other
//     strategy field does.
//   - DEEP, FROZEN snapshots, so nothing the caller keeps editing can reach a
//     finished sweep.
//
// Pure: no React, DOM, IO, or persistence. Sweep artifacts are never written to
// SQLite — they are provenance for an in-session UI action, not stored evidence.

import { canonicalize } from '../core/hashing';
import type {
  SweepConfig,
  SweepParamKey,
  SweepResult,
} from './paramSweep';
import type {
  RunContext,
  RunHoldoutSplit,
  RunRange,
  RunDatasetSnapshot,
} from './runArtifact';
import type { ParamsStrategy } from './strategy';

/** Bump when the field set that defines "same sweep inputs" changes. */
export const SWEEP_CONTEXT_VERSION = 'sweep-context-v1';

/** Inclusive bar-index bounds a sweep OPTIMISED over. With Holdout on this is
 *  the in-sample segment only; `holdout` still records the split it came from,
 *  so a percentage change invalidates even when the boundary happens to clamp
 *  to the same index. */
export interface SweepRange {
  from: number;
  to: number;
  holdout: RunHoldoutSplit | null;
}

/** The base strategy a sweep varied its axes around. `fixed` holds every field
 *  the sweep held CONSTANT; the swept axis keys are removed from it and listed
 *  in `swept` instead. That masking is an equality decision only — the sweep
 *  itself still runs against the full live strategy. */
export interface SweepStrategyBasis {
  fixed: Record<string, unknown>;
  /** Sorted, de-duplicated axis keys. */
  swept: SweepParamKey[];
}

/** Everything that determines what a sweep grid contains. Two contexts with the
 *  same key describe the same deterministic sweep. */
export interface SweepContext {
  dataset: RunDatasetSnapshot;
  basis: SweepStrategyBasis;
  config: SweepConfig;
  range: SweepRange;
}

/** A sweep grid bound to the inputs that produced it. Frozen end to end. */
export interface CompletedSweep {
  context: SweepContext;
  result: SweepResult;
}

export interface DescribeSweepContextInput {
  /** The panel's live run context: dataset identity, strategy, and run range. */
  run: RunContext;
  config: SweepConfig;
}

export interface CreateSweepArtifactInput {
  context: SweepContext;
  result: SweepResult;
}

/** The two halves a late sweep completion must satisfy before it may be
 *  written: it must still own the sweep slot, and the inputs it started for
 *  must still be the live ones. */
export interface SweepWriteGuard {
  /** Context the sweep started for. */
  started: SweepContext;
  /** Context the UI describes right now; null = no usable inputs. */
  live: SweepContext | null;
  /** Generation this sweep claimed when it started. */
  generation: number;
  /** Generation that currently owns the sweep slot. */
  owner: number;
}

/** The bars a sweep optimises over, derived from the run range so the BUG-001
 *  in-sample boundary stays single-sourced: full period, or `[from,
 *  splitIndex - 1]` when Holdout is on. */
export function sweepRangeFromRunRange(range: RunRange): SweepRange {
  if (range.holdout == null) return { from: range.from, to: range.to, holdout: null };
  return {
    from: range.from,
    to: range.holdout.splitIndex - 1,
    holdout: { pct: range.holdout.pct, splitIndex: range.holdout.splitIndex },
  };
}

/** Field-by-field copy with an explicit `null` second axis. `canonicalize` is
 *  JSON-based, so an absent `y` and `y: null` would otherwise key differently
 *  for the same 1-D sweep. */
export function normalizeSweepConfig(config: SweepConfig): SweepConfig {
  return {
    x: { key: config.x.key, min: config.x.min, max: config.x.max, step: config.x.step },
    y: config.y == null
      ? null
      : { key: config.y.key, min: config.y.min, max: config.y.max, step: config.y.step },
    metric: config.metric,
  };
}

/** The strategy params this sweep varies — the ones a cell click is allowed to
 *  change without invalidating the grid. */
export function sweptParamKeys(config: SweepConfig): SweepParamKey[] {
  const keys = new Set<SweepParamKey>([config.x.key]);
  if (config.y != null) keys.add(config.y.key);
  return [...keys].sort();
}

/** Split the live strategy into "held constant" and "swept". */
export function describeSweepBasis(strategy: ParamsStrategy, config: SweepConfig): SweepStrategyBasis {
  const swept = sweptParamKeys(config);
  const fixed = cloneDeep(strategy) as unknown as Record<string, unknown>;
  for (const key of swept) delete fixed[key];
  return { fixed, swept };
}

/** Build the immutable description of one set of sweep inputs. Used for BOTH
 *  the live editor state and the finished sweep, so the two are comparable. */
export function describeSweepContext(input: DescribeSweepContextInput): SweepContext {
  const config = normalizeSweepConfig(input.config);
  return freezeDeep({
    dataset: cloneDeep(input.run.dataset),
    basis: describeSweepBasis(input.run.strategy, config),
    config,
    range: sweepRangeFromRunRange(input.run.range),
  });
}

/** Canonical string identity of a sweep context. Object key order cannot change
 *  it (core/hashing sorts deeply). */
export function sweepContextKey(context: SweepContext): string {
  return canonicalize({
    version: SWEEP_CONTEXT_VERSION,
    dataset: context.dataset,
    basis: context.basis,
    config: context.config,
    range: context.range,
  });
}

/** The ONE equality used to decide whether a completed sweep still describes
 *  the live inputs. A missing context is never equal to anything, so "no
 *  dataset" and "candles not loaded" both fail closed. */
export function sameSweepContext(a: SweepContext | null | undefined, b: SweepContext | null | undefined): boolean {
  if (a == null || b == null) return false;
  return sweepContextKey(a) === sweepContextKey(b);
}

/** Bind a finished sweep to its context. Every part is deep-cloned and frozen,
 *  so nothing the caller keeps can reach the stored grid. */
export function createSweepArtifact(input: CreateSweepArtifactInput): CompletedSweep {
  return freezeDeep({
    context: cloneDeep(input.context),
    result: cloneDeep(input.result),
  });
}

/** May this sweep completion be written? Both halves must hold; a superseded
 *  sweep is discarded silently rather than reported as an error. */
export function sweepResultIsWritable(guard: SweepWriteGuard): boolean {
  return guard.generation === guard.owner && sameSweepContext(guard.started, guard.live);
}

// The two helpers below are intentionally duplicated from `runArtifact.ts`
// rather than shared: exporting them would mean editing the merged
// BUG-RESULT-CONTEXT-001 contract file, and moving them into a new module would
// mix a refactor into a fix PR. Extracting one `services/immutable.ts` is
// recorded as a follow-up suggestion instead.

/** structuredClone, not a JSON round-trip: a sweep cell's metric can legitimately
 *  be a value JSON would distort, and the basis must survive verbatim. */
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
