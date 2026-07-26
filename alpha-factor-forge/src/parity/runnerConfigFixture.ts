// TypeScript-reference builder for the RUNNER-CONFIG-001 parity fixture.
// Pure and deterministic; scripts/generate-runner-config-fixtures.ts owns file
// IO.
//
// Unlike the engine fixtures, EVERY expected leaf here compares exactly: this
// slice produces identifiers, integers, counts, booleans, and axis values that
// both languages derive with identical IEEE-754 operations. A float axis case
// deliberately locks accumulated `min + i*step` drift so a Rust port that
// "tidies" the arithmetic fails instead of silently enumerating a different
// hypothesis set.
//
// Error cases are HELD by the TypeScript reference: generation and the vitest
// freshness test both execute the real functions and require a `RangeError`
// carrying the recorded fragment, so a fixture can never claim a rejection the
// reference does not actually perform.

import { DATASET_HASH_VERSION, STRATEGY_HASH_VERSION } from '../core/hashing';
import {
  DISCOVERY_AXIS_KEYS,
  DISCOVERY_CONFIG_VERSION,
  DISCOVERY_CONTRACT_VERSIONS,
  DISCOVERY_DEFAULT_CANDIDATE_CAP,
  DISCOVERY_ENUMERATION_VERSION,
  DISCOVERY_HARD_CANDIDATE_CAP,
  DISCOVERY_MAX_AXIS_VALUES,
  DISCOVERY_PRESET_VERSION,
  DISCOVERY_SUPPORTED_SIGNAL_IDS,
  axisValues,
  parseDiscoveryConfig,
  resolveConcurrency,
  type DiscoveryAxis,
  type ResolvedDiscoveryConfig,
} from '../services/discoveryConfig';
import {
  DISCOVERY_VALIDITY_RULE_IDS,
  enumerateCandidates,
  type CandidatePlan,
} from '../services/candidateEnumeration';
import {
  DISCOVERY_SEED_VERSION,
  deriveDiscoverySeed,
  discoverySeedPreimage,
  type DeriveSeedArgs,
} from '../services/discoverySeed';
import { DEFAULT_GATE_CONFIG } from '../services/gate';
import { DEFAULT_SCORE_CONFIG } from '../services/score';
import { defaultStrategy } from '../services/strategy';
import { FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';

export const PARITY_FIXTURE_SCHEMA_VERSION = 'rs-core-parity-fixture-v1';
export const RUNNER_CONFIG_PARITY_FIXTURE_VERSION = 'runner-config-parity-v1';
/** Every expected leaf compares exactly; no tolerance applies to this slice. */
export const EXPECTED_NUMERIC_POLICY = 'exact-v1';

export type RunnerConfigSourceKey =
  | 'generator'
  | 'discoveryConfig'
  | 'candidateEnumeration'
  | 'discoverySeed'
  | 'hashing'
  | 'strategy'
  | 'gate'
  | 'score'
  | 'randomEntry';

const DATASET_A = `${DATASET_HASH_VERSION}:${'a'.repeat(64)}`;
const DATASET_B = `${DATASET_HASH_VERSION}:${'b'.repeat(64)}`;
const STRATEGY_A = `${STRATEGY_HASH_VERSION}:${'1'.repeat(64)}`;
const STRATEGY_B = `${STRATEGY_HASH_VERSION}:${'2'.repeat(64)}`;
const MAX_U32_VALUE = 0xffff_ffff;

type JsonObject = Record<string, unknown>;

function toHex(bytes: Uint8Array): string {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

/** Run a reference call that MUST throw, and record the message fragment. */
function heldError(fragment: string, run: () => unknown): string {
  let thrown: unknown;
  try {
    run();
  } catch (error) {
    thrown = error;
  }
  if (!(thrown instanceof RangeError)) {
    throw new Error(`expected a RangeError for fragment "${fragment}"`);
  }
  if (!thrown.message.includes(fragment)) {
    throw new Error(`error "${thrown.message}" does not contain "${fragment}"`);
  }
  return fragment;
}

async function heldAsyncError(fragment: string, run: () => Promise<unknown>): Promise<string> {
  let thrown: unknown;
  try {
    await run();
  } catch (error) {
    thrown = error;
  }
  if (!(thrown instanceof RangeError)) {
    throw new Error(`expected a RangeError for fragment "${fragment}"`);
  }
  if (!thrown.message.includes(fragment)) {
    throw new Error(`error "${thrown.message}" does not contain "${fragment}"`);
  }
  return fragment;
}

// ---------- shared config inputs ----------

interface BaseSpec {
  id: string;
  axes: { key: string; min: number; max: number; step: number }[];
  strategy?: JsonObject;
}

function baseSpec(spec: BaseSpec): JsonObject {
  return {
    id: spec.id,
    presetVersion: DISCOVERY_PRESET_VERSION,
    strategy: { ...defaultStrategy(), ...spec.strategy },
    axes: spec.axes,
  };
}

function envelope(bases: BaseSpec[], overrides: JsonObject = {}): JsonObject {
  return {
    envelopeVersion: DISCOVERY_CONFIG_VERSION,
    contracts: { ...DISCOVERY_CONTRACT_VERSIONS },
    dataset: { id: 7, contentHash: DATASET_A },
    bases: bases.map(baseSpec),
    embargo: { holdingAllowanceBars: 10 },
    execution: { startEquity: 10000 },
    benchmarkCosts: { feePct: 0.05, slipPct: 0.02 },
    randomEntry: { runs: 200 },
    gateConfig: { ...DEFAULT_GATE_CONFIG },
    scoreConfig: {
      caps: { ...DEFAULT_SCORE_CONFIG.caps },
      weights: { ...DEFAULT_SCORE_CONFIG.weights },
    },
    rootSeed: 20260726,
    caps: { candidates: DISCOVERY_DEFAULT_CANDIDATE_CAP },
    maxConcurrency: null,
    ...overrides,
  };
}

const SINGLE_AXIS_CONFIG = envelope([
  { id: 'ma-cross', axes: [{ key: 'fastMA', min: 5, max: 11, step: 3 }] },
]);

const MULTI_BASE_CONFIG = envelope(
  [
    {
      id: 'ma-cross',
      axes: [
        { key: 'fastMA', min: 5, max: 9, step: 2 },
        { key: 'slowMA', min: 20, max: 30, step: 10 },
      ],
    },
    {
      id: 'rsi-reversion',
      axes: [{ key: 'rsiPeriod', min: 10, max: 14, step: 2 }],
      strategy: { entrySig: 'rsiOversold', exitSig: 'rsiOverbought' },
    },
  ],
  {
    embargo: { holdingAllowanceBars: 0 },
    execution: { startEquity: 25000.5 },
    randomEntry: { runs: 1000 },
    gateConfig: {
      ...DEFAULT_GATE_CONFIG,
      minTrades: 5,
      rollingWindowBars: 20,
      minRollingPositiveRatio: 1,
      maxDrawdown: 1,
      minRandomEntryPercentile: 0,
    },
    scoreConfig: {
      caps: { ...DEFAULT_SCORE_CONFIG.caps, profitFactor: 2.5, dataMiningLog10: 3 },
      weights: { ...DEFAULT_SCORE_CONFIG.weights, complexity: 0, turnover: 2 },
    },
    rootSeed: MAX_U32_VALUE,
    caps: { candidates: DISCOVERY_HARD_CANDIDATE_CAP },
    maxConcurrency: 3,
  },
);

const PRUNE_CONFIG = envelope([
  { id: 'ma-cross', axes: [{ key: 'fastMA', min: 18, max: 24, step: 3 }] },
]);

const DUPLICATE_CONFIG = envelope([
  { id: 'grid-low', axes: [{ key: 'fastMA', min: 5, max: 11, step: 3 }] },
  { id: 'grid-high', axes: [{ key: 'fastMA', min: 8, max: 14, step: 3 }] },
]);

const DISJOINT_CONFIG = envelope([
  { id: 'low', axes: [{ key: 'fastMA', min: 5, max: 6, step: 1 }] },
  { id: 'high', axes: [{ key: 'fastMA', min: 15, max: 16, step: 1 }] },
]);

const DISJOINT_REVERSED_CONFIG = envelope([
  { id: 'high', axes: [{ key: 'fastMA', min: 15, max: 16, step: 1 }] },
  { id: 'low', axes: [{ key: 'fastMA', min: 5, max: 6, step: 1 }] },
]);

/** Mutate a deep clone of the canonical envelope for one error case. */
function broken(mutate: (config: JsonObject) => void): JsonObject {
  const config = clone(SINGLE_AXIS_CONFIG);
  mutate(config);
  return config;
}

function firstBase(config: JsonObject): JsonObject {
  return (config.bases as JsonObject[])[0];
}

function firstStrategy(config: JsonObject): JsonObject {
  return firstBase(config).strategy as JsonObject;
}

// ---------- fixture shape ----------

export interface SeedCase {
  id: string;
  input: DeriveSeedArgs;
  expected: { preimageHex: string; seed: number };
}

export interface AxisCase {
  id: string;
  input: DiscoveryAxis;
  expected: number[];
}

export interface ConcurrencyCase {
  id: string;
  input: { requested: number | null; logicalCores: number };
  expected: number;
}

export interface ConfigCase {
  id: string;
  logicalCores: number;
  input: JsonObject;
  expected: ResolvedDiscoveryConfig;
}

export interface EnumerationCase {
  id: string;
  logicalCores: number;
  input: JsonObject;
  expected: CandidatePlan;
}

export interface ErrorCase<Input> {
  id: string;
  input: Input;
  expectedErrorIncludes: string;
}

export async function buildRunnerConfigParityFixture(
  sourceHashes: Record<RunnerConfigSourceKey, string>,
) {
  const seedCases: SeedCase[] = [];
  for (const [id, input] of [
    ['seed-root-zero', { rootSeed: 0, datasetContentHash: DATASET_A, strategyHash: STRATEGY_A, purpose: 'random-entry' }],
    ['seed-root-max-u32', { rootSeed: MAX_U32_VALUE, datasetContentHash: DATASET_A, strategyHash: STRATEGY_A, purpose: 'random-entry' }],
    ['seed-root-mid', { rootSeed: 20260726, datasetContentHash: DATASET_A, strategyHash: STRATEGY_A, purpose: 'random-entry' }],
    ['seed-other-strategy', { rootSeed: 20260726, datasetContentHash: DATASET_A, strategyHash: STRATEGY_B, purpose: 'random-entry' }],
    ['seed-other-dataset', { rootSeed: 20260726, datasetContentHash: DATASET_B, strategyHash: STRATEGY_A, purpose: 'random-entry' }],
  ] as [string, DeriveSeedArgs][]) {
    seedCases.push({
      id,
      input,
      expected: {
        preimageHex: toHex(discoverySeedPreimage(input)),
        seed: await deriveDiscoverySeed(input),
      },
    });
  }

  const seedErrorCases: ErrorCase<DeriveSeedArgs>[] = (
    [
      ['seed-negative-root', { rootSeed: -1, datasetContentHash: DATASET_A, strategyHash: STRATEGY_A, purpose: 'random-entry' }, 'rootSeed must be an integer in [0, 4294967295]'],
      ['seed-root-above-u32', { rootSeed: MAX_U32_VALUE + 1, datasetContentHash: DATASET_A, strategyHash: STRATEGY_A, purpose: 'random-entry' }, 'rootSeed must be an integer in [0, 4294967295]'],
      ['seed-fractional-root', { rootSeed: 1.5, datasetContentHash: DATASET_A, strategyHash: STRATEGY_A, purpose: 'random-entry' }, 'rootSeed must be an integer in [0, 4294967295]'],
      ['seed-legacy-dataset-hash', { rootSeed: 1, datasetContentHash: 'legacy-unversioned', strategyHash: STRATEGY_A, purpose: 'random-entry' }, 'datasetContentHash must be a durable dataset-content-v2 identity'],
      ['seed-ephemeral-strategy-hash', { rootSeed: 1, datasetContentHash: DATASET_A, strategyHash: 'ephemeral-fnv1a:0000000000000000', purpose: 'random-entry' }, 'strategyHash must be a durable strategy-v2 identity'],
      // A correct prefix is NOT an identity: the digest must be present, the
      // right length, and lowercase, or a malformed value seeds a real stream.
      ['seed-empty-dataset-digest', { rootSeed: 1, datasetContentHash: 'dataset-content-v2:', strategyHash: STRATEGY_A, purpose: 'random-entry' }, 'datasetContentHash must be a durable dataset-content-v2 identity'],
      ['seed-truncated-strategy-digest', { rootSeed: 1, datasetContentHash: DATASET_A, strategyHash: 'strategy-v2:abc123', purpose: 'random-entry' }, 'strategyHash must be a durable strategy-v2 identity'],
      ['seed-uppercase-strategy-digest', { rootSeed: 1, datasetContentHash: DATASET_A, strategyHash: `strategy-v2:${'A'.repeat(64)}`, purpose: 'random-entry' }, 'strategyHash must be a durable strategy-v2 identity'],
      ['seed-non-hex-strategy-digest', { rootSeed: 1, datasetContentHash: DATASET_A, strategyHash: `strategy-v2:${'g'.repeat(64)}`, purpose: 'random-entry' }, 'strategyHash must be a durable strategy-v2 identity'],
      ['seed-unknown-purpose', { rootSeed: 1, datasetContentHash: DATASET_A, strategyHash: STRATEGY_A, purpose: 'gate' }, 'unsupported seed purpose "gate"'],
    ] as [string, DeriveSeedArgs, string][]
  ).map(([id, input, fragment]) => ({
    id,
    input,
    expectedErrorIncludes: heldError(fragment, () => discoverySeedPreimage(input)),
  }));

  const axisCases: AxisCase[] = (
    [
      ['axis-integer-inclusive', { key: 'fastMA', min: 5, max: 11, step: 3 }],
      ['axis-integer-truncated', { key: 'fastMA', min: 5, max: 12, step: 3 }],
      ['axis-single-value', { key: 'fastMA', min: 9, max: 9, step: 1 }],
      ['axis-float-exact-halves', { key: 'bbMult', min: 1.5, max: 2.5, step: 0.5 }],
      // Locks accumulated `min + i*step` drift EXACTLY (0.1 * 3 is not 0.3).
      ['axis-float-binary-drift', { key: 'tpPct', min: 0, max: 0.5, step: 0.1 }],
      ['axis-max-values-boundary', { key: 'fastMA', min: 1, max: 64, step: 1 }],
    ] as [string, DiscoveryAxis][]
  ).map(([id, input]) => ({ id, input, expected: axisValues(input) }));

  const axisErrorCases: ErrorCase<DiscoveryAxis>[] = (
    [
      ['axis-above-value-cap', { key: 'fastMA', min: 1, max: 65, step: 1 }, `axis "fastMA" produces more than ${DISCOVERY_MAX_AXIS_VALUES} values`],
    ] as [string, DiscoveryAxis, string][]
  ).map(([id, input, fragment]) => ({
    id,
    input,
    expectedErrorIncludes: heldError(fragment, () => axisValues(input)),
  }));

  const concurrencyCases: ConcurrencyCase[] = (
    [
      ['concurrency-default-single-core', null, 1],
      ['concurrency-default-dual-core', null, 2],
      ['concurrency-default-many-cores', null, 16],
      ['concurrency-override-floor', 1, 4],
      ['concurrency-override-all-cores', 4, 4],
    ] as [string, number | null, number][]
  ).map(([id, requested, logicalCores]) => ({
    id,
    input: { requested, logicalCores },
    expected: resolveConcurrency(requested, logicalCores),
  }));

  const concurrencyErrorCases: ErrorCase<ConcurrencyCase['input']>[] = (
    [
      ['concurrency-zero-override', { requested: 0, logicalCores: 4 }, 'maxConcurrency must be an integer in [1, 4]'],
      ['concurrency-above-cores', { requested: 5, logicalCores: 4 }, 'maxConcurrency must be an integer in [1, 4]'],
      ['concurrency-fractional-override', { requested: 2.5, logicalCores: 4 }, 'maxConcurrency must be an integer in [1, 4]'],
      ['concurrency-zero-cores', { requested: null, logicalCores: 0 }, 'logicalCores must be an integer >= 1'],
    ] as [string, ConcurrencyCase['input'], string][]
  ).map(([id, input, fragment]) => ({
    id,
    input,
    expectedErrorIncludes: heldError(fragment, () =>
      resolveConcurrency(input.requested, input.logicalCores),
    ),
  }));

  const configCases: ConfigCase[] = (
    [
      ['config-default-single-base', 8, SINGLE_AXIS_CONFIG],
      ['config-multi-base-overrides', 4, MULTI_BASE_CONFIG],
    ] as [string, number, JsonObject][]
  ).map(([id, logicalCores, input]) => ({
    id,
    logicalCores,
    input: clone(input),
    expected: parseDiscoveryConfig(input, { logicalCores }),
  }));

  const configErrorSpecs: [string, JsonObject, string][] = [
    ['config-unknown-envelope-key', broken((config) => { config.extra = 1; }), 'discoveryConfig has unknown key "extra"'],
    // Two unknown non-ASCII keys whose UTF-16 and UTF-8 orders DISAGREE:
    // "\u{1F600}" sorts first in UTF-16 (JavaScript's default) and last in
    // UTF-8 (Rust's `String: Ord`). Both languages must name "＀" first.
    ['config-unknown-key-utf8-order', broken((config) => { config['\u{1F600}'] = 1; config['＀'] = 1; }), 'discoveryConfig has unknown key "＀"'],
    ['config-missing-envelope-key', broken((config) => { delete config.rootSeed; }), 'discoveryConfig is missing key "rootSeed"'],
    ['config-envelope-version-mismatch', broken((config) => { config.envelopeVersion = 'discovery-config-v2'; }), 'envelopeVersion must be "discovery-config-v1"'],
    ['config-contract-version-mismatch', broken((config) => { (config.contracts as JsonObject).gate = 'gate-v2'; }), 'contracts.gate must be "gate-v1"'],
    ['config-preset-version-mismatch', broken((config) => { firstBase(config).presetVersion = 'preset-v9'; }), 'presetVersion must be "discovery-preset-v1"'],
    ['config-dataset-legacy-hash', broken((config) => { (config.dataset as JsonObject).contentHash = 'legacy-unversioned'; }), 'contentHash must be a durable dataset-content-v2 identity'],
    ['config-dataset-empty-digest', broken((config) => { (config.dataset as JsonObject).contentHash = 'dataset-content-v2:'; }), 'contentHash must be a durable dataset-content-v2 identity'],
    ['config-dataset-uppercase-digest', broken((config) => { (config.dataset as JsonObject).contentHash = `dataset-content-v2:${'A'.repeat(64)}`; }), 'contentHash must be a durable dataset-content-v2 identity'],
    ['config-dataset-id-zero', broken((config) => { (config.dataset as JsonObject).id = 0; }), 'discoveryConfig.dataset.id must be an integer in [1,'],
    ['config-blocks-mode-rejected', broken((config) => { firstStrategy(config).mode = 'blocks'; }), 'mode must be "params"'],
    ['config-code-mode-rejected', broken((config) => { firstStrategy(config).mode = 'code'; }), 'mode must be "params"'],
    ['config-unsupported-signal', broken((config) => { firstStrategy(config).entrySig = 'stochOversold'; }), 'entrySig must be one of'],
    ['config-unknown-fill-mode', broken((config) => { firstStrategy(config).fillMode = 'open'; }), 'fillMode must be one of'],
    ['config-strategy-unknown-key', broken((config) => { firstStrategy(config).extra = 1; }), 'strategy has unknown key "extra"'],
    ['config-strategy-missing-key', broken((config) => { delete firstStrategy(config).bbMult; }), 'strategy is missing key "bbMult"'],
    ['config-period-below-one', broken((config) => { firstStrategy(config).fastMA = 0; }), 'fastMA must be an integer >= 1'],
    ['config-period-fractional', broken((config) => { firstStrategy(config).slowMA = 21.5; }), 'slowMA must be an integer >= 1'],
    ['config-multiplier-not-positive', broken((config) => { firstStrategy(config).bbMult = 0; }), 'bbMult must be > 0'],
    ['config-size-out-of-range', broken((config) => { firstStrategy(config).sizePct = 0; }), 'sizePct must be in (0, 100]'],
    // Percent units are bounded at 100 because the engine's normalized
    // fraction check rejects anything above 1 — admitting these would queue a
    // run guaranteed to throw once a job executes.
    ['config-fee-percent-above-range', broken((config) => { firstStrategy(config).feePct = 101; }), 'feePct must be in [0, 100]'],
    ['config-slippage-percent-above-range', broken((config) => { firstStrategy(config).slipPct = 100.5; }), 'slipPct must be in [0, 100]'],
    ['config-stop-loss-percent-above-range', broken((config) => { firstStrategy(config).slPct = 101; }), 'slPct must be in [0, 100]'],
    ['config-take-profit-percent-negative', broken((config) => { firstStrategy(config).tpPct = -1; }), 'tpPct must be in [0, 100]'],
    ['config-axis-generates-percent-above-range', broken((config) => { firstBase(config).axes = [{ key: 'tpPct', min: 90, max: 110, step: 10 }]; }), 'generates an invalid value: tpPct must be in [0, 100]'],
    ['config-level-out-of-range', broken((config) => { firstStrategy(config).rsiBuy = 101; }), 'rsiBuy must be in [0, 100]'],
    ['config-axis-key-not-whitelisted', broken((config) => { firstBase(config).axes = [{ key: 'feePct', min: 0, max: 0.1, step: 0.05 }]; }), 'key must be one of'],
    ['config-axis-step-not-positive', broken((config) => { firstBase(config).axes = [{ key: 'fastMA', min: 5, max: 11, step: 0 }]; }), 'step must be > 0'],
    ['config-axis-inverted-range', broken((config) => { firstBase(config).axes = [{ key: 'fastMA', min: 11, max: 5, step: 1 }]; }), 'max must be >= min'],
    ['config-axis-fractional-integer-bound', broken((config) => { firstBase(config).axes = [{ key: 'fastMA', min: 5, max: 11, step: 0.5 }]; }), 'must be an integer for the integer axis "fastMA"'],
    ['config-axis-repeated-key', broken((config) => { firstBase(config).axes = [{ key: 'fastMA', min: 5, max: 11, step: 3 }, { key: 'fastMA', min: 5, max: 11, step: 3 }]; }), 'repeats axis key "fastMA"'],
    ['config-axis-above-value-cap', broken((config) => { firstBase(config).axes = [{ key: 'fastMA', min: 1, max: 65, step: 1 }]; }), `produces more than ${DISCOVERY_MAX_AXIS_VALUES} values`],
    ['config-axis-generates-invalid-value', broken((config) => { firstBase(config).axes = [{ key: 'rsiBuy', min: 90, max: 110, step: 10 }]; }), 'generates an invalid value: rsiBuy must be in [0, 100]'],
    ['config-empty-bases', broken((config) => { config.bases = []; }), 'bases must contain at least one base preset'],
    ['config-duplicate-base-id', broken((config) => { config.bases = [firstBase(config), clone(firstBase(config))]; }), 'repeats base id "ma-cross"'],
    ['config-invalid-base-id', broken((config) => { firstBase(config).id = 'MA Cross'; }), 'id must match'],
    ['config-benchmark-costs-mismatch', broken((config) => { config.benchmarkCosts = { feePct: 0.01, slipPct: 0.02 }; }), 'benchmarkCosts must match bases[0] costs'],
    ['config-random-entry-runs-above-cap', broken((config) => { (config.randomEntry as JsonObject).runs = 1001; }), 'runs must be an integer in [1, 1000]'],
    ['config-negative-holding-allowance', broken((config) => { (config.embargo as JsonObject).holdingAllowanceBars = -1; }), 'holdingAllowanceBars must be an integer in [0,'],
    ['config-start-equity-zero', broken((config) => { (config.execution as JsonObject).startEquity = 0; }), 'startEquity must be > 0'],
    ['config-candidate-cap-above-hard-cap', broken((config) => { (config.caps as JsonObject).candidates = DISCOVERY_HARD_CANDIDATE_CAP + 1; }), `candidates must be an integer in [1, ${DISCOVERY_HARD_CANDIDATE_CAP}]`],
    ['config-root-seed-above-u32', broken((config) => { config.rootSeed = MAX_U32_VALUE + 1; }), 'rootSeed must be an integer in [0, 4294967295]'],
    ['config-max-concurrency-string', broken((config) => { config.maxConcurrency = 'auto'; }), 'maxConcurrency must be a number or null'],
    ['config-max-concurrency-above-cores', broken((config) => { config.maxConcurrency = 9; }), 'maxConcurrency must be an integer in [1, 8]'],
    ['config-gate-min-trades-invalid', broken((config) => { (config.gateConfig as JsonObject).minTrades = 0; }), 'minTrades must be a positive integer'],
    ['config-gate-fraction-invalid', broken((config) => { (config.gateConfig as JsonObject).maxDrawdown = 1.5; }), 'maxDrawdown must be a fraction in (0, 1]'],
    ['config-gate-percentile-invalid', broken((config) => { (config.gateConfig as JsonObject).minRandomEntryPercentile = 101; }), 'minRandomEntryPercentile must be in [0, 100]'],
    ['config-score-cap-invalid', broken((config) => { ((config.scoreConfig as JsonObject).caps as JsonObject).cagr = 0; }), 'cap cagr must be finite and > 0'],
    ['config-score-profit-factor-cap-invalid', broken((config) => { ((config.scoreConfig as JsonObject).caps as JsonObject).profitFactor = 1; }), 'cap profitFactor must be > 1 (1 is the break-even floor)'],
    ['config-score-negative-weight', broken((config) => { ((config.scoreConfig as JsonObject).weights as JsonObject).cagr = -1; }), 'weight cagr must be finite and >= 0'],
    ['config-score-regime-weight-deferred', broken((config) => { ((config.scoreConfig as JsonObject).weights as JsonObject).regime = 0.1; }), 'regime weight must stay 0 until REGIME-001 implements the regime classifier'],
  ];

  const configErrorCases: (ErrorCase<JsonObject> & { logicalCores: number })[] =
    configErrorSpecs.map(([id, input, fragment]) => ({
      id,
      logicalCores: 8,
      input,
      expectedErrorIncludes: heldError(fragment, () =>
        parseDiscoveryConfig(input, { logicalCores: 8 }),
      ),
    }));

  const enumerationSpecs: [string, number, JsonObject][] = [
    ['enumerate-single-axis', 8, SINGLE_AXIS_CONFIG],
    ['enumerate-multi-base-product', 4, MULTI_BASE_CONFIG],
    ['enumerate-cross-field-prune', 8, PRUNE_CONFIG],
    ['enumerate-cross-base-duplicates', 8, DUPLICATE_CONFIG],
    ['enumerate-disjoint-bases', 8, DISJOINT_CONFIG],
    ['enumerate-disjoint-bases-reversed', 8, DISJOINT_REVERSED_CONFIG],
  ];
  const enumerationCases: EnumerationCase[] = [];
  for (const [id, logicalCores, input] of enumerationSpecs) {
    enumerationCases.push({
      id,
      logicalCores,
      input: clone(input),
      expected: await enumerateCandidates(parseDiscoveryConfig(input, { logicalCores })),
    });
  }

  const enumerationErrorSpecs: [string, JsonObject, string][] = [
    [
      'enumerate-above-candidate-cap',
      envelope([{ id: 'wide', axes: [{ key: 'fastMA', min: 1, max: 20, step: 1 }] }], {
        caps: { candidates: 10 },
      }),
      'raw combination count 20 exceeds the candidate cap 10',
    ],
    [
      'enumerate-all-pruned',
      envelope([{ id: 'all-invalid', axes: [{ key: 'fastMA', min: 21, max: 24, step: 3 }] }]),
      'enumeration produced no valid candidates',
    ],
  ];
  const enumerationErrorCases: (ErrorCase<JsonObject> & { logicalCores: number })[] = [];
  for (const [id, input, fragment] of enumerationErrorSpecs) {
    enumerationErrorCases.push({
      id,
      logicalCores: 8,
      input: clone(input),
      expectedErrorIncludes: await heldAsyncError(fragment, () =>
        enumerateCandidates(parseDiscoveryConfig(input, { logicalCores: 8 })),
      ),
    });
  }

  return {
    schemaVersion: PARITY_FIXTURE_SCHEMA_VERSION,
    fixtureVersion: RUNNER_CONFIG_PARITY_FIXTURE_VERSION,
    contracts: {
      config: DISCOVERY_CONFIG_VERSION,
      preset: DISCOVERY_PRESET_VERSION,
      enumeration: DISCOVERY_ENUMERATION_VERSION,
      seed: DISCOVERY_SEED_VERSION,
      strategyHash: STRATEGY_HASH_VERSION,
      datasetHash: DATASET_HASH_VERSION,
    },
    generator: {
      reference: 'src/parity/runnerConfigFixture.ts',
      command: 'npm run fixtures:runner-config',
      sourceHashEncoding: FIXTURE_SOURCE_HASH_ENCODING,
      sourceHashes,
    },
    expectedNumericPolicy: EXPECTED_NUMERIC_POLICY,
    caps: {
      defaultCandidateCap: DISCOVERY_DEFAULT_CANDIDATE_CAP,
      hardCandidateCap: DISCOVERY_HARD_CANDIDATE_CAP,
      maxAxisValues: DISCOVERY_MAX_AXIS_VALUES,
    },
    axisKeys: [...DISCOVERY_AXIS_KEYS],
    supportedSignalIds: [...DISCOVERY_SUPPORTED_SIGNAL_IDS],
    validityRuleIds: [...DISCOVERY_VALIDITY_RULE_IDS],
    seedCases,
    seedErrorCases,
    axisCases,
    axisErrorCases,
    concurrencyCases,
    concurrencyErrorCases,
    configCases,
    configErrorCases,
    enumerationCases,
    enumerationErrorCases,
  };
}

export type RunnerConfigParityFixture = Awaited<
  ReturnType<typeof buildRunnerConfigParityFixture>
>;
