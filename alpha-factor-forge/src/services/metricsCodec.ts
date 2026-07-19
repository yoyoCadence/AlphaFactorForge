// THE shared JSON-safe metrics codec (PERSIST-001, PR #64 handoff Resolution).
//
// Every persistence boundary (report export, validation records) encodes a
// core `Metrics` through THIS module so there is exactly one codec and it
// cannot drift. Non-finite numeric fields become null plus an explicit status
// entry (METRIC-001 vocabulary) — never JSON.stringify's silent null.
//
// `assertJsonSafe` is the boundary guard: it recursively walks a payload and
// THROWS on any unencoded non-finite number before serialization.

import type { Metrics } from '../core/metrics';
import { nonFiniteStatus, type NonFiniteStatus } from './nonFinite';

/** Metrics with every top-level numeric field narrowed to finite-or-null. */
export type EncodedMetricValues = {
  [K in keyof Metrics]: Metrics[K] extends number ? number | null : Metrics[K];
};

export interface EncodedMetrics {
  values: EncodedMetricValues;
  /** Explicit status for every field that is null because it was non-finite. */
  nonFinite: Partial<Record<keyof Metrics, NonFiniteStatus>>;
}

/** Encode metrics for JSON: non-finite numeric fields become null + a status
 *  entry. monthlyReturns are equity ratios and stay finite by construction,
 *  but the record is CLONED so the snapshot never aliases the caller's
 *  object (PR #65 review: shallow copies made "immutable" records mutable). */
export function encodeMetrics(metrics: Metrics): EncodedMetrics {
  const values: Record<string, unknown> = { ...metrics, monthlyReturns: { ...metrics.monthlyReturns } };
  const nonFinite: Partial<Record<keyof Metrics, NonFiniteStatus>> = {};
  for (const key of Object.keys(metrics) as (keyof Metrics)[]) {
    const v = metrics[key];
    if (typeof v !== 'number') continue;
    const status = nonFiniteStatus(v);
    if (status) {
      nonFinite[key] = status;
      values[key] = null;
    }
  }
  return { values: values as EncodedMetricValues, nonFinite };
}

/**
 * Deep clone a plain-data value (objects/arrays/primitives) so a snapshot
 * shares NO references with its source. Unlike a JSON round-trip this
 * preserves non-finite numbers, so a later `assertJsonSafe` still sees them
 * instead of a silently-nulled copy. Functions/classes are not supported —
 * snapshots are plain data by contract.
 */
export function deepSnapshot<T>(value: T): T {
  if (value === null || typeof value !== 'object') return value;
  if (Array.isArray(value)) return value.map((item) => deepSnapshot(item)) as T;
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(value)) out[k] = deepSnapshot(v);
  return out as T;
}

/** The ONLY sanctioned path to a persisted JSON string: guard, then
 *  stringify. Placing the recursive check immediately at the serialization
 *  boundary means a value mutated AFTER snapshotting still fails closed
 *  instead of becoming a silent JSON null (PR #65 review). */
export function toJsonSafeString(value: unknown, label: string): string {
  assertJsonSafe(value, label);
  return JSON.stringify(value);
}

/**
 * Recursively assert every nested number in `value` is finite. Throws on the
 * first unencoded Infinity/-Infinity/NaN with its path, so a payload can never
 * reach JSON.stringify and be silently nulled. Cycle-safe.
 */
export function assertJsonSafe(value: unknown, label = 'payload'): void {
  const seen = new Set<object>();
  const walk = (v: unknown, path: string): void => {
    if (typeof v === 'number') {
      if (!Number.isFinite(v)) {
        throw new Error(`${label}: unencoded non-finite number at ${path}`);
      }
      return;
    }
    if (v === null || typeof v !== 'object') return;
    if (seen.has(v)) return;
    seen.add(v);
    if (Array.isArray(v)) {
      v.forEach((item, i) => walk(item, `${path}[${i}]`));
      return;
    }
    for (const [k, item] of Object.entries(v)) walk(item, `${path}.${k}`);
  };
  walk(value, label);
}
