// RUNNER-CONFIG-001: strict `discovery-config-v1` parsing (PR #66 Resolution
// D2 + D4). This module is the discovery run's INPUT contract and its only
// admission gate — nothing downstream re-checks these invariants.
//
// Everything here is pure: no SQLite, no threads, no Tauri events, no UI, and
// no Test-segment execution. Parsing is deliberately total and fail-closed —
// unknown keys, a version mismatch, a non-finite number, a non-params
// candidate mode, an out-of-domain parameter, or an over-cap raw product all
// throw a `RangeError` BEFORE any candidate is enumerated or any job exists.
//
// See docs/discovery-config-contract.md for the recorded conventions.

import { DATASET_HASH_VERSION, STRATEGY_HASH_VERSION } from '../core/hashing';
import { DEFAULT_GATE_CONFIG, GATE_CONTRACT_VERSION, type GateConfig } from './gate';
import { DEFAULT_SCORE_CONFIG, SCORE_FORMULA_VERSION, type ScoreConfig } from './score';
import { MAX_RANDOM_ENTRY_RUNS } from './randomEntry';
import { DISCOVERY_SEED_VERSION, MAX_U32 } from './discoverySeed';
import type { Direction, FillMode } from '../core/backtest';
import type { ParamsStrategy, SignalId } from './strategy';

export const DISCOVERY_CONFIG_VERSION = 'discovery-config-v1';
export const DISCOVERY_PRESET_VERSION = 'discovery-preset-v1';
export const DISCOVERY_ENUMERATION_VERSION = 'discovery-enumeration-v1';

/** Every contract this input envelope is pinned to. A run whose recorded
 *  versions differ from the build's is rejected rather than reinterpreted. */
export const DISCOVERY_CONTRACT_VERSIONS = {
  strategyHash: STRATEGY_HASH_VERSION,
  datasetHash: DATASET_HASH_VERSION,
  split: 'validation-split-v1',
  embargo: 'embargo-derivation-v1',
  backtest: 'backtest-execution-v1',
  metrics: 'metrics-v1',
  benchmarks: 'benchmark-suite-v1',
  randomEntry: 'random-entry-v1',
  gate: GATE_CONTRACT_VERSION,
  score: SCORE_FORMULA_VERSION,
  seed: DISCOVERY_SEED_VERSION,
  enumeration: DISCOVERY_ENUMERATION_VERSION,
} as const;
export type DiscoveryContractVersions = typeof DISCOVERY_CONTRACT_VERSIONS;

/** Resolution D2 caps. The default is the UI-facing budget; the hard cap may
 *  only move with a config-contract bump plus performance evidence. */
export const DISCOVERY_DEFAULT_CANDIDATE_CAP = 256;
export const DISCOVERY_HARD_CANDIDATE_CAP = 4096;
/** Per-axis value cap, matching the recorded parameter-sweep v1 convention. */
export const DISCOVERY_MAX_AXIS_VALUES = 64;

// ---------- parameter domains ----------

type NumericDomain = 'period' | 'level' | 'positive' | 'nonNegative' | 'sizePercent';

/** Domain of every numeric `ParamsStrategy` field, in declaration order. */
const NUMERIC_PARAM_DOMAINS = {
  fastMA: 'period',
  slowMA: 'period',
  emaPeriod: 'period',
  rsiPeriod: 'period',
  rsiBuy: 'level',
  rsiSell: 'level',
  macdFast: 'period',
  macdSlow: 'period',
  macdSignal: 'period',
  bbPeriod: 'period',
  bbMult: 'positive',
  slPct: 'nonNegative',
  tpPct: 'nonNegative',
  feePct: 'nonNegative',
  slipPct: 'nonNegative',
  sizePct: 'sizePercent',
} as const satisfies Record<string, NumericDomain>;

export type NumericParamKey = keyof typeof NUMERIC_PARAM_DOMAINS;

/**
 * Whitelisted grid axes: the indicator and risk parameters that define the
 * HYPOTHESIS. Cost and sizing (`feePct`, `slipPct`, `sizePct`) are execution
 * model, never an axis — sweeping them would let discovery "win" by assuming
 * cheaper fills instead of by finding a better signal.
 */
export const DISCOVERY_AXIS_KEYS = [
  'fastMA',
  'slowMA',
  'emaPeriod',
  'rsiPeriod',
  'rsiBuy',
  'rsiSell',
  'macdFast',
  'macdSlow',
  'macdSignal',
  'bbPeriod',
  'bbMult',
  'slPct',
  'tpPct',
] as const;
export type DiscoveryAxisKey = (typeof DISCOVERY_AXIS_KEYS)[number];

/** The 12 signal ids the params pipeline actually supports; `stoch*` await a
 *  core STOCH indicator and are rejected here, not deep inside the engine. */
export const DISCOVERY_SUPPORTED_SIGNAL_IDS = [
  'maCrossUp',
  'maCrossDown',
  'emaCrossUp',
  'emaCrossDown',
  'priceAboveSlow',
  'priceBelowSlow',
  'rsiOversold',
  'rsiOverbought',
  'macdCrossUp',
  'macdCrossDown',
  'bbLowerTouch',
  'bbUpperTouch',
] as const;

const FILL_MODES: readonly FillMode[] = ['close', 'nextOpen'];
const DIRECTIONS: readonly Direction[] = ['long', 'short', 'both'];

/** Exact `ParamsStrategy` key set, in declaration order. Adding a strategy
 *  field is a contract change: it alters `strategy-v2` identity. */
const STRATEGY_KEYS = [
  'mode',
  'fastMA',
  'slowMA',
  'emaPeriod',
  'rsiPeriod',
  'rsiBuy',
  'rsiSell',
  'macdFast',
  'macdSlow',
  'macdSignal',
  'bbPeriod',
  'bbMult',
  'entrySig',
  'exitSig',
  'entryRules',
  'exitRules',
  'entryCode',
  'exitCode',
  'slPct',
  'tpPct',
  'feePct',
  'slipPct',
  'sizePct',
  'fillMode',
  'direction',
] as const;

const GATE_KEYS = [
  'minTrades',
  'minAvgTradeReturn',
  'rollingWindowBars',
  'minRollingPositiveRatio',
  'maxDrawdown',
  'maxMonthlyContribution',
  'maxSingleTradeContribution',
  'minRandomEntryPercentile',
] as const;

const SCORE_CAP_KEYS = [
  'cagr',
  'sortino',
  'calmar',
  'profitFactor',
  'consistencySigmaScale',
  'complexityUnits',
  'turnover',
  'dataMiningLog10',
] as const;

const SCORE_WEIGHT_KEYS = [
  'cagr',
  'sortino',
  'calmar',
  'regime',
  'profitFactor',
  'consistency',
  'complexity',
  'turnover',
  'dataMining',
] as const;

// ---------- resolved shapes ----------

export interface DiscoveryAxis {
  key: DiscoveryAxisKey;
  min: number;
  max: number;
  step: number;
}

export interface DiscoveryBase {
  /** Stable, lowercase, hyphenated id; unique within a run. */
  id: string;
  presetVersion: typeof DISCOVERY_PRESET_VERSION;
  strategy: ParamsStrategy;
  axes: DiscoveryAxis[];
}

export interface ResolvedConcurrency {
  /** What the config asked for; null means "use the machine default". */
  requested: number | null;
  resolved: number;
  logicalCores: number;
}

export interface ResolvedDiscoveryConfig {
  envelopeVersion: typeof DISCOVERY_CONFIG_VERSION;
  contracts: DiscoveryContractVersions;
  dataset: { id: number; contentHash: string };
  bases: DiscoveryBase[];
  embargo: { holdingAllowanceBars: number };
  execution: { startEquity: number };
  /** Resolved numeric benchmark costs, never a mutable source pointer. */
  benchmarkCosts: { feePct: number; slipPct: number };
  randomEntry: { runs: number };
  gateConfig: GateConfig;
  scoreConfig: ScoreConfig;
  rootSeed: number;
  caps: { candidates: number };
  concurrency: ResolvedConcurrency;
}

export interface ParseDiscoveryConfigOptions {
  /** Logical CPU count of the machine that will run the job (caller-supplied
   *  so this module stays pure). */
  logicalCores: number;
}

// ---------- strict readers ----------

function fail(message: string): never {
  throw new RangeError(message);
}

function requireObject(value: unknown, path: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function requireExactKeys(
  object: Record<string, unknown>,
  path: string,
  keys: readonly string[],
): void {
  const allowed = new Set<string>(keys);
  for (const key of keys) {
    if (!Object.prototype.hasOwnProperty.call(object, key)) {
      fail(`${path} is missing key "${key}"`);
    }
  }
  for (const key of Object.keys(object).sort()) {
    if (!allowed.has(key)) fail(`${path} has unknown key "${key}"`);
  }
}

function requireNumber(object: Record<string, unknown>, path: string, key: string): number {
  const value = object[key];
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    fail(`${path}.${key} must be a finite number`);
  }
  return value;
}

function requireString(object: Record<string, unknown>, path: string, key: string): string {
  const value = object[key];
  if (typeof value !== 'string') fail(`${path}.${key} must be a string`);
  return value;
}

function requireArray(object: Record<string, unknown>, path: string, key: string): unknown[] {
  const value = object[key];
  if (!Array.isArray(value)) fail(`${path}.${key} must be an array`);
  return value;
}

function requireLiteral<T extends string>(
  object: Record<string, unknown>,
  path: string,
  key: string,
  allowed: readonly T[],
): T {
  const value = requireString(object, path, key);
  if (!(allowed as readonly string[]).includes(value)) {
    fail(`${path}.${key} must be one of ${allowed.join(', ')}`);
  }
  return value as T;
}

function requireIntegerInRange(
  value: number,
  path: string,
  min: number,
  max: number,
): number {
  if (!Number.isSafeInteger(value) || value < min || value > max) {
    fail(`${path} must be an integer in [${min}, ${max}]`);
  }
  return value;
}

/** Domain check shared by base-preset fields and generated axis values, so an
 *  axis can never produce a value the base preset itself could not hold. */
export function checkNumericParam(key: NumericParamKey, value: number): string | null {
  if (!Number.isFinite(value)) return `${key} must be a finite number`;
  switch (NUMERIC_PARAM_DOMAINS[key]) {
    case 'period':
      return Number.isSafeInteger(value) && value >= 1
        ? null
        : `${key} must be an integer >= 1`;
    case 'level':
      return value >= 0 && value <= 100 ? null : `${key} must be in [0, 100]`;
    case 'positive':
      return value > 0 ? null : `${key} must be > 0`;
    case 'nonNegative':
      return value >= 0 ? null : `${key} must be >= 0`;
    case 'sizePercent':
      return value > 0 && value <= 100 ? null : `${key} must be in (0, 100]`;
  }
}

// ---------- strategy preset ----------

function parseStrategy(value: unknown, path: string): ParamsStrategy {
  const object = requireObject(value, path);
  requireExactKeys(object, path, STRATEGY_KEYS);

  // Resolution D2: params only. Blocks and AI DSL are later phases; code mode
  // is permanently excluded from discovery. Rejecting here is the recorded
  // contract that lets the pure Rust engine skip those paths entirely.
  const mode = requireString(object, path, 'mode');
  if (mode !== 'params') {
    fail(`${path}.mode must be "params" (discovery v1 enumerates params-mode candidates only)`);
  }

  const numeric = {} as Record<NumericParamKey, number>;
  for (const key of Object.keys(NUMERIC_PARAM_DOMAINS) as NumericParamKey[]) {
    const parsed = requireNumber(object, path, key);
    const problem = checkNumericParam(key, parsed);
    if (problem) fail(`${path}.${problem}`);
    numeric[key] = parsed;
  }

  const entrySig = requireLiteral(object, path, 'entrySig', DISCOVERY_SUPPORTED_SIGNAL_IDS);
  const exitSig = requireLiteral(object, path, 'exitSig', DISCOVERY_SUPPORTED_SIGNAL_IDS);
  const fillMode = requireLiteral(object, path, 'fillMode', FILL_MODES);
  const direction = requireLiteral(object, path, 'direction', DIRECTIONS);

  // Dormant in params mode but part of `strategy-v2` identity, so they must be
  // present and well-typed. Their CONTENTS are never interpreted here.
  const entryRules = requireArray(object, path, 'entryRules');
  const exitRules = requireArray(object, path, 'exitRules');
  const entryCode = requireString(object, path, 'entryCode');
  const exitCode = requireString(object, path, 'exitCode');

  return {
    mode: 'params',
    fastMA: numeric.fastMA,
    slowMA: numeric.slowMA,
    emaPeriod: numeric.emaPeriod,
    rsiPeriod: numeric.rsiPeriod,
    rsiBuy: numeric.rsiBuy,
    rsiSell: numeric.rsiSell,
    macdFast: numeric.macdFast,
    macdSlow: numeric.macdSlow,
    macdSignal: numeric.macdSignal,
    bbPeriod: numeric.bbPeriod,
    bbMult: numeric.bbMult,
    entrySig: entrySig as SignalId,
    exitSig: exitSig as SignalId,
    entryRules: entryRules as ParamsStrategy['entryRules'],
    exitRules: exitRules as ParamsStrategy['exitRules'],
    entryCode,
    exitCode,
    slPct: numeric.slPct,
    tpPct: numeric.tpPct,
    feePct: numeric.feePct,
    slipPct: numeric.slipPct,
    sizePct: numeric.sizePct,
    fillMode,
    direction,
  };
}

// ---------- axes ----------

const BASE_ID_PATTERN = /^[a-z0-9][a-z0-9-]*$/;

function parseAxis(value: unknown, path: string): DiscoveryAxis {
  const object = requireObject(value, path);
  requireExactKeys(object, path, ['key', 'min', 'max', 'step']);
  const key = requireLiteral(object, path, 'key', DISCOVERY_AXIS_KEYS);
  const min = requireNumber(object, path, 'min');
  const max = requireNumber(object, path, 'max');
  const step = requireNumber(object, path, 'step');
  if (step <= 0) fail(`${path}.step must be > 0`);
  if (max < min) fail(`${path}.max must be >= min`);
  if (NUMERIC_PARAM_DOMAINS[key] === 'period') {
    for (const [name, bound] of [['min', min], ['max', max], ['step', step]] as const) {
      if (!Number.isSafeInteger(bound)) {
        fail(`${path}.${name} must be an integer for the integer axis "${key}"`);
      }
    }
  }
  return { key, min, max, step };
}

/**
 * Inclusive axis values `min + i*step` while `<= max`.
 *
 * Values are computed by MULTIPLICATION, never by accumulating `+= step`:
 * accumulation drifts differently once a float step is involved, and the Rust
 * port must produce bit-identical values.
 */
export function axisValues(axis: DiscoveryAxis): number[] {
  const values: number[] = [];
  for (let i = 0; ; i++) {
    const value = axis.min + i * axis.step;
    if (!(value <= axis.max)) break;
    if (values.length >= DISCOVERY_MAX_AXIS_VALUES) {
      throw new RangeError(
        `axis "${axis.key}" produces more than ${DISCOVERY_MAX_AXIS_VALUES} values`,
      );
    }
    values.push(value);
  }
  if (values.length === 0) {
    throw new RangeError(`axis "${axis.key}" produces no values`);
  }
  return values;
}

function parseBase(value: unknown, path: string): DiscoveryBase {
  const object = requireObject(value, path);
  requireExactKeys(object, path, ['id', 'presetVersion', 'strategy', 'axes']);
  const id = requireString(object, path, 'id');
  if (!BASE_ID_PATTERN.test(id)) {
    fail(`${path}.id must match ${BASE_ID_PATTERN.source}`);
  }
  const presetVersion = requireString(object, path, 'presetVersion');
  if (presetVersion !== DISCOVERY_PRESET_VERSION) {
    fail(`${path}.presetVersion must be "${DISCOVERY_PRESET_VERSION}"`);
  }
  const strategy = parseStrategy(object.strategy, `${path}.strategy`);

  const rawAxes = requireArray(object, path, 'axes');
  const axes: DiscoveryAxis[] = [];
  const seen = new Set<DiscoveryAxisKey>();
  for (let index = 0; index < rawAxes.length; index++) {
    const axis = parseAxis(rawAxes[index], `${path}.axes[${index}]`);
    if (seen.has(axis.key)) fail(`${path}.axes[${index}] repeats axis key "${axis.key}"`);
    seen.add(axis.key);
    // Every generated value must satisfy the same domain as the base field.
    for (const generated of axisValues(axis)) {
      const problem = checkNumericParam(axis.key, generated);
      if (problem) fail(`${path}.axes[${index}] generates an invalid value: ${problem}`);
    }
    axes.push(axis);
  }
  return { id, presetVersion: DISCOVERY_PRESET_VERSION, strategy, axes };
}

// ---------- gate / score configs ----------

/** Mirrors `gate.ts`'s own validator messages exactly so a config rejected at
 *  admission is rejected identically at judgment time. */
function parseGateConfig(value: unknown, path: string): GateConfig {
  const object = requireObject(value, path);
  requireExactKeys(object, path, GATE_KEYS);
  const read = (key: (typeof GATE_KEYS)[number]): number => requireNumber(object, path, key);

  const config: GateConfig = {
    minTrades: read('minTrades'),
    minAvgTradeReturn: read('minAvgTradeReturn'),
    rollingWindowBars: read('rollingWindowBars'),
    minRollingPositiveRatio: read('minRollingPositiveRatio'),
    maxDrawdown: read('maxDrawdown'),
    maxMonthlyContribution: read('maxMonthlyContribution'),
    maxSingleTradeContribution: read('maxSingleTradeContribution'),
    minRandomEntryPercentile: read('minRandomEntryPercentile'),
  };

  if (!Number.isSafeInteger(config.minTrades) || config.minTrades < 1) {
    fail('minTrades must be a positive integer');
  }
  if (!Number.isSafeInteger(config.rollingWindowBars) || config.rollingWindowBars < 1) {
    fail('rollingWindowBars must be a positive integer');
  }
  for (const key of [
    'minRollingPositiveRatio',
    'maxDrawdown',
    'maxMonthlyContribution',
    'maxSingleTradeContribution',
  ] as const) {
    const fraction = config[key];
    if (fraction <= 0 || fraction > 1) fail(`${key} must be a fraction in (0, 1]`);
  }
  if (config.minRandomEntryPercentile < 0 || config.minRandomEntryPercentile > 100) {
    fail('minRandomEntryPercentile must be in [0, 100]');
  }
  return config;
}

/** Mirrors `score.ts`'s own validator messages exactly. */
function parseScoreConfig(value: unknown, path: string): ScoreConfig {
  const object = requireObject(value, path);
  requireExactKeys(object, path, ['caps', 'weights']);
  const capsObject = requireObject(object.caps, `${path}.caps`);
  requireExactKeys(capsObject, `${path}.caps`, SCORE_CAP_KEYS);
  const weightsObject = requireObject(object.weights, `${path}.weights`);
  requireExactKeys(weightsObject, `${path}.weights`, SCORE_WEIGHT_KEYS);

  const caps = {} as ScoreConfig['caps'];
  for (const key of SCORE_CAP_KEYS) {
    const parsed = requireNumber(capsObject, `${path}.caps`, key);
    if (parsed <= 0) fail(`cap ${key} must be finite and > 0`);
    caps[key] = parsed;
  }
  if (caps.profitFactor <= 1) {
    fail('cap profitFactor must be > 1 (1 is the break-even floor)');
  }

  const weights = {} as ScoreConfig['weights'];
  for (const key of SCORE_WEIGHT_KEYS) {
    const parsed = requireNumber(weightsObject, `${path}.weights`, key);
    if (parsed < 0) fail(`weight ${key} must be finite and >= 0`);
    weights[key] = parsed;
  }
  if (weights.regime !== 0) {
    fail('regime weight must stay 0 until REGIME-001 implements the regime classifier');
  }
  return { caps, weights };
}

// ---------- concurrency ----------

/** Resolution D4: concurrency affects performance only. Default is
 *  `max(1, logicalCores - 1)`; an override must fit `1..=logicalCores`. */
export function resolveConcurrency(requested: number | null, logicalCores: number): number {
  if (!Number.isSafeInteger(logicalCores) || logicalCores < 1) {
    fail('logicalCores must be an integer >= 1');
  }
  if (requested === null) return Math.max(1, logicalCores - 1);
  if (!Number.isSafeInteger(requested) || requested < 1 || requested > logicalCores) {
    fail(`maxConcurrency must be an integer in [1, ${logicalCores}]`);
  }
  return requested;
}

// ---------- envelope ----------

const ENVELOPE_KEYS = [
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

const CONTRACT_KEYS = Object.keys(DISCOVERY_CONTRACT_VERSIONS).sort() as (keyof DiscoveryContractVersions)[];

/**
 * Parse and resolve one `discovery-config-v1` envelope. Throws `RangeError`
 * with a path-qualified message on the first problem; never returns a
 * partially-validated config.
 */
export function parseDiscoveryConfig(
  value: unknown,
  options: ParseDiscoveryConfigOptions,
): ResolvedDiscoveryConfig {
  const path = 'discoveryConfig';
  const object = requireObject(value, path);
  requireExactKeys(object, path, ENVELOPE_KEYS);

  const envelopeVersion = requireString(object, path, 'envelopeVersion');
  if (envelopeVersion !== DISCOVERY_CONFIG_VERSION) {
    fail(`${path}.envelopeVersion must be "${DISCOVERY_CONFIG_VERSION}"`);
  }

  const contractsObject = requireObject(object.contracts, `${path}.contracts`);
  requireExactKeys(contractsObject, `${path}.contracts`, CONTRACT_KEYS);
  for (const key of CONTRACT_KEYS) {
    const recorded = requireString(contractsObject, `${path}.contracts`, key);
    if (recorded !== DISCOVERY_CONTRACT_VERSIONS[key]) {
      fail(
        `${path}.contracts.${key} must be "${DISCOVERY_CONTRACT_VERSIONS[key]}" (recorded "${recorded}")`,
      );
    }
  }

  const datasetObject = requireObject(object.dataset, `${path}.dataset`);
  requireExactKeys(datasetObject, `${path}.dataset`, ['id', 'contentHash']);
  const datasetId = requireIntegerInRange(
    requireNumber(datasetObject, `${path}.dataset`, 'id'),
    `${path}.dataset.id`,
    1,
    Number.MAX_SAFE_INTEGER,
  );
  const contentHash = requireString(datasetObject, `${path}.dataset`, 'contentHash');
  if (!contentHash.startsWith(`${DATASET_HASH_VERSION}:`)) {
    fail(`${path}.dataset.contentHash must be a durable ${DATASET_HASH_VERSION} identity`);
  }

  const rawBases = requireArray(object, path, 'bases');
  if (rawBases.length === 0) fail(`${path}.bases must contain at least one base preset`);
  const bases: DiscoveryBase[] = [];
  const baseIds = new Set<string>();
  for (let index = 0; index < rawBases.length; index++) {
    const base = parseBase(rawBases[index], `${path}.bases[${index}]`);
    if (baseIds.has(base.id)) fail(`${path}.bases[${index}] repeats base id "${base.id}"`);
    baseIds.add(base.id);
    bases.push(base);
  }

  const embargoObject = requireObject(object.embargo, `${path}.embargo`);
  requireExactKeys(embargoObject, `${path}.embargo`, ['holdingAllowanceBars']);
  const holdingAllowanceBars = requireIntegerInRange(
    requireNumber(embargoObject, `${path}.embargo`, 'holdingAllowanceBars'),
    `${path}.embargo.holdingAllowanceBars`,
    0,
    Number.MAX_SAFE_INTEGER,
  );

  const executionObject = requireObject(object.execution, `${path}.execution`);
  requireExactKeys(executionObject, `${path}.execution`, ['startEquity']);
  const startEquity = requireNumber(executionObject, `${path}.execution`, 'startEquity');
  if (startEquity <= 0) fail(`${path}.execution.startEquity must be > 0`);

  const costsObject = requireObject(object.benchmarkCosts, `${path}.benchmarkCosts`);
  requireExactKeys(costsObject, `${path}.benchmarkCosts`, ['feePct', 'slipPct']);
  const benchmarkCosts = {
    feePct: requireNumber(costsObject, `${path}.benchmarkCosts`, 'feePct'),
    slipPct: requireNumber(costsObject, `${path}.benchmarkCosts`, 'slipPct'),
  };
  for (const key of ['feePct', 'slipPct'] as const) {
    const problem = checkNumericParam(key, benchmarkCosts[key]);
    if (problem) fail(`${path}.benchmarkCosts.${problem}`);
  }
  // Benchmarks inherit the candidate's costs (docs/benchmark-suite-contract.md).
  // Recording a resolved value that contradicts a base preset would make the
  // fairness convention unauditable, so a mismatch fails closed.
  for (let index = 0; index < bases.length; index++) {
    const strategy = bases[index].strategy;
    if (strategy.feePct !== benchmarkCosts.feePct || strategy.slipPct !== benchmarkCosts.slipPct) {
      fail(
        `${path}.benchmarkCosts must match bases[${index}] costs ` +
          `(feePct ${strategy.feePct}, slipPct ${strategy.slipPct})`,
      );
    }
  }

  const randomEntryObject = requireObject(object.randomEntry, `${path}.randomEntry`);
  requireExactKeys(randomEntryObject, `${path}.randomEntry`, ['runs']);
  const runs = requireIntegerInRange(
    requireNumber(randomEntryObject, `${path}.randomEntry`, 'runs'),
    `${path}.randomEntry.runs`,
    1,
    MAX_RANDOM_ENTRY_RUNS,
  );

  const gateConfig = parseGateConfig(object.gateConfig, `${path}.gateConfig`);
  const scoreConfig = parseScoreConfig(object.scoreConfig, `${path}.scoreConfig`);

  const rootSeed = requireIntegerInRange(
    requireNumber(object, path, 'rootSeed'),
    `${path}.rootSeed`,
    0,
    MAX_U32,
  );

  const capsObject = requireObject(object.caps, `${path}.caps`);
  requireExactKeys(capsObject, `${path}.caps`, ['candidates']);
  const candidates = requireIntegerInRange(
    requireNumber(capsObject, `${path}.caps`, 'candidates'),
    `${path}.caps.candidates`,
    1,
    DISCOVERY_HARD_CANDIDATE_CAP,
  );

  const requestedConcurrency = object.maxConcurrency;
  if (requestedConcurrency !== null && typeof requestedConcurrency !== 'number') {
    fail(`${path}.maxConcurrency must be a number or null`);
  }
  if (typeof requestedConcurrency === 'number' && !Number.isFinite(requestedConcurrency)) {
    fail(`${path}.maxConcurrency must be a finite number or null`);
  }
  const resolved = resolveConcurrency(
    requestedConcurrency === null ? null : (requestedConcurrency as number),
    options.logicalCores,
  );

  return {
    envelopeVersion: DISCOVERY_CONFIG_VERSION,
    contracts: { ...DISCOVERY_CONTRACT_VERSIONS },
    dataset: { id: datasetId, contentHash },
    bases,
    embargo: { holdingAllowanceBars },
    execution: { startEquity },
    benchmarkCosts,
    randomEntry: { runs },
    gateConfig,
    scoreConfig,
    rootSeed,
    caps: { candidates },
    concurrency: {
      requested: requestedConcurrency === null ? null : (requestedConcurrency as number),
      resolved,
      logicalCores: options.logicalCores,
    },
  };
}

/** Convenience defaults for callers building a config; the parser still owns
 *  every invariant, so these are a starting point and not a bypass. */
export const DISCOVERY_DEFAULTS = {
  gateConfig: DEFAULT_GATE_CONFIG,
  scoreConfig: DEFAULT_SCORE_CONFIG,
  candidateCap: DISCOVERY_DEFAULT_CANDIDATE_CAP,
} as const;
