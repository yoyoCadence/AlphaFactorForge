// TypeScript-reference builder for the RS-CORE-005 Gate + Score parity
// fixture. Pure and deterministic; scripts/generate-gate-score-fixtures.ts
// owns file IO. Discovery candidates are params-mode only per the PR #66
// Resolution: blocks/code and the expression interpreter stay out of this
// fixture and out of the Rust discovery core.

import type { BacktestResult } from '../core/backtest';
import type { ClosedTrade, Metrics } from '../core/metrics';
import {
  DETERMINISTIC_BENCHMARK_IDS,
  type BenchmarkRun,
  type DeterministicBenchmarkId,
} from '../services/benchmarks';
import {
  DEFAULT_GATE_CONFIG,
  GATE_CONTRACT_VERSION,
  evaluateGate,
  type GateConfig,
  type GateCriterionId,
  type GateVerdict,
} from '../services/gate';
import { nonFiniteStatus, type NonFiniteStatus } from '../services/nonFinite';
import {
  SCORE_FORMULA_VERSION,
  complexityUnits,
  scoreCandidate,
  type ScoreBreakdown,
  type ScoreCaps,
  type ScoreWeights,
} from '../services/score';
import { defaultStrategy, type ParamsStrategy, type SignalId } from '../services/strategy';
import { encodeGateVerdict } from '../services/validationRecord';
import type { RandomEntryBenchmark } from '../services/randomEntry';
import { FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';

export const PARITY_FIXTURE_SCHEMA_VERSION = 'rs-core-parity-fixture-v1';
export const GATE_SCORE_PARITY_FIXTURE_VERSION = 'gate-score-parity-v1';
export const METRICS_CONTRACT_VERSION = 'metrics-v1';
export const SPECIAL_INPUT_NUMBER_ENCODING = 'explicit-numeric-status-v1';
export const EXPECTED_FLOAT_ENCODING = 'decimal-significant-15-v1';

export type FixtureNumericTag = NonFiniteStatus | 'negative_zero';
export type FixtureNumber = number | FixtureNumericTag;

const MAX_SAFE = Number.MAX_SAFE_INTEGER;
const T0 = Date.UTC(2024, 0, 1);

const GATE_CRITERION_ORDER: GateCriterionId[] = [
  'minTrades',
  'avgTradeReturn',
  'rollingConsistency',
  'maxDrawdown',
  'monthlyConcentration',
  'tradeConcentration',
  'benchmarkWins',
  'randomEntryPercentile',
];

const SCORE_COMPONENT_ORDER = [
  'cagr',
  'sortino',
  'calmar',
  'regime',
  'profitFactor',
  'consistency',
] as const;

const SCORE_PENALTY_ORDER = ['complexity', 'turnover', 'dataMining'] as const;

function decodeNumber(value: FixtureNumber): number {
  if (typeof value === 'number') return value;
  switch (value) {
    case 'positive_infinity':
      return Infinity;
    case 'negative_infinity':
      return -Infinity;
    case 'nan':
      return NaN;
    case 'negative_zero':
      return -0;
  }
}

function encodeNumber(value: number): FixtureNumber {
  if (Object.is(value, -0)) return 'negative_zero';
  return nonFiniteStatus(value) ?? value;
}

/** Canonicalize only expected finite non-integer float leaves. This keeps the
 * committed JSON blob stable across Node/OS libm tails (sqrt/log10) while
 * remaining far tighter than the reviewed Rust comparison tolerance. Inputs
 * and exact integer leaves are never rounded. */
function canonicalizeExpected<T>(value: T, forceTolerantFloat = false): T {
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new Error('expected parity output must contain only finite numbers or null');
    }
    if (Object.is(value, -0)) return 0 as T;
    if (Number.isInteger(value) && !forceTolerantFloat) return value;
    return Number(value.toPrecision(15)) as T;
  }
  if (Array.isArray(value)) {
    return value.map((item) => canonicalizeExpected(item, forceTolerantFloat)) as T;
  }
  if (value !== null && typeof value === 'object') {
    const out: Record<string, unknown> = {};
    const consistencyEntry = 'id' in value && value.id === 'consistency';
    for (const [key, child] of Object.entries(value)) {
      const childIsTolerant =
        forceTolerantFloat || (consistencyEntry && (key === 'raw' || key === 'evidence'));
      out[key] = canonicalizeExpected(child, childIsTolerant);
    }
    return out as T;
  }
  return value;
}

function zeroMetrics(overrides: Partial<Metrics> = {}): Metrics {
  return {
    netReturn: 0,
    cagr: 0,
    maxDrawdown: 0,
    sharpe: 0,
    sortino: 0,
    calmar: 0,
    winRate: 0,
    tradeCount: 0,
    profitFactor: 0,
    avgTradeReturn: 0,
    medianTradeReturn: 0,
    avgHoldingBars: 0,
    exposure: 0,
    turnover: 0,
    largestWin: 0,
    largestLoss: 0,
    consecutiveLosses: 0,
    monthlyReturns: {},
    ...overrides,
  };
}

function placeholderTrade(index: number): ClosedTrade {
  return {
    entryTime: T0 + index * 2 - 1,
    exitTime: T0 + index * 2,
    side: 'LONG',
    entryPrice: 100,
    exitPrice: 101,
    pnl: 1,
    pnlPct: 0.01,
    bars: 1,
  };
}

// ---------- compact Gate fixture input ----------

interface GateCandidateInput {
  metrics: {
    netReturn: FixtureNumber;
    tradeCount: FixtureNumber;
    avgTradeReturn: FixtureNumber;
    maxDrawdown: FixtureNumber;
  };
  /** Gate reads only the equity values; deterministic times are synthesized. */
  equity: FixtureNumber[];
  /** Gate reads only cost-inclusive pnl and UTC exit month. */
  trades: { pnl: FixtureNumber; exitTime: FixtureNumber }[];
}

type EncodedGateConfig = { [K in keyof GateConfig]?: FixtureNumber };

interface GateCaseInput {
  candidate: GateCandidateInput;
  benchmarks: { id: DeterministicBenchmarkId; netReturn: FixtureNumber }[];
  randomEntryPercentile: FixtureNumber;
  config?: EncodedGateConfig;
}

function decodeGateConfig(config: EncodedGateConfig | undefined): Partial<GateConfig> | undefined {
  if (!config) return undefined;
  const decoded: Partial<GateConfig> = {};
  for (const [key, value] of Object.entries(config) as [keyof GateConfig, FixtureNumber][]) {
    decoded[key] = decodeNumber(value);
  }
  return decoded;
}

function gateArgs(input: GateCaseInput): Parameters<typeof evaluateGate>[0] {
  const trades: ClosedTrade[] = input.candidate.trades.map((trade, index) => ({
    ...placeholderTrade(index),
    pnl: decodeNumber(trade.pnl),
    exitTime: decodeNumber(trade.exitTime),
  }));
  const candidateResult: BacktestResult = {
    trades,
    equity: input.candidate.equity.map((equity, time) => ({
      time,
      equity: decodeNumber(equity),
    })),
    metrics: zeroMetrics({
      netReturn: decodeNumber(input.candidate.metrics.netReturn),
      tradeCount: decodeNumber(input.candidate.metrics.tradeCount),
      avgTradeReturn: decodeNumber(input.candidate.metrics.avgTradeReturn),
      maxDrawdown: decodeNumber(input.candidate.metrics.maxDrawdown),
    }),
  };
  const benchmarks: BenchmarkRun[] = input.benchmarks.map((benchmark) => ({
    id: benchmark.id,
    strat: null,
    result: {
      trades: [],
      equity: [],
      metrics: zeroMetrics({ netReturn: decodeNumber(benchmark.netReturn) }),
    },
  }));
  const randomEntry: RandomEntryBenchmark = {
    runs: 1,
    seed: 0,
    netReturns: [],
    candidateNetReturn: candidateResult.metrics.netReturn,
    candidatePercentile: decodeNumber(input.randomEntryPercentile),
  };
  return {
    candidateResult,
    benchmarks,
    randomEntry,
    config: decodeGateConfig(input.config),
  };
}

function risingEquity(count = 100): FixtureNumber[] {
  return Array.from({ length: count }, (_, index) => 1_000 + index);
}

function spreadTrades(count = 36, pnl: FixtureNumber = 10): GateCandidateInput['trades'] {
  return Array.from({ length: count }, (_, index) => ({
    pnl,
    exitTime: Date.UTC(2024, index % 6, 10) + index,
  }));
}

function benchmarkReturns(netReturn: FixtureNumber = 0.1): GateCaseInput['benchmarks'] {
  return DETERMINISTIC_BENCHMARK_IDS.map((id) => ({ id, netReturn }));
}

function baseGateInput(): GateCaseInput {
  return {
    candidate: {
      metrics: {
        netReturn: 0.5,
        tradeCount: 36,
        avgTradeReturn: 0.01,
        maxDrawdown: 0.1,
      },
      equity: risingEquity(),
      trades: spreadTrades(),
    },
    benchmarks: benchmarkReturns(),
    randomEntryPercentile: 97,
  };
}

interface GateCaseDefinition {
  id: string;
  input: GateCaseInput;
  expectedFailed: GateCriterionId[];
}

function isolatedGateFailure(
  id: string,
  criterion: GateCriterionId,
  mutate: (input: GateCaseInput) => void,
): GateCaseDefinition {
  const input = baseGateInput();
  mutate(input);
  return { id, input, expectedFailed: [criterion] };
}

function buildGateCaseDefinitions(): GateCaseDefinition[] {
  const fullConfigBoundary: GateCaseInput = {
    candidate: {
      metrics: {
        netReturn: 0.5,
        tradeCount: 4,
        avgTradeReturn: 0.006,
        maxDrawdown: 0.2,
      },
      // Four one-bar windows: positive, negative, positive, negative => 0.5.
      equity: [100, 101, 100, 101, 100],
      // Total 100; largest month 50%, largest trade 25%.
      trades: [
        { pnl: 25, exitTime: Date.UTC(2024, 0, 10) },
        { pnl: 25, exitTime: Date.UTC(2024, 0, 20) },
        { pnl: 25, exitTime: Date.UTC(2024, 1, 10) },
        { pnl: 25, exitTime: Date.UTC(2024, 1, 20) },
      ],
    },
    benchmarks: benchmarkReturns(0.49),
    randomEntryPercentile: 90,
    config: {
      minTrades: 4,
      minAvgTradeReturn: 0.005,
      rollingWindowBars: 1,
      minRollingPositiveRatio: 0.5,
      maxDrawdown: 0.2,
      maxMonthlyContribution: 0.5,
      maxSingleTradeContribution: 0.25,
      minRandomEntryPercentile: 90,
    },
  };

  const partialConfig = baseGateInput();
  partialConfig.candidate.trades = spreadTrades(6);
  partialConfig.candidate.metrics.tradeCount = 6;
  partialConfig.config = { minTrades: 5 };

  const utcBoundary: GateCaseInput = {
    candidate: {
      metrics: {
        netReturn: 0.5,
        tradeCount: 2,
        avgTradeReturn: 0.01,
        maxDrawdown: 0.1,
      },
      equity: risingEquity(4),
      trades: [
        { pnl: 10, exitTime: Date.UTC(2024, 0, 31, 23, 59, 59, 999) },
        { pnl: 10, exitTime: Date.UTC(2024, 1, 1, 0, 0, 0, 0) },
      ],
    },
    benchmarks: benchmarkReturns(),
    randomEntryPercentile: 97,
    config: {
      minTrades: 2,
      rollingWindowBars: 1,
      maxMonthlyContribution: 0.5,
      maxSingleTradeContribution: 0.5,
    },
  };

  const shortEquity = baseGateInput();
  shortEquity.candidate.equity = risingEquity(DEFAULT_GATE_CONFIG.rollingWindowBars);

  const nonPositive = baseGateInput();
  nonPositive.candidate.trades = spreadTrades(36, -10);

  const invalidDateEvidence = baseGateInput();
  invalidDateEvidence.candidate.trades[0].exitTime = MAX_SAFE;

  const finiteRatioOverflow = baseGateInput();
  finiteRatioOverflow.candidate.trades = [
    { pnl: Number.MAX_VALUE, exitTime: Date.UTC(2024, 0, 10) },
    { pnl: -Number.MAX_VALUE, exitTime: Date.UTC(2024, 1, 10) },
    { pnl: Number.MIN_VALUE, exitTime: Date.UTC(2024, 2, 10) },
  ];

  const nonFiniteStatuses = baseGateInput();
  nonFiniteStatuses.candidate.metrics.avgTradeReturn = 'positive_infinity';
  nonFiniteStatuses.candidate.metrics.maxDrawdown = 'negative_infinity';
  nonFiniteStatuses.randomEntryPercentile = 'nan';

  const nonFiniteDerivedEvidence = baseGateInput();
  nonFiniteDerivedEvidence.candidate.equity[50] = 'nan';
  nonFiniteDerivedEvidence.candidate.trades[0].pnl = 'positive_infinity';
  nonFiniteDerivedEvidence.candidate.trades[1].exitTime = 'positive_infinity';
  nonFiniteDerivedEvidence.benchmarks[2].netReturn = 'negative_infinity';

  return [
    { id: 'gate-default-pass', input: baseGateInput(), expectedFailed: [] },
    { id: 'gate-full-config-boundary-pass', input: fullConfigBoundary, expectedFailed: [] },
    { id: 'gate-partial-config-pass', input: partialConfig, expectedFailed: [] },
    isolatedGateFailure('gate-min-trades-fail', 'minTrades', (input) => {
      input.candidate.metrics.tradeCount = 29;
      input.candidate.trades = spreadTrades(29);
    }),
    isolatedGateFailure('gate-unsafe-trade-count-fails-closed', 'minTrades', (input) => {
      input.candidate.metrics.tradeCount = MAX_SAFE + 1;
    }),
    isolatedGateFailure('gate-nonfinite-trade-count-fails-closed', 'minTrades', (input) => {
      input.candidate.metrics.tradeCount = 'nan';
    }),
    isolatedGateFailure('gate-fractional-trade-count-fails-closed', 'minTrades', (input) => {
      input.candidate.metrics.tradeCount = 36.5;
    }),
    isolatedGateFailure('gate-negative-trade-count-fails-closed', 'minTrades', (input) => {
      input.candidate.metrics.tradeCount = -1;
    }),
    isolatedGateFailure('gate-avg-return-strict-tie', 'avgTradeReturn', (input) => {
      input.candidate.metrics.avgTradeReturn = 0;
    }),
    isolatedGateFailure('gate-rolling-consistency-fail', 'rollingConsistency', (input) => {
      input.candidate.equity = [...risingEquity()].reverse();
    }),
    isolatedGateFailure('gate-max-drawdown-fail', 'maxDrawdown', (input) => {
      input.candidate.metrics.maxDrawdown = 0.36;
    }),
    isolatedGateFailure('gate-monthly-concentration-fail', 'monthlyConcentration', (input) => {
      for (const [index, trade] of input.candidate.trades.entries()) {
        trade.exitTime = Date.UTC(2024, 0, 10) + index;
      }
    }),
    isolatedGateFailure('gate-trade-concentration-fail', 'tradeConcentration', (input) => {
      input.candidate.trades.push({ pnl: 150, exitTime: Date.UTC(2024, 6, 20) });
      input.candidate.metrics.tradeCount = input.candidate.trades.length;
    }),
    isolatedGateFailure('gate-benchmark-strict-tie', 'benchmarkWins', (input) => {
      input.benchmarks[1].netReturn = 0.5;
    }),
    isolatedGateFailure('gate-random-percentile-fail', 'randomEntryPercentile', (input) => {
      input.randomEntryPercentile = 94.999;
    }),
    {
      id: 'gate-short-equity-fails-closed',
      input: shortEquity,
      expectedFailed: ['rollingConsistency'],
    },
    {
      id: 'gate-nonpositive-profit-fails-closed',
      input: nonPositive,
      expectedFailed: ['monthlyConcentration', 'tradeConcentration'],
    },
    { id: 'gate-utc-month-boundary-pass', input: utcBoundary, expectedFailed: [] },
    {
      id: 'gate-invalid-date-evidence-fails-closed',
      input: invalidDateEvidence,
      expectedFailed: ['monthlyConcentration'],
    },
    {
      id: 'gate-finite-ratio-overflow-fails-closed',
      input: finiteRatioOverflow,
      expectedFailed: ['monthlyConcentration', 'tradeConcentration'],
    },
    {
      id: 'gate-nonfinite-statuses-fail-closed',
      input: nonFiniteStatuses,
      expectedFailed: ['avgTradeReturn', 'maxDrawdown', 'randomEntryPercentile'],
    },
    {
      id: 'gate-nonfinite-derived-evidence-fails-closed',
      input: nonFiniteDerivedEvidence,
      expectedFailed: ['rollingConsistency', 'monthlyConcentration', 'tradeConcentration', 'benchmarkWins'],
    },
  ];
}

// ---------- params-only complexity and Score fixture input ----------

export interface ParamsOnlyScoreStrategy {
  entrySig: SignalId;
  exitSig: SignalId;
  slPct: FixtureNumber;
  tpPct: FixtureNumber;
}

function paramsStrategy(
  overrides: Partial<ParamsOnlyScoreStrategy> = {},
): ParamsOnlyScoreStrategy {
  const base = defaultStrategy();
  return {
    entrySig: base.entrySig,
    exitSig: base.exitSig,
    slPct: encodeNumber(base.slPct),
    tpPct: encodeNumber(base.tpPct),
    ...overrides,
  };
}

function toStrategy(input: ParamsOnlyScoreStrategy): ParamsStrategy {
  return {
    ...defaultStrategy(),
    mode: 'params',
    entrySig: input.entrySig,
    exitSig: input.exitSig,
    slPct: decodeNumber(input.slPct),
    tpPct: decodeNumber(input.tpPct),
  };
}

interface ScoreMetricsInput {
  cagr: FixtureNumber;
  sortino: FixtureNumber;
  calmar: FixtureNumber;
  profitFactor: FixtureNumber;
  turnover: FixtureNumber;
  monthlyReturns: Record<string, FixtureNumber>;
}

type EncodedScoreCaps = { [K in keyof ScoreCaps]?: FixtureNumber };
type EncodedScoreWeights = { [K in keyof ScoreWeights]?: FixtureNumber };

interface EncodedScoreConfig {
  caps?: EncodedScoreCaps;
  weights?: EncodedScoreWeights;
}

interface ScoreCaseInput {
  validation: {
    metrics: ScoreMetricsInput;
    closedTradeCount: number;
    totalBars: number;
  };
  strategy: ParamsOnlyScoreStrategy;
  testedCombinations: FixtureNumber;
  config?: EncodedScoreConfig;
}

function decodeRecord<T extends object>(record: T): { [K in keyof T]: number } {
  const decoded = {} as { [K in keyof T]: number };
  for (const [key, value] of Object.entries(record) as [keyof T, FixtureNumber][]) {
    decoded[key] = decodeNumber(value);
  }
  return decoded;
}

function decodeScoreConfig(
  config: EncodedScoreConfig | undefined,
): Parameters<typeof scoreCandidate>[0]['config'] {
  if (!config) return undefined;
  return {
    ...(config.caps ? { caps: decodeRecord(config.caps) } : {}),
    ...(config.weights ? { weights: decodeRecord(config.weights) } : {}),
  };
}

function scoreArgs(input: ScoreCaseInput): Parameters<typeof scoreCandidate>[0] {
  const metricsInput = input.validation.metrics;
  const trades = Array.from(
    { length: input.validation.closedTradeCount },
    (_, index) => placeholderTrade(index),
  );
  const equity = Array.from({ length: input.validation.totalBars }, (_, index) => ({
    time: T0 + index,
    equity: 1_000,
  }));
  const monthlyReturns = Object.fromEntries(
    Object.entries(metricsInput.monthlyReturns).map(([key, value]) => [key, decodeNumber(value)]),
  );
  return {
    validationRun: {
      validation: {
        trades,
        equity,
        metrics: zeroMetrics({
          cagr: decodeNumber(metricsInput.cagr),
          sortino: decodeNumber(metricsInput.sortino),
          calmar: decodeNumber(metricsInput.calmar),
          profitFactor: decodeNumber(metricsInput.profitFactor),
          turnover: decodeNumber(metricsInput.turnover),
          monthlyReturns,
        }),
      },
    },
    strat: toStrategy(input.strategy),
    testedCombinations: decodeNumber(input.testedCombinations),
    config: decodeScoreConfig(input.config),
  };
}

function baseScoreInput(): ScoreCaseInput {
  return {
    validation: {
      metrics: {
        cagr: 0.5,
        sortino: 2.5,
        calmar: 10,
        profitFactor: 2,
        turnover: 0.05,
        monthlyReturns: {
          '2024-01': 0.02,
          '2024-02': 0.02,
          '2024-03': 0.02,
        },
      },
      closedTradeCount: 5,
      totalBars: 100,
    },
    strategy: paramsStrategy(),
    testedCombinations: 100,
  };
}

function buildComplexityDefinitions() {
  return [
    {
      id: 'complexity-ma-family',
      input: paramsStrategy({ entrySig: 'maCrossUp', exitSig: 'maCrossDown' }),
      expectedUnits: 8,
    },
    {
      id: 'complexity-ema-family',
      input: paramsStrategy({ entrySig: 'emaCrossUp', exitSig: 'emaCrossDown' }),
      expectedUnits: 7,
    },
    {
      id: 'complexity-price-slow-family-one-risk',
      input: paramsStrategy({ entrySig: 'priceAboveSlow', exitSig: 'priceBelowSlow', slPct: 1 }),
      expectedUnits: 8,
    },
    {
      id: 'complexity-rsi-family-two-risks',
      input: paramsStrategy({ entrySig: 'rsiOversold', exitSig: 'rsiOverbought', slPct: 1, tpPct: 2 }),
      expectedUnits: 11,
    },
    {
      id: 'complexity-macd-family',
      input: paramsStrategy({ entrySig: 'macdCrossUp', exitSig: 'macdCrossDown' }),
      expectedUnits: 9,
    },
    {
      id: 'complexity-bollinger-family',
      input: paramsStrategy({ entrySig: 'bbLowerTouch', exitSig: 'bbUpperTouch' }),
      expectedUnits: 8,
    },
  ];
}

function buildScoreCaseDefinitions(): { id: string; input: ScoreCaseInput }[] {
  const partialOverride = baseScoreInput();
  partialOverride.validation.metrics.monthlyReturns = {
    '2024-01': -1,
    '2024-02': -1,
    '2024-03': 1,
    '2024-04': 1,
  };
  partialOverride.strategy = paramsStrategy({
    entrySig: 'rsiOversold',
    exitSig: 'rsiOverbought',
    slPct: 1,
    tpPct: 2,
  });
  partialOverride.testedCombinations = MAX_SAFE;
  partialOverride.config = {
    caps: { sortino: 10, consistencySigmaScale: 2 },
    weights: { consistency: 0.75, complexity: 0.25 },
  };

  const nonFinite = baseScoreInput();
  nonFinite.validation.metrics = {
    cagr: 'negative_zero',
    sortino: 'positive_infinity',
    calmar: 'negative_infinity',
    profitFactor: 'nan',
    turnover: 'positive_infinity',
    monthlyReturns: { '2024-01': 0.02, '2024-02': 0.03 },
  };
  nonFinite.validation.closedTradeCount = 0;
  nonFinite.validation.totalBars = 0;
  nonFinite.testedCombinations = 1;
  nonFinite.config = {
    weights: {
      cagr: 'negative_zero',
      sortino: 'negative_zero',
      calmar: 'negative_zero',
      regime: 'negative_zero',
      profitFactor: 'negative_zero',
      consistency: 'negative_zero',
      complexity: 'negative_zero',
      turnover: 'negative_zero',
      dataMining: 'negative_zero',
    },
  };

  const extremeFinite = baseScoreInput();
  extremeFinite.validation.metrics = {
    cagr: 2,
    sortino: 10,
    calmar: 20,
    profitFactor: 5,
    turnover: 0.2,
    monthlyReturns: {
      '2024-01': Number.MAX_VALUE,
      '2024-02': -Number.MAX_VALUE,
      '2024-03': 0,
    },
  };
  extremeFinite.strategy = paramsStrategy({
    entrySig: 'bbLowerTouch',
    exitSig: 'bbUpperTouch',
  });
  extremeFinite.testedCombinations = 10_000;

  return [
    { id: 'score-default-baseline', input: baseScoreInput() },
    { id: 'score-partial-config-population-sigma-max-safe-n', input: partialOverride },
    { id: 'score-nonfinite-statuses-negative-zero-insufficient', input: nonFinite },
    { id: 'score-extreme-finite-months-and-clamps', input: extremeFinite },
  ];
}

// ---------- held TS-reference errors ----------

interface ErrorExpectation {
  id: string;
  expectedErrorIncludes: string;
}

function heldError(
  run: () => void,
  expectation: ErrorExpectation,
  kind: 'range' | 'error' = 'range',
): ErrorExpectation {
  let thrown: unknown = null;
  try {
    run();
  } catch (error) {
    thrown = error;
  }
  if (thrown === null) throw new Error(`${expectation.id}: the TS reference did not throw`);
  if (kind === 'range' && !(thrown instanceof RangeError)) {
    throw new Error(`${expectation.id}: the TS reference must throw a RangeError`);
  }
  if (kind === 'error' && !(thrown instanceof Error)) {
    throw new Error(`${expectation.id}: the TS reference must throw an Error`);
  }
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  if (!message.includes(expectation.expectedErrorIncludes)) {
    throw new Error(
      `${expectation.id}: TS error "${message}" must mention ${expectation.expectedErrorIncludes}`,
    );
  }
  return expectation;
}

function buildGateErrorDefinitions(): {
  id: string;
  input: GateCaseInput;
  expectedErrorIncludes: string;
}[] {
  const errorCase = (
    id: string,
    fragment: string,
    mutate: (input: GateCaseInput) => void,
  ) => {
    const input = baseGateInput();
    mutate(input);
    return { id, input, expectedErrorIncludes: fragment };
  };
  return [
    errorCase('gate-duplicate-benchmark', 'duplicate deterministic benchmark', (input) => {
      input.benchmarks.push({ ...input.benchmarks[0] });
    }),
    errorCase('gate-missing-benchmark', 'missing deterministic benchmark', (input) => {
      input.benchmarks = input.benchmarks.slice(0, 3);
    }),
    errorCase('gate-invalid-min-trades', 'minTrades', (input) => {
      input.config = { minTrades: 0 };
    }),
    errorCase('gate-fractional-min-trades', 'minTrades', (input) => {
      input.config = { minTrades: 1.5 };
    }),
    errorCase('gate-min-trades-above-safe-range', 'minTrades', (input) => {
      input.config = { minTrades: MAX_SAFE + 1 };
    }),
    errorCase('gate-nonfinite-min-avg-return', 'minAvgTradeReturn', (input) => {
      input.config = { minAvgTradeReturn: 'positive_infinity' };
    }),
    errorCase('gate-invalid-rolling-window', 'rollingWindowBars', (input) => {
      input.config = { rollingWindowBars: 0 };
    }),
    errorCase('gate-fractional-rolling-window', 'rollingWindowBars', (input) => {
      input.config = { rollingWindowBars: 1.5 };
    }),
    errorCase('gate-rolling-window-above-safe-range', 'rollingWindowBars', (input) => {
      input.config = { rollingWindowBars: MAX_SAFE + 1 };
    }),
    errorCase('gate-invalid-min-rolling-ratio', 'minRollingPositiveRatio', (input) => {
      input.config = { minRollingPositiveRatio: 0 };
    }),
    errorCase('gate-invalid-max-drawdown', 'maxDrawdown', (input) => {
      input.config = { maxDrawdown: 0 };
    }),
    errorCase('gate-invalid-monthly-contribution', 'maxMonthlyContribution', (input) => {
      input.config = { maxMonthlyContribution: 0 };
    }),
    errorCase('gate-invalid-single-trade-contribution', 'maxSingleTradeContribution', (input) => {
      input.config = { maxSingleTradeContribution: 0 };
    }),
    errorCase('gate-negative-percentile', 'minRandomEntryPercentile', (input) => {
      input.config = { minRandomEntryPercentile: -1 };
    }),
    errorCase('gate-invalid-percentile', 'minRandomEntryPercentile', (input) => {
      input.config = { minRandomEntryPercentile: 101 };
    }),
    errorCase('gate-nonfinite-percentile', 'minRandomEntryPercentile', (input) => {
      input.config = { minRandomEntryPercentile: 'nan' };
    }),
  ];
}

function buildScoreErrorDefinitions(): {
  id: string;
  input: ScoreCaseInput;
  expectedErrorIncludes: string;
  kind?: 'range' | 'error';
}[] {
  const errorCase = (
    id: string,
    fragment: string,
    mutate: (input: ScoreCaseInput) => void,
    kind?: 'range' | 'error',
  ) => {
    const input = baseScoreInput();
    mutate(input);
    return { id, input, expectedErrorIncludes: fragment, kind };
  };
  return [
    errorCase('score-invalid-cap-zero', 'cap cagr', (input) => {
      input.config = { caps: { cagr: 0 } };
    }),
    errorCase('score-invalid-profit-factor-cap', 'profitFactor', (input) => {
      input.config = { caps: { profitFactor: 1 } };
    }),
    errorCase('score-invalid-negative-weight', 'weight calmar', (input) => {
      input.config = { weights: { calmar: -0.1 } };
    }),
    errorCase('score-invalid-nonfinite-weight', 'weight cagr', (input) => {
      input.config = { weights: { cagr: 'positive_infinity' } };
    }),
    errorCase('score-invalid-nonfinite-cap', 'cap cagr', (input) => {
      input.config = { caps: { cagr: 'nan' } };
    }),
    errorCase('score-deferred-regime-weight', 'REGIME-001', (input) => {
      input.config = { weights: { regime: 0.5 } };
    }),
    errorCase('score-tested-combinations-zero', 'testedCombinations', (input) => {
      input.testedCombinations = 0;
    }),
    errorCase('score-tested-combinations-fractional', 'testedCombinations', (input) => {
      input.testedCombinations = 1.5;
    }),
    errorCase('score-tested-combinations-above-safe-range', 'testedCombinations', (input) => {
      input.testedCombinations = MAX_SAFE + 1;
    }),
    errorCase('score-unsupported-stoch-signal', 'stoch', (input) => {
      input.strategy.entrySig = 'stochOversold';
    }, 'error'),
    errorCase(
      'score-resolved-weight-aggregate-overflow',
      'resolved score weights produce a non-finite score',
      (input) => {
        input.config = {
          weights: {
            calmar: Number.MAX_VALUE,
            consistency: Number.MAX_VALUE,
          },
        };
      },
    ),
  ];
}

export interface FixtureSourceHashes {
  generator: string;
  gate: string;
  score: string;
  strategy: string;
  benchmarks: string;
  validationRecord: string;
  metricsCodec: string;
  nonFinite: string;
}

function assertGateShape(id: string, verdict: GateVerdict): void {
  const ids = verdict.criteria.map((criterion) => criterion.id);
  if (ids.join('|') !== GATE_CRITERION_ORDER.join('|')) {
    throw new Error(`${id}: Gate criteria order drifted`);
  }
}

function assertScoreShape(id: string, breakdown: ScoreBreakdown): void {
  if (breakdown.formulaVersion !== SCORE_FORMULA_VERSION || breakdown.segment !== 'validation') {
    throw new Error(`${id}: Score envelope drifted`);
  }
  if (breakdown.components.map((entry) => entry.id).join('|') !== SCORE_COMPONENT_ORDER.join('|')) {
    throw new Error(`${id}: Score component order drifted`);
  }
  if (breakdown.penalties.map((entry) => entry.id).join('|') !== SCORE_PENALTY_ORDER.join('|')) {
    throw new Error(`${id}: Score penalty order drifted`);
  }
}

/** Build one combined envelope, but keep Gate and Score cases independent:
 * scoreCandidate deliberately does not enforce Gate ordering itself. */
export function buildGateScoreParityFixture(sourceHashes: FixtureSourceHashes) {
  const complexityCases = buildComplexityDefinitions().map((definition) => {
    const expected = complexityUnits(toStrategy(definition.input));
    if (expected.units !== definition.expectedUnits) {
      throw new Error(`${definition.id}: expected ${definition.expectedUnits} complexity units`);
    }
    return {
      id: definition.id,
      input: { strategy: definition.input },
      expected: canonicalizeExpected(expected),
    };
  });

  const gateCases = buildGateCaseDefinitions().map((definition) => {
    const verdict = evaluateGate(gateArgs(definition.input));
    assertGateShape(definition.id, verdict);
    const failed = verdict.criteria.filter((criterion) => !criterion.pass).map((criterion) => criterion.id);
    if (failed.join('|') !== definition.expectedFailed.join('|')) {
      throw new Error(`${definition.id}: failed criteria ${failed.join(', ') || '(none)'}`);
    }
    return {
      id: definition.id,
      input: definition.input,
      expected: canonicalizeExpected(encodeGateVerdict(verdict)),
    };
  });

  const scoreCases = buildScoreCaseDefinitions().map((definition) => {
    const breakdown = scoreCandidate(scoreArgs(definition.input));
    assertScoreShape(definition.id, breakdown);
    return {
      id: definition.id,
      input: definition.input,
      expected: canonicalizeExpected(breakdown),
    };
  });

  const gateErrorCases = buildGateErrorDefinitions().map((definition) => ({
    ...heldError(
      () => evaluateGate(gateArgs(definition.input)),
      { id: definition.id, expectedErrorIncludes: definition.expectedErrorIncludes },
    ),
    input: definition.input,
  }));

  const scoreErrorCases = buildScoreErrorDefinitions().map((definition) => ({
    ...heldError(
      () => scoreCandidate(scoreArgs(definition.input)),
      { id: definition.id, expectedErrorIncludes: definition.expectedErrorIncludes },
      definition.kind,
    ),
    input: definition.input,
  }));

  return {
    schemaVersion: PARITY_FIXTURE_SCHEMA_VERSION,
    fixtureVersion: GATE_SCORE_PARITY_FIXTURE_VERSION,
    contracts: {
      metrics: METRICS_CONTRACT_VERSION,
      gate: GATE_CONTRACT_VERSION,
      score: SCORE_FORMULA_VERSION,
    },
    generator: {
      command: 'npm run fixtures:gate-score',
      referenceRuntime: 'typescript',
      sourceHashEncoding: FIXTURE_SOURCE_HASH_ENCODING,
      sourceHashes,
    },
    numericEncoding: {
      specialInputNumbers: SPECIAL_INPUT_NUMBER_ENCODING,
      expectedTolerantFloats: EXPECTED_FLOAT_ENCODING,
    },
    tolerance: {
      default: { absolute: 1e-12, relative: 1e-10 },
      exact: [
        'schema, fixture, contract, provenance, and numeric-encoding versions',
        'case ids, inventory order, input keys, and input integers',
        'JSON object keys (order-insensitive), array order/length, booleans, strings, and nulls',
        'Gate criterion ids/order/pass/detail/valueStatus and discrete count values',
        'Score formula/segment/entry ids/order/rawStatus and evidence strings/counts',
        'complexity units/counts, testedCombinations integers, and error fragments',
      ],
      approximate: [
        'finite non-integer Gate values/config thresholds',
        'finite non-integer Score raw/normalized/weight/contribution/config/evidence values and score',
      ],
    },
    complexityCases,
    gateCases,
    scoreCases,
    gateErrorCases,
    scoreErrorCases,
  };
}

export type GateScoreParityFixture = ReturnType<typeof buildGateScoreParityFixture>;
