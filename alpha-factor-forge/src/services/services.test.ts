import { describe, it, expect } from 'vitest';
import type { Candle } from '../core/backtest';
import type { Metrics } from '../core/metrics';
import { buildParamsSignals, buildBlocksSignals, buildSignals } from './strategySignals';
import { runParamsBacktest, barsPerYear, toExecCostFractions } from './backtestRunner';
import { metricsToBacktestSummary } from './metricsMapper';
import { defaultStrategy, type ParamsStrategy } from './strategy';
import { toCoreCandles } from './candleAdapter';
import { makeSampleCandles } from './sampleData';

/** Build flat candles from a close series (only closes matter for these tests). */
const mk = (closes: number[]): Candle[] =>
  closes.map((c, i) => ({ t: i, o: c, h: c, l: c, c, v: 1 }));

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

describe('buildParamsSignals', () => {
  it('maCrossUp fires only at the hand-verified cross index', () => {
    // sma2 / sma3 over this series cross (fast above slow) exactly at i=4.
    const candles = mk([10, 8, 6, 8, 10, 12]);
    const strat: ParamsStrategy = {
      ...defaultStrategy(),
      fastMA: 2,
      slowMA: 3,
      entrySig: 'maCrossUp',
      exitSig: 'maCrossDown',
    };
    const { entry, exit } = buildParamsSignals(candles, strat);
    expect(entry).toEqual([false, false, false, false, true, false]);
    expect(exit).toEqual([false, false, false, false, false, false]);
  });

  it('never signals on the first bar (needs a previous bar)', () => {
    const candles = mk([1, 2, 3, 4]);
    const strat: ParamsStrategy = { ...defaultStrategy(), fastMA: 2, slowMA: 3 };
    expect(buildParamsSignals(candles, strat).entry[0]).toBe(false);
  });

  it('throws a clear error for stoch signals (not yet in core)', () => {
    const candles = mk([1, 2, 3, 4, 5]);
    const strat: ParamsStrategy = { ...defaultStrategy(), entrySig: 'stochOversold' };
    expect(() => buildParamsSignals(candles, strat)).toThrow(/stoch/i);
  });
});

describe('barsPerYear', () => {
  it('maps known intervals and falls back to daily', () => {
    expect(barsPerYear('1h')).toBe(8760);
    expect(barsPerYear('1d')).toBe(365);
    expect(barsPerYear('nonsense')).toBe(365);
  });
});

describe('toExecCostFractions (legacy clamp)', () => {
  it('converts normal percent units to fractions', () => {
    expect(toExecCostFractions({ feePct: 0.05, slipPct: 0.02, sizePct: 100 })).toEqual({
      feePct: 0.0005,
      slippagePct: 0.0002,
      sizingPct: 1,
    });
  });

  it('clamps negative fee/slip to zero (no rebate)', () => {
    const f = toExecCostFractions({ feePct: -1, slipPct: -5, sizePct: 50 });
    expect(f.feePct).toBe(0);
    expect(f.slippagePct).toBe(0);
  });

  it('treats sizePct 0 as the 100% fallback and caps above 100', () => {
    expect(toExecCostFractions({ feePct: 0, slipPct: 0, sizePct: 0 }).sizingPct).toBe(1);
    expect(toExecCostFractions({ feePct: 0, slipPct: 0, sizePct: 150 }).sizingPct).toBe(1);
  });

  it('floors a tiny sizePct at 0.01', () => {
    expect(toExecCostFractions({ feePct: 0, slipPct: 0, sizePct: 0.5 }).sizingPct).toBe(0.01);
  });
});

describe('runParamsBacktest', () => {
  const uptrend = mk([10, 9, 8, 9, 10, 11, 12, 13, 14, 15]);
  const base: ParamsStrategy = {
    ...defaultStrategy(),
    fastMA: 2,
    slowMA: 3,
    entrySig: 'maCrossUp',
    exitSig: 'maCrossDown',
  };

  it('runs the pipeline and returns aligned equity + metrics', () => {
    const res = runParamsBacktest({ candles: uptrend, strat: { ...base, feePct: 0, slipPct: 0 }, interval: '1h' });
    expect(res.equity.length).toBe(uptrend.length);
    expect(res.metrics.tradeCount).toBe(res.trades.length);
    expect(res.trades.length).toBeGreaterThanOrEqual(1);
  });

  it('applies legacy percent -> fraction conversion (fees reduce return)', () => {
    const noFee = runParamsBacktest({ candles: uptrend, strat: { ...base, feePct: 0, slipPct: 0 }, interval: '1h' });
    const withFee = runParamsBacktest({ candles: uptrend, strat: { ...base, feePct: 1, slipPct: 0 }, interval: '1h' });
    expect(withFee.metrics.netReturn).toBeLessThan(noFee.metrics.netReturn);
  });
});

describe('metricsToBacktestSummary', () => {
  it('maps camelCase metrics onto snake_case columns with default segment', () => {
    const m: Metrics = { ...zeroMetrics(), netReturn: 0.5, maxDrawdown: 0.2, winRate: 0.6, tradeCount: 8, profitFactor: 2.5 };
    const s = metricsToBacktestSummary(m, { strategyId: 1, datasetId: 2, startTime: 0, endTime: 100 });
    expect(s.segment).toBe('full');
    expect(s.strategy_id).toBe(1);
    expect(s.dataset_id).toBe(2);
    expect(s.net_return).toBe(0.5);
    expect(s.max_drawdown).toBe(0.2);
    expect(s.win_rate).toBe(0.6);
    expect(s.trade_count).toBe(8);
    expect(s.profit_factor).toBe(2.5);
  });

  it('coerces non-finite metrics to null and respects an explicit segment', () => {
    const m: Metrics = { ...zeroMetrics(), profitFactor: Infinity };
    const s = metricsToBacktestSummary(m, { strategyId: 1, datasetId: 2, segment: 'train', startTime: 0, endTime: 1 });
    expect(s.profit_factor).toBeNull();
    expect(s.segment).toBe('train');
  });
});

describe('toCoreCandles', () => {
  it('maps persisted candle fields to the core short shape', () => {
    const core = toCoreCandles([
      { timestamp: 5, open: 1, high: 2, low: 0.5, close: 1.5, volume: 9 },
    ]);
    expect(core).toEqual([{ t: 5, o: 1, h: 2, l: 0.5, c: 1.5, v: 9 }]);
  });
});

describe('makeSampleCandles', () => {
  it('is deterministic for a given seed and well-formed', () => {
    const a = makeSampleCandles({ count: 50, seed: 7 });
    const b = makeSampleCandles({ count: 50, seed: 7 });
    expect(a).toEqual(b);
    expect(a.length).toBe(50);
    // OHLC invariants + strictly increasing timestamps.
    for (let i = 0; i < a.length; i++) {
      expect(a[i].high).toBeGreaterThanOrEqual(a[i].low);
      expect(a[i].high).toBeGreaterThanOrEqual(Math.max(a[i].open, a[i].close));
      expect(a[i].low).toBeLessThanOrEqual(Math.min(a[i].open, a[i].close));
      if (i > 0) expect(a[i].timestamp).toBeGreaterThan(a[i - 1].timestamp);
    }
  });

  it('runs end to end through the backtest pipeline', () => {
    const candles = toCoreCandles(makeSampleCandles({ count: 300, seed: 1 }));
    const res = runParamsBacktest({ candles, strat: defaultStrategy(), interval: '1h' });
    expect(res.equity.length).toBe(candles.length);
    expect(Number.isFinite(res.metrics.netReturn)).toBe(true);
  });
});

describe('buildBlocksSignals', () => {
  it('a maFast crossUp maSlow rule matches the params signal', () => {
    const candles = mk([10, 8, 6, 8, 10, 12]);
    const strat: ParamsStrategy = {
      ...defaultStrategy(),
      mode: 'blocks',
      fastMA: 2,
      slowMA: 3,
      entryRules: [{ l: 'maFast', op: 'crossUp', r: 'maSlow' }],
      exitRules: [{ l: 'maFast', op: 'crossDown', r: 'maSlow' }],
    };
    const { entry, exit } = buildBlocksSignals(candles, strat);
    expect(entry).toEqual([false, false, false, false, true, false]);
    expect(exit.every((v) => v === false)).toBe(true);
  });

  it('ANDs all rules (a contradictory pair never fires) and ignores bar 0', () => {
    const t: ParamsStrategy = {
      ...defaultStrategy(),
      mode: 'blocks',
      entryRules: [{ l: 'price', op: '>', r: '0' }, { l: 'price', op: '<', r: '0' }],
      exitRules: [],
    };
    expect(buildBlocksSignals(mk([1, 2, 3, 4]), t).entry.every((v) => v === false)).toBe(true);

    const single: ParamsStrategy = { ...defaultStrategy(), mode: 'blocks', entryRules: [{ l: 'price', op: '>', r: '0' }], exitRules: [] };
    const e = buildBlocksSignals(mk([1, 2, 3, 4]), single).entry;
    expect(e[0]).toBe(false);
    expect(e.slice(1).every((v) => v === true)).toBe(true);
  });

  it('an empty rule list never fires', () => {
    const t: ParamsStrategy = { ...defaultStrategy(), mode: 'blocks', entryRules: [], exitRules: [] };
    const { entry, exit } = buildBlocksSignals(mk([1, 2, 3]), t);
    expect(entry.every((v) => !v)).toBe(true);
    expect(exit.every((v) => !v)).toBe(true);
  });
});

describe('buildSignals dispatch', () => {
  const candles = mk([10, 8, 6, 8, 10, 12]);
  it('routes by mode', () => {
    const p: ParamsStrategy = { ...defaultStrategy(), fastMA: 2, slowMA: 3 };
    const b: ParamsStrategy = {
      ...defaultStrategy(),
      mode: 'blocks',
      fastMA: 2,
      slowMA: 3,
      entryRules: [{ l: 'maFast', op: 'crossUp', r: 'maSlow' }],
      exitRules: [{ l: 'maFast', op: 'crossDown', r: 'maSlow' }],
    };
    expect(buildSignals(candles, p)).toEqual(buildParamsSignals(candles, p));
    expect(buildSignals(candles, b)).toEqual(buildBlocksSignals(candles, b));
  });
});
