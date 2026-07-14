import { describe, expect, it } from 'vitest';
import { runBacktest, type BacktestConfig, type Candle, type FillMode, type Signals } from './index';

const START = 10_000;

function bars(values: Array<{ open: number; close?: number }>): Candle[] {
  return values.map(({ open, close = open }, t) => ({
    t,
    o: open,
    h: Math.max(open, close),
    l: Math.min(open, close),
    c: close,
    v: 1,
  }));
}

function signals(count: number, entries: number[], exits: number[]): Signals {
  const entry = new Array<boolean>(count).fill(false);
  const exit = new Array<boolean>(count).fill(false);
  for (const i of entries) entry[i] = true;
  for (const i of exits) exit[i] = true;
  return { entry, exit };
}

function config(fillMode: FillMode = 'close'): BacktestConfig {
  return {
    exec: { direction: 'both', sizingPct: 1, fillMode },
    cost: { feePct: 0, slippagePct: 0 },
    barsPerYear: 365,
    startEquity: START,
  };
}

describe('runBacktest — both direction reversal contract', () => {
  it('reverses long to short and back to long on close fills', () => {
    const candles = bars([
      { open: 100 },
      { open: 110 },
      { open: 90 },
      { open: 120 },
    ]);
    const result = runBacktest(candles, signals(4, [0, 2], [1]), config());

    expect(result.trades).toHaveLength(3);
    expect(result.trades.map((trade) => ({
      side: trade.side,
      entryTime: trade.entryTime,
      exitTime: trade.exitTime,
      entryPrice: trade.entryPrice,
      exitPrice: trade.exitPrice,
      bars: trade.bars,
    }))).toEqual([
      { side: 'LONG', entryTime: 0, exitTime: 1, entryPrice: 100, exitPrice: 110, bars: 1 },
      { side: 'SHORT', entryTime: 1, exitTime: 2, entryPrice: 110, exitPrice: 90, bars: 1 },
      { side: 'LONG', entryTime: 2, exitTime: 3, entryPrice: 90, exitPrice: 120, bars: 1 },
    ]);
  });

  it('reverses at the following execution open in nextOpen mode', () => {
    const candles = bars([
      { open: 100, close: 101 },
      { open: 110, close: 111 },
      { open: 90, close: 91 },
      { open: 120, close: 121 },
      { open: 130, close: 131 },
    ]);
    const result = runBacktest(candles, signals(5, [0, 2], [1]), config('nextOpen'));

    expect(result.trades).toHaveLength(3);
    expect(result.trades.map((trade) => ({
      side: trade.side,
      entryTime: trade.entryTime,
      exitTime: trade.exitTime,
      entryPrice: trade.entryPrice,
      exitPrice: trade.exitPrice,
      bars: trade.bars,
    }))).toEqual([
      { side: 'LONG', entryTime: 1, exitTime: 2, entryPrice: 110, exitPrice: 90, bars: 1 },
      { side: 'SHORT', entryTime: 2, exitTime: 3, entryPrice: 90, exitPrice: 120, bars: 1 },
      { side: 'LONG', entryTime: 3, exitTime: 4, entryPrice: 120, exitPrice: 131, bars: 1 },
    ]);
  });

  it('gives entry precedence when entry and exit are both true', () => {
    const candles = bars([{ open: 100 }, { open: 110 }, { open: 120 }]);
    const result = runBacktest(candles, signals(3, [0, 1], [1]), config());

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({
      side: 'LONG',
      entryTime: 0,
      exitTime: 2,
      entryPrice: 100,
      exitPrice: 120,
      bars: 2,
    });
  });

  it('opens short from flat and retains it on a repeated exit signal', () => {
    const candles = bars([{ open: 100 }, { open: 90 }, { open: 80 }]);
    const result = runBacktest(candles, signals(3, [], [0, 1]), config());

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({
      side: 'SHORT',
      entryTime: 0,
      exitTime: 2,
      entryPrice: 100,
      exitPrice: 80,
      bars: 2,
    });
  });

  it('does not reverse a final-bar nextOpen signal beyond the tested range', () => {
    const candles = bars([
      { open: 100, close: 101 },
      { open: 110, close: 111 },
      { open: 90, close: 91 },
    ]);
    const result = runBacktest(candles, signals(3, [0], [2]), config('nextOpen'));

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({
      side: 'LONG',
      entryTime: 1,
      exitTime: 2,
      entryPrice: 110,
      exitPrice: 91,
      bars: 1,
    });
  });
});

describe('runBacktest — normalized fraction validation', () => {
  const candles = bars([{ open: 100 }]);
  const noSignals = signals(1, [], []);
  const base = config();

  const invalidCases: Array<{
    name: string;
    mutate: (value: number) => BacktestConfig;
    value: number;
    field: string;
  }> = [
    {
      name: 'negative sizing',
      mutate: (value) => ({ ...base, exec: { ...base.exec, sizingPct: value } }),
      value: -0.01,
      field: 'exec.sizingPct',
    },
    {
      name: 'sizing above one',
      mutate: (value) => ({ ...base, exec: { ...base.exec, sizingPct: value } }),
      value: 1.01,
      field: 'exec.sizingPct',
    },
    {
      name: 'non-finite sizing',
      mutate: (value) => ({ ...base, exec: { ...base.exec, sizingPct: value } }),
      value: Number.NaN,
      field: 'exec.sizingPct',
    },
    {
      name: 'negative fee',
      mutate: (value) => ({ ...base, cost: { ...base.cost, feePct: value } }),
      value: -0.01,
      field: 'cost.feePct',
    },
    {
      name: 'fee above one',
      mutate: (value) => ({ ...base, cost: { ...base.cost, feePct: value } }),
      value: 1.01,
      field: 'cost.feePct',
    },
    {
      name: 'non-finite fee',
      mutate: (value) => ({ ...base, cost: { ...base.cost, feePct: value } }),
      value: Number.POSITIVE_INFINITY,
      field: 'cost.feePct',
    },
    {
      name: 'negative slippage',
      mutate: (value) => ({ ...base, cost: { ...base.cost, slippagePct: value } }),
      value: -0.01,
      field: 'cost.slippagePct',
    },
    {
      name: 'slippage above one',
      mutate: (value) => ({ ...base, cost: { ...base.cost, slippagePct: value } }),
      value: 1.01,
      field: 'cost.slippagePct',
    },
    {
      name: 'non-finite slippage',
      mutate: (value) => ({ ...base, cost: { ...base.cost, slippagePct: value } }),
      value: Number.NEGATIVE_INFINITY,
      field: 'cost.slippagePct',
    },
    {
      name: 'negative stop loss',
      mutate: (value) => ({ ...base, risk: { stopLossPct: value } }),
      value: -0.01,
      field: 'risk.stopLossPct',
    },
    {
      name: 'zero stop loss',
      mutate: (value) => ({ ...base, risk: { stopLossPct: value } }),
      value: 0,
      field: 'risk.stopLossPct',
    },
    {
      name: 'stop loss above one',
      mutate: (value) => ({ ...base, risk: { stopLossPct: value } }),
      value: 1.01,
      field: 'risk.stopLossPct',
    },
    {
      name: 'non-finite stop loss',
      mutate: (value) => ({ ...base, risk: { stopLossPct: value } }),
      value: Number.NaN,
      field: 'risk.stopLossPct',
    },
    {
      name: 'negative take profit',
      mutate: (value) => ({ ...base, risk: { takeProfitPct: value } }),
      value: -0.01,
      field: 'risk.takeProfitPct',
    },
    {
      name: 'zero take profit',
      mutate: (value) => ({ ...base, risk: { takeProfitPct: value } }),
      value: 0,
      field: 'risk.takeProfitPct',
    },
    {
      name: 'take profit above one',
      mutate: (value) => ({ ...base, risk: { takeProfitPct: value } }),
      value: 1.01,
      field: 'risk.takeProfitPct',
    },
    {
      name: 'non-finite take profit',
      mutate: (value) => ({ ...base, risk: { takeProfitPct: value } }),
      value: Number.POSITIVE_INFINITY,
      field: 'risk.takeProfitPct',
    },
  ];

  it.each(invalidCases)('rejects $name', ({ mutate, value, field }) => {
    const runInvalidConfig = () => runBacktest(candles, noSignals, mutate(value));
    expect(runInvalidConfig).toThrow(RangeError);
    expect(runInvalidConfig).toThrow(`BacktestConfig.${field}`);
  });

  it('accepts inclusive zero/one boundaries for sizing and costs', () => {
    expect(() => runBacktest(candles, noSignals, {
      ...base,
      exec: { ...base.exec, sizingPct: 0 },
      cost: { feePct: 1, slippagePct: 0 },
    })).not.toThrow();
  });

  it('accepts one as the maximum active risk fraction', () => {
    expect(() => runBacktest(candles, noSignals, {
      ...base,
      risk: { stopLossPct: 1, takeProfitPct: 1 },
    })).not.toThrow();
  });
});
