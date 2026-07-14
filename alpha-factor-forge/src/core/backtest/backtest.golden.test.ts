import { describe, expect, it } from 'vitest';
import { toCoreCandles } from '../../services/candleAdapter';
import { toExecCostFractions } from '../../services/backtestRunner';
import { makeSampleCandles } from '../../services/sampleData';
import {
  runBacktest,
  type BacktestConfig,
  type BacktestResult,
  type Candle,
  type Signals,
} from './index';

// Behaviour lock only: these expectations record CURRENT engine output. They
// are not an endorsement that the execution assumptions are financially
// correct. See docs/engine-parity-report.md before changing any value here.
const GOLDEN_CANDLES = toCoreCandles(makeSampleCandles({ seed: 42, count: 300 }));

function alternatingSignals(count: number): Signals {
  const entry = new Array<boolean>(count).fill(false);
  const exit = new Array<boolean>(count).fill(false);
  for (let i = 5; i < count; i += 20) entry[i] = true;
  for (let i = 12; i < count; i += 20) exit[i] = true;
  return { entry, exit };
}

const GOLDEN_SIGNALS = alternatingSignals(GOLDEN_CANDLES.length);
const BASE_CONFIG: BacktestConfig = {
  exec: { direction: 'long', sizingPct: 1, fillMode: 'close' },
  cost: { feePct: 0.0005, slippagePct: 0.0002 },
  barsPerYear: 8_760,
  startEquity: 10_000,
};

interface GoldenExpectation {
  tradeCount: number;
  first: { entryTime: number; exitTime: number; entryPrice: number; exitPrice: number };
  last: { entryTime: number; exitTime: number; entryPrice: number; exitPrice: number };
  netReturn: number;
  maxDrawdown: number;
  sharpe: number;
}

const GOLDEN_CASES: Array<{ name: string; config: BacktestConfig; expected: GoldenExpectation }> = [
  {
    name: 'long / close fill / no SLTP',
    config: BASE_CONFIG,
    expected: {
      tradeCount: 15,
      first: {
        entryTime: 1_704_085_200_000,
        exitTime: 1_704_110_400_000,
        entryPrice: 102.09385618029492,
        exitPrice: 100.48625138913827,
      },
      last: {
        entryTime: 1_705_093_200_000,
        exitTime: 1_705_118_400_000,
        entryPrice: 115.46141284576223,
        exitPrice: 117.15940225611321,
      },
      netReturn: -0.0010048472611153825,
      maxDrawdown: 0.05395966184288516,
      sharpe: 0.044244685223664024,
    },
  },
  {
    name: 'long / nextOpen fill',
    config: { ...BASE_CONFIG, exec: { ...BASE_CONFIG.exec, fillMode: 'nextOpen' } },
    expected: {
      tradeCount: 15,
      first: {
        entryTime: 1_704_088_800_000,
        exitTime: 1_704_114_000_000,
        entryPrice: 102.09385618029492,
        exitPrice: 100.48625138913827,
      },
      last: {
        entryTime: 1_705_096_800_000,
        exitTime: 1_705_122_000_000,
        entryPrice: 115.46141284576223,
        exitPrice: 117.15940225611321,
      },
      netReturn: -0.0010048472611153825,
      maxDrawdown: 0.05395966184288516,
      sharpe: 0.043369168149892924,
    },
  },
  {
    name: 'both / close fill / SL 2% / TP 4%',
    config: {
      ...BASE_CONFIG,
      exec: { ...BASE_CONFIG.exec, direction: 'both' },
      risk: { stopLossPct: 0.02, takeProfitPct: 0.04 },
    },
    expected: {
      tradeCount: 15,
      first: {
        entryTime: 1_704_085_200_000,
        exitTime: 1_704_106_800_000,
        entryPrice: 102.09385618029492,
        exitPrice: 100.03196866087768,
      },
      last: {
        entryTime: 1_705_093_200_000,
        exitTime: 1_705_118_400_000,
        entryPrice: 115.46141284576223,
        exitPrice: 117.15940225611321,
      },
      netReturn: -0.004775387411850351,
      maxDrawdown: 0.05325022427784083,
      sharpe: -0.3412716253585222,
    },
  },
  {
    name: 'short / close fill / no SLTP',
    config: { ...BASE_CONFIG, exec: { ...BASE_CONFIG.exec, direction: 'short' } },
    expected: {
      tradeCount: 15,
      first: {
        entryTime: 1_704_085_200_000,
        exitTime: 1_704_110_400_000,
        entryPrice: 102.05302680369812,
        exitPrice: 100.52645393020215,
      },
      last: {
        entryTime: 1_705_093_200_000,
        exitTime: 1_705_118_400_000,
        entryPrice: 115.41523751568994,
        exitPrice: 117.20627539164276,
      },
      netReturn: -0.04319001591392513,
      maxDrawdown: 0.06500526765109116,
      sharpe: -4.285603138121173,
    },
  },
];

function assertFiniteResult(result: BacktestResult): void {
  expect(result.equity.length).toBeGreaterThan(0);
  expect(result.equity.every((point) => Number.isFinite(point.equity))).toBe(true);
  expect(Number.isFinite(result.metrics.netReturn)).toBe(true);
  expect(Number.isFinite(result.metrics.maxDrawdown)).toBe(true);
  expect(Number.isFinite(result.metrics.sharpe)).toBe(true);
  for (let i = 0; i < result.trades.length; i++) {
    const trade = result.trades[i];
    expect(trade.entryTime).toBeLessThanOrEqual(trade.exitTime);
    expect(Number.isFinite(trade.entryPrice)).toBe(true);
    expect(Number.isFinite(trade.exitPrice)).toBe(true);
    if (i > 0) expect(trade.entryTime).toBeGreaterThanOrEqual(result.trades[i - 1].exitTime);
  }
}

describe('runBacktest — golden behaviour lock', () => {
  it.each(GOLDEN_CASES)('$name', ({ config, expected }) => {
    const result = runBacktest(GOLDEN_CANDLES, GOLDEN_SIGNALS, config);
    const first = result.trades[0];
    const last = result.trades[result.trades.length - 1];

    expect(result.trades).toHaveLength(expected.tradeCount);
    if (!first || !last) throw new Error('golden case must produce first and last trades');
    expect(first.entryTime).toBe(expected.first.entryTime);
    expect(first.exitTime).toBe(expected.first.exitTime);
    expect(first.entryPrice).toBeCloseTo(expected.first.entryPrice, 9);
    expect(first.exitPrice).toBeCloseTo(expected.first.exitPrice, 9);
    expect(last.entryTime).toBe(expected.last.entryTime);
    expect(last.exitTime).toBe(expected.last.exitTime);
    expect(last.entryPrice).toBeCloseTo(expected.last.entryPrice, 9);
    expect(last.exitPrice).toBeCloseTo(expected.last.exitPrice, 9);
    expect(result.metrics.netReturn).toBeCloseTo(expected.netReturn, 6);
    expect(result.metrics.maxDrawdown).toBeCloseTo(expected.maxDrawdown, 6);
    expect(result.metrics.sharpe).toBeCloseTo(expected.sharpe, 6);
    assertFiniteResult(result);
  });
});

describe('runBacktest — boundary behaviour lock', () => {
  const flatCandles = (count: number): Candle[] =>
    Array.from({ length: count }, (_, i) => ({ t: i, o: 100, h: 100, l: 100, c: 100, v: 1 }));
  const frictionless: BacktestConfig = {
    exec: { direction: 'long', sizingPct: 1, fillMode: 'close' },
    cost: { feePct: 0, slippagePct: 0 },
    barsPerYear: 365,
    startEquity: 10_000,
  };

  it('processes entry and exit signals on the same bar without invalid output', () => {
    const candles = flatCandles(3);
    const result = runBacktest(candles, { entry: [false, true, false], exit: [false, true, false] }, frictionless);

    assertFiniteResult(result);
    expect(result.trades).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({ entryTime: 1, exitTime: 2, bars: 1 });
  });

  it('handles a one-candle dataset', () => {
    const result = runBacktest(flatCandles(1), { entry: [true], exit: [false] }, frictionless);

    assertFiniteResult(result);
    expect(result.equity).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({ entryTime: 0, exitTime: 0, bars: 0 });
  });

  it('handles from equal to to', () => {
    const result = runBacktest(
      flatCandles(3),
      { entry: [false, true, false], exit: [false, false, false] },
      { ...frictionless, from: 1, to: 1 },
    );

    assertFiniteResult(result);
    expect(result.equity).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({ entryTime: 1, exitTime: 1, bars: 0 });
  });

  it('uses the legacy 100% fallback when UI sizePct is zero', () => {
    const converted = toExecCostFractions({ feePct: 0, slipPct: 0, sizePct: 0 });
    const candles = flatCandles(1);
    const signals = { entry: [true], exit: [false] };
    const result = runBacktest(
      candles,
      signals,
      { ...frictionless, exec: { ...frictionless.exec, sizingPct: converted.sizingPct } },
    );
    const expected = runBacktest(candles, signals, frictionless);

    expect(converted.sizingPct).toBe(1);
    assertFiniteResult(result);
    expect(result.trades).toHaveLength(1);
    expect(result).toEqual(expected);
  });

  it('clamps negative UI fee and slippage to zero before running the engine', () => {
    const converted = toExecCostFractions({ feePct: -1, slipPct: -5, sizePct: 100 });
    const candles = flatCandles(2);
    const signals = { entry: [true, false], exit: [false, true] };
    const result = runBacktest(candles, signals, {
      ...frictionless,
      cost: { feePct: converted.feePct, slippagePct: converted.slippagePct },
    });
    const expected = runBacktest(candles, signals, frictionless);

    expect(converted).toMatchObject({ feePct: 0, slippagePct: 0 });
    assertFiniteResult(result);
    expect(result).toEqual(expected);
  });
});
