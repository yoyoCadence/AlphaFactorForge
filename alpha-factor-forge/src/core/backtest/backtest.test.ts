// FULL — backtest determinism + hashing stability tests. Run: npm test
import { describe, it, expect } from 'vitest';
import { runBacktest, type Candle, type Signals, type BacktestConfig } from './index';
import { strategyHashSync, datasetHash, canonicalize } from '../hashing';

function synthCandles(n: number): Candle[] {
  const out: Candle[] = [];
  let price = 100;
  for (let i = 0; i < n; i++) {
    // deterministic pseudo-walk (no Math.random)
    price += Math.sin(i / 5) * 1.5 + Math.cos(i / 11) * 0.8;
    const o = price;
    const c = price + Math.sin(i / 3);
    const h = Math.max(o, c) + 1;
    const l = Math.min(o, c) - 1;
    out.push({ t: i * 60_000, o, h, l, c, v: 1000 + i });
  }
  return out;
}

function altSignals(n: number): Signals {
  const entry = new Array(n).fill(false);
  const exit = new Array(n).fill(false);
  for (let i = 5; i < n; i += 20) entry[i] = true;
  for (let i = 12; i < n; i += 20) exit[i] = true;
  return { entry, exit };
}

const cfg: BacktestConfig = {
  exec: { direction: 'long', sizingPct: 1, fillMode: 'close' },
  cost: { feePct: 0.0005, slippagePct: 0.0002 },
  barsPerYear: 525600,
};

describe('runBacktest — deterministic', () => {
  it('same inputs -> identical output', () => {
    const candles = synthCandles(300);
    const sig = altSignals(300);
    const a = runBacktest(candles, sig, cfg);
    const b = runBacktest(candles, sig, cfg);
    expect(a.metrics.netReturn).toBe(b.metrics.netReturn);
    expect(a.trades.length).toBe(b.trades.length);
    expect(canonicalize(a.trades)).toBe(canonicalize(b.trades));
  });

  it('fees+slippage reduce return vs frictionless', () => {
    const candles = synthCandles(300);
    const sig = altSignals(300);
    const withCost = runBacktest(candles, sig, cfg);
    const noCost = runBacktest(candles, sig, {
      ...cfg,
      cost: { feePct: 0, slippagePct: 0 },
    });
    expect(withCost.metrics.netReturn).toBeLessThanOrEqual(noCost.metrics.netReturn);
  });

  it('produces an equity point per tested bar', () => {
    const candles = synthCandles(120);
    const r = runBacktest(candles, altSignals(120), cfg);
    expect(r.equity.length).toBe(120);
  });
});

describe('hashing — stable + order-independent', () => {
  it('strategyHashSync ignores key order', () => {
    const a = strategyHashSync({ a: 1, b: 2 }, { feePct: 0.1, slippagePct: 0.2 });
    const b = strategyHashSync({ b: 2, a: 1 }, { slippagePct: 0.2, feePct: 0.1 });
    expect(a).toBe(b);
  });

  it('datasetHash differs when bounds differ', async () => {
    const base = { exchange: 'binance', symbol: 'BTCUSDT', interval: '1h', startTime: 0, endTime: 100 };
    const h1 = await datasetHash(base);
    const h2 = await datasetHash({ ...base, endTime: 200 });
    expect(h1).not.toBe(h2);
  });
});
