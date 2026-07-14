import { describe, expect, it } from 'vitest';
import {
  runBacktest,
  type BacktestConfig,
  type Candle,
  type Direction,
  type FillMode,
  type RiskModel,
  type Signals,
} from './index';

const START = 10_000;

function bar(t: number, o: number, h: number, l: number, c: number): Candle {
  return { t, o, h, l, c, v: 1 };
}

function signals(count: number, entries: number[], exits: number[]): Signals {
  const entry = new Array<boolean>(count).fill(false);
  const exit = new Array<boolean>(count).fill(false);
  for (const i of entries) entry[i] = true;
  for (const i of exits) exit[i] = true;
  return { entry, exit };
}

function config(args: {
  direction?: Direction;
  fillMode?: FillMode;
  slippagePct?: number;
  risk?: RiskModel;
} = {}): BacktestConfig {
  return {
    exec: {
      direction: args.direction ?? 'long',
      sizingPct: 1,
      fillMode: args.fillMode ?? 'close',
    },
    cost: { feePct: 0, slippagePct: args.slippagePct ?? 0 },
    risk: args.risk,
    barsPerYear: 365,
    startEquity: START,
  };
}

describe('runBacktest — execution-bar timing', () => {
  it('executes nextOpen entry and exit on the following bars without future equity leakage', () => {
    const candles = [
      bar(0, 100, 105, 95, 101),
      bar(1, 110, 115, 105, 111),
      bar(2, 120, 125, 115, 121),
      bar(3, 130, 135, 125, 131),
    ];
    const result = runBacktest(candles, signals(4, [0], [2]), config({ fillMode: 'nextOpen' }));

    expect(result.equity[0].equity).toBe(START);
    expect(result.equity[2].equity).toBeCloseTo(START * (121 / 110), 12);
    expect(result.trades).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({
      entryTime: 1,
      exitTime: 3,
      entryPrice: 110,
      exitPrice: 130,
      bars: 2,
    });
  });

  it('does not fill a final-bar nextOpen entry when no execution candle exists', () => {
    const result = runBacktest(
      [bar(0, 100, 101, 99, 100)],
      signals(1, [0], []),
      config({ fillMode: 'nextOpen' }),
    );

    expect(result.trades).toHaveLength(0);
    expect(result.equity[0].equity).toBe(START);
    expect(result.metrics.netReturn).toBe(0);
  });

  it('settles a final-bar nextOpen exit at EOD close with normal exit slippage', () => {
    const candles = [
      bar(0, 100, 101, 99, 100),
      bar(1, 100, 101, 99, 100),
      bar(2, 90, 92, 78, 80),
    ];
    const result = runBacktest(
      candles,
      signals(3, [0], [2]),
      config({ fillMode: 'nextOpen', slippagePct: 0.01 }),
    );

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0]).toMatchObject({
      entryTime: 1,
      exitTime: 2,
      entryPrice: 101,
      exitPrice: 79.2,
      bars: 1,
    });
  });
});

describe('runBacktest — gap-aware risk fills', () => {
  const cases: Array<{
    name: string;
    direction: Direction;
    risk: RiskModel;
    riskBar: Candle;
    expectedEntry: number;
    expectedExit: number;
  }> = [
    {
      name: 'long stop gaps down and sells at slipped open',
      direction: 'long',
      risk: { stopLossPct: 0.1 },
      riskBar: bar(1, 80, 85, 75, 82),
      expectedEntry: 101,
      expectedExit: 79.2,
    },
    {
      name: 'long target gaps up and sells at slipped open',
      direction: 'long',
      risk: { takeProfitPct: 0.1 },
      riskBar: bar(1, 120, 125, 115, 122),
      expectedEntry: 101,
      expectedExit: 118.8,
    },
    {
      name: 'short stop gaps up and buys at slipped open',
      direction: 'short',
      risk: { stopLossPct: 0.1 },
      riskBar: bar(1, 120, 125, 115, 122),
      expectedEntry: 99,
      expectedExit: 121.2,
    },
    {
      name: 'short target gaps down and buys at slipped open',
      direction: 'short',
      risk: { takeProfitPct: 0.1 },
      riskBar: bar(1, 80, 85, 75, 82),
      expectedEntry: 99,
      expectedExit: 80.8,
    },
  ];

  it.each(cases)('$name', ({ direction, risk, riskBar, expectedEntry, expectedExit }) => {
    const result = runBacktest(
      [bar(0, 100, 100, 100, 100), riskBar],
      signals(2, [0], []),
      config({ direction, risk, slippagePct: 0.01 }),
    );

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0].entryPrice).toBeCloseTo(expectedEntry, 12);
    expect(result.trades[0].exitPrice).toBeCloseTo(expectedExit, 12);
    expect(result.trades[0].exitTime).toBe(1);
  });

  it('chooses the conservative stop when one candle touches SL and TP', () => {
    const result = runBacktest(
      [bar(0, 100, 100, 100, 100), bar(1, 100, 120, 80, 100)],
      signals(2, [0], []),
      config({ risk: { stopLossPct: 0.1, takeProfitPct: 0.1 }, slippagePct: 0.01 }),
    );

    expect(result.trades).toHaveLength(1);
    expect(result.trades[0].exitPrice).toBeCloseTo(89.991, 12);
  });
});

describe('runBacktest — EOD exit slippage', () => {
  it.each([
    { direction: 'long' as const, entryPrice: 101, exitPrice: 99 },
    { direction: 'short' as const, entryPrice: 99, exitPrice: 101 },
  ])('$direction closes with the correct side of slippage', ({ direction, entryPrice, exitPrice }) => {
    const result = runBacktest(
      [bar(0, 100, 100, 100, 100)],
      signals(1, [0], []),
      config({ direction, slippagePct: 0.01 }),
    );

    expect(result.trades[0].entryPrice).toBe(entryPrice);
    expect(result.trades[0].exitPrice).toBe(exitPrice);
  });
});
