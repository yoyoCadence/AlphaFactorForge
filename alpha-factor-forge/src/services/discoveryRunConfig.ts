// RUNNER-UI-001b-1 — build one `discovery-config-v1` envelope from what the
// workspace already has on screen.
//
// `start_discovery` takes the full admission envelope: thirteen exact keys, ten
// pinned contract versions, the dataset identity, base presets with axes, the
// complete Gate and Score configs, an explicit seed, and the caps. Hand-writing
// that in a component would mean a second, drifting copy of a contract the
// backend rejects wholesale — so this module assembles it from the modules that
// already own each part, and then validates the result with
// `parseDiscoveryConfig`: the SAME parser the backend mirrors.
//
// Consequences worth stating, because they are the point of the module:
//   - A malformed run fails HERE, with the parser's path-qualified message,
//     before any Tauri call is made and therefore before any run row exists.
//   - Benchmark costs are DERIVED from the base strategy, never entered
//     separately, because the envelope requires them to agree with every base
//     preset and a second input could only ever disagree.
//   - The strategy passes through `checkNumericParam` inside the parser, so the
//     STRATEGY-VALIDATION-001 rule set applies to a discovery run for free.
//
// Pure: no React, DOM, IO, or Tauri. The caller invokes the command.

import {
  DISCOVERY_CONFIG_VERSION,
  DISCOVERY_CONTRACT_VERSIONS,
  DISCOVERY_DEFAULT_CANDIDATE_CAP,
  DISCOVERY_PRESET_VERSION,
  parseDiscoveryConfig,
  type DiscoveryAxis,
  type ResolvedDiscoveryConfig,
} from './discoveryConfig';
import { DEFAULT_GATE_CONFIG } from './gate';
import { DEFAULT_SCORE_CONFIG } from './score';
import { DEFAULT_RANDOM_ENTRY_RUNS } from './randomEntry';
import { MAX_U32 } from './discoverySeed';
import type { ParamsStrategy } from './strategy';

/** `core/backtest` starts every run from this equity when none is given; the
 *  discovery envelope must state it explicitly, so it is stated here once. */
export const DISCOVERY_DEFAULT_START_EQUITY = 10_000;

/** Default base-preset id when the caller does not name one. Must match the
 *  envelope's lowercase-hyphenated id pattern. */
export const DISCOVERY_DEFAULT_BASE_ID = 'workspace-strategy';

export interface DiscoveryRunOptions {
  /** Grid axes for the single base preset. Validated by `parseDiscoveryConfig`. */
  axes: DiscoveryAxis[];
  /**
   * Caller-approved allowance for trades whose holding period could span a
   * segment boundary. Explicit, never implied — the VAL-003 contract — so the
   * UI must show it even when it is 0.
   */
  holdingAllowanceBars: number;
  /**
   * Explicit `seed-v1` root. Required, and never defaulted here: a run's whole
   * Random Entry distribution is derived from it, so it has to be a value the
   * user can see, keep, and re-enter to reproduce the run.
   */
  rootSeed: number;
  randomEntryRuns?: number;
  candidateCap?: number;
  startEquity?: number;
  baseId?: string;
}

export interface BuildDiscoveryConfigInput {
  /** Durable dataset identity; `hash` is the `dataset-content-v2` value. */
  dataset: { id: number; hash: string };
  /** The workspace strategy, used as the single base preset. */
  strategy: ParamsStrategy;
  options: DiscoveryRunOptions;
  /**
   * Cores used for LOCAL validation only. The envelope always sends
   * `maxConcurrency: null` (see below), so this cannot change what the backend
   * resolves; it exists because the shared parser requires a number.
   */
  logicalCores: number;
}

export interface BuiltDiscoveryConfig {
  /** The exact JSON value to hand to `start_discovery`. */
  envelope: Record<string, unknown>;
  /** What the shared parser resolved it to — for display, not for sending. */
  resolved: ResolvedDiscoveryConfig;
}

/** The thirteen envelope keys, in the order `discovery-config-v1` declares
 *  them. Pinned here so a backend key change fails this module's test rather
 *  than a user's run. */
export const DISCOVERY_ENVELOPE_KEYS = [
  'envelopeVersion',
  'contracts',
  'dataset',
  'bases',
  'embargo',
  'execution',
  'benchmarkCosts',
  'randomEntry',
  'gateConfig',
  'scoreConfig',
  'rootSeed',
  'caps',
  'maxConcurrency',
] as const;

/**
 * Assemble and validate one run envelope. Throws `RangeError` with the parser's
 * path-qualified message when the result would not be admissible, so the caller
 * never reaches `invoke` with a config the backend will reject.
 */
export function buildDiscoveryConfig(input: BuildDiscoveryConfigInput): BuiltDiscoveryConfig {
  const { dataset, strategy, options } = input;

  const envelope: Record<string, unknown> = {
    envelopeVersion: DISCOVERY_CONFIG_VERSION,
    // Copied from the owning constants, never retyped: the parser rejects any
    // recorded version that differs from this build's.
    contracts: { ...DISCOVERY_CONTRACT_VERSIONS },
    dataset: { id: dataset.id, contentHash: dataset.hash },
    bases: [
      {
        id: options.baseId ?? DISCOVERY_DEFAULT_BASE_ID,
        presetVersion: DISCOVERY_PRESET_VERSION,
        // Deep clone: a resolved config must not alias the editor's object, or
        // a later edit would silently change what the run recorded.
        strategy: cloneStrategy(strategy),
        axes: options.axes.map((axis) => ({
          key: axis.key,
          min: axis.min,
          max: axis.max,
          step: axis.step,
        })),
      },
    ],
    embargo: { holdingAllowanceBars: options.holdingAllowanceBars },
    execution: { startEquity: options.startEquity ?? DISCOVERY_DEFAULT_START_EQUITY },
    // Derived, not asked for: the envelope requires these to equal every base
    // preset's costs, so a separate input could only introduce a mismatch.
    benchmarkCosts: { feePct: strategy.feePct, slipPct: strategy.slipPct },
    randomEntry: { runs: options.randomEntryRuns ?? DEFAULT_RANDOM_ENTRY_RUNS },
    gateConfig: { ...DEFAULT_GATE_CONFIG },
    scoreConfig: {
      caps: { ...DEFAULT_SCORE_CONFIG.caps },
      weights: { ...DEFAULT_SCORE_CONFIG.weights },
    },
    rootSeed: options.rootSeed,
    caps: { candidates: options.candidateCap ?? DISCOVERY_DEFAULT_CANDIDATE_CAP },
    // Always null in v1: "use the machine default", resolved by the backend with
    // ITS core count. Sending an explicit number would let a value validated
    // against the WebView's `hardwareConcurrency` be rejected by a backend that
    // reports a different one.
    maxConcurrency: null,
  };

  // The single admission authority. Validating here means an invalid run is a
  // visible error in the workspace, not a rejected invoke after the fact.
  const resolved = parseDiscoveryConfig(envelope, { logicalCores: input.logicalCores });
  return { envelope, resolved };
}

/**
 * A `u32` root seed, uniform over the contract's whole range. Callers keep and
 * display it: reproducing a run means re-entering this number.
 *
 * The clamp is not decoration. `Math.random()` is exclusive of 1, but the
 * generator is injectable, and `floor(1 * (MAX_U32 + 1))` is `MAX_U32 + 1` —
 * one past the range the envelope accepts. A seed that fails admission only for
 * the last possible draw is the kind of defect that surfaces once in
 * production, so it is closed here and asserted at both extremes.
 */
export function randomRootSeed(random: () => number = Math.random): number {
  return Math.min(MAX_U32, Math.floor(random() * (MAX_U32 + 1)));
}

function cloneStrategy(strategy: ParamsStrategy): ParamsStrategy {
  return structuredClone(strategy);
}
