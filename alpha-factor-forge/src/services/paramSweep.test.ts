import { describe, it, expect } from 'vitest';
import type { Candle } from '../core/backtest';
import type { Metrics } from '../core/metrics';
import { defaultStrategy, type ParamsStrategy } from './strategy';
import { toCoreCandles } from './candleAdapter';
import { makeSampleCandles } from './sampleData';
import {
  buildAxisValues,
  countSweepCombos,
  sweepMetricValue,
  runParamSweep,
  SWEEP_MAX_COMBOS,
  type SweepConfig,
} from './paramSweep';

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

/** Synthetic series long enough to exercise indicator periods and trade. */
const candles: Candle[] = toCoreCandles(makeSampleCandles({ count: 300, seed: 5 }));
const base: ParamsStrategy = {
  ...defaultStrategy(),
  entrySig: 'maCrossUp',
  exitSig: 'maCrossDown',
  feePct: 0,
  slipPct: 0,
};

describe('buildAxisValues', () => {
  it('builds an inclusive stepped range', () => {
    expect(buildAxisValues(5, 20, 3)).toEqual([5, 8, 11, 14, 17, 20]);
  });

  it('returns [min] when max < min', () => {
    expect(buildAxisValues(10, 4, 2)).toEqual([10]);
  });

  it('treats a 0/negative step as 1', () => {
    expect(buildAxisValues(1, 4, 0)).toEqual([1, 2, 3, 4]);
    expect(buildAxisValues(1, 4, -1)).toEqual([1, 2, 3, 4]);
  });

  it('absorbs float drift (no 0.30000000000000004 cells)', () => {
    expect(buildAxisValues(0, 0.3, 0.1)).toEqual([0, 0.1, 0.2, 0.3]);
  });

  it('caps an axis at 64 values', () => {
    expect(buildAxisValues(1, 1000, 1)).toHaveLength(64);
  });
});

describe('countSweepCombos', () => {
  it('multiplies the two axis lengths (1 row when 1-D)', () => {
    expect(countSweepCombos({ x: { key: 'fastMA', min: 5, max: 9, step: 1 }, metric: 'net' })).toBe(5);
    expect(
      countSweepCombos({
        x: { key: 'fastMA', min: 5, max: 9, step: 1 },
        y: { key: 'slowMA', min: 20, max: 23, step: 1 },
        metric: 'net',
      }),
    ).toBe(20);
  });
});

describe('sweepMetricValue', () => {
  it('selects the right scalar and stores dd as -maxDrawdown', () => {
    const m: Metrics = { ...zeroMetrics(), netReturn: 0.4, sharpe: 1.2, winRate: 0.6, maxDrawdown: 0.25 };
    expect(sweepMetricValue(m, 'net')).toBe(0.4);
    expect(sweepMetricValue(m, 'sharpe')).toBe(1.2);
    expect(sweepMetricValue(m, 'winRate')).toBe(0.6);
    expect(sweepMetricValue(m, 'dd')).toBe(-0.25);
  });

  it('guards non-finite profitFactor/calmar (Infinity -> 99, else 0)', () => {
    expect(sweepMetricValue({ ...zeroMetrics(), profitFactor: Infinity }, 'pf')).toBe(99);
    expect(sweepMetricValue({ ...zeroMetrics(), profitFactor: NaN }, 'pf')).toBe(0);
    expect(sweepMetricValue({ ...zeroMetrics(), calmar: Infinity }, 'calmar')).toBe(99);
  });
});

describe('runParamSweep', () => {
  it('produces a 1-row grid for a 1-D sweep, one cell per x', () => {
    const sweep: SweepConfig = { x: { key: 'fastMA', min: 5, max: 9, step: 1 }, metric: 'net' };
    const res = runParamSweep({ candles, strat: base, interval: '1h', sweep });

    expect(res.xs).toEqual([5, 6, 7, 8, 9]);
    expect(res.ys).toEqual([null]);
    expect(res.grid).toHaveLength(1);
    expect(res.grid[0]).toHaveLength(5);
    expect(res.grid[0].map((c) => c.x)).toEqual([5, 6, 7, 8, 9]);
    expect(res.grid[0].every((c) => c.y === null)).toBe(true);
  });

  it('produces a 2-D grid (rows = ys, cols = xs)', () => {
    const sweep: SweepConfig = {
      x: { key: 'fastMA', min: 5, max: 8, step: 1 },
      y: { key: 'slowMA', min: 20, max: 22, step: 1 },
      metric: 'sharpe',
    };
    const res = runParamSweep({ candles, strat: base, interval: '1h', sweep });

    expect(res.xs).toEqual([5, 6, 7, 8]);
    expect(res.ys).toEqual([20, 21, 22]);
    expect(res.grid).toHaveLength(3);
    expect(res.grid.every((row) => row.length === 4)).toBe(true);
    // each cell carries its own (x, y) coordinate
    expect(res.grid[1][2]).toMatchObject({ x: 7, y: 21 });
  });

  it('best is the highest-metric cell that actually traded, and lies on the grid', () => {
    const sweep: SweepConfig = { x: { key: 'fastMA', min: 3, max: 12, step: 1 }, metric: 'net' };
    const res = runParamSweep({ candles, strat: base, interval: '1h', sweep });

    expect(res.best).not.toBeNull();
    const best = res.best!;
    expect(best.trades).toBeGreaterThan(0);
    // no traded cell beats best
    const maxTraded = Math.max(
      ...res.grid[0].filter((c) => c.trades > 0 && c.metric != null).map((c) => c.metric as number),
    );
    expect(best.metric).toBe(maxTraded);
    expect(res.xs).toContain(best.x);
  });

  it('best is null when no combo trades', () => {
    // an entry condition that can never be true => zero trades everywhere
    const noTrade: ParamsStrategy = {
      ...base,
      mode: 'blocks',
      entryRules: [{ l: 'price', op: '>', r: '' }], // blank operand never fires
      exitRules: [],
    };
    const sweep: SweepConfig = { x: { key: 'fastMA', min: 5, max: 8, step: 1 }, metric: 'net' };
    const res = runParamSweep({ candles, strat: noTrade, interval: '1h', sweep });

    expect(res.grid[0].every((c) => c.trades === 0)).toBe(true);
    expect(res.best).toBeNull();
  });

  it('keeps lo <= hi and finite even with no trades', () => {
    const sweep: SweepConfig = { x: { key: 'fastMA', min: 5, max: 7, step: 1 }, metric: 'net' };
    const res = runParamSweep({ candles, strat: base, interval: '1h', sweep });
    expect(Number.isFinite(res.lo)).toBe(true);
    expect(Number.isFinite(res.hi)).toBe(true);
    expect(res.lo).toBeLessThanOrEqual(res.hi);
  });

  it('is deterministic (identical inputs -> identical result)', () => {
    const sweep: SweepConfig = { x: { key: 'fastMA', min: 4, max: 10, step: 2 }, metric: 'sharpe' };
    const a = runParamSweep({ candles, strat: base, interval: '1h', sweep });
    const b = runParamSweep({ candles, strat: base, interval: '1h', sweep });
    expect(a).toEqual(b);
  });

  it('throws when combos exceed the cap', () => {
    const sweep: SweepConfig = {
      x: { key: 'fastMA', min: 1, max: 40, step: 1 }, // 40
      y: { key: 'slowMA', min: 1, max: 40, step: 1 }, // 40 -> 1600 > 256
      metric: 'net',
    };
    expect(() => runParamSweep({ candles, strat: base, interval: '1h', sweep })).toThrow(/上限/);
    expect(SWEEP_MAX_COMBOS).toBe(256);
  });

  it('rejects a 2-D sweep that varies the same param on both axes', () => {
    // x.key === y.key would let the y assignment overwrite x, producing a grid
    // whose (x, y) coordinates lie about which value was actually backtested.
    const sweep: SweepConfig = {
      x: { key: 'fastMA', min: 5, max: 8, step: 1 },
      y: { key: 'fastMA', min: 5, max: 8, step: 1 },
      metric: 'net',
    };
    expect(() => runParamSweep({ candles, strat: base, interval: '1h', sweep })).toThrow(RangeError);
    expect(() => runParamSweep({ candles, strat: base, interval: '1h', sweep })).toThrow(/X \/ Y/);
  });

  it('rejects an empty y-axis range (non-finite bound)', () => {
    const sweep: SweepConfig = {
      x: { key: 'fastMA', min: 5, max: 8, step: 1 },
      y: { key: 'slowMA', min: NaN, max: 22, step: 1 },
      metric: 'net',
    };
    expect(() => runParamSweep({ candles, strat: base, interval: '1h', sweep })).toThrow(/Y 軸/);
  });

  it('honors a [from, to] sub-range (sweep in-sample only)', () => {
    const sweep: SweepConfig = { x: { key: 'fastMA', min: 5, max: 7, step: 1 }, metric: 'net' };
    const full = runParamSweep({ candles, strat: base, interval: '1h', sweep });
    const partial = runParamSweep({ candles, strat: base, interval: '1h', sweep, from: 0, to: 150 });
    // a shorter window generally yields different metrics than the full range
    expect(partial.grid[0].map((c) => c.metric)).not.toEqual(full.grid[0].map((c) => c.metric));
  });
});
