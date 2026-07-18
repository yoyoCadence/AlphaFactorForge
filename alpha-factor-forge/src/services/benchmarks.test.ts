import { describe, it, expect } from 'vitest';
import type { Candle } from '../core/backtest';
import { runParamsBacktest } from './backtestRunner';
import { toCoreCandles } from './candleAdapter';
import { makeSampleCandles } from './sampleData';
import {
  DETERMINISTIC_BENCHMARK_IDS,
  benchmarkStrategy,
  runDeterministicBenchmarks,
} from './benchmarks';

/** Flat candles from a close series; time = index. */
const mk = (closes: number[]): Candle[] =>
  closes.map((c, i) => ({ t: i, o: c, h: c, l: c, c, v: 1 }));

const noCosts = { feePct: 0, slipPct: 0 };

describe('benchmarkStrategy', () => {
  it('locks the doc §6 definitions onto the shared execution model', () => {
    const sma = benchmarkStrategy('smaCross', { feePct: 0.05, slipPct: 0.02 });
    expect(sma).toMatchObject({
      fastMA: 50,
      slowMA: 200,
      entrySig: 'maCrossUp',
      exitSig: 'maCrossDown',
      mode: 'params',
      direction: 'long',
      fillMode: 'close',
      sizePct: 100,
      slPct: 0,
      tpPct: 0,
      feePct: 0.05,
      slipPct: 0.02,
    });
    expect(benchmarkStrategy('rsiReversion', noCosts)).toMatchObject({
      rsiPeriod: 14,
      rsiBuy: 30,
      rsiSell: 70,
      entrySig: 'rsiOversold',
      exitSig: 'rsiOverbought',
    });
    expect(benchmarkStrategy('bollingerReversion', noCosts)).toMatchObject({
      bbPeriod: 20,
      bbMult: 2,
      entrySig: 'bbLowerTouch',
      exitSig: 'bbUpperTouch',
    });
  });
});

describe('runDeterministicBenchmarks — buy & hold', () => {
  it('enters at the first tested close and settles at the segment end', () => {
    const candles = mk([100, 110, 121]);
    const [bh] = runDeterministicBenchmarks({ candles, interval: '1d', costs: noCosts });
    expect(bh.id).toBe('buyHold');
    expect(bh.strat).toBeNull();
    expect(bh.result.trades).toHaveLength(1);
    const trade = bh.result.trades[0];
    expect(trade.side).toBe('LONG');
    expect(trade.entryPrice).toBe(100);
    expect(trade.exitPrice).toBe(121);
    // all-in, no costs: net return = 121/100 - 1
    expect(bh.result.metrics.netReturn).toBeCloseTo(0.21, 10);
  });

  it('respects an explicit [from, to] segment', () => {
    const candles = mk([100, 110, 121, 133.1]);
    const [bh] = runDeterministicBenchmarks({
      candles,
      interval: '1d',
      costs: noCosts,
      from: 1,
      to: 2,
    });
    expect(bh.result.trades[0].entryPrice).toBe(110);
    expect(bh.result.trades[0].exitPrice).toBe(121);
  });

  it('pays the inherited candidate costs on entry and exit', () => {
    const candles = mk([100, 100, 100]);
    const [free] = runDeterministicBenchmarks({ candles, interval: '1d', costs: noCosts });
    const [paid] = runDeterministicBenchmarks({
      candles,
      interval: '1d',
      costs: { feePct: 0.1, slipPct: 0.05 },
    });
    expect(free.result.metrics.netReturn).toBeCloseTo(0, 10);
    expect(paid.result.metrics.netReturn).toBeLessThan(0);
  });
});

describe('runDeterministicBenchmarks — suite', () => {
  const candles = toCoreCandles(makeSampleCandles({ seed: 42, count: 600 }));
  const args = { candles, interval: '1h', costs: { feePct: 0.05, slipPct: 0.02 } };

  it('returns all four benchmarks in the fixed deterministic order', () => {
    const runs = runDeterministicBenchmarks(args);
    expect(runs.map((r) => r.id)).toEqual([...DETERMINISTIC_BENCHMARK_IDS]);
    for (const run of runs) {
      expect(Number.isFinite(run.result.metrics.netReturn)).toBe(true);
      expect(run.result.equity.length).toBe(candles.length);
    }
  });

  it('signal benchmarks match a direct pipeline run of the same strategy (parity)', () => {
    const runs = runDeterministicBenchmarks(args);
    for (const run of runs) {
      if (!run.strat) continue;
      expect(run.result).toEqual(
        runParamsBacktest({ candles, strat: run.strat, interval: args.interval }),
      );
    }
  });

  it('is deterministic for identical input', () => {
    expect(runDeterministicBenchmarks(args)).toEqual(runDeterministicBenchmarks(args));
  });

  it('fails closed on an empty candle series', () => {
    expect(() =>
      runDeterministicBenchmarks({ candles: [], interval: '1h', costs: noCosts }),
    ).toThrow(RangeError);
  });
});
