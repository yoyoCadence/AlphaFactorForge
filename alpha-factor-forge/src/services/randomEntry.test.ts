import { describe, it, expect } from 'vitest';
import type { BacktestResult, Candle } from '../core/backtest';
import type { ClosedTrade, Metrics } from '../core/metrics';
import { runParamsBacktest } from './backtestRunner';
import { defaultStrategy } from './strategy';
import { toCoreCandles } from './candleAdapter';
import { makeSampleCandles, mulberry32 } from './sampleData';
import {
  DEFAULT_RANDOM_ENTRY_RUNS,
  planRandomTrades,
  runRandomEntryBenchmark,
} from './randomEntry';

/** Flat/rising candles from a close series; time = index. */
const mk = (closes: number[]): Candle[] =>
  closes.map((c, i) => ({ t: i, o: c, h: c, l: c, c, v: 1 }));

const rising = (n: number): Candle[] =>
  mk(Array.from({ length: n }, (_, i) => 100 * Math.pow(1.01, i)));

const zeroMetrics = (): Metrics => ({
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
});

const fakeTrade = (bars: number): ClosedTrade => ({
  entryTime: 0,
  exitTime: bars,
  side: 'LONG',
  entryPrice: 100,
  exitPrice: 100,
  pnl: 0,
  pnlPct: 0,
  bars,
});

const fakeCandidate = (barsList: number[], netReturn: number): BacktestResult => ({
  trades: barsList.map(fakeTrade),
  equity: [],
  metrics: { ...zeroMetrics(), netReturn },
});

const noCosts = { feePct: 0, slipPct: 0 };

describe('planRandomTrades', () => {
  it('places back-to-back trades from the segment start when rand is 0', () => {
    // durations all pool[0] = 3, all gap weights 0 -> entries at 0 and 4.
    expect(planRandomTrades(() => 0, 0, 20, [3], 2)).toEqual([
      { entryIdx: 0, exitIdx: 3 },
      { entryIdx: 4, exitIdx: 7 },
    ]);
  });

  it('clips a trade that overruns the segment and drops trades with no room', () => {
    expect(planRandomTrades(() => 0, 0, 9, [50], 1)).toEqual([{ entryIdx: 0, exitIdx: null }]);
    // three 4-bar holds need 15 slots in a 10-bar segment: two fit, third drops.
    expect(planRandomTrades(() => 0, 0, 9, [4], 3)).toEqual([
      { entryIdx: 0, exitIdx: 4 },
      { entryIdx: 5, exitIdx: 9 },
    ]);
  });

  it('keeps seeded trades sorted, non-overlapping, and inside the segment', () => {
    const rand = mulberry32(123);
    for (let round = 0; round < 20; round++) {
      const planned = planRandomTrades(rand, 10, 79, [2, 5, 9], 4);
      let prevExit = 9; // one bar before the segment start
      for (const t of planned) {
        expect(t.entryIdx).toBeGreaterThan(prevExit);
        expect(t.entryIdx).toBeGreaterThanOrEqual(10);
        expect(t.entryIdx).toBeLessThanOrEqual(79);
        if (t.exitIdx != null) {
          expect(t.exitIdx - t.entryIdx === 2 || t.exitIdx - t.entryIdx === 5 || t.exitIdx - t.entryIdx === 9).toBe(true);
          expect(t.exitIdx).toBeLessThanOrEqual(79);
          prevExit = t.exitIdx;
        } else {
          expect(t).toBe(planned[planned.length - 1]); // only the last trade clips
          prevExit = 79;
        }
      }
    }
  });
});

describe('runRandomEntryBenchmark', () => {
  const candles = rising(60);
  const base = {
    candles,
    interval: '1h',
    costs: noCosts,
    candidateResult: fakeCandidate([2, 4], 0.05),
    seed: 42,
    runs: 40,
  };

  it('is deterministic for the same seed and differs across seeds', () => {
    const a = runRandomEntryBenchmark(base);
    const b = runRandomEntryBenchmark(base);
    const c = runRandomEntryBenchmark({ ...base, seed: 43 });
    expect(a).toEqual(b);
    expect(a.netReturns).not.toEqual(c.netReturns);
    expect(a.netReturns).toHaveLength(40);
  });

  it('defaults to the recorded run count when runs is omitted', () => {
    const { runs } = runRandomEntryBenchmark({ ...base, runs: undefined });
    expect(runs).toBe(DEFAULT_RANDOM_ENTRY_RUNS);
  });

  it('ranks the candidate against the simulated distribution (strict beat)', () => {
    const high = runRandomEntryBenchmark({
      ...base,
      candidateResult: fakeCandidate([2, 4], 10),
    });
    const low = runRandomEntryBenchmark({
      ...base,
      candidateResult: fakeCandidate([2, 4], -0.99),
    });
    expect(high.candidatePercentile).toBe(100); // beats every random long in a rising market
    expect(low.candidatePercentile).toBe(0);
    expect(high.candidateNetReturn).toBe(10);
  });

  it('applies the inherited costs to every simulated run (paired, same seed)', () => {
    const free = runRandomEntryBenchmark(base);
    const paid = runRandomEntryBenchmark({ ...base, costs: { feePct: 0.5, slipPct: 0.1 } });
    for (let i = 0; i < free.netReturns.length; i++) {
      expect(paid.netReturns[i]).toBeLessThan(free.netReturns[i]);
    }
  });

  it('fails closed on invalid input', () => {
    expect(() => runRandomEntryBenchmark({ ...base, runs: 0 })).toThrow(RangeError);
    expect(() => runRandomEntryBenchmark({ ...base, runs: 1001 })).toThrow(RangeError);
    expect(() => runRandomEntryBenchmark({ ...base, runs: 1.5 })).toThrow(RangeError);
    expect(() => runRandomEntryBenchmark({ ...base, seed: -1 })).toThrow(RangeError);
    expect(() => runRandomEntryBenchmark({ ...base, seed: 0.5 })).toThrow(RangeError);
    expect(() => runRandomEntryBenchmark({ ...base, candles: [] })).toThrow(RangeError);
    expect(() => runRandomEntryBenchmark({ ...base, from: 5, to: 2 })).toThrow(RangeError);
    expect(() =>
      runRandomEntryBenchmark({ ...base, candidateResult: fakeCandidate([], 0) }),
    ).toThrow(/closed candidate trade/);
  });

  it('ranks a real candidate end to end over sample data', () => {
    const series = toCoreCandles(makeSampleCandles({ seed: 42, count: 300 }));
    const candidate = runParamsBacktest({
      candles: series,
      strat: defaultStrategy(),
      interval: '1h',
    });
    expect(candidate.trades.length).toBeGreaterThan(0);
    const bench = runRandomEntryBenchmark({
      candles: series,
      interval: '1h',
      costs: { feePct: 0.05, slipPct: 0.02 },
      candidateResult: candidate,
      seed: 7,
      runs: 30,
    });
    expect(bench.netReturns).toHaveLength(30);
    expect(bench.candidatePercentile).toBeGreaterThanOrEqual(0);
    expect(bench.candidatePercentile).toBeLessThanOrEqual(100);
    for (const r of bench.netReturns) expect(Number.isFinite(r)).toBe(true);
  });
});
