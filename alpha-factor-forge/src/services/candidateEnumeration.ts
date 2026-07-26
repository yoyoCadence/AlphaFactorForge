// RUNNER-CONFIG-001: deterministic params-only candidate enumeration
// (`discovery-enumeration-v1`, PR #66 Resolution D2).
//
// Enumeration is the run's LINEAGE: it fixes which hypotheses exist, how many
// were really tested (Score's data-mining `N`), and each candidate's stable
// identity. It must therefore depend only on the config's own content — never
// on object iteration order, thread scheduling, completion order, or a SQLite
// row id. Candidates are sorted by their durable `strategy-v2` hash and the
// stable index is assigned afterwards.
//
// Pure: no SQLite, threads, events, UI, or Test-segment execution.

import { strategyHash } from '../core/hashing';
import {
  DISCOVERY_ENUMERATION_VERSION,
  axisValues,
  deepCloneJson,
  type DiscoveryAxisKey,
  type DiscoveryBase,
  type ResolvedDiscoveryConfig,
} from './discoveryConfig';
import { deriveDiscoverySeed } from './discoverySeed';
import type { ParamsStrategy } from './strategy';

/**
 * Cross-field validity rules applied to every concrete combination. A grid
 * axis is independent by construction, so a Cartesian product inevitably
 * contains combinations that are not valid hypotheses (a "fast" MA slower than
 * the "slow" one). These are PRUNED and counted, not rejected — pruning is
 * the expected outcome of a legal grid, unlike a malformed config.
 *
 * Rules apply regardless of which signal a base preset selects, so the pruned
 * count stays a property of the grid alone and cannot shift when an unrelated
 * signal id changes.
 */
export const DISCOVERY_VALIDITY_RULE_IDS = [
  'fastMA<slowMA',
  'macdFast<macdSlow',
  'rsiBuy<rsiSell',
] as const;
export type DiscoveryValidityRuleId = (typeof DISCOVERY_VALIDITY_RULE_IDS)[number];

/** The first violated rule id in fixed order, or null when the combination is
 *  a valid hypothesis. */
export function candidateValidity(strategy: ParamsStrategy): DiscoveryValidityRuleId | null {
  if (!(strategy.fastMA < strategy.slowMA)) return 'fastMA<slowMA';
  if (!(strategy.macdFast < strategy.macdSlow)) return 'macdFast<macdSlow';
  if (!(strategy.rsiBuy < strategy.rsiSell)) return 'rsiBuy<rsiSell';
  return null;
}

export interface EnumerationCounts {
  /** Cartesian product size across every base, before any filtering. */
  raw: number;
  /** Combinations dropped by `candidateValidity`. */
  prunedInvalid: number;
  /** Survivors whose `strategy-v2` hash was already produced (any base). */
  duplicates: number;
  /** Distinct hypotheses actually queued; this is Score's `N`. */
  finalUnique: number;
}

export interface EnumeratedCandidate {
  /** Stable index assigned AFTER sorting by strategy hash. */
  index: number;
  strategyHash: string;
  /** The base preset the surviving combination came from. */
  baseId: string;
  /** Axis values applied on top of that base preset. */
  appliedAxes: Partial<Record<DiscoveryAxisKey, number>>;
  strategy: ParamsStrategy;
  /** Deterministic per-candidate sub-seeds (`seed-v1`). */
  seeds: { randomEntry: number };
}

export interface CandidatePlan {
  contractVersion: typeof DISCOVERY_ENUMERATION_VERSION;
  datasetContentHash: string;
  rootSeed: number;
  counts: EnumerationCounts;
  candidates: EnumeratedCandidate[];
  /** Resolution D2/D5: N is derived here, never accepted from the config. */
  testedCombinations: { n: number; basis: 'lineage-final-unique' };
}

interface Combination {
  baseId: string;
  appliedAxes: Partial<Record<DiscoveryAxisKey, number>>;
  strategy: ParamsStrategy;
}

/** Row-major odometer over the base's declared axes: the LAST axis varies
 *  fastest. Order does not affect the plan (candidates are hash-sorted) but is
 *  fixed so generated fixtures stay reproducible. */
function combinationsForBase(base: DiscoveryBase): Combination[] {
  const grids = base.axes.map((axis) => ({ key: axis.key, values: axisValues(axis) }));
  // Every combination gets its OWN deep copy. A shallow spread would leave all
  // candidates aliasing one `entryRules`/`exitRules` array, so mutating a
  // single candidate would change the content of every other candidate while
  // their already-computed hashes and seeds stayed put — an inconsistency the
  // runner would then persist as an immutable audit record.
  let combinations: Combination[] = [
    { baseId: base.id, appliedAxes: {}, strategy: deepCloneJson(base.strategy) },
  ];
  for (const grid of grids) {
    const next: Combination[] = [];
    for (const combination of combinations) {
      for (const value of grid.values) {
        next.push({
          baseId: combination.baseId,
          appliedAxes: { ...combination.appliedAxes, [grid.key]: value },
          strategy: { ...deepCloneJson(combination.strategy), [grid.key]: value },
        });
      }
    }
    combinations = next;
  }
  return combinations;
}

/**
 * Raw Cartesian product size across every base, computed with an explicit
 * safe-integer guard so an over-cap grid can never overflow into a small
 * number and slip past the cap check.
 */
export function rawCombinationCount(bases: readonly DiscoveryBase[]): number {
  let total = 0;
  for (const base of bases) {
    let product = 1;
    for (const axis of base.axes) {
      product *= axisValues(axis).length;
      if (!Number.isSafeInteger(product)) {
        throw new RangeError(`base "${base.id}" raw combination count is not a safe integer`);
      }
    }
    total += product;
    if (!Number.isSafeInteger(total)) {
      throw new RangeError('raw combination count is not a safe integer');
    }
  }
  return total;
}

/** Byte-order comparison of two ASCII `strategy-v2:` identities; matches the
 *  Rust port's `str` ordering exactly. */
function compareHash(left: string, right: string): number {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

/**
 * Enumerate, prune, deduplicate, sort, index, and seed every candidate for a
 * resolved config. Async because durable `strategy-v2` identity requires
 * SHA-256; the result is fully deterministic.
 *
 * Throws `RangeError` when the raw product exceeds the configured candidate
 * cap (checked BEFORE any candidate is built, so no jobs can be created for an
 * over-budget run) or when nothing survives pruning.
 */
export async function enumerateCandidates(
  config: ResolvedDiscoveryConfig,
): Promise<CandidatePlan> {
  const raw = rawCombinationCount(config.bases);
  if (raw > config.caps.candidates) {
    throw new RangeError(
      `raw combination count ${raw} exceeds the candidate cap ${config.caps.candidates}`,
    );
  }

  let prunedInvalid = 0;
  let duplicates = 0;
  const byHash = new Map<string, Combination>();

  for (const base of config.bases) {
    for (const combination of combinationsForBase(base)) {
      if (candidateValidity(combination.strategy) !== null) {
        prunedInvalid++;
        continue;
      }
      const hash = await strategyHash(combination.strategy, {
        feePct: combination.strategy.feePct,
        slippagePct: combination.strategy.slipPct,
      });
      if (byHash.has(hash)) {
        duplicates++;
        continue;
      }
      byHash.set(hash, combination);
    }
  }

  const survivors = [...byHash.entries()].sort((left, right) => compareHash(left[0], right[0]));
  if (survivors.length === 0) {
    throw new RangeError('enumeration produced no valid candidates');
  }

  const candidates: EnumeratedCandidate[] = [];
  for (let index = 0; index < survivors.length; index++) {
    const [hash, combination] = survivors[index];
    candidates.push({
      index,
      strategyHash: hash,
      baseId: combination.baseId,
      appliedAxes: combination.appliedAxes,
      strategy: combination.strategy,
      seeds: {
        randomEntry: await deriveDiscoverySeed({
          rootSeed: config.rootSeed,
          datasetContentHash: config.dataset.contentHash,
          strategyHash: hash,
          purpose: 'random-entry',
        }),
      },
    });
  }

  const finalUnique = candidates.length;
  if (prunedInvalid + duplicates + finalUnique !== raw) {
    // Defensive: the four counters are an audit record, so an inconsistency
    // must fail the run rather than be persisted.
    throw new RangeError('enumeration counters do not reconcile with the raw product');
  }

  return {
    contractVersion: DISCOVERY_ENUMERATION_VERSION,
    datasetContentHash: config.dataset.contentHash,
    rootSeed: config.rootSeed,
    counts: { raw, prunedInvalid, duplicates, finalUnique },
    candidates,
    testedCombinations: { n: finalUnique, basis: 'lineage-final-unique' },
  };
}
