import { describe, expect, it } from 'vitest';
import { runBacktest, type BacktestConfig, type Candle, type Direction, type Signals } from './index';

const START = 10_000;
const FEE = 0.01;

function candles(closes: number[]): Candle[] {
  return closes.map((c, i) => ({ t: i, o: c, h: c, l: c, c, v: 1 }));
}

function roundTripSignals(count: number, entries: number[], exits: number[]): Signals {
  const entry = new Array<boolean>(count).fill(false);
  const exit = new Array<boolean>(count).fill(false);
  for (const i of entries) entry[i] = true;
  for (const i of exits) exit[i] = true;
  return { entry, exit };
}

function config(direction: Direction, sizingPct = 1): BacktestConfig {
  return {
    exec: { direction, sizingPct, fillMode: 'close' },
    cost: { feePct: FEE, slippagePct: 0 },
    barsPerYear: 365,
    startEquity: START,
  };
}

function expectSettledReconciliation(result: ReturnType<typeof runBacktest>): void {
  const finalEquity = result.equity[result.equity.length - 1].equity;
  const totalTradePnl = result.trades.reduce((sum, trade) => sum + trade.pnl, 0);
  expect(finalEquity).toBeCloseTo(START + totalTradePnl, 9);
  expect(result.metrics.netReturn).toBeCloseTo(finalEquity / START - 1, 12);
}

describe('runBacktest — fee-inclusive accounting contract', () => {
  it('reconciles a hand-calculated long round trip', () => {
    // Entry budget includes its fee: 10,000 / 1.01 = 9,900.990099 notional.
    // Gross +990.099009; fees 99.009901 + 108.910891; net +782.178218.
    const result = runBacktest(candles([100, 110]), roundTripSignals(2, [0], [1]), config('long'));

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0].pnl).toBeCloseTo(782.178217821782, 9);
    expect(result.trades[0].pnlPct).toBeCloseTo(0.079, 12);
    expect(result.equity[0].equity).toBeCloseTo(9_900.9900990099, 9);
    expect(result.equity[1].equity).toBeCloseTo(10_782.1782178218, 9);
    expect(result.metrics.maxDrawdown).toBeCloseTo(0.00990099009901, 12);
    expectSettledReconciliation(result);
  });

  it('reconciles a hand-calculated 1x collateral short round trip', () => {
    // Same fee-inclusive entry budget; price falls 100 -> 90.
    // Gross +990.099009; fees 99.009901 + 89.108911; net +801.980198.
    const result = runBacktest(candles([100, 90]), roundTripSignals(2, [0], [1]), config('short'));

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0].pnl).toBeCloseTo(801.980198019802, 9);
    expect(result.trades[0].pnlPct).toBeCloseTo(0.081, 12);
    expect(result.equity[1].equity).toBeCloseTo(10_801.9801980198, 9);
    expectSettledReconciliation(result);
  });

  it('reconciles a losing short without manufacturing or losing collateral', () => {
    const result = runBacktest(candles([100, 110]), roundTripSignals(2, [0], [1]), config('short'));

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0].pnl).toBeCloseTo(-1_198.0198019802, 9);
    expect(result.trades[0].pnlPct).toBeCloseTo(-0.121, 12);
    expect(result.equity[1].equity).toBeCloseTo(8_801.9801980198, 9);
    expectSettledReconciliation(result);
  });

  it('charges both fees on a flat EOD close and settles the final equity point', () => {
    const result = runBacktest(candles([100]), roundTripSignals(1, [0], []), config('long'));

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({ entryTime: 0, exitTime: 0, bars: 0 });
    expect(result.trades[0].pnl).toBeCloseTo(-198.019801980198, 9);
    expect(result.trades[0].pnlPct).toBeCloseTo(-0.02, 12);
    expect(result.equity[0].equity).toBeCloseTo(9_801.9801980198, 9);
    expect(result.metrics.maxDrawdown).toBeCloseTo(0.01980198019802, 12);
    expectSettledReconciliation(result);
  });

  it('keeps unused cash outside a partial-size position', () => {
    const result = runBacktest(candles([100, 110]), roundTripSignals(2, [0], [1]), config('long', 0.5));

    expect(result.trades[0].pnl).toBeCloseTo(391.089108910891, 9);
    expect(result.trades[0].pnlPct).toBeCloseTo(0.079, 12);
    expect(result.equity[1].equity).toBeCloseTo(10_391.0891089109, 9);
    expectSettledReconciliation(result);
  });

  it('reconciles multiple fee-inclusive trades to final equity', () => {
    const result = runBacktest(
      candles([100, 110, 100, 90]),
      roundTripSignals(4, [0, 2], [1, 3]),
      config('long'),
    );

    expect(result.trades).toHaveLength(2);
    expectSettledReconciliation(result);
  });
});
