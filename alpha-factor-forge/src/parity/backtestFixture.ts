// TS-reference builder for the RS-CORE-002 backtest + metrics parity fixture.
// Pure and deterministic; scripts/generate-backtest-fixtures.ts owns file IO.
//
// Every case's expected output is produced by the REAL TypeScript engine (the
// reference implementation), so intent lives in the case inputs plus the
// sanity invariants below — if a scenario stops exercising its target branch
// (e.g. a stop-loss gap), generation fails instead of silently degrading.

import { runBacktest, type BacktestConfig, type Candle as CoreCandle } from '../core/backtest';
import type { Metrics } from '../core/metrics';
import { makeSampleCandles } from '../services/sampleData';
import type { NonFiniteStatus } from '../services/nonFinite';
import { nonFiniteStatus } from '../services/nonFinite';

export const BACKTEST_PARITY_FIXTURE_VERSION = 'backtest-parity-v1';
export const PARITY_FIXTURE_SCHEMA_VERSION = 'rs-core-parity-fixture-v1';
export const CANDLE_CONTRACT_VERSION = 'ohlcv-candle-v1';
/** Same adopted contract the validation record embeds (docs/backtest-execution-contract.md). */
export const EXECUTION_CONTRACT_VERSION = 'backtest-execution-v1';
export const METRICS_CONTRACT_VERSION = 'metrics-v1';

export interface FixtureCandle {
  timestamp: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

/** Finite number, or the METRIC-001 status of a legitimate non-finite value. */
export type MetricLeaf = number | NonFiniteStatus;

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

const bar = (
  index: number,
  open: number,
  high: number,
  low: number,
  close: number,
  intervalMs = HOUR,
): FixtureCandle => ({ timestamp: T0 + index * intervalMs, open, high, low, close, volume: 1 });

/** Trending candles where each bar closes at `closes[i]` and opens near the
 *  previous close, with a small deterministic high/low envelope. */
const path = (closes: number[], intervalMs = HOUR): FixtureCandle[] =>
  closes.map((close, index) => {
    const open = index === 0 ? close : closes[index - 1];
    const high = Math.max(open, close) * 1.002;
    const low = Math.min(open, close) * 0.998;
    return bar(index, open, high, low, close, intervalMs);
  });

const signalArrays = (length: number, entries: number[], exits: number[]) => ({
  entry: Array.from({ length }, (_, i) => entries.includes(i)),
  exit: Array.from({ length }, (_, i) => exits.includes(i)),
});

interface ConfigOverrides {
  exec?: Partial<BacktestConfig['exec']>;
  cost?: Partial<BacktestConfig['cost']>;
  risk?: BacktestConfig['risk'];
  barsPerYear?: number;
  startEquity?: number;
  from?: number;
  to?: number;
}

const baseConfig = (over: ConfigOverrides = {}): BacktestConfig => ({
  exec: { direction: 'long', sizingPct: 1, fillMode: 'close', ...over.exec },
  cost: { feePct: 0, slippagePct: 0, ...over.cost },
  ...(over.risk !== undefined ? { risk: over.risk } : {}),
  barsPerYear: over.barsPerYear ?? 8_760,
  ...(over.startEquity !== undefined ? { startEquity: over.startEquity } : {}),
  ...(over.from !== undefined ? { from: over.from } : {}),
  ...(over.to !== undefined ? { to: over.to } : {}),
});

interface CaseDefinition {
  id: string;
  candles: FixtureCandle[];
  signals: { entry: boolean[]; exit: boolean[] };
  config: BacktestConfig;
  /** Generation-time invariant: the scenario must keep exercising its branch. */
  sanity: (result: ReturnType<typeof runBacktest>) => void;
}

const expectTrades = (label: string, actual: number, expected: number): void => {
  if (actual !== expected) {
    throw new Error(`${label}: expected ${expected} trades, engine produced ${actual}`);
  }
};

function momentumSignals(closes: number[]): { entry: boolean[]; exit: boolean[] } {
  return {
    entry: closes.map(
      (close, i) => i >= 2 && close > closes[i - 1] && closes[i - 1] > closes[i - 2],
    ),
    exit: closes.map((close, i) => i >= 1 && close < closes[i - 1]),
  };
}

function buildCases(): CaseDefinition[] {
  const sampleDaily = makeSampleCandles({
    count: 180,
    startTime: T0,
    intervalMs: DAY,
    startPrice: 100,
    seed: 7,
  }) as FixtureCandle[];
  const sampleCloses = sampleDaily.map((candle) => candle.close);

  return [
    {
      id: 'long-close-two-roundtrips',
      candles: path([100, 104, 102, 106, 103, 108, 105, 101]),
      // exit+entry on the same bar 4: close-mode exits first, then re-enters.
      signals: signalArrays(8, [1, 4], [4, 6]),
      config: baseConfig(),
      sanity: (result) => expectTrades('long-close-two-roundtrips', result.trades.length, 2),
    },
    {
      id: 'long-close-costs-partial-sizing',
      candles: path([100, 103, 101, 107, 104, 110]),
      signals: signalArrays(6, [0, 3], [2, 5]),
      config: baseConfig({ exec: { sizingPct: 0.5 }, cost: { feePct: 0.0005, slippagePct: 0.0002 } }),
      sanity: (result) => expectTrades('long-close-costs-partial-sizing', result.trades.length, 2),
    },
    {
      id: 'short-close-win-and-loss',
      candles: path([100, 96, 99, 94, 98, 103]),
      signals: signalArrays(6, [0, 3], [2, 5]),
      config: baseConfig({ exec: { direction: 'short' }, cost: { feePct: 0.0005, slippagePct: 0.0002 } }),
      sanity: (result) => {
        expectTrades('short-close-win-and-loss', result.trades.length, 2);
        if (result.trades[0].side !== 'SHORT') throw new Error('short case must open SHORT');
      },
    },
    {
      id: 'both-close-reversals',
      candles: path([100, 103, 99, 104, 101, 106, 103, 100]),
      // bar1 LONG; bar3 SHORT reversal; bar5 entry+exit together => LONG wins;
      // bar6 LONG again while LONG => hold (no trade).
      signals: {
        entry: [false, true, false, false, false, true, true, false],
        exit: [false, false, false, true, false, true, false, false],
      },
      config: baseConfig({ exec: { direction: 'both' } }),
      sanity: (result) => {
        expectTrades('both-close-reversals', result.trades.length, 3);
        if (result.trades[1].side !== 'SHORT') throw new Error('reversal must open SHORT');
      },
    },
    {
      id: 'long-nextopen-pending-and-final-bar',
      candles: path([100, 102, 104, 101, 105, 103]),
      // final-bar entry signal (i == to) has no execution candle => no fill.
      signals: signalArrays(6, [0, 5], [2]),
      config: baseConfig({ exec: { fillMode: 'nextOpen' }, cost: { feePct: 0.0005, slippagePct: 0.0002 } }),
      sanity: (result) => {
        expectTrades('long-nextopen-pending-and-final-bar', result.trades.length, 1);
        if (result.trades[0].entryTime !== T0 + 1 * HOUR) {
          throw new Error('nextOpen entry must fill on the following bar');
        }
      },
    },
    {
      id: 'both-nextopen-reversal',
      candles: path([100, 103, 99, 104, 101, 106, 102]),
      signals: {
        entry: [true, false, false, false, true, false, false],
        exit: [false, false, true, false, false, false, false],
      },
      config: baseConfig({ exec: { direction: 'both', fillMode: 'nextOpen' } }),
      sanity: (result) => expectTrades('both-nextopen-reversal', result.trades.length, 3),
    },
    {
      id: 'long-stoploss-gap-through',
      candles: [bar(0, 100, 101, 99, 100), bar(1, 90, 92, 88, 91), bar(2, 91, 93, 90, 92)],
      signals: signalArrays(3, [0], []),
      config: baseConfig({ risk: { stopLossPct: 0.05 }, cost: { feePct: 0, slippagePct: 0.0002 } }),
      sanity: (result) => {
        expectTrades('long-stoploss-gap-through', result.trades.length, 1);
        // gap-aware base is min(open, slPrice) = 90, then closing-side slippage.
        if (Math.abs(result.trades[0].exitPrice - 90 * (1 - 0.0002)) > 1e-9) {
          throw new Error('stop-loss must fill at the gapped open');
        }
      },
    },
    {
      id: 'long-takeprofit-then-gap-up',
      candles: [
        bar(0, 100, 101, 99, 100),
        bar(1, 105, 112, 104, 108), // TP 110 inside range, open below => base 110
        bar(2, 100, 101, 99, 100),
        bar(3, 116, 118, 114, 117), // TP gap: open 116 above 110 => base 116
        bar(4, 117, 118, 116, 117),
      ],
      signals: signalArrays(5, [0, 2], []),
      config: baseConfig({ risk: { takeProfitPct: 0.1 } }),
      sanity: (result) => {
        expectTrades('long-takeprofit-then-gap-up', result.trades.length, 2);
        if (Math.abs(result.trades[0].exitPrice - 110) > 1e-9) throw new Error('TP base must be 110');
        if (Math.abs(result.trades[1].exitPrice - 116) > 1e-9) throw new Error('TP gap base must be 116');
      },
    },
    {
      id: 'short-stoploss-and-takeprofit',
      candles: [
        bar(0, 100, 101, 99, 100),
        bar(1, 103, 106, 102, 105), // short SL 105: base max(103, 105) = 105
        bar(2, 100, 101, 99, 100),
        bar(3, 92, 95, 88, 94), // short TP 90: base min(92, 90) = 90
        bar(4, 94, 95, 93, 94),
      ],
      signals: signalArrays(5, [0, 2], []),
      config: baseConfig({
        exec: { direction: 'short' },
        risk: { stopLossPct: 0.05, takeProfitPct: 0.1 },
        cost: { feePct: 0.0005, slippagePct: 0.0002 },
      }),
      sanity: (result) => {
        expectTrades('short-stoploss-and-takeprofit', result.trades.length, 2);
        if (result.trades[0].pnl >= 0) throw new Error('first short trade must lose to its stop');
        if (result.trades[1].pnl <= 0) throw new Error('second short trade must win to its target');
      },
    },
    {
      id: 'stoploss-wins-ambiguous-bar',
      candles: [bar(0, 100, 101, 99, 100), bar(1, 100, 106, 94, 100), bar(2, 100, 101, 99, 100)],
      signals: signalArrays(3, [0], []),
      config: baseConfig({ risk: { stopLossPct: 0.05, takeProfitPct: 0.05 } }),
      sanity: (result) => {
        expectTrades('stoploss-wins-ambiguous-bar', result.trades.length, 1);
        if (result.trades[0].pnl >= 0) throw new Error('ambiguous SL/TP bar must resolve to the stop');
      },
    },
    {
      id: 'full-sizing-budgets-entry-fee',
      candles: path([100, 100, 100, 100]),
      signals: signalArrays(4, [0], [3]),
      config: baseConfig({ cost: { feePct: 0.001 } }),
      sanity: (result) => {
        expectTrades('full-sizing-budgets-entry-fee', result.trades.length, 1);
        if (result.trades[0].pnl >= 0) throw new Error('flat market with fees must lose the fees');
      },
    },
    {
      id: 'eod-settles-open-position',
      candles: path([100, 104, 108, 112]),
      signals: signalArrays(4, [1], []),
      config: baseConfig({ cost: { feePct: 0.0005, slippagePct: 0.0002 } }),
      sanity: (result) => {
        expectTrades('eod-settles-open-position', result.trades.length, 1);
        const last = result.equity[result.equity.length - 1];
        if (last.equity <= 10_000) throw new Error('winning EOD settlement must beat start equity');
      },
    },
    {
      id: 'from-to-subrange',
      candles: path([100, 102, 104, 103, 105, 107, 106, 108, 110, 109]),
      signals: signalArrays(10, [3], [6]),
      config: baseConfig({ from: 2, to: 7 }),
      sanity: (result) => {
        expectTrades('from-to-subrange', result.trades.length, 1);
        if (result.equity.length !== 6) throw new Error('sub-range must emit 6 equity points');
      },
    },
    {
      id: 'single-bar-from-equals-to',
      candles: path([100, 102, 104, 103, 105]),
      signals: signalArrays(5, [2], []),
      config: baseConfig({ from: 2, to: 2, cost: { feePct: 0.0005 } }),
      sanity: (result) => {
        expectTrades('single-bar-from-equals-to', result.trades.length, 1);
        if (result.trades[0].bars !== 0) throw new Error('same-bar entry+EOD must hold 0 bars');
      },
    },
    {
      id: 'no-trades-zero-metrics',
      candles: path([100, 101, 102, 101, 100]),
      signals: signalArrays(5, [], []),
      config: baseConfig(),
      sanity: (result) => expectTrades('no-trades-zero-metrics', result.trades.length, 0),
    },
    {
      id: 'rising-no-downside-infinite-ratios',
      candles: path([100, 102, 104, 106, 108, 110]),
      signals: signalArrays(6, [0], []),
      config: baseConfig({ barsPerYear: 6 }),
      sanity: (result) => {
        expectTrades('rising-no-downside-infinite-ratios', result.trades.length, 1);
        if (result.metrics.sortino !== Infinity) throw new Error('no-downside case must yield infinite Sortino');
        if (result.metrics.calmar !== Infinity) throw new Error('zero-drawdown case must yield infinite Calmar');
        if (result.metrics.profitFactor !== Infinity) throw new Error('loss-free case must yield infinite PF');
      },
    },
    {
      id: 'sample-daily-long-nextopen-risk',
      candles: sampleDaily,
      signals: momentumSignals(sampleCloses),
      config: baseConfig({
        exec: { fillMode: 'nextOpen' },
        cost: { feePct: 0.0005, slippagePct: 0.0002 },
        risk: { stopLossPct: 0.05, takeProfitPct: 0.1 },
        barsPerYear: 365,
      }),
      sanity: (result) => {
        if (result.trades.length < 5) throw new Error('sample nextOpen case must trade repeatedly');
        if (Object.keys(result.metrics.monthlyReturns).length < 4) {
          throw new Error('sample case must span multiple calendar months');
        }
      },
    },
    {
      id: 'sample-daily-both-close',
      candles: sampleDaily,
      signals: {
        entry: sampleCloses.map((close, i) => i >= 1 && close > sampleCloses[i - 1]),
        exit: sampleCloses.map((close, i) => i >= 1 && close < sampleCloses[i - 1]),
      },
      config: baseConfig({ exec: { direction: 'both' }, cost: { feePct: 0.0005, slippagePct: 0.0002 }, barsPerYear: 365 }),
      sanity: (result) => {
        if (result.trades.length < 10) throw new Error('sample both case must reverse repeatedly');
        if (!result.trades.some((trade) => trade.side === 'SHORT')) {
          throw new Error('sample both case must include SHORT trades');
        }
      },
    },
  ];
}

export interface ErrorCaseDefinition {
  id: string;
  input: {
    candles: FixtureCandle[];
    signals: { entry: boolean[]; exit: boolean[] };
    config: BacktestConfig;
  };
  expectedErrorIncludes: string;
}

function buildErrorCases(): ErrorCaseDefinition[] {
  const candles = path([100, 101, 102]);
  const signals = signalArrays(3, [0], [2]);
  return [
    {
      id: 'sizing-above-one-fails-closed',
      input: { candles, signals, config: baseConfig({ exec: { sizingPct: 1.5 } }) },
      expectedErrorIncludes: 'exec.sizingPct',
    },
    {
      id: 'negative-fee-fails-closed',
      input: { candles, signals, config: baseConfig({ cost: { feePct: -0.1 } }) },
      expectedErrorIncludes: 'cost.feePct',
    },
    {
      id: 'zero-stoploss-fails-closed',
      input: { candles, signals, config: baseConfig({ risk: { stopLossPct: 0 } }) },
      expectedErrorIncludes: 'risk.stopLossPct',
    },
  ];
}

const encodeLeaf = (value: number): MetricLeaf => nonFiniteStatus(value) ?? value;

function encodeMetricsForParity(metrics: Metrics): Record<string, MetricLeaf | Record<string, number>> {
  const encoded: Record<string, MetricLeaf | Record<string, number>> = {};
  for (const [key, value] of Object.entries(metrics)) {
    if (typeof value === 'number') {
      encoded[key] = encodeLeaf(value);
    } else {
      for (const monthly of Object.values(value as Record<string, number>)) {
        if (!Number.isFinite(monthly)) throw new Error('monthly returns must stay finite');
      }
      encoded[key] = { ...(value as Record<string, number>) };
    }
  }
  return encoded;
}

function assertFiniteResult(id: string, result: ReturnType<typeof runBacktest>): void {
  for (const trade of result.trades) {
    for (const value of [trade.entryPrice, trade.exitPrice, trade.pnl, trade.pnlPct]) {
      if (!Number.isFinite(value)) throw new Error(`${id}: trade values must be finite`);
    }
  }
  for (const point of result.equity) {
    if (!Number.isFinite(point.equity)) throw new Error(`${id}: equity must stay finite`);
  }
}

export interface FixtureSourceHashes {
  generator: string;
  backtest: string;
  metrics: string;
  sampleData: string;
}

export function buildBacktestParityFixture(sourceHashes: FixtureSourceHashes) {
  const cases = buildCases().map((definition) => {
    const result = runBacktest(toCore(definition.candles), definition.signals, definition.config);
    definition.sanity(result);
    assertFiniteResult(definition.id, result);
    return {
      id: definition.id,
      input: {
        candles: definition.candles,
        signals: definition.signals,
        config: definition.config,
      },
      expected: {
        trades: result.trades,
        equity: result.equity,
        metrics: encodeMetricsForParity(result.metrics),
      },
    };
  });

  return {
    schemaVersion: PARITY_FIXTURE_SCHEMA_VERSION,
    fixtureVersion: BACKTEST_PARITY_FIXTURE_VERSION,
    contracts: {
      candle: CANDLE_CONTRACT_VERSION,
      execution: EXECUTION_CONTRACT_VERSION,
      metrics: METRICS_CONTRACT_VERSION,
    },
    generator: {
      command: 'npm run fixtures:backtest',
      referenceRuntime: 'typescript',
      sourceHashEncoding: 'utf8-lf-v1',
      sourceHashes,
    },
    tolerance: {
      default: { absolute: 1e-12, relative: 1e-10 },
      exact: [
        'schemaVersion and contract versions',
        'case ids and config values',
        'trade sides, bars, entry/exit timestamps',
        'equity point timestamps and array lengths',
        'monthly-return keys',
        'METRIC-001 non-finite statuses',
        'error-case messages contain their expected fragment',
      ],
    },
    cases,
    errorCases: buildErrorCases(),
  };
}

export type BacktestParityFixture = ReturnType<typeof buildBacktestParityFixture>;
