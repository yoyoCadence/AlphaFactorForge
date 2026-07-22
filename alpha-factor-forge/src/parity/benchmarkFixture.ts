// TS-reference builder for the RS-CORE-004 parity fixture: the mulberry32
// PRNG, the deterministic §6 benchmark suite, and the Random Entry Monte
// Carlo benchmark. Pure and deterministic; scripts/ owns file IO.
//
// Per the PR #66 Resolution D3 the RAW PRNG u32 sequence compares exactly;
// suite/random-entry trades, equity, and metric leaves follow the shared
// tolerance policy with METRIC-001 statuses exact. Every error expectation is
// HELD by the TypeScript reference at generation time (PR #69 pattern).

import { runBacktest, type BacktestConfig, type Candle as CoreCandle } from '../core/backtest';
import type { ClosedTrade, Metrics } from '../core/metrics';
import type { BacktestResult } from '../core/backtest';
import { runDeterministicBenchmarks, type BenchmarkCosts } from '../services/benchmarks';
import {
  planRandomTrades,
  runRandomEntryBenchmark,
  DEFAULT_RANDOM_ENTRY_RUNS,
  MAX_RANDOM_ENTRY_RUNS,
} from '../services/randomEntry';
import { makeSampleCandles, mulberry32 } from '../services/sampleData';
import { FIXTURE_SOURCE_HASH_ENCODING } from './indicatorFixture';
import { encodeMetricsForParity } from './parityEncode';

export const BENCHMARK_PARITY_FIXTURE_VERSION = 'benchmark-parity-v1';
export const PARITY_FIXTURE_SCHEMA_VERSION = 'rs-core-parity-fixture-v1';
export const CANDLE_CONTRACT_VERSION = 'ohlcv-candle-v1';
export const PRNG_CONTRACT_VERSION = 'mulberry32-v1';
export const BENCHMARK_CONTRACT_VERSION = 'benchmark-suite-v1';
export const RANDOM_ENTRY_CONTRACT_VERSION = 'random-entry-v1';
export const EXECUTION_CONTRACT_VERSION = 'backtest-execution-v1';
export const METRICS_CONTRACT_VERSION = 'metrics-v1';
export const PARAMS_SIGNALS_CONTRACT_VERSION = 'params-signals-v1';

export interface FixtureCandle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

const toCore = (candles: FixtureCandle[]): CoreCandle[] =>
  candles.map((candle) => ({
    t: candle.timestamp,
    o: candle.open,
    h: candle.high,
    l: candle.low,
    c: candle.close,
    v: candle.volume,
  }));

const HOUR = 3_600_000;
const DAY = 86_400_000;
const T0 = Date.UTC(2024, 0, 1);

const path = (closes: number[], intervalMs = HOUR): FixtureCandle[] =>
  closes.map((close, index) => {
    const open = index === 0 ? close : closes[index - 1];
    return {
      timestamp: T0 + index * intervalMs,
      open,
      high: Math.max(open, close) * 1.002,
      low: Math.min(open, close) * 0.998,
      close,
      volume: 1,
    };
  });

const rising = (count: number): FixtureCandle[] => {
  // Keep exact fixture inputs reproducible across JS runtimes. Math.pow is a
  // platform-provided approximation and can differ by one ULP in its tail.
  const closes: number[] = [];
  let close = 100;
  for (let i = 0; i < count; i++) {
    closes.push(close);
    close *= 1.01;
  }
  return path(closes);
};

const flat = (count: number): FixtureCandle[] => path(new Array<number>(count).fill(100));

/** The exact u32 the mulberry32 float was derived from (float = u32 / 2^32,
 *  so multiplying back is exact). */
const rawU32 = (next: () => number): number => next() * 4_294_967_296;

// ---------- PRNG cases ----------

interface PrngCaseDefinition {
  id: string;
  seed: number;
  count: number;
}

function buildPrngCases() {
  const definitions: PrngCaseDefinition[] = [
    { id: 'prng-seed-42', seed: 42, count: 64 },
    { id: 'prng-seed-7', seed: 7, count: 64 },
    { id: 'prng-seed-u32-max', seed: 4_294_967_295, count: 64 },
    { id: 'prng-seed-123', seed: 123, count: 32 },
    // TS mulberry32 truncates the seed with `>>> 0`; the sequence must equal
    // the plain seed-123 sequence and Rust must truncate identically.
    { id: 'prng-seed-truncated-2pow32-plus-123', seed: 4_294_967_296 + 123, count: 32 },
  ];
  const cases = definitions.map((definition) => {
    const next = mulberry32(definition.seed);
    return {
      id: definition.id,
      input: { seed: definition.seed, count: definition.count },
      expected: { rawU32: Array.from({ length: definition.count }, () => rawU32(next)) },
    };
  });
  const plain = cases.find((c) => c.id === 'prng-seed-123')!;
  const truncated = cases.find((c) => c.id === 'prng-seed-truncated-2pow32-plus-123')!;
  if (plain.expected.rawU32.join() !== truncated.expected.rawU32.join()) {
    throw new Error('mulberry32 seed truncation must reduce 2^32+123 to 123');
  }
  for (const parityCase of cases) {
    if (!parityCase.expected.rawU32.every((value) => Number.isInteger(value) && value >= 0)) {
      throw new Error(`${parityCase.id}: raw u32 reconstruction must be exact`);
    }
  }
  return cases;
}

// ---------- deterministic benchmark suite cases ----------

interface SuiteCaseDefinition {
  id: string;
  candles: FixtureCandle[];
  interval: string;
  costs: BenchmarkCosts;
  startEquity?: number;
  from?: number;
  to?: number;
  sanity: (runs: ReturnType<typeof runDeterministicBenchmarks>) => void;
}

function buildSuiteCases(): SuiteCaseDefinition[] {
  const sampleDaily = makeSampleCandles({
    count: 240,
    startTime: T0,
    intervalMs: DAY,
    startPrice: 100,
    seed: 11,
  }) as FixtureCandle[];
  const smaCrossDaily = path(
    [
      ...Array.from({ length: 200 }, (_, i) => 200 - i * 0.5),
      ...Array.from({ length: 60 }, (_, i) => 104 + i * 4),
    ],
    DAY,
  );

  return [
    {
      id: 'suite-small-no-cost',
      candles: path([100, 103, 101, 106, 104, 108, 105, 109, 107, 111]),
      interval: '1h',
      costs: { feePct: 0, slipPct: 0 },
      sanity: (runs) => {
        if (runs.length !== 4) throw new Error('suite must produce four benchmarks');
        if (runs[0].result.trades.length !== 1) throw new Error('buyHold must trade once');
      },
    },
    {
      id: 'suite-sample-daily-costs',
      candles: sampleDaily,
      interval: '1d',
      costs: { feePct: 0.05, slipPct: 0.02 },
      sanity: (runs) => {
        const total = runs.reduce((sum, run) => sum + run.result.trades.length, 0);
        if (total < 3) throw new Error('sample suite must trade across benchmarks');
        const rsi = runs.find((run) => run.id === 'rsiReversion')!;
        const bollinger = runs.find((run) => run.id === 'bollingerReversion')!;
        if (rsi.result.trades.length + bollinger.result.trades.length < 1) {
          throw new Error('reversion benchmarks must fire on the sample series');
        }
      },
    },
    {
      id: 'suite-sma-cross-trades',
      candles: smaCrossDaily,
      interval: '1d',
      costs: { feePct: 0, slipPct: 0 },
      sanity: (runs) => {
        const sma = runs.find((run) => run.id === 'smaCross')!;
        if (sma.result.trades.length < 1) {
          throw new Error('SMA 50/200 parity case must produce a closed trade');
        }
      },
    },
    {
      id: 'suite-subrange-prototype-key-interval',
      candles: path([
        100, 102, 104, 103, 105, 107, 106, 108, 110, 109,
        111, 113, 112, 114, 116, 115, 117, 119, 118, 120,
        122, 121, 123, 125, 124, 126, 128, 127, 129, 131,
      ]),
      interval: 'toString',
      costs: { feePct: 0.05, slipPct: 0.02 },
      startEquity: 5_000,
      from: 5,
      to: 25,
      sanity: (runs) => {
        const buyHold = runs[0];
        if (buyHold.result.trades.length !== 1) throw new Error('subrange buyHold must trade once');
        if (buyHold.result.equity.length !== 21) {
          throw new Error('subrange suite must emit 21 equity points');
        }
      },
    },
  ];
}

// ---------- Random Entry planner cases ----------

interface PlannerCaseDefinition {
  id: string;
  seed: number;
  from: number;
  to: number;
  holdingPool: number[];
  tradeCount: number;
  sanity: (planned: ReturnType<typeof planRandomTrades>) => void;
}

function buildPlannerCases(): PlannerCaseDefinition[] {
  return [
    {
      id: 'planner-basic',
      seed: 9,
      from: 5,
      to: 60,
      holdingPool: [2, 5, 9],
      tradeCount: 4,
      sanity: (planned) => {
        if (planned.length !== 4 || planned.some((trade) => trade.exitIdx === null)) {
          throw new Error('planner-basic must place four unclipped trades');
        }
      },
    },
    {
      id: 'planner-clip-and-drop',
      seed: 3,
      from: 0,
      to: 9,
      holdingPool: [50],
      tradeCount: 2,
      sanity: (planned) => {
        if (planned.length !== 1 || planned[0].exitIdx !== null) {
          throw new Error('planner-clip-and-drop must clip the first trade and drop the second');
        }
      },
    },
  ];
}

// ---------- Random Entry benchmark cases ----------

/** Minimal candidate persisted in the fixture: the fields the Random Entry
 *  benchmark actually reads (closed-trade bars pool + net return). */
interface FixtureCandidate {
  trades: ClosedTrade[];
  netReturn: number;
}

const candidateResult = (candidate: FixtureCandidate): BacktestResult => ({
  trades: candidate.trades,
  equity: [],
  metrics: { netReturn: candidate.netReturn } as unknown as Metrics,
});

const fakeTrade = (bars: number): ClosedTrade => ({
  entryTime: 0,
  exitTime: bars,
  side: 'LONG',
  entryPrice: 100,
  exitPrice: 101,
  pnl: 10,
  pnlPct: 0.1,
  bars,
});

interface RandomEntryCaseDefinition {
  id: string;
  candles: FixtureCandle[];
  interval: string;
  costs: BenchmarkCosts;
  candidate: FixtureCandidate;
  seed: number;
  runs?: number;
  startEquity?: number;
  from?: number;
  to?: number;
  sanity: (benchmark: ReturnType<typeof runRandomEntryBenchmark>) => void;
}

function momentumCandidate(candles: FixtureCandle[]): FixtureCandidate {
  const closes = candles.map((candle) => candle.close);
  const signals = {
    entry: closes.map((close, i) => i >= 2 && close > closes[i - 1] && closes[i - 1] > closes[i - 2]),
    exit: closes.map((close, i) => i >= 1 && close < closes[i - 1]),
  };
  const config: BacktestConfig = {
    exec: { direction: 'long', sizingPct: 1, fillMode: 'nextOpen' },
    cost: { feePct: 0.0005, slippagePct: 0.0002 },
    risk: { stopLossPct: 0.05, takeProfitPct: 0.1 },
    barsPerYear: 365,
  };
  const result = runBacktest(toCore(candles), signals, config);
  if (result.trades.length < 5) {
    throw new Error('the real candidate must produce a usable holding-period pool');
  }
  return { trades: result.trades, netReturn: result.metrics.netReturn };
}

function buildRandomEntryCases(): RandomEntryCaseDefinition[] {
  const sampleDaily = makeSampleCandles({
    count: 180,
    startTime: T0,
    intervalMs: DAY,
    startPrice: 100,
    seed: 7,
  }) as FixtureCandle[];

  return [
    {
      id: 'random-entry-fake-candidate',
      candles: rising(60),
      interval: '1h',
      costs: { feePct: 0, slipPct: 0 },
      candidate: { trades: [fakeTrade(2), fakeTrade(4)], netReturn: 0.05 },
      seed: 42,
      runs: 40,
      sanity: (benchmark) => {
        if (benchmark.netReturns.length !== 40) throw new Error('fake candidate must run 40 sims');
      },
    },
    {
      id: 'random-entry-real-candidate',
      candles: sampleDaily,
      interval: '1d',
      costs: { feePct: 0.05, slipPct: 0.02 },
      candidate: momentumCandidate(sampleDaily),
      seed: 7,
      runs: 30,
      sanity: (benchmark) => {
        if (benchmark.netReturns.length !== 30) throw new Error('real candidate must run 30 sims');
        if (benchmark.candidatePercentile < 0 || benchmark.candidatePercentile > 100) {
          throw new Error('percentile must stay in range');
        }
      },
    },
    {
      id: 'random-entry-zero-bar-clamp-subrange',
      candles: rising(40),
      interval: '1h',
      costs: { feePct: 0, slipPct: 0 },
      candidate: { trades: [fakeTrade(0)], netReturn: 0.02 },
      seed: 19,
      runs: 25,
      from: 5,
      to: 30,
      sanity: (benchmark) => {
        if (
          benchmark.netReturns.length !== 25 ||
          benchmark.netReturns.some((value) => value < 0.009 || value > 0.011)
        ) {
          throw new Error('zero-bar holding periods must clamp to one bar in the subrange');
        }
      },
    },
    {
      id: 'random-entry-flat-tie-default-runs',
      candles: flat(40),
      interval: '1h',
      costs: { feePct: 0, slipPct: 0 },
      candidate: { trades: [fakeTrade(2)], netReturn: 0 },
      seed: 123,
      from: 5,
      to: 30,
      sanity: (benchmark) => {
        if (
          benchmark.runs !== DEFAULT_RANDOM_ENTRY_RUNS ||
          benchmark.netReturns.some((value) => value !== 0)
        ) {
          throw new Error('default Random Entry runs over a flat subrange must all return zero');
        }
        if (benchmark.candidatePercentile !== 0) {
          throw new Error('tied random returns must not count as strictly beaten');
        }
      },
    },
    {
      id: 'random-entry-min-seed-min-runs',
      candles: rising(12),
      interval: '1h',
      costs: { feePct: 0, slipPct: 0 },
      candidate: { trades: [fakeTrade(1)], netReturn: 0 },
      seed: 0,
      runs: 1,
      sanity: (benchmark) => {
        if (benchmark.seed !== 0 || benchmark.runs !== 1 || benchmark.netReturns.length !== 1) {
          throw new Error('minimum accepted Random Entry seed/runs must execute once');
        }
      },
    },
    {
      id: 'random-entry-max-seed-max-runs',
      candles: flat(12),
      interval: '1h',
      costs: { feePct: 0, slipPct: 0 },
      candidate: { trades: [fakeTrade(1)], netReturn: 0 },
      seed: Number.MAX_SAFE_INTEGER,
      runs: MAX_RANDOM_ENTRY_RUNS,
      sanity: (benchmark) => {
        if (
          benchmark.seed !== Number.MAX_SAFE_INTEGER ||
          benchmark.runs !== MAX_RANDOM_ENTRY_RUNS ||
          benchmark.netReturns.length !== MAX_RANDOM_ENTRY_RUNS
        ) {
          throw new Error('maximum accepted Random Entry seed/runs must execute fully');
        }
      },
    },
  ];
}

// ---------- TS-held error cases ----------

interface ErrorExpectation {
  id: string;
  expectedErrorIncludes: string;
}

function heldError(run: () => void, expectation: ErrorExpectation): ErrorExpectation {
  let thrown: unknown = null;
  try {
    run();
  } catch (error) {
    thrown = error;
  }
  if (thrown === null) throw new Error(`${expectation.id}: the TS reference did not throw`);
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  if (!message.includes(expectation.expectedErrorIncludes)) {
    throw new Error(
      `${expectation.id}: TS error "${message}" must mention ${expectation.expectedErrorIncludes}`,
    );
  }
  return expectation;
}

function assertFiniteResult(id: string, result: BacktestResult): void {
  for (const trade of result.trades) {
    for (const value of [trade.entryPrice, trade.exitPrice, trade.pnl, trade.pnlPct]) {
      if (!Number.isFinite(value)) throw new Error(`${id}: trade values must stay finite`);
    }
  }
  for (const point of result.equity) {
    if (!Number.isFinite(point.equity)) throw new Error(`${id}: equity must stay finite`);
  }
}

export interface FixtureSourceHashes {
  generator: string;
  parityEncode: string;
  nonFinite: string;
  benchmarks: string;
  randomEntry: string;
  backtestRunner: string;
  strategySignals: string;
  strategy: string;
  indicators: string;
  backtest: string;
  metrics: string;
  sampleData: string;
}

export function buildBenchmarkParityFixture(sourceHashes: FixtureSourceHashes) {
  const prngCases = buildPrngCases();

  const suiteCases = buildSuiteCases().map((definition) => {
    const runs = runDeterministicBenchmarks({
      candles: toCore(definition.candles),
      interval: definition.interval,
      costs: definition.costs,
      ...(definition.startEquity !== undefined ? { startEquity: definition.startEquity } : {}),
      ...(definition.from !== undefined ? { from: definition.from } : {}),
      ...(definition.to !== undefined ? { to: definition.to } : {}),
    });
    definition.sanity(runs);
    for (const run of runs) assertFiniteResult(`${definition.id}.${run.id}`, run.result);
    return {
      id: definition.id,
      input: {
        candles: definition.candles,
        interval: definition.interval,
        costs: definition.costs,
        startEquity: definition.startEquity ?? null,
        from: definition.from ?? null,
        to: definition.to ?? null,
      },
      expected: {
        benchmarks: runs.map((run) => ({
          id: run.id,
          stratIsNull: run.strat === null,
          strat: run.strat,
          result: {
            trades: run.result.trades,
            equity: run.result.equity,
            metrics: encodeMetricsForParity(run.result.metrics),
          },
        })),
      },
    };
  });

  const plannerCases = buildPlannerCases().map((definition) => {
    const planned = planRandomTrades(
      mulberry32(definition.seed),
      definition.from,
      definition.to,
      definition.holdingPool,
      definition.tradeCount,
    );
    definition.sanity(planned);
    return {
      id: definition.id,
      input: {
        seed: definition.seed,
        from: definition.from,
        to: definition.to,
        holdingPool: definition.holdingPool,
        tradeCount: definition.tradeCount,
      },
      expected: { planned },
    };
  });

  const randomEntryCases = buildRandomEntryCases().map((definition) => {
    const benchmark = runRandomEntryBenchmark({
      candles: toCore(definition.candles),
      interval: definition.interval,
      costs: definition.costs,
      candidateResult: candidateResult(definition.candidate),
      seed: definition.seed,
      ...(definition.runs !== undefined ? { runs: definition.runs } : {}),
      ...(definition.startEquity !== undefined ? { startEquity: definition.startEquity } : {}),
      ...(definition.from !== undefined ? { from: definition.from } : {}),
      ...(definition.to !== undefined ? { to: definition.to } : {}),
    });
    definition.sanity(benchmark);
    for (const value of [
      ...benchmark.netReturns,
      benchmark.candidateNetReturn,
      benchmark.candidatePercentile,
    ]) {
      if (!Number.isFinite(value)) {
        throw new Error(`${definition.id}: Random Entry outputs must stay finite`);
      }
    }
    return {
      id: definition.id,
      input: {
        candles: definition.candles,
        interval: definition.interval,
        costs: definition.costs,
        candidate: definition.candidate,
        seed: definition.seed,
        runs: definition.runs ?? null,
        startEquity: definition.startEquity ?? null,
        from: definition.from ?? null,
        to: definition.to ?? null,
      },
      expected: benchmark,
    };
  });

  const errorCandles = rising(20);
  const errorCandidate: FixtureCandidate = { trades: [fakeTrade(2)], netReturn: 0.05 };
  const randomEntryError = (
    id: string,
    fragment: string,
    over: Partial<{ candles: FixtureCandle[]; candidate: FixtureCandidate; seed: number; runs: number; from: number; to: number }>,
  ) => {
    const input = {
      candles: over.candles ?? errorCandles,
      interval: '1h',
      costs: { feePct: 0, slipPct: 0 } as BenchmarkCosts,
      candidate: over.candidate ?? errorCandidate,
      seed: over.seed ?? 1,
      runs: over.runs ?? 10,
      from: over.from ?? null,
      to: over.to ?? null,
    };
    return {
      ...heldError(
        () =>
          runRandomEntryBenchmark({
            candles: toCore(input.candles),
            interval: input.interval,
            costs: input.costs,
            candidateResult: candidateResult(input.candidate),
            seed: input.seed,
            runs: input.runs,
            ...(input.from !== null ? { from: input.from } : {}),
            ...(input.to !== null ? { to: input.to } : {}),
          }),
        { id, expectedErrorIncludes: fragment },
      ),
      input,
    };
  };

  const benchmarkErrorCases = [
    {
      ...heldError(
        () =>
          runDeterministicBenchmarks({
            candles: [],
            interval: '1h',
            costs: { feePct: 0, slipPct: 0 },
          }),
        { id: 'benchmarks-empty-candles', expectedErrorIncludes: 'non-empty candle series' },
      ),
      input: {
        candles: [] as FixtureCandle[],
        interval: '1h',
        costs: { feePct: 0, slipPct: 0 } as BenchmarkCosts,
        startEquity: null,
        from: null,
        to: null,
      },
    },
  ];

  const randomEntryErrorCases = [
    randomEntryError('random-entry-zero-runs', 'runs must be an integer', { runs: 0 }),
    randomEntryError('random-entry-runs-above-cap', 'runs must be an integer', {
      runs: MAX_RANDOM_ENTRY_RUNS + 1,
    }),
    randomEntryError('random-entry-negative-seed', 'seed must be a non-negative safe integer', {
      seed: -1,
    }),
    randomEntryError(
      'random-entry-seed-above-safe-range',
      'seed must be a non-negative safe integer',
      { seed: Number.MAX_SAFE_INTEGER + 1 },
    ),
    randomEntryError('random-entry-empty-candles', 'non-empty candle series', { candles: [] }),
    randomEntryError('random-entry-inverted-segment', 'non-empty [from, to] segment', {
      from: 5,
      to: 2,
    }),
    randomEntryError('random-entry-no-candidate-trades', 'at least one closed candidate trade', {
      candidate: { trades: [], netReturn: 0 },
    }),
  ];

  return {
    schemaVersion: PARITY_FIXTURE_SCHEMA_VERSION,
    fixtureVersion: BENCHMARK_PARITY_FIXTURE_VERSION,
    contracts: {
      candle: CANDLE_CONTRACT_VERSION,
      execution: EXECUTION_CONTRACT_VERSION,
      metrics: METRICS_CONTRACT_VERSION,
      signals: PARAMS_SIGNALS_CONTRACT_VERSION,
      prng: PRNG_CONTRACT_VERSION,
      benchmarks: BENCHMARK_CONTRACT_VERSION,
      randomEntry: RANDOM_ENTRY_CONTRACT_VERSION,
    },
    generator: {
      command: 'npm run fixtures:benchmarks',
      referenceRuntime: 'typescript',
      sourceHashEncoding: FIXTURE_SOURCE_HASH_ENCODING,
      sourceHashes,
    },
    tolerance: {
      default: { absolute: 1e-12, relative: 1e-10 },
      exact: [
        'schemaVersion and contract versions',
        'case ids and inputs',
        'raw PRNG u32 sequences',
        'planner entry/exit indexes and clipping',
        'trade sides, bars, entry/exit timestamps',
        'equity point timestamps and array lengths',
        'runs, seeds, and monthly-return keys',
        'METRIC-001 non-finite statuses',
        'error-case messages contain their expected fragment',
      ],
    },
    prngCases,
    suiteCases,
    plannerCases,
    randomEntryCases,
    benchmarkErrorCases,
    randomEntryErrorCases,
  };
}

export type BenchmarkParityFixture = ReturnType<typeof buildBenchmarkParityFixture>;
